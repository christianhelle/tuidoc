//! Builds real .docx packages from hand-written OOXML, independently of the code under test.
#![allow(dead_code)]

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
pub const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
pub const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// A document part wrapping the given body content.
pub fn document(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document {W}><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"#
    )
}

/// A part of another kind, such as `w:styles` or `w:footnotes`, wrapping the given content.
pub fn part(root: &str, content: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:{root} {W}>{content}</w:{root}>"#
    )
}

/// A paragraph of plain text.
pub fn p(text: &str) -> String {
    format!("<w:p><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>")
}

/// A paragraph of plain text in a style.
pub fn styled(style: &str, text: &str) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>"
    )
}

/// The relationships part for a document that links to the given parts and addresses.
/// Each is `(id, kind, target)`; a target with `://` in it is an address outside the package.
pub fn rels(links: &[(&str, &str, &str)]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (id, kind, target) in links {
        let mode = match target.contains("://") {
            true => " TargetMode=\"External\"",
            false => "",
        };
        out.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{REL}/{kind}\" Target=\"{target}\"{mode}/>"
        ));
    }
    out.push_str("</Relationships>");
    out
}

pub fn package(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in parts {
        zip.start_file(*name, options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// A package holding a document with the given body content, plus any extra parts.
pub fn docx_with(body: &str, extras: &[(&str, &str)]) -> Vec<u8> {
    let document = document(body);
    let mut parts = vec![
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", document.as_str()),
    ];
    parts.extend_from_slice(extras);
    package(&parts)
}

/// A package holding a document with the given body content.
pub fn docx(body: &str) -> Vec<u8> {
    docx_with(body, &[])
}

/// Styles with a default paragraph style, three headings and a bold character style.
pub fn styles() -> String {
    part(
        "styles",
        r#"<w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="berschrift3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:rPr><w:i/></w:rPr></w:style><w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/><w:rPr><w:b/></w:rPr></w:style>"#,
    )
}

/// Numbering with a bulleted list (1) and a numbered one (2) that counts 1. a) i.
pub fn numbering() -> String {
    part(
        "numbering",
        r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="·"/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="o"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2)"/></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="lowerRoman"/><w:lvlText w:val="%3."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#,
    )
}

/// A list paragraph in the given list at the given depth.
pub fn item(list: u32, depth: u32, text: &str) -> String {
    format!(
        "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"{depth}\"/><w:numId w:val=\"{list}\"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"
    )
}

/// A table of plain-text cells, one inner slice per row.
pub fn table(rows: &[&[&str]]) -> String {
    let columns = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    let mut out =
        String::from("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/></w:tblPr><w:tblGrid>");
    for _ in 0..columns {
        out.push_str("<w:gridCol w:w=\"2000\"/>");
    }
    out.push_str("</w:tblGrid>");
    for row in rows {
        out.push_str("<w:tr>");
        for cell in *row {
            out.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"2000\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>",
                p(cell)
            ));
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}

/// A package with styles, numbering, a footnote, a comment, a header and a hyperlink, around
/// the given body content.
pub fn rich(body: &str) -> Vec<u8> {
    let styles = styles();
    let numbering = numbering();
    let rels = rels(&[
        ("rId1", "styles", "styles.xml"),
        ("rId2", "numbering", "numbering.xml"),
        ("rId3", "footnotes", "footnotes.xml"),
        ("rId4", "comments", "comments.xml"),
        ("rId5", "header", "header1.xml"),
        ("rId6", "hyperlink", "https://example.com/"),
    ]);
    let footnotes = part(
        "footnotes",
        &format!(
            "<w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:id=\"1\">{}</w:footnote>",
            p("A note at the foot.")
        ),
    );
    let comments = part(
        "comments",
        &format!(
            "<w:comment w:id=\"0\" w:author=\"Ada\">{}</w:comment>",
            p("Check this.")
        ),
    );
    let header = part("hdr", &p("Running head"));
    docx_with(
        body,
        &[
            ("word/_rels/document.xml.rels", &rels),
            ("word/styles.xml", &styles),
            ("word/numbering.xml", &numbering),
            ("word/footnotes.xml", &footnotes),
            ("word/comments.xml", &comments),
            ("word/header1.xml", &header),
        ],
    )
}

pub struct TempDocx {
    _dir: tempfile::TempDir,
    pub path: PathBuf,
}

pub fn write_temp(bytes: &[u8]) -> TempDocx {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    std::fs::write(&path, bytes).unwrap();
    TempDocx { _dir: dir, path }
}

pub fn read_part(path: &Path, name: &str) -> Option<String> {
    read_part_of(&std::fs::read(path).unwrap(), name)
}

pub fn read_part_of(bytes: &[u8], name: &str) -> Option<String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    Some(content)
}
