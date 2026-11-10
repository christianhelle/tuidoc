//! What undo and redo go through, and what yanked text waits in.

use crate::model::{Block, Fragment, Position};

/// How many changes can be undone.
const DEPTH: usize = 200;

/// One command's effect on the body: a run of blocks replaced by another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Tells changes apart, so the document knows whether it is as it was last saved.
    pub id: u64,
    /// The first block replaced.
    pub at: usize,
    pub before: Vec<Block>,
    pub after: Vec<Block>,
    pub cursor_before: Position,
    pub cursor_after: Position,
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Change>,
    redo: Vec<Change>,
}

impl History {
    /// Records a change just made. There is nothing to redo after a new change.
    pub fn record(&mut self, change: Change) {
        self.undo.push(change);
        self.redo.clear();
        if self.undo.len() > DEPTH {
            self.undo.remove(0);
        }
    }

    pub fn take_undo(&mut self) -> Option<Change> {
        let change = self.undo.pop()?;
        self.redo.push(change.clone());
        Some(change)
    }

    pub fn take_redo(&mut self) -> Option<Change> {
        let change = self.redo.pop()?;
        self.undo.push(change.clone());
        Some(change)
    }

    /// The change the document currently stands at, if any has been made and not undone.
    pub fn latest(&self) -> Option<u64> {
        self.undo.last().map(|change| change.id)
    }
}

/// What the last yank, delete or change took, ready to be pasted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Register {
    /// Text from inside paragraphs, one fragment per paragraph it ran through.
    Chars(Vec<Fragment>),
    /// Whole paragraphs and tables.
    Lines(Vec<Block>),
}

impl Register {
    /// The text alone, as it goes to the system clipboard.
    pub fn text(&self) -> String {
        match self {
            Self::Chars(fragments) => fragments
                .iter()
                .map(Fragment::text)
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Lines(blocks) => {
                let mut out = String::new();
                for block in blocks {
                    block_text(block, &mut out);
                }
                out
            }
        }
    }
}

fn block_text(block: &Block, out: &mut String) {
    match block {
        Block::Paragraph(paragraph) => {
            out.push_str(&paragraph.text());
            out.push('\n');
        }
        Block::Table(table) => {
            for row in &table.rows {
                let cells: Vec<String> = row
                    .cells
                    .iter()
                    .map(|cell| {
                        let mut text = String::new();
                        for block in &cell.blocks {
                            block_text(block, &mut text);
                        }
                        text.trim_end().replace('\n', " ")
                    })
                    .collect();
                out.push_str(&cells.join("\t"));
                out.push('\n');
            }
        }
        Block::Opaque(opaque) => {
            for line in &opaque.text {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
}
