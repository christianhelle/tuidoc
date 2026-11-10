mod common;

use common::{app, colours_at, item, p, rich_app, screen, shot, styled, table, type_keys, typed};
use ratatui::style::Color;
use tuidoc::app::App;
use tuidoc::help;
use tuidoc::ui::{INSERT_HINTS, NORMAL_HINTS, VISUAL_HINTS};

/// A document with a bit of everything in it.
fn report() -> App {
    let footnote = "<w:p><w:r><w:t xml:space=\"preserve\">The quick brown fox jumps over the lazy dog and keeps running until the line wraps at the edge of the pane.</w:t></w:r><w:r><w:footnoteReference w:id=\"1\"/></w:r></w:p>";
    rich_app(&format!(
        "{}{}{}{}{}{}{}",
        styled("Title", "Quarterly report"),
        styled("Heading1", "Summary"),
        footnote,
        item(1, 0, "First bullet"),
        item(1, 1, "Nested bullet"),
        item(2, 0, "Numbered"),
        table(&[&["Region", "Sales"], &["North", "1,200"]]),
    ))
}

fn badge_of(app: &mut App) -> (Color, Color) {
    // The status line is the last row when the document has one story, and it has tabs here.
    colours_at(app, 1, 19)
}

#[test]
fn screen_in_normal_mode() {
    insta::assert_snapshot!(shot(&mut report()));
}

#[test]
fn screen_while_typing() {
    insta::assert_snapshot!(shot(&mut typed(report(), "jjA and more")));
}

#[test]
fn screen_with_the_outline_open() {
    insta::assert_snapshot!(shot(&mut typed(report(), "gOj")));
}

#[test]
fn screen_with_help() {
    insta::assert_snapshot!(screen(&mut typed(report(), "?"), 90, 24));
}

#[test]
fn screen_with_the_go_to_dialog() {
    insta::assert_snapshot!(shot(&mut typed(report(), "⌃g12")));
}

#[test]
fn screen_with_unsaved_changes_on_quitting() {
    insta::assert_snapshot!(shot(&mut typed(report(), "xq")));
}

#[test]
fn screen_with_a_footnote_looked_up() {
    insta::assert_snapshot!(shot(&mut typed(report(), "jj$K")));
}

#[test]
fn screen_with_the_style_picker() {
    insta::assert_snapshot!(shot(&mut typed(report(), "ghhead")));
}

#[test]
fn screen_of_the_footnotes_story() {
    insta::assert_snapshot!(shot(&mut typed(report(), "gtgt")));
}

#[test]
fn screen_of_a_table_with_merged_and_wrapped_cells() {
    let body = format!(
        "<w:tbl><w:tblGrid><w:gridCol w:w=\"1500\"/><w:gridCol w:w=\"3000\"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr>{}</w:tc></w:tr><w:tr><w:tc>{}</w:tc><w:tc>{}{}</w:tc></w:tr></w:tbl>",
        p("Spanning both columns"),
        p("Narrow cell with text that wraps"),
        p("First paragraph"),
        p("Second paragraph"),
    );
    insta::assert_snapshot!(shot(&mut app(&body)));
}

#[test]
fn screen_with_pictures_breaks_and_alignment() {
    let body = format!(
        "{}{}{}{}",
        r#"<w:p><w:r><w:t>Before</w:t></w:r><w:r><w:drawing><wp:inline xmlns:wp="x"><wp:docPr id="1" name="Picture 1" descr="A chart of sales"/></wp:inline></w:drawing></w:r><w:r><w:t>after</w:t><w:br/><w:t>next line</w:t><w:tab/><w:t>tabbed</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>To the right</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:ind w:left="1440"/></w:pPr><w:r><w:t>Indented an inch</w:t></w:r></w:p>"#,
    );
    insta::assert_snapshot!(shot(&mut app(&body)));
}

#[test]
fn the_title_shows_the_file_the_story_and_unsaved_changes() {
    let mut app = report();
    assert!(shot(&mut app).starts_with("┏ [No Name] — Body ━"));
    type_keys(&mut app, "x");
    assert!(shot(&mut app).starts_with("┏ [No Name]* — Body ━"));
}

#[test]
fn the_badge_takes_the_colour_of_the_mode() {
    let mut app = report();
    assert_eq!(badge_of(&mut app), (Color::Black, Color::Blue));
    type_keys(&mut app, "i");
    assert_eq!(badge_of(&mut app), (Color::Black, Color::Green));
    type_keys(&mut app, "⎋v");
    assert_eq!(badge_of(&mut app), (Color::Black, Color::Magenta));
}

#[test]
fn headings_are_cyan_and_note_references_yellow() {
    let mut app = report();
    // "Summary" on the fourth row, after the border, the title and a blank row.
    assert_eq!(colours_at(&mut app, 5, 3).0, Color::Cyan);
    let screen = shot(&mut app);
    let row = screen
        .lines()
        .position(|line| line.contains("[1]"))
        .unwrap();
    let column = screen
        .lines()
        .nth(row)
        .unwrap()
        .chars()
        .position(|c| c == '[')
        .unwrap();
    assert_eq!(
        colours_at(&mut app, column as u16, row as u16).0,
        Color::Yellow
    );
}

#[test]
fn an_error_is_shown_in_red() {
    let mut app = typed(report(), "/zebra⏎");
    let screen = shot(&mut app);
    assert!(screen.lines().last().unwrap().contains("Not found: zebra"));
    assert_eq!(colours_at(&mut app, 10, 19).0, Color::Red);
}

#[test]
fn the_search_is_typed_on_the_status_line() {
    let mut app = typed(report(), "/fox");
    assert!(
        shot(&mut app)
            .lines()
            .last()
            .unwrap()
            .starts_with(" NORMAL  /fox")
    );
}

#[test]
fn every_hint_fits_an_eighty_column_terminal() {
    for hints in [NORMAL_HINTS, VISUAL_HINTS, INSERT_HINTS] {
        let width = unicode_width::UnicodeWidthStr::width(hints);
        assert!(width + " NORMAL ".len() <= 80, "{hints} is too long");
    }
    let mut app = app(&p("text"));
    assert!(shot(&mut app).lines().last().unwrap().ends_with("q quit"));
}

#[test]
fn every_help_table_fits_its_dialog_and_a_small_terminal() {
    for table in [
        help::NORMAL_HELP,
        help::INSERT_HELP,
        help::VISUAL_HELP,
        help::OUTLINE_HELP,
    ] {
        assert!(table.len() <= help::HELP_ROWS);
        assert_eq!(table.last(), Some(&("", "Press any key to close")));
        for (keys, action) in table {
            let keys = unicode_width::UnicodeWidthStr::width(*keys);
            let action = unicode_width::UnicodeWidthStr::width(*action);
            assert!(keys < help::HELP_KEYS, "{action} has too many keys");
            assert!(help::HELP_KEYS + action <= usize::from(help::HELP_WIDTH) - 2);
        }
    }
}

#[test]
fn the_view_follows_the_cursor_down_a_long_document() {
    let body: String = (1..=40).map(|n| p(&format!("Paragraph {n}"))).collect();
    let mut app = typed(app(&body), "G");
    let screen = shot(&mut app);
    assert!(screen.contains("Paragraph 40"));
    assert!(!screen.contains("Paragraph 1 "));
    assert!(
        screen.lines().last().unwrap().ends_with("¶ 40/40") || screen.contains("40  Paragraph 40")
    );
}

#[test]
fn a_selection_is_drawn_reversed() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    let mut app = typed(app(&p("one two")), "ve");
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    tuidoc::ui::fit(&mut app, ratatui::layout::Rect::new(0, 0, 80, 20));
    terminal
        .draw(|frame| tuidoc::ui::draw(frame, &app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer[(5, 1)].modifier.contains(Modifier::REVERSED));
    assert!(buffer[(7, 1)].modifier.contains(Modifier::REVERSED));
    assert!(!buffer[(8, 1)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn a_tiny_terminal_does_not_panic() {
    let mut app = typed(report(), "gO");
    for (width, height) in [(1, 1), (5, 3), (20, 4), (0, 0)] {
        screen(&mut app, width.max(1), height.max(1));
    }
}
