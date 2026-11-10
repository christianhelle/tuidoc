mod common;

use common::{docx, p, rich, table};
use tuidoc::docx;
use tuidoc::layout::Layout;
use tuidoc::model::{BlockPath, Position};

fn layout(bytes: Vec<u8>, width: u16) -> Layout {
    let opened = docx::open(bytes).unwrap();
    Layout::build(&opened.stories[0], &opened.styles, &opened.numbering, width)
}

fn lines(layout: &Layout) -> Vec<String> {
    layout
        .rows
        .iter()
        .flat_map(|row| &row.lines)
        .map(|line| {
            line.glyphs
                .iter()
                .map(|glyph| glyph.text.as_str())
                .collect()
        })
        .collect()
}

#[test]
fn a_paragraph_wraps_at_spaces() {
    let layout = layout(docx(&p("one two three four")), 9);
    assert_eq!(lines(&layout), ["one two ", "three ", "four"]);
}

#[test]
fn a_word_longer_than_the_line_is_broken() {
    let layout = layout(docx(&p("abcdefghij")), 4);
    assert_eq!(lines(&layout), ["abcd", "efgh", "ij"]);
}

#[test]
fn wide_characters_take_two_columns() {
    let layout = layout(docx(&p("日本語のテキスト")), 6);
    assert_eq!(lines(&layout), ["日本語", "のテキ", "スト"]);
}

#[test]
fn an_empty_paragraph_still_has_a_line_for_the_cursor() {
    let layout = layout(docx(&format!("{}{}", p(""), p("x"))), 20);
    assert_eq!(lines(&layout), ["", "x"]);
    assert_eq!(layout.row_of(Position::new(BlockPath::Top(1), 0)), 1);
}

#[test]
fn a_blank_row_separates_paragraphs_but_not_list_items() {
    let body = format!(
        "{}{}{}{}",
        p("one"),
        common::item(1, 0, "a"),
        common::item(1, 0, "b"),
        p("two")
    );
    let layout = layout(rich(&body), 20);
    let text: Vec<bool> = (0..layout.rows.len())
        .map(|row| layout.has_text(row))
        .collect();
    assert_eq!(text, [true, false, true, true, false, true]);
}

#[test]
fn every_place_in_a_wrapped_paragraph_is_found_on_its_line() {
    let layout = layout(docx(&p("one two three four")), 9);
    let at = |offset| Position::new(BlockPath::Top(0), offset);
    assert_eq!(layout.row_of(at(0)), 0);
    assert_eq!(layout.row_of(at(8)), 1);
    assert_eq!(layout.row_of(at(17)), 2);
    let (_, line) = layout.find(at(10)).unwrap();
    assert_eq!(line.column_of(10), 2);
    assert_eq!(layout.position_at(1, 2, false), Some(at(10)));
    // Past the end of a line lands on its last character, or after it when typing.
    assert_eq!(layout.position_at(2, 30, false), Some(at(17)));
    assert_eq!(layout.position_at(2, 30, true), Some(at(18)));
}

#[test]
fn a_tables_cells_are_laid_side_by_side() {
    let layout = layout(docx(&table(&[&["a", "b"]])), 40);
    assert_eq!(layout.rows.len(), 3);
    let row = &layout.rows[1];
    assert_eq!(row.lines.len(), 2);
    assert!(row.lines[0].left < row.lines[1].left);
    assert!(layout.rows[0].decor[0].1.starts_with('┌'));
    assert!(layout.rows[2].decor[0].1.ends_with('┘'));
}

#[test]
fn a_table_is_squeezed_into_a_narrow_pane() {
    let layout = layout(docx(&table(&[&["alpha", "beta", "gamma"]])), 16);
    let border = &layout.rows[0].decor[0].1;
    assert!(border.chars().count() <= 16, "{border}");
}
