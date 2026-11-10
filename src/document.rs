//! An open document: its stories, the file it belongs to, and every change made to it.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::changes::{Change, History};
use crate::docx::package::Package;
use crate::docx::{self, DocxError};
use crate::model::{
    Block, BlockPath, Fragment, Paragraph, Position, Props, Story, index_of, with_index,
};
use crate::numbering::Numbering;
use crate::styles::Styles;

/// Why an edit wasn't made, in words for the status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Only the body can be edited so far.
    ReadOnlyStory,
    /// The change would run from inside a table's cell to outside it.
    AcrossCells,
    /// There is nothing there to act on.
    Nothing,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReadOnlyStory => "Headers, footers and notes can't be edited yet",
            Self::AcrossCells => "Can't change text across the edge of a table cell",
            Self::Nothing => "Nothing to change here",
        })
    }
}

#[derive(Debug)]
pub struct Document {
    pub package: Package,
    /// What the file held when it was last read or written, to notice another program's edits.
    pub(crate) on_disk: Option<Arc<Vec<u8>>>,
    pub(crate) path: Option<PathBuf>,
    pub stories: Vec<Story>,
    pub styles: Styles,
    pub numbering: Numbering,
    /// Parts other than the body that tuidoc has rewritten or added, such as the styles after
    /// a heading style was needed that the file didn't have.
    pub(crate) parts: BTreeMap<String, String>,
    history: History,
    /// The body as it was before the change being made, and where the cursor was.
    pending: Option<(Vec<Block>, Position)>,
    next_change: u64,
    /// The change the document stood at when it was last saved.
    saved: Option<u64>,
    /// Counts every alteration, so a layout knows when it is out of date.
    version: u64,
}

impl Document {
    /// An empty document that has no file yet.
    pub fn blank() -> Result<Self, DocxError> {
        Self::from_bytes(docx::blank::package(), None)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocxError> {
        let path = path.as_ref();
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        Self::from_bytes(std::fs::read(path)?, Some(absolute))
    }

    pub fn from_bytes(bytes: Vec<u8>, path: Option<PathBuf>) -> Result<Self, DocxError> {
        let opened = docx::open(bytes)?;
        Ok(Self {
            on_disk: path.is_some().then(|| opened.package.bytes.clone()),
            package: opened.package,
            path,
            stories: opened.stories,
            styles: opened.styles,
            numbering: opened.numbering,
            parts: BTreeMap::new(),
            history: History::default(),
            pending: None,
            next_change: 1,
            saved: None,
            version: 0,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The file's name, or a stand-in for a document that has none yet.
    pub fn file_name(&self) -> String {
        self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "[No Name]".to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Whether there are changes the file doesn't have.
    pub fn is_dirty(&self) -> bool {
        self.history.latest() != self.saved || self.pending_differs()
    }

    fn pending_differs(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(before, _)| *before != self.stories[0].blocks)
    }

    pub(crate) fn mark_saved(&mut self) {
        self.saved = self.history.latest();
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn body(&self) -> &Story {
        &self.stories[0]
    }

    /// The body, about to be altered.
    pub(crate) fn body_mut(&mut self) -> &mut Story {
        self.version += 1;
        &mut self.stories[0]
    }

    pub fn paragraph(&self, path: BlockPath) -> Option<&Paragraph> {
        self.body().paragraph(path)
    }

    pub(crate) fn paragraph_mut(&mut self, path: BlockPath) -> Option<&mut Paragraph> {
        self.body_mut().paragraph_mut(path)
    }

    /// How many places a paragraph has.
    pub fn len(&self, path: BlockPath) -> usize {
        self.paragraph(path).map_or(0, Paragraph::len)
    }

    // ---- undo ----

    /// Starts a change that `commit` will finish. Everything in between undoes as one.
    pub fn begin(&mut self, cursor: Position) {
        if self.pending.is_none() {
            self.pending = Some((self.stories[0].blocks.clone(), cursor));
        }
    }

    /// Finishes a change, remembering what it replaced. A change that altered nothing leaves
    /// no trace.
    pub fn commit(&mut self, cursor: Position) {
        let Some((before, cursor_before)) = self.pending.take() else {
            return;
        };
        let after = &self.stories[0].blocks;
        let same_start = before
            .iter()
            .zip(after)
            .take_while(|(old, new)| old == new)
            .count();
        if same_start == before.len() && same_start == after.len() {
            return;
        }
        let same_end = before[same_start..]
            .iter()
            .rev()
            .zip(after[same_start..].iter().rev())
            .take_while(|(old, new)| old == new)
            .count();
        let change = Change {
            id: self.next_change,
            at: same_start,
            before: before[same_start..before.len() - same_end].to_vec(),
            after: after[same_start..after.len() - same_end].to_vec(),
            cursor_before,
            cursor_after: cursor,
        };
        self.next_change += 1;
        self.history.record(change);
    }

    /// Undoes the latest change and says where the cursor was before it.
    pub fn undo(&mut self) -> Option<Position> {
        let change = self.history.take_undo()?;
        let end = change.at + change.after.len();
        self.body_mut().blocks.splice(change.at..end, change.before);
        Some(change.cursor_before)
    }

    pub fn redo(&mut self) -> Option<Position> {
        let change = self.history.take_redo()?;
        let end = change.at + change.before.len();
        self.body_mut().blocks.splice(change.at..end, change.after);
        Some(change.cursor_after)
    }

    // ---- editing text ----

    /// Types text in. A line feed in it starts a new paragraph.
    pub fn insert_text(&mut self, at: Position, text: &str) -> Position {
        let mut at = at;
        for (index, piece) in text.split('\n').enumerate() {
            if index > 0 {
                at = self.split(at);
            }
            let piece = piece.trim_end_matches('\r');
            let before = self.len(at.path);
            if let Some(paragraph) = self.paragraph_mut(at.path) {
                paragraph.insert_text(at.offset, piece);
            }
            at.offset += self.len(at.path) - before;
        }
        at
    }

    /// Breaks a paragraph in two and says where the second one starts.
    pub fn split(&mut self, at: Position) -> Position {
        let Some(paragraph) = self.paragraph_mut(at.path) else {
            return at;
        };
        let rest = paragraph.split(at.offset);
        let next = with_index(at.path, index_of(at.path) + 1);
        if let Some(container) = self.body_mut().container_mut(at.path) {
            container.insert(index_of(next), Block::Paragraph(rest));
        }
        Position::new(next, 0)
    }

    /// Joins a paragraph with the one after it, if that is a paragraph too, and says where
    /// they met.
    pub fn join(&mut self, path: BlockPath) -> Option<Position> {
        let next = with_index(path, index_of(path) + 1);
        self.paragraph(next)?;
        let offset = self.len(path);
        let container = self.body_mut().container_mut(path)?;
        let Block::Paragraph(second) = container.remove(index_of(next)) else {
            return None;
        };
        let Some(Block::Paragraph(first)) = container.get_mut(index_of(path)) else {
            return None;
        };
        first.join(second);
        Some(Position::new(path, offset))
    }

    /// Whether two paragraphs sit in the same list of blocks: both in the body, or both in
    /// one table cell.
    pub fn together(first: BlockPath, second: BlockPath) -> bool {
        with_index(first, 0) == with_index(second, 0)
    }

    /// Takes out the text from one place up to another, joining the paragraphs it ran
    /// through, and hands back what was there, one fragment per paragraph.
    pub fn delete(&mut self, from: Position, to: Position) -> Result<Vec<Fragment>, Refusal> {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        if !Self::together(from.path, to.path) {
            return Err(Refusal::AcrossCells);
        }
        let taken = self.copy(from, to)?;
        if from.path == to.path {
            if let Some(paragraph) = self.paragraph_mut(from.path) {
                paragraph.delete(from.offset..to.offset);
            }
            return Ok(taken);
        }
        let first_len = self.len(from.path);
        if let Some(paragraph) = self.paragraph_mut(from.path) {
            paragraph.delete(from.offset..first_len);
        }
        if let Some(paragraph) = self.paragraph_mut(to.path) {
            paragraph.delete(0..to.offset);
        }
        let (first, last) = (index_of(from.path), index_of(to.path));
        if let Some(container) = self.body_mut().container_mut(from.path) {
            container.drain(first + 1..last);
        }
        self.join(from.path);
        Ok(taken)
    }

    /// A copy of the text from one place up to another, one fragment per paragraph.
    pub fn copy(&self, from: Position, to: Position) -> Result<Vec<Fragment>, Refusal> {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        if !Self::together(from.path, to.path) {
            return Err(Refusal::AcrossCells);
        }
        let mut fragments = Vec::new();
        for index in index_of(from.path)..=index_of(to.path) {
            let path = with_index(from.path, index);
            let Some(paragraph) = self.paragraph(path) else {
                // A table between two paragraphs can't be part of a run of text.
                return Err(Refusal::AcrossCells);
            };
            let start = if path == from.path { from.offset } else { 0 };
            let end = if path == to.path {
                to.offset
            } else {
                paragraph.len()
            };
            fragments.push(paragraph.fragment(start..end));
        }
        Ok(fragments)
    }

    /// Puts fragments in at a place, a new paragraph for each after the first, and says where
    /// the last one ended.
    pub fn insert_fragments(&mut self, at: Position, fragments: &[Fragment]) -> Position {
        let mut at = at;
        for (index, fragment) in fragments.iter().enumerate() {
            if index > 0 {
                at = self.split(at);
            }
            if let Some(paragraph) = self.paragraph_mut(at.path) {
                paragraph.insert_fragment(at.offset, fragment);
            }
            at.offset += fragment.len();
        }
        at
    }

    /// Takes whole blocks out of the list a paragraph sits in. A body or a cell is never left
    /// without a paragraph: an empty one takes the place of the last to go.
    pub fn delete_blocks(&mut self, first: BlockPath, last: BlockPath) -> Vec<Block> {
        let range = index_of(first).min(index_of(last))..=index_of(first).max(index_of(last));
        let Some(container) = self.body_mut().container_mut(first) else {
            return Vec::new();
        };
        let taken: Vec<Block> = container.drain(range.clone()).collect();
        if !container
            .iter()
            .any(|block| matches!(block, Block::Paragraph(_)))
        {
            let mut empty = Paragraph::default();
            // The new paragraph keeps the style of the one it replaces, as Word's would.
            if let Some(Block::Paragraph(old)) = taken.first() {
                empty.props = old.props.clone();
                empty.props.remove("sectPr");
            }
            container.insert(
                (*range.start()).min(container.len()),
                Block::Paragraph(empty),
            );
        }
        taken
    }

    /// Puts blocks into the list a paragraph sits in, at that paragraph's index.
    pub fn insert_blocks(&mut self, at: BlockPath, blocks: Vec<Block>) {
        let inside_cell = matches!(at, BlockPath::Cell { .. });
        let Some(container) = self.body_mut().container_mut(at) else {
            return;
        };
        let index = index_of(at).min(container.len());
        let blocks = blocks
            .into_iter()
            // A table can't be pasted into a cell: tuidoc couldn't show it there.
            .filter(|block| !inside_cell || matches!(block, Block::Paragraph(_)))
            .map(detached);
        container.splice(index..index, blocks);
    }

    /// Makes a new, empty paragraph in the style of a neighbour and says where it is.
    pub fn open_paragraph(&mut self, beside: BlockPath, below: bool) -> Position {
        let props = self
            .paragraph(beside)
            .map(|paragraph| {
                let mut props = paragraph.props.clone();
                props.remove("sectPr");
                props
            })
            .unwrap_or_default();
        let at = with_index(beside, index_of(beside) + usize::from(below));
        if let Some(container) = self.body_mut().container_mut(beside) {
            container.insert(index_of(at), Block::Paragraph(Paragraph::new(props)));
        }
        Position::new(at, 0)
    }

    /// Changes the properties of a paragraph.
    pub fn change_paragraph(&mut self, path: BlockPath, change: impl FnOnce(&mut Props)) {
        if let Some(paragraph) = self.paragraph_mut(path) {
            change(&mut paragraph.props);
            paragraph.dirty = true;
        }
    }

    /// The text of the body's paragraphs, for tests and searches.
    pub fn text(&self) -> Vec<String> {
        self.body()
            .paths()
            .into_iter()
            .filter_map(|path| self.paragraph(path))
            .map(Paragraph::text)
            .collect()
    }
}

/// A copy of a block that owes nothing to the file, so pasting it twice can't write the same
/// paragraph identifiers twice.
fn detached(block: Block) -> Block {
    match block {
        Block::Paragraph(paragraph) => Block::Paragraph(paragraph.detached()),
        Block::Table(mut table) => {
            table.source = None;
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    cell.blocks = std::mem::take(&mut cell.blocks)
                        .into_iter()
                        .map(detached)
                        .collect();
                }
            }
            Block::Table(table)
        }
        opaque => opaque,
    }
}
