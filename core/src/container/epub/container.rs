use alloc::borrow::ToOwned;
use alloc::string::String;

use crate::container::xml::{XmlEvent, XmlParser};
use crate::zip::{ZipEntryReader, ZipFileEntry};
use crate::fs::File;

use super::Result;
use super::error::{EpubError, RequiredFileTypes};

const CONTAINER_PATH: &str = "META-INF/container.xml";

pub(super) fn parse(
    file: &mut impl File,
    entries: &[ZipFileEntry],
) -> Result<String> {
    let entry = entries
        .iter()
        .find(|e| e.name == CONTAINER_PATH)
        .ok_or(EpubError::FileMissing(RequiredFileTypes::Container))?;

    let reader = ZipEntryReader::new(file, entry)?;
    let mut parser = XmlParser::<_, 512>::new(reader, entry.size as _)?;
    loop {
        let event = parser.next_event()?;
        match event {
            XmlEvent::StartElement => {
                if parser.name()? != "rootfile" {
                    continue;
                }

                let mut attrs = parser.attr()?;
                loop {
                    let attr = attrs.next_attr();
                    if attr.is_none() {
                        break;
                    }
                    let (name, value) = attr.unwrap();
                    if name == "full-path" {
                        return Ok(value.to_owned());
                    }
                }
            }
            XmlEvent::EndOfFile => break,
            _ => {}
        }
    }
    Err(EpubError::FileMissing(RequiredFileTypes::ContentOpf))
}
