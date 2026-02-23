use embedded_graphics::{
    Pixel,
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, OriginDimensions, Size},
};

pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 480;
pub const BUFFER_SIZE: usize = WIDTH * HEIGHT / 8;

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

impl Rotation {
    pub fn repr(self) -> &'static str {
        match self {
            Rotation::Rotate0 => "0°",
            Rotation::Rotate90 => "90°",
            Rotation::Rotate180 => "180°",
            Rotation::Rotate270 => "270°",
        }
    }
    pub fn size(self) -> Size {
        match self {
            Rotation::Rotate0 | Rotation::Rotate180 => Size::new(WIDTH as u32, HEIGHT as u32),
            Rotation::Rotate90 | Rotation::Rotate270 => Size::new(HEIGHT as u32, WIDTH as u32),
        }
    }
}

pub struct DisplayBuffers {
    framebuffer: [[u8; BUFFER_SIZE]; 2],
    active: bool,
    pub rotation: Rotation,
}

impl Default for DisplayBuffers {
    fn default() -> Self {
        Self::with_rotation(Rotation::Rotate0)
    }
}

impl DisplayBuffers {
    pub fn with_rotation(rotation: Rotation) -> Self {
        // Clear screen to white
        let mut ret = Self {
            framebuffer: [[0; BUFFER_SIZE]; 2],
            active: false,
            rotation,
        };
        ret.framebuffer[0].fill(0xFF);
        ret.framebuffer[1].fill(0xFF);
        ret
    }

    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
    }

    pub fn get_active_buffer_mut(&mut self) -> &mut [u8; BUFFER_SIZE] {
        if self.active {
            &mut self.framebuffer[1]
        } else {
            &mut self.framebuffer[0]
        }
    }

    pub fn get_active_buffer(&self) -> &[u8; BUFFER_SIZE] {
        if self.active {
            &self.framebuffer[1]
        } else {
            &self.framebuffer[0]
        }
    }

    pub fn get_inactive_buffer(&self) -> &[u8; BUFFER_SIZE] {
        if self.active {
            &self.framebuffer[0]
        } else {
            &self.framebuffer[1]
        }
    }

    pub fn clear_screen(&mut self, color: u8) {
        self.get_active_buffer_mut().fill(color);
    }

    pub fn swap_buffers(&mut self) {
        self.active = !self.active;
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: BinaryColor) {
        let size = self.size();
        if x < 0 || y < 0 || x as u32 >= size.width || y as u32 >= size.height {
            return;
        }
        let (x, y) = match self.rotation {
            Rotation::Rotate0 => (x as usize, y as usize),
            Rotation::Rotate90 => (y as usize, HEIGHT - 1 - x as usize),
            Rotation::Rotate180 => (WIDTH - 1 - x as usize, HEIGHT - 1 - y as usize),
            Rotation::Rotate270 => (WIDTH - 1 - y as usize, x as usize),
        };
        if x < WIDTH && y < HEIGHT {
            let index = y * WIDTH + x;
            let byte_index = index / 8;
            let bit_index = 7 - (index % 8);
            match color {
                BinaryColor::On => {
                    self.get_active_buffer_mut()[byte_index] |= 1 << bit_index;
                }
                BinaryColor::Off => {
                    self.get_active_buffer_mut()[byte_index] &= !(1 << bit_index);
                }
            }
        }
    }

    pub fn blit(&mut self, src: &[u8], w: u16, h: u16) {
        let Size { width, height } = self.size();

        let offset_x = (width as i32 - w as i32) / 2;
        let offset_y = (height as i32 - h as i32) / 2;

        for y in 0..h as _ {
            for x in 0..w as _ {
                let index = (y as usize * w as usize + x as usize) / 8;
                let bit_index = 7 - ((y as usize * w as usize + x as usize) % 8);
                let color = if (src[index] >> bit_index) & 1 == 1 {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                self.set_pixel(x as i32 + offset_x, y as i32 + offset_y, color);
            }
        }
    }
}

impl OriginDimensions for DisplayBuffers {
    fn size(&self) -> Size {
        self.rotation.size()
    }
}

impl DrawTarget for DisplayBuffers {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            self.set_pixel(coord.x, coord.y, color);
        }
        Ok(())
    }
}
