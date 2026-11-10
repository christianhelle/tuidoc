//! List numbering: which bullet or number stands in front of each list paragraph.

use std::collections::HashMap;

use crate::docx::DocxError;
use crate::docx::xml;

/// One level of a list definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    /// The number format, such as `decimal` or `bullet`.
    pub format: String,
    /// The label's pattern, where `%1` stands for the first level's number.
    pub text: String,
    pub start: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Numbering {
    /// The levels of each abstract definition, by its id.
    definitions: HashMap<String, Vec<Level>>,
    /// Which definition each list uses, by the list's id.
    lists: HashMap<String, String>,
    /// The list ids in the order the file has them.
    order: Vec<String>,
}

/// A paragraph's place in a list: the list's id and how deep the paragraph is in it.
pub type ListItem = (String, usize);

impl Numbering {
    pub fn parse(source: &str) -> Result<Self, DocxError> {
        let root = xml::parse(source)?;
        let mut numbering = Self::default();
        for child in &root.children {
            match child.name.as_str() {
                "w:abstractNum" => {
                    let Some(id) = child.attr(source, "w:abstractNumId") else {
                        continue;
                    };
                    let levels = child
                        .children
                        .iter()
                        .filter(|level| level.is("w:lvl"))
                        .map(|level| {
                            let val = |name: &str| {
                                level.child(name).and_then(|e| e.attr(source, "w:val"))
                            };
                            Level {
                                format: val("w:numFmt").unwrap_or_else(|| "decimal".into()),
                                text: val("w:lvlText").unwrap_or_default(),
                                start: val("w:start").and_then(|s| s.parse().ok()).unwrap_or(1),
                            }
                        })
                        .collect();
                    numbering.definitions.insert(id, levels);
                }
                "w:num" => {
                    let id = child.attr(source, "w:numId");
                    let definition = child
                        .child("w:abstractNumId")
                        .and_then(|e| e.attr(source, "w:val"));
                    if let (Some(id), Some(definition)) = (id, definition) {
                        numbering.order.push(id.clone());
                        numbering.lists.insert(id, definition);
                    }
                }
                _ => {}
            }
        }
        Ok(numbering)
    }

    fn levels(&self, list: &str) -> Option<&Vec<Level>> {
        self.definitions.get(self.lists.get(list)?)
    }

    /// The first list whose top level is bulleted, or numbered.
    pub fn find(&self, bulleted: bool) -> Option<&str> {
        self.order
            .iter()
            .find(|id| {
                self.levels(id)
                    .and_then(|levels| levels.first())
                    .is_some_and(|level| (level.format == "bullet") == bulleted)
            })
            .map(String::as_str)
    }

    /// Whether a list's top level is bulleted, if the list exists at all.
    pub fn is_bulleted(&self, list: &str) -> Option<bool> {
        Some(self.levels(list)?.first()?.format == "bullet")
    }

    /// The largest ids in use, for making up new ones: of definitions, then of lists.
    pub fn highest_ids(&self) -> (u32, u32) {
        let highest = |ids: &mut dyn Iterator<Item = &String>| {
            ids.filter_map(|id| id.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
        };
        (
            highest(&mut self.definitions.keys()),
            highest(&mut self.lists.keys()),
        )
    }

    /// The label in front of each paragraph, given every paragraph's place in a list in reading
    /// order. Numbers count up as the paragraphs go by.
    pub fn labels(&self, items: &[Option<ListItem>]) -> Vec<Option<String>> {
        let mut counters: HashMap<&str, Vec<u32>> = HashMap::new();
        items
            .iter()
            .map(|item| {
                let (list, depth) = item.as_ref()?;
                let definition = self.lists.get(list)?;
                let levels = self.definitions.get(definition)?;
                let depth = (*depth).min(levels.len().checked_sub(1)?);
                let counts = counters
                    .entry(definition.as_str())
                    .or_insert_with(|| vec![0; levels.len()]);
                counts[depth] = match counts[depth] {
                    0 => levels[depth].start,
                    count => count + 1,
                };
                for deeper in &mut counts[depth + 1..] {
                    *deeper = 0;
                }
                let level = &levels[depth];
                if level.format == "bullet" {
                    return Some(["•", "◦", "▪"][depth % 3].to_string());
                }
                if level.format == "none" {
                    return Some(String::new());
                }
                let mut label = level.text.clone();
                for (index, other) in levels.iter().enumerate().take(depth + 1) {
                    let count = match counts[index] {
                        0 => other.start,
                        count => count,
                    };
                    label =
                        label.replace(&format!("%{}", index + 1), &number(count, &other.format));
                }
                Some(label)
            })
            .collect()
    }
}

/// A count written in a list's number format.
pub fn number(count: u32, format: &str) -> String {
    match format {
        "lowerLetter" => letters(count, b'a'),
        "upperLetter" => letters(count, b'A'),
        "lowerRoman" => roman(count).to_lowercase(),
        "upperRoman" => roman(count),
        _ => count.to_string(),
    }
}

/// a, b … z, aa, bb …, as Word counts with letters.
fn letters(count: u32, first: u8) -> String {
    if count == 0 {
        return String::new();
    }
    let letter = (first + ((count - 1) % 26) as u8) as char;
    std::iter::repeat_n(letter, ((count - 1) / 26 + 1) as usize).collect()
}

fn roman(mut count: u32) -> String {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, numeral) in NUMERALS {
        while count >= value {
            out.push_str(numeral);
            count -= value;
        }
    }
    out
}
