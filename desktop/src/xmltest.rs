use std::{collections::HashMap, env::args, path::PathBuf};

use log::{info, trace};
use trusty_core::{container::xml, fs::Filesystem, zip};

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
    for entry in entries {
        let xml_names = &[".opf", ".ncx", ".xml", ".xhtml", ".html"];
        info!("Entry: {}", entry.name);
        if !xml_names.iter().any(|ext| entry.name.ends_with(ext)) {
            continue;
        }
        info!("Found XML file: {}", entry.name);
        let mut zip_entry = zip::ZipEntryReader::new(&mut file, &entry).unwrap();
        let mut parser = xml::XmlParser::<_, 4096>::new(&mut zip_entry, entry.size as _).unwrap();
        let mut counts = HashMap::new();
        let mut stack = Vec::new();
        loop {
            let event = parser.next_event().unwrap();
            trace!("Event: {event:?}");

            match event {
                xml::XmlEvent::StartElement => {
                    let name = parser.name().unwrap();
                    *counts.entry(name.to_owned()).or_insert(0) += 1;
                    stack.push(name.to_owned());
                }
                xml::XmlEvent::EndElement => {
                    let name = parser.name().unwrap();
                    let prev = stack.pop().unwrap();
                    assert_eq!(name, prev);
                }
                xml::XmlEvent::EndOfFile => break,
                _ => {}
            }
        }
        for name in &stack {
            info!("Unclosed element: {name}");
        }
        assert!(stack.is_empty(), "Unclosed elements remain");

        info!("Element counts: {counts:?}");
    }

    info!("Finished parsing XML files");
}
