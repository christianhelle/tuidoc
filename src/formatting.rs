//! Changing how text looks: bold and its kin, paragraph styles, lists and indents.

use std::ops::Range;

use crate::document::{Document, Refusal};
use crate::docx::package::{CONTENT_TYPES, RELATIONSHIPS, Relationship, directory};
use crate::docx::xml;
use crate::model::props::{PARAGRAPH_ORDER, RUN_ORDER};
use crate::model::{BlockPath, Position, index_of, with_index};
use crate::numbering::Numbering;
use crate::styles::Styles;

/// A format that is either on or off for each character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Bold,
    Italic,
    Underline,
    Strike,
}

impl Toggle {
    /// The name of the run property that carries it.
    fn property(self) -> &'static str {
        match self {
            Self::Bold => "b",
            Self::Italic => "i",
            Self::Underline => "u",
            Self::Strike => "strike",
        }
    }

    fn xml(self, on: bool) -> String {
        match (self, on) {
            (Self::Underline, true) => "<w:u w:val=\"single\"/>".into(),
            (Self::Underline, false) => "<w:u w:val=\"none\"/>".into(),
            (_, true) => format!("<w:{}/>", self.property()),
            (_, false) => format!("<w:{} w:val=\"0\"/>", self.property()),
        }
    }
}

const WORD: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";
const HEADING_SIZES: [u32; 6] = [32, 28, 26, 24, 22, 22];
/// Twentieths of a point that one step of indent moves a paragraph: half an inch.
const INDENT_STEP: u32 = 720;
const DEEPEST: usize = 8;

impl Document {
    /// The stretch of each paragraph that lies between two places.
    fn stretches(
        &self,
        from: Position,
        to: Position,
    ) -> Result<Vec<(BlockPath, Range<usize>)>, Refusal> {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        if !Self::together(from.path, to.path) {
            return Err(Refusal::AcrossCells);
        }
        Ok((index_of(from.path)..=index_of(to.path))
            .map(|index| with_index(from.path, index))
            .filter_map(|path| {
                let len = self.paragraph(path)?.len();
                let start = if path == from.path { from.offset } else { 0 };
                let end = if path == to.path { to.offset } else { len };
                Some((path, start..end.min(len)))
            })
            .collect())
    }

    /// Switches a format on for the text between two places, or off if all of it already had
    /// it, as the buttons in Word do.
    pub fn toggle(&mut self, from: Position, to: Position, toggle: Toggle) -> Result<(), Refusal> {
        let stretches = self.stretches(from, to)?;
        let name = toggle.property();
        let mut any = false;
        let mut all = true;
        for (path, range) in &stretches {
            let Some(paragraph) = self.paragraph(*path) else {
                continue;
            };
            for props in paragraph.formats(range.clone()) {
                any = true;
                all &= self.styles.flag(&paragraph.props, props, name);
            }
        }
        if !any {
            return Err(Refusal::Nothing);
        }
        let on = !all;
        let styles = self.styles.clone();
        for (path, range) in stretches {
            let Some(paragraph) = self.paragraph_mut(path) else {
                continue;
            };
            let paragraph_props = paragraph.props.clone();
            paragraph.reformat(range, |props| {
                props.remove(name);
                // Saying nothing is enough when the style already agrees.
                if styles.inherited_flag(&paragraph_props, props, name) != on {
                    props.set(name, &toggle.xml(on), RUN_ORDER);
                }
            });
        }
        Ok(())
    }

    /// Makes paragraphs headings of a level, or ordinary text for level 0.
    pub fn set_heading(&mut self, paths: &[BlockPath], level: u8) {
        let style = match level {
            0 => None,
            level => Some(self.heading_style(level)),
        };
        self.set_style(paths, style.as_deref());
    }

    /// Puts paragraphs in a style, or back in the default one. Either way they leave any list
    /// they were put in by hand.
    pub fn set_style(&mut self, paths: &[BlockPath], style: Option<&str>) {
        for path in paths {
            self.change_paragraph(*path, |props| {
                props.remove("numPr");
                match style {
                    Some(id) => props.set(
                        "pStyle",
                        &format!("<w:pStyle w:val=\"{}\"/>", xml::escape(id)),
                        PARAGRAPH_ORDER,
                    ),
                    None => {
                        props.remove("pStyle");
                    }
                }
            });
        }
    }

    /// Takes paragraphs from no list to a bulleted one, to a numbered one, and back out,
    /// going by where the first of them stands.
    pub fn cycle_list(&mut self, paths: &[BlockPath]) {
        let current = paths
            .first()
            .and_then(|path| self.paragraph(*path))
            .and_then(|paragraph| self.styles.list_item(&paragraph.props))
            .map(|(list, _)| self.numbering.is_bulleted(&list).unwrap_or(false));
        let list = match current {
            None => Some(self.list(true)),
            Some(true) => Some(self.list(false)),
            Some(false) => None,
        };
        for path in paths {
            let depth = self
                .paragraph(*path)
                .and_then(|paragraph| self.styles.list_item(&paragraph.props))
                .map_or(0, |(_, depth)| depth);
            // A list that comes from the style has to be switched off rather than left out.
            let styled = self.paragraph(*path).is_some_and(|paragraph| {
                let mut bare = paragraph.props.clone();
                bare.remove("numPr");
                self.styles.list_item(&bare).is_some()
            });
            let numbering = match &list {
                Some(list) => Some(numbering_xml(list, depth)),
                None if styled => Some(numbering_xml("0", 0)),
                None => None,
            };
            self.change_paragraph(*path, |props| match numbering {
                Some(raw) => props.set("numPr", &raw, PARAGRAPH_ORDER),
                None => {
                    props.remove("numPr");
                }
            });
        }
    }

    /// Takes a paragraph out of whatever list it is in.
    pub fn leave_list(&mut self, path: BlockPath) {
        self.change_paragraph(path, |props| {
            props.remove("numPr");
        });
        let styled = self
            .paragraph(path)
            .is_some_and(|paragraph| self.styles.list_item(&paragraph.props).is_some());
        if styled {
            self.change_paragraph(path, |props| {
                props.set("numPr", &numbering_xml("0", 0), PARAGRAPH_ORDER);
            });
        }
    }

    /// Moves list items a level deeper or shallower, and other paragraphs' left edge in or
    /// out by half an inch.
    pub fn shift(&mut self, paths: &[BlockPath], deeper: bool) {
        for path in paths {
            let Some(paragraph) = self.paragraph(*path) else {
                continue;
            };
            if let Some((list, depth)) = self.styles.list_item(&paragraph.props) {
                let depth = match deeper {
                    true => (depth + 1).min(DEEPEST),
                    false => depth.saturating_sub(1),
                };
                self.change_paragraph(*path, |props| {
                    props.set("numPr", &numbering_xml(&list, depth), PARAGRAPH_ORDER);
                });
                continue;
            }
            let existing = paragraph.props.get("ind").map(str::to_string);
            let left = existing
                .as_deref()
                .and_then(|raw| xml::attr_of(raw, "left").or_else(|| xml::attr_of(raw, "start")))
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let left = match deeper {
                true => left + INDENT_STEP,
                false => left.saturating_sub(INDENT_STEP),
            };
            let raw = match existing {
                Some(raw) => set_attr(&raw, "left", &left.to_string()),
                None => format!("<w:ind w:left=\"{left}\"/>"),
            };
            self.change_paragraph(*path, |props| props.set("ind", &raw, PARAGRAPH_ORDER));
        }
    }

    // ---- definitions the file may not have yet ----

    /// A part's text as it now stands: as tuidoc rewrote it, or as the file has it.
    fn part_source(&self, part: &str) -> Option<String> {
        match self.parts.get(part) {
            Some(content) => Some(content.clone()),
            None => self.package.read(part).ok().flatten(),
        }
    }

    /// The part of a kind the document links to, made and linked first if there is none.
    fn ensure_part(&mut self, kind: &str, file: &str, root: &str) -> String {
        if let Some(part) = self.package.part(kind) {
            return part.to_string();
        }
        let part = format!("{}{file}", directory(&self.package.main));
        self.parts.insert(
            part.clone(),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:{root} xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"></w:{root}>"
            ),
        );
        if let Some(types) = self.part_source(CONTENT_TYPES) {
            let entry = format!(
                "<Override PartName=\"/{part}\" ContentType=\"{WORD}.{kind}+xml\"/></Types>"
            );
            self.parts
                .insert(CONTENT_TYPES.into(), types.replacen("</Types>", &entry, 1));
        }
        let rels_part = self.package.main_rels();
        let rels = self.part_source(&rels_part).unwrap_or_else(|| {
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".into()
        });
        let number = (1..)
            .find(|number| self.package.target(&format!("rId{number}")).is_none())
            .unwrap_or(1);
        let id = format!("rId{number}");
        let entry = format!(
            "<Relationship Id=\"{id}\" Type=\"{RELATIONSHIPS}{kind}\" Target=\"{file}\"/></Relationships>"
        );
        self.parts
            .insert(rels_part, rels.replacen("</Relationships>", &entry, 1));
        self.package.relationships.push(Relationship {
            id,
            kind: kind.into(),
            target: part.clone(),
            external: false,
        });
        part
    }

    /// The id of the heading style of a level, which is added to the file's styles if they
    /// don't have one: Word leaves out the styles a document has never used.
    fn heading_style(&mut self, level: u8) -> String {
        let name = format!("heading {level}");
        if let Some(style) = self.styles.named(&name) {
            return style.id.clone();
        }
        let part = self.ensure_part("styles", "styles.xml", "styles");
        let source = self.part_source(&part).unwrap_or_default();
        let id = std::iter::once(format!("Heading{level}"))
            .chain((2..).map(|n| format!("Heading{level}x{n}")))
            .find(|id| !self.styles.has(id))
            .unwrap_or_default();
        let based_on = match self.styles.normal() {
            Some(normal) => format!(
                "<w:basedOn w:val=\"{0}\"/><w:next w:val=\"{0}\"/>",
                xml::escape(normal)
            ),
            None => String::new(),
        };
        let size = HEADING_SIZES[usize::from(level - 1).min(5)];
        let style = format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>{based_on}<w:uiPriority w:val=\"9\"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before=\"240\" w:after=\"80\"/><w:outlineLvl w:val=\"{}\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"{size}\"/></w:rPr></w:style></w:styles>",
            level - 1
        );
        let source = close_root(&source, "w:styles").replacen("</w:styles>", &style, 1);
        if let Ok(styles) = Styles::parse(&source) {
            self.styles = styles;
        }
        self.parts.insert(part, source);
        id
    }

    /// The id of a bulleted or a numbered list to put paragraphs in: one the file already
    /// has, or else a new one.
    fn list(&mut self, bulleted: bool) -> String {
        if let Some(list) = self.numbering.find(bulleted) {
            return list.to_string();
        }
        let part = self.ensure_part("numbering", "numbering.xml", "numbering");
        let source = close_root(&self.part_source(&part).unwrap_or_default(), "w:numbering");
        let (highest_definition, highest_list) = self.numbering.highest_ids();
        let (definition, list) = (highest_definition + 1, highest_list + 1);
        let mut levels = String::new();
        for level in 0..=DEEPEST {
            let (format, text) = match bulleted {
                true => ("bullet", ["•", "o", "▪"][level % 3].to_string()),
                false => (
                    ["decimal", "lowerLetter", "lowerRoman"][level % 3],
                    format!("%{}.", level + 1),
                ),
            };
            levels.push_str(&format!(
                "<w:lvl w:ilvl=\"{level}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{format}\"/><w:lvlText w:val=\"{text}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
                INDENT_STEP as usize * (level + 1)
            ));
        }
        let abstract_xml = format!(
            "<w:abstractNum w:abstractNumId=\"{definition}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>{levels}</w:abstractNum>"
        );
        let list_xml =
            format!("<w:num w:numId=\"{list}\"><w:abstractNumId w:val=\"{definition}\"/></w:num>");
        // Word wants every definition before the first list, and lists before the closing
        // odds and ends.
        let first_list = xml::tag_within(&source, "w:num")
            .map(|tag| tag.as_ptr() as usize - source.as_ptr() as usize);
        let mut source = source;
        let lists_end = match first_list {
            Some(at) => {
                source.insert_str(at, &abstract_xml);
                source.rfind("</w:num>").map(|end| end + "</w:num>".len())
            }
            None => None,
        };
        match lists_end {
            Some(at) => source.insert_str(at, &list_xml),
            None => {
                let at = xml::tag_within(&source, "w:numIdMacAtCleanup")
                    .map(|tag| tag.as_ptr() as usize - source.as_ptr() as usize)
                    .or_else(|| source.rfind("</w:numbering>"))
                    .unwrap_or(source.len());
                let both = match first_list {
                    Some(_) => list_xml,
                    None => format!("{abstract_xml}{list_xml}"),
                };
                source.insert_str(at, &both);
            }
        }
        if let Ok(numbering) = Numbering::parse(&source) {
            self.numbering = numbering;
        }
        self.parts.insert(part, source);
        list.to_string()
    }
}

fn numbering_xml(list: &str, depth: usize) -> String {
    format!(
        "<w:numPr><w:ilvl w:val=\"{depth}\"/><w:numId w:val=\"{}\"/></w:numPr>",
        xml::escape(list)
    )
}

/// A part whose root element was written as empty, opened up so things can go inside it.
fn close_root(source: &str, root: &str) -> String {
    if source.contains(&format!("</{root}>")) {
        return source.to_string();
    }
    match source.trim_end().strip_suffix("/>") {
        Some(rest) => format!("{rest}></{root}>"),
        None => source.to_string(),
    }
}

/// An element's opening tag with one attribute set, whether or not it had it.
fn set_attr(tag: &str, name: &str, value: &str) -> String {
    let needle = format!("w:{name}=\"");
    match tag.find(&needle) {
        Some(found) => {
            let start = found + needle.len();
            let end = start + tag[start..].find('"').unwrap_or(0);
            format!("{}{value}{}", &tag[..start], &tag[end..])
        }
        None => match tag.find(|c: char| c.is_whitespace() || c == '/' || c == '>') {
            Some(at) => format!("{} w:{name}=\"{value}\"{}", &tag[..at], &tag[at..]),
            None => tag.to_string(),
        },
    }
}
