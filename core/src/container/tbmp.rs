use embedded_io::SeekFrom;
use log::info;

use crate::{fs::File, res::font};

const TBMP_MAGIC: &[u8; 4] = b"TBMP";

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Header {
    pub magic: [u8; 4],
    pub width: u16,
    pub height: u16,
    pub background: Background,
}

impl Header {
    pub fn buffer_size(&self) -> usize {
        (self.width as usize * self.height as usize + 7) / 8
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Background {
    White = 0,
    Black = 1,
}

#[derive(Debug)]
pub enum Error {
    IoError(embedded_io::ErrorKind),
    InvalidFormat,
}

impl Error {
    pub fn from<Error: embedded_io::Error>(err: Error) -> Self {
        Self::IoError(embedded_io::Error::kind(&err))
    }
}

type Result<T> = core::result::Result<T, Error>;

pub fn parse_header(file: &mut impl File) -> Result<Header> {
    let header = unsafe { file.read_sized::<Header>().map_err(Error::from)? };
    if &header.magic != TBMP_MAGIC {
        return Err(Error::InvalidFormat);
    }
    if header.width % 8 != 0 || header.height % 8 != 0 {
        return Err(Error::InvalidFormat);
    }
    info!("Parsed TBMP header: width={}, height={}", header.width, header.height);
    Ok(header)
}

pub fn load_buffer(
    file: &mut impl File,
    header: &Header,
    which: font::Mode,
    buffer: &mut [u8],
) -> Result<()> {
    let size = header.width as usize * header.height as usize / 8 ;
    if buffer.len() < size {
        return Err(Error::InvalidFormat);
    }

    let offset = match which {
        font::Mode::Bw => 0,
        font::Mode::Msb => size as u64,
        font::Mode::Lsb => (size * 2) as u64,
    } + core::mem::size_of::<Header>() as u64;
    file.seek(SeekFrom::Start(offset)).map_err(Error::from)?;
    file.read(buffer).map_err(Error::from)?;

    Ok(())
}

pub fn write(
    file: &mut impl File,
    width: u16,
    height: u16,
    background: Background,
    bw: &[u8],
    msb: &[u8],
    lsb: &[u8],
) -> Result<()> {
    let header = Header {
        magic: *TBMP_MAGIC,
        width,
        height,
        background,
    };
    unsafe { file.write_sized(&header).map_err(Error::from)? };
    file.write_all(bw).map_err(Error::from)?;
    file.write_all(msb).map_err(Error::from)?;
    file.write_all(lsb).map_err(Error::from)?;

    Ok(())
}
