mod common;

use common::{document, docx, docx_with, item, p, package, rich, styled, table};
use tuidoc::docx::{self, DocxError};
use tuidoc::model::{AtomKind, Block, BlockPath, Inline, Merge, Paragraph, Story, StoryKind};

fn body(content: &str) -> Story {
    docx::open(docx(content)).unwrap().stories.remove(0)
}

fn paragraph(story: &Story, index: usize) -> &Paragraph {
    story.paragraph(BlockPath::Top(index)).unwrap()
}

#[test]
fn reads_paragraphs_as_their_text() {
    let story = body(&format!("{}{}", p("Hello"), p("World &amp; all")));
    assert_eq!(paragraph(&story, 0).text(), "Hello");
    assert_eq!(paragraph(&story, 1).text(), "World & all");
}

#[test]
fn a_paragraph_knows_exactly_where_it_came_from() {
    let story = body(&format!("{}{}", p("One"), p("Two")));
    let span = paragraph(&story, 1).source.clone().unwrap();
    assert_eq!(&story.source[span], p("Two"));
    assert!(!paragraph(&story, 1).dirty);
}

#[test]
fn the_head_and_tail_surround_the_blocks() {
    let story = body(&p("One"));
    assert!(story.source[story.head.clone()].ends_with("<w:body>"));
    assert!(story.source[story.tail.clone()].starts_with("</w:body>"));
}

#[test]
fn the_section_properties_are_kept_but_are_not_a_paragraph() {
    let story = body(&p("One"));
    assert_eq!(story.blocks.len(), 2);
    assert!(
        matches!(&story.blocks[1], Block::Opaque(opaque) if opaque.raw.starts_with("<w:sectPr"))
    );
    assert_eq!(story.paths(), vec![BlockPath::Top(0)]);
}

#[test]
fn a_run_with_a_tab_and_a_break_splits_into_text_and_atoms() {
    let story = body(
        "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t></w:r></w:p>",
    );
    let paragraph = paragraph(&story, 0);
    assert_eq!(paragraph.len(), 5);
    assert_eq!(paragraph.text(), "a\tb\nc");
    assert!(
        matches!(&paragraph.inlines[1], Inline::Atom(atom) if atom.kind == AtomKind::Tab && atom.props.flag("b") == Some(true))
    );
}

#[test]
fn a_page_break_is_told_from_a_line_break() {
    let story = body("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
    assert!(
        matches!(&paragraph(&story, 0).inlines[0], Inline::Atom(atom) if atom.kind == AtomKind::PageBreak)
    );
}

#[test]
fn a_picture_becomes_an_atom_with_its_description() {
    let story = body(
        r#"<w:p><w:r><w:drawing><wp:inline xmlns:wp="x"><wp:docPr id="1" name="Picture 1" descr="A chart of sales"/></wp:inline></w:drawing></w:r></w:p>"#,
    );
    assert!(
        matches!(&paragraph(&story, 0).inlines[0], Inline::Atom(atom) if atom.kind == AtomKind::Image("A chart of sales".into()))
    );
}

#[test]
fn a_picture_without_a_description_goes_by_its_name() {
    let story = body(
        r#"<w:p><w:r><w:drawing><wp:inline xmlns:wp="x"><wp:docPr id="1" name="Picture 1"/></wp:inline></w:drawing></w:r></w:p>"#,
    );
    assert!(
        matches!(&paragraph(&story, 0).inlines[0], Inline::Atom(atom) if atom.kind == AtomKind::Image("Picture 1".into()))
    );
}

#[test]
fn bookmarks_and_field_codes_take_up_no_place() {
    let story = body(
        r#"<w:p><w:bookmarkStart w:id="0" w:name="here"/><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>7</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:bookmarkEnd w:id="0"/></w:p>"#,
    );
    let paragraph = paragraph(&story, 0);
    assert_eq!(paragraph.text(), "7");
    assert_eq!(paragraph.len(), 1);
    assert_eq!(paragraph.inlines.len(), 7);
}

#[test]
fn tracked_deletions_are_hidden_and_insertions_shown() {
    let story = body(
        r#"<w:p><w:del w:id="1" w:author="A"><w:r><w:delText>gone</w:delText></w:r></w:del><w:ins w:id="2" w:author="A"><w:r><w:t>new</w:t></w:r></w:ins></w:p>"#,
    );
    assert_eq!(paragraph(&story, 0).text(), "new");
}

#[test]
fn a_hyperlink_wraps_its_text_and_knows_where_it_leads() {
    let story = docx::open(rich(r#"<w:p><w:r><w:t>See </w:t></w:r><w:hyperlink r:id="rId6"><w:r><w:t>the site</w:t></w:r></w:hyperlink></w:p>"#))
        .unwrap()
        .stories
        .remove(0);
    let paragraph = paragraph(&story, 0);
    assert_eq!(paragraph.text(), "See the site");
    let wraps = paragraph.inlines[1].wraps();
    assert_eq!(wraps.len(), 1);
    assert_eq!(wraps[0].link.as_deref(), Some("https://example.com/"));
    assert!(paragraph.inlines[0].wraps().is_empty());
}

#[test]
fn something_unknown_in_a_paragraph_is_kept_as_one_place() {
    let story = body(
        r#"<w:p><w:r><w:t>a</w:t></w:r><w:sdt><w:sdtContent><w:r><w:t>inside</w:t></w:r></w:sdtContent></w:sdt></w:p>"#,
    );
    let paragraph = paragraph(&story, 0);
    assert_eq!(paragraph.len(), 2);
    assert!(
        matches!(&paragraph.inlines[1], Inline::Atom(atom) if atom.kind == AtomKind::Unknown && atom.bare)
    );
}

#[test]
fn footnote_and_comment_references_carry_their_ids() {
    let story = body(
        r#"<w:p><w:r><w:footnoteReference w:id="3"/></w:r><w:r><w:commentReference w:id="5"/></w:r></w:p>"#,
    );
    let paragraph = paragraph(&story, 0);
    assert!(
        matches!(&paragraph.inlines[0], Inline::Atom(atom) if atom.kind == AtomKind::Footnote("3".into()))
    );
    assert!(
        matches!(&paragraph.inlines[1], Inline::Atom(atom) if atom.kind == AtomKind::Comment("5".into()))
    );
}

#[test]
fn paragraph_properties_are_kept_as_written() {
    let story = body(&styled("Heading1", "Title"));
    let props = &paragraph(&story, 0).props;
    assert_eq!(props.val("pStyle").as_deref(), Some("Heading1"));
    assert_eq!(props.xml(), "<w:pStyle w:val=\"Heading1\"/>");
}

#[test]
fn reads_a_table_into_rows_and_cells() {
    let story = body(&table(&[&["a", "b"], &["c", "d"]]));
    let Block::Table(table) = &story.blocks[0] else {
        panic!("not a table");
    };
    assert_eq!(table.grid, vec![2000, 2000]);
    assert_eq!(table.rows.len(), 2);
    assert!(!table.is_dirty());
    let path = BlockPath::Cell {
        table: 0,
        row: 1,
        cell: 0,
        block: 0,
    };
    assert_eq!(story.paragraph(path).unwrap().text(), "c");
    assert_eq!(story.paths().len(), 4);
}

#[test]
fn merged_cells_report_their_span_and_merge() {
    let story = body(&format!(
        "<w:tbl><w:tblGrid><w:gridCol w:w=\"100\"/><w:gridCol w:w=\"100\"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/><w:vMerge w:val=\"restart\"/></w:tcPr>{}</w:tc></w:tr><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr>{}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>",
        p("wide"),
        p(""),
        p("x")
    ));
    let Block::Table(table) = &story.blocks[0] else {
        panic!("not a table");
    };
    assert_eq!(table.rows[0].cells[0].span, 2);
    assert_eq!(table.rows[0].cells[0].merge, Merge::Start);
    assert_eq!(table.rows[1].cells[0].merge, Merge::Continue);
    assert_eq!(table.columns(), 2);
}

#[test]
fn a_table_inside_a_cell_is_kept_whole_with_its_text() {
    let inner = table(&[&["deep"]]);
    let story = body(&format!(
        "<w:tbl><w:tr><w:tc>{inner}{}</w:tc></w:tr></w:tbl>",
        p("after")
    ));
    let Block::Table(table) = &story.blocks[0] else {
        panic!("not a table");
    };
    assert!(
        matches!(&table.rows[0].cells[0].blocks[0], Block::Opaque(opaque) if opaque.text == ["deep"])
    );
}

#[test]
fn a_content_control_around_paragraphs_shows_its_text() {
    let story = body(&format!(
        "<w:sdt><w:sdtContent>{}{}</w:sdtContent></w:sdt>",
        p("Contents"),
        p("Chapter 1")
    ));
    assert!(
        matches!(&story.blocks[0], Block::Opaque(opaque) if opaque.text == ["Contents", "Chapter 1"])
    );
}

#[test]
fn finds_the_other_stories_through_the_relationships() {
    let opened = docx::open(rich(&p("Body text"))).unwrap();
    let kinds: Vec<StoryKind> = opened.stories.iter().map(|story| story.kind).collect();
    assert_eq!(
        kinds,
        vec![
            StoryKind::Body,
            StoryKind::Header,
            StoryKind::Footnotes,
            StoryKind::Comments
        ]
    );
    assert!(opened.stories[0].editable);
    assert!(!opened.stories[1].editable);
}

#[test]
fn footnotes_are_labelled_and_the_separator_is_left_out() {
    let opened = docx::open(rich(&p("Body text"))).unwrap();
    let notes = &opened.stories[2];
    assert_eq!(notes.notes.len(), 1);
    assert_eq!(notes.notes[0].id, "1");
    assert_eq!(paragraph(notes, 0).text(), "[1] A note at the foot.");
}

#[test]
fn comments_are_labelled_with_their_author() {
    let opened = docx::open(rich(&p("Body text"))).unwrap();
    assert_eq!(
        paragraph(&opened.stories[3], 0).text(),
        "[c0] Ada: Check this."
    );
}

#[test]
fn the_document_is_found_wherever_the_package_says_it_is() {
    let root = common::ROOT_RELS.replace("word/document.xml", "elsewhere/main.xml");
    let bytes = package(&[
        ("_rels/.rels", &root),
        ("elsewhere/main.xml", &document(&p("Found"))),
    ]);
    let opened = docx::open(bytes).unwrap();
    assert_eq!(paragraph(&opened.stories[0], 0).text(), "Found");
}

#[test]
fn a_file_that_is_not_a_zip_is_refused() {
    assert!(matches!(
        docx::open(b"plain text".to_vec()),
        Err(DocxError::Zip(_))
    ));
}

#[test]
fn a_zip_that_is_not_a_document_is_refused() {
    let bytes = package(&[("hello.txt", "hi")]);
    assert!(matches!(docx::open(bytes), Err(DocxError::Invalid(_))));
}

#[test]
fn malformed_xml_is_reported() {
    let bytes = docx_with("<w:p><w:r></w:p>", &[]);
    assert!(matches!(docx::open(bytes), Err(DocxError::Xml(_))));
}

#[test]
fn list_paragraphs_keep_their_numbering_properties() {
    let story = body(&item(2, 1, "nested"));
    let raw = paragraph(&story, 0).props.get("numPr").unwrap().to_string();
    assert!(raw.contains("w:numId w:val=\"2\""));
}

#[test]
fn a_header_with_nothing_in_it_gets_no_tab() {
    let rels = common::rels(&[
        ("rId1", "header", "header1.xml"),
        ("rId2", "header", "header2.xml"),
    ]);
    let (empty, full) = (
        common::part("hdr", "<w:p/>"),
        common::part("hdr", &p("Head")),
    );
    let bytes = docx_with(
        &p("Body"),
        &[
            ("word/_rels/document.xml.rels", &rels),
            ("word/header1.xml", &empty),
            ("word/header2.xml", &full),
        ],
    );
    let opened = docx::open(bytes).unwrap();
    assert_eq!(opened.stories.len(), 2);
    assert_eq!(paragraph(&opened.stories[1], 0).text(), "Head");
}
