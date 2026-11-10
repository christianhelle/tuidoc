mod common;

use common::{docx, item, p, rich, styled};
use tuidoc::docx;
use tuidoc::model::{BlockPath, Props};
use tuidoc::numbering::{ListItem, number};

fn props_of(opened: &docx::Docx, index: usize) -> &Props {
    &opened.stories[0]
        .paragraph(BlockPath::Top(index))
        .unwrap()
        .props
}

#[test]
fn a_heading_style_makes_its_text_bold() {
    let opened = docx::open(rich(&styled("Heading1", "Title"))).unwrap();
    let format = opened
        .styles
        .format(props_of(&opened, 0), &Props::default());
    assert!(format.bold);
    assert!(!format.italic);
}

#[test]
fn a_run_can_switch_off_what_its_style_switches_on() {
    let opened = docx::open(rich(&styled("Heading1", "Title"))).unwrap();
    let mut run = Props::default();
    run.push("b", "<w:b w:val=\"0\"/>");
    assert!(!opened.styles.format(props_of(&opened, 0), &run).bold);
}

#[test]
fn a_character_style_applies_to_its_run() {
    let opened = docx::open(rich(&p("text"))).unwrap();
    let mut run = Props::default();
    run.push("rStyle", "<w:rStyle w:val=\"Strong\"/>");
    assert!(opened.styles.format(props_of(&opened, 0), &run).bold);
}

#[test]
fn underline_none_is_not_underlined() {
    let opened = docx::open(rich(&p("text"))).unwrap();
    let mut run = Props::default();
    run.push("u", "<w:u w:val=\"none\"/>");
    assert!(!opened.styles.format(props_of(&opened, 0), &run).underline);
    let mut run = Props::default();
    run.push("u", "<w:u w:val=\"single\"/>");
    assert!(opened.styles.format(props_of(&opened, 0), &run).underline);
}

#[test]
fn heading_levels_come_from_the_outline_level() {
    let opened = docx::open(rich(&format!(
        "{}{}{}",
        styled("Heading1", "One"),
        styled("Heading2", "Two"),
        p("Plain")
    )))
    .unwrap();
    assert_eq!(opened.styles.heading_level(props_of(&opened, 0)), Some(1));
    assert_eq!(opened.styles.heading_level(props_of(&opened, 1)), Some(2));
    assert_eq!(opened.styles.heading_level(props_of(&opened, 2)), None);
}

#[test]
fn a_heading_is_known_by_its_name_whatever_its_id() {
    let opened = docx::open(rich(&styled("berschrift3", "Drei"))).unwrap();
    assert_eq!(opened.styles.heading_level(props_of(&opened, 0)), Some(3));
}

#[test]
fn the_title_style_is_recognised() {
    let opened = docx::open(rich(&styled("Title", "Report"))).unwrap();
    assert!(opened.styles.is_title(props_of(&opened, 0)));
}

#[test]
fn a_document_without_styles_has_plain_text() {
    let opened = docx::open(docx(&p("text"))).unwrap();
    let format = opened
        .styles
        .format(props_of(&opened, 0), &Props::default());
    assert!(!format.bold);
    assert_eq!(opened.styles.heading_level(props_of(&opened, 0)), None);
}

#[test]
fn styles_are_found_by_the_name_word_shows() {
    let opened = docx::open(rich(&p("text"))).unwrap();
    assert_eq!(opened.styles.named("heading 3").unwrap().id, "berschrift3");
    assert_eq!(opened.styles.normal(), Some("Normal"));
}

fn items(list: &[(u32, usize)]) -> Vec<Option<ListItem>> {
    list.iter()
        .map(|(id, depth)| (*id != 0).then(|| (id.to_string(), *depth)))
        .collect()
}

#[test]
fn numbered_lists_count_up_and_nest() {
    let opened = docx::open(rich(&item(2, 0, "x"))).unwrap();
    let labels = opened
        .numbering
        .labels(&items(&[(2, 0), (2, 1), (2, 1), (2, 2), (2, 0), (2, 1)]));
    let labels: Vec<&str> = labels
        .iter()
        .map(|label| label.as_deref().unwrap())
        .collect();
    assert_eq!(labels, ["1.", "a)", "b)", "i.", "2.", "a)"]);
}

#[test]
fn bullets_change_with_depth() {
    let opened = docx::open(rich(&item(1, 0, "x"))).unwrap();
    let labels = opened.numbering.labels(&items(&[(1, 0), (1, 1)]));
    assert_eq!(labels, [Some("•".to_string()), Some("◦".to_string())]);
}

#[test]
fn paragraphs_outside_any_list_have_no_label() {
    let opened = docx::open(rich(&item(1, 0, "x"))).unwrap();
    let labels = opened.numbering.labels(&items(&[(0, 0), (9, 0)]));
    assert_eq!(labels, [None, None]);
}

#[test]
fn finds_a_bulleted_and_a_numbered_list_to_reuse() {
    let opened = docx::open(rich(&p("x"))).unwrap();
    assert_eq!(opened.numbering.find(true), Some("1"));
    assert_eq!(opened.numbering.find(false), Some("2"));
    assert_eq!(opened.numbering.highest_ids(), (1, 2));
}

#[test]
fn numbers_are_written_in_letters_and_roman_numerals() {
    assert_eq!(number(3, "lowerLetter"), "c");
    assert_eq!(number(27, "upperLetter"), "AA");
    assert_eq!(number(14, "lowerRoman"), "xiv");
    assert_eq!(number(1990, "upperRoman"), "MCMXC");
    assert_eq!(number(12, "decimal"), "12");
}
