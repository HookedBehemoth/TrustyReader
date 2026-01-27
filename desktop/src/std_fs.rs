use std::io::Seek;

pub struct StdFilesystem {
    base_path: std::path::PathBuf,
}

impl StdFilesystem {
    pub fn new_with_base_path(base_path: std::path::PathBuf) -> Self {
        StdFilesystem { base_path }
    }
}

impl trusty_core::fs::Filesystem<StdFileReader> for StdFilesystem {
    fn open(&mut self, path: &str) -> trusty_core::fs::Result<StdFileReader> {
        let path = self.base_path.join(path);
        match std::fs::File::open(path) {
            Ok(file) => match StdFileReader::new(file) {
                Ok(reader) => Ok(reader),
                Err(_) => Err(trusty_core::fs::Error::IoFailure),
            },
            Err(_) => Err(trusty_core::fs::Error::NotFound),
        }
    }

    fn exists(&mut self, path: &str) -> trusty_core::fs::Result<bool> {
        let path = self.base_path.join(path);
        Ok(path.exists())
    }

    fn create_dir_all(&mut self, path: &str) -> trusty_core::fs::Result<()> {
        let path = self.base_path.join(path);
        match std::fs::create_dir_all(path) {
            Ok(()) => Ok(()),
            Err(_) => Err(trusty_core::fs::Error::Unknown),
        }
    }
}

pub struct StdFileReader {
    file: std::fs::File,
    size: usize,
}

impl StdFileReader {
    pub fn new(mut file: std::fs::File) -> std::io::Result<Self> {
        let size = file.seek(std::io::SeekFrom::End(0))? as usize;
        file.seek(std::io::SeekFrom::Start(0))?;
        Ok(StdFileReader { file, size })
    }
}

impl trusty_core::io::Stream for StdFileReader {
    fn size(&self) -> usize {
        self.size
    }
    fn seek(&mut self, pos: usize) -> core::result::Result<(), ()> {
        self.file
            .seek(std::io::SeekFrom::Start(pos as u64))
            .map_err(|_| ())?;
        Ok(())
    }
    fn skip(&mut self, len: usize) -> core::result::Result<(), ()> {
        self.file.seek_relative(len as _).map_err(|_| ())
    }
}

impl trusty_core::io::Read for StdFileReader {
    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, ()> {
        use std::io::Read;
        match self.file.read(buf) {
            Ok(len) => Ok(len),
            Err(e) => Err(()),
        }
    }
}
