//! Turns a story back into its part's XML, copying whatever wasn't edited byte for byte.

use crate::model::table::Table;
use crate::model::{Block, Inline, Paragraph, Props, Story, Wrapper};

use super::xml;

/// Markers that describe how the text was last laid out or spell-checked, which no longer
/// holds once the paragraph has changed.
const STALE: [&str; 2] = ["<w:proofErr", "<w:lastRenderedPageBreak"];

/// The story's part as it should now be written.
pub fn story_xml(story: &Story) -> String {
    let mut out = String::with_capacity(story.source.len() + 256);
    out.push_str(&story.source[story.head.clone()]);
    for block in &story.blocks {
        write_block(&mut out, block, &story.source);
    }
    out.push_str(&story.source[story.tail.clone()]);
    out
}

fn write_block(out: &mut String, block: &Block, source: &str) {
    match block {
        Block::Paragraph(paragraph) => match &paragraph.source {
            Some(span) if !paragraph.dirty => out.push_str(&source[span.clone()]),
            _ => write_paragraph(out, paragraph),
        },
        Block::Table(table) => match &table.source {
            Some(span) if !table.is_dirty() => out.push_str(&source[span.clone()]),
            _ => write_table(out, table, source),
        },
        Block::Opaque(opaque) => out.push_str(&opaque.raw),
    }
}

fn write_table(out: &mut String, table: &Table, source: &str) {
    out.push_str(&table.open_tag);
    out.push_str(&table.pre);
    for row in &table.rows {
        out.push_str(&row.open_tag);
        out.push_str(&row.pre);
        for cell in &row.cells {
            out.push_str(&cell.open_tag);
            out.push_str(&cell.pre);
            for block in &cell.blocks {
                write_block(out, block, source);
            }
            out.push_str("</w:tc>");
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
}

/// A run's opening, with its properties if it has any.
fn open_run(out: &mut String, props: &Props) {
    out.push_str("<w:r>");
    if !props.is_empty() {
        out.push_str("<w:rPr>");
        out.push_str(&props.xml());
        out.push_str("</w:rPr>");
    }
}

pub fn write_paragraph(out: &mut String, paragraph: &Paragraph) {
    out.push_str(&paragraph.open_tag);
    if !paragraph.props.is_empty() {
        out.push_str("<w:pPr>");
        out.push_str(&paragraph.props.xml());
        out.push_str("</w:pPr>");
    }
    let mut open: Vec<&Wrapper> = Vec::new();
    for inline in &paragraph.inlines {
        if let Inline::Marker(marker) = inline
            && STALE.iter().any(|stale| marker.raw.starts_with(stale))
        {
            continue;
        }
        let wraps = inline.wraps();
        let shared = open
            .iter()
            .zip(wraps.iter())
            .take_while(|(open, wanted)| open.id == wanted.id)
            .count();
        while open.len() > shared {
            if let Some(wrapper) = open.pop() {
                close(out, wrapper);
            }
        }
        for wrapper in &wraps[shared..] {
            out.push_str(&wrapper.open_tag);
            open.push(wrapper);
        }
        match inline {
            Inline::Text(run) => {
                open_run(out, &run.props);
                out.push_str("<w:t xml:space=\"preserve\">");
                out.push_str(&xml::escape(&run.text));
                out.push_str("</w:t></w:r>");
            }
            Inline::Atom(atom) if atom.bare => out.push_str(&atom.raw),
            Inline::Atom(atom) => {
                open_run(out, &atom.props);
                out.push_str(&atom.raw);
                out.push_str("</w:r>");
            }
            Inline::Marker(marker) => match &marker.run {
                Some(props) => {
                    open_run(out, props);
                    out.push_str(&marker.raw);
                    out.push_str("</w:r>");
                }
                None => out.push_str(&marker.raw),
            },
        }
    }
    while let Some(wrapper) = open.pop() {
        close(out, wrapper);
    }
    out.push_str("</w:p>");
}

fn close(out: &mut String, wrapper: &Wrapper) {
    out.push_str("</");
    out.push_str(&wrapper.name);
    out.push('>');
}
