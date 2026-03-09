use embedded_io::{Error, ErrorKind};

use crate::{container::book::Book, fs::{self, DirEntry, Directory}};

pub fn parse_all_books<FS: fs::Filesystem>(filesystem: &mut FS) -> Result<(), ErrorKind> {
    let root = filesystem.open_directory("/").map_err(|e| e.kind())?;
    let entries = root.list().map_err(|e| e.kind())?;
    for entry in entries {
        if entry.is_directory() {
            continue;
        }
        let mut file = filesystem.open_file_entry(&root, &entry, fs::Mode::Read).map_err(|e| e.kind())?;
        log::info!("Parsing book from file: {}", entry.name());
        let book = Book::from_file(entry.name(), filesystem, &mut file).unwrap();
        log::info!("Parsed book: {}", book.title());
        for i in 0..book.chapter_count() {
            log::info!("Parsing chapter {} of {}", i + 1, book.chapter_count());
            let chapter = book.chapter(i, &mut file).unwrap();
            log::info!("Parsed chapter: {:?}", chapter.title);
        }
        log::info!("Finished parsing book: {}", book.title());
    }
    Ok(())
}
