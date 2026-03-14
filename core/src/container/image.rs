use embedded_io::{Read, Seek};

use crate::{
    container::{jpeg, png, tbmp},
    fs::File,
    res::font::Mode,
};

pub enum Image {
    Tbmp(tbmp::Header),
    OneBpp(DecodedImage),
}

impl Image {
    pub fn buffer_size(&self) -> usize {
        match self {
            Image::Tbmp(header) => header.buffer_size(),
            Image::OneBpp(decoded) => decoded.buffer_size(),
        }
    }

    /// TODO: move out of format
    pub fn height(&self) -> u16 {
        match self {
            Image::Tbmp(header) => header.height,
            Image::OneBpp(decoded) => decoded.height,
        }
    }

    pub fn has_grayscale(&self) -> bool {
        match self {
            Image::Tbmp(_) => true,
            _ => false,
        }
    }

    pub fn blit_bw(
        &self,
        file: &mut impl File,
        offset: u16,
        buffers: &mut crate::framebuffer::DisplayBuffers,
    ) {
        match self {
            Image::Tbmp(tbmp) => {
                let mut buffer = alloc::vec![0u8; tbmp.buffer_size()];
                tbmp::load_buffer(file, &tbmp, Mode::Bw, &mut buffer).ok();
                buffers.blit(&buffer, tbmp.width, tbmp.height, offset);
            }
            Image::OneBpp(decoded) => {
                buffers.blit(&decoded.data, decoded.width, decoded.height, offset);
            }
        }
    }

    pub fn blit_gray(
        &self,
        file: &mut impl File,
        offset: u16,
        mode: Mode,
        buffers: &mut crate::framebuffer::DisplayBuffers,
    ) {
        match self {
            Image::Tbmp(tbmp) => {
                let mut buffer = alloc::vec![0u8; tbmp.buffer_size()];
                tbmp::load_buffer(file, &tbmp, mode, &mut buffer).ok();
                buffers.blit(&buffer, tbmp.width, tbmp.height, offset);
            }
            _ => {}
        }
    }
}

pub struct DecodedImage {
    /// Image width in pixels.
    pub width: u16,
    /// Image height in pixels.
    pub height: u16,
    /// Packed 1-bit pixel data, `stride * height` bytes.
    pub data: alloc::vec::Vec<u8>,
}

impl DecodedImage {
    fn buffer_size(&self) -> usize {
        (self.width.div_ceil(8) * self.height) as usize
    }
}

pub fn decode<R: Read + Seek>(
    format: Format,
    file: &mut R,
    size: u32,
    max_w: u16,
    max_h: u16,
) -> Result<Image, &'static str> {
    match format {
        Format::Tbmp => {
            let header = tbmp::parse_header(file).map_err(|_| "image: failed to parse TBMP")?;
            Ok(Image::Tbmp(header))
        }
        Format::Jpeg => {
            let image = jpeg::decode_jpeg_streaming(file, size, max_w, max_h)?;
            Ok(Image::OneBpp(image))
        }
        Format::Png => {
            let image = png::decode_png_from(file, max_w, max_h)?;
            Ok(Image::OneBpp(image))
        }
    }
}

/// Read image dimensions without decoding pixel data.
/// Auto-detects format from file magic bytes.
/// Returns `(width, height)` in native (unscaled) pixels.
pub fn read_size<R: Read + Seek>(
    file: &mut R,
    size: u32,
) -> Result<(u16, u16), &'static str> {
    let mut magic = [0u8; 2];
    file.read_exact(&mut magic).map_err(|_| "image: read magic")?;

    if magic[0] == 0xFF && magic[1] == 0xD8 {
        jpeg::read_jpeg_size(file, size)
    } else if magic[0] == 0x89 && magic[1] == 0x50 {
        png::read_png_size(file)
    } else {
        Err("image: unknown format")
    }
}

/// Compute the scaled output dimensions given raw image size and max bounds.
/// Uses aspect-ratio-preserving scaling (fits within max_w × max_h).
pub fn scaled_size(raw_w: u16, raw_h: u16, max_w: u16, max_h: u16) -> (u16, u16) {
    if raw_w <= max_w && raw_h <= max_h {
        return (raw_w, raw_h);
    }
    // Width-bound when raw_w/max_w > raw_h/max_h (cross-multiply to avoid division)
    if (raw_w as u32) * (max_h as u32) > (raw_h as u32) * (max_w as u32) {
        let out_w = max_w;
        let out_h = ((raw_h as u32) * (max_w as u32) / (raw_w as u32)).max(1) as u16;
        (out_w, out_h)
    } else {
        let out_h = max_h;
        let out_w = ((raw_w as u32) * (max_h as u32) / (raw_h as u32)).max(1) as u16;
        (out_w, out_h)
    }
}

pub fn get_format(ext: &str) -> Option<Format> {
    if ext.eq_ignore_ascii_case("png") {
        Some(Format::Png)
    } else if ext.eq_ignore_ascii_case("tbmp") {
        Some(Format::Tbmp)
    } else if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        Some(Format::Jpeg)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Tbmp,
    Jpeg,
    Png,
}

impl Format {
    pub fn guess_from_filename(filename: &str) -> Option<Self> {
        if let Some(pos) = filename.rfind('.') {
            let ext = &filename[pos + 1..];
            get_format(ext)
        } else {
            None
        }
    }
}
