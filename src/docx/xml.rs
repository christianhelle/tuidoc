//! A small element tree over a part's XML that remembers where every element sits in its source.

use quick_xml::Reader;
use quick_xml::events::Event;

use super::DocxError;

/// One element and the byte offsets of its tags, so its exact source text can be copied out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// The name as written, prefix included.
    pub name: String,
    pub start: usize,
    /// Where the opening tag ends, which is also where the content starts.
    pub open_end: usize,
    /// Where the closing tag starts; equal to `end` for an element written as empty.
    pub close_start: usize,
    pub end: usize,
    pub children: Vec<Element>,
}

impl Element {
    /// The whole element as it stands in the source.
    pub fn raw<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }

    /// The opening tag as it stands in the source, which for an empty element is all of it.
    pub fn open_tag<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.open_end]
    }

    /// An opening tag that content can follow, whether or not the element was written as empty.
    pub fn opening(&self, source: &str) -> String {
        let tag = self.open_tag(source);
        match tag.strip_suffix("/>") {
            Some(rest) => format!("{}>", rest.trim_end()),
            None => tag.to_string(),
        }
    }

    /// Everything between the tags.
    pub fn inner<'a>(&self, source: &'a str) -> &'a str {
        &source[self.open_end..self.close_start]
    }

    /// The name without its prefix.
    pub fn local(&self) -> &str {
        local_name(&self.name)
    }

    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.is(name))
    }

    /// An attribute's value. A name with a prefix must match exactly; one without matches any
    /// prefix.
    pub fn attr(&self, source: &str, name: &str) -> Option<String> {
        attr_of(self.open_tag(source), name)
    }
}

/// The part of a name after its prefix.
pub fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Reads an attribute out of an opening tag. A name with a prefix must match exactly; one without
/// matches any prefix.
pub fn attr_of(open_tag: &str, name: &str) -> Option<String> {
    let bytes = open_tag.as_bytes();
    // Skip the element's own name.
    let mut at = open_tag.find(|c: char| c.is_whitespace())?;
    loop {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let key_start = at;
        while at < bytes.len() && bytes[at] != b'=' && bytes[at] != b'>' && bytes[at] != b'/' {
            at += 1;
        }
        if at >= bytes.len() || bytes[at] != b'=' {
            return None;
        }
        let key = open_tag[key_start..at].trim();
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let quote = *bytes.get(at)?;
        if quote != b'"' && quote != b'\'' {
            return None;
        }
        let value_start = at + 1;
        let value_end = value_start + open_tag[value_start..].find(quote as char)?;
        let matches = if name.contains(':') {
            key == name
        } else {
            local_name(key) == name
        };
        if matches {
            return Some(unescape(&open_tag[value_start..value_end]));
        }
        at = value_end + 1;
    }
}

/// The `w:val` of the first element with this name inside a piece of XML, such as the list
/// number inside a paragraph's numbering properties.
pub fn val_within(xml: &str, name: &str) -> Option<String> {
    let tag = tag_within(xml, name)?;
    attr_of(tag, "val")
}

/// The opening tag of the first element with this name inside a piece of XML.
pub fn tag_within<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("<{name}");
    let mut from = 0;
    while let Some(found) = xml[from..].find(&needle) {
        let start = from + found;
        let after = xml[start + needle.len()..].chars().next()?;
        if after.is_whitespace() || after == '/' || after == '>' {
            let end = start + xml[start..].find('>')?;
            return Some(&xml[start..=end]);
        }
        from = start + 1;
    }
    None
}

pub fn xml_error(error: impl std::fmt::Display) -> DocxError {
    DocxError::Xml(error.to_string())
}

/// Builds the element tree of a part.
pub fn parse(source: &str) -> Result<Element, DocxError> {
    let mut reader = Reader::from_str(source);
    let mut open: Vec<Element> = Vec::new();
    let mut root = None;
    loop {
        let before = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(xml_error)?;
        let after = reader.buffer_position() as usize;
        let finished = match event {
            Event::Start(start) => {
                open.push(element(start.name().as_ref(), before, after));
                None
            }
            Event::Empty(start) => Some(element(start.name().as_ref(), before, after)),
            Event::End(_) => {
                let mut element = open
                    .pop()
                    .ok_or_else(|| xml_error("a closing tag with nothing open"))?;
                element.close_start = before;
                element.end = after;
                Some(element)
            }
            Event::Eof => break,
            _ => None,
        };
        if let Some(element) = finished {
            match open.last_mut() {
                Some(parent) => parent.children.push(element),
                None => root = Some(element),
            }
        }
    }
    if !open.is_empty() {
        return Err(xml_error("an element is never closed"));
    }
    root.ok_or_else(|| xml_error("no root element"))
}

fn element(name: &str, start: usize, open_end: usize) -> Element {
    Element {
        name: name.to_string(),
        start,
        open_end,
        close_start: open_end,
        end: open_end,
        children: Vec::new(),
    }
}

/// Text made safe to put between tags or inside a quoted attribute.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// The text an escaped piece of XML stands for. Entities it doesn't know are left as written.
pub fn unescape(text: &str) -> String {
    match quick_xml::escape::unescape(text) {
        Ok(text) => text.into_owned(),
        Err(_) => text.to_string(),
    }
}
