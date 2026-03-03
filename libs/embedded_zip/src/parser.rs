use crate::{ZipError, ZipFileEntry};
use alloc::{boxed::Box, vec::Vec};
use embedded_io::{Read, Seek, SeekFrom};
use memchr::memmem;
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
    signature: [u8; 4],
    disk_number: u16,
    central_dir_start_disk: u16,
    num_entries_this_disk: u16,
    total_num_entries: u16,
    central_dir_size: u32,
    central_dir_offset: u32,
    comment_length: u16,
}
const END_CENTRAL_DIR_MAGIC: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

#[repr(C, packed)]
#[derive(zerocopy::FromBytes)]
struct CentralDirEntry {
    signature: [u8; 4],
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
const CENTRAL_DIR_ENTRY_MAGIC: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

fn find_end_central_directory<Reader>(reader: &mut Reader) -> Result<EndCentralDir, ZipError>
where
    Reader: Read + Seek,
{
    let mut buf = [0u8; 1024];

    let seek_start = buf.len() as i64;
    reader
        .seek(SeekFrom::End(-seek_start))
        .map_err(ZipError::from_io_error)?;
    reader
        .read_exact(&mut buf)
        .map_err(ZipError::from_read_exact_error)?;

    let sz = core::mem::size_of::<EndCentralDir>();
    let Some(idx) = memmem::rfind(&buf[..buf.len() - sz + 4], &END_CENTRAL_DIR_MAGIC) else {
        return Err(ZipError::InvalidData);
    };
    Ok(EndCentralDir::read_from_bytes(&buf[idx..idx + sz]).unwrap())
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

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(entry_count)
        .map_err(|_| ZipError::OutOfMemory)?;
    reader
        .seek(SeekFrom::Start(dir.central_dir_offset as u64))
        .map_err(ZipError::from_io_error)?;
    for _ in 0..entry_count {
        let mut cde_buf = [0u8; core::mem::size_of::<CentralDirEntry>()];
        reader
            .read_exact(&mut cde_buf)
            .map_err(ZipError::from_read_exact_error)?;
        let cde = CentralDirEntry::read_from_bytes(&cde_buf).unwrap();
        if cde.signature != CENTRAL_DIR_ENTRY_MAGIC {
            return Err(ZipError::InvalidSignature);
        }

        const MAX_FILENAME: usize = 512;
        let filename_len = cde.filename_len as usize;
        if filename_len > MAX_FILENAME {
            let offset = cde.filename_len + cde.extra_len + cde.comment_len;
            reader
                .seek(SeekFrom::Current(offset as _))
                .map_err(ZipError::from_io_error)?;
            continue;
        }
        let mut name_buf = [0u8; MAX_FILENAME];
        reader
            .read_exact(&mut name_buf[..filename_len])
            .map_err(ZipError::from_read_exact_error)?;

        let name = str::from_utf8(name_buf[..filename_len].trim_ascii())
            .map_err(|_| ZipError::InvalidData)?;
        let entry = ZipFileEntry::new(name, cde.uncompressed_size, cde.local_header_offset);
        entries.push(entry);

        #[cfg(feature = "log")]
        log::info!("Parsed ZIP entry: {} (hash: {})", name, entry.name_hash);

        // Skip extra and comment
        let offset = cde.extra_len + cde.comment_len;
        reader
            .seek(SeekFrom::Current(offset as _))
            .map_err(ZipError::from_io_error)?;
    }

    Ok(entries.into_boxed_slice())
}
