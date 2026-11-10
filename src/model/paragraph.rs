//! A paragraph, and every way of changing one.

use std::ops::Range;
use std::sync::Arc;

use unicode_segmentation::UnicodeSegmentation;

use super::inline::{Atom, AtomKind, Fragment, Inline, Run, Wraps, plain_text, unwrapped};
use super::props::Props;

/// The opening tag given to a paragraph tuidoc made itself.
pub const PLAIN_OPEN_TAG: &str = "<w:p>";

/// One place in a paragraph: a character of text or an atom, with the format it is in.
#[derive(Debug, Clone, Copy)]
pub struct Cell<'a> {
    /// The character, or nothing for an atom.
    pub text: Option<&'a str>,
    pub atom: Option<&'a AtomKind>,
    pub props: &'a Props,
    pub wraps: &'a Wraps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    /// Where the paragraph sits in its part's source, for one that was read from a file.
    pub source: Option<Range<usize>>,
    /// Whether it differs from that source, and so has to be written out again.
    pub dirty: bool,
    /// The opening tag as written, which carries the identifiers Word gives each paragraph.
    pub open_tag: Arc<str>,
    pub props: Props,
    pub inlines: Vec<Inline>,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self::new(Props::default())
    }
}

impl Paragraph {
    /// An empty paragraph that exists only in memory so far.
    pub fn new(props: Props) -> Self {
        Self {
            source: None,
            dirty: true,
            open_tag: PLAIN_OPEN_TAG.into(),
            props,
            inlines: Vec::new(),
        }
    }

    /// A copy that owes nothing to the file, for pasting: two paragraphs can't share the
    /// identifiers Word gave the original.
    pub fn detached(&self) -> Self {
        let mut copy = self.clone();
        copy.source = None;
        copy.dirty = true;
        copy.open_tag = PLAIN_OPEN_TAG.into();
        copy.props.remove("sectPr");
        copy
    }

    /// How many places the paragraph has: characters as a reader sees them, plus atoms.
    pub fn len(&self) -> usize {
        self.inlines.iter().map(Inline::cells).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The text alone, with a placeholder character for each atom.
    pub fn text(&self) -> String {
        plain_text(&self.inlines)
    }

    /// Every place in the paragraph, in order.
    pub fn cells(&self) -> Vec<Cell<'_>> {
        let mut cells = Vec::new();
        for inline in &self.inlines {
            match inline {
                Inline::Text(run) => {
                    cells.extend(run.text.graphemes(true).map(|grapheme| Cell {
                        text: Some(grapheme),
                        atom: None,
                        props: &run.props,
                        wraps: &run.wraps,
                    }));
                }
                Inline::Atom(atom) => cells.push(Cell {
                    text: None,
                    atom: Some(&atom.kind),
                    props: &atom.props,
                    wraps: &atom.wraps,
                }),
                Inline::Marker(_) => {}
            }
        }
        cells
    }

    /// The characters alone, one string per place, with atoms as their placeholder.
    pub fn graphemes(&self) -> Vec<String> {
        self.cells()
            .iter()
            .map(|cell| match (cell.text, cell.atom) {
                (Some(text), _) => text.to_string(),
                (None, Some(AtomKind::Tab)) => "\t".to_string(),
                _ => super::inline::OBJECT.to_string(),
            })
            .collect()
    }

    /// Makes sure a piece starts exactly at this place, and says which piece that is. Markers
    /// sitting at the boundary stay after it.
    fn boundary(&mut self, offset: usize) -> usize {
        let mut seen = 0;
        let mut after_previous = 0;
        for index in 0..self.inlines.len() {
            let cells = self.inlines[index].cells();
            if cells == 0 {
                continue;
            }
            if offset <= seen {
                return after_previous;
            }
            if offset < seen + cells {
                if let Inline::Text(run) = &mut self.inlines[index] {
                    let byte = run
                        .text
                        .grapheme_indices(true)
                        .nth(offset - seen)
                        .map_or(run.text.len(), |(byte, _)| byte);
                    let rest = Run {
                        text: run.text.split_off(byte),
                        props: run.props.clone(),
                        wraps: run.wraps.clone(),
                    };
                    self.inlines.insert(index + 1, Inline::Text(rest));
                }
                return index + 1;
            }
            seen += cells;
            after_previous = index + 1;
        }
        after_previous
    }

    /// Types text in at a place. It takes the format of what comes before it, as Word does, or
    /// of what follows at the start of a paragraph.
    pub fn insert_text(&mut self, at: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        self.dirty = true;
        let index = self.boundary(at);
        if index > 0
            && let Inline::Text(run) = &mut self.inlines[index - 1]
        {
            run.text.push_str(text);
            return;
        }
        let next = self.inlines[index..]
            .iter()
            .position(|inline| inline.cells() > 0)
            .map(|found| index + found);
        if index == 0
            && let Some(next) = next
            && let Inline::Text(run) = &mut self.inlines[next]
        {
            run.text.insert_str(0, text);
            return;
        }
        let model = match index {
            0 => next.map(|next| &self.inlines[next]),
            _ => Some(&self.inlines[index - 1]),
        };
        let run = Run {
            text: text.to_string(),
            props: model
                .and_then(|inline| inline.props().cloned())
                .unwrap_or_default(),
            wraps: model.map_or_else(unwrapped, |inline| inline.wraps().clone()),
        };
        self.inlines.insert(index, Inline::Text(run));
    }

    /// Puts an atom in at a place, in the format of what comes before it.
    pub fn insert_atom(&mut self, at: usize, kind: AtomKind, raw: &str) {
        self.dirty = true;
        let index = self.boundary(at);
        let model = index.checked_sub(1).map(|before| &self.inlines[before]);
        let atom = Atom {
            kind,
            raw: raw.into(),
            props: model
                .and_then(|inline| inline.props().cloned())
                .unwrap_or_default(),
            wraps: model.map_or_else(unwrapped, |inline| inline.wraps().clone()),
            bare: false,
        };
        self.inlines.insert(index, Inline::Atom(atom));
    }

    /// A copy of a stretch of the paragraph, without its markers.
    pub fn fragment(&self, range: Range<usize>) -> Fragment {
        let mut copy = self.clone();
        copy.delete(range)
    }

    /// Takes a stretch out. Markers inside it stay behind, so that a bookmark or a field never
    /// loses one of its ends.
    pub fn delete(&mut self, range: Range<usize>) -> Fragment {
        if range.start >= range.end {
            return Fragment::default();
        }
        self.dirty = true;
        let start = self.boundary(range.start);
        let end = self.boundary(range.end).max(start);
        let mut taken = Vec::new();
        let mut kept = Vec::new();
        for inline in self.inlines.drain(start..end) {
            match inline {
                Inline::Marker(_) => kept.push(inline),
                other => taken.push(other),
            }
        }
        self.inlines.splice(start..start, kept);
        self.normalize();
        Fragment(taken)
    }

    /// Puts a fragment in at a place, keeping its own formatting.
    pub fn insert_fragment(&mut self, at: usize, fragment: &Fragment) {
        if fragment.0.is_empty() {
            return;
        }
        self.dirty = true;
        let index = self.boundary(at);
        self.inlines
            .splice(index..index, fragment.0.iter().cloned());
        self.normalize();
    }

    /// Breaks the paragraph in two at a place and hands back the second half, which keeps the
    /// first one's style. A section break belongs to the end of a section, so it goes with the
    /// second half.
    pub fn split(&mut self, at: usize) -> Paragraph {
        self.dirty = true;
        let index = self.boundary(at);
        let mut rest = Paragraph::new(self.props.clone());
        rest.inlines = self.inlines.split_off(index);
        self.props.remove("sectPr");
        rest
    }

    /// Appends another paragraph's content. The style stays this paragraph's own.
    pub fn join(&mut self, other: Paragraph) {
        self.dirty = true;
        if let Some(section) = other.props.get("sectPr") {
            let section = section.to_string();
            self.props
                .set("sectPr", &section, super::props::PARAGRAPH_ORDER);
        }
        self.inlines.extend(other.inlines);
        self.normalize();
    }

    /// The formats in use over a stretch, one per piece of text or atom.
    pub fn formats(&self, range: Range<usize>) -> Vec<&Props> {
        let mut seen = 0;
        let mut found = Vec::new();
        for inline in &self.inlines {
            let cells = inline.cells();
            if cells > 0 && seen < range.end && seen + cells > range.start {
                found.extend(inline.props());
            }
            seen += cells;
        }
        found
    }

    /// Changes the format of everything in a stretch.
    pub fn reformat(&mut self, range: Range<usize>, mut change: impl FnMut(&mut Props)) {
        if range.start >= range.end {
            return;
        }
        self.dirty = true;
        let start = self.boundary(range.start);
        let end = self.boundary(range.end).max(start);
        for inline in &mut self.inlines[start..end] {
            if let Some(props) = inline.props_mut() {
                change(props);
            }
        }
        self.normalize();
    }

    /// Joins neighbouring pieces of text that have ended up in the same format, and drops
    /// empty ones.
    pub fn normalize(&mut self) {
        let mut merged: Vec<Inline> = Vec::with_capacity(self.inlines.len());
        for inline in self.inlines.drain(..) {
            match (merged.last_mut(), inline) {
                (_, Inline::Text(run)) if run.text.is_empty() => {}
                (Some(Inline::Text(last)), Inline::Text(run))
                    if last.props == run.props && last.wraps == run.wraps =>
                {
                    last.text.push_str(&run.text);
                }
                (_, inline) => merged.push(inline),
            }
        }
        self.inlines = merged;
    }
}
