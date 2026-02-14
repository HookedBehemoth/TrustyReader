use super::error::EpubError;
use crate::container::xml::{self, XmlEvent, XmlParser};

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use embedded_io::Read;
use log::trace;

pub struct TableOfContents {
    pub nav_map: NavMap,
}

pub fn parse(
    reader: &mut impl Read,
    size: usize,
    file_resolver: &super::FileResolver,
) -> super::Result<TableOfContents> {
    let mut parser = xml::XmlParser::new(reader, size, 1024)?;

    loop {
        let event = parser.next_event()?;
        trace!("Event: {event:?}");

        match event {
            xml::XmlEvent::StartElement if parser.name()? == "navMap" => {
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
            XmlEvent::StartElement => {
                let (name, mut attrs) = parser.name_and_attrs()?;
                match name {
                    "navPoint" => {
                        flush(
                            &mut nav_points,
                            &mut label,
                            &mut file_idx,
                            &mut anchor,
                            depth,
                        );
                        depth += 1;
                    }
                    "content" => {
                        let src = attrs.get("src").ok_or(EpubError::InvalidData)?;
                        let mut parts = src.splitn(2, '#');
                        let file_path = parts.next().ok_or(EpubError::InvalidData)?;
                        file_idx = file_resolver.content_idx(file_path);
                        let anchor_part = parts.next();
                        anchor = anchor_part.map(|s| s.to_owned());
                    }
                    "navLabel" => {
                        if parser.next_event()? != XmlEvent::StartElement
                            || parser.name()? != "text"
                        {
                            return Err(EpubError::InvalidData);
                        };
                        if parser.next_event()? != XmlEvent::Text {
                            return Err(EpubError::InvalidData);
                        }
                        label = Some(parser.block()?.to_owned());
                        if parser.next_event()? != XmlEvent::EndElement || parser.name()? != "text"
                        {
                            return Err(EpubError::InvalidData);
                        }
                        if parser.next_event()? != XmlEvent::EndElement
                            || parser.name()? != "navLabel"
                        {
                            return Err(EpubError::InvalidData);
                        }
                    }
                    _ => {}
                }
            }
            XmlEvent::EndElement => match parser.name()? {
                "navPoint" => {
                    flush(
                        &mut nav_points,
                        &mut label,
                        &mut file_idx,
                        &mut anchor,
                        depth,
                    );
                    depth -= 1;
                }
                "navMap" => break,
                _ => {}
            },
            XmlEvent::EndOfFile => break,
            _ => {}
        }
    }

    Ok(NavMap { nav_points })
}
