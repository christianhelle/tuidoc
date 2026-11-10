//! The grammar of keys: counts, operators and motions, put together into commands.

use crossterm::event::{KeyCode, KeyEvent};

use crate::keys::is_ctrl;
use crate::motion::Motion;

/// Something done to a stretch of text that a motion or a selection marks out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    Bold,
    Italic,
    Underline,
    Strike,
    Indent,
    Outdent,
}

impl Operator {
    /// The keys that start it, for showing what has been typed so far.
    fn keys(self) -> &'static str {
        match self {
            Self::Delete => "d",
            Self::Change => "c",
            Self::Yank => "y",
            Self::Bold => "gb",
            Self::Italic => "gi",
            Self::Underline => "gu",
            Self::Strike => "gs",
            Self::Indent => ">",
            Self::Outdent => "<",
        }
    }

    /// Whether it always acts on whole paragraphs, whatever motion it is given.
    pub fn is_paragraph_wide(self) -> bool {
        matches!(self, Self::Indent | Self::Outdent)
    }
}

/// A stretch of text named by what it is rather than by how to get across it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    InnerWord,
    AWord,
    Paragraph,
}

/// Where typing starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertAt {
    Here,
    After,
    FirstNonBlank,
    End,
    Below,
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    Middle,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Move(Motion, Option<usize>),
    Operate(Operator, Motion, Option<usize>),
    /// An operator on this many whole paragraphs, as `dd` and `3yy`.
    OperateLines(Operator, usize),
    OperateObject(Operator, Object),
    /// An operator on what visual mode has selected.
    OperateSelection(Operator),
    Insert(InsertAt),
    DeleteChars(usize),
    DeleteBefore(usize),
    DeleteToEnd,
    ChangeToEnd,
    Substitute(usize),
    Replace(char),
    Join(usize),
    Paste {
        before: bool,
        count: usize,
    },
    Undo,
    Redo,
    Visual,
    VisualLine,
    SwapEnds,
    /// A heading of this level, or ordinary text for 0.
    Heading(u8),
    CycleList,
    StylePicker,
    NextStory,
    PreviousStory,
    Outline,
    Peek,
    Search,
    SearchNext,
    SearchPrevious,
    SearchWord,
    RepeatFind {
        reversed: bool,
    },
    GoTo,
    Save,
    SaveAs,
    Open,
    Reload,
    Quit,
    SaveAndQuit,
    QuitNow,
    Help,
    Scroll(Scroll),
    Escape,
}

/// The keys typed so far that don't yet make a command.
#[derive(Debug, Default)]
pub struct Grammar {
    count: Option<usize>,
    operator: Option<Operator>,
    /// The count typed after the operator, as the 3 in `d3w`.
    inner_count: Option<usize>,
    /// A key that waits for another: `g`, `z`, `f` and the like.
    prefix: Option<char>,
}

impl Grammar {
    /// Drops whatever was half typed.
    pub fn forget(&mut self) {
        *self = Self::default();
    }

    /// The keys typed so far, as vim shows them at the edge of the screen.
    pub fn pending_keys(&self) -> String {
        let mut keys = String::new();
        if let Some(count) = self.count {
            keys.push_str(&count.to_string());
        }
        if let Some(operator) = self.operator {
            keys.push_str(operator.keys());
        }
        if let Some(count) = self.inner_count {
            keys.push_str(&count.to_string());
        }
        if let Some(prefix) = self.prefix {
            keys.push(prefix);
        }
        keys
    }

    fn total(&self) -> Option<usize> {
        match (self.count, self.inner_count) {
            (None, None) => None,
            (outer, inner) => Some(outer.unwrap_or(1) * inner.unwrap_or(1)),
        }
    }

    fn digit(&mut self, digit: usize) {
        let count = match self.operator {
            Some(_) => &mut self.inner_count,
            None => &mut self.count,
        };
        *count = Some((count.unwrap_or(0) * 10 + digit).min(99_999));
    }

    fn has_count(&self) -> bool {
        match self.operator {
            Some(_) => self.inner_count.is_some(),
            None => self.count.is_some(),
        }
    }

    /// The command a finished key sequence makes, with the grammar ready for the next one.
    fn finish(&mut self, command: Command) -> Option<Command> {
        self.forget();
        Some(command)
    }

    /// A motion on its own, or as what an operator acts over.
    fn moved(&mut self, motion: Motion) -> Option<Command> {
        let count = self.total();
        match self.operator {
            Some(operator) => self.finish(Command::Operate(operator, motion, count)),
            None => self.finish(Command::Move(motion, count)),
        }
    }

    /// An operator key: it waits for a motion, or typed twice takes whole paragraphs.
    fn operator(&mut self, operator: Operator, visual: bool) -> Option<Command> {
        if visual {
            return self.finish(Command::OperateSelection(operator));
        }
        match self.operator {
            Some(pending) if pending == operator => {
                let count = self.total().unwrap_or(1);
                self.finish(Command::OperateLines(operator, count))
            }
            Some(_) => {
                self.forget();
                None
            }
            None => {
                self.operator = Some(operator);
                self.prefix = None;
                None
            }
        }
    }

    /// The motion a key stands for, if it is one that needs no second key.
    fn motion(key: KeyEvent) -> Option<Motion> {
        if is_ctrl(key) {
            return Some(match key.code {
                KeyCode::Char('d') => Motion::HalfPageDown,
                KeyCode::Char('u') => Motion::HalfPageUp,
                KeyCode::Char('f') => Motion::PageDown,
                KeyCode::Char('b') => Motion::PageUp,
                KeyCode::Home => Motion::First,
                KeyCode::End => Motion::Last,
                _ => return None,
            });
        }
        Some(match key.code {
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => Motion::Left,
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => Motion::Right,
            KeyCode::Char('j') | KeyCode::Down => Motion::Down,
            KeyCode::Char('k') | KeyCode::Up => Motion::Up,
            KeyCode::Char('w') => Motion::WordForward(false),
            KeyCode::Char('W') => Motion::WordForward(true),
            KeyCode::Char('b') => Motion::WordBack(false),
            KeyCode::Char('B') => Motion::WordBack(true),
            KeyCode::Char('e') => Motion::WordEnd(false),
            KeyCode::Char('E') => Motion::WordEnd(true),
            KeyCode::Char('0') => Motion::ParagraphStart,
            KeyCode::Char('^') => Motion::FirstNonBlank,
            KeyCode::Char('$') => Motion::ParagraphEnd,
            KeyCode::Home => Motion::LineStart,
            KeyCode::End => Motion::LineEnd,
            KeyCode::Char('{') => Motion::PreviousParagraph,
            KeyCode::Char('}') => Motion::NextParagraph,
            KeyCode::Char('G') => Motion::Last,
            KeyCode::Char('H') => Motion::ScreenTop,
            KeyCode::Char('M') => Motion::ScreenMiddle,
            KeyCode::Char('L') => Motion::ScreenBottom,
            KeyCode::PageDown => Motion::PageDown,
            KeyCode::PageUp => Motion::PageUp,
            KeyCode::Tab => Motion::NextCell,
            KeyCode::BackTab => Motion::PreviousCell,
            _ => return None,
        })
    }

    /// What the key after a waiting key makes of the two.
    fn after_prefix(&mut self, prefix: char, key: KeyEvent, visual: bool) -> Option<Command> {
        let KeyCode::Char(c) = key.code else {
            self.forget();
            return None;
        };
        match (prefix, c) {
            ('f' | 'F' | 't' | 'T', target) => self.moved(Motion::Find {
                target,
                forward: prefix.is_lowercase(),
                till: prefix.eq_ignore_ascii_case(&'t'),
            }),
            ('r', replacement) => self.finish(Command::Replace(replacement)),
            ('g', 'g') => self.moved(Motion::First),
            ('g', 'j') => self.moved(Motion::Down),
            ('g', 'k') => self.moved(Motion::Up),
            ('g', 'b') => self.operator(Operator::Bold, visual),
            ('g', 'i') => self.operator(Operator::Italic, visual),
            ('g', 'u') => self.operator(Operator::Underline, visual),
            ('g', 's') => self.operator(Operator::Strike, visual),
            ('g', level @ '0'..='6') => self.finish(Command::Heading(level as u8 - b'0')),
            ('g', 'l') => self.finish(Command::CycleList),
            ('g', 'h') => self.finish(Command::StylePicker),
            ('g', 't') => self.finish(Command::NextStory),
            ('g', 'T') => self.finish(Command::PreviousStory),
            ('g', 'O') => self.finish(Command::Outline),
            ('z', 'z') => self.finish(Command::Scroll(Scroll::Middle)),
            ('z', 't') => self.finish(Command::Scroll(Scroll::Top)),
            ('z', 'b') => self.finish(Command::Scroll(Scroll::Bottom)),
            ('Z', 'Z') => self.finish(Command::SaveAndQuit),
            ('Z', 'Q') => self.finish(Command::QuitNow),
            ('[', '[') => self.moved(Motion::PreviousHeading),
            (']', ']') => self.moved(Motion::NextHeading),
            ('i' | 'a', object) => {
                let object = match (prefix, object) {
                    ('i', 'w') => Object::InnerWord,
                    ('a', 'w') => Object::AWord,
                    (_, 'p') => Object::Paragraph,
                    _ => {
                        self.forget();
                        return None;
                    }
                };
                match self.operator {
                    Some(operator) => self.finish(Command::OperateObject(operator, object)),
                    None => {
                        self.forget();
                        None
                    }
                }
            }
            _ => {
                self.forget();
                None
            }
        }
    }

    /// Takes a key in normal mode and gives the command it completes, if it completes one.
    pub fn normal(&mut self, key: KeyEvent) -> Option<Command> {
        self.key(key, false)
    }

    /// Takes a key in a visual mode, where operators act at once on the selection.
    pub fn visual(&mut self, key: KeyEvent) -> Option<Command> {
        self.key(key, true)
    }

    fn key(&mut self, key: KeyEvent, visual: bool) -> Option<Command> {
        if key.code == KeyCode::Esc || (is_ctrl(key) && key.code == KeyCode::Char('c') && visual) {
            return self.finish(Command::Escape);
        }
        if let Some(prefix) = self.prefix.take() {
            return self.after_prefix(prefix, key, visual);
        }
        if !is_ctrl(key)
            && let KeyCode::Char(c) = key.code
        {
            if let Some(digit) = c.to_digit(10)
                && (digit > 0 || self.has_count())
            {
                self.digit(digit as usize);
                return None;
            }
            let operator = match c {
                'd' => Some(Operator::Delete),
                'c' => Some(Operator::Change),
                'y' => Some(Operator::Yank),
                '>' => Some(Operator::Indent),
                '<' => Some(Operator::Outdent),
                'x' | 's' if visual => Some(match c {
                    'x' => Operator::Delete,
                    _ => Operator::Change,
                }),
                _ => None,
            };
            if let Some(operator) = operator {
                return self.operator(operator, visual);
            }
            let waits = matches!(c, 'g' | 'z' | 'Z' | 'f' | 'F' | 't' | 'T' | '[' | ']')
                || (c == 'r' && !visual && self.operator.is_none())
                || (matches!(c, 'i' | 'a') && self.operator.is_some());
            if waits {
                self.prefix = Some(c);
                return None;
            }
        }
        if let Some(motion) = Self::motion(key) {
            return self.moved(motion);
        }
        // Everything else stands alone, and makes no sense after an operator.
        if self.operator.is_some() {
            self.forget();
            return None;
        }
        let count = self.total().unwrap_or(1);
        let command = if is_ctrl(key) {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('y') => Command::Redo,
                KeyCode::Char('z') => Command::Undo,
                KeyCode::Char('g') => Command::GoTo,
                KeyCode::Char('s') => Command::Save,
                KeyCode::Char('o') => Command::Open,
                KeyCode::Char('q') | KeyCode::Char('c') => Command::Quit,
                _ => {
                    self.forget();
                    return None;
                }
            }
        } else {
            match key.code {
                KeyCode::Char('v') => Command::Visual,
                KeyCode::Char('V') => Command::VisualLine,
                KeyCode::Char('o') if visual => Command::SwapEnds,
                KeyCode::Char('J') => Command::Join(count),
                KeyCode::Char('/') => Command::Search,
                KeyCode::Char('n') => Command::SearchNext,
                KeyCode::Char('N') => Command::SearchPrevious,
                KeyCode::Char('*') => Command::SearchWord,
                KeyCode::Char(';') => Command::RepeatFind { reversed: false },
                KeyCode::Char(',') => Command::RepeatFind { reversed: true },
                KeyCode::Char('K') => Command::Peek,
                KeyCode::Char('?') | KeyCode::F(1) => Command::Help,
                KeyCode::F(6)
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::SHIFT) =>
                {
                    Command::PreviousStory
                }
                KeyCode::F(6) => Command::NextStory,
                KeyCode::F(12) => Command::SaveAs,
                KeyCode::F(5) => Command::Reload,
                KeyCode::Delete if visual => Command::OperateSelection(Operator::Delete),
                _ if visual => {
                    self.forget();
                    return None;
                }
                KeyCode::Char('i') => Command::Insert(InsertAt::Here),
                KeyCode::Char('a') => Command::Insert(InsertAt::After),
                KeyCode::Char('I') => Command::Insert(InsertAt::FirstNonBlank),
                KeyCode::Char('A') => Command::Insert(InsertAt::End),
                KeyCode::Char('o') => Command::Insert(InsertAt::Below),
                KeyCode::Char('O') => Command::Insert(InsertAt::Above),
                KeyCode::Char('x') | KeyCode::Delete => Command::DeleteChars(count),
                KeyCode::Char('X') => Command::DeleteBefore(count),
                KeyCode::Char('D') => Command::DeleteToEnd,
                KeyCode::Char('C') => Command::ChangeToEnd,
                KeyCode::Char('s') => Command::Substitute(count),
                KeyCode::Char('S') => Command::OperateLines(Operator::Change, count),
                KeyCode::Char('Y') => Command::OperateLines(Operator::Yank, count),
                KeyCode::Char('p') => Command::Paste {
                    before: false,
                    count,
                },
                KeyCode::Char('P') => Command::Paste {
                    before: true,
                    count,
                },
                KeyCode::Char('u') => Command::Undo,
                KeyCode::Char('q') => Command::Quit,
                _ => {
                    self.forget();
                    return None;
                }
            }
        };
        self.finish(command)
    }
}
