use alloc::{boxed::Box, string::String, vec, vec::Vec};
use embedded_io::SeekFrom;
use miniz_oxide::{
    DataFormat, MZFlush,
    inflate::{self, TINFLStatus},
};

pub struct ZipFileEntry {
    pub name: String,
    pub size: u32,
    offset: u32,
}

pub fn parse_zip<Reader: crate::fs::File>(
    reader: &mut Reader,
) -> Result<Box<[ZipFileEntry]>, ZipError> {
    let end_dir = find_end_central_directory(reader)?;
    read_central_directory(reader, &end_dir)
}

#[repr(C, packed)]
struct EndCentralDir {
    signature: u32,
    disk_number: u16,
    central_dir_start_disk: u16,
    num_entries_this_disk: u16,
    total_num_entries: u16,
    central_dir_size: u32,
    central_dir_offset: u32,
    comment_length: u16,
}

#[repr(C, packed)]
struct CentralDirEntry {
    signature: u32,
    version_made: u16,
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
    comment_len: u16,
    disk_start: u16,
    internal_attr: u16,
    external_attr: u32,
    local_header_offset: u32,
}

#[repr(C, packed)]
struct LocalFileHeader {
    signature: u32,
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

fn find_end_central_directory<Reader: crate::fs::File>(
    reader: &mut Reader,
) -> Result<EndCentralDir, ZipError> {
    let mut buf = [0u8; 1024];

    let file_size = reader.size();
    let seek_start = if file_size > buf.len() {
        file_size - buf.len()
    } else {
        0
    };
    reader
        .seek(SeekFrom::Start(seek_start as u64))
        .map_err(|_| ZipError::IoError)?;
    let read = reader.read(&mut buf).map_err(|_| ZipError::IoError)?;

    for i in (0..read - 4).rev() {
        if buf[i..i + 4] != [0x50, 0x4b, 0x05, 0x06] {
            continue;
        }
        unsafe {
            let mut dir: EndCentralDir = core::mem::zeroed();
            let dir_buf = core::slice::from_raw_parts_mut(
                &mut dir as *mut EndCentralDir as *mut u8,
                core::mem::size_of::<EndCentralDir>(),
            );
            dir_buf.copy_from_slice(&buf[i..i + core::mem::size_of::<EndCentralDir>()]);
            return Ok(dir);
        }
    }

    Err(ZipError::InvalidData)
}

fn read_central_directory<Reader: crate::fs::File>(
    reader: &mut Reader,
    dir: &EndCentralDir,
) -> Result<Box<[ZipFileEntry]>, ZipError> {
    let entry_count = dir.total_num_entries as usize;
    if entry_count == 0 {
        return Err(ZipError::InvalidData);
    }

    let mut entries = Vec::with_capacity(entry_count);
    reader
        .seek(SeekFrom::Start(dir.central_dir_offset as u64))
        .map_err(|_| ZipError::IoError)?;
    for _ in 0..entry_count {
        let cde: CentralDirEntry = unsafe { reader.read_sized().map_err(|_| ZipError::IoError)? };
        if cde.signature != 0x02014b50 {
            return Err(ZipError::InvalidSignature);
        }

        let mut name_buf = vec![0u8; cde.filename_len as usize];
        reader.read(&mut name_buf).map_err(|_| ZipError::IoError)?;

        // Skip extra and comment
        reader
            .seek(SeekFrom::Current(cde.extra_len as _))
            .map_err(|_| ZipError::IoError)?;
        reader
            .seek(SeekFrom::Current(cde.comment_len as _))
            .map_err(|_| ZipError::IoError)?;
        let name = String::from_utf8(name_buf).map_err(|_| ZipError::InvalidData)?;
        let entry = ZipFileEntry {
            name,
            size: cde.uncompressed_size,
            offset: cde.local_header_offset,
        };
        entries.push(entry);
    }

    Ok(entries.into_boxed_slice())
}

/// Error type for zip entry reading operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipError {
    IoError,
    InvalidSignature,
    UnsupportedCompression,
    DecompressionError,
    InvalidData,
}

impl core::fmt::Display for ZipError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ZipError::IoError => write!(f, "I/O error occurred"),
            ZipError::InvalidSignature => write!(f, "Invalid zip signature"),
            ZipError::UnsupportedCompression => write!(f, "Unsupported compression method"),
            ZipError::DecompressionError => write!(f, "Error during decompression"),
            ZipError::InvalidData => write!(f, "Invalid zip data"),
        }
    }
}

/// A streaming reader for a single zip entry.
/// Supports both stored (uncompressed) and deflate-compressed entries.
pub struct ZipEntryReader<'a, R: crate::fs::File> {
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
    finished: bool,
}

impl<'a, R: crate::fs::File> ZipEntryReader<'a, R> {
    /// Create a new streaming reader for a zip entry.
    /// This seeks to the entry's data and prepares for reading.
    pub fn new(reader: &'a mut R, entry: &ZipFileEntry) -> Result<Self, ZipError> {
        reader
            .seek(SeekFrom::Start(entry.offset as u64))
            .map_err(|_| ZipError::IoError)?;

        // Read local file header
        let lfh: LocalFileHeader = unsafe { reader.read_sized().map_err(|_| ZipError::IoError)? };

        if lfh.signature != 0x04034b50 {
            return Err(ZipError::InvalidSignature);
        }

        // Skip filename and extra field
        reader
            .seek(SeekFrom::Current(lfh.filename_len as _))
            .map_err(|_| ZipError::IoError)?;
        reader
            .seek(SeekFrom::Current(lfh.extra_len as _))
            .map_err(|_| ZipError::IoError)?;

        let compression = lfh.compression;

        // Only support stored (0) and deflate (8)
        if compression != 0 && compression != 8 {
            return Err(ZipError::UnsupportedCompression);
        }

        let inflater = if compression == 8 {
            Some(Box::new(inflate::stream::InflateState::new(DataFormat::Raw)))
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
            finished: false,
        })
    }

    /// Returns the total uncompressed size of the entry
    pub fn uncompressed_size(&self) -> usize {
        self.uncompressed_remaining
    }

    /// Returns true if all data has been read
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Read decompressed data into the provided buffer.
    /// Returns the number of bytes written to the buffer.
    pub fn read(&mut self, out_buf: &mut [u8]) -> Result<usize, ZipError> {
        if self.finished || out_buf.is_empty() {
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
            self.finished = true;
            return Ok(0);
        }

        let read = self
            .reader
            .read(&mut out_buf[..to_read])
            .map_err(|_| ZipError::IoError)?;

        self.compressed_remaining -= read;
        self.uncompressed_remaining -= read;

        if self.compressed_remaining == 0 {
            self.finished = true;
        }

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
                    .map_err(|_| ZipError::IoError)?;
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

            match inflater.last_status() {
                TINFLStatus::Done => {
                    self.finished = true;
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
                    // Output buffer is full, return what we have
                    break;
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

        while !self.finished && offset < result.len() {
            let read = self.read(&mut result[offset..])?;
            if read == 0 {
                break;
            }
            offset += read;
        }

        result.truncate(offset);
        Ok(result)
    }
}

/// Convenience function to read an entire zip entry into a Vec
pub fn read_entry<Reader: crate::fs::File>(
    reader: &mut Reader,
    entry: &ZipFileEntry,
) -> Result<Vec<u8>, ZipError> {
    let entry_reader = ZipEntryReader::new(reader, entry)?;
    entry_reader.read_to_end()
}
