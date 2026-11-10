//! The zip package: which part is the document, and what that part links to.

use std::io::{Cursor, Read};
use std::sync::Arc;

use zip::ZipArchive;

use super::DocxError;
use super::xml;

pub const ROOT_RELS: &str = "_rels/.rels";
pub const CONTENT_TYPES: &str = "[Content_Types].xml";
pub const RELATIONSHIPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const STRICT_RELATIONSHIPS: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships/";

/// A link from the document part to another part or to somewhere outside the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    /// The last word of the relationship type, such as `styles` or `hyperlink`.
    pub kind: String,
    /// The part's name inside the package, or the address as written for an external link.
    pub target: String,
    pub external: bool,
}

/// An opened package, together with the bytes it was read from.
#[derive(Debug, Clone)]
pub struct Package {
    pub bytes: Arc<Vec<u8>>,
    /// The name of the main document part, which isn't always `word/document.xml`.
    pub main: String,
    pub relationships: Vec<Relationship>,
}

impl Package {
    /// Finds the document inside a package. Whether this is a Word document is decided by what
    /// the package says it holds, never by the file's name.
    pub fn open(bytes: Arc<Vec<u8>>) -> Result<Self, DocxError> {
        let root = read_part(&bytes, ROOT_RELS)?
            .ok_or_else(|| DocxError::Invalid("the package has no relationships".into()))?;
        let main = relationships(&root, "")?
            .into_iter()
            .find(|relationship| relationship.kind == "officeDocument")
            .map(|relationship| relationship.target)
            .ok_or_else(|| DocxError::Invalid("the package holds no document".into()))?;
        let relationships = match read_part(&bytes, &rels_part(&main))? {
            Some(rels) => relationships(&rels, directory(&main))?,
            None => Vec::new(),
        };
        let package = Self {
            bytes,
            main,
            relationships,
        };
        if package.read(&package.main)?.is_none() {
            return Err(DocxError::Invalid("the document part is missing".into()));
        }
        Ok(package)
    }

    /// A part's text, or nothing when the package has no such part.
    pub fn read(&self, part: &str) -> Result<Option<String>, DocxError> {
        read_part(&self.bytes, part)
    }

    /// The parts the document links to with this kind of relationship, in the order written.
    pub fn parts(&self, kind: &str) -> Vec<&str> {
        self.relationships
            .iter()
            .filter(|relationship| relationship.kind == kind && !relationship.external)
            .map(|relationship| relationship.target.as_str())
            .collect()
    }

    pub fn part(&self, kind: &str) -> Option<&str> {
        self.parts(kind).first().copied()
    }

    /// Where a relationship points, such as the address behind a hyperlink.
    pub fn target(&self, id: &str) -> Option<&str> {
        self.relationships
            .iter()
            .find(|relationship| relationship.id == id)
            .map(|relationship| relationship.target.as_str())
    }

    /// The part listing the main document's relationships.
    pub fn main_rels(&self) -> String {
        rels_part(&self.main)
    }
}

/// The folder a part sits in, with its trailing slash, or nothing for a part at the top.
pub fn directory(part: &str) -> &str {
    part.rfind('/').map_or("", |slash| &part[..=slash])
}

/// The name of the part that lists another part's relationships.
pub fn rels_part(part: &str) -> String {
    let directory = directory(part);
    format!("{directory}_rels/{}.rels", &part[directory.len()..])
}

fn read_part(bytes: &[u8], part: &str) -> Result<Option<String>, DocxError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let mut file = match archive.by_name(part) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    // A byte order mark would throw off every offset into the part.
    Ok(Some(match content.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => content,
    }))
}

fn relationships(rels: &str, base: &str) -> Result<Vec<Relationship>, DocxError> {
    let root = xml::parse(rels)?;
    Ok(root
        .children
        .iter()
        .filter(|child| child.local() == "Relationship")
        .filter_map(|child| {
            let kind = child.attr(rels, "Type")?;
            let kind = kind
                .strip_prefix(RELATIONSHIPS)
                .or_else(|| kind.strip_prefix(STRICT_RELATIONSHIPS))
                .unwrap_or(&kind)
                .to_string();
            let target = child.attr(rels, "Target")?;
            let external = child.attr(rels, "TargetMode").as_deref() == Some("External");
            Some(Relationship {
                id: child.attr(rels, "Id")?,
                kind,
                target: if external {
                    target
                } else {
                    resolve(base, &target)
                },
                external,
            })
        })
        .collect())
}

/// Turns a target written relative to a part's folder into a part name.
fn resolve(base: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_string();
    }
    let mut segments: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for segment in target.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}
