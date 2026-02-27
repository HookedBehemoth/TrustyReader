#![no_std]

mod entry;
mod error;
mod parser;

pub use entry::{ZipEntryReader, ZipFileEntry, read_entry};
pub use error::ZipError;
pub use parser::parse_zip;
extern crate alloc;
