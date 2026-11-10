//! Reading and writing the .docx package a document lives in.

pub mod blank;
pub mod package;
pub mod reader;
pub mod saving;
pub mod writer;
pub mod xml;

use std::fmt;
use std::sync::Arc;

use crate::model::{Story, StoryKind};
use crate::numbering::Numbering;
use crate::styles::Styles;
use package::Package;

#[derive(Debug)]
pub enum DocxError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Xml(String),
    /// A required part or relationship is absent.
    Invalid(String),
    /// Another program modified the file after it was opened.
    ChangedOnDisk,
    /// The document has never been saved, so it has no file to go to.
    NoFileName,
}

impl fmt::Display for DocxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Zip(error) => write!(f, "not a readable document: {error}"),
            Self::Xml(message) => write!(f, "malformed XML: {message}"),
            Self::Invalid(message) => write!(f, "invalid document: {message}"),
            Self::ChangedOnDisk => write!(f, "the file changed on disk since it was opened"),
            Self::NoFileName => write!(f, "no file name"),
        }
    }
}

impl std::error::Error for DocxError {}

impl From<std::io::Error> for DocxError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for DocxError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error)
    }
}

/// Everything read out of a package: its stories and what is needed to show them.
#[derive(Debug, Clone)]
pub struct Docx {
    pub package: Package,
    /// The body first, then headers, footers, notes and comments, where the file has them.
    pub stories: Vec<Story>,
    pub styles: Styles,
    pub numbering: Numbering,
}

/// Reads a document from the bytes of its file.
pub fn open(bytes: Vec<u8>) -> Result<Docx, DocxError> {
    let package = Package::open(Arc::new(bytes))?;
    let styles = match package.part("styles") {
        Some(part) => match package.read(part)? {
            Some(source) => Styles::parse(&source)?,
            None => Styles::default(),
        },
        None => Styles::default(),
    };
    let numbering = match package.part("numbering") {
        Some(part) => match package.read(part)? {
            Some(source) => Numbering::parse(&source)?,
            None => Numbering::default(),
        },
        None => Numbering::default(),
    };
    let mut stories = Vec::new();
    let kinds = [
        (StoryKind::Body, "", "Body"),
        (StoryKind::Header, "header", "Header"),
        (StoryKind::Footer, "footer", "Footer"),
        (StoryKind::Footnotes, "footnotes", "Footnotes"),
        (StoryKind::Endnotes, "endnotes", "Endnotes"),
        (StoryKind::Comments, "comments", "Comments"),
    ];
    for (kind, relationship, title) in kinds {
        let parts: Vec<String> = match kind {
            StoryKind::Body => vec![package.main.clone()],
            _ => package
                .parts(relationship)
                .into_iter()
                .map(str::to_string)
                .collect(),
        };
        let several = parts.len() > 1;
        for (index, part) in parts.iter().enumerate() {
            let Some(source) = package.read(part)? else {
                continue;
            };
            let title = match several {
                true => format!("{title} {}", index + 1),
                false => title.to_string(),
            };
            let story = reader::read_story(kind, &title, part, source.into(), &package)?;
            let notes = matches!(
                kind,
                StoryKind::Footnotes | StoryKind::Endnotes | StoryKind::Comments
            );
            // Word writes a header and footer for every kind of page, most of them empty.
            let margin = matches!(kind, StoryKind::Header | StoryKind::Footer);
            let blank = story
                .paths()
                .iter()
                .all(|path| story.paragraph(*path).is_none_or(|p| p.is_empty()));
            if !(notes && story.notes.is_empty()) && !(margin && blank) {
                stories.push(story);
            }
        }
    }
    Ok(Docx {
        package,
        stories,
        styles,
        numbering,
    })
}
