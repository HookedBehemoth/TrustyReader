use alloc::{
    borrow::ToOwned,
    boxed::Box,
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use log::info;

use crate::{
    container::{
        epub::{
            Epub, FileResolver,
            error::{EpubError, RequiredFileTypes},
        },
        xml::{XmlEvent, XmlParser},
    },
    fs::File,
    zip::ZipEntryReader,
};

use super::Result;

/// This is not necessarily complete, but it covers all the
/// file types we want to support.
enum MediaType {
    Image,
    Xhtml,
    Css,
    Ncx,
}

impl TryFrom<&str> for MediaType {
    type Error = ();

    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "image/jpeg" | "image/png" | "image/gif" => Ok(MediaType::Image),
            "application/xhtml+xml" => Ok(MediaType::Xhtml),
            "text/css" => Ok(MediaType::Css),
            "application/x-dtbncx+xml" => Ok(MediaType::Ncx),
            _ => Err(()),
        }
    }
}

struct ManifestItem {
    // id: String,
    media_type: MediaType,
    file_idx: u16,
}

pub struct SpineItem {
    pub file_idx: u16,
}

pub struct Metadata {
    pub title: String,
    pub author: Option<String>,
    pub language: Option<hypher::Lang>,
}

pub fn parse(file: &mut impl File, file_resolver: FileResolver, rootfile: &str) -> Result<Epub> {
    let entry = file_resolver
        .file(rootfile)
        .ok_or(EpubError::FileMissing(RequiredFileTypes::ContentOpf))?;
    let reader = ZipEntryReader::new(file, entry)?;
    let mut parser = Box::new(XmlParser::<_, 4096>::new(reader, entry.size as _)?);

    let mut manifest = BTreeMap::<String, ManifestItem>::new();
    let mut spine = Vec::<SpineItem>::new();
    let mut metadata = None;

    loop {
        let event = parser.next_event()?;
        match event {
            XmlEvent::StartElement => {
                let (name, ..) = parser.name_and_attrs()?;
                match name {
                    "manifest" => manifest = parse_manifest(&mut parser, &file_resolver)?,
                    "spine" => spine = parse_spine(&mut parser, &manifest)?,
                    "metadata" => metadata = Some(parse_metadata(&mut parser)?),
                    _ => {}
                }
            }
            XmlEvent::EndOfFile => break,
            _ => {}
        }
    }

    let epub = Epub {
        file_resolver,
        spine,
        metadata: metadata.ok_or(EpubError::InvalidData)?,
    };
    Ok(epub)
}

fn parse_metadata<R: embedded_io::Read, const C: usize>(
    parser: &mut XmlParser<R, C>,
) -> Result<Metadata> {
    info!("Parsing metadata");

    let mut title = None;
    let mut author = None;
    let mut language = None;
    loop {
        match parser.next_event()? {
            XmlEvent::StartElement => {
                let name = parser.name()?;
                match name {
                    "dc:title" => {
                        if XmlEvent::Text != parser.next_event()? {
                            return Err(EpubError::InvalidData);
                        }
                        title = parser.block().map(|s| s.to_string()).ok();
                    }
                    "dc:creator" => {
                        if XmlEvent::Text != parser.next_event()? {
                            return Err(EpubError::InvalidData);
                        }
                        author = parser.block().map(|s| s.to_string()).ok();
                    }
                    "dc:language" => {
                        if XmlEvent::Text != parser.next_event()? {
                            return Err(EpubError::InvalidData);
                        }
                        let code = parser.block()?;
                        let Ok(code) = code.as_bytes()[..].try_into() else {
                            continue;
                        };
                        language = hypher::Lang::from_iso(code);
                    }
                    _ => {}
                }
            }
            XmlEvent::EndElement => {
                if parser.name()? == "metadata" {
                    break;
                }
            }
            XmlEvent::EndOfFile => return Err(EpubError::InvalidData),
            _ => {}
        }
    }

    Ok(Metadata {
        title: title.ok_or(EpubError::InvalidData)?,
        author,
        language,
    })
}

fn parse_manifest<R: embedded_io::Read, const C: usize>(
    parser: &mut XmlParser<R, C>,
    file_resolver: &FileResolver,
) -> Result<BTreeMap<String, ManifestItem>> {
    info!("Parsing manifest");

    let mut manifest = BTreeMap::new();

    loop {
        match parser.next_event()? {
            XmlEvent::StartElement => {
                let (name, mut attrs) = parser.name_and_attrs()?;
                if name != "item" {
                    continue;
                }
                let mut id = None;
                let mut file_idx = None;
                let mut media_type = None;
                loop {
                    match attrs.next_attr() {
                        Some(("href", value)) => {
                            file_idx = file_resolver.content_idx(value);
                        }
                        Some(("id", value)) => {
                            id = Some(value.to_owned());
                        }
                        Some(("media-type", value)) => {
                            media_type = MediaType::try_from(value).ok();
                        }
                        Some(_) => continue,
                        None => break,
                    }
                }
                if let (Some(id), Some(file_idx), Some(media_type)) = (id, file_idx, media_type) {
                    manifest.insert(
                        id,
                        ManifestItem {
                            media_type,
                            file_idx,
                        },
                    );
                }
            }
            XmlEvent::EndElement => {
                if parser.name()? == "manifest" {
                    break;
                }
            }
            XmlEvent::EndOfFile => return Err(EpubError::InvalidData),
            _ => {}
        }
    }

    Ok(manifest)
}

fn parse_spine<R: embedded_io::Read, const C: usize>(
    parser: &mut XmlParser<R, C>,
    manifest: &BTreeMap<String, ManifestItem>,
) -> Result<Vec<SpineItem>> {
    info!("Parsing spine");

    let mut spine = Vec::new();

    loop {
        match parser.next_event()? {
            XmlEvent::StartElement => {
                let (name, mut attrs) = parser.name_and_attrs()?;
                if name != "itemref" {
                    continue;
                }

                loop {
                    match attrs.next_attr() {
                        Some(("idref", value)) => {
                            if let Some(item) = manifest.get(value) {
                                spine.push(SpineItem {
                                    file_idx: item.file_idx,
                                });
                            }
                        }
                        Some(_) => continue,
                        None => break,
                    }
                }
            }
            XmlEvent::EndElement => {
                if parser.name()? == "spine" {
                    break;
                }
            }
            XmlEvent::EndOfFile => return Err(EpubError::InvalidData),
            _ => {}
        }
    }

    Ok(spine)
}
