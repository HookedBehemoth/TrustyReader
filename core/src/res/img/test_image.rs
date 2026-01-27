// Image dimensions: 800x480
// Black/White encoding: 1-bit, 8 pixels per byte
// Bit values: 0=Black, 1=White
// Grayscale encoding: 2-bit grayscale split into LSB and MSB arrays (1 bit per pixel each)
// Colors: 00=White, 01=Light Gray, 10=Gray, 11=Dark Gray
// Ranges: 0-51=White, 52-102=Dark Gray, 103-153=Gray, 154-204=Light Gray, 205-255=White
// LSB array: least significant bit of each pixel
// MSB array: most significant bit of each pixel

use crate::framebuffer::BUFFER_SIZE;

pub static TEST_IMAGE: &'static [u8; BUFFER_SIZE] = include_bytes!("./test_image.bin");
pub static TEST_IMAGE_LSB: &'static [u8; BUFFER_SIZE] = include_bytes!("./test_image_lsb.bin");
pub static TEST_IMAGE_MSB: &'static [u8; BUFFER_SIZE] = include_bytes!("./test_image_msb.bin");