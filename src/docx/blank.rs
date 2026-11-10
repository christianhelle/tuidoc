//! The package a new, empty document starts from.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const MAIN: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const OFFICE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE: &str = "http://schemas.openxmlformats.org/package/2006";
const WORD: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";

/// The bytes of a package holding one empty paragraph on an A4 page. It is read like any
/// other file, so a new document and an opened one go down the same road.
pub fn package() -> Vec<u8> {
    let parts = [
        (
            "[Content_Types].xml",
            format!(
                "{HEADER}<Types xmlns=\"{PACKAGE}/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"{WORD}.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"{WORD}.styles+xml\"/></Types>"
            ),
        ),
        (
            "_rels/.rels",
            format!(
                "{HEADER}<Relationships xmlns=\"{PACKAGE}/relationships\"><Relationship Id=\"rId1\" Type=\"{OFFICE}/officeDocument\" Target=\"word/document.xml\"/></Relationships>"
            ),
        ),
        (
            "word/document.xml",
            format!(
                "{HEADER}<w:document xmlns:w=\"{MAIN}\" xmlns:r=\"{OFFICE}\"><w:body><w:p/><w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>"
            ),
        ),
        (
            "word/styles.xml",
            format!(
                "{HEADER}<w:styles xmlns:w=\"{MAIN}\"><w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"160\" w:line=\"259\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style></w:styles>"
            ),
        ),
        (
            "word/_rels/document.xml.rels",
            format!(
                "{HEADER}<Relationships xmlns=\"{PACKAGE}/relationships\"><Relationship Id=\"rId1\" Type=\"{OFFICE}/styles\" Target=\"styles.xml\"/></Relationships>"
            ),
        ),
    ];
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in parts {
        // Writing to memory can't fail, and a blank document has nothing else to report.
        let _ = zip.start_file(name, options);
        let _ = zip.write_all(content.as_bytes());
    }
    zip.finish().map(Cursor::into_inner).unwrap_or_default()
}
