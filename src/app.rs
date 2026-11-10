//! The editor's state, and what every key does to it in each mode.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::changes::Register;
use crate::command::{Command, Grammar, InsertAt, Object, Operator, Scroll};
use crate::document::{Document, Refusal};
use crate::docx::saving;
use crate::file_dialog::{DialogAction, DialogKind, FileDialog};
use crate::formatting::Toggle;
use crate::help::{self, HelpTable};
use crate::keys::{is_ctrl, is_text_input};
use crate::layout::Layout;
use crate::line_edit::LineEdit;
use crate::model::{AtomKind, Block, BlockPath, Position, Story, StoryKind, index_of, with_index};
use crate::motion::{Motion, View};
use crate::picker::{Picked, Picker};
use crate::status::Status;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Search,
    GoTo,
    Help,
    ConfirmDiscard,
    FileDialog,
    StylePicker,
    Outline,
    Peek,
}

/// What was asked for that unsaved changes stand in the way of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStep {
    Quit,
    OpenFile,
    Reload,
}

/// A heading as the outline lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub path: BlockPath,
}

#[derive(Debug)]
pub struct App {
    doc: Document,
    /// Which story is on screen.
    story: usize,
    /// Where the cursor is in each story, and the first row each shows.
    cursors: Vec<Position>,
    tops: Vec<usize>,
    /// Where a visual selection started.
    anchor: Position,
    mode: Mode,
    grammar: Grammar,
    layout: Layout,
    /// The story, document version and width the layout was made for.
    stamp: Option<(usize, u64, u16)>,
    width: u16,
    height: usize,
    /// The column a run of vertical moves keeps to.
    goal: Option<u16>,
    status: Status,
    line: LineEdit,
    search_query: String,
    last_find: Option<(char, bool, bool)>,
    register: Option<Register>,
    /// Text waiting to be handed to the system clipboard.
    clipboard: Option<String>,
    file_dialog: Option<FileDialog>,
    dialog_directory: PathBuf,
    next_step: Option<NextStep>,
    picker: Option<Picker>,
    /// The ids of the styles the picker lists, in its order.
    picker_styles: Vec<Option<String>>,
    outline_open: bool,
    outline_at: usize,
    peek: Vec<String>,
    help_from: Mode,
    should_quit: bool,
}

impl App {
    pub fn new(doc: Document) -> Self {
        let cursors = doc
            .stories
            .iter()
            .map(|story| {
                let path = story.paths().first().copied().unwrap_or(BlockPath::Top(0));
                Position::new(path, 0)
            })
            .collect::<Vec<_>>();
        let dialog_directory = doc
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let mut app = Self {
            anchor: cursors[0],
            tops: vec![0; cursors.len()],
            cursors,
            doc,
            story: 0,
            mode: Mode::Normal,
            grammar: Grammar::default(),
            layout: Layout::default(),
            stamp: None,
            width: 76,
            height: 20,
            goal: None,
            status: Status::default(),
            line: LineEdit::default(),
            search_query: String::new(),
            last_find: None,
            register: None,
            clipboard: None,
            file_dialog: None,
            dialog_directory,
            next_step: None,
            picker: None,
            picker_styles: Vec::new(),
            outline_open: false,
            outline_at: 0,
            peek: Vec::new(),
            help_from: Mode::Normal,
            should_quit: false,
        };
        app.settle();
        app
    }

    // ---- what the screen is drawn from ----

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn story_index(&self) -> usize {
        self.story
    }

    pub fn story(&self) -> &Story {
        &self.doc.stories[self.story]
    }

    pub fn cursor(&self) -> Position {
        self.cursors[self.story]
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn top(&self) -> usize {
        self.tops[self.story]
    }

    pub fn status(&self) -> &str {
        self.status.text()
    }

    pub fn status_is_error(&self) -> bool {
        self.status.is_error()
    }

    pub fn line(&self) -> &LineEdit {
        &self.line
    }

    pub fn pending_keys(&self) -> String {
        self.grammar.pending_keys()
    }

    pub fn file_dialog(&self) -> Option<&FileDialog> {
        self.file_dialog.as_ref()
    }

    pub fn picker(&self) -> Option<&Picker> {
        self.picker.as_ref()
    }

    pub fn next_step(&self) -> Option<NextStep> {
        self.next_step
    }

    pub fn outline_is_open(&self) -> bool {
        self.outline_open
    }

    pub fn outline_at(&self) -> usize {
        self.outline_at
    }

    pub fn peek(&self) -> &[String] {
        &self.peek
    }

    pub fn register(&self) -> Option<&Register> {
        self.register.as_ref()
    }

    /// The keys worth listing for the mode help was asked from.
    pub fn help_table(&self) -> HelpTable {
        help::table(self.help_from)
    }

    /// Text that should go to the system clipboard, once.
    pub fn take_clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    /// The cursor's paragraph number and how many there are, for the status line.
    pub fn place(&self) -> (usize, usize) {
        let index = self.layout.index_of(self.cursor().path).unwrap_or(0);
        (index + 1, self.layout.paths.len())
    }

    /// Whether a place in the text is part of the visual selection.
    pub fn is_selected(&self, path: BlockPath, offset: usize) -> bool {
        let (first, last) = self.selection_ends();
        match self.mode {
            Mode::Visual => {
                let position = Position::new(path, offset);
                first <= position && position <= last
            }
            Mode::VisualLine => first.path <= path && path <= last.path,
            _ => false,
        }
    }

    /// The headings of the story on screen.
    pub fn outline(&self) -> Vec<Heading> {
        let story = self.story();
        self.layout
            .paths
            .iter()
            .filter_map(|path| {
                let paragraph = story.paragraph(*path)?;
                let level = match self.doc.styles.is_title(&paragraph.props) {
                    true => 1,
                    false => self.doc.styles.heading_level(&paragraph.props)?,
                };
                Some(Heading {
                    level,
                    text: paragraph.text(),
                    path: *path,
                })
            })
            .collect()
    }

    // ---- keeping the view in step ----

    /// Tells the app how much room the text has.
    pub fn fit(&mut self, width: u16, height: u16) {
        self.width = width.max(4);
        self.height = usize::from(height).max(1);
        self.settle();
    }

    fn relayout(&mut self) {
        let stamp = Some((self.story, self.doc.version(), self.width));
        if self.stamp != stamp {
            self.layout = Layout::build(
                &self.doc.stories[self.story],
                &self.doc.styles,
                &self.doc.numbering,
                self.width,
            );
            self.stamp = stamp;
        }
    }

    fn typing(&self) -> bool {
        self.mode == Mode::Insert
    }

    /// Brings the layout up to date and the cursor back onto text and into view.
    fn settle(&mut self) {
        self.relayout();
        let mut cursor = self.cursor();
        if self.layout.index_of(cursor.path).is_none() {
            let path = self
                .layout
                .paths
                .iter()
                .find(|path| **path >= cursor.path)
                .or(self.layout.paths.last())
                .copied()
                .unwrap_or(cursor.path);
            cursor = Position::new(path, 0);
        }
        let len = self.story().paragraph(cursor.path).map_or(0, |p| p.len());
        let last = match self.typing() {
            true => len,
            false => len.saturating_sub(1),
        };
        cursor.offset = cursor.offset.min(last);
        self.cursors[self.story] = cursor;
        let row = self.layout.row_of(cursor);
        let top = &mut self.tops[self.story];
        if row < *top {
            *top = row;
        } else if row >= *top + self.height {
            *top = row + 1 - self.height;
        }
        // The first paragraph's row is 0, so nothing above it is ever worth scrolling to.
        *top = (*top).min(self.layout.rows.len().saturating_sub(1));
    }

    fn set_cursor(&mut self, position: Position) {
        self.cursors[self.story] = position;
    }

    fn view(&self) -> View<'_> {
        View {
            story: self.story(),
            layout: &self.layout,
            styles: &self.doc.styles,
            top: self.top(),
            height: self.height,
            goal: self.goal,
        }
    }

    fn len(&self, path: BlockPath) -> usize {
        self.story().paragraph(path).map_or(0, |p| p.len())
    }

    fn say(&mut self, text: impl Into<String>) {
        self.status.say(text);
    }

    fn report(&mut self, text: impl Into<String>) {
        self.status.report_error(text);
    }

    // ---- keys ----

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Windows reports a key's release as well as its press.
        if key.kind == KeyEventKind::Release {
            return;
        }
        if !matches!(self.mode, Mode::GoTo | Mode::FileDialog) {
            self.status.clear();
        }
        match self.mode {
            Mode::Normal => {
                if let Some(command) = self.grammar.normal(key) {
                    self.run(command);
                }
            }
            Mode::Visual | Mode::VisualLine => {
                if let Some(command) = self.grammar.visual(key) {
                    self.run(command);
                }
            }
            Mode::Insert => self.insert_key(key),
            Mode::Search => self.search_key(key),
            Mode::GoTo => self.go_to_key(key),
            Mode::Help | Mode::Peek => self.mode = self.help_from,
            Mode::ConfirmDiscard => self.confirm_key(key),
            Mode::FileDialog => self.dialog_key(key),
            Mode::StylePicker => self.picker_key(key),
            Mode::Outline => self.outline_key(key),
        }
        self.settle();
    }

    /// Text pasted into the terminal goes in where typing would.
    pub fn handle_paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        match self.mode {
            Mode::Insert => {
                let cursor = self.doc.insert_text(self.cursor(), &text);
                self.set_cursor(cursor);
            }
            Mode::Normal => self.edit(|app| {
                let cursor = app.doc.insert_text(app.cursor(), &text);
                app.set_cursor(cursor);
                Ok(())
            }),
            _ => {}
        }
        self.settle();
    }

    /// Moves the view by rows, as the mouse wheel does, pulling the cursor along to stay in it.
    pub fn scroll(&mut self, by: isize) {
        let last = self.layout.rows.len().saturating_sub(1) as isize;
        let top = (self.top() as isize + by).clamp(0, last) as usize;
        self.tops[self.story] = top;
        let row = self.layout.row_of(self.cursor());
        let bottom = top + self.height - 1;
        if row < top || row > bottom {
            let (from, step) = if row < top { (top, 1) } else { (bottom, -1) };
            if let Some(row) = self.layout.text_row(from as isize, step, 0)
                && let Some(position) = self.layout.position_at(row, 0, false)
                && (top..=bottom).contains(&row)
            {
                self.set_cursor(position);
            }
        }
    }

    /// Puts the cursor where the mouse clicked: a row of the view and a column of the text.
    pub fn click(&mut self, row: usize, column: u16) {
        if !matches!(self.mode, Mode::Normal | Mode::Insert) {
            return;
        }
        if let Some(position) = self
            .layout
            .position_at(self.top() + row, column, self.typing())
        {
            self.set_cursor(position);
            self.goal = None;
        }
        self.settle();
    }

    /// Shows another story.
    pub fn show_story(&mut self, story: usize) {
        if story < self.doc.stories.len() && matches!(self.mode, Mode::Normal) {
            self.story = story;
            self.goal = None;
            self.settle();
        }
    }

    /// Jumps to a heading of the outline, as clicking it does.
    pub fn go_to_heading(&mut self, index: usize) {
        if let Some(heading) = self.outline().get(index) {
            self.outline_at = index;
            self.set_cursor(Position::new(heading.path, 0));
            self.mode = Mode::Normal;
            self.settle();
            self.tops[self.story] = self.layout.row_of(self.cursor());
        }
    }

    // ---- commands ----

    fn run(&mut self, command: Command) {
        let cursor = self.cursor();
        let visual = matches!(self.mode, Mode::Visual | Mode::VisualLine);
        match command {
            Command::Move(motion, count) => self.go(motion, count),
            Command::Operate(operator, motion, count) => {
                let Some(mut target) = self.land(motion, count) else {
                    return;
                };
                // A word motion never takes an operator past the end of its paragraph.
                if matches!(motion, Motion::WordForward(_)) && target.path != cursor.path {
                    target = Position::new(cursor.path, self.len(cursor.path));
                }
                if motion.is_inclusive() {
                    target.offset = (target.offset + 1).min(self.len(target.path));
                }
                self.operate(operator, cursor, target, motion.is_linewise());
            }
            Command::OperateLines(operator, count) => {
                let last = self
                    .view()
                    .land(Motion::NextParagraph, cursor, Some(count.max(1) - 1))
                    .filter(|_| count > 1)
                    .filter(|last| Document::together(last.path, cursor.path))
                    .unwrap_or(cursor);
                self.operate(operator, cursor, last, true);
            }
            Command::OperateObject(operator, object) => match object {
                Object::Paragraph => self.operate(operator, cursor, cursor, true),
                Object::InnerWord | Object::AWord => {
                    let around = object == Object::AWord;
                    if let Some((start, end)) = self.view().word_at(cursor, around) {
                        let from = Position::new(cursor.path, start);
                        self.operate(operator, from, Position::new(cursor.path, end), false);
                    }
                }
            },
            Command::OperateSelection(operator) => {
                let (first, last) = self.selection_ends();
                let linewise = self.mode == Mode::VisualLine;
                let end = Position::new(last.path, (last.offset + 1).min(self.len(last.path)));
                self.mode = Mode::Normal;
                self.operate(operator, first, end, linewise);
            }
            Command::Insert(at) => self.insert(at),
            Command::DeleteChars(count) => {
                let end = (cursor.offset + count).min(self.len(cursor.path));
                self.operate(
                    Operator::Delete,
                    cursor,
                    Position::new(cursor.path, end),
                    false,
                );
            }
            Command::DeleteBefore(count) => {
                let start = Position::new(cursor.path, cursor.offset.saturating_sub(count));
                self.operate(Operator::Delete, start, cursor, false);
            }
            Command::DeleteToEnd | Command::ChangeToEnd => {
                let operator = match command {
                    Command::DeleteToEnd => Operator::Delete,
                    _ => Operator::Change,
                };
                let end = Position::new(cursor.path, self.len(cursor.path));
                self.operate(operator, cursor, end, false);
            }
            Command::Substitute(count) => {
                let end = (cursor.offset + count).min(self.len(cursor.path));
                self.operate(
                    Operator::Change,
                    cursor,
                    Position::new(cursor.path, end),
                    false,
                );
            }
            Command::Replace(replacement) => self.edit(|app| {
                if app.len(cursor.path) == 0 {
                    return Err(Refusal::Nothing);
                }
                // Typed after the old character, the new one takes its format.
                let after = Position::new(cursor.path, cursor.offset + 1);
                app.doc.insert_text(after, &replacement.to_string());
                app.doc.delete(cursor, after)?;
                Ok(())
            }),
            Command::Join(count) => {
                let paths = match visual {
                    true => {
                        let (first, last) = self.selection_ends();
                        self.mode = Mode::Normal;
                        self.set_cursor(first);
                        index_of(last.path)
                            .saturating_sub(index_of(first.path))
                            .max(1)
                    }
                    false => count.max(2) - 1,
                };
                self.edit(|app| {
                    let path = app.cursor().path;
                    for _ in 0..paths {
                        let next = with_index(path, index_of(path) + 1);
                        let (Some(first), Some(second)) =
                            (app.doc.paragraph(path), app.doc.paragraph(next))
                        else {
                            return Err(Refusal::Nothing);
                        };
                        let spaced = first.text().ends_with(char::is_whitespace)
                            || second.text().starts_with(char::is_whitespace);
                        if !first.is_empty() && !second.is_empty() && !spaced {
                            let end = Position::new(path, first.len());
                            app.doc.insert_text(end, " ");
                        }
                        if let Some(met) = app.doc.join(path) {
                            app.set_cursor(Position::new(met.path, met.offset.saturating_sub(1)));
                        }
                    }
                    Ok(())
                });
            }
            Command::Paste { before, count } => self.paste(before, count),
            Command::Undo | Command::Redo => {
                if self.story != 0 {
                    return self.report(Refusal::ReadOnlyStory.to_string());
                }
                let position = match command {
                    Command::Undo => self.doc.undo(),
                    _ => self.doc.redo(),
                };
                match position {
                    Some(position) => self.set_cursor(position),
                    None if command == Command::Undo => self.say("Nothing to undo"),
                    None => self.say("Nothing to redo"),
                }
            }
            Command::Visual | Command::VisualLine => {
                let wanted = match command {
                    Command::Visual => Mode::Visual,
                    _ => Mode::VisualLine,
                };
                if self.mode == wanted {
                    self.mode = Mode::Normal;
                } else {
                    if !visual {
                        self.anchor = cursor;
                    }
                    self.mode = wanted;
                }
            }
            Command::SwapEnds => {
                self.set_cursor(self.anchor);
                self.anchor = cursor;
            }
            Command::Heading(level) => {
                let paths = self.target_paragraphs();
                self.edit(|app| {
                    app.doc.set_heading(&paths, level);
                    Ok(())
                });
            }
            Command::CycleList => {
                let paths = self.target_paragraphs();
                self.edit(|app| {
                    app.doc.cycle_list(&paths);
                    Ok(())
                });
            }
            Command::StylePicker => self.open_picker(),
            Command::NextStory | Command::PreviousStory => {
                let count = self.doc.stories.len();
                let step = if command == Command::NextStory {
                    1
                } else {
                    count - 1
                };
                self.mode = Mode::Normal;
                self.show_story((self.story + step) % count);
            }
            Command::Outline => {
                self.outline_open = true;
                let headings = self.outline();
                self.outline_at = headings
                    .iter()
                    .rposition(|heading| heading.path <= cursor.path)
                    .unwrap_or(0);
                self.mode = Mode::Outline;
            }
            Command::Peek => self.look_up(),
            Command::Search => {
                self.line = LineEdit::default();
                self.help_from = Mode::Normal;
                self.mode = Mode::Search;
            }
            Command::SearchNext => self.find(true),
            Command::SearchPrevious => self.find(false),
            Command::SearchWord => {
                if let Some((start, end)) = self.view().word_at(cursor, false)
                    && let Some(paragraph) = self.story().paragraph(cursor.path)
                {
                    self.search_query = paragraph.graphemes()[start..end].concat();
                    self.find(true);
                }
            }
            Command::RepeatFind { reversed } => {
                if let Some((target, forward, till)) = self.last_find {
                    let motion = Motion::Find {
                        target,
                        forward: forward != reversed,
                        till,
                    };
                    if let Some(position) = self.view().land(motion, cursor, None) {
                        self.set_cursor(position);
                    }
                }
            }
            Command::GoTo => {
                self.line = LineEdit::default();
                self.mode = Mode::GoTo;
            }
            Command::Save => self.save(),
            Command::SaveAs => self.open_dialog(DialogKind::Save),
            Command::Open => self.ask_before_losing_edits(NextStep::OpenFile),
            Command::Reload => self.ask_before_losing_edits(NextStep::Reload),
            Command::Quit => self.ask_before_losing_edits(NextStep::Quit),
            Command::SaveAndQuit => {
                self.next_step = Some(NextStep::Quit);
                match self.doc.is_dirty() || self.doc.path().is_none() {
                    true => self.save(),
                    false => self.take_step(),
                }
            }
            Command::QuitNow => self.should_quit = true,
            Command::Help => {
                self.help_from = self.mode;
                self.mode = Mode::Help;
            }
            Command::Scroll(place) => {
                let row = self.layout.row_of(cursor);
                self.tops[self.story] = match place {
                    Scroll::Top => row,
                    Scroll::Middle => row.saturating_sub(self.height / 2),
                    Scroll::Bottom => (row + 1).saturating_sub(self.height),
                };
            }
            Command::Escape => {
                if visual {
                    self.mode = Mode::Normal;
                }
            }
        }
    }

    /// The paragraphs a paragraph-wide command acts on: the selection's, or the cursor's.
    fn target_paragraphs(&mut self) -> Vec<BlockPath> {
        let cursor = self.cursor();
        if !matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            return vec![cursor.path];
        }
        let (first, last) = self.selection_ends();
        self.mode = Mode::Normal;
        self.set_cursor(first);
        self.paragraphs_between(first.path, last.path)
    }

    /// The paragraphs from one to another in reading order.
    fn paragraphs_between(&self, first: BlockPath, last: BlockPath) -> Vec<BlockPath> {
        let (first, last) = (first.min(last), first.max(last));
        self.layout
            .paths
            .iter()
            .copied()
            .filter(|path| first <= *path && *path <= last)
            .collect()
    }

    /// The first and the last place of the selection, in reading order.
    fn selection_ends(&self) -> (Position, Position) {
        let (cursor, anchor) = (self.cursor(), self.anchor);
        (cursor.min(anchor), cursor.max(anchor))
    }

    fn land(&mut self, motion: Motion, count: Option<usize>) -> Option<Position> {
        if let Motion::Find {
            target,
            forward,
            till,
        } = motion
        {
            self.last_find = Some((target, forward, till));
        }
        self.view().land(motion, self.cursor(), count)
    }

    fn go(&mut self, motion: Motion, count: Option<usize>) {
        if motion.is_vertical() && self.goal.is_none() {
            self.goal = self
                .layout
                .find(self.cursor())
                .map(|(_, line)| line.column_of(self.cursor().offset));
        }
        if !motion.is_vertical() {
            self.goal = None;
        }
        if let Some(position) = self.land(motion, count) {
            self.set_cursor(position);
        }
    }

    // ---- editing ----

    /// Makes a change that undoes as one. Stories other than the body can't be changed yet.
    fn edit(&mut self, change: impl FnOnce(&mut Self) -> Result<(), Refusal>) {
        if self.story != 0 {
            return self.report(Refusal::ReadOnlyStory.to_string());
        }
        self.doc.begin(self.cursor());
        if let Err(refusal) = change(self) {
            self.report(refusal.to_string());
        }
        self.settle();
        // A change that goes on into typing is finished when the typing is.
        if !self.typing() {
            self.doc.commit(self.cursor());
        }
    }

    fn yank(&mut self, register: Register) {
        self.clipboard = Some(register.text());
        self.register = Some(register);
    }

    /// The whole blocks from one paragraph to another, which must sit side by side.
    fn blocks_between(&self, first: BlockPath, last: BlockPath) -> Result<Vec<Block>, Refusal> {
        if !Document::together(first, last) {
            return Err(Refusal::AcrossCells);
        }
        let container = self.story().container(first).ok_or(Refusal::Nothing)?;
        let range = index_of(first).min(index_of(last))..=index_of(first).max(index_of(last));
        container
            .get(range)
            .map(<[Block]>::to_vec)
            .ok_or(Refusal::Nothing)
    }

    fn operate(&mut self, operator: Operator, from: Position, to: Position, linewise: bool) {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        let linewise = linewise || operator.is_paragraph_wide();
        if operator == Operator::Yank {
            let taken = match linewise {
                true => self.blocks_between(from.path, to.path).map(Register::Lines),
                false => self.doc_copy(from, to).map(Register::Chars),
            };
            match taken {
                Ok(register) => {
                    self.yank(register);
                    self.set_cursor(if linewise {
                        self.cursor().min(from)
                    } else {
                        from
                    });
                }
                Err(refusal) => self.report(refusal.to_string()),
            }
            return;
        }
        // The whole of both paragraphs, for the operators that take text rather than blocks.
        let whole = (
            Position::new(from.path, 0),
            Position::new(to.path, self.len(to.path)),
        );
        self.edit(|app| {
            match operator {
                Operator::Delete if linewise => {
                    if !Document::together(from.path, to.path) {
                        return Err(Refusal::AcrossCells);
                    }
                    let reading = app.layout.index_of(from.path).unwrap_or(0);
                    let blocks = app.doc.delete_blocks(from.path, to.path);
                    app.yank(Register::Lines(blocks));
                    app.relayout();
                    let last = app.layout.paths.len().saturating_sub(1);
                    if let Some(path) = app.layout.paths.get(reading.min(last)) {
                        app.set_cursor(Position::new(*path, 0));
                    }
                }
                Operator::Delete => {
                    let taken = app.doc.delete(from, to)?;
                    app.yank(Register::Chars(taken));
                    app.set_cursor(from);
                }
                Operator::Change => {
                    let (start, end) = if linewise { whole } else { (from, to) };
                    let taken = app.doc.delete(start, end)?;
                    app.yank(Register::Chars(taken));
                    app.set_cursor(start);
                    app.mode = Mode::Insert;
                }
                Operator::Bold | Operator::Italic | Operator::Underline | Operator::Strike => {
                    let toggle = match operator {
                        Operator::Bold => Toggle::Bold,
                        Operator::Italic => Toggle::Italic,
                        Operator::Underline => Toggle::Underline,
                        _ => Toggle::Strike,
                    };
                    let (start, end) = if linewise { whole } else { (from, to) };
                    app.doc.toggle(start, end, toggle)?;
                }
                Operator::Indent | Operator::Outdent => {
                    let paths = app.paragraphs_between(from.path, to.path);
                    app.doc.shift(&paths, operator == Operator::Indent);
                }
                Operator::Yank => {}
            }
            Ok(())
        });
    }

    fn doc_copy(
        &self,
        from: Position,
        to: Position,
    ) -> Result<Vec<crate::model::Fragment>, Refusal> {
        if self.story == 0 {
            return self.doc.copy(from, to);
        }
        // Other stories can be copied from, though not changed.
        if from.path != to.path {
            return Err(Refusal::AcrossCells);
        }
        let paragraph = self.story().paragraph(from.path).ok_or(Refusal::Nothing)?;
        Ok(vec![paragraph.fragment(from.offset..to.offset)])
    }

    fn paste(&mut self, before: bool, count: usize) {
        let Some(register) = self.register.clone() else {
            return self.say("Nothing to paste");
        };
        let cursor = self.cursor();
        self.edit(|app| {
            match &register {
                Register::Lines(blocks) => {
                    let at = with_index(cursor.path, index_of(cursor.path) + usize::from(!before));
                    for _ in 0..count.max(1) {
                        app.doc.insert_blocks(at, blocks.clone());
                    }
                    app.set_cursor(Position::new(at, 0));
                }
                Register::Chars(fragments) => {
                    let offset = match before {
                        true => cursor.offset,
                        false => (cursor.offset + 1).min(app.len(cursor.path)),
                    };
                    let mut end = Position::new(cursor.path, offset);
                    for _ in 0..count.max(1) {
                        end = app.doc.insert_fragments(end, fragments);
                    }
                    app.set_cursor(Position::new(end.path, end.offset.saturating_sub(1)));
                }
            }
            Ok(())
        });
    }

    fn insert(&mut self, at: InsertAt) {
        if self.story != 0 {
            return self.report(Refusal::ReadOnlyStory.to_string());
        }
        let cursor = self.cursor();
        let len = self.len(cursor.path);
        self.doc.begin(cursor);
        self.mode = Mode::Insert;
        let position = match at {
            InsertAt::Here => cursor,
            InsertAt::After => Position::new(cursor.path, (cursor.offset + 1).min(len)),
            InsertAt::FirstNonBlank => self
                .view()
                .land(Motion::FirstNonBlank, cursor, None)
                .unwrap_or(cursor),
            InsertAt::End => Position::new(cursor.path, len),
            InsertAt::Below => self.doc.open_paragraph(cursor.path, true),
            InsertAt::Above => self.doc.open_paragraph(cursor.path, false),
        };
        self.set_cursor(position);
    }

    fn leave_insert(&mut self) {
        self.mode = Mode::Normal;
        let mut cursor = self.cursor();
        cursor.offset = cursor.offset.saturating_sub(1);
        self.set_cursor(cursor);
        self.settle();
        self.doc.commit(self.cursor());
    }

    fn insert_key(&mut self, key: KeyEvent) {
        let cursor = self.cursor();
        let len = self.len(cursor.path);
        let start = Position::new(cursor.path, 0);
        match key.code {
            KeyCode::Esc => self.leave_insert(),
            KeyCode::Char('c') if is_ctrl(key) => self.leave_insert(),
            KeyCode::F(1) => {
                self.help_from = Mode::Insert;
                self.mode = Mode::Help;
            }
            KeyCode::Char('s') if is_ctrl(key) => {
                self.doc.commit(cursor);
                self.save();
                // Saving ends in normal mode; typing goes on unless a dialog came up.
                if self.mode == Mode::Normal {
                    self.mode = Mode::Insert;
                    self.doc.begin(cursor);
                }
            }
            KeyCode::Char('w') if is_ctrl(key) => {
                let from = self
                    .view()
                    .land(Motion::WordBack(false), cursor, None)
                    .filter(|from| from.path == cursor.path)
                    .unwrap_or(start);
                let _ = self.doc.delete(from, cursor);
                self.set_cursor(from);
            }
            KeyCode::Char('u') if is_ctrl(key) => {
                let _ = self.doc.delete(start, cursor);
                self.set_cursor(start);
            }
            KeyCode::Char(c) if is_text_input(key) => {
                let position = self.doc.insert_text(cursor, &c.to_string());
                self.set_cursor(position);
            }
            KeyCode::Enter => {
                let paragraph = self.doc.paragraph(cursor.path);
                let listed =
                    paragraph.is_some_and(|p| self.doc.styles.list_item(&p.props).is_some());
                let heading =
                    paragraph.is_some_and(|p| self.doc.styles.heading_level(&p.props).is_some());
                if len == 0 && listed {
                    // Enter on an empty item is how a list is ended.
                    self.doc.leave_list(cursor.path);
                } else {
                    let position = self.doc.split(cursor);
                    // The paragraph after a heading is ordinary text, as in Word.
                    if heading && cursor.offset == len {
                        self.doc.set_style(&[position.path], None);
                    }
                    self.set_cursor(position);
                }
            }
            KeyCode::Backspace if cursor.offset > 0 => {
                let from = Position::new(cursor.path, cursor.offset - 1);
                let _ = self.doc.delete(from, cursor);
                self.set_cursor(from);
            }
            KeyCode::Backspace => {
                let listed = self
                    .doc
                    .paragraph(cursor.path)
                    .is_some_and(|p| self.doc.styles.list_item(&p.props).is_some());
                if listed {
                    self.doc.leave_list(cursor.path);
                } else if let Some(index) = index_of(cursor.path).checked_sub(1)
                    && let Some(met) = self.doc.join(with_index(cursor.path, index))
                {
                    self.set_cursor(met);
                }
            }
            KeyCode::Delete if cursor.offset < len => {
                let _ = self
                    .doc
                    .delete(cursor, Position::new(cursor.path, cursor.offset + 1));
            }
            KeyCode::Delete => {
                self.doc.join(cursor.path);
            }
            KeyCode::Tab | KeyCode::BackTab if matches!(cursor.path, BlockPath::Cell { .. }) => {
                let motion = match key.code {
                    KeyCode::Tab => Motion::NextCell,
                    _ => Motion::PreviousCell,
                };
                if let Some(position) = self.view().land(motion, cursor, None) {
                    self.set_cursor(position);
                }
            }
            KeyCode::Tab => {
                if let Some(paragraph) = self.doc.paragraph_mut(cursor.path) {
                    paragraph.insert_atom(cursor.offset, AtomKind::Tab, "<w:tab/>");
                    self.set_cursor(Position::new(cursor.path, cursor.offset + 1));
                }
            }
            KeyCode::Left => {
                self.set_cursor(Position::new(cursor.path, cursor.offset.saturating_sub(1)))
            }
            KeyCode::Right => {
                self.set_cursor(Position::new(cursor.path, (cursor.offset + 1).min(len)))
            }
            KeyCode::Up | KeyCode::Down => {
                self.relayout();
                let motion = if key.code == KeyCode::Up {
                    Motion::Up
                } else {
                    Motion::Down
                };
                self.go(motion, None);
            }
            KeyCode::Home | KeyCode::End => {
                self.relayout();
                if let Some((_, line)) = self.layout.find(cursor) {
                    let offset = match (key.code, line.last) {
                        (KeyCode::Home, _) => line.start,
                        (_, true) => line.end(),
                        (_, false) => line.end().saturating_sub(1),
                    };
                    self.set_cursor(Position::new(cursor.path, offset));
                }
            }
            _ => {}
        }
        if !matches!(key.code, KeyCode::Up | KeyCode::Down) {
            self.goal = None;
        }
    }

    // ---- prompts and dialogs ----

    fn search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                self.search_query = self.line.text();
                self.find(true);
            }
            _ => self.line.handle_key(key),
        }
    }

    fn find(&mut self, forward: bool) {
        if self.search_query.is_empty() {
            return self.say("Nothing to search for");
        }
        match self
            .view()
            .search(&self.search_query, self.cursor(), forward)
        {
            Some(position) => {
                self.set_cursor(position);
                self.goal = None;
            }
            None => self.report(format!("Not found: {}", self.search_query)),
        }
    }

    fn go_to_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.status.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => match self.line.text().trim().parse::<usize>() {
                Ok(number) if number >= 1 => {
                    self.status.clear();
                    self.mode = Mode::Normal;
                    self.go(Motion::First, Some(number));
                }
                _ => self.report("Type a paragraph number"),
            },
            _ => {
                self.status.clear();
                self.line.handle_key(key);
            }
        }
    }

    fn look_up(&mut self) {
        let cursor = self.cursor();
        let Some(paragraph) = self.story().paragraph(cursor.path) else {
            return;
        };
        let cells = paragraph.cells();
        let Some(cell) = cells.get(cursor.offset) else {
            return self.say("Nothing to look up here");
        };
        let note = |kind: StoryKind, id: &str| -> Vec<String> {
            self.doc
                .stories
                .iter()
                .filter(|story| story.kind == kind)
                .flat_map(|story| {
                    story
                        .notes
                        .iter()
                        .filter(|note| note.id == id)
                        .flat_map(|note| &story.blocks[note.blocks.clone()])
                        .filter_map(|block| match block {
                            Block::Paragraph(paragraph) => Some(paragraph.text()),
                            _ => None,
                        })
                })
                .collect()
        };
        let lines = match cell.atom {
            Some(AtomKind::Footnote(id)) => note(StoryKind::Footnotes, id),
            Some(AtomKind::Endnote(id)) => note(StoryKind::Endnotes, id),
            Some(AtomKind::Comment(id)) => note(StoryKind::Comments, id),
            Some(AtomKind::Image(alt)) if !alt.is_empty() => vec![alt.clone()],
            _ => cell
                .wraps
                .iter()
                .filter_map(|wrapper| wrapper.link.as_deref())
                .map(str::to_string)
                .collect(),
        };
        if lines.is_empty() {
            return self.say("Nothing to look up here");
        }
        self.peek = lines;
        self.help_from = Mode::Normal;
        self.mode = Mode::Peek;
    }

    fn open_picker(&mut self) {
        if self.story != 0 {
            return self.report(Refusal::ReadOnlyStory.to_string());
        }
        let styles = self.doc.styles.paragraph_styles();
        self.picker_styles = styles.iter().map(|style| Some(style.id.clone())).collect();
        let mut names: Vec<String> = styles.iter().map(|style| style.name.clone()).collect();
        if names.is_empty() {
            names.push("Normal".into());
            self.picker_styles.push(None);
        }
        self.picker = Some(Picker::new(names));
        self.mode = Mode::StylePicker;
    }

    fn picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = &mut self.picker else {
            self.mode = Mode::Normal;
            return;
        };
        match picker.handle_key(key) {
            Picked::Stay => {}
            Picked::Cancel => {
                self.picker = None;
                self.mode = Mode::Normal;
            }
            Picked::Chosen(index) => {
                let style = self.picker_styles.get(index).cloned().flatten();
                // The default style is had by naming none.
                let style = style.filter(|id| self.doc.styles.normal() != Some(id.as_str()));
                self.picker = None;
                self.mode = Mode::Normal;
                let path = self.cursor().path;
                self.edit(|app| {
                    app.doc.set_style(&[path], style.as_deref());
                    Ok(())
                });
            }
        }
    }

    fn outline_key(&mut self, key: KeyEvent) {
        let count = self.outline().len();
        if key.code == KeyCode::Enter {
            return self.go_to_heading(self.outline_at);
        }
        if key.code == KeyCode::Tab {
            self.mode = Mode::Normal;
            return;
        }
        let Some(command) = self.grammar.normal(key) else {
            return;
        };
        let last = count.saturating_sub(1);
        match command {
            Command::Move(Motion::Down, n) => {
                self.outline_at = (self.outline_at + n.unwrap_or(1)).min(last);
            }
            Command::Move(Motion::Up, n) => {
                self.outline_at = self.outline_at.saturating_sub(n.unwrap_or(1));
            }
            Command::Move(Motion::First, _) => self.outline_at = 0,
            Command::Move(Motion::Last, _) => self.outline_at = last,
            Command::Escape => self.mode = Mode::Normal,
            Command::Outline | Command::Quit => {
                self.outline_open = false;
                self.mode = Mode::Normal;
            }
            Command::Help => {
                self.help_from = Mode::Outline;
                self.mode = Mode::Help;
            }
            _ => {}
        }
    }

    // ---- files ----

    fn ask_before_losing_edits(&mut self, step: NextStep) {
        self.next_step = Some(step);
        match self.doc.is_dirty() {
            true => self.mode = Mode::ConfirmDiscard,
            false => self.take_step(),
        }
    }

    /// Does what was waiting on the document being saved or given up.
    fn take_step(&mut self) {
        self.mode = Mode::Normal;
        match self.next_step.take() {
            Some(NextStep::Quit) => self.should_quit = true,
            Some(NextStep::OpenFile) => self.open_dialog(DialogKind::Open),
            Some(NextStep::Reload) => match self.doc.path().map(Path::to_path_buf) {
                Some(path) => self.load(&path),
                None => self.report("There is no file to reload"),
            },
            None => {}
        }
    }

    fn confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') => {
                self.mode = Mode::Normal;
                self.save();
            }
            KeyCode::Char('d') => self.take_step(),
            _ => {
                self.next_step = None;
                self.mode = Mode::Normal;
            }
        }
    }

    fn open_dialog(&mut self, kind: DialogKind) {
        self.file_dialog = Some(FileDialog::new(kind, &self.dialog_directory));
        self.mode = Mode::FileDialog;
    }

    fn dialog_key(&mut self, key: KeyEvent) {
        let Some(dialog) = &mut self.file_dialog else {
            self.mode = Mode::Normal;
            return;
        };
        let action = dialog.handle_key(key);
        self.dialog_directory = dialog.directory().to_path_buf();
        match action {
            DialogAction::Stay => {}
            DialogAction::Cancel => {
                self.file_dialog = None;
                self.next_step = None;
                self.mode = Mode::Normal;
            }
            DialogAction::Save(path) | DialogAction::Overwrite(path) => self.save_to(Some(&path)),
            DialogAction::Open(path) => self.load(&path),
        }
    }

    /// Saves to the document's file, asking for one first if it has none.
    fn save(&mut self) {
        match self.doc.path() {
            Some(_) => self.save_to(None),
            None => self.open_dialog(DialogKind::Save),
        }
    }

    fn save_to(&mut self, target: Option<&Path>) {
        match saving::save(&mut self.doc, target) {
            Ok(()) => {
                self.file_dialog = None;
                self.mode = Mode::Normal;
                self.say(format!("Saved {}", self.doc.file_name()));
                self.take_step();
            }
            Err(error) => {
                // A save that failed mustn't go on to quit or open something else.
                self.next_step = None;
                match &mut self.file_dialog {
                    Some(dialog) => dialog.set_message(error.to_string()),
                    None => self.report(format!("Not saved: {error}")),
                }
            }
        }
    }

    fn load(&mut self, path: &Path) {
        match Document::open(path) {
            Ok(doc) => {
                let (width, height) = (self.width, self.height);
                let outline_open = self.outline_open;
                *self = Self::new(doc);
                self.outline_open = outline_open;
                self.fit(width, height as u16);
            }
            Err(error) => {
                let message = format!("Cannot open {}: {error}", path.display());
                match &mut self.file_dialog {
                    Some(dialog) => dialog.set_message(message),
                    None => self.report(message),
                }
            }
        }
    }
}
