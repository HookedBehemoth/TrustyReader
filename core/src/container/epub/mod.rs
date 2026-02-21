use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use log::info;

use crate::{fs::File, zip::{self, ZipEntryReader}};

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
    let entry = epub.file_resolver.entry(chapter.file_idx).unwrap();
    info!("Chapter file entry: {}", entry.name);
    let reader = ZipEntryReader::new(file, entry)?;

    spine::parse(title, reader, entry.size as usize).map_err(Into::into)
}