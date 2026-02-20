use alloc::{
    borrow::ToOwned, string::{String, ToString}, vec::Vec
};
use log::{info, trace};

use crate::{
    container::epub,
    fs::File,
    zip::ZipEntryReader,
};
use embedded_xml::{Event, Reader};

enum BookFormat {
    PlainText(String, String),
    Markdown(String, String),
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
            _ => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::PlainText(name.to_string(), text)
            }
            // _ => return None,
        };

        Some(Book { format })
    }

    pub fn title(&self) -> &str {
        match &self.format {
            BookFormat::PlainText(title, _) => title,
            BookFormat::Markdown(title, _) => title,
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
        match &self.format {
            BookFormat::PlainText(_, text) => Some(Chapter::from_plaintext(text)),
            BookFormat::Markdown(_, text) => Some(Chapter::from_plaintext(text)),
            BookFormat::Epub(epub) => Chapter::from_epub(epub, index, file),
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
            .map(|p| Paragraph { text: p.to_string() })
            .collect();
        Chapter { title: None, paragraphs }
    }
    fn from_epub(epub: &epub::Epub, index: usize, file: &mut impl File) -> Option<Self> {
        info!("Loading chapter {} from EPUB", index);
        let chapter = epub.spine.get(index)?;
        info!("Chapter file index: {}", chapter.file_idx);
        // TODO: Map Spine entries to TOC entries while parsing
        let title = if let Some(toc) = &epub.toc {
            toc.nav_map
                .nav_points
                .iter()
                .find(|entry| entry.file_idx == chapter.file_idx)
                .map(|entry| entry.label.clone())
        } else {
            None
        };
        info!("Chapter title: {:?}", title);
        let entry = epub.file_resolver.entry(chapter.file_idx)?;
        info!("Chapter file entry: {}", entry.name);
        let reader = ZipEntryReader::new(file, entry).ok()?;
        // TODO: Ensure this is XHTML here or while parsing?
        let mut parser = Reader::new(reader, entry.size as _, 8096).ok()?;

        let mut paragraphs = alloc::vec![];

        // TODO: semantic parsing
        // TODO: style sheet parsing
        loop {
            let event = parser.next_event().ok()?;
            trace!("XML event: {:?}", event);
            match event {
                Event::Text { content } => {
                    let text = content.to_owned();
                    paragraphs.push(Paragraph { text });
                }
                Event::EndOfFile => break,
                _ => {}
            }
        }

        Some(Chapter { title, paragraphs })
    }
}
