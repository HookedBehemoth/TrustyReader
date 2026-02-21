use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::Write;
use log::info;

use crate::{container::epub, fs::File};
use embedded_xml as xml;

enum BookFormat {
    PlainText(String, String),
    Markdown(String, String),
    Xml(String, String),
    Html(String, String),
    Xhtml(String, String),
    Epub(epub::Epub),
}

pub struct Book {
    format: BookFormat,
}

pub struct Chapter {
    pub title: Option<String>,
    // TODO: we'd need a custom file format if we want to allow arbitrary seeking
    // Keep it like this for now? We have roughly 200KB free rn and an extra 48kB
    // if we reuse the framebuffer here.
    pub paragraphs: Vec<Paragraph>,
}

pub struct Paragraph {
    pub text: String,
}

impl Book {
    pub fn from_file(file_name: &str, file: &mut impl File) -> Option<Self> {
        info!("Loading book from file: {}", file_name);
        let (name, ext) = file_name.rsplit_once('.').unwrap_or((file_name, ""));
        let format = match ext.to_ascii_lowercase().as_str() {
            "md" => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::Markdown(name.to_string(), text)
            }
            "epub" => {
                let epub = epub::parse(file).ok()?;
                BookFormat::Epub(epub)
            }
            "xml" => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::Xml(name.to_string(), text)
            }
            "html" => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::Html(name.to_string(), text)
            }
            "xhtml" => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::Xhtml(name.to_string(), text)
            }
            _ => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::PlainText(name.to_string(), text)
            } // _ => return None,
        };

        Some(Book { format })
    }

    pub fn title(&self) -> &str {
        match &self.format {
            BookFormat::PlainText(title, _) => title,
            BookFormat::Markdown(title, _) => title,
            BookFormat::Xhtml(title, _) => title,
            BookFormat::Html(title, _) => title,
            BookFormat::Xml(title, _) => title,
            BookFormat::Epub(epub) => &epub.metadata.title,
        }
    }

    pub fn chapter_count(&self) -> usize {
        match &self.format {
            BookFormat::Epub(epub) => epub.spine.len(),
            _ => 1,
        }
    }

    pub fn chapter(&self, index: usize, file: &mut impl File) -> Option<Chapter> {
        let size = file.size();
        match &self.format {
            BookFormat::PlainText(_, text) => Some(Chapter::from_plaintext(text)),
            BookFormat::Markdown(_, text) => Some(Chapter::from_plaintext(text)),
            BookFormat::Html(_, text) => Chapter::from_html(text),
            BookFormat::Xml(_, text) => Chapter::from_xml(text),
            BookFormat::Xhtml(_, text) => epub::spine::parse(None, text.as_bytes(), size).ok(),
            BookFormat::Epub(epub) => epub::parse_chapter(epub, index, file).ok(),
        }
    }

    pub fn language(&self) -> Option<hypher::Lang> {
        match &self.format {
            BookFormat::Epub(epub) => epub.metadata.language,
            _ => None,
        }
    }
}

impl Chapter {
    fn from_plaintext(text: &str) -> Self {
        let paragraphs = text
            .split("\n\n")
            .map(|p| Paragraph {
                text: p.to_string(),
            })
            .collect();
        Chapter { title: None, paragraphs }
    }
    
    fn from_xml(text: &str) -> Option<Self> {
        let mut reader = xml::Reader::new(text.as_bytes(), text.len() as _, 8096).ok()?;

        let mut depth = 0;

        let mut paragraphs = Vec::new();
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
            write!(text, "{event:?}\n\n").unwrap();
            paragraphs.push(Paragraph { text });
        }

        Some(Chapter { title: None, paragraphs })
    }

    fn from_html(text: &str) -> Option<Self> {
        if text.contains("<?xml") {
            epub::spine::parse(None, text.as_bytes(), text.len()).ok()
        } else {
            Some(Self::from_plaintext(text))
        }
    }
}
