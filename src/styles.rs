//! The document's styles, and what a paragraph or run ends up looking like once they apply.

use std::collections::HashMap;

use crate::docx::DocxError;
use crate::docx::xml::{self, Element};
use crate::model::Props;

/// The formatting tuidoc can show and change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Format {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub id: String,
    /// The name Word shows, which unlike the id is the same in every language.
    pub name: String,
    pub paragraph: bool,
    pub based_on: Option<String>,
    pub paragraph_props: Props,
    pub run_props: Props,
}

#[derive(Debug, Clone, Default)]
pub struct Styles {
    styles: HashMap<String, Style>,
    /// The ids in the order the file lists them, for showing a list of styles.
    order: Vec<String>,
    default_paragraph: Option<String>,
    default_run_props: Props,
    default_paragraph_props: Props,
}

/// How deep a chain of styles based on styles is followed before giving up on it.
const DEPTH: usize = 20;

impl Styles {
    pub fn parse(source: &str) -> Result<Self, DocxError> {
        let root = xml::parse(source)?;
        let mut styles = Self::default();
        let props = |element: Option<&Element>| {
            let mut props = Props::default();
            for child in element.map_or(&[][..], |element| &element.children[..]) {
                let name = child.name.strip_prefix("w:").unwrap_or(&child.name);
                props.push(name, child.raw(source));
            }
            props
        };
        for child in &root.children {
            match child.name.as_str() {
                "w:docDefaults" => {
                    let run = child.child("w:rPrDefault").and_then(|d| d.child("w:rPr"));
                    let paragraph = child.child("w:pPrDefault").and_then(|d| d.child("w:pPr"));
                    styles.default_run_props = props(run);
                    styles.default_paragraph_props = props(paragraph);
                }
                "w:style" => {
                    let Some(id) = child.attr(source, "w:styleId") else {
                        continue;
                    };
                    let val = |name: &str| child.child(name).and_then(|e| e.attr(source, "w:val"));
                    let paragraph = child.attr(source, "w:type").as_deref() == Some("paragraph");
                    if paragraph && child.attr(source, "w:default").is_some_and(|v| v != "0") {
                        styles.default_paragraph = Some(id.clone());
                    }
                    styles.order.push(id.clone());
                    styles.styles.insert(
                        id.clone(),
                        Style {
                            name: val("w:name").unwrap_or_else(|| id.clone()),
                            id,
                            paragraph,
                            based_on: val("w:basedOn"),
                            paragraph_props: props(child.child("w:pPr")),
                            run_props: props(child.child("w:rPr")),
                        },
                    );
                }
                _ => {}
            }
        }
        Ok(styles)
    }

    pub fn get(&self, id: &str) -> Option<&Style> {
        self.styles.get(id)
    }

    /// The style Word calls by this name, whatever language the file was written in.
    pub fn named(&self, name: &str) -> Option<&Style> {
        self.order
            .iter()
            .filter_map(|id| self.styles.get(id))
            .find(|style| style.name.eq_ignore_ascii_case(name))
    }

    /// The paragraph styles, in the file's order.
    pub fn paragraph_styles(&self) -> Vec<&Style> {
        self.order
            .iter()
            .filter_map(|id| self.styles.get(id))
            .filter(|style| style.paragraph)
            .collect()
    }

    /// The id of the style paragraphs have when they name none.
    pub fn normal(&self) -> Option<&str> {
        self.default_paragraph.as_deref()
    }

    pub fn has(&self, id: &str) -> bool {
        self.styles.contains_key(id)
    }

    /// A style and everything it is based on, nearest first.
    fn chain(&self, id: Option<&str>) -> Vec<&Style> {
        let mut chain = Vec::new();
        let mut next = id;
        while let Some(style) = next.and_then(|id| self.styles.get(id)) {
            if chain.len() >= DEPTH {
                break;
            }
            chain.push(style);
            next = style.based_on.as_deref();
        }
        chain
    }

    fn paragraph_chain(&self, paragraph: &Props) -> Vec<&Style> {
        let id = paragraph.val("pStyle");
        self.chain(id.as_deref().or(self.default_paragraph.as_deref()))
    }

    /// The style a paragraph is in.
    pub fn style_of(&self, paragraph: &Props) -> Option<&Style> {
        self.paragraph_chain(paragraph).first().copied()
    }

    /// A paragraph property as it applies: set on the paragraph itself, or by its style, or by
    /// a style that one is based on, or by the document's defaults.
    pub fn paragraph_prop(&self, paragraph: &Props, name: &str) -> Option<String> {
        if let Some(raw) = paragraph.get(name) {
            return Some(raw.to_string());
        }
        self.paragraph_chain(paragraph)
            .iter()
            .find_map(|style| style.paragraph_props.get(name))
            .or_else(|| self.default_paragraph_props.get(name))
            .map(str::to_string)
    }

    /// Whether an on/off run property applies, before the run itself has its say.
    pub fn inherited_flag(&self, paragraph: &Props, run: &Props, name: &str) -> bool {
        let character = run.val("rStyle");
        self.chain(character.as_deref())
            .iter()
            .find_map(|style| style.run_props.flag(name))
            .or_else(|| {
                self.paragraph_chain(paragraph)
                    .iter()
                    .find_map(|style| style.run_props.flag(name))
            })
            .or_else(|| self.default_run_props.flag(name))
            .unwrap_or(false)
    }

    /// Whether an on/off run property applies to a run.
    pub fn flag(&self, paragraph: &Props, run: &Props, name: &str) -> bool {
        run.flag(name)
            .unwrap_or_else(|| self.inherited_flag(paragraph, run, name))
    }

    pub fn format(&self, paragraph: &Props, run: &Props) -> Format {
        Format {
            bold: self.flag(paragraph, run, "b"),
            italic: self.flag(paragraph, run, "i"),
            underline: self.flag(paragraph, run, "u"),
            strike: self.flag(paragraph, run, "strike"),
        }
    }

    /// The heading level of a paragraph, 1 for the largest, if it is a heading.
    pub fn heading_level(&self, paragraph: &Props) -> Option<u8> {
        if let Some(raw) = self.paragraph_prop(paragraph, "outlineLvl") {
            let level: u8 = xml::attr_of(&raw, "val")?.parse().ok()?;
            return (level < 9).then_some(level + 1);
        }
        self.paragraph_chain(paragraph).iter().find_map(|style| {
            let number = style.name.to_ascii_lowercase();
            let number = number.strip_prefix("heading ")?;
            number.parse().ok().filter(|level| (1..=9).contains(level))
        })
    }

    /// Whether a paragraph is the document's title.
    pub fn is_title(&self, paragraph: &Props) -> bool {
        self.paragraph_chain(paragraph)
            .iter()
            .any(|style| style.name.eq_ignore_ascii_case("title"))
    }
}
