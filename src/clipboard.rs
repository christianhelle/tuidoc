//! Copying to the system clipboard by asking the terminal to do it.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The escape sequence that tells a terminal to put text on the clipboard. It works over SSH
/// and needs nothing installed; terminals that don't know it ignore it.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let bits = u32::from(group[0]) << 16 | u32::from(group[1]) << 8 | u32::from(group[2]);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            match index <= chunk.len() {
                true => out.push(ALPHABET[(bits >> shift) as usize & 63] as char),
                false => out.push('='),
            }
        }
    }
    out
}
