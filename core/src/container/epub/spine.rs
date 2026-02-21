use alloc::{string::String, vec::Vec};
use log::trace;

use crate::{
    container::book::{Chapter, Paragraph},
    layout,
    res::font,
};
use embedded_xml as xml;

pub fn parse<R: embedded_io::Read>(
    title: Option<String>,
    reader: R,
    size: usize,
) -> super::Result<Chapter> {
    // TODO: Ensure this is XHTML here or while parsing?
    let mut parser = xml::Reader::new(reader, size as _, 8096)?;

    let mut paragraphs = alloc::vec![];

    // TODO: semantic parsing
    // TODO: style sheet parsing
    loop {
        let event = parser.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::StartElement { name: "body", .. } => {
                paragraphs = parse_body(&mut parser)?;
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
) -> super::Result<Vec<Paragraph>> {
    let mut parser = BodyParser::new();

    loop {
        let event = reader.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::StartElement {
                name: "h1" | "h2" | "h3" | "h4" | "h5" | "h6",
                ..
            } => {
                parser.set_bold(true);
            }
            xml::Event::EndElement {
                name: "h1" | "h2" | "h3" | "h4" | "h5" | "h6",
                ..
            } => {
                parser.set_bold(false);
            }
            xml::Event::StartElement { name: "i", .. } => {
                parser.set_italic(true);
            }
            xml::Event::EndElement { name: "i", .. } => {
                parser.set_italic(false);
            }
            xml::Event::StartElement { name: "br", .. } => {
                parser.break_line();
            }
            xml::Event::EndElement { name: "body" } => break,
            xml::Event::EndOfFile => break,
            _ => {}
        }
        match event {
            xml::Event::Text { .. } => {}
            // xml::Event::StartElement { name, .. } => {
            //     info!("Start element: {name}");
            // }
            // xml::Event::EndElement { name, .. } => {
            //     info!("End element: {name}");
            // }
            _ => {
                trace!("{event:?}");
            }
        }
        match event {
            xml::Event::StartElement {
                name: "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p",
                ..
            } => {
                parser.flush_run();
            }
            xml::Event::EndElement {
                name: "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p",
            } => {
                parser.flush_run();
            }
            xml::Event::Text { content } => {
                parser.push_text(content);
            }
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
            self.paragraphs.push(Paragraph { runs, indent: self.indent, alignment: self.alignment });
        }
    }

    fn into_paragraphs(mut self) -> Vec<Paragraph> {
        self.flush_run();
        self.paragraphs
    }

    fn push_text(&mut self, text: &str) {
        self.current_run.push_str(text);
    }
}
