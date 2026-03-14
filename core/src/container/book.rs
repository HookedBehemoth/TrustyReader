use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use embedded_io::Write;
use log::info;
use zerocopy::{FromBytes, IntoBytes};

use super::{css, epub, markdown, plaintext, xml};
use crate::{
    container::image,
    fs::{self, File},
    layout,
};

enum BookFormat {
    PlainText(String, String),
    Markdown(String, String),
    Xml(String, String),
    Html(String, String, Option<css::Stylesheet>),
    Xhtml(String, String, Option<css::Stylesheet>),
    Epub(epub::Epub),
}

pub struct Book<Filesystem: fs::Filesystem> {
    filesystem: Filesystem,
    cache_directory: String,
    format: BookFormat,
}

#[derive(zerocopy::Immutable, zerocopy::FromBytes, zerocopy::IntoBytes)]
pub struct Progress {
    pub chapter: u16,
    pub paragraph: u16,
    pub line: u16,
}

pub struct Chapter {
    pub title: Option<String>,
    // TODO: we'd need a custom file format if we want to allow arbitrary seeking
    // Keep it like this for now? We have roughly 200KB free rn and an extra 48kB
    // if we reuse the framebuffer here.
    pub paragraphs: Vec<Paragraph>,
}

pub struct Text {
    pub runs: Vec<layout::Run>,
    pub alignment: Option<layout::Alignment>,
    pub indent: Option<u16>,
}

pub enum Paragraph {
    Text(Text),
    Image { key: u16, width: u16, height: u16 },
    Hr,
}

const BASE_PATH: &str = ".trusty";

impl<Filesystem: fs::Filesystem> Book<Filesystem> {
    pub fn from_file(
        file_path: &str,
        filesystem: Filesystem,
        file: &mut impl File,
    ) -> Option<Self> {
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
                let stylesheet = filesystem
                    .open_file(&css_path, fs::Mode::Read)
                    .ok()
                    .and_then(|mut css_file| {
                        let css_contents = css_file.read_to_end().ok()?;
                        let css_text = String::from_utf8(css_contents).ok()?;
                        let mut stylesheet = css::Stylesheet::default();
                        stylesheet.extend_from_sheet(&css_text);
                        Some(stylesheet)
                    });
                BookFormat::Html(name.to_string(), text, stylesheet)
            }
            "xhtml" => {
                let contents = file.read_to_end().ok()?;
                let text = String::from_utf8(contents).ok()?;

                let css_path = alloc::format!("{}.css", name);
                let stylesheet = filesystem
                    .open_file(&css_path, fs::Mode::Read)
                    .ok()
                    .and_then(|mut css_file| {
                        let css_contents = css_file.read_to_end().ok()?;
                        let css_text = String::from_utf8(css_contents).ok()?;
                        let mut stylesheet = css::Stylesheet::default();
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

        let cache_directory = format.cache_path();
        filesystem.create_dir_all(&cache_directory).ok();

        Some(Book {
            filesystem,
            cache_directory,
            format,
        })
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
                epub::spine::parse(None, text.as_bytes(), size, stylesheet.as_ref(), None).ok()
            }
            BookFormat::Epub(epub) => epub::parse_chapter(epub, index, file).ok(),
        }
    }

    pub fn image(&self, key: u16, max: (u16, u16), file: &mut impl File) -> Option<image::DecodedImage> {
        let image = match &self.format {
            BookFormat::Epub(epub) => epub::parse_image(epub, key, max, file).ok(),
            _ => None,
        };
        
        image
    }

    pub fn language(&self) -> Option<hypher::Lang> {
        match &self.format {
            BookFormat::Epub(epub) => epub.metadata.language,
            _ => None,
        }
    }

    fn open_cache_file(&self, name: &str, mode: crate::fs::Mode) -> Option<Filesystem::File> {
        let path = alloc::format!("{}/{}", self.cache_directory, name);
        self.filesystem.open_file(&path, mode).ok()
    }

    pub fn store_progress(&self, progress: Progress) -> Option<()> {
        let mut file = self.open_cache_file("progress.pod", crate::fs::Mode::Write)?;
        let bytes = progress.as_bytes();
        file.write(bytes).ok()?;
        Some(())
    }

    pub fn load_progress(&self) -> Progress {
        self.open_cache_file("progress.pod", crate::fs::Mode::Read)
            .and_then(|mut file| file.read_to_end().ok())
            .and_then(|contents| Progress::read_from_bytes(&contents).ok())
            .unwrap_or(Progress {
                chapter: 0,
                paragraph: 0,
                line: 0,
            })
    }
}

impl BookFormat {
    fn cache_path(&self) -> String {
        let title = match self {
            BookFormat::Epub(epub) => {
                if let Some(author) = &epub.metadata.author {
                    return alloc::format!(
                        "{BASE_PATH}/cache/{} - {}",
                        author.replace(|c: char| UNSAFE_CHARS.contains(&c), "_"),
                        epub.metadata
                            .title
                            .replace(|c: char| UNSAFE_CHARS.contains(&c), "_")
                    );
                }
                epub.metadata.title.as_str()
            }
            BookFormat::PlainText(title, _) => title,
            BookFormat::Markdown(title, _) => title,
            BookFormat::Xhtml(title, _, _) => title,
            BookFormat::Html(title, _, _) => title,
            BookFormat::Xml(title, _) => title,
        };

        alloc::format!(
            "{BASE_PATH}/cache/{}",
            title.replace(|c: char| UNSAFE_CHARS.contains(&c), "_")
        )
    }
}

const UNSAFE_CHARS: &[char] = &['/', '\\', '?', '%', '*', ':', '|', '"', '<', '>'];

impl Chapter {
    fn from_html(text: &str, stylesheet: Option<&css::Stylesheet>) -> Option<Self> {
        if text.contains("<?xml") {
            epub::spine::parse(None, text.as_bytes(), text.len(), stylesheet, None).ok()
        } else {
            Some(plaintext::from_str(text))
        }
    }
}
