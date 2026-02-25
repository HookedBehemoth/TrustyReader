use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use log::info;

use super::{epub, markdown, plaintext, xml, css};
use crate::{fs::{self, File, Filesystem}, layout};

enum BookFormat {
    PlainText(String, String),
    Markdown(String, String),
    Xml(String, String),
    Html(String, String, Option<css::Stylesheet>),
    Xhtml(String, String, Option<css::Stylesheet>),
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
    pub runs: Vec<layout::Run>,
    pub alignment: Option<layout::Alignment>,
    pub indent: Option<u16>,
}

impl Book {
    pub fn from_file(file_path: &str, filesystem: &impl Filesystem, file: &mut impl File) -> Option<Self> {
        info!("Loading book from file: {}", file_path);
        let (name, ext) = file_path.rsplit_once('.').unwrap_or((file_path, ""));
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
                let css_path = alloc::format!("{}.css", name);
                let stylesheet = filesystem.open_file(&css_path, fs::Mode::Read).ok().and_then(|mut css_file| {
                    let css_contents = css_file.read_to_end().ok()?;
                    let css_text = String::from_utf8(css_contents).ok()?;
                    let mut stylesheet = css::Stylesheet::new();
                    stylesheet.extend_from_sheet(&css_text);
                    Some(stylesheet)
                });
                BookFormat::Html(name.to_string(), text, stylesheet)
            }
            "xhtml" => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;

                let css_path = alloc::format!("{}.css", name);
                let stylesheet = filesystem.open_file(&css_path, fs::Mode::Read).ok().and_then(|mut css_file| {
                    let css_contents = css_file.read_to_end().ok()?;
                    let css_text = String::from_utf8(css_contents).ok()?;
                    let mut stylesheet = css::Stylesheet::new();
                    stylesheet.extend_from_sheet(&css_text);
                    Some(stylesheet)
                });
                BookFormat::Xhtml(name.to_string(), text, stylesheet)
            }
            _ => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;
                BookFormat::PlainText(name.to_string(), text)
            }
        };

        Some(Book { format })
    }

    pub fn title(&self) -> &str {
        match &self.format {
            BookFormat::PlainText(title, _) => title,
            BookFormat::Markdown(title, _) => title,
            BookFormat::Xhtml(title, _, _) => title,
            BookFormat::Html(title, _, _) => title,
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
            BookFormat::PlainText(_, text) => Some(plaintext::from_str(text)),
            BookFormat::Markdown(_, text) => Some(markdown::from_str(text)),
            BookFormat::Html(_, text, stylesheet) => Chapter::from_html(text, stylesheet.as_ref()),
            BookFormat::Xml(_, text) => xml::from_str(text),
            BookFormat::Xhtml(_, text, stylesheet) => {
                epub::spine::parse(None, text.as_bytes(), size, stylesheet.as_ref()).ok()
            }
            BookFormat::Epub(epub) => epub::parse_chapter(epub, index, file).ok(),
        }
    }

    pub fn language(&self) -> Option<hypher::Lang> {
        match &self.format {
            BookFormat::Epub(epub) => epub.metadata.language,
            _ => None,
        }
    }

    pub fn directory_name(&self) -> String {
        let title = match &self.format {
            BookFormat::Epub(epub) => {
                if let Some(author) = &epub.metadata.author {
                    return alloc::format!(
                        "{} - {}",
                        author.replace(|c: char| UNSAFE_CHARS.contains(&c), "_"),
                        epub.metadata.title.replace(|c: char| UNSAFE_CHARS.contains(&c), "_"))
                }
                epub.metadata.title.as_str()
            },
            BookFormat::PlainText(title, _) => title,
            BookFormat::Markdown(title, _) => title,
            BookFormat::Xhtml(title, _, _) => title,
            BookFormat::Html(title, _, _) => title,
            BookFormat::Xml(title, _) => title,
        };

        title.replace(|c: char| UNSAFE_CHARS.contains(&c), "_")
    }
}

const UNSAFE_CHARS: &[char] = &['/', '\\', '?', '%', '*', ':', '|', '"', '<', '>'];

impl Chapter {
    fn from_html(text: &str, stylesheet: Option<&css::Stylesheet>) -> Option<Self> {
        if text.contains("<?xml") {
            epub::spine::parse(None, text.as_bytes(), text.len(), stylesheet).ok()
        } else {
            Some(plaintext::from_str(text))
        }
    }
}
