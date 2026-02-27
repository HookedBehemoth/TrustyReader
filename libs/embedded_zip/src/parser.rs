use crate::{ZipError, ZipFileEntry};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use embedded_io::{Read, Seek, SeekFrom};
use zerocopy::FromBytes;

pub fn parse_zip<Reader>(reader: &mut Reader) -> Result<Box<[ZipFileEntry]>, ZipError>
where
    Reader: Read + Seek,
{
    let end_dir = find_end_central_directory(reader)?;
    read_central_directory(reader, &end_dir)
}

#[repr(C, packed)]
#[derive(zerocopy::FromBytes)]
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
#[derive(zerocopy::FromBytes)]
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
    pub local_header_offset: u32,
}

fn find_end_central_directory<Reader>(reader: &mut Reader) -> Result<EndCentralDir, ZipError>
where
    Reader: Read + Seek,
{
    let mut buf = [0u8; 1024];

    let seek_start = buf.len() as i64;
    reader
        .seek(SeekFrom::End(-seek_start))
        .map_err(ZipError::from_io_error)?;
    let read = reader.read(&mut buf).map_err(ZipError::from_io_error)?;

    for i in (0..read - 4).rev() {
        if buf[i..i + 4] != [0x50, 0x4b, 0x05, 0x06] {
            continue;
        }
        let sz = core::mem::size_of::<EndCentralDir>();
        return Ok(EndCentralDir::read_from_bytes(&buf[i..i + sz]).unwrap());
    }

    Err(ZipError::InvalidData)
}

fn read_central_directory<Reader>(
    reader: &mut Reader,
    dir: &EndCentralDir,
) -> Result<Box<[ZipFileEntry]>, ZipError>
where
    Reader: Read + Seek,
{
    let entry_count = dir.total_num_entries as usize;
    if entry_count == 0 {
        return Err(ZipError::InvalidData);
    }

    let mut entries = Vec::with_capacity(entry_count);
    reader
        .seek(SeekFrom::Start(dir.central_dir_offset as u64))
        .map_err(ZipError::from_io_error)?;
    for _ in 0..entry_count {
        let mut cde_buf = [0u8; core::mem::size_of::<CentralDirEntry>()];
        reader
            .read_exact(&mut cde_buf)
            .map_err(ZipError::from_read_exact_error)?;
        let cde = CentralDirEntry::read_from_bytes(&cde_buf).unwrap();
        if cde.signature != 0x02014b50 {
            return Err(ZipError::InvalidSignature);
        }

        let mut name_buf = vec![0u8; cde.filename_len as usize];
        reader
            .read_exact(&mut name_buf)
            .map_err(ZipError::from_read_exact_error)?;

        // Skip extra and comment
        reader
            .seek(SeekFrom::Current(cde.extra_len as _))
            .map_err(ZipError::from_io_error)?;
        reader
            .seek(SeekFrom::Current(cde.comment_len as _))
            .map_err(ZipError::from_io_error)?;
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
