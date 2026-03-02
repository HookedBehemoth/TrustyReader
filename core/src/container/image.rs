use crate::{
    container::{jpeg, png, tbmp},
    fs::File, res::font::Mode,
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

    pub fn has_grayscale(&self) -> bool {
        match self {
            Image::Tbmp(_) => true,
            _ => false,
        }
    }

    pub fn blit_bw(
        &self,
        file: &mut impl File,
        buffers: &mut crate::framebuffer::DisplayBuffers,
    ) {
        match self {
            Image::Tbmp(tbmp) => {
                let mut buffer = alloc::vec![0u8; tbmp.buffer_size()];
                tbmp::load_buffer(file, &tbmp, Mode::Bw, &mut buffer).ok();
            }
            Image::OneBpp(decoded) => {
                buffers.blit(&decoded.data, decoded.width, decoded.height);
            }
        }
    }

    pub fn blit_gray(&self, file: &mut impl File, mode: Mode, buffers: &mut crate::framebuffer::DisplayBuffers) {
        match self {
            Image::Tbmp(tbmp) => {
                let mut buffer = alloc::vec![0u8; tbmp.buffer_size()];
                tbmp::load_buffer(file, &tbmp, mode, &mut buffer).ok();
                buffers.blit(&buffer, tbmp.width, tbmp.height);
            }
            _ => { }
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

pub fn decode(
    format: Format,
    file: &mut impl File,
    max_w: u16,
    max_h: u16,
) -> Result<Image, &'static str> {
    log::info!("Decoding image with format {:?}, max dimensions {}x{}", format, max_w, max_h);
    let sz = file.size() as _;
    match format {
        Format::Tbmp => {
            let header = tbmp::parse_header(file).map_err(|_| "image: failed to parse TBMP")?;
            Ok(Image::Tbmp(header))
        }
        Format::Jpeg => {
            let image = jpeg::decode_jpeg_streaming(file, sz, max_w, max_h)?;
            Ok(Image::OneBpp(image))
        }
        Format::Png => {
            let image = png::decode_png_from(file, max_w, max_h)?;
            Ok(Image::OneBpp(image))
        }
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
