//! A list to choose from that narrows as its filter is typed.

use crossterm::event::{KeyCode, KeyEvent};

use crate::keys::is_ctrl;
use crate::line_edit::LineEdit;

/// What a key press in the picker came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Picked {
    Stay,
    Cancel,
    /// The index of the chosen item among all the items.
    Chosen(usize),
}

#[derive(Debug, Default)]
pub struct Picker {
    items: Vec<String>,
    filter: LineEdit,
    /// Which of the items left by the filter is highlighted.
    highlight: usize,
}

impl Picker {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            filter: LineEdit::default(),
            highlight: 0,
        }
    }

    pub fn filter(&self) -> &LineEdit {
        &self.filter
    }

    pub fn highlight(&self) -> usize {
        self.highlight
    }

    /// The items the filter leaves, each with its index among all of them.
    pub fn visible(&self) -> Vec<(usize, &str)> {
        let wanted = self.filter.text().to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.to_lowercase().contains(&wanted))
            .map(|(index, item)| (index, item.as_str()))
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Picked {
        let count = self.visible().len();
        match key.code {
            KeyCode::Esc => return Picked::Cancel,
            KeyCode::Enter => {
                return match self.visible().get(self.highlight) {
                    Some((index, _)) => Picked::Chosen(*index),
                    None => Picked::Stay,
                };
            }
            KeyCode::Down | KeyCode::Tab => self.highlight = (self.highlight + 1) % count.max(1),
            KeyCode::Char('n') if is_ctrl(key) => {
                self.highlight = (self.highlight + 1) % count.max(1);
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.highlight = (self.highlight + count.max(1) - 1) % count.max(1);
            }
            KeyCode::Char('p') if is_ctrl(key) => {
                self.highlight = (self.highlight + count.max(1) - 1) % count.max(1);
            }
            _ => {
                self.filter.handle_key(key);
                self.highlight = 0;
            }
        }
        Picked::Stay
    }
}
