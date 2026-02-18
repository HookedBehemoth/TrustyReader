use alloc::{
    borrow::ToOwned,
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use log::{error, info};

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
#[derive(PartialEq, Eq, Debug)]
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
    pub cover_id: Option<String>,
}

pub fn parse(file: &mut impl File, file_resolver: FileResolver, rootfile: &str) -> Result<Epub> {
    let entry = file_resolver
        .file(rootfile)
        .ok_or(EpubError::FileMissing(RequiredFileTypes::ContentOpf))?;
    let reader = ZipEntryReader::new(file, entry)?;
    let mut parser = XmlParser::new(reader, entry.size as _, 4096)?;

    let mut metadata = None;
    let mut manifest = BTreeMap::<String, ManifestItem>::new();
    let mut spine = Vec::<SpineItem>::new();
    let mut ncx_toc_entry = None;

    loop {
        let event = parser.next_event()?;
        match event {
            XmlEvent::StartElement { name: "metadata", .. } => {
                metadata = Some(parse_metadata(&mut parser)?);
            }
            XmlEvent::StartElement { name: "manifest", .. } => {
                manifest = parse_manifest(&mut parser, &file_resolver)?;
            }
            XmlEvent::StartElement { name: "spine", attrs } => {
                if let Some(entry) = attrs.get("toc").and_then(|v| manifest.get(v)) {
                    if entry.media_type == MediaType::Ncx {
                        ncx_toc_entry = Some(entry.file_idx);
                    } else {
                        log::error!("TOC entry has wrong media type: {:?}", entry.media_type);
                    }
                }
                spine = parse_spine(&mut parser, &manifest)?
            }
            XmlEvent::EndOfFile => break,
            _ => {}
        }
    }
    drop(parser);

    let cover = metadata
        .as_ref()
        .and_then(|m| m.cover_id.as_deref())
        .and_then(|cover_id| manifest.get(cover_id))
        .map(|item| item.file_idx);

    drop(manifest);

    let toc = if let Some(entry) = ncx_toc_entry {
        if let Some(entry) = file_resolver.entry(entry) {
            let mut reader = ZipEntryReader::new(file, entry)?;
            match super::ncx::parse(&mut reader, entry.size as _, &file_resolver) {
                Ok(toc) => Some(toc),
                Err(e) => {
                    info!("Failed to parse NCX: {e:?}");
                    None
                }
            }
        } else {
            info!("TOC entry not found in zip file");
            None
        }
    } else {
        info!("No NCX TOC entry found in manifest");
        None
    };

    let epub = Epub {
        file_resolver,
        spine,
        metadata: metadata.ok_or(EpubError::InvalidData)?,
        toc,
        cover,
    };
    Ok(epub)
}

fn parse_metadata<R: embedded_io::Read>(parser: &mut XmlParser<R>) -> Result<Metadata> {
    info!("Parsing metadata");

    let mut title = None;
    let mut author = None;
    let mut language = None;
    let mut cover_id = None;
    loop {
        match parser.next_event()? {
            XmlEvent::StartElement { name: "dc:title", .. } => {
                let XmlEvent::Text { content } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
                title = Some(content.to_string());
            }
            XmlEvent::StartElement { name: "dc:creator", .. } => {
                let XmlEvent::Text { content } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
                author = Some(content.to_string());
            }
            XmlEvent::StartElement { name: "dc:language", .. } => {
                let XmlEvent::Text { content } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
                let Ok(code) = content.as_bytes()[..].try_into() else {
                    continue;
                };
                language = hypher::Lang::from_iso(code);
            }
            XmlEvent::StartElement { name: "meta", attrs } => {
                if attrs.get("name") == Some("cover")
                    && let Some(content) = attrs.get("content")
                {
                    cover_id = Some(content.to_owned());
                }
            }
            XmlEvent::EndElement { name: "metadata" } => {
                break;
            }
            XmlEvent::EndOfFile => return Err(EpubError::InvalidData),
            _ => {}
        }
    }

    Ok(Metadata {
        title: title.ok_or(EpubError::InvalidData)?,
        author,
        language,
        cover_id,
    })
}

fn parse_manifest<R: embedded_io::Read>(
    parser: &mut XmlParser<R>,
    file_resolver: &FileResolver,
) -> Result<BTreeMap<String, ManifestItem>> {
    info!("Parsing manifest");

    let mut manifest = BTreeMap::new();

    loop {
        match parser.next_event()? {
            XmlEvent::StartElement { name: "item", attrs } => {
                let mut id = None;
                let mut file_idx = None;
                let mut media_type = None;
                for (name, value) in attrs {
                    match name {
                        "href" => file_idx = file_resolver.content_idx(value),
                        "id" => id = Some(value.to_owned()),
                        "media-type" => media_type = MediaType::try_from(value).ok(),
                        _ => continue,
                    }
                }
                if let (Some(id), Some(file_idx), Some(media_type)) = (id, file_idx, media_type) {
                    manifest.insert(id, ManifestItem { media_type, file_idx });
                }
            }
            XmlEvent::EndElement { name: "manifest" } => {
                break;
            }
            XmlEvent::EndOfFile => return Err(EpubError::InvalidData),
            _ => {}
        }
    }

    Ok(manifest)
}

fn parse_spine<R: embedded_io::Read>(
    parser: &mut XmlParser<R>,
    manifest: &BTreeMap<String, ManifestItem>,
) -> Result<Vec<SpineItem>> {
    info!("Parsing spine");

    let mut spine = Vec::new();

    loop {
        match parser.next_event()? {
            XmlEvent::StartElement { name: "itemref", attrs } => {
                if let Some(value) = attrs.get("idref") {
                    match manifest.get(value) {
                        Some(ManifestItem { file_idx, .. }) => {
                            spine.push(SpineItem { file_idx: *file_idx })
                        }
                        None => error!("Couldn't find idref: {} in manifest", value),
                    }
                }
            }
            XmlEvent::EndElement { name: "spine" } => {
                break;
            }
            XmlEvent::EndOfFile => return Err(EpubError::InvalidData),
            _ => {}
        }
    }

    Ok(spine)
}
