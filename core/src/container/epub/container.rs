use alloc::borrow::ToOwned;
use alloc::string::String;
use log::{info, trace};

use embedded_xml as xml;
use crate::fs::File;
use zip::{ZipEntryReader, ZipFileEntry};

use super::Result;
use super::error::{EpubError, RequiredFileTypes};

const CONTAINER_PATH: &str = "META-INF/container.xml";

pub(super) fn parse(file: &mut impl File, entries: &[ZipFileEntry]) -> Result<String> {
    let entry = entries
        .iter()
        .find(|e| e.name == CONTAINER_PATH)
        .ok_or(EpubError::FileMissing(RequiredFileTypes::Container))?;

    info!("Parsing EPUB container");

    let reader = ZipEntryReader::new(file, entry)?;
    let mut parser = xml::Reader::new(reader, entry.size as _, 512)?;
    loop {
        let event = parser.next_event()?;
        trace!("Event: {event:?}");

        match event {
            xml::Event::StartElement { name: "rootfile", attrs } => {
                return attrs
                    .get("full-path")
                    .map(|s| s.to_owned())
                    .ok_or(EpubError::InvalidData);
            }
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }
    Err(EpubError::FileMissing(RequiredFileTypes::ContentOpf))
}
