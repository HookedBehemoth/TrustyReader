use alloc::{boxed::Box, string::String, vec, vec::Vec};
use embedded_io::{Read, Seek, SeekFrom};
use miniz_oxide::{
    DataFormat, MZFlush,
    inflate::{self, TINFLStatus},
};
use zerocopy::FromBytes;

use crate::ZipError;

pub struct ZipFileEntry {
    pub name: String,
    pub size: u32,
    pub(crate) offset: u32,
}

#[repr(C, packed)]
#[derive(zerocopy::FromBytes)]
struct LocalFileHeader {
    signature: [u8; 4],
    version_needed: u16,
    flags: u16,
    compression: u16,
    mod_time: u16,
    mod_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    filename_len: u16,
    extra_len: u16,
}
const LOCAL_FILE_HEADER_MAGIC: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
const COMPRESSION_STORED: u16 = 0;
const COMPRESSION_DEFLATE: u16 = 8;

/// A streaming reader for a single zip entry.
/// Supports both stored (uncompressed) and deflate-compressed entries.
pub struct ZipEntryReader<'a, R> {
    reader: &'a mut R,
    compression: u16,
    compressed_remaining: usize,
    uncompressed_remaining: usize,
    // Inflate state for deflate decompression
    inflater: Option<Box<inflate::stream::InflateState>>,
    // Input buffer for compressed data
    in_buf: [u8; 512],
    in_buf_start: usize,
    in_buf_end: usize,
}

impl<'a, R: Read + Seek> ZipEntryReader<'a, R> {
    /// Create a new streaming reader for a zip entry.
    /// This seeks to the entry's data and prepares for reading.
    pub fn new(reader: &'a mut R, entry: &ZipFileEntry) -> Result<Self, ZipError> {
        reader
            .seek(SeekFrom::Start(entry.offset as u64))
            .map_err(ZipError::from_io_error)?;

        // Read local file header
        let mut lfh_bytes = [0u8; core::mem::size_of::<LocalFileHeader>()];
        reader
            .read_exact(&mut lfh_bytes)
            .map_err(ZipError::from_read_exact_error)?;
        let lfh = LocalFileHeader::read_from_bytes(&lfh_bytes).unwrap();

        if lfh.signature != LOCAL_FILE_HEADER_MAGIC {
            return Err(ZipError::InvalidSignature);
        }

        // Skip filename and extra field
        let offset = lfh.filename_len + lfh.extra_len;
        reader
            .seek(SeekFrom::Current(offset as _))
            .map_err(ZipError::from_io_error)?;

        let compression = lfh.compression;

        // Only support stored (0) and deflate (8)
        if compression != COMPRESSION_STORED && compression != COMPRESSION_DEFLATE {
            return Err(ZipError::UnsupportedCompression);
        }

        let inflater = if compression == COMPRESSION_DEFLATE {
            Some(Box::new(inflate::stream::InflateState::new(
                DataFormat::Raw,
            )))
        } else {
            None
        };

        Ok(Self {
            reader,
            compression,
            compressed_remaining: lfh.compressed_size as usize,
            uncompressed_remaining: lfh.uncompressed_size as usize,
            inflater,
            in_buf: [0u8; 512],
            in_buf_start: 0,
            in_buf_end: 0,
        })
    }
}

impl<'a, R: Read> ZipEntryReader<'a, R> {
    /// Returns the total uncompressed size of the entry
    pub fn uncompressed_size(&self) -> usize {
        self.uncompressed_remaining
    }

    /// Read decompressed data into the provided buffer.
    /// Returns the number of bytes written to the buffer.
    pub fn read(&mut self, out_buf: &mut [u8]) -> Result<usize, ZipError> {
        if out_buf.is_empty() {
            return Ok(0);
        }

        if self.compression == 0 {
            self.read_stored(out_buf)
        } else {
            self.read_deflate(out_buf)
        }
    }

    /// Read from a stored (uncompressed) entry
    fn read_stored(&mut self, out_buf: &mut [u8]) -> Result<usize, ZipError> {
        let to_read = core::cmp::min(out_buf.len(), self.compressed_remaining);
        if to_read == 0 {
            return Ok(0);
        }

        let read = self
            .reader
            .read(&mut out_buf[..to_read])
            .map_err(ZipError::from_io_error)?;

        self.compressed_remaining -= read;
        self.uncompressed_remaining -= read;

        Ok(read)
    }

    /// Read from a deflate-compressed entry
    fn read_deflate(&mut self, out_buf: &mut [u8]) -> Result<usize, ZipError> {
        let inflater = self.inflater.as_mut().unwrap();
        let mut total_out = 0;

        loop {
            // Refill input buffer if needed
            if self.in_buf_start >= self.in_buf_end && self.compressed_remaining > 0 {
                let to_read = core::cmp::min(self.in_buf.len(), self.compressed_remaining);
                let read = self
                    .reader
                    .read(&mut self.in_buf[..to_read])
                    .map_err(ZipError::from_io_error)?;
                self.in_buf_start = 0;
                self.in_buf_end = read;
                self.compressed_remaining -= read;
            }

            let in_slice = &self.in_buf[self.in_buf_start..self.in_buf_end];
            let out_slice = &mut out_buf[total_out..];

            if out_slice.is_empty() {
                break;
            }

            let flush = if self.compressed_remaining == 0 && self.in_buf_start >= self.in_buf_end {
                MZFlush::Finish
            } else {
                MZFlush::None
            };

            let result = inflate::stream::inflate(inflater, in_slice, out_slice, flush);

            self.in_buf_start += result.bytes_consumed;
            total_out += result.bytes_written;
            #[cfg(feature = "log")]
            log::trace!(
                "Inflate result: {:?}, bytes consumed: {}, bytes written: {}",
                result.status,
                result.bytes_consumed,
                result.bytes_written
            );

            match inflater.last_status() {
                TINFLStatus::Done => {
                    break;
                }
                TINFLStatus::NeedsMoreInput => {
                    if self.compressed_remaining == 0 && self.in_buf_start >= self.in_buf_end {
                        // No more input available but inflater needs more - error
                        return Err(ZipError::DecompressionError);
                    }
                    // Continue loop to read more input
                }
                TINFLStatus::HasMoreOutput => {
                    if out_slice.is_empty() {
                        break;
                    } else {
                        continue;
                    }
                }
                _ => {
                    return Err(ZipError::DecompressionError);
                }
            }
        }

        self.uncompressed_remaining = self.uncompressed_remaining.saturating_sub(total_out);
        Ok(total_out)
    }

    /// Read the entire entry into a Vec.
    /// This is a convenience method for when you need all the data at once.
    pub fn read_to_end(mut self) -> Result<Vec<u8>, ZipError> {
        let mut result = vec![0u8; self.uncompressed_remaining];
        let mut offset = 0;

        while offset < result.len() {
            let read = self.read(&mut result[offset..])?;
            if read == 0 {
                break;
            }
            offset += read;
        }

        result.truncate(offset);
        Ok(result)
    }

    pub fn skip(&mut self, n: u64) -> Result<u64, ZipError> {
        let mut buf = [0u8; 512];
        let mut remaining = n;

        while remaining > 0 {
            let to_read = core::cmp::min(buf.len(), remaining as _);
            let read = self.read(&mut buf[..to_read])?;
            if read == 0 {
                break;
            }
            remaining -= read as u64;
        }

        Ok(n - remaining)
    }
}

/// Convenience function to read an entire zip entry into a Vec
pub fn read_entry<Reader: Read + Seek>(
    reader: &mut Reader,
    entry: &ZipFileEntry,
) -> Result<Vec<u8>, ZipError> {
    let entry_reader = ZipEntryReader::new(reader, entry)?;
    entry_reader.read_to_end()
}

impl<Reader> embedded_io::ErrorType for ZipEntryReader<'_, Reader> {
    type Error = ZipError;
}

impl<Reader: Read> embedded_io::Read for ZipEntryReader<'_, Reader> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.read(buf)
    }
}

impl<Reader: Read> embedded_io::Seek for ZipEntryReader<'_, Reader> {
    fn seek(&mut self, pos: embedded_io::SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            SeekFrom::Current(n) => self.skip(n as _),
            _ => Err(ZipError::IoError(embedded_io::ErrorKind::Unsupported)),
        }
    }
}
