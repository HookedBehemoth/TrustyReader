use super::error::EpubError;
use crate::container::xml::{self, XmlEvent, XmlParser};

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use embedded_io::Read;
use log::{info, trace};

pub struct TableOfContents {
    pub nav_map: NavMap,
}

pub fn parse(
    reader: &mut impl Read,
    size: usize,
    file_resolver: &super::FileResolver,
) -> super::Result<TableOfContents> {
    info!("Parsing NCX file");
    let mut parser = xml::XmlParser::new(reader, size, 1024)?;

    loop {
        let event = parser.next_event()?;
        trace!("Event: {event:?}");

        match event {
            xml::XmlEvent::StartElement { name: "navMap", .. } => {
                let nav_map = parse_nav_map(&mut parser, file_resolver)?;
                return Ok(TableOfContents { nav_map });
            }
            xml::XmlEvent::EndOfFile => break,
            _ => {}
        }
    }

    Err(EpubError::InvalidData)
}

pub struct NavPoint {
    pub label: String,
    pub file_idx: u16,
    pub anchor: Option<String>,
    pub depth: u16,
}

pub struct NavMap {
    pub nav_points: Vec<NavPoint>,
}

fn parse_nav_map<R: Read>(
    parser: &mut XmlParser<R>,
    file_resolver: &super::FileResolver,
) -> super::Result<NavMap> {
    let mut nav_points = Vec::new();
    let mut label = None;
    let mut file_idx = None;
    let mut anchor = None;
    let mut depth = 0;

    fn flush(
        points: &mut Vec<NavPoint>,
        label: &mut Option<String>,
        file_idx: &mut Option<u16>,
        anchor: &mut Option<String>,
        depth: u16,
    ) {
        if let (Some(label), Some(file_idx)) = (label.take(), file_idx.take()) {
            points.push(NavPoint {
                label,
                file_idx,
                anchor: anchor.take(),
                depth,
            });
        }
    }

    loop {
        let event = parser.next_event()?;

        match event {
            XmlEvent::StartElement { name: "navPoint", .. } => {
                flush(
                    &mut nav_points,
                    &mut label,
                    &mut file_idx,
                    &mut anchor,
                    depth,
                );
                depth += 1;
            }
            XmlEvent::StartElement { name: "content", mut attrs } => {
                let src = attrs.get("src").ok_or(EpubError::InvalidData)?;
                let mut parts = src.splitn(2, '#');
                let file_path = parts.next().ok_or(EpubError::InvalidData)?;
                file_idx = file_resolver.content_idx(file_path);
                let anchor_part = parts.next();
                anchor = anchor_part.map(|s| s.to_owned());
            }
            XmlEvent::StartElement { name: "navLabel", .. } => {
                let XmlEvent::StartElement { name: "text", .. } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
                let XmlEvent::Text { content } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
                label = Some(content.to_owned());
                let XmlEvent::EndElement { name: "text" } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
                let XmlEvent::EndElement { name: "navLabel" } = parser.next_event()? else {
                    return Err(EpubError::InvalidData);
                };
            }
            XmlEvent::EndElement { name: "navPoint" } => {
                flush(
                    &mut nav_points,
                    &mut label,
                    &mut file_idx,
                    &mut anchor,
                    depth,
                );
                depth -= 1;
            }
            XmlEvent::EndElement { name: "navMap" } => break,
            XmlEvent::EndOfFile => break,
            _ => {}
        }
    }

    Ok(NavMap { nav_points })
}
