//! Drawing the screen, and working out what a mouse click landed on.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout as Split, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, Mode, NextStep};
use crate::file_dialog::{DialogKind, FileDialog};
use crate::help::{HELP_KEYS, HELP_WIDTH};
use crate::layout::{HEADER_STYLE, LINE_STYLE, shown};
use crate::line_edit::Window;
use crate::numbers;

/// The cursor and highlighted dialog lines. Reversed video follows the terminal's theme.
const SELECTED_STYLE: Style = Style::new().add_modifier(Modifier::REVERSED);
const OUTLINE_WIDTH: u16 = 28;
/// How many rows the mouse wheel moves.
const WHEEL_STEP: isize = 3;

/// Where everything goes. Drawing and the mouse both go by this, so they can't disagree.
struct Areas {
    outline: Option<Rect>,
    document: Rect,
    tabs: Rect,
    status: Rect,
}

fn areas(screen: Rect, app: &App) -> Areas {
    let tab_rows = u16::from(app.document().stories.len() > 1);
    let [main, tabs, status] = Split::vertical([
        Constraint::Min(0),
        Constraint::Length(tab_rows),
        Constraint::Length(1),
    ])
    .areas(screen);
    // A narrow terminal keeps its width for the text.
    if app.outline_is_open() && main.width >= OUTLINE_WIDTH * 2 {
        let [outline, document] =
            Split::horizontal([Constraint::Length(OUTLINE_WIDTH), Constraint::Min(0)]).areas(main);
        return Areas {
            outline: Some(outline),
            document,
            tabs,
            status,
        };
    }
    Areas {
        outline: None,
        document: main,
        tabs,
        status,
    }
}

/// The part of the document pane the text is drawn in, right of the line numbers.
fn text_area(document: Rect) -> Rect {
    let inner = Block::bordered().inner(document);
    let gutter = numbers::WIDTH.min(inner.width);
    Rect::new(
        inner.x + gutter,
        inner.y,
        inner.width - gutter,
        inner.height,
    )
}

/// Tells the app how much room the text has, before it is drawn.
pub fn fit(app: &mut App, screen: Rect) {
    let text = text_area(areas(screen, app).document);
    app.fit(text.width, text.height);
}

pub fn handle_mouse(app: &mut App, screen: Rect, event: MouseEvent) {
    let areas = areas(screen, app);
    let text = text_area(areas.document);
    let at = (event.column, event.row);
    let inside = |area: Rect| area.contains(at.into());
    match event.kind {
        MouseEventKind::ScrollDown => app.scroll(WHEEL_STEP),
        MouseEventKind::ScrollUp => app.scroll(-WHEEL_STEP),
        MouseEventKind::Down(MouseButton::Left) => {
            if inside(text) {
                app.click(usize::from(at.1 - text.y), at.0 - text.x);
            } else if inside(areas.tabs) {
                if let Some(tab) = visible_tabs(areas.tabs, app)
                    .iter()
                    .find(|tab| (tab.x..tab.x + tab.width).contains(&at.0))
                {
                    app.show_story(tab.index);
                }
            } else if let Some(outline) = areas.outline {
                let inner = Block::bordered().inner(outline);
                if inside(inner) && app.mode() == Mode::Normal {
                    let first = outline_first(app, inner.height);
                    app.go_to_heading(first + usize::from(at.1 - inner.y));
                }
            }
        }
        _ => {}
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let areas = areas(frame.area(), app);
    let mode = app.mode();
    let marker = if app.document().is_dirty() { "*" } else { "" };
    let title = format!(
        "{}{marker} — {}",
        app.document().file_name(),
        app.story().title
    );
    let block = section_block(mode != Mode::Outline, &title);
    frame.render_widget(block, areas.document);
    let text = text_area(areas.document);
    let cursor = draw_text(frame.buffer_mut(), text, app);
    if let Some(outline) = areas.outline {
        draw_outline(frame, outline, app);
    }
    draw_tabs(frame.buffer_mut(), areas.tabs, app);
    draw_status(frame.buffer_mut(), areas.status, app);
    if let Some((x, y)) = cursor
        && matches!(
            mode,
            Mode::Normal | Mode::Insert | Mode::Visual | Mode::VisualLine
        )
    {
        frame.set_cursor_position((x, y));
    }
    if let Some(dialog) = app.file_dialog() {
        draw_file_dialog(frame, dialog);
    }
    match mode {
        Mode::ConfirmDiscard => draw_discard_dialog(frame, app),
        Mode::GoTo => draw_go_to_dialog(frame, app),
        Mode::Help => draw_help(frame, app),
        Mode::Peek => draw_peek(frame, app),
        Mode::StylePicker => draw_picker(frame, app),
        Mode::Search if areas.status.width > 0 && areas.status.height > 0 => {
            let status = areas.status;
            let x = status.x + SEARCH_PROMPT_WIDTH + search_window(status, app).caret_x;
            frame.set_cursor_position((x.min(status.right().saturating_sub(1)), status.y));
        }
        _ => {}
    }
}

/// Draws the rows of the story that are in view, and says where the cursor ended up.
fn draw_text(buffer: &mut Buffer, area: Rect, app: &App) -> Option<(u16, u16)> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let layout = app.layout();
    let cursor = app.cursor();
    let cursor_row = layout.row_of(cursor);
    // Rows without text have no number, so that 5j goes to the row numbered 5.
    let text_rows_between = |a: usize, b: usize| {
        (a.min(b)..a.max(b))
            .filter(|row| layout.has_text(*row))
            .count()
    };
    let (paragraph, _) = app.place();
    let mut cursor_at = None;
    for (offset, row) in layout
        .rows
        .iter()
        .enumerate()
        .skip(app.top())
        .take(usize::from(area.height))
        .map(|(index, row)| (index - app.top(), (index, row)))
    {
        let (index, row) = row;
        let y = area.y + offset as u16;
        if !row.lines.is_empty() {
            let (label, style) = match index == cursor_row {
                true => (
                    numbers::current(paragraph),
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                false => (
                    numbers::relative(text_rows_between(index, cursor_row)),
                    LINE_STYLE,
                ),
            };
            buffer.set_string(area.x - numbers::WIDTH.min(area.x), y, label, style);
        }
        let room = |x: u16| usize::from(area.width.saturating_sub(x));
        for (x, text, style) in &row.decor {
            buffer.set_stringn(area.x + x, y, text, room(*x), *style);
        }
        for line in &row.lines {
            if let Some((x, label)) = &line.label {
                buffer.set_stringn(area.x + x, y, label, room(*x), Style::new());
            }
            let mut x = line.x;
            for (index, glyph) in line.glyphs.iter().enumerate() {
                let mut style = glyph.style;
                if app.is_selected(line.path, line.start + index) {
                    style = style.patch(SELECTED_STYLE);
                }
                if x < area.width {
                    // A selected line break has no width, yet should show as selected.
                    let text = match glyph.width {
                        0 if style.add_modifier.contains(Modifier::REVERSED) => " ",
                        _ => glyph.text.as_str(),
                    };
                    buffer.set_stringn(area.x + x, y, text, room(x), style);
                }
                x += glyph.width;
            }
            if line.glyphs.is_empty()
                && app.is_selected(line.path, line.start)
                && line.x < area.width
            {
                buffer.set_stringn(area.x + line.x, y, " ", 1, SELECTED_STYLE);
            }
            let holds_cursor = line.path == cursor.path
                && ((line.start..line.end()).contains(&cursor.offset)
                    || (line.last && cursor.offset >= line.end()));
            if holds_cursor {
                let column = line
                    .column_of(cursor.offset)
                    .min(area.width.saturating_sub(1));
                cursor_at = Some((area.x + column, y));
            }
        }
    }
    cursor_at
}

/// The first heading shown, late enough to keep the highlighted one in view.
fn outline_first(app: &App, rows: u16) -> usize {
    (app.outline_at() + 1).saturating_sub(usize::from(rows))
}

fn draw_outline(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.mode() == Mode::Outline;
    let block = section_block(focused, "Outline");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let headings = app.outline();
    let room = usize::from(inner.width);
    let buffer = frame.buffer_mut();
    if headings.is_empty() && inner.height > 0 {
        buffer.set_stringn(inner.x, inner.y, "No headings", room, LINE_STYLE);
    }
    let first = outline_first(app, inner.height);
    for (offset, (index, heading)) in headings
        .iter()
        .enumerate()
        .skip(first)
        .take(usize::from(inner.height))
        .enumerate()
    {
        let indent = "  ".repeat(usize::from(heading.level.saturating_sub(1)).min(4));
        let picked = index == app.outline_at();
        let marker = if picked && focused { ">" } else { " " };
        let line = format!("{marker} {indent}{}", shown(&heading.text));
        let style = match (picked && focused, heading.level) {
            (true, _) => SELECTED_STYLE,
            (false, 1) => HEADER_STYLE,
            _ => Style::new(),
        };
        let line = format!("{line:<room$}");
        buffer.set_stringn(inner.x, inner.y + offset as u16, line, room, style);
    }
}

/// Lists the keys that matter right now, over the middle of the screen.
fn draw_help(frame: &mut Frame, app: &App) {
    let lines: Vec<Line> = app
        .help_table()
        .iter()
        .map(|(keys, action)| {
            let padding = " ".repeat(HELP_KEYS.saturating_sub(keys.width()));
            Line::from(vec![
                Span::styled(format!("{keys}{padding}"), Style::new().fg(Color::Cyan)),
                Span::raw(*action),
            ])
        })
        .collect();
    let area = frame.area().centered(
        Constraint::Length(HELP_WIDTH),
        Constraint::Length(lines.len() as u16 + 2),
    );
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Keys ")),
        area,
    );
}

const PEEK_WIDTH: u16 = 60;

/// Shows a footnote, a comment or where a link leads, over the middle of the screen.
fn draw_peek(frame: &mut Frame, app: &App) {
    let room = usize::from(PEEK_WIDTH - 2);
    let mut lines: Vec<Line> = Vec::new();
    for text in app.peek() {
        let mut line = String::new();
        for word in shown(text).split(' ') {
            if !line.is_empty() && line.width() + 1 + word.width() > room {
                lines.push(Line::raw(std::mem::take(&mut line)));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(Line::raw(line));
    }
    lines.truncate(12);
    lines.push(Line::styled("Press any key to close", LINE_STYLE));
    let area = frame.area().centered(
        Constraint::Length(PEEK_WIDTH),
        Constraint::Length(lines.len() as u16 + 2),
    );
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::Cyan))
        .title(" Look up ");
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

const PICKER_WIDTH: u16 = 52;
const PICKER_ROWS: usize = 12;
const PICKER_LABEL: &str = "Style: ";

fn draw_picker(frame: &mut Frame, app: &App) {
    let Some(picker) = app.picker() else {
        return;
    };
    let visible = picker.visible();
    let rows = visible.len().clamp(1, PICKER_ROWS);
    let area = frame.area().centered(
        Constraint::Length(PICKER_WIDTH),
        Constraint::Length(rows as u16 + 4),
    );
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::Yellow))
        .title(" Paragraph style ");
    let inner = block.inner(area);
    let room = usize::from(inner.width);
    let window = picker
        .filter()
        .window(room.saturating_sub(PICKER_LABEL.len() + 1));
    let mut lines = vec![Line::raw(format!("{PICKER_LABEL}{}", window.text))];
    let first = (picker.highlight() + 1).saturating_sub(rows);
    for (index, (_, name)) in visible.iter().enumerate().skip(first).take(rows) {
        let picked = index == picker.highlight();
        let marker = if picked { ">" } else { " " };
        let style = if picked { SELECTED_STYLE } else { Style::new() };
        lines.push(Line::styled(
            format!("{marker} {:<room$}", shown(name)),
            style,
        ));
    }
    if visible.is_empty() {
        lines.push(Line::styled("No style matches", LINE_STYLE));
    }
    lines.push(Line::from(vec![
        Span::styled("Enter", Style::new().fg(Color::Yellow)),
        Span::raw(" apply   "),
        Span::styled("↑ ↓", Style::new().fg(Color::Cyan)),
        Span::raw(" choose   "),
        Span::styled("Esc", Style::new().fg(Color::Cyan)),
        Span::raw(" cancel"),
    ]));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    if inner.width > 0 && inner.height > 0 {
        let x = inner.x + PICKER_LABEL.len() as u16 + window.caret_x;
        frame.set_cursor_position((x.min(inner.right().saturating_sub(1)), inner.y));
    }
}

/// The width of the badge and the ` /` before the typed text.
const SEARCH_PROMPT_WIDTH: u16 = " NORMAL  /".len() as u16;

/// The part of the search that fits on the status line, with a cell free for the terminal cursor.
fn search_window(status: Rect, app: &App) -> Window {
    let room = status.width.saturating_sub(SEARCH_PROMPT_WIDTH + 1);
    app.line().window(usize::from(room))
}

const GO_TO_DIALOG_WIDTH: u16 = 52;
const GO_TO_LABEL: &str = "Paragraph: ";

/// Asks where to jump, with the terminal cursor at the caret.
fn draw_go_to_dialog(frame: &mut Frame, app: &App) {
    let last_line = if app.status_is_error() {
        Line::styled(app.status().to_string(), Style::new().fg(Color::Red))
    } else {
        Line::from(vec![
            Span::styled("Enter", Style::new().fg(Color::Yellow)),
            Span::raw(" go   "),
            Span::styled("Esc", Style::new().fg(Color::Cyan)),
            Span::raw(" cancel"),
        ])
    };
    let area = frame.area().centered(
        Constraint::Length(GO_TO_DIALOG_WIDTH),
        Constraint::Length(4),
    );
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::Yellow))
        .title(" Go to ");
    let inner = block.inner(area);
    // One cell stays free for the terminal cursor after the text.
    let room = inner.width.saturating_sub(GO_TO_LABEL.len() as u16 + 1);
    let window = app.line().window(usize::from(room));
    let lines = vec![
        Line::raw(format!("{GO_TO_LABEL}{}", window.text)),
        last_line,
    ];
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    if inner.width > 0 && inner.height > 0 {
        let x = inner.x + GO_TO_LABEL.len() as u16 + window.caret_x;
        frame.set_cursor_position((x.min(inner.right().saturating_sub(1)), inner.y));
    }
}

const DISCARD_DIALOG_WIDTH: u16 = 60;

/// Asks what to do with unsaved edits, naming what happens next.
fn draw_discard_dialog(frame: &mut Frame, app: &App) {
    let next = match app.next_step() {
        Some(NextStep::OpenFile) => "open",
        Some(NextStep::Reload) => "reload",
        _ => "quit",
    };
    let lines = vec![
        Line::raw(format!(
            "{} has unsaved changes.",
            app.document().file_name()
        )),
        Line::from(vec![
            Span::styled("s", Style::new().fg(Color::Yellow)),
            Span::raw(format!(" save and {next}   ")),
            Span::styled("d", Style::new().fg(Color::Red)),
            Span::raw(format!(" discard and {next}   ")),
            Span::styled("Esc", Style::new().fg(Color::Cyan)),
            Span::raw(" stay"),
        ]),
    ];
    let area = frame.area().centered(
        Constraint::Length(DISCARD_DIALOG_WIDTH),
        Constraint::Length(lines.len() as u16 + 2),
    );
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::Red))
        .title(" Unsaved changes ");
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// A pane's box: a thick border marks the one in use.
fn section_block(focused: bool, title: &str) -> Block<'static> {
    let border = if focused {
        BorderType::Thick
    } else {
        BorderType::Plain
    };
    Block::bordered()
        .border_type(border)
        .title(format!(" {title} "))
}

const FILE_DIALOG_WIDTH: u16 = 60;
const FILE_DIALOG_LIST_ROWS: usize = 12;
const FILE_DIALOG_NAME_LABEL: &str = "Name: ";

/// Draws the dialog over the middle of the screen.
fn draw_file_dialog(frame: &mut Frame, dialog: &FileDialog) {
    let area = frame.area();
    let list_rows = dialog.entries().len().clamp(1, FILE_DIALOG_LIST_ROWS) as u16;
    let popup = area.centered(
        Constraint::Length(FILE_DIALOG_WIDTH),
        Constraint::Length(list_rows + 5),
    );
    let title = match dialog.kind() {
        DialogKind::Save => " Save as ",
        DialogKind::Open => " Open ",
    };
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::Yellow))
        .title(title);
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let room = usize::from(inner.width);
    // A short terminal gives up the list first, then the path, then the hint.
    let list_rows = inner.height.saturating_sub(3);
    let show_path = inner.height >= 3;
    let show_hint = inner.height >= 2;
    let buffer = frame.buffer_mut();
    if show_path {
        let path = dialog.directory().display().to_string();
        buffer.set_stringn(
            inner.x,
            inner.y,
            keep_end(&path, room),
            room,
            Style::default(),
        );
    }
    draw_file_dialog_list(buffer, inner, dialog, list_rows);

    let name_y = inner.y + u16::from(show_path) + list_rows;
    let label_width = FILE_DIALOG_NAME_LABEL.len();
    // One cell stays free for the terminal cursor after the name.
    let name = dialog
        .name_line()
        .window(room.saturating_sub(label_width + 1));
    let line = format!("{FILE_DIALOG_NAME_LABEL}{}", name.text);
    buffer.set_stringn(inner.x, name_y, &line, room, Style::default());
    if show_hint {
        buffer.set_line(inner.x, name_y + 1, &file_dialog_hint(dialog), inner.width);
    }
    if dialog.pending_overwrite().is_none() {
        let x = inner.x + label_width as u16 + name.caret_x;
        frame.set_cursor_position((x.min(inner.right().saturating_sub(1)), name_y));
    }
}

/// Draws as much of the list as fits, starting late enough to show the highlight.
fn draw_file_dialog_list(buffer: &mut Buffer, inner: Rect, dialog: &FileDialog, rows: u16) {
    let room = usize::from(inner.width);
    let first = dialog.highlight().map_or(0, |highlight| {
        (highlight + 1).saturating_sub(usize::from(rows))
    });
    let visible = dialog.entries().iter().enumerate().skip(first);
    for (offset, (index, entry)) in visible.take(usize::from(rows)).enumerate() {
        let y = inner.y + 1 + offset as u16;
        let selected = dialog.highlight() == Some(index);
        let (marker, style) = if selected {
            (">", SELECTED_STYLE)
        } else {
            (" ", Style::default())
        };
        let line = format!("{marker} {:<room$}", entry.label());
        buffer.set_stringn(inner.x, y, line, room, style);
    }
}

fn file_dialog_hint(dialog: &FileDialog) -> Line<'static> {
    if let Some(path) = dialog.pending_overwrite() {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        Line::raw(format!("{name} exists. Overwrite? [y/n]"))
    } else if dialog.message().is_empty() {
        let action = match dialog.kind() {
            DialogKind::Save => " save   ",
            DialogKind::Open => " open   ",
        };
        Line::from(vec![
            Span::styled("Enter", Style::new().fg(Color::Yellow)),
            Span::raw(action),
            Span::styled("Esc", Style::new().fg(Color::Cyan)),
            Span::raw(" cancel"),
        ])
    } else {
        Line::styled(dialog.message().to_string(), Style::new().fg(Color::Red))
    }
}

/// Cuts text from the left to fit `room` terminal cells, so its end stays readable.
fn keep_end(text: &str, room: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if text_width(&chars) <= room {
        return text.to_string();
    }
    let mut start = 0;
    while start < chars.len() && text_width(&chars[start..]) + 1 > room {
        start += 1;
    }
    let tail: String = chars[start..].iter().collect();
    format!("…{tail}")
}

fn text_width(chars: &[char]) -> usize {
    chars.iter().map(|c| c.width().unwrap_or(0)).sum()
}

/// A story's tab and where it's drawn.
struct Tab {
    index: usize,
    x: u16,
    width: u16,
    name: String,
}

/// The tabs that fit, starting late enough to keep the current story on screen.
fn visible_tabs(area: Rect, app: &App) -> Vec<Tab> {
    let names: Vec<String> = app
        .document()
        .stories
        .iter()
        .map(|story| format!(" {} ", story.title))
        .collect();
    let current = app.story_index();
    let mut first = 0;
    while first < current
        && names[first..=current]
            .iter()
            .map(|n| n.width())
            .sum::<usize>()
            > usize::from(area.width)
    {
        first += 1;
    }
    let mut x = area.x;
    let mut tabs = Vec::new();
    for (index, name) in names.into_iter().enumerate().skip(first) {
        if x >= area.right() {
            break;
        }
        let width = name.width() as u16;
        tabs.push(Tab {
            index,
            x,
            width,
            name,
        });
        x = x.saturating_add(width);
    }
    tabs
}

/// Shows the stories' names, marking the current one.
fn draw_tabs(buffer: &mut Buffer, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    for tab in visible_tabs(area, app) {
        let style = if tab.index == app.story_index() {
            SELECTED_STYLE
        } else {
            Style::default()
        };
        let room = usize::from(area.right() - tab.x);
        buffer.set_stringn(tab.x, area.y, &tab.name, room, style);
    }
}

pub const NORMAL_HINTS: &str =
    " ? help · i edit · v select · / find · gO outline · Ctrl-S save · q quit";
pub const VISUAL_HINTS: &str =
    " d delete · y copy · c change · gb bold · gi italic · Esc back · ? help";
pub const INSERT_HINTS: &str = " Esc done · Enter new paragraph · Ctrl-S save · F1 help";
const OUTLINE_HINTS: &str = " Enter go · j k move · Esc back · gO close · ? help";

fn draw_status(buffer: &mut Buffer, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (name, color, hint) = match app.mode() {
        Mode::Normal => ("NORMAL", Color::Blue, NORMAL_HINTS),
        Mode::Insert => ("INSERT", Color::Green, INSERT_HINTS),
        Mode::Visual => ("VISUAL", Color::Magenta, VISUAL_HINTS),
        Mode::VisualLine => ("V-LINE", Color::Magenta, VISUAL_HINTS),
        Mode::Outline => ("NORMAL", Color::Blue, OUTLINE_HINTS),
        Mode::FileDialog => match app.file_dialog().map(FileDialog::kind) {
            Some(DialogKind::Open) => ("OPEN", Color::Blue, " Choose a document"),
            _ => ("SAVE", Color::Blue, " Choose a folder and a name"),
        },
        _ => ("NORMAL", Color::Blue, ""),
    };
    let (message, message_style) = if app.mode() == Mode::Search {
        (
            format!(" /{}", search_window(area, app).text),
            Style::default(),
        )
    } else if app.mode() == Mode::GoTo || app.status().is_empty() {
        (hint.to_string(), Style::default())
    } else if app.status_is_error() {
        (format!(" {}", app.status()), Style::new().fg(Color::Red))
    } else {
        (format!(" {}", app.status()), Style::default())
    };
    let badge = format!(" {name} ");
    let badge_style = Style::new().fg(Color::Black).bg(color);
    let room = usize::from(area.width);
    buffer.set_stringn(area.x, area.y, &badge, room, badge_style);
    let badge_width = badge.len() as u16;
    if badge_width < area.width {
        buffer.set_stringn(
            area.x + badge_width,
            area.y,
            message,
            room - usize::from(badge_width),
            message_style,
        );
    }
    // Keys typed so far, then where the cursor is, at the right edge.
    let (paragraph, count) = app.place();
    let pending = app.pending_keys();
    let place = match pending.is_empty() {
        true => format!(" ¶ {paragraph}/{count}"),
        false => format!(" {pending}  ¶ {paragraph}/{count}"),
    };
    let width = place.width() as u16;
    let used = badge_width + hint.width() as u16;
    if used + width < area.width || (!pending.is_empty() && width < area.width) {
        buffer.set_stringn(
            area.right() - width,
            area.y,
            &place,
            usize::from(width),
            Style::default(),
        );
    }
}
