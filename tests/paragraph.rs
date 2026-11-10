mod common;

use common::docx;
use tuidoc::docx;
use tuidoc::model::props::{PARAGRAPH_ORDER, RUN_ORDER};
use tuidoc::model::{AtomKind, BlockPath, Inline, Paragraph, Props};

fn read(xml: &str) -> Paragraph {
    docx::open(docx(xml)).unwrap().stories[0]
        .paragraph(BlockPath::Top(0))
        .unwrap()
        .clone()
}

/// "plain" then "bold" in bold then "tail".
fn mixed() -> Paragraph {
    read(
        "<w:p><w:r><w:t>plain</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t>tail</w:t></w:r></w:p>",
    )
}

fn bold_at(paragraph: &Paragraph, offset: usize) -> bool {
    paragraph.cells()[offset].props.flag("b") == Some(true)
}

#[test]
fn length_counts_characters_as_a_reader_sees_them() {
    let paragraph = read("<w:p><w:r><w:t>ae\u{301}🇩🇰</w:t><w:tab/></w:r></w:p>");
    assert_eq!(paragraph.len(), 4);
}

#[test]
fn typing_takes_the_format_of_what_comes_before() {
    let mut paragraph = mixed();
    paragraph.insert_text(9, "er");
    assert_eq!(paragraph.text(), "plainboldertail");
    assert!(bold_at(&paragraph, 9));
    assert!(bold_at(&paragraph, 10));
    assert!(!bold_at(&paragraph, 11));
    assert!(paragraph.dirty);
}

#[test]
fn typing_at_the_start_takes_the_format_of_what_follows() {
    let mut paragraph = read("<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r></w:p>");
    paragraph.insert_text(0, "very ");
    assert_eq!(paragraph.text(), "very bold");
    assert!(bold_at(&paragraph, 0));
}

#[test]
fn typing_into_an_empty_paragraph_makes_plain_text() {
    let mut paragraph = Paragraph::default();
    paragraph.insert_text(0, "hello");
    assert_eq!(paragraph.text(), "hello");
    assert_eq!(paragraph.len(), 5);
}

#[test]
fn deleting_across_formats_hands_back_what_was_taken() {
    let mut paragraph = mixed();
    let taken = paragraph.delete(3..11);
    assert_eq!(paragraph.text(), "plail");
    assert_eq!(taken.text(), "inboldta");
    assert_eq!(taken.len(), 8);
}

#[test]
fn deleting_merges_the_neighbours_left_in_the_same_format() {
    let mut paragraph = mixed();
    paragraph.delete(5..9);
    assert_eq!(paragraph.inlines.len(), 1);
    assert_eq!(paragraph.text(), "plaintail");
}

#[test]
fn deleting_leaves_bookmarks_and_field_ends_behind() {
    let mut paragraph = read(
        r#"<w:p><w:r><w:t>ab</w:t></w:r><w:bookmarkStart w:id="0" w:name="x"/><w:r><w:t>cd</w:t></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t>ef</w:t></w:r></w:p>"#,
    );
    paragraph.delete(1..5);
    assert_eq!(paragraph.text(), "af");
    let markers = paragraph
        .inlines
        .iter()
        .filter(|inline| matches!(inline, Inline::Marker(_)))
        .count();
    assert_eq!(markers, 2);
}

#[test]
fn a_copied_fragment_leaves_the_paragraph_alone() {
    let paragraph = mixed();
    let fragment = paragraph.fragment(5..9);
    assert_eq!(fragment.text(), "bold");
    assert_eq!(paragraph.text(), "plainboldtail");
    assert!(!paragraph.dirty);
}

#[test]
fn a_pasted_fragment_keeps_its_own_format() {
    let mut paragraph = mixed();
    let fragment = paragraph.fragment(5..9);
    paragraph.insert_fragment(0, &fragment);
    assert_eq!(paragraph.text(), "boldplainboldtail");
    assert!(bold_at(&paragraph, 0));
    assert!(!bold_at(&paragraph, 4));
}

#[test]
fn splitting_gives_the_second_half_the_same_style() {
    let mut paragraph =
        read("<w:p><w:pPr><w:pStyle w:val=\"Quote\"/></w:pPr><w:r><w:t>one two</w:t></w:r></w:p>");
    let rest = paragraph.split(3);
    assert_eq!(paragraph.text(), "one");
    assert_eq!(rest.text(), " two");
    assert_eq!(rest.props.val("pStyle").as_deref(), Some("Quote"));
    assert!(rest.source.is_none());
    assert!(rest.dirty && paragraph.dirty);
}

#[test]
fn a_section_break_goes_with_the_second_half_of_a_split() {
    let mut paragraph = read(
        "<w:p><w:pPr><w:sectPr><w:pgSz w:w=\"1\"/></w:sectPr></w:pPr><w:r><w:t>ab</w:t></w:r></w:p>",
    );
    let rest = paragraph.split(1);
    assert!(paragraph.props.get("sectPr").is_none());
    assert!(rest.props.get("sectPr").is_some());
}

#[test]
fn joining_appends_the_text_and_keeps_the_first_style() {
    let mut first =
        read("<w:p><w:pPr><w:pStyle w:val=\"Quote\"/></w:pPr><w:r><w:t>one</w:t></w:r></w:p>");
    let mut second = Paragraph::default();
    second.insert_text(0, " two");
    first.join(second);
    assert_eq!(first.text(), "one two");
    assert_eq!(first.props.val("pStyle").as_deref(), Some("Quote"));
    assert_eq!(first.inlines.len(), 1);
}

#[test]
fn reformatting_a_stretch_splits_the_text_at_its_edges() {
    let mut paragraph = read("<w:p><w:r><w:t>one two three</w:t></w:r></w:p>");
    paragraph.reformat(4..7, |props| props.set("b", "<w:b/>", RUN_ORDER));
    assert_eq!(paragraph.text(), "one two three");
    assert_eq!(paragraph.inlines.len(), 3);
    assert!(!bold_at(&paragraph, 3));
    assert!(bold_at(&paragraph, 4));
    assert!(bold_at(&paragraph, 6));
    assert!(!bold_at(&paragraph, 7));
}

#[test]
fn the_formats_over_a_stretch_are_reported() {
    let paragraph = mixed();
    assert_eq!(paragraph.formats(0..5).len(), 1);
    assert_eq!(paragraph.formats(4..6).len(), 2);
    assert_eq!(paragraph.formats(0..13).len(), 3);
}

#[test]
fn an_inserted_tab_is_one_place() {
    let mut paragraph = mixed();
    paragraph.insert_atom(5, AtomKind::Tab, "<w:tab/>");
    assert_eq!(paragraph.text(), "plain\tboldtail");
    assert_eq!(paragraph.len(), 14);
}

#[test]
fn a_detached_copy_owes_nothing_to_the_file() {
    let paragraph = read("<w:p w14:paraId=\"1\" xmlns:w14=\"x\"><w:r><w:t>a</w:t></w:r></w:p>");
    let copy = paragraph.detached();
    assert!(copy.source.is_none() && copy.dirty);
    assert_eq!(&*copy.open_tag, "<w:p>");
    assert_eq!(copy.text(), "a");
}

#[test]
fn a_new_run_property_goes_where_the_schema_wants_it() {
    let mut props = Props::default();
    props.push("rFonts", "<w:rFonts w:ascii=\"Arial\"/>");
    props.push("sz", "<w:sz w:val=\"24\"/>");
    props.set("b", "<w:b/>", RUN_ORDER);
    props.set("u", "<w:u w:val=\"single\"/>", RUN_ORDER);
    assert_eq!(
        props.xml(),
        "<w:rFonts w:ascii=\"Arial\"/><w:b/><w:sz w:val=\"24\"/><w:u w:val=\"single\"/>"
    );
}

#[test]
fn a_property_from_another_namespace_stays_last() {
    let mut props = Props::default();
    props.push("w14:ligatures", "<w14:ligatures w14:val=\"all\"/>");
    props.set("i", "<w:i/>", RUN_ORDER);
    assert_eq!(props.xml(), "<w:i/><w14:ligatures w14:val=\"all\"/>");
}

#[test]
fn setting_a_property_again_replaces_it() {
    let mut props = Props::default();
    props.set("jc", "<w:jc w:val=\"left\"/>", PARAGRAPH_ORDER);
    props.set("pStyle", "<w:pStyle w:val=\"A\"/>", PARAGRAPH_ORDER);
    props.set("jc", "<w:jc w:val=\"center\"/>", PARAGRAPH_ORDER);
    assert_eq!(
        props.xml(),
        "<w:pStyle w:val=\"A\"/><w:jc w:val=\"center\"/>"
    );
    props.remove("jc");
    assert_eq!(props.val("jc"), None);
}
