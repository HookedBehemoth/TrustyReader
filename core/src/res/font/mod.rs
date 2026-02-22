use embedded_graphics::{pixelcolor::BinaryColor, prelude::OriginDimensions};
use log::{trace, warn};

use crate::framebuffer::DisplayBuffers;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FontSize {
    Size26,
    Size28,
    Size30,
}
impl FontSize {
    pub fn repr(self) -> &'static str {
        match self {
            FontSize::Size26 => "26",
            FontSize::Size28 => "28",
            FontSize::Size30 => "30",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    Bookerly,
}
impl FontFamily {
    pub fn repr(self) -> &'static str {
        match self {
            FontFamily::Bookerly => "Bookerly",
        }
    }
}

pub mod bookerly_26;
pub mod bookerly_28;
pub mod bookerly_30;
pub mod bookerly_bold_26;
pub mod bookerly_bold_28;
pub mod bookerly_bold_30;
pub mod bookerly_bold_italic_26;
pub mod bookerly_bold_italic_28;
pub mod bookerly_bold_italic_30;
pub mod bookerly_italic_26;
pub mod bookerly_italic_28;
pub mod bookerly_italic_30;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Font {
    pub family: FontFamily,
    pub size: FontSize,
}

impl Font {
    pub fn new(family: FontFamily, size: FontSize) -> Self {
        Self { family, size }
    }

    pub fn y_advance(&self) -> u16 {
        match self.size {
            FontSize::Size26 => 30,
            FontSize::Size28 => 32,
            FontSize::Size30 => 34,
        }
    }

    pub fn bookerly(size: FontSize) -> Self {
        Self { family: FontFamily::Bookerly, size }
    }
}

impl Font {
    pub fn definition(&self, style: FontStyle) -> &'static FontDefinition<'static> {
        use FontFamily::*;
        use FontSize::*;
        use FontStyle::*;
        match (&self.family, &self.size, &style) {
            (Bookerly, Size26, Regular) => &bookerly_26::FONT,
            (Bookerly, Size28, Regular) => &bookerly_28::FONT,
            (Bookerly, Size30, Regular) => &bookerly_30::FONT,
            (Bookerly, Size26, Bold) => &bookerly_bold_26::FONT,
            (Bookerly, Size28, Bold) => &bookerly_bold_28::FONT,
            (Bookerly, Size30, Bold) => &bookerly_bold_30::FONT,
            (Bookerly, Size26, BoldItalic) => &bookerly_bold_italic_26::FONT,
            (Bookerly, Size28, BoldItalic) => &bookerly_bold_italic_28::FONT,
            (Bookerly, Size30, BoldItalic) => &bookerly_bold_italic_30::FONT,
            (Bookerly, Size26, Italic) => &bookerly_italic_26::FONT,
            (Bookerly, Size28, Italic) => &bookerly_italic_28::FONT,
            (Bookerly, Size30, Italic) => &bookerly_italic_30::FONT,
        }
    }
}

#[repr(C)]
pub struct FontDefinition<'a> {
    pub size: u32,
    pub y_advance: u8,
    pub glyphs: &'a [Glyph],
    pub bitmap_bw: &'a [u8],
    pub bitmap_msb: &'a [u8],
    pub bitmap_lsb: &'a [u8],
}

impl FontDefinition<'_> {
    pub fn get_glyph(&self, codepoint: u16) -> Option<&Glyph> {
        match self
            .glyphs
            .binary_search_by(|glyph| glyph.codepoint.cmp(&codepoint))
        {
            Ok(index) => Some(&self.glyphs[index]),
            Err(_) => None,
        }
    }

    pub fn codepoint_width(&self, codepoint: u16) -> Option<u8> {
        self.get_glyph(codepoint).map(|glyph| glyph.x_advance())
    }

    pub fn char_width(&self, ch: char) -> Option<u8> {
        self.codepoint_width(ch as u16)
    }

    pub fn word_width(&self, word: &str) -> u16 {
        word.chars().fold(0u16, |acc, codepoint| {
            acc + self.char_width(codepoint).unwrap_or(0) as u16
        })
    }
}

#[repr(C)]
pub struct Glyph {
    pub codepoint: u16,
    pub bitmap_index: u16,
    pub blob: u32,
}

impl Glyph {
    pub const fn new(
        codepoint: u16,
        bitmap_index: u16,
        x_advance: u8,
        width: u8,
        height: u8,
        xmin: i8,
        ymin: i8,
    ) -> Self {
        assert!(x_advance < 0x40);
        assert!(width < 0x40);
        assert!(height < 0x40);
        assert!(xmin >= -32 && xmin < 32);
        assert!(ymin >= -32 && ymin < 32);
        let blob = ((x_advance as u32) << 0x1A)
            | ((width as u32) << 0x14)
            | ((height as u32) << 0x0E)
            | (((xmin as i32 + 32) as u32) << 8)
            | ((ymin as i32 + 32) as u32);
        Self { codepoint, bitmap_index, blob }
    }

    const MASK: u32 = 0x3F;

    pub fn x_advance(&self) -> u8 {
        ((self.blob >> 0x1A) & Self::MASK) as u8
    }
    pub fn width(&self) -> u8 {
        ((self.blob >> 0x14) & Self::MASK) as u8
    }
    pub fn height(&self) -> u8 {
        ((self.blob >> 0x0E) & Self::MASK) as u8
    }
    pub fn xmin(&self) -> i8 {
        ((self.blob >> 0x08) & Self::MASK) as i8 - 32
    }
    pub fn ymin(&self) -> i8 {
        ((self.blob >> 0x00) & Self::MASK) as i8 - 32
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Bw,
    Msb,
    Lsb,
}

pub fn draw_glyph(
    font: &FontDefinition,
    codepoint: u16,
    display_buffers: &mut DisplayBuffers,
    x_offset: isize,
    y_offset: isize,
    mode: Mode,
) -> Result<u8, usize> {
    let glyph = font
        .glyphs
        .binary_search_by(|glyph| glyph.codepoint.cmp(&codepoint))?;
    let glyph = &font.glyphs[glyph];

    let bitmap = match mode {
        Mode::Bw => font.bitmap_bw,
        Mode::Msb => font.bitmap_msb,
        Mode::Lsb => font.bitmap_lsb,
    };
    let width = glyph.width();
    let height = glyph.height();
    let x_advance = glyph.x_advance();
    let xmin = glyph.xmin();
    let ymin = glyph.ymin();

    let size = display_buffers.size();
    if x_offset > size.width as _ ||
       y_offset > size.height as _{
        warn!("Glyph not placed on screen");
        return Ok(x_advance);
    }

    trace!(
        "Drawing glyph U+{:04X} at offset ({}, {}) with size {}x{}, xmin {}, ymin {}",
        codepoint, x_offset, y_offset, width, height, xmin, ymin
    );

    let x_offset = x_offset + xmin as isize;
    let y_offset = y_offset - height as isize - ymin as isize;
    for y in 0..height as isize {
        for x in 0..width as isize {
            let fb_x = x_offset + x;
            let fb_y = y_offset + y;
            if fb_x < 0 || fb_x >= size.width as isize || fb_y < 0 || fb_y >= size.height as isize {
                warn!("Pixel out of bounds: fb_x={}, fb_y={}", fb_x, fb_y);
                continue;
            }

            let bitmap_index =
                glyph.bitmap_index as usize + (y as usize * width as usize + x as usize) / 8;
            let bitmap_bit = 7 - ((y as usize * width as usize + x as usize) % 8);

            let byte = bitmap[bitmap_index];
            let pixel_on = (byte >> bitmap_bit) & 1;

            if mode == Mode::Bw {
                if pixel_on == 0 {
                    display_buffers.set_pixel(fb_x as _, fb_y as _, BinaryColor::Off);
                }
            } else {
                if pixel_on != 0 {
                    display_buffers.set_pixel(fb_x as _, fb_y as _, BinaryColor::On);
                }
            }
        }
    }

    Ok(x_advance)
}
