//! What a paragraph is made of: text, things that stand where a character would, and markers.

use std::sync::Arc;

use unicode_segmentation::UnicodeSegmentation;

use super::props::Props;

/// The elements wrapped around a piece of a paragraph, outermost first: a hyperlink, a tracked
/// insertion, a simple field.
pub type Wraps = Arc<[Wrapper]>;

/// An element that wraps runs, such as `w:hyperlink`.
#[derive(Debug, Clone)]
pub struct Wrapper {
    /// Tells one hyperlink from the next one, even when both are written the same way.
    pub id: u32,
    pub open_tag: Arc<str>,
    /// The element's name as written, for closing it.
    pub name: Arc<str>,
    /// Where a hyperlink leads.
    pub link: Option<Arc<str>>,
}

impl PartialEq for Wrapper {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Wrapper {}

/// A stretch of text in one format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub props: Props,
    pub wraps: Wraps,
}

/// What an atom stands for, which decides how it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomKind {
    Tab,
    LineBreak,
    PageBreak,
    /// A picture or shape, with its alternative text.
    Image(String),
    Footnote(String),
    Endnote(String),
    Comment(String),
    /// Anything tuidoc can't show: it is kept, and can be deleted, but not looked inside.
    Unknown,
}

/// Something that isn't text but takes up one place in the paragraph, as a character would.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub kind: AtomKind,
    /// The XML it was read from: a run's child, or a whole element when `bare`.
    pub raw: Arc<str>,
    pub props: Props,
    pub wraps: Wraps,
    /// Whether the XML stands on its own in the paragraph rather than inside a run.
    pub bare: bool,
}

/// Something with no place of its own in the text: a bookmark's end, a field's code, tracked
/// deleted text. Markers come in pairs more often than not, so edits never drop them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub raw: Arc<str>,
    /// The properties of the run it was in, for a marker that must sit inside a run.
    pub run: Option<Props>,
    pub wraps: Wraps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(Run),
    Atom(Atom),
    Marker(Marker),
}

impl Inline {
    /// How many places this takes up in the paragraph: one per character as a reader sees them.
    pub fn cells(&self) -> usize {
        match self {
            Self::Text(run) => run.text.graphemes(true).count(),
            Self::Atom(_) => 1,
            Self::Marker(_) => 0,
        }
    }

    pub fn wraps(&self) -> &Wraps {
        match self {
            Self::Text(run) => &run.wraps,
            Self::Atom(atom) => &atom.wraps,
            Self::Marker(marker) => &marker.wraps,
        }
    }

    /// The format, for the kinds that have one of their own.
    pub fn props(&self) -> Option<&Props> {
        match self {
            Self::Text(run) => Some(&run.props),
            Self::Atom(atom) => Some(&atom.props),
            Self::Marker(_) => None,
        }
    }

    pub fn props_mut(&mut self) -> Option<&mut Props> {
        match self {
            Self::Text(run) => Some(&mut run.props),
            Self::Atom(atom) if !atom.bare => Some(&mut atom.props),
            _ => None,
        }
    }
}

/// No wrappers at all.
pub fn unwrapped() -> Wraps {
    Arc::from(Vec::new())
}

/// A piece of formatted text lifted out of a paragraph, to be put back somewhere else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fragment(pub Vec<Inline>);

impl Fragment {
    pub fn len(&self) -> usize {
        self.0.iter().map(Inline::cells).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The text alone, with a placeholder character for each atom.
    pub fn text(&self) -> String {
        plain_text(&self.0)
    }
}

/// The character standing for an atom in plain text.
pub const OBJECT: char = '\u{fffc}';

pub fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(run) => out.push_str(&run.text),
            Inline::Atom(atom) => out.push(match atom.kind {
                AtomKind::Tab => '\t',
                AtomKind::LineBreak => '\n',
                _ => OBJECT,
            }),
            Inline::Marker(_) => {}
        }
    }
    out
}
