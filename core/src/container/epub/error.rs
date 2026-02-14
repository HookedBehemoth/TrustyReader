use crate::{container::xml::XmlError, zip::ZipError};

#[derive(Debug)]
pub enum RequiredFileTypes {
    Container,
    ContentOpf,
}

#[derive(Debug)]
pub enum EpubError {
    ZipError(ZipError),
    XmlError(XmlError),
    FileMissing(RequiredFileTypes),
    InvalidData,
}

impl From<ZipError> for EpubError {
    fn from(err: ZipError) -> Self {
        EpubError::ZipError(err)
    }
}

impl From<XmlError> for EpubError {
    fn from(err: XmlError) -> Self {
        EpubError::XmlError(err)
    }
}
