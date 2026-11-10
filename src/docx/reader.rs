//! Turns a part's XML into paragraphs and tables, remembering where each one came from.

use std::sync::Arc;

use super::DocxError;
use super::package::Package;
use super::xml::{self, Element};
use crate::model::inline::unwrapped;
use crate::model::table::{Cell, Row};
use crate::model::{
    Atom, AtomKind, Block, Inline, Marker, Merge, Note, Opaque, Paragraph, Props, Run, Story,
    StoryKind, Table, Wrapper, Wraps,
};

const MAIN_NAMESPACES: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

/// Elements that wrap runs and are kept around whatever is edited inside them.
const WRAPPERS: [&str; 8] = [
    "w:hyperlink",
    "w:ins",
    "w:moveTo",
    "w:smartTag",
    "w:fldSimple",
    "w:customXml",
    "w:dir",
    "w:bdo",
];

/// Paragraph children with no place in the text, kept exactly where they are.
const MARKERS: [&str; 14] = [
    "w:del",
    "w:moveFrom",
    "w:bookmarkStart",
    "w:bookmarkEnd",
    "w:commentRangeStart",
    "w:commentRangeEnd",
    "w:proofErr",
    "w:permStart",
    "w:permEnd",
    "w:moveFromRangeStart",
    "w:moveFromRangeEnd",
    "w:moveToRangeStart",
    "w:moveToRangeEnd",
    "w:customXmlInsRangeStart",
];

/// Run children with no place in the text.
const RUN_MARKERS: [&str; 9] = [
    "w:fldChar",
    "w:instrText",
    "w:lastRenderedPageBreak",
    "w:footnoteRef",
    "w:endnoteRef",
    "w:annotationRef",
    "w:separator",
    "w:continuationSeparator",
    "w:delText",
];

struct Reading<'a> {
    source: &'a str,
    package: &'a Package,
    next_wrapper: u32,
}

/// Reads one part as a story.
pub fn read_story(
    kind: StoryKind,
    title: &str,
    part: &str,
    source: Arc<str>,
    package: &Package,
) -> Result<Story, DocxError> {
    let root = xml::parse(&source)?;
    let bound = MAIN_NAMESPACES
        .iter()
        .any(|namespace| root.attr(&source, "xmlns:w").as_deref() == Some(namespace));
    if !bound {
        return Err(DocxError::Invalid(format!(
            "{part} doesn't use the w: prefix for Word's elements"
        )));
    }
    let mut reading = Reading {
        source: &source,
        package,
        next_wrapper: 0,
    };
    let mut story = Story {
        kind,
        title: title.to_string(),
        part: part.to_string(),
        source: source.clone(),
        head: 0..root.open_end,
        tail: root.close_start..source.len(),
        blocks: Vec::new(),
        notes: Vec::new(),
        editable: kind == StoryKind::Body,
    };
    match kind {
        StoryKind::Body => {
            let body = root
                .child("w:body")
                .ok_or_else(|| DocxError::Invalid("the document has no body".into()))?;
            story.head = 0..body.open_end;
            story.tail = body.close_start..source.len();
            story.blocks = reading.blocks(&body.children);
        }
        StoryKind::Header | StoryKind::Footer => {
            story.blocks = reading.blocks(&root.children);
        }
        StoryKind::Footnotes | StoryKind::Endnotes | StoryKind::Comments => {
            for note in &root.children {
                // Word keeps the rule above the footnotes in here as notes of their own.
                if note.attr(&source, "w:type").is_some() {
                    continue;
                }
                let id = note.attr(&source, "w:id").unwrap_or_default();
                let mut blocks = reading.blocks(&note.children);
                let label = match kind {
                    StoryKind::Comments => {
                        let author = note.attr(&source, "w:author").unwrap_or_default();
                        format!("[c{id}] {author}: ")
                    }
                    _ => format!("[{id}] "),
                };
                if let Some(Block::Paragraph(first)) = blocks.first_mut() {
                    first.insert_text(0, &label);
                    first.dirty = false;
                }
                let start = story.blocks.len();
                story.blocks.extend(blocks);
                story.notes.push(Note {
                    id,
                    blocks: start..story.blocks.len(),
                });
            }
        }
    }
    if !story
        .blocks
        .iter()
        .any(|block| !matches!(block, Block::Opaque(_)))
        && kind != StoryKind::Body
    {
        story
            .blocks
            .push(Block::Paragraph(clean(Paragraph::default())));
    }
    Ok(story)
}

/// A paragraph made up while reading, which mustn't count as an edit.
fn clean(mut paragraph: Paragraph) -> Paragraph {
    paragraph.dirty = false;
    paragraph
}

impl Reading<'_> {
    fn blocks(&mut self, elements: &[Element]) -> Vec<Block> {
        elements
            .iter()
            .map(|element| match element.name.as_str() {
                "w:p" => Block::Paragraph(self.paragraph(element)),
                "w:tbl" => match self.table(element) {
                    Some(table) => Block::Table(table),
                    None => self.opaque(element),
                },
                _ => self.opaque(element),
            })
            .collect()
    }

    /// Keeps an element as it is, along with whatever text can be found inside it to show.
    fn opaque(&mut self, element: &Element) -> Block {
        let mut text = Vec::new();
        self.paragraph_texts(element, &mut text);
        Block::Opaque(Opaque {
            raw: element.raw(self.source).into(),
            text,
        })
    }

    fn paragraph_texts(&mut self, element: &Element, out: &mut Vec<String>) {
        for child in &element.children {
            if child.is("w:p") {
                out.push(self.paragraph(child).text());
            } else {
                self.paragraph_texts(child, out);
            }
        }
    }

    fn paragraph(&mut self, element: &Element) -> Paragraph {
        let mut paragraph = Paragraph {
            source: Some(element.start..element.end),
            dirty: false,
            open_tag: element.opening(self.source).into(),
            props: Props::default(),
            inlines: Vec::new(),
        };
        if let Some(properties) = element.child("w:pPr") {
            paragraph.props = self.props(properties);
        }
        let wraps = unwrapped();
        self.inlines(&element.children, &wraps, &mut paragraph.inlines);
        paragraph
    }

    fn props(&self, element: &Element) -> Props {
        let mut props = Props::default();
        for child in &element.children {
            let name = child.name.strip_prefix("w:").unwrap_or(&child.name);
            props.push(name, child.raw(self.source));
        }
        props
    }

    fn inlines(&mut self, elements: &[Element], wraps: &Wraps, out: &mut Vec<Inline>) {
        for element in elements {
            let name = element.name.as_str();
            if name == "w:pPr" {
                continue;
            }
            if name == "w:r" {
                self.run(element, wraps, out);
            } else if WRAPPERS.contains(&name) {
                self.next_wrapper += 1;
                let link = (name == "w:hyperlink")
                    .then(|| self.link(element))
                    .flatten();
                let mut inner: Vec<Wrapper> = wraps.to_vec();
                inner.push(Wrapper {
                    id: self.next_wrapper,
                    open_tag: element.opening(self.source).into(),
                    name: name.into(),
                    link,
                });
                let inner: Wraps = inner.into();
                let before = out.len();
                self.inlines(&element.children, &inner, out);
                // A wrapper with nothing in it would vanish on the way out; keep it whole.
                if out.len() == before {
                    out.push(Inline::Marker(Marker {
                        raw: element.raw(self.source).into(),
                        run: None,
                        wraps: wraps.clone(),
                    }));
                }
            } else if MARKERS.contains(&name) {
                out.push(Inline::Marker(Marker {
                    raw: element.raw(self.source).into(),
                    run: None,
                    wraps: wraps.clone(),
                }));
            } else {
                out.push(Inline::Atom(Atom {
                    kind: AtomKind::Unknown,
                    raw: element.raw(self.source).into(),
                    props: Props::default(),
                    wraps: wraps.clone(),
                    bare: true,
                }));
            }
        }
    }

    /// Where a hyperlink leads: an address outside the document, or a bookmark inside it.
    fn link(&self, element: &Element) -> Option<Arc<str>> {
        if let Some(id) = element.attr(self.source, "r:id") {
            return self.package.target(&id).map(Arc::from);
        }
        element
            .attr(self.source, "w:anchor")
            .map(|anchor| format!("#{anchor}").into())
    }

    /// Splits a run into its text, atoms and markers, each keeping the run's format.
    fn run(&mut self, element: &Element, wraps: &Wraps, out: &mut Vec<Inline>) {
        let props = element
            .child("w:rPr")
            .map(|properties| self.props(properties))
            .unwrap_or_default();
        for child in &element.children {
            let name = child.name.as_str();
            let raw = child.raw(self.source);
            let atom = |kind| {
                Inline::Atom(Atom {
                    kind,
                    raw: raw.into(),
                    props: props.clone(),
                    wraps: wraps.clone(),
                    bare: false,
                })
            };
            let id = || child.attr(self.source, "w:id").unwrap_or_default();
            out.push(match name {
                "w:rPr" => continue,
                "w:t" => Inline::Text(Run {
                    text: xml::unescape(child.inner(self.source)),
                    props: props.clone(),
                    wraps: wraps.clone(),
                }),
                "w:tab" | "w:ptab" => atom(AtomKind::Tab),
                "w:br" => match child.attr(self.source, "w:type").as_deref() {
                    Some("page") | Some("column") => atom(AtomKind::PageBreak),
                    _ => atom(AtomKind::LineBreak),
                },
                "w:cr" => atom(AtomKind::LineBreak),
                "w:noBreakHyphen" => Inline::Text(Run {
                    text: "\u{2011}".into(),
                    props: props.clone(),
                    wraps: wraps.clone(),
                }),
                "w:drawing" | "w:pict" | "w:object" | "mc:AlternateContent" => {
                    atom(AtomKind::Image(self.alt_text(child)))
                }
                "w:footnoteReference" => atom(AtomKind::Footnote(id())),
                "w:endnoteReference" => atom(AtomKind::Endnote(id())),
                "w:commentReference" => atom(AtomKind::Comment(id())),
                _ if RUN_MARKERS.contains(&name) => Inline::Marker(Marker {
                    raw: raw.into(),
                    run: Some(props.clone()),
                    wraps: wraps.clone(),
                }),
                _ => atom(AtomKind::Unknown),
            });
        }
    }

    /// The description given to a picture, or failing that its name.
    fn alt_text(&self, element: &Element) -> String {
        fn find<'e>(element: &'e Element, name: &str) -> Option<&'e Element> {
            element.children.iter().find_map(|child| {
                if child.is(name) {
                    Some(child)
                } else {
                    find(child, name)
                }
            })
        }
        let Some(properties) = find(element, "wp:docPr") else {
            return String::new();
        };
        properties
            .attr(self.source, "descr")
            .filter(|text| !text.trim().is_empty())
            .or_else(|| properties.attr(self.source, "name"))
            .unwrap_or_default()
    }

    /// Reads a table, or declines one laid out in a way tuidoc couldn't write back.
    fn table(&mut self, element: &Element) -> Option<Table> {
        let mut pre = String::new();
        let mut grid = Vec::new();
        let mut rows = Vec::new();
        for child in &element.children {
            match child.name.as_str() {
                "w:tr" => rows.push(self.row(child)?),
                "w:tblPr" | "w:tblGrid" | "w:tblPrEx" if rows.is_empty() => {
                    if child.is("w:tblGrid") {
                        grid = child
                            .children
                            .iter()
                            .filter(|column| column.is("w:gridCol"))
                            .map(|column| {
                                column
                                    .attr(self.source, "w:w")
                                    .and_then(|width| width.parse().ok())
                                    .unwrap_or(0)
                            })
                            .collect();
                    }
                    pre.push_str(child.raw(self.source));
                }
                _ => return None,
            }
        }
        if rows.is_empty() {
            return None;
        }
        Some(Table {
            source: Some(element.start..element.end),
            open_tag: element.opening(self.source).into(),
            pre: pre.into(),
            grid,
            rows,
        })
    }

    fn row(&mut self, element: &Element) -> Option<Row> {
        let mut pre = String::new();
        let mut cells = Vec::new();
        for child in &element.children {
            match child.name.as_str() {
                "w:tc" => cells.push(self.cell(child)?),
                "w:trPr" | "w:tblPrEx" if cells.is_empty() => pre.push_str(child.raw(self.source)),
                _ => return None,
            }
        }
        (!cells.is_empty()).then(|| Row {
            open_tag: element.opening(self.source).into(),
            pre: pre.into(),
            cells,
        })
    }

    fn cell(&mut self, element: &Element) -> Option<Cell> {
        let mut pre = String::new();
        let mut span = 1;
        let mut merge = Merge::None;
        let mut content = Vec::new();
        for child in &element.children {
            if child.is("w:tcPr") {
                if !content.is_empty() {
                    return None;
                }
                pre.push_str(child.raw(self.source));
                if let Some(columns) = child.child("w:gridSpan") {
                    span = columns
                        .attr(self.source, "w:val")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1)
                        .max(1);
                }
                if let Some(vertical) = child.child("w:vMerge") {
                    merge = match vertical.attr(self.source, "w:val").as_deref() {
                        Some("restart") => Merge::Start,
                        _ => Merge::Continue,
                    };
                }
            } else {
                content.push(child);
            }
        }
        let blocks: Vec<Block> = content
            .into_iter()
            .map(|child| match child.name.as_str() {
                "w:p" => Block::Paragraph(self.paragraph(child)),
                _ => self.opaque(child),
            })
            .collect();
        blocks
            .iter()
            .any(|block| matches!(block, Block::Paragraph(_)))
            .then(|| Cell {
                open_tag: element.opening(self.source).into(),
                pre: pre.into(),
                span,
                merge,
                blocks,
            })
    }
}
