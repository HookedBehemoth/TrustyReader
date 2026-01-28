use alloc::boxed::Box;

use crate::{
    framebuffer::{BUFFER_SIZE, HEIGHT, WIDTH},
    fs::File,
};

#[repr(C, packed)]
pub struct ImageHeader {
    pub mark: [u8; 4],
    pub width: u16,
    pub height: u16,
    pub color_mode: ColorMode,
    pub compression: Compression,
    pub data_size: u32,
    pub md5: [u8; 8],
}

#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum ColorMode {
    Monochrome = 0,
}

#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum Compression {
    None = 0,
}

static XTG_MARKER: &[u8; 4] = b"XTG\0";
static XTH_MARKER: &[u8; 4] = b"XTH\0";

#[derive(Debug)]
pub enum XtError {
    InvalidSignature,
    InvalidData,
    IoError,
}

type Result<T> = core::result::Result<T, XtError>;

pub fn parse_xtg(file: &mut impl File) -> Result<Box<[u8; BUFFER_SIZE]>> {
    let header: ImageHeader = unsafe { file.read_sized().map_err(|_| XtError::IoError)? };
    if &header.mark != XTG_MARKER {
        return Err(XtError::InvalidSignature);
    }
    if header.width as usize != HEIGHT
        || header.height as usize != WIDTH
        || header.color_mode != ColorMode::Monochrome
    {
        return Err(XtError::InvalidData);
    }
    let mut data = Box::new([0u8; BUFFER_SIZE]);
    file.read_exact(&mut data[..])
        .map_err(|_| XtError::IoError)?;
    Ok(data)
}

pub fn parse_xth(file: &mut impl File) -> Result<Box<[[u8; BUFFER_SIZE]; 2]>> {
    let header: ImageHeader = unsafe { file.read_sized().map_err(|_| XtError::IoError)? };
    if &header.mark != XTH_MARKER {
        return Err(XtError::InvalidSignature);
    }
    if header.width as usize != HEIGHT
        || header.height as usize != WIDTH
        || header.color_mode != ColorMode::Monochrome
    {
        return Err(XtError::InvalidData);
    }
    let mut data = Box::new([[0u8; BUFFER_SIZE]; 2]);
    file.read_exact(&mut data[0])
        .map_err(|_| XtError::IoError)?;
    file.read_exact(&mut data[1])
        .map_err(|_| XtError::IoError)?;
    Ok(data)
}

pub fn write_xtg(
    file: &mut impl File,
    data: &[u8; BUFFER_SIZE],
) -> core::result::Result<(), XtError> {
    let header = ImageHeader {
        mark: *XTG_MARKER,
        width: HEIGHT as _,
        height: WIDTH as _,
        color_mode: ColorMode::Monochrome,
        compression: Compression::None,
        data_size: BUFFER_SIZE as u32,
        md5: [0u8; 8],
    };
    unsafe { file.write_sized(&header).map_err(|_| XtError::IoError)? };
    file.write_all(data).map_err(|_| XtError::IoError)?;
    Ok(())
}

pub fn write_xth(
    file: &mut impl File,
    data_lsb: &[u8; BUFFER_SIZE],
    data_msb: &[u8; BUFFER_SIZE],
) -> core::result::Result<(), XtError> {
    let header = ImageHeader {
        mark: *XTH_MARKER,
        width: HEIGHT as _,
        height: WIDTH as _,
        color_mode: ColorMode::Monochrome,
        compression: Compression::None,
        data_size: BUFFER_SIZE as u32,
        md5: [0u8; 8],
    };
    unsafe { file.write_sized(&header).map_err(|_| XtError::IoError)? };
    file.write_all(data_lsb).map_err(|_| XtError::IoError)?;
    file.write_all(data_msb).map_err(|_| XtError::IoError)?;
    Ok(())
}
