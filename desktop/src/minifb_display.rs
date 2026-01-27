use log::info;
use trusty_core::{
    display::{GrayscaleMode, RefreshMode},
    framebuffer::{DisplayBuffers, HEIGHT, Rotation, WIDTH},
    input::{ButtonState, Buttons},
};

const BUFFER_SIZE: usize = WIDTH * HEIGHT / 8;
const DISPLAY_BUFFER_SIZE: usize = WIDTH * HEIGHT;

pub struct MinifbDisplay {
    is_grayscale: bool,
    // Simulated EInk buffers
    lsb_buffer: Box<[u8; BUFFER_SIZE]>,
    msb_buffer: Box<[u8; BUFFER_SIZE]>,
    // Actual display buffer
    display_buffer: Box<[u32; DISPLAY_BUFFER_SIZE]>,
    window: minifb::Window,
    buttons: ButtonState,
    internal_rotation: Rotation,
    scale: minifb::Scale,
}

#[derive(PartialEq, Eq, Debug)]
enum BlitMode {
    // Blit the active framebuffer as full black/white
    Full,
    Partial,
    // Blit the difference between LSB and MSB buffers
    Grayscale,
    // Revert Greyscale to black/white
    GrayscaleRevert,
    GrayscaleOneshot,
}

impl Default for MinifbDisplay {
    fn default() -> Self {
        let mut ret = Self {
            is_grayscale: false,
            lsb_buffer: Box::new([0; BUFFER_SIZE]),
            msb_buffer: Box::new([0; BUFFER_SIZE]),
            display_buffer: Box::new([0; DISPLAY_BUFFER_SIZE]),
            window: Self::create_window(Rotation::Rotate90, minifb::Scale::X2),
            buttons: ButtonState::default(),
            internal_rotation: Rotation::Rotate90,
            scale: minifb::Scale::X2,
        };

        ret.display_buffer.fill(0xFFFFFFFF);

        ret
    }
}

impl MinifbDisplay {
    fn create_window(rotation: Rotation, scale: minifb::Scale) -> minifb::Window {
        let (width, height) = match rotation {
            Rotation::Rotate0 | Rotation::Rotate180 => (WIDTH, HEIGHT),
            Rotation::Rotate90 | Rotation::Rotate270 => (HEIGHT, WIDTH),
        };

        let options = minifb::WindowOptions {
            borderless: false,
            title: true,
            resize: true,
            scale,
            ..minifb::WindowOptions::default()
        };
        let mut window = minifb::Window::new("Trusty Desktop", width, height, options)
            .unwrap_or_else(|e| {
                panic!("Unable to open window: {}", e);
            });

        window.set_target_fps(5);
        window
    }

    pub fn is_open(&self) -> bool {
        self.window.is_open() && !self.window.is_key_down(minifb::Key::Escape)
    }

    pub fn update_display(&mut self /*, window: &mut minifb::Window */) {
        let (width, height) = match self.internal_rotation {
            Rotation::Rotate0 | Rotation::Rotate180 => (WIDTH, HEIGHT),
            Rotation::Rotate90 | Rotation::Rotate270 => (HEIGHT, WIDTH),
        };
        self.window
            .update_with_buffer(&*self.display_buffer, width, height)
            .unwrap();
    }

    fn get_display_idx(fb_idx: usize, rotation: Rotation) -> usize {
        match rotation {
            Rotation::Rotate0 => fb_idx,
            Rotation::Rotate90 => {
                let x = fb_idx % WIDTH;
                let y = fb_idx / WIDTH;
                (x * HEIGHT) + (HEIGHT - y - 1)
            }
            Rotation::Rotate180 => WIDTH * HEIGHT - fb_idx - 1,
            Rotation::Rotate270 => {
                let x = fb_idx % WIDTH;
                let y = fb_idx / WIDTH;
                ((WIDTH - x - 1) * HEIGHT) + y
            }
        }
    }

    fn get_pixel(&self, fb_idx: usize) -> u32 {
        let display_idx = Self::get_display_idx(fb_idx, self.internal_rotation);
        self.display_buffer[display_idx]
    }

    /// Set a pixel in the display buffer
    fn set_pixel(&mut self, fb_idx: usize, value: u32) {
        let display_idx = Self::get_display_idx(fb_idx, self.internal_rotation);
        self.display_buffer[display_idx] = value;
    }

    pub fn update(&mut self) {
        self.window.update();
        let mut current: u8 = 0;
        if self.window.is_key_down(minifb::Key::Left) {
            current |= 1 << (Buttons::Left as u8);
        }
        if self.window.is_key_down(minifb::Key::Right) {
            current |= 1 << (Buttons::Right as u8);
        }
        if self.window.is_key_down(minifb::Key::Up) {
            current |= 1 << (Buttons::Up as u8);
        }
        if self.window.is_key_down(minifb::Key::Down) {
            current |= 1 << (Buttons::Down as u8);
        }
        if self.window.is_key_down(minifb::Key::Enter) {
            current |= 1 << (Buttons::Confirm as u8);
        }
        if self.window.is_key_down(minifb::Key::Backspace) {
            current |= 1 << (Buttons::Back as u8);
        }
        if self.window.is_key_down(minifb::Key::M) {
            info!("Rotating display");
            let old_rotation = self.internal_rotation;
            let new_rotation = match self.internal_rotation {
                Rotation::Rotate0 => Rotation::Rotate90,
                Rotation::Rotate90 => Rotation::Rotate180,
                Rotation::Rotate180 => Rotation::Rotate270,
                Rotation::Rotate270 => Rotation::Rotate0,
            };
            // Rotate display buffer
            let mut new_display_buffer = Box::new([0; DISPLAY_BUFFER_SIZE]);
            for fb_idx in 0..(WIDTH * HEIGHT) {
                new_display_buffer[Self::get_display_idx(fb_idx, new_rotation)] =
                    self.display_buffer[Self::get_display_idx(fb_idx, old_rotation)];
            }
            self.display_buffer = new_display_buffer;
            self.window = Self::create_window(new_rotation, self.scale);
            self.internal_rotation = new_rotation;
            self.update_display();
        }
        if self.window.is_key_down(minifb::Key::S) {
            info!("Toggling scale");
            self.scale = match self.scale {
                minifb::Scale::X1 => minifb::Scale::X2,
                minifb::Scale::X2 => minifb::Scale::X4,
                minifb::Scale::X4 => minifb::Scale::X1,
                _ => minifb::Scale::X2,
            };
            self.window = Self::create_window(self.internal_rotation, self.scale);
            self.update_display();
        }
        self.buttons.update(current);
    }

    pub fn get_buttons(&self) -> ButtonState {
        self.buttons
    }

    fn blit_internal(&mut self, mode: BlitMode) {
        info!("Blitting with mode: {:?}", mode);
        match mode {
            BlitMode::Full => {
                for i in 0..self.lsb_buffer.len() {
                    let byte = self.lsb_buffer[i];
                    for bit in 0..8 {
                        let pixel_index = i * 8 + bit;
                        let pixel_value = if (byte & (1 << (7 - bit))) != 0 {
                            0xFFFFFFFF
                        } else {
                            0xFF000000
                        };
                        self.set_pixel(pixel_index, pixel_value);
                    }
                }
            }
            BlitMode::Partial => {
                for i in 0..self.lsb_buffer.len() {
                    let curr_byte = self.lsb_buffer[i];
                    let prev_byte = self.msb_buffer[i];
                    for bit in 0..8 {
                        let current_bit = (curr_byte >> (7 - bit)) & 0x01;
                        let previous_bit = (prev_byte >> (7 - bit)) & 0x01;
                        if current_bit == previous_bit {
                            continue;
                        }
                        if current_bit == 1 {
                            let pixel_index = i * 8 + bit;
                            self.set_pixel(pixel_index, 0xFFFFFFFF);
                        } else {
                            let pixel_index = i * 8 + bit;
                            self.set_pixel(pixel_index, 0xFF000000);
                        }
                    }
                }
            }
            BlitMode::Grayscale => {
                for i in 0..self.lsb_buffer.len() {
                    let lsb_byte = self.lsb_buffer[i];
                    let msb_byte = self.msb_buffer[i];
                    for bit in 0..8 {
                        let pixel_index = i * 8 + bit;
                        let lsb_bit = (lsb_byte >> (7 - bit)) & 0x01;
                        let msb_bit = (msb_byte >> (7 - bit)) & 0x01;
                        let current_pixel = self.get_pixel(pixel_index);
                        let new_pixel = match (msb_bit, lsb_bit) {
                            (0, 0) => continue,
                            (0, 1) => current_pixel.saturating_sub(0x555555), // Black -> Dark Gray
                            (1, 0) => current_pixel.saturating_sub(0xAAAAAA), // Black -> Gray
                            (1, 1) => current_pixel.saturating_add(0x333333), // White -> Light Gray
                            _ => unreachable!(),
                        };
                        self.set_pixel(pixel_index, new_pixel);
                    }
                }
            }
            BlitMode::GrayscaleRevert => {
                for i in 0..self.lsb_buffer.len() {
                    let lsb_byte = self.lsb_buffer[i];
                    let msb_byte = self.msb_buffer[i];
                    for bit in 0..8 {
                        let pixel_index = i * 8 + bit;
                        let lsb_bit = (lsb_byte >> (7 - bit)) & 0x01;
                        let msb_bit = (msb_byte >> (7 - bit)) & 0x01;
                        let current_pixel = self.get_pixel(pixel_index);
                        let new_pixel = match (msb_bit, lsb_bit) {
                            (0, 0) => continue,
                            (0, 1) => current_pixel.saturating_add(0x555555), // Dark Gray  -> Black
                            (1, 0) => current_pixel.saturating_add(0xAAAAAA), // Gray       -> Black
                            (1, 1) => current_pixel.saturating_sub(0x333333), // Light Gray -> White
                            _ => unreachable!(),
                        };
                        self.set_pixel(pixel_index, new_pixel);
                    }
                }
            }
            BlitMode::GrayscaleOneshot => {
                for i in 0..self.lsb_buffer.len() {
                    let lsb_byte = self.lsb_buffer[i];
                    let msb_byte = self.msb_buffer[i];
                    for bit in 0..8 {
                        let pixel_index = i * 8 + bit;
                        let lsb_bit = (lsb_byte >> (7 - bit)) & 0x01;
                        let msb_bit = (msb_byte >> (7 - bit)) & 0x01;
                        let new_pixel = match (msb_bit, lsb_bit) {
                            (0, 0) => 0xFFFFFFFF, // Black
                            (0, 1) => 0xFFAAAAAA, // Dark Gray
                            (1, 0) => 0xFF555555, // Gray
                            (1, 1) => 0xFF000000, // White
                            _ => unreachable!(),
                        };
                        self.set_pixel(pixel_index, new_pixel);
                    }
                }
            }
        }
        self.update_display();
    }
}

impl trusty_core::display::Display for MinifbDisplay {
    fn display(&mut self, buffers: &mut DisplayBuffers, mode: RefreshMode) {
        // revert grayscale first
        if self.is_grayscale {
            self.blit_internal(BlitMode::GrayscaleRevert);
            self.is_grayscale = false;
        }

        let current = buffers.get_active_buffer();
        let previous = buffers.get_inactive_buffer();
        self.lsb_buffer.copy_from_slice(&current[..]);
        self.msb_buffer.copy_from_slice(&previous[..]);
        if mode == RefreshMode::Fast {
            self.blit_internal(BlitMode::Partial);
        } else {
            self.blit_internal(BlitMode::Full);
        }
        buffers.swap_buffers();
    }
    fn copy_to_lsb(&mut self, buffers: &[u8; BUFFER_SIZE]) {
        self.lsb_buffer.copy_from_slice(buffers);
    }
    fn copy_to_msb(&mut self, buffers: &[u8; BUFFER_SIZE]) {
        self.msb_buffer.copy_from_slice(buffers);
    }
    fn copy_grayscale_buffers(&mut self, lsb: &[u8; BUFFER_SIZE], msb: &[u8; BUFFER_SIZE]) {
        self.lsb_buffer.copy_from_slice(lsb);
        self.msb_buffer.copy_from_slice(msb);
    }
    fn display_differential_grayscale(&mut self) {
        self.is_grayscale = true;
        self.blit_internal(BlitMode::Grayscale);
    }
    fn display_absolute_grayscale(&mut self, _: GrayscaleMode) {
        self.blit_internal(BlitMode::GrayscaleOneshot);
    }
}
