mod common;

use common::{
    app_of, docx, docx_with, item, p, read_part, read_part_of, rich, styled, table, text,
    type_keys, typed, write_temp,
};
use tuidoc::app::{App, Mode};
use tuidoc::document::Document;
use tuidoc::docx::saving::{package_bytes, save};
use tuidoc::docx::{self, DocxError};
use tuidoc::model::BlockPath;

const DOCUMENT: &str = "word/document.xml";

fn opened(path: &std::path::Path) -> App {
    let mut app = App::new(Document::open(path).unwrap());
    app.fit(60, 16);
    app
}

/// The document part after typing keys into a package and saving it.
fn saved(bytes: Vec<u8>, keys: &str) -> (String, Vec<u8>) {
    let app = typed(app_of(bytes), keys);
    let bytes = package_bytes(app.document()).unwrap();
    (read_part_of(&bytes, DOCUMENT).unwrap(), bytes)
}

#[test]
fn saving_without_changes_keeps_every_part_byte_for_byte() {
    let original = rich(&format!(
        "{}{}{}{}",
        styled("Heading1", "Title"),
        p("Body &amp; more"),
        item(1, 0, "bullet"),
        table(&[&["a", "b"]])
    ));
    let (_, bytes) = saved(original.clone(), "");
    for part in [
        DOCUMENT,
        "word/styles.xml",
        "word/numbering.xml",
        "word/footnotes.xml",
        "[Content_Types].xml",
    ] {
        assert_eq!(read_part_of(&bytes, part), read_part_of(&original, part));
    }
}

#[test]
fn an_edit_rewrites_only_the_paragraph_it_touched() {
    let (first, second, third) = (
        "<w:p w:rsidR=\"00A1\"><w:r><w:rPr><w:rFonts w:ascii=\"Arial\"/></w:rPr><w:t>one</w:t></w:r></w:p>",
        p("two"),
        "<w:p><w:r><w:drawing><wp:inline xmlns:wp=\"x\"><wp:docPr id=\"1\" name=\"Pic\"/></wp:inline></w:drawing></w:r></w:p>",
    );
    let original = docx(&format!("{first}{second}{third}"));
    let (xml, _) = saved(original, "jAX⎋");
    assert!(xml.contains(first));
    assert!(xml.contains(third));
    assert!(!xml.contains(&second));
    assert!(xml.contains("<w:p><w:r><w:t xml:space=\"preserve\">twoX</w:t></w:r></w:p>"));
}

#[test]
fn an_edited_paragraph_keeps_its_identifiers_formats_and_pictures() {
    let paragraph = "<w:p w:rsidR=\"00A1\"><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii=\"Arial\"/><w:color w:val=\"FF0000\"/></w:rPr><w:t>red</w:t></w:r><w:r><w:drawing><wp:inline xmlns:wp=\"x\"><wp:docPr id=\"1\" name=\"Pic\"/></wp:inline></w:drawing></w:r></w:p>";
    let (xml, _) = saved(docx(paragraph), "iX⎋");
    assert!(xml.contains("<w:p w:rsidR=\"00A1\"><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii=\"Arial\"/><w:color w:val=\"FF0000\"/></w:rPr><w:t xml:space=\"preserve\">Xred</w:t></w:r><w:r><w:drawing><wp:inline xmlns:wp=\"x\"><wp:docPr id=\"1\" name=\"Pic\"/></wp:inline></w:drawing></w:r></w:p>"));
}

#[test]
fn text_is_escaped_on_the_way_out() {
    let (xml, _) = saved(docx(&p("")), "ia < b & c⎋");
    assert!(xml.contains("a &lt; b &amp; c"));
}

#[test]
fn a_hyperlink_still_wraps_its_text_after_an_edit() {
    let body = r#"<w:p><w:r><w:t>See </w:t></w:r><w:hyperlink r:id="rId6"><w:r><w:t>site</w:t></w:r></w:hyperlink><w:r><w:t> now</w:t></w:r></w:p>"#;
    let (xml, _) = saved(rich(body), "wAX⎋0wwix⎋");
    assert!(xml.contains(
        r#"<w:hyperlink r:id="rId6"><w:r><w:t xml:space="preserve">site</w:t></w:r></w:hyperlink>"#
    ));
}

#[test]
fn bookmarks_and_fields_survive_an_edit_around_them() {
    let body = r#"<w:p><w:bookmarkStart w:id="0" w:name="b"/><w:r><w:t>Page </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>7</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:bookmarkEnd w:id="0"/></w:p>"#;
    let (xml, _) = saved(docx(body), "dw");
    for kept in [
        "<w:bookmarkStart w:id=\"0\" w:name=\"b\"/>",
        "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
        "<w:r><w:instrText> PAGE </w:instrText></w:r>",
        "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>",
        "<w:bookmarkEnd w:id=\"0\"/>",
    ] {
        assert!(xml.contains(kept), "{kept} is missing from {xml}");
    }
}

#[test]
fn spelling_marks_are_dropped_from_a_rewritten_paragraph() {
    let body = r#"<w:p><w:proofErr w:type="spellStart"/><w:r><w:t>teh</w:t></w:r><w:proofErr w:type="spellEnd"/></w:p>"#;
    let (xml, _) = saved(docx(body), "x");
    assert!(!xml.contains("proofErr"));
}

#[test]
fn an_edit_in_a_table_keeps_the_tables_own_properties() {
    let body = table(&[&["a", "b"]]);
    let (xml, _) = saved(docx(&body), "ix⎋");
    assert!(xml.contains("<w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/></w:tblPr><w:tblGrid>"));
    assert!(xml.contains("<w:tc><w:tcPr><w:tcW w:w=\"2000\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:t xml:space=\"preserve\">xa</w:t></w:r></w:p></w:tc>"));
    assert!(xml.contains(&format!(
        "<w:tc><w:tcPr><w:tcW w:w=\"2000\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>",
        p("b")
    )));
    docx::xml::parse(&xml).unwrap();
}

#[test]
fn what_is_saved_reads_back_as_what_was_on_screen() {
    let original = rich(&format!("{}{}", p("one two"), table(&[&["a"]])));
    let app = typed(app_of(original), "gbiwo new paragraph⎋ggg1");
    let bytes = package_bytes(app.document()).unwrap();
    let reopened = app_of(bytes);
    assert_eq!(text(&reopened), text(&app));
    let document = reopened.document();
    let first = document.paragraph(BlockPath::Top(0)).unwrap();
    assert_eq!(document.styles.heading_level(&first.props), Some(1));
    assert_eq!(first.cells()[0].props.flag("b"), Some(true));
}

#[test]
fn the_section_properties_stay_last_in_the_body() {
    let (xml, _) = saved(docx(&p("one")), "otwo⎋Gothree⎋");
    assert!(xml.ends_with(
        "<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr></w:body></w:document>"
    ));
}

#[test]
fn a_new_heading_style_is_written_into_the_styles_part() {
    let (xml, bytes) = saved(rich(&p("Title")), "g4");
    assert!(xml.contains("<w:pStyle w:val=\"Heading4\"/>"));
    let styles = read_part_of(&bytes, "word/styles.xml").unwrap();
    assert!(styles.contains("<w:style w:type=\"paragraph\" w:styleId=\"Heading4\"><w:name w:val=\"heading 4\"/><w:basedOn w:val=\"Normal\"/>"));
    assert!(styles.ends_with("</w:style></w:styles>"));
    docx::xml::parse(&styles).unwrap();
}

#[test]
fn a_document_without_styles_gets_a_styles_part_when_it_needs_one() {
    let (_, bytes) = saved(docx(&p("Title")), "g1");
    let styles = read_part_of(&bytes, "word/styles.xml").unwrap();
    assert!(styles.contains("w:styleId=\"Heading1\""));
    let types = read_part_of(&bytes, "[Content_Types].xml").unwrap();
    assert!(types.contains("<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>"));
    let rels = read_part_of(&bytes, "word/_rels/document.xml.rels").unwrap();
    assert!(rels.contains("relationships/styles\" Target=\"styles.xml\""));
    let reopened = docx::open(bytes).unwrap();
    let props = &reopened.stories[0]
        .paragraph(BlockPath::Top(0))
        .unwrap()
        .props;
    assert_eq!(reopened.styles.heading_level(props), Some(1));
}

#[test]
fn a_new_list_is_written_into_a_new_numbering_part() {
    let (xml, bytes) = saved(docx(&p("item")), "gl");
    assert!(xml.contains("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr>"));
    let numbering = read_part_of(&bytes, "word/numbering.xml").unwrap();
    docx::xml::parse(&numbering).unwrap();
    let definition = numbering.find("<w:abstractNum ").unwrap();
    let list = numbering.find("<w:num ").unwrap();
    assert!(definition < list);
    let types = read_part_of(&bytes, "[Content_Types].xml").unwrap();
    assert!(types.contains("/word/numbering.xml"));
    let rels = read_part_of(&bytes, "word/_rels/document.xml.rels").unwrap();
    assert!(rels.contains("relationships/numbering\" Target=\"numbering.xml\""));
}

#[test]
fn a_numbered_list_is_added_before_the_lists_already_there() {
    let numbering = common::part(
        "numbering",
        r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/><w:lvlText w:val="-"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
    );
    let rels = common::rels(&[("rId1", "numbering", "numbering.xml")]);
    let bytes = docx_with(
        &p("item"),
        &[
            ("word/_rels/document.xml.rels", &rels),
            ("word/numbering.xml", &numbering),
        ],
    );
    let (xml, bytes) = saved(bytes, "glgl");
    assert!(xml.contains("<w:numId w:val=\"2\"/>"));
    let numbering = read_part_of(&bytes, "word/numbering.xml").unwrap();
    let second_definition = numbering
        .find("<w:abstractNum w:abstractNumId=\"1\">")
        .unwrap();
    let first_list = numbering.find("<w:num w:numId=\"1\">").unwrap();
    let second_list = numbering.find("<w:num w:numId=\"2\">").unwrap();
    assert!(second_definition < first_list && first_list < second_list);
    docx::xml::parse(&numbering).unwrap();
}

#[test]
fn ctrl_s_saves_to_the_file_and_clears_the_unsaved_mark() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    type_keys(&mut app, "Atwo⎋");
    assert!(app.document().is_dirty());
    type_keys(&mut app, "⌃s");
    assert!(!app.document().is_dirty());
    assert_eq!(app.status(), "Saved report.docx");
    assert!(read_part(&file.path, DOCUMENT).unwrap().contains("onetwo"));
    type_keys(&mut app, "Athree⎋⌃s");
    assert!(
        read_part(&file.path, DOCUMENT)
            .unwrap()
            .contains("onetwothree")
    );
}

#[test]
fn ctrl_s_saves_while_typing_and_typing_goes_on() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    type_keys(&mut app, "Atwo⌃s");
    assert_eq!(app.mode(), Mode::Insert);
    assert!(!app.document().is_dirty());
    type_keys(&mut app, "!⎋");
    assert!(app.document().is_dirty());
    assert_eq!(text(&app), ["onetwo!"]);
}

#[test]
fn undoing_past_a_save_is_an_unsaved_change() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    type_keys(&mut app, "x⌃su");
    assert!(app.document().is_dirty());
    type_keys(&mut app, "⌃r");
    assert!(!app.document().is_dirty());
}

#[test]
fn a_file_changed_by_another_program_is_not_overwritten() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    let theirs = docx(&p("theirs"));
    std::fs::write(&file.path, &theirs).unwrap();
    type_keys(&mut app, "x⌃s");
    assert!(app.status_is_error());
    assert!(app.status().contains("changed on disk"));
    assert!(app.document().is_dirty());
    assert_eq!(std::fs::read(&file.path).unwrap(), theirs);
}

#[test]
fn a_document_with_no_file_asks_where_to_save() {
    let mut app = App::new(Document::blank().unwrap());
    type_keys(&mut app, "ihello⎋⌃s");
    assert_eq!(app.mode(), Mode::FileDialog);
}

#[test]
fn a_blank_document_saves_as_a_document_that_opens_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.docx");
    let mut document = Document::blank().unwrap();
    assert_eq!(document.file_name(), "[No Name]");
    assert!(matches!(
        save(&mut document, None),
        Err(DocxError::NoFileName)
    ));
    save(&mut document, Some(&path)).unwrap();
    assert_eq!(document.file_name(), "new.docx");
    let app = typed(opened(&path), "ihello⎋g1gl");
    assert_eq!(text(&app), ["hello"]);
}

#[test]
fn saving_and_quitting_does_both() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    type_keys(&mut app, "xZZ");
    assert!(app.should_quit());
    assert!(read_part(&file.path, DOCUMENT).unwrap().contains(">ne<"));
}

#[test]
fn choosing_to_save_before_quitting_saves_and_quits() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    type_keys(&mut app, "xqs");
    assert!(app.should_quit());
    assert!(read_part(&file.path, DOCUMENT).unwrap().contains(">ne<"));
}

#[test]
fn a_save_that_fails_does_not_go_on_to_quit() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    std::fs::write(&file.path, docx(&p("theirs"))).unwrap();
    type_keys(&mut app, "xqs");
    assert!(!app.should_quit());
    assert!(app.status_is_error());
}

#[test]
fn f5_reloads_the_file_from_disk() {
    let file = write_temp(&docx(&p("one")));
    let mut app = opened(&file.path);
    std::fs::write(&file.path, docx(&p("theirs"))).unwrap();
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::F(5),
    ));
    assert_eq!(text(&app), ["theirs"]);
}
