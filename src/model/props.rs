//! The properties of a run or a paragraph, kept as the XML they were written in.

use std::sync::Arc;

use crate::docx::xml;

/// The order Word insists on for the children of a run's properties.
pub const RUN_ORDER: &[&str] = &[
    "rStyle",
    "rFonts",
    "b",
    "bCs",
    "i",
    "iCs",
    "caps",
    "smallCaps",
    "strike",
    "dstrike",
    "outline",
    "shadow",
    "emboss",
    "imprint",
    "noProof",
    "snapToGrid",
    "vanish",
    "webHidden",
    "color",
    "spacing",
    "w",
    "kern",
    "position",
    "sz",
    "szCs",
    "highlight",
    "u",
    "effect",
    "bdr",
    "shd",
    "fitText",
    "vertAlign",
    "rtl",
    "cs",
    "em",
    "lang",
    "eastAsianLayout",
    "specVanish",
    "oMath",
    "rPrChange",
];

/// The order Word insists on for the children of a paragraph's properties.
pub const PARAGRAPH_ORDER: &[&str] = &[
    "pStyle",
    "keepNext",
    "keepLines",
    "pageBreakBefore",
    "framePr",
    "widowControl",
    "numPr",
    "suppressLineNumbers",
    "pBdr",
    "shd",
    "tabs",
    "suppressAutoHyphens",
    "kinsoku",
    "wordWrap",
    "overflowPunct",
    "topLinePunct",
    "autoSpaceDE",
    "autoSpaceDN",
    "bidi",
    "adjustRightInd",
    "snapToGrid",
    "spacing",
    "ind",
    "contextualSpacing",
    "mirrorIndents",
    "suppressOverlap",
    "jc",
    "textDirection",
    "textAlignment",
    "textboxTightWrap",
    "outlineLvl",
    "divId",
    "cnfStyle",
    "rPr",
    "sectPr",
    "pPrChange",
];

/// One property: a whole child element of `w:rPr` or `w:pPr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prop {
    /// The element's name without `w:`; names from other namespaces keep their prefix.
    pub name: Arc<str>,
    pub raw: Arc<str>,
}

/// A set of properties in the order they are written. Whatever tuidoc doesn't understand stays
/// in here untouched, which is how fonts, colours and the rest survive an edit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Props(Vec<Prop>);

impl Props {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds a property as read, after the ones already there.
    pub fn push(&mut self, name: &str, raw: &str) {
        self.0.push(Prop {
            name: name.into(),
            raw: raw.into(),
        });
    }

    /// The XML of the property with this name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|prop| &*prop.name == name)
            .map(|prop| &*prop.raw)
    }

    /// The property's `w:val`.
    pub fn val(&self, name: &str) -> Option<String> {
        let raw = self.get(name)?;
        xml::attr_of(&raw[..=raw.find('>')?], "val")
    }

    /// Whether an on/off property is set here, and which way. Absent means "ask the style".
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.get(name)?;
        Some(match self.val(name) {
            Some(value) => !matches!(value.as_str(), "0" | "false" | "off" | "none"),
            None => true,
        })
    }

    /// Sets a property, putting a new one where the schema's order wants it.
    pub fn set(&mut self, name: &str, raw: &str, order: &[&str]) {
        self.remove(name);
        let rank = |name: &str| order.iter().position(|known| *known == name);
        let at = match rank(name) {
            Some(mine) => self
                .0
                .iter()
                .position(|prop| rank(&prop.name).is_none_or(|theirs| theirs > mine))
                .unwrap_or(self.0.len()),
            None => self.0.len(),
        };
        self.0.insert(
            at,
            Prop {
                name: name.into(),
                raw: raw.into(),
            },
        );
    }

    pub fn remove(&mut self, name: &str) -> Option<Prop> {
        let at = self.0.iter().position(|prop| &*prop.name == name)?;
        Some(self.0.remove(at))
    }

    /// The properties as XML, ready to sit inside `w:rPr` or `w:pPr`.
    pub fn xml(&self) -> String {
        self.0.iter().map(|prop| &*prop.raw).collect()
    }
}
