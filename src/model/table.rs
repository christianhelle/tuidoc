//! A table: rows of cells, each holding paragraphs of its own.

use std::ops::Range;
use std::sync::Arc;

use super::Block;

/// How a cell takes part in a cell merged down several rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Merge {
    None,
    /// The first cell of a merged column, which holds the content.
    Start,
    /// A cell below it, which is drawn as part of the one above.
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub open_tag: Arc<str>,
    /// The cell's properties as written.
    pub pre: Arc<str>,
    /// How many grid columns the cell spans.
    pub span: usize,
    pub merge: Merge,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub open_tag: Arc<str>,
    /// The row's properties as written.
    pub pre: Arc<str>,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Where the table sits in its part's source.
    pub source: Option<Range<usize>>,
    pub open_tag: Arc<str>,
    /// The table's properties and grid as written.
    pub pre: Arc<str>,
    /// The width of each grid column, in twentieths of a point.
    pub grid: Vec<u32>,
    pub rows: Vec<Row>,
}

impl Table {
    /// Whether anything inside differs from the source, so the table has to be written again.
    pub fn is_dirty(&self) -> bool {
        self.source.is_none()
            || self.rows.iter().any(|row| {
                row.cells.iter().any(|cell| {
                    cell.blocks.iter().any(|block| match block {
                        Block::Paragraph(paragraph) => {
                            paragraph.dirty || paragraph.source.is_none()
                        }
                        Block::Table(table) => table.is_dirty(),
                        Block::Opaque(_) => false,
                    })
                })
            })
    }

    /// How many grid columns the table has, going by its widest row if the grid says less.
    pub fn columns(&self) -> usize {
        self.rows
            .iter()
            .map(|row| row.cells.iter().map(|cell| cell.span).sum())
            .max()
            .unwrap_or(0)
            .max(self.grid.len())
    }
}
