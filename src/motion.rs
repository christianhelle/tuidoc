//! The ways the cursor moves, and where each one lands.

use crate::layout::Layout;
use crate::model::inline::OBJECT;
use crate::model::{BlockPath, Position, Story};
use crate::styles::Styles;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    /// One screen line up or down, which in wrapped prose is what moving by lines means.
    Up,
    Down,
    /// Words; `true` for the kind that only whitespace divides.
    WordForward(bool),
    WordBack(bool),
    WordEnd(bool),
    ParagraphStart,
    FirstNonBlank,
    ParagraphEnd,
    LineStart,
    LineEnd,
    PreviousParagraph,
    NextParagraph,
    PreviousHeading,
    NextHeading,
    /// The first paragraph, or with a count the nth.
    First,
    /// The last paragraph, or with a count the nth.
    Last,
    ScreenTop,
    ScreenMiddle,
    ScreenBottom,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    Find {
        target: char,
        forward: bool,
        till: bool,
    },
    NextCell,
    PreviousCell,
}

impl Motion {
    /// Whether an operator given this motion acts on whole paragraphs.
    pub fn is_linewise(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::PreviousParagraph
                | Self::NextParagraph
                | Self::PreviousHeading
                | Self::NextHeading
                | Self::First
                | Self::Last
                | Self::ScreenTop
                | Self::ScreenMiddle
                | Self::ScreenBottom
                | Self::HalfPageDown
                | Self::HalfPageUp
                | Self::PageDown
                | Self::PageUp
        )
    }

    /// Whether an operator given this motion takes the character it lands on too.
    pub fn is_inclusive(self) -> bool {
        matches!(
            self,
            Self::WordEnd(_) | Self::ParagraphEnd | Self::LineEnd | Self::Find { .. }
        )
    }

    /// Whether the motion keeps to a column, so that a run of them doesn't drift sideways
    /// through short lines.
    pub fn is_vertical(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::HalfPageDown
                | Self::HalfPageUp
                | Self::PageDown
                | Self::PageUp
        )
    }
}

/// What a motion has to go on.
pub struct View<'a> {
    pub story: &'a Story,
    pub layout: &'a Layout,
    pub styles: &'a Styles,
    /// The first row on screen and how many fit.
    pub top: usize,
    pub height: usize,
    /// The column a run of vertical moves is keeping to.
    pub goal: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punctuation,
}

fn class(grapheme: &str, big: bool) -> Class {
    let Some(first) = grapheme.chars().next() else {
        return Class::Space;
    };
    if first.is_whitespace() {
        Class::Space
    } else if big || first.is_alphanumeric() || first == '_' {
        Class::Word
    } else {
        Class::Punctuation
    }
}

impl View<'_> {
    fn graphemes(&self, path: BlockPath) -> Vec<String> {
        self.story
            .paragraph(path)
            .map(|paragraph| paragraph.graphemes())
            .unwrap_or_default()
    }

    fn len(&self, path: BlockPath) -> usize {
        self.story.paragraph(path).map_or(0, |p| p.len())
    }

    /// The paragraph a number of paragraphs on in reading order, stopping at the ends.
    fn neighbour(&self, path: BlockPath, by: isize) -> Option<BlockPath> {
        let index = self.layout.index_of(path)? as isize + by;
        let last = self.layout.paths.len().checked_sub(1)? as isize;
        self.layout
            .paths
            .get(index.clamp(0, last) as usize)
            .copied()
    }

    fn strict_neighbour(&self, path: BlockPath, by: isize) -> Option<BlockPath> {
        let index = self.layout.index_of(path)? as isize + by;
        usize::try_from(index)
            .ok()
            .and_then(|index| self.layout.paths.get(index))
            .copied()
    }

    /// Where a motion lands from a place, or nothing if it can't go anywhere.
    pub fn land(&self, motion: Motion, from: Position, count: Option<usize>) -> Option<Position> {
        let times = count.unwrap_or(1).max(1);
        let len = self.len(from.path);
        let at = |offset: usize| Some(Position::new(from.path, offset));
        match motion {
            Motion::Left => at(from.offset.saturating_sub(times)),
            Motion::Right => at((from.offset + times).min(len)),
            Motion::Up => self.vertical(from, -(times as isize)),
            Motion::Down => self.vertical(from, times as isize),
            Motion::HalfPageDown => self.vertical(from, (self.height / 2).max(1) as isize),
            Motion::HalfPageUp => self.vertical(from, -((self.height / 2).max(1) as isize)),
            Motion::PageDown => self.vertical(from, self.height.max(1) as isize),
            Motion::PageUp => self.vertical(from, -(self.height.max(1) as isize)),
            Motion::WordForward(big) => {
                Some((0..times).fold(from, |position, _| self.word_forward(position, big)))
            }
            Motion::WordBack(big) => {
                Some((0..times).fold(from, |position, _| self.word_back(position, big)))
            }
            Motion::WordEnd(big) => {
                Some((0..times).fold(from, |position, _| self.word_end(position, big)))
            }
            Motion::ParagraphStart => at(0),
            Motion::FirstNonBlank => at(self
                .graphemes(from.path)
                .iter()
                .position(|g| class(g, true) != Class::Space)
                .unwrap_or(0)),
            Motion::ParagraphEnd => at(len.saturating_sub(1)),
            Motion::LineStart => {
                let (_, line) = self.layout.find(from)?;
                at(line.start)
            }
            Motion::LineEnd => {
                let (_, line) = self.layout.find(from)?;
                at(line.end().saturating_sub(1).max(line.start))
            }
            Motion::PreviousParagraph => Some(Position::new(
                self.neighbour(from.path, -(times as isize))?,
                0,
            )),
            Motion::NextParagraph => {
                Some(Position::new(self.neighbour(from.path, times as isize)?, 0))
            }
            Motion::PreviousHeading | Motion::NextHeading => {
                let forward = motion == Motion::NextHeading;
                let mut index = self.layout.index_of(from.path)?;
                let mut found = None;
                for _ in 0..times {
                    loop {
                        index = match forward {
                            true => index + 1,
                            false => index.checked_sub(1)?,
                        };
                        let path = *self.layout.paths.get(index)?;
                        let heading = self
                            .story
                            .paragraph(path)
                            .is_some_and(|p| self.styles.heading_level(&p.props).is_some());
                        if heading {
                            found = Some(path);
                            break;
                        }
                    }
                }
                Some(Position::new(found?, 0))
            }
            Motion::First | Motion::Last => {
                let last = self.layout.paths.len().checked_sub(1)?;
                let index = match (count, motion) {
                    (Some(nth), _) => nth.saturating_sub(1).min(last),
                    (None, Motion::First) => 0,
                    (None, _) => last,
                };
                Some(Position::new(self.layout.paths[index], 0))
            }
            Motion::ScreenTop | Motion::ScreenMiddle | Motion::ScreenBottom => {
                let last = self.layout.rows.len().checked_sub(1)?;
                let bottom = (self.top + self.height.saturating_sub(1)).min(last);
                let (row, step) = match motion {
                    Motion::ScreenTop => (self.top.min(last), 1),
                    Motion::ScreenMiddle => ((self.top + bottom) / 2, 1),
                    _ => (bottom, -1),
                };
                let row = self.layout.text_row(row as isize, step, 0)?;
                let line = self.layout.rows[row].lines.first()?;
                Some(Position::new(line.path, line.start))
            }
            Motion::Find {
                target,
                forward,
                till,
            } => {
                let graphemes = self.graphemes(from.path);
                let matches = |index: &usize| graphemes[*index].starts_with(target);
                let mut offset = from.offset;
                for _ in 0..times {
                    offset = match forward {
                        true => (offset + 1..graphemes.len()).find(matches)?,
                        false => (0..offset).rev().find(matches)?,
                    };
                }
                at(match (till, forward) {
                    (false, _) => offset,
                    (true, true) => offset - 1,
                    (true, false) => offset + 1,
                })
            }
            Motion::NextCell | Motion::PreviousCell => {
                let BlockPath::Cell {
                    table, row, cell, ..
                } = from.path
                else {
                    return None;
                };
                let step = if motion == Motion::NextCell { 1 } else { -1 };
                let mut path = from.path;
                loop {
                    path = self.strict_neighbour(path, step)?;
                    let same = matches!(path, BlockPath::Cell { table: t, row: r, cell: c, .. }
                        if (t, r, c) == (table, row, cell));
                    if !same {
                        break;
                    }
                }
                // Coming back into a cell lands on its first paragraph, not its last.
                while step < 0
                    && let Some(before) = self.strict_neighbour(path, -1)
                    && with_cell(before) == with_cell(path)
                    && matches!(path, BlockPath::Cell { .. })
                {
                    path = before;
                }
                Some(Position::new(path, 0))
            }
        }
    }

    /// Moves a number of text rows down, or up for a negative number, keeping to a column.
    fn vertical(&self, from: Position, by: isize) -> Option<Position> {
        let (mut row, line) = self.layout.find(from)?;
        let column = self.goal.unwrap_or_else(|| line.column_of(from.offset));
        let step = by.signum();
        for _ in 0..by.unsigned_abs() {
            match self.layout.text_row(row as isize + step, step, column) {
                Some(next) => row = next,
                None => break,
            }
        }
        self.layout.position_at(row, column, false)
    }

    fn word_forward(&self, from: Position, big: bool) -> Position {
        let graphemes = self.graphemes(from.path);
        let mut index = from.offset;
        if let Some(start) = graphemes.get(index).map(|g| class(g, big)) {
            if start != Class::Space {
                while index < graphemes.len() && class(&graphemes[index], big) == start {
                    index += 1;
                }
            }
            while index < graphemes.len() && class(&graphemes[index], big) == Class::Space {
                index += 1;
            }
        }
        if index < graphemes.len() {
            return Position::new(from.path, index);
        }
        let mut path = from.path;
        while let Some(next) = self.strict_neighbour(path, 1) {
            path = next;
            let graphemes = self.graphemes(path);
            let first = graphemes.iter().position(|g| class(g, big) != Class::Space);
            // An empty paragraph counts as a word, as an empty line does in vim.
            match (first, graphemes.is_empty()) {
                (Some(first), _) => return Position::new(path, first),
                (None, true) => return Position::new(path, 0),
                (None, false) => {}
            }
        }
        Position::new(from.path, graphemes.len())
    }

    fn word_back(&self, from: Position, big: bool) -> Position {
        let mut path = from.path;
        let mut graphemes = self.graphemes(path);
        let mut index = from.offset.min(graphemes.len());
        loop {
            while index > 0 && class(&graphemes[index - 1], big) == Class::Space {
                index -= 1;
            }
            if index > 0 {
                let kind = class(&graphemes[index - 1], big);
                while index > 0 && class(&graphemes[index - 1], big) == kind {
                    index -= 1;
                }
                return Position::new(path, index);
            }
            let Some(previous) = self.strict_neighbour(path, -1) else {
                return Position::new(path, 0);
            };
            path = previous;
            graphemes = self.graphemes(path);
            index = graphemes.len();
            if graphemes.is_empty() {
                return Position::new(path, 0);
            }
        }
    }

    fn word_end(&self, from: Position, big: bool) -> Position {
        let mut path = from.path;
        let mut graphemes = self.graphemes(path);
        let mut index = from.offset + 1;
        loop {
            while index < graphemes.len() && class(&graphemes[index], big) == Class::Space {
                index += 1;
            }
            if index < graphemes.len() {
                let kind = class(&graphemes[index], big);
                while index + 1 < graphemes.len() && class(&graphemes[index + 1], big) == kind {
                    index += 1;
                }
                return Position::new(path, index);
            }
            let Some(next) = self.strict_neighbour(path, 1) else {
                return Position::new(path, graphemes.len().saturating_sub(1));
            };
            path = next;
            graphemes = self.graphemes(path);
            index = 0;
        }
    }

    /// The word under a place: its start and the place past its end. `around` takes the
    /// spaces after it as well.
    pub fn word_at(&self, at: Position, around: bool) -> Option<(usize, usize)> {
        let graphemes = self.graphemes(at.path);
        let kind = class(graphemes.get(at.offset)?, false);
        let mut start = at.offset;
        while start > 0 && class(&graphemes[start - 1], false) == kind {
            start -= 1;
        }
        let mut end = at.offset + 1;
        while end < graphemes.len() && class(&graphemes[end], false) == kind {
            end += 1;
        }
        if around {
            while end < graphemes.len() && class(&graphemes[end], false) == Class::Space {
                end += 1;
            }
        }
        Some((start, end))
    }

    /// The next place after `from` where `needle` occurs, ignoring case, going round the end
    /// of the story back to its start; or the previous one, going backwards.
    pub fn search(&self, needle: &str, from: Position, forward: bool) -> Option<Position> {
        let needle: Vec<String> =
            unicode_segmentation::UnicodeSegmentation::graphemes(needle, true)
                .map(str::to_lowercase)
                .collect();
        if needle.is_empty() {
            return None;
        }
        let mut found: Vec<Position> = Vec::new();
        for path in &self.layout.paths {
            let hay: Vec<String> = self
                .graphemes(*path)
                .iter()
                .map(|g| match g.starts_with(OBJECT) {
                    true => "\0".to_string(),
                    false => g.to_lowercase(),
                })
                .collect();
            if hay.len() < needle.len() {
                continue;
            }
            found.extend(
                (0..=hay.len() - needle.len())
                    .filter(|start| hay[*start..*start + needle.len()] == needle[..])
                    .map(|start| Position::new(*path, start)),
            );
        }
        match forward {
            true => found
                .iter()
                .find(|position| **position > from)
                .or(found.first()),
            false => found
                .iter()
                .rev()
                .find(|position| **position < from)
                .or(found.last()),
        }
        .copied()
    }
}

/// A path with its place inside the cell forgotten, to tell whether two share a cell.
fn with_cell(path: BlockPath) -> BlockPath {
    crate::model::with_index(path, 0)
}
