use crate::zip::ZipError;

use embedded_xml as xml;

#[derive(Debug)]
pub enum RequiredFileTypes {
    Container,
    ContentOpf,
}

#[derive(Debug)]
pub enum EpubError {
    ZipError(ZipError),
    XmlError(xml::Error),
    FileMissing(RequiredFileTypes),
    InvalidData,
}

impl From<ZipError> for EpubError {
    fn from(err: ZipError) -> Self {
        EpubError::ZipError(err)
    }
}

impl From<xml::Error> for EpubError {
    fn from(err: xml::Error) -> Self {
        EpubError::XmlError(err)
    }
}
