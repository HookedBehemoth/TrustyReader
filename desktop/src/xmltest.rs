use std::{collections::HashMap, env::args, path::PathBuf};

use log::{error, info, trace};
use trusty_core::{fs::Filesystem, zip};
use embedded_xml as xml;

use crate::std_fs::StdFilesystem;

mod std_fs;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    for path in args().skip(1) {
        test_file(&path);
    }
}

fn test_file(path: &str) {
    let fs = StdFilesystem::new_with_base_path(PathBuf::from(""));
    let mut file = fs.open_file(&path, trusty_core::fs::Mode::Read).unwrap();
    let entries = zip::parse_zip(&mut file).unwrap();
    let mut max_text_size = 0;
    for entry in entries {
        let xml_names = &[".opf", ".ncx", ".xml", ".xhtml", ".html"];
        info!("Entry: {}", entry.name);
        if !xml_names.iter().any(|ext| entry.name.ends_with(ext)) {
            continue;
        }
        info!("Found XML file: {}", entry.name);
        let mut zip_entry = zip::ZipEntryReader::new(&mut file, &entry).unwrap();
        let mut parser = xml::Reader::new(&mut zip_entry, entry.size as _, 4096).unwrap();
        let mut counts = HashMap::new();
        let mut stack = Vec::new();
        let mut text_size = 0;
        loop {
            let event = match parser.next_event() {
                Ok(event) => event,
                Err(e) => {
                    error!("Error parsing XML: {e:?}");
                    break;
                }
            };
            trace!("Event: {event:?}");

            match event {
                xml::Event::StartElement { name, .. } => {
                    *counts.entry(name.to_owned()).or_insert(0) += 1;
                    stack.push(name.to_owned());
                }
                xml::Event::EndElement { name } => {
                    let prev = stack.pop().unwrap();
                    assert_eq!(name, prev);
                }
                xml::Event::Text { content } => text_size += content.len(),
                xml::Event::EndOfFile => break,
                _ => {}
            }
        }
        for name in &stack {
            error!("Unclosed element: {name}");
        }

        if text_size > max_text_size {
            max_text_size = text_size;
        }

        info!("Element counts: {counts:?}");
    }
    info!("Max text size: {}", max_text_size);

    info!("Finished parsing XML files");
}
