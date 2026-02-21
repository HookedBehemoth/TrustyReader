/*!
A no_std XML reader using embedded-io for memory constrainted environment.

## Features
- no_std
- alloc optional
- streaming

## Usage
```
# use embedded_xml as xml;
# fn main() -> Result<(), xml::Error> {
# let xml = "<?xml version=\"1.0\"?>";
# let mut reader = xml.as_bytes();
let mut reader = xml::Reader::new(&mut reader, xml.len(), 64)?;
loop {
    match reader.next_event()? {
        xml::Event::StartElement { name, attrs } => {
            println!("Start element: {name} with attributes: {attrs:?}");
        }
        xml::Event::EndElement { name } => {
            println!("End element: {name}");
        }
        xml::Event::EndOfFile => break,
        _ => {}
    }
}
# Ok(())
# }
```

## Limitations & non-goals
- UTF-8 only
- no rewinding
- no DTD support
- no XPath
- no decoding
- individual "Events" have to fit inside the internal buffer
*/

#![no_std]
// stable in 1.95
#![feature(assert_matches)]

mod reader;
mod attributes;
mod events;

#[cfg(test)]
mod tests;

#[cfg(feature = "alloc")]
extern crate alloc;

pub use events::Event;
pub use reader::Reader;
pub use attributes::AttributeReader;

#[cfg(feature = "alloc")]
pub type OwnedReader<R> = Reader<R, alloc::vec::Vec<u8>>;

#[derive(Debug)]
pub enum Error {
    IoError(embedded_io::ErrorKind),
    Utf8Error(core::str::Utf8Error),
    InvalidState,
    Eof,
}

type Result<T> = core::result::Result<T, Error>;

impl From<core::str::Utf8Error> for Error {
    fn from(err: core::str::Utf8Error) -> Self {
        Error::Utf8Error(err)
    }
}
