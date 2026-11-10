//! A document as tuidoc holds it: stories made of paragraphs and tables.

pub mod inline;
pub mod paragraph;
pub mod props;
pub mod table;

use std::ops::Range;
use std::sync::Arc;

pub use inline::{Atom, AtomKind, Fragment, Inline, Marker, Run, Wrapper, Wraps};
pub use paragraph::Paragraph;
pub use props::Props;
pub use table::{Merge, Table};

/// Something tuidoc keeps but doesn't edit: a section's page setup, a content control, a table
/// inside a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opaque {
    pub raw: Arc<str>,
    /// The text inside it, one string per paragraph, shown but not editable.
    pub text: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    Opaque(Opaque),
}

/// Which part of the document a story is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryKind {
    Body,
    Header,
    Footer,
    Footnotes,
    Endnotes,
    Comments,
}

/// A footnote, endnote or comment inside its story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    /// Which of the story's blocks belong to it.
    pub blocks: Range<usize>,
}

/// One run of text with a beginning and an end: the body, a header, the footnotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    pub kind: StoryKind,
    /// What the story is called on its tab.
    pub title: String,
    pub part: String,
    pub source: Arc<str>,
    /// The source before the first block.
    pub head: Range<usize>,
    /// The source after the last block.
    pub tail: Range<usize>,
    pub blocks: Vec<Block>,
    pub notes: Vec<Note>,
    pub editable: bool,
}

/// Where a paragraph is in a story: among the top-level blocks, or in a table's cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlockPath {
    Top(usize),
    Cell {
        table: usize,
        row: usize,
        cell: usize,
        block: usize,
    },
}

impl BlockPath {
    /// The top-level block this is, or is inside.
    pub fn top(self) -> usize {
        match self {
            Self::Top(index) => index,
            Self::Cell { table, .. } => table,
        }
    }
}

/// A place in a story's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub path: BlockPath,
    pub offset: usize,
}

impl Position {
    pub fn new(path: BlockPath, offset: usize) -> Self {
        Self { path, offset }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.path, self.offset).cmp(&(other.path, other.offset))
    }
}

impl Story {
    pub fn paragraph(&self, path: BlockPath) -> Option<&Paragraph> {
        let block = match path {
            BlockPath::Top(index) => self.blocks.get(index)?,
            BlockPath::Cell {
                table,
                row,
                cell,
                block,
            } => match self.blocks.get(table)? {
                Block::Table(table) => table.rows.get(row)?.cells.get(cell)?.blocks.get(block)?,
                _ => return None,
            },
        };
        match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        }
    }

    pub fn paragraph_mut(&mut self, path: BlockPath) -> Option<&mut Paragraph> {
        match self.container_mut(path)?.get_mut(index_of(path))? {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        }
    }

    /// The list of blocks a paragraph sits in: the story's own, or a cell's.
    pub fn container(&self, path: BlockPath) -> Option<&Vec<Block>> {
        match path {
            BlockPath::Top(_) => Some(&self.blocks),
            BlockPath::Cell {
                table, row, cell, ..
            } => match self.blocks.get(table)? {
                Block::Table(table) => Some(&table.rows.get(row)?.cells.get(cell)?.blocks),
                _ => None,
            },
        }
    }

    pub fn container_mut(&mut self, path: BlockPath) -> Option<&mut Vec<Block>> {
        match path {
            BlockPath::Top(_) => Some(&mut self.blocks),
            BlockPath::Cell {
                table, row, cell, ..
            } => match self.blocks.get_mut(table)? {
                Block::Table(table) => {
                    Some(&mut table.rows.get_mut(row)?.cells.get_mut(cell)?.blocks)
                }
                _ => None,
            },
        }
    }

    /// Every paragraph in reading order, tables row by row and cell by cell.
    pub fn paths(&self) -> Vec<BlockPath> {
        let mut paths = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            match block {
                Block::Paragraph(_) => paths.push(BlockPath::Top(index)),
                Block::Table(table) => {
                    for (row, cells) in table.rows.iter().enumerate() {
                        for (cell, content) in cells.cells.iter().enumerate() {
                            for (block, inner) in content.blocks.iter().enumerate() {
                                if matches!(inner, Block::Paragraph(_)) {
                                    paths.push(BlockPath::Cell {
                                        table: index,
                                        row,
                                        cell,
                                        block,
                                    });
                                }
                            }
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
        paths
    }
}

/// A path's place within its own container.
pub fn index_of(path: BlockPath) -> usize {
    match path {
        BlockPath::Top(index) => index,
        BlockPath::Cell { block, .. } => block,
    }
}

/// The same container, another place in it.
pub fn with_index(path: BlockPath, index: usize) -> BlockPath {
    match path {
        BlockPath::Top(_) => BlockPath::Top(index),
        BlockPath::Cell {
            table, row, cell, ..
        } => BlockPath::Cell {
            table,
            row,
            cell,
            block: index,
        },
    }
}
