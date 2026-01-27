use crate::io;

#[derive(Debug)]
pub enum Error {
    NotFound,
    IoFailure,
    Unknown,
}

pub type Result<T> = core::result::Result<T, Error>;

pub trait Filesystem<File: io::Read + io::Stream> {
    fn open(&mut self, path: &str) -> Result<File>;
    fn exists(&mut self, path: &str) -> Result<bool>;
    fn create_dir_all(&mut self, path: &str) -> Result<()>;
    // fn size
}