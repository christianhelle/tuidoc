//! Lays a story out as rows of terminal cells, and says which place in the text each cell shows.

use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::model::{AtomKind, Block, BlockPath, Paragraph, Position, Story, Table};
use crate::numbering::Numbering;
use crate::styles::Styles;

pub const HEADER_STYLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
/// Table lines, placeholders and whatever else isn't the document's own text.
pub const LINE_STYLE: Style = Style::new().fg(Color::DarkGray);
const LINK_STYLE: Style = Style::new()
    .fg(Color::Cyan)
    .add_modifier(Modifier::UNDERLINED);
const REFERENCE_STYLE: Style = Style::new().fg(Color::Yellow);
const TAB_STOP: u16 = 4;
/// Twentieths of a point of indent that one terminal column stands for.
const TWIPS_PER_COLUMN: u32 = 240;

/// What one place in a paragraph looks like on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub text: String,
    pub width: u16,
    pub style: Style,
}

/// One screen line of a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLine {
    pub path: BlockPath,
    /// The place in the paragraph the line starts at.
    pub start: usize,
    pub glyphs: Vec<Glyph>,
    /// Where the line's column of text begins: the pane's edge, or a table cell's.
    pub left: u16,
    /// How wide that column is.
    pub room: u16,
    /// Where the first glyph is drawn.
    pub x: u16,
    /// A list's bullet or number, and where it is drawn.
    pub label: Option<(u16, String)>,
    /// Whether this is the paragraph's last line, where the cursor may sit past the end.
    pub last: bool,
}

impl TextLine {
    /// The place just past the line's last one.
    pub fn end(&self) -> usize {
        self.start + self.glyphs.len()
    }

    /// The column a place in the paragraph is drawn at.
    pub fn column_of(&self, offset: usize) -> u16 {
        let before = offset.saturating_sub(self.start).min(self.glyphs.len());
        self.x + self.glyphs[..before].iter().map(|g| g.width).sum::<u16>()
    }

    /// The place drawn at a column, or the nearest one. `past_end` lets it be the place after
    /// the last character, where typing would go.
    pub fn offset_at(&self, column: u16, past_end: bool) -> usize {
        let mut x = self.x;
        for (index, glyph) in self.glyphs.iter().enumerate() {
            if column < x + glyph.width.max(1) {
                return self.start + index;
            }
            x += glyph.width;
        }
        match past_end && self.last {
            true => self.end(),
            false => self.end().saturating_sub(1).max(self.start),
        }
    }

    fn covers(&self, column: u16) -> bool {
        (self.left..self.left + self.room.max(1)).contains(&column)
    }
}

/// One row of the screen: lines of text (several side by side in a table) and decoration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub lines: Vec<TextLine>,
    /// Things drawn that aren't editable text: table borders, placeholders.
    pub decor: Vec<(u16, String, Style)>,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub rows: Vec<Row>,
    /// Every paragraph that can hold the cursor, in reading order.
    pub paths: Vec<BlockPath>,
    pub width: u16,
}

struct Building<'a> {
    styles: &'a Styles,
    labels: HashMap<BlockPath, (String, usize)>,
}

impl Layout {
    pub fn build(story: &Story, styles: &Styles, numbering: &Numbering, width: u16) -> Self {
        let width = width.max(4);
        let paths = story.paths();
        let items: Vec<_> = paths
            .iter()
            .map(|path| {
                story
                    .paragraph(*path)
                    .and_then(|paragraph| styles.list_item(&paragraph.props))
            })
            .collect();
        let labels = numbering
            .labels(&items)
            .into_iter()
            .zip(&paths)
            .zip(&items)
            .filter_map(|((label, path), item)| {
                Some((*path, (label?, item.as_ref().map_or(0, |item| item.1))))
            })
            .collect();
        let building = Building { styles, labels };
        let mut rows: Vec<Row> = Vec::new();
        // Whether the block above wants a blank row after it, and whether it was a list item.
        let mut above: Option<(bool, bool)> = None;
        for (index, block) in story.blocks.iter().enumerate() {
            let (new_rows, empty, listed) = match block {
                Block::Paragraph(paragraph) => {
                    let path = BlockPath::Top(index);
                    let lines = building.wrap(paragraph, path, 0, width);
                    let rows = lines.into_iter().map(|line| Row {
                        lines: vec![line],
                        decor: Vec::new(),
                    });
                    (
                        rows.collect::<Vec<_>>(),
                        paragraph.is_empty(),
                        building.labels.contains_key(&path),
                    )
                }
                Block::Table(table) => (building.table(table, index, width), false, false),
                Block::Opaque(opaque) => {
                    let rows: Vec<Row> = opaque
                        .text
                        .iter()
                        .filter(|text| !text.trim().is_empty())
                        .map(|text| Row {
                            lines: Vec::new(),
                            decor: vec![(0, clip(text, width), LINE_STYLE)],
                        })
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    (rows, false, false)
                }
            };
            if let Some((above_empty, above_listed)) = above
                && !above_empty
                && !empty
                && !(above_listed && listed)
            {
                rows.push(Row::default());
            }
            rows.extend(new_rows);
            above = Some((empty, listed));
        }
        Self { rows, paths, width }
    }

    /// The row a place in the text is drawn on, and its line within that row.
    pub fn find(&self, position: Position) -> Option<(usize, &TextLine)> {
        let mut fallback = None;
        for (index, row) in self.rows.iter().enumerate() {
            for line in &row.lines {
                if line.path != position.path {
                    continue;
                }
                if (line.start..line.end()).contains(&position.offset) {
                    return Some((index, line));
                }
                if line.last {
                    fallback = Some((index, line));
                }
            }
        }
        fallback
    }

    pub fn row_of(&self, position: Position) -> usize {
        self.find(position).map_or(0, |(row, _)| row)
    }

    /// The place in the text nearest a column of a row.
    pub fn position_at(&self, row: usize, column: u16, past_end: bool) -> Option<Position> {
        let lines = &self.rows.get(row)?.lines;
        let line = lines
            .iter()
            .find(|line| line.covers(column))
            .or_else(|| lines.iter().min_by_key(|line| line.left.abs_diff(column)))?;
        Some(Position::new(line.path, line.offset_at(column, past_end)))
    }

    /// The nearest row at or after `from`, going `step` rows at a time, with text under a
    /// column: the next line down a paragraph, or the next line down a table's cell.
    pub fn text_row(&self, from: isize, step: isize, column: u16) -> Option<usize> {
        let mut row = from;
        let mut any = None;
        while row >= 0 && (row as usize) < self.rows.len() {
            let lines = &self.rows[row as usize].lines;
            if lines.iter().any(|line| line.covers(column)) {
                return Some(row as usize);
            }
            if any.is_none() && !lines.is_empty() {
                any = Some(row as usize);
            }
            row += step;
        }
        any
    }

    /// Whether a row holds text, as against a border or the blank between paragraphs.
    pub fn has_text(&self, row: usize) -> bool {
        self.rows.get(row).is_some_and(|row| !row.lines.is_empty())
    }

    /// Where a paragraph comes in reading order.
    pub fn index_of(&self, path: BlockPath) -> Option<usize> {
        self.paths.iter().position(|other| *other == path)
    }
}

impl Building<'_> {
    /// Breaks a paragraph into lines that fit `room` columns starting at `left`.
    fn wrap(&self, paragraph: &Paragraph, path: BlockPath, left: u16, room: u16) -> Vec<TextLine> {
        let room = room.max(1);
        let heading = self.styles.heading_level(&paragraph.props).is_some();
        let title = self.styles.is_title(&paragraph.props);
        let align = self
            .styles
            .paragraph_prop(&paragraph.props, "jc")
            .and_then(|raw| crate::docx::xml::attr_of(&raw, "val"));
        let label = self.labels.get(&path);
        let (label_x, indent) = match label {
            Some((label, depth)) => {
                let label_x = (*depth as u16 * 2).min(room / 2);
                (label_x, label_x + label.width() as u16 + 1)
            }
            None => (0, self.indent(paragraph).min(room / 2)),
        };
        let text_room = room.saturating_sub(indent).max(1);

        let glyphs: Vec<(Glyph, Break)> = paragraph
            .cells()
            .iter()
            .map(|cell| {
                let format = self.styles.format(&paragraph.props, cell.props);
                let mut style = Style::new();
                for (on, modifier) in [
                    (format.bold, Modifier::BOLD),
                    (format.italic, Modifier::ITALIC),
                    (format.underline, Modifier::UNDERLINED),
                    (format.strike, Modifier::CROSSED_OUT),
                ] {
                    if on {
                        style = style.add_modifier(modifier);
                    }
                }
                if heading || title {
                    style = style.patch(HEADER_STYLE);
                }
                if cell.wraps.iter().any(|wrapper| wrapper.link.is_some()) {
                    style = style.patch(LINK_STYLE);
                }
                glyph(cell.text, cell.atom, style)
            })
            .collect();

        // Where each line starts.
        let mut starts = vec![0];
        let mut width = 0u16;
        let mut break_at: Option<usize> = None;
        let mut widths = Vec::with_capacity(glyphs.len());
        for (index, (glyph, kind)) in glyphs.iter().enumerate() {
            let line_start = starts[starts.len() - 1];
            if *kind == Break::Alone && index > line_start {
                starts.push(index);
                width = 0;
                break_at = None;
            }
            let line_start = starts[starts.len() - 1];
            let mut w = match *kind {
                Break::Tab => TAB_STOP - width % TAB_STOP,
                _ => glyph.width,
            };
            let space = *kind == Break::Space;
            if width + w > text_room && index > line_start && !space {
                let at = break_at.filter(|at| *at > line_start).unwrap_or(index);
                starts.push(at);
                width = widths[at..index].iter().sum();
                break_at = None;
                if *kind == Break::Tab {
                    w = TAB_STOP - width % TAB_STOP;
                }
            }
            widths.push(w);
            width += w;
            if space || *kind == Break::Tab {
                break_at = Some(index + 1);
            }
            if matches!(kind, Break::After | Break::Alone) && index + 1 < glyphs.len() {
                starts.push(index + 1);
                width = 0;
                break_at = None;
            }
        }

        let count = starts.len();
        let mut all: Vec<Option<Glyph>> = glyphs
            .into_iter()
            .zip(widths)
            .map(|((mut glyph, kind), width)| {
                if kind == Break::Tab {
                    glyph.text = " ".repeat(width as usize);
                }
                glyph.width = width;
                Some(glyph)
            })
            .collect();
        (0..count)
            .map(|line| {
                let start = starts[line];
                let end = starts.get(line + 1).copied().unwrap_or(all.len());
                let glyphs: Vec<Glyph> = all[start..end]
                    .iter_mut()
                    .filter_map(Option::take)
                    .collect();
                let used: u16 = glyphs.iter().map(|g| g.width).sum();
                let slack = text_room.saturating_sub(used);
                let shift = match align.as_deref() {
                    Some("center") => slack / 2,
                    Some("right") | Some("end") => slack,
                    _ if title => slack / 2,
                    _ => 0,
                };
                TextLine {
                    path,
                    start,
                    glyphs,
                    left,
                    room,
                    x: left + indent + shift,
                    label: (line == 0)
                        .then(|| label.map(|(label, _)| (left + label_x, label.clone())))
                        .flatten(),
                    last: line + 1 == count,
                }
            })
            .collect()
    }

    /// A paragraph's left indent, in columns.
    fn indent(&self, paragraph: &Paragraph) -> u16 {
        let Some(raw) = self.styles.paragraph_prop(&paragraph.props, "ind") else {
            return 0;
        };
        let twips = crate::docx::xml::attr_of(&raw, "left")
            .or_else(|| crate::docx::xml::attr_of(&raw, "start"))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        (twips / TWIPS_PER_COLUMN).min(40) as u16
    }

    /// Lays a table out as a grid of boxes, each cell's paragraphs wrapped to its width.
    fn table(&self, table: &Table, index: usize, width: u16) -> Vec<Row> {
        let columns = table.columns().max(1);
        let widths = column_widths(&table.grid, columns, width);
        // The x of the border left of each grid column, and of the last border.
        let mut edges = vec![0u16];
        for width in &widths {
            edges.push(edges[edges.len() - 1] + width + 1);
        }
        let boundaries = |row: Option<&crate::model::table::Row>| -> Vec<u16> {
            let mut found = vec![0];
            let mut column = 0;
            for cell in row.map_or(&[][..], |row| &row.cells[..]) {
                column = (column + cell.span).min(columns);
                found.push(edges[column]);
            }
            found
        };
        let last_edge = edges[columns];
        let rule = |above: &[u16], below: &[u16]| -> Row {
            let mut text = String::new();
            for x in 0..=last_edge {
                let (up, down) = (above.contains(&x), below.contains(&x));
                text.push(match (x, up, down) {
                    (0, false, _) => '┌',
                    (0, _, false) => '└',
                    (0, ..) => '├',
                    (x, false, _) if x == last_edge => '┐',
                    (x, _, false) if x == last_edge => '┘',
                    (x, ..) if x == last_edge => '┤',
                    (_, true, true) => '┼',
                    (_, true, false) => '┴',
                    (_, false, true) => '┬',
                    _ => '─',
                });
            }
            Row {
                lines: Vec::new(),
                decor: vec![(0, text, LINE_STYLE)],
            }
        };

        let mut rows = Vec::new();
        for (row_index, row) in table.rows.iter().enumerate() {
            let above = row_index.checked_sub(1).map(|above| &table.rows[above]);
            let mut top = boundaries(above);
            let mut here = boundaries(Some(row));
            for found in [&mut top, &mut here] {
                if !found.contains(&last_edge) {
                    found.push(last_edge);
                }
            }
            rows.push(rule(if above.is_some() { &top } else { &[] }, &here));

            // Each cell's lines, top to bottom.
            let mut column = 0;
            let mut cells: Vec<(u16, Vec<Result<TextLine, String>>)> = Vec::new();
            for (cell_index, cell) in row.cells.iter().enumerate() {
                let start = edges[column.min(columns)];
                column = (column + cell.span).min(columns);
                let end = edges[column];
                let left = start + 2;
                let room = end.saturating_sub(start + 3).max(1);
                let mut lines = Vec::new();
                for (block_index, block) in cell.blocks.iter().enumerate() {
                    match block {
                        Block::Paragraph(paragraph) => {
                            let path = BlockPath::Cell {
                                table: index,
                                row: row_index,
                                cell: cell_index,
                                block: block_index,
                            };
                            lines
                                .extend(self.wrap(paragraph, path, left, room).into_iter().map(Ok));
                        }
                        Block::Table(_) => lines.push(Err("[table]".to_string())),
                        Block::Opaque(opaque) if opaque.raw.starts_with("<w:tbl") => {
                            lines.push(Err("[table]".to_string()));
                        }
                        Block::Opaque(opaque) => {
                            lines.extend(opaque.text.iter().map(|text| Err(text.clone())));
                        }
                    }
                }
                cells.push((
                    left,
                    lines
                        .into_iter()
                        .map(|l| l.map_err(|t| clip(&t, room)))
                        .collect(),
                ));
            }
            let height = cells
                .iter()
                .map(|(_, lines)| lines.len())
                .max()
                .unwrap_or(1)
                .max(1);
            let mut content: Vec<Row> = (0..height)
                .map(|_| Row {
                    lines: Vec::new(),
                    decor: here
                        .iter()
                        .map(|x| (*x, "│".to_string(), LINE_STYLE))
                        .collect(),
                })
                .collect();
            for (left, lines) in cells {
                for (line_index, line) in lines.into_iter().enumerate() {
                    match line {
                        Ok(line) => content[line_index].lines.push(line),
                        Err(text) => content[line_index].decor.push((left, text, LINE_STYLE)),
                    }
                }
            }
            rows.extend(content);
        }
        let bottom = {
            let mut found = boundaries(table.rows.last());
            if !found.contains(&last_edge) {
                found.push(last_edge);
            }
            found
        };
        rows.push(rule(&bottom, &[]));
        rows
    }
}

/// How a glyph takes part in line breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Break {
    No,
    /// A line may break after it, and it may hang past the edge.
    Space,
    Tab,
    /// The line ends after it.
    After,
    /// It has a line to itself.
    Alone,
}

fn glyph(text: Option<&str>, atom: Option<&AtomKind>, style: Style) -> (Glyph, Break) {
    let placeholder = |text: String, style: Style, kind: Break| {
        let width = text.width() as u16;
        (Glyph { text, width, style }, kind)
    };
    match (text, atom) {
        (Some(text), _) => {
            let shown = shown(text);
            let kind = match text.chars().all(char::is_whitespace) && text != "\u{a0}" {
                true => Break::Space,
                false => Break::No,
            };
            placeholder(shown, style, kind)
        }
        (None, Some(AtomKind::Tab)) => placeholder(" ".into(), style, Break::Tab),
        (None, Some(AtomKind::LineBreak)) => placeholder(String::new(), style, Break::After),
        (None, Some(AtomKind::PageBreak)) => {
            placeholder("──── page break ────".into(), LINE_STYLE, Break::Alone)
        }
        (None, Some(AtomKind::Image(alt))) => {
            let text = match alt.is_empty() {
                true => "[image]".to_string(),
                false => format!("[image: {}]", shown(alt)),
            };
            placeholder(text, LINE_STYLE, Break::No)
        }
        (None, Some(AtomKind::Footnote(id) | AtomKind::Endnote(id))) => {
            placeholder(format!("[{id}]"), REFERENCE_STYLE, Break::No)
        }
        (None, Some(AtomKind::Comment(id))) => {
            placeholder(format!("[c{id}]"), REFERENCE_STYLE, Break::No)
        }
        _ => placeholder("[…]".into(), LINE_STYLE, Break::No),
    }
}

/// Text made safe to draw: control and zero-width characters would upset the terminal's idea
/// of where things are.
pub fn shown(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && !matches!(c, '\u{200b}'..='\u{200f}' | '\u{feff}'))
        .collect()
}

/// Text cut to fit a width.
fn clip(text: &str, width: u16) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in shown(text).chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > width as usize {
            break;
        }
        used += w;
        out.push(ch);
    }
    out
}

/// Shares the pane's width between a table's columns in proportion to the widths the document
/// gives them, leaving room for the borders.
fn column_widths(grid: &[u32], columns: usize, width: u16) -> Vec<u16> {
    let available = usize::from(width)
        .saturating_sub(columns + 1)
        .max(columns * 3);
    let weights: Vec<usize> = (0..columns)
        .map(|column| grid.get(column).copied().filter(|w| *w > 0).unwrap_or(1000) as usize)
        .collect();
    let total: usize = weights.iter().sum::<usize>().max(1);
    // A table narrower than the page doesn't need the whole pane; 120 twips a column is a
    // comfortable reading width.
    let wanted = (total / 120).clamp(columns * 3, available);
    let mut widths: Vec<u16> = weights
        .iter()
        .map(|weight| (weight * wanted / total).max(3) as u16)
        .collect();
    let mut over = widths
        .iter()
        .map(|w| *w as usize)
        .sum::<usize>()
        .saturating_sub(available);
    while over > 0 {
        let Some(widest) = widths.iter_mut().filter(|w| **w > 3).max() else {
            break;
        };
        *widest -= 1;
        over -= 1;
    }
    widths
}
