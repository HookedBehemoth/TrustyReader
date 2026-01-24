use embedded_graphics::{Pixel, pixelcolor::BinaryColor, prelude::{DrawTarget, OriginDimensions, Size}};

pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 480;

/// Refresh modes for the display
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum RefreshMode {
    /// Full refresh with complete waveform
    Full,
    /// Half refresh (1720ms) - balanced quality and speed
    Half,
    /// Fast refresh using custom LUT
    Fast,
}

/// Display rotation/orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// No rotation (landscape, 800x480)
    Rotate0,
    /// 90° clockwise (portrait, 480x800)
    Rotate90,
    /// 180° rotation (landscape upside-down, 800x480)
    Rotate180,
    /// 270° clockwise / 90° counter-clockwise (portrait, 480x800)
    Rotate270,
}

pub trait Display {
    fn set_rotation(&mut self, rotation: Rotation);
    fn get_rotation(&self) -> Rotation;
    fn width(&self) -> usize;
    fn height(&self) -> usize;

    fn get_framebuffer(&self) -> &[u8];
    fn get_framebuffer_mut(&mut self) -> Framebuffer<'_>;
    fn display(&mut self, mode: RefreshMode);
    fn copy_to_lsb(&mut self);
    fn copy_to_msb(&mut self);
    fn display_grayscale(&mut self);
}

pub struct Framebuffer<'a> {
    buffer: &'a mut [u8],
    // width: usize,
    // height: usize,
    rotation: Rotation,
}
impl Framebuffer<'_> {
    pub fn new(
        buffer: &'_ mut [u8],
        // width: usize,
        // height: usize,
        rotation: Rotation) -> Framebuffer<'_> {
        Framebuffer {
            buffer,
            // width,
            // height,
            rotation,
        }
    }
}
impl OriginDimensions for Framebuffer<'_> {
    fn size(&self) -> Size {
        match self.rotation {
            Rotation::Rotate0 | Rotation::Rotate180 => Size::new(WIDTH as u32, HEIGHT as u32),
            Rotation::Rotate90 | Rotation::Rotate270 => Size::new(HEIGHT as u32, WIDTH as u32),
        }
    }
}
impl DrawTarget for Framebuffer<'_> {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let (x, y) = match self.rotation {
                Rotation::Rotate0 => (coord.x as usize, coord.y as usize),
                Rotation::Rotate90 => (HEIGHT - 1 - coord.y as usize, coord.x as usize),
                Rotation::Rotate180 => (WIDTH - 1 - coord.x as usize, HEIGHT - 1 - coord.y as usize),
                Rotation::Rotate270 => (coord.y as usize, WIDTH - 1 - coord.x as usize),
            };
            if x < WIDTH && y < HEIGHT {
                let index = y * WIDTH + x;
                let byte_index = index / 8;
                let bit_index = 7 - (index % 8);
                match color {
                    BinaryColor::On => {
                        self.buffer[byte_index] |= 1 << bit_index;
                    }
                    BinaryColor::Off => {
                        self.buffer[byte_index] &= !(1 << bit_index);
                    }
                }
            }
        }
        Ok(())
    }
}
