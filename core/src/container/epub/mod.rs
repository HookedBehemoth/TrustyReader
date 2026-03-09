use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use log::{info, trace};

use crate::{container::{css, image}, fs::File, zip::{self, ZipEntryReader}};
use super::book;

pub mod container;
pub mod error;
pub mod ncx;
pub mod opf;
pub mod spine;

type Result<T> = core::result::Result<T, error::EpubError>;

pub struct FileResolver {
    entries: Box<[zip::ZipFileEntry]>,
    root: String,
}

impl FileResolver {
    pub fn content_idx(&self, path: &str) -> Option<u16> {
        let full_path: PathBuf = heapless::format!("{}{}", self.root, path).ok()?;
        self.file_idx(&full_path)
    }
    pub fn file_idx(&self, path: &str) -> Option<u16> {
        let idx = self.entries.iter().position(|e| e.name == path)?;
        Some(idx as u16)
    }
    pub fn content(&self, path: &str) -> Option<&zip::ZipFileEntry> {
        let full_path: PathBuf = heapless::format!("{}{}", self.root, path).ok()?;
        self.file(&full_path)
    }
    pub fn file(&self, path: &str) -> Option<&zip::ZipFileEntry> {
        self.entries.iter().find(|e| e.name == path)
    }
    pub fn entry(&self, idx: u16) -> Option<&zip::ZipFileEntry> {
        self.entries.get(idx as usize)
    }
}

pub struct Epub {
    pub file_resolver: FileResolver,
    pub spine: Vec<opf::SpineItem>,
    pub metadata: opf::Metadata,
    pub toc: Option<ncx::TableOfContents>,
    pub cover: Option<u16>,
    pub stylesheet: css::Stylesheet,
}

type PathBuf = heapless::String<256>;

pub fn parse(file: &mut impl File) -> Result<Epub> {
    let entries = zip::parse_zip(file)?;
    info!("Parsed ZIP with {} entries", entries.len());
    let rootfile = container::parse(file, &entries)?;
    info!("Located rootfile: {}", rootfile);
    let root = match rootfile.rfind('/') {
        Some(pos) => &rootfile[..=pos],
        None => "",
    }
    .to_owned();

    let file_resolver = FileResolver { entries, root };

    let epub = opf::parse(file, file_resolver, &rootfile)?;

    Ok(epub)
}

pub fn parse_chapter(epub: &Epub, index: usize, file: &mut impl File) -> Result<super::book::Chapter> {
    info!("Loading chapter {} from EPUB", index);
    let chapter = epub.spine.get(index).ok_or(error::EpubError::InvalidData)?;
    trace!("Chapter file index: {}", chapter.file_idx);
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
    let entry = epub.file_resolver.entry(chapter.file_idx).unwrap();
    info!("Chapter file entry: {}", entry.name);
    let reader = ZipEntryReader::new(file, entry)?;

    let folder = if let Some(pos) = entry.name.rfind('/') {
        &entry.name[..=pos]
    } else {
        ""
    };
    let resolver = spine::SpineFileResolver { folder, file_resolver: &epub.file_resolver };
    let mut chapter = spine::parse(title, reader, entry.size as usize, Some(&epub.stylesheet), Some(resolver))?;

    // Resolve image sizes now that the XHTML reader has released the file
    for para in &mut chapter.paragraphs {
        if let book::Paragraph::Image { key, width, height } = para {
            info!("Sizing image with key {} in chapter", key);
            if let Ok(size) = read_image_size(epub, *key, file) {
                *width = size.0;
                *height = size.1;
            }
        }
    }

    Ok(chapter)
}

pub fn read_image_size(epub: &Epub, key: u16, file: &mut impl File) -> Result<(u16, u16)> {
    let entry = epub.file_resolver.entry(key).ok_or(error::EpubError::InvalidState)?;
    let mut reader = ZipEntryReader::new(file, entry)?;
    image::read_size(&mut reader, entry.size)
        .map_err(|_| error::EpubError::InvalidData)
}

pub fn parse_image(epub: &Epub, key: u16, max: (u16, u16), file: &mut impl File) -> Result<image::Image> {
    info!("Loading image with key {} from EPUB", key);
    let entry = epub.file_resolver.entry(key).ok_or(error::EpubError::InvalidState)?;
    info!("Image file entry: {}", entry.name);
    // TODO: use mime type from manifest, maybe Vec<Option<Format>>
    let format = image::Format::guess_from_filename(&entry.name).ok_or(error::EpubError::InvalidFormat)?;
    let mut reader = ZipEntryReader::new(file, entry)?;
    let img = image::decode(format, &mut reader, entry.size, max.0, max.1)
        .unwrap();
    Ok(img)
}
