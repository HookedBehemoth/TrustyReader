use crate::container::xml;
use alloc::{borrow::ToOwned, boxed::Box, string::String};
use embedded_io::Read;
use log::{info, trace};

pub struct Ncx {}

pub fn parse(reader: &mut impl Read, size: usize) -> Result<Ncx, xml::XmlError> {
    let mut parser = Box::new(xml::XmlParser::<_, 1024>::new(reader, size)?);

    let mut nav_counter = 0;
    let mut stack = heapless::Vec::<String, 16>::new();
    loop {
        let event = parser.next_event()?;
        trace!("Event: {event:?}");

        match event {
            xml::XmlEvent::StartElement => {
                let name = parser.name()?;
                stack.push(name.to_owned()).unwrap();
                if name == "navPoint" {
                    nav_counter += 1;
                }
            }
            xml::XmlEvent::EndElement => {
                let name = parser.name()?;
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

    info!("Found {nav_counter} navPoints in NCX");
    Ok(Ncx {})
}
