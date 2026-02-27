/// Error type for zip entry reading operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipError {
    IoError(embedded_io::ErrorKind),
    InvalidSignature,
    UnsupportedCompression,
    DecompressionError,
    InvalidData,
}

impl ZipError {
    pub(crate) fn from_io_error(error: impl embedded_io::Error) -> Self {
        ZipError::IoError(error.kind())
    }

    pub(crate) fn from_read_exact_error<E: embedded_io::Error>(error: embedded_io::ReadExactError<E>) -> Self {
        match error {
            embedded_io::ReadExactError::UnexpectedEof => ZipError::InvalidData,
            embedded_io::ReadExactError::Other(e) => ZipError::from_io_error(e),
        }
    }
}

impl embedded_io::Error for ZipError {
    fn kind(&self) -> embedded_io::ErrorKind {
        match self {
            ZipError::IoError(kind) => *kind,
            ZipError::InvalidSignature | ZipError::InvalidData => {
                embedded_io::ErrorKind::InvalidData
            }
            ZipError::UnsupportedCompression => embedded_io::ErrorKind::Unsupported,
            ZipError::DecompressionError => embedded_io::ErrorKind::Other,
        }
    }
}
