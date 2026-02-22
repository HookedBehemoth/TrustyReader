use alloc::{string::String, vec::Vec};
use log::trace;

use crate::{
    container::{
        book::{Chapter, Paragraph},
        css,
    },
    layout,
    res::font,
};
use embedded_xml as xml;

pub fn parse<R: embedded_io::Read>(
    title: Option<String>,
    reader: R,
    size: usize,
    stylesheet: Option<&css::Stylesheet>,
) -> super::Result<Chapter> {
    // TODO: Ensure this is XHTML here or while parsing?
    let mut parser = xml::Reader::new(reader, size as _, 8096)?;

    let mut paragraphs = alloc::vec![];

    loop {
        let event = parser.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::StartElement { name: "body", .. } => {
                paragraphs = parse_body(&mut parser, stylesheet)?;
                break;
            }
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }

    Ok(Chapter { title, paragraphs })
}

fn parse_body<R: embedded_io::Read>(
    reader: &mut xml::OwnedReader<R>,
    stylesheet: Option<&css::Stylesheet>,
) -> super::Result<Vec<Paragraph>> {
    let mut parser = BodyParser::new();

    fn is_block_element(name: &str) -> bool {
        matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "li")
    }
    fn is_italic(name: &str) -> bool {
        matches!(name, "i" | "em")
    }
    fn is_bold(name: &str) -> bool {
        matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "b")
    }

    loop {
        let event = reader.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::EndElement { name: "body" } => break,
            xml::Event::StartElement { name, attrs } => {
                if is_block_element(name) {
                    parser.flush_run();
                }

                parser.increase_depth();

                let class = attrs.get("class");
                let style = stylesheet
                    .and_then(|s| class.map(|c| s.get(c)))
                    .unwrap_or_default()
                    + attrs
                        .get("style")
                        .map(css::Rule::from_str)
                        .unwrap_or_default();

                if is_bold(name) {
                    parser.set_bold(true);
                } else if is_italic(name) {
                    parser.set_italic(true);
                } else if name == "br" {
                    parser.break_line();
                }

                if let Some(italic) = style.italic {
                    parser.set_italic(italic);
                    parser.italic_depth = Some(parser.depth);
                }
                if let Some(bold) = style.bold {
                    parser.set_bold(bold);
                    parser.bold_depth = Some(parser.depth);
                }
                if let Some(alignment) = style.alignment {
                    parser.alignment = Some(alignment);
                }
                if let Some(indent) = style.indent {
                    parser.indent = Some(indent);
                }
            }
            xml::Event::EndElement { name } => {
                if is_block_element(name) {
                    parser.flush_run();
                }

                if is_bold(name) {
                    parser.set_bold(false);
                } else if is_italic(name) {
                    parser.set_italic(false);
                }

                parser.decrease_depth();
            }
            xml::Event::Text { content } => {
                parser.push_text(content);
            }
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }

    Ok(parser.into_paragraphs())
}

struct BodyParser {
    paragraphs: Vec<Paragraph>,
    runs: Vec<layout::Run>,
    alignment: Option<layout::Alignment>,
    indent: Option<u16>,
    current_run: String,
    bold: bool,
    italic: bool,
    depth: u8,
    italic_depth: Option<u8>,
    bold_depth: Option<u8>,
}

impl BodyParser {
    fn new() -> Self {
        Self {
            paragraphs: Vec::new(),
            runs: Vec::new(),
            alignment: None,
            indent: None,
            current_run: String::new(),
            bold: false,
            italic: false,
            depth: 0,
            italic_depth: None,
            bold_depth: None,
        }
    }

    fn set_bold(&mut self, bold: bool) {
        if self.bold != bold {
            self.flush_text(false);
            self.bold = bold;
        }
    }

    fn set_italic(&mut self, italic: bool) {
        if self.italic != italic {
            self.flush_text(false);
            self.italic = italic;
        }
    }

    fn style(&self) -> font::FontStyle {
        match (self.bold, self.italic) {
            (false, false) => font::FontStyle::Regular,
            (true, false) => font::FontStyle::Bold,
            (false, true) => font::FontStyle::Italic,
            (true, true) => font::FontStyle::BoldItalic,
        }
    }

    fn flush_text(&mut self, breaking: bool) {
        if !self.current_run.is_empty() {
            let text = core::mem::take(&mut self.current_run);
            self.runs.push(layout::Run {
                text,
                style: self.style(),
                breaking,
            });
        }
    }

    fn break_line(&mut self) {
        self.flush_text(true);
    }

    fn flush_run(&mut self) {
        self.flush_text(false);
        if !self.runs.is_empty() {
            let runs = core::mem::take(&mut self.runs);
            self.paragraphs.push(Paragraph {
                runs,
                indent: self.indent,
                alignment: self.alignment,
            });
            self.indent = None;
            self.alignment = None;
        }
    }

    fn into_paragraphs(mut self) -> Vec<Paragraph> {
        self.flush_run();
        self.paragraphs
    }

    fn push_text(&mut self, text: &str) {
        self.current_run.push_str(text);
    }

    fn increase_depth(&mut self) {
        self.depth += 1;
    }

    fn decrease_depth(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
        if let Some(italic_depth) = self.italic_depth {
            if self.depth < italic_depth {
                self.set_italic(false);
                self.italic_depth = None;
            }
        }
        if let Some(bold_depth) = self.bold_depth {
            if self.depth < bold_depth {
                self.set_bold(false);
                self.bold_depth = None;
            }
        }
    }
}
