use embedded_io::{Error, ErrorKind};

use crate::{container::book::Book, fs::{self, DirEntry, Directory}};

pub fn parse_all_books<FS: fs::Filesystem>(filesystem: &mut FS) -> Result<(), ErrorKind> {
    let root = filesystem.open_directory("/").map_err(|e| e.kind())?;
    let entries = root.list().map_err(|e| e.kind())?;
    
    // let timings 
    for entry in entries {
        if entry.is_directory() {
            continue;
        }
        if !entry.name().ends_with(".epub") {
            continue;
        }
        let mut file = filesystem.open_file_entry(&root, &entry, fs::Mode::Read).map_err(|e| e.kind())?;
        log::info!("Parsing book from file: {}", entry.name());
        let book = Book::from_file(entry.name(), filesystem, &mut file).unwrap();
        log::info!("Parsed book: {}", book.title());
        for i in 0..book.chapter_count() {
            log::info!("Parsing chapter {} of {}", i + 1, book.chapter_count());
            if let Some(chapter) = book.chapter(i, &mut file) {
                log::info!("Parsed chapter: {:?}", chapter.title);
            } else {
                log::error!("Failed to parse chapter {}", i + 1);
            };
        }
        log::info!("Finished parsing book: {}", book.title());
    }
    Ok(())
}
