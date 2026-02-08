use std::{fs, io::Seek};

use embedded_io::{ErrorType, SeekFrom};
use log::info;
use trusty_core::fs::{DirEntry, Mode};

pub struct StdFilesystem {
    base_path: std::path::PathBuf,
}

impl StdFilesystem {
    pub fn new_with_base_path(base_path: std::path::PathBuf) -> Self {
        info!("Using StdFilesystem with base path: {:?}", base_path);
        StdFilesystem { base_path }
    }
}

impl ErrorType for StdFilesystem {
    type Error = embedded_io::ErrorKind;
}

type Result<T> = core::result::Result<T, embedded_io::ErrorKind>;

impl trusty_core::fs::Filesystem for StdFilesystem {
    type File
        = StdFileReader;
    type Directory
        = StdDirectory;

    fn open_file(&self, path: &str, mode: Mode) -> Result<StdFileReader> {
        let path = self.base_path.join(path);
        let options = match mode {
            Mode::Read => fs::OpenOptions::new().read(true).clone(),
            Mode::Write => fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .clone(),
            Mode::ReadWrite => fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .clone(),
        };
        match options.open(path) {
            Ok(file) => match StdFileReader::new(file) {
                Ok(reader) => Ok(reader),
                Err(_) => Err(embedded_io::ErrorKind::InvalidInput),
            },
            Err(_) => Err(embedded_io::ErrorKind::NotFound),
        }
    }

    fn open_directory(&self, path: &str) -> std::result::Result<Self::Directory, Self::Error> {
        info!("Opening directory at path: {}", path);
        let path = self.base_path.join(path);
        if path.exists() == false || !path.is_dir() {
            return Err(embedded_io::ErrorKind::NotFound);
        }
        Ok(StdDirectory { path })
    }

    fn open_file_entry(
        &self,
        dir: &Self::Directory,
        entry: &StdDirEntry,
        mode: Mode,
    ) -> std::result::Result<Self::File, Self::Error> {
        let path = dir.path.join(entry.name());
        let path = path.to_str().ok_or(embedded_io::ErrorKind::InvalidInput)?;
        self.open_file(path, mode)
    }

    fn exists(&self, path: &str) -> Result<bool> {
        let path = self.base_path.join(path);
        Ok(path.exists())
    }

    fn create_dir_all(&self, path: &str) -> Result<()> {
        let path = self.base_path.join(path);
        match std::fs::create_dir_all(path) {
            Ok(()) => Ok(()),
            Err(_) => Err(embedded_io::ErrorKind::AlreadyExists),
        }
    }
}

pub struct StdFileReader {
    file: std::io::BufReader<std::fs::File>,
    size: usize,
}

impl StdFileReader {
    pub fn new(mut file: std::fs::File) -> std::io::Result<Self> {
        let size = file.seek(std::io::SeekFrom::End(0))? as usize;
        file.seek(std::io::SeekFrom::Start(0))?;
        Ok(StdFileReader {
            file: std::io::BufReader::new(file),
            size,
        })
    }
}

impl trusty_core::fs::File for StdFileReader {
    fn size(&self) -> usize {
        self.size
    }
}

impl ErrorType for StdFileReader {
    type Error = std::io::Error;
}

impl embedded_io::Seek for StdFileReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos.into())
    }
}

impl embedded_io::Read for StdFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::Read;
        self.file.read(buf)
    }
}

impl embedded_io::Write for StdFileReader {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use std::io::Write;
        self.file.get_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        self.file.get_mut().flush()
    }
}

pub struct StdDirectory {
    pub path: std::path::PathBuf,
}

impl ErrorType for StdDirectory {
    type Error = embedded_io::ErrorKind;
}

impl trusty_core::fs::Directory for StdDirectory {
    type Entry = StdDirEntry;
    fn list(&self) -> Result<Vec<Self::Entry>> {
        let mut result = Vec::new();
        for entry in
            std::fs::read_dir(&self.path).map_err(|_| embedded_io::ErrorKind::InvalidInput)?
        {
            match entry {
                Ok(dir_entry) => {
                    let metadata = dir_entry
                        .metadata()
                        .map_err(|_| embedded_io::ErrorKind::InvalidInput)?;
                    let is_directory = metadata.is_dir();
                    let size = metadata.len() as usize;
                    let name = dir_entry.file_name().to_string_lossy().into_owned();
                    result.push(StdDirEntry {
                        name,
                        size,
                        is_directory,
                    });
                }
                Err(_) => return Err(embedded_io::ErrorKind::InvalidInput),
            }
        }
        Ok(result)
    }
}

pub struct StdDirEntry {
    name: String,
    size: usize,
    is_directory: bool,
}

impl trusty_core::fs::DirEntry for StdDirEntry {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_directory(&self) -> bool {
        self.is_directory
    }

    fn size(&self) -> usize {
        self.size
    }
}
