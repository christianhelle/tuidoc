//! The line numbers beside the text: the cursor's own, and every other line's distance from it.

/// The gutter's width: room for three digits and a space before the text.
pub const WIDTH: u16 = 4;

/// The cursor line's label: its paragraph's number, at the left as vim puts it.
pub fn current(number: usize) -> String {
    format!("{number:<3} ")
}

/// Another line's label: how many lines away it is, for counts like `5j`.
pub fn relative(distance: usize) -> String {
    format!("{distance:>3} ")
}
