use crate::{container::book, layout, res::font};
use alloc::{string::String, vec::Vec};
use log::info;
use core::fmt::Write;
use embedded_xml as xml;

pub fn from_str(text: &str) -> Option<book::Chapter> {
    let mut reader = xml::Reader::new(text.as_bytes(), text.len() as _, 8096).ok()?;

    let mut depth = 0;

    let mut runs = Vec::new();
    loop {
        let event = reader.next_event().ok()?;
        match event {
            xml::Event::StartElement { .. } => depth += 1,
            xml::Event::EndElement { .. } => depth -= 1,
            xml::Event::EndOfFile => break,
            _ => {}
        }
        let mut text = String::new();
        for _ in 0..depth {
            text.push_str("-");
        }
        write!(text, "{event:?}").unwrap();
        info!("XML event: {text:?}");
        runs.push(layout::Run {
            text,
            style: font::FontStyle::Regular,
            breaking: true,
        });
    }

    let paragraph = book::Paragraph {
        runs,
        alignment: Some(layout::Alignment::Start),
        indent: Some(0),
    };
    Some(book::Chapter { title: None, paragraphs: alloc::vec![paragraph] })
}
