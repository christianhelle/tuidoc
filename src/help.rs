//! What the help dialog lists: one table of keys for each mode.

use crate::app::Mode;

/// The dialog's width, which every table has to fit.
pub const HELP_WIDTH: u16 = 76;
/// The width of the key column.
pub const HELP_KEYS: usize = 25;
/// The longest table that fits a 24-line terminal with the dialog's border.
pub const HELP_ROWS: usize = 22;

pub type HelpTable = &'static [(&'static str, &'static str)];

pub const NORMAL_HELP: HelpTable = &[
    (
        "h j k l ← ↓ ↑ →",
        "Move; j and k go by screen line; counts as in 5j",
    ),
    ("w b e W B E", "Next word, word back, end of word"),
    (
        "0 ^ $ Home End",
        "Paragraph start, first text, end; line edges",
    ),
    (
        "f F t T ; ,",
        "Find a character in the paragraph, and again",
    ),
    (
        "{ } [[ ]]",
        "Previous, next paragraph; previous, next heading",
    ),
    ("gg G {n}G Ctrl-G", "First, last, nth paragraph; ask which"),
    ("H M L zz zt zb", "Move on screen; place the cursor's line"),
    ("Ctrl-D/U/F/B PgDn PgUp", "Half page and page"),
    ("Tab Shift-Tab", "Next, previous cell in a table"),
    (
        "i a I A o O",
        "Type here, after, at the start, end, below, above",
    ),
    (
        "x X D C s S r J",
        "Delete, change, replace characters; join",
    ),
    (
        "d c y + move, dd cc yy",
        "Delete, change, copy; iw aw ip as in diw",
    ),
    ("p P u Ctrl-R", "Paste after, before; undo, redo"),
    ("v V", "Select text, or whole paragraphs"),
    (
        "gb gi gu gs + move",
        "Bold, italic, underline, strike, as in gbiw",
    ),
    ("g0 g1…g6 gl gh", "Normal, heading 1–6, list, pick a style"),
    (">> <<", "List level or indent, deeper and back"),
    (
        "/ n N * K",
        "Find, next, previous, this word; look up a note",
    ),
    ("gt gT gO", "Next, previous part of the document; outline"),
    ("Ctrl-S F12 Ctrl-O F5", "Save, save as, open, reload"),
    ("q ZZ ZQ", "Quit, save and quit, quit without saving"),
    ("", "Press any key to close"),
];

pub const INSERT_HELP: HelpTable = &[
    ("Esc Ctrl-C", "Back to normal mode"),
    ("Enter", "New paragraph; ends the list on an empty item"),
    ("Backspace Delete", "Delete; at a paragraph's edge, join it"),
    ("← → ↑ ↓ Home End", "Move"),
    (
        "Ctrl-W Ctrl-U",
        "Delete the word before, or back to the start",
    ),
    (
        "Tab Shift-Tab",
        "A tab; in a table, the next or previous cell",
    ),
    ("Ctrl-S", "Save"),
    ("", "Press any key to close"),
];

pub const VISUAL_HELP: HelpTable = &[
    ("h j k l w b e …", "Extend the selection with any move"),
    ("o", "Jump to the other end of the selection"),
    ("d x y c s", "Delete, copy, or change what is selected"),
    ("gb gi gu gs", "Bold, italic, underline, strike"),
    ("g0 g1…g6 gl", "Normal, heading 1–6, list"),
    ("> < J", "Indent, outdent, join the paragraphs"),
    ("v V", "Switch between text and paragraphs, or leave"),
    ("Esc Ctrl-C", "Back to normal mode"),
    ("", "Press any key to close"),
];

pub const OUTLINE_HELP: HelpTable = &[
    ("j k ↓ ↑ gg G", "Move through the headings"),
    ("Enter", "Go to the heading"),
    ("Esc Tab", "Back to the document"),
    ("gO q", "Close the outline"),
    ("", "Press any key to close"),
];

/// The table for the mode help was asked from.
pub fn table(mode: Mode) -> HelpTable {
    match mode {
        Mode::Insert => INSERT_HELP,
        Mode::Visual | Mode::VisualLine => VISUAL_HELP,
        Mode::Outline => OUTLINE_HELP,
        _ => NORMAL_HELP,
    }
}
