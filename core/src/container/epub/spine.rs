use alloc::{string::String, vec::Vec};
use log::trace;

use crate::container::book::{Chapter, Paragraph};
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
    let mut paragraphs = alloc::vec![];

    let mut current_run = String::new();
    loop {
        let event = reader.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::StartElement {
                name: "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p",
                ..
            } => {
                if current_run.len() > 0 {
                    let text = core::mem::take(&mut current_run);
                    paragraphs.push(Paragraph {
                        text,
                    });
                }
            },
            xml::Event::EndElement {
                name: "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p",
            } => {
                if current_run.len() > 0 {
                    let text = core::mem::take(&mut current_run);
                    paragraphs.push(Paragraph {
                        text,
                    });
                }
            },
            xml::Event::Text { content } => {
                current_run.push_str(content);
            }
            xml::Event::EndElement { name: "body" } => break,
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }
    paragraphs.push(Paragraph {
        text: core::mem::take(&mut current_run),
    });

    Ok(paragraphs)
}
