use argh::FromArgs;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use trusty_core::{
    container::{tbmp, xt},
    framebuffer::{HEIGHT, WIDTH},
    fs::{Filesystem, Mode},
};

use crate::std_fs::StdFilesystem;

mod std_fs;

#[derive(FromArgs)]
/// Conversion options
struct Args {
    /// input image path
    #[argh(option, short = 'i')]
    input_path: String,

    /// output XT file path
    #[argh(option, short = 'o')]
    output_path: String,

    /// monochrome
    #[argh(switch, short = 'm')]
    monochrome: bool,

    /// output TBMP
    #[argh(switch, short = 't')]
    tbmp: bool,

    /// dither
    #[argh(switch, short = 'd')]
    dither: bool,
}

fn main() {
    let args: Args = argh::from_env();

    let mut image = image::open(&args.input_path).expect("Failed to open input image");

    let width = image.width() as usize;
    let height = image.height() as usize;
    if width > HEIGHT as _ || height > WIDTH as _ {
        image = image.resize(HEIGHT as _, WIDTH as _, FilterType::Lanczos3)
    }
    if args.tbmp {
        encode_tbmp(image, &args.output_path, args.dither);
    } else if args.monochrome {
        encode_xtg(image, &args.output_path);
    } else {
        encode_xth(image, &args.output_path);
    }
}

const BAYER_8X8_MATRIX: [[u8; 8]; 8] = [
    [00, 48, 12, 60, 03, 51, 15, 63],
    [32, 16, 44, 28, 35, 19, 47, 31],
    [08, 56, 04, 52, 11, 59, 07, 55],
    [40, 24, 36, 20, 43, 27, 39, 23],
    [02, 50, 14, 62, 01, 49, 13, 61],
    [34, 18, 46, 30, 33, 17, 45, 29],
    [10, 58, 06, 54, 09, 57, 05, 53],
    [42, 26, 38, 22, 41, 25, 37, 21],
];

fn encode_tbmp(img: DynamicImage, out_dir: &str, dither: bool) {
    let (width, height) = img.dimensions();
    let aligned_width = (width + 7) & 0xFFF8;
    let aligned_height = (height + 7) & 0xFFF8;
    let buffer_size = (aligned_width * aligned_height / 8) as usize;
    let image = img.into_luma8();
    let mut bw_buffer = vec![0u8; buffer_size];
    let mut msb_buffer = vec![0u8; buffer_size];
    let mut lsb_buffer = vec![0u8; buffer_size];
    for x in 0..aligned_width {
        for y in 0..aligned_height {
            if x >= width || y >= height {
                let byte_index = (y * (aligned_width / 8) + (x / 8)) as usize;
                let bit_index = 7 - (x % 8);
                bw_buffer[byte_index] |= 1 << bit_index;
                continue;
            }
            let pixel = image.get_pixel(x as u32, y as u32);
            let luma = pixel[0];
            let (bw, msb, lsb) = if dither {
                let threshold = BAYER_8X8_MATRIX[y as usize % 8][x as usize % 8] * 4;
                if luma >= threshold {
                    (1u8, 0u8, 0u8)
                } else if luma >= threshold.saturating_sub(64) {
                    (1u8, 0u8, 1u8)
                } else if luma >= threshold.saturating_sub(128) {
                    (0u8, 1u8, 0u8)
                } else if luma >= threshold.saturating_sub(196) {
                    (0u8, 1u8, 1u8)
                } else {
                    (0u8, 0u8, 0u8)
                }
            } else {
                if luma >= 205 {
                    (1u8, 0u8, 0u8)
                } else if luma >= 154 {
                    (1u8, 0u8, 1u8)
                } else if luma >= 103 {
                    (0u8, 1u8, 0u8)
                } else if luma >= 52 {
                    (0u8, 1u8, 1u8)
                } else {
                    (0u8, 0u8, 0u8)
                }
            };
            let byte_index = (y * (aligned_width / 8) + (x / 8)) as usize;
            let bit_index = 7 - (x % 8);
            bw_buffer[byte_index] |= bw << bit_index;
            msb_buffer[byte_index] |= msb << bit_index;
            lsb_buffer[byte_index] |= lsb << bit_index;
        }
    }

    let fs = StdFilesystem::new_with_base_path(".".into());
    let mut out = fs
        .open_file(out_dir, Mode::Write)
        .expect("Failed to create output XTH file");

    tbmp::write(
        &mut out,
        aligned_width as u16,
        aligned_height as u16,
        tbmp::Background::White,
        &bw_buffer,
        &msb_buffer,
        &lsb_buffer,
    )
    .expect("Failed to write TBMP file");
}

fn encode_xth(img: DynamicImage, out_dir: &str) {
    let (width, height) = img.dimensions();
    let buffer_size = (width * height / 8) as usize;
    let image = img.into_luma8();
    let mut buffer1 = vec![0u8; buffer_size];
    let mut buffer2 = vec![0u8; buffer_size];
    for x in 0..width {
        for y in 0..height {
            let pixel = image.get_pixel(x as u32, y as u32);
            let luma = pixel[0];
            let (bit1, bit2) = if luma < 64 {
                (1, 1)
            } else if luma < 128 {
                (0, 1)
            } else if luma < 192 {
                (1, 0)
            } else {
                (0, 0)
            };
            let byte_index = (y * (width / 8) + (x / 8)) as usize;
            let bit_index = 7 - (x % 8);
            buffer1[byte_index] |= bit1 << bit_index;
            buffer2[byte_index] |= bit2 << bit_index;
        }
    }

    let fs = StdFilesystem::new_with_base_path(".".into());
    let mut out = fs
        .open_file(out_dir, Mode::Write)
        .expect("Failed to create output XTH file");

    xt::write_xth(
        &mut out,
        &buffer1.try_into().unwrap(),
        &buffer2.try_into().unwrap(),
    )
    .expect("Failed to write XTH file");
}

fn encode_xtg(img: DynamicImage, out_dir: &str) {
    let (width, height) = img.dimensions();
    let buffer_size = (width * height / 8) as usize;
    let image = img.into_luma8();
    let mut buffer = vec![0u8; buffer_size];
    for x in 0..width {
        for y in 0..height {
            let pixel = image.get_pixel(x as u32, y as u32);
            let luma = pixel[0];
            let bit = if luma < 128 { 0 } else { 1 };
            let byte_index = (y * (width / 8) + (x / 8)) as usize;
            let bit_index = 7 - (x % 8);
            buffer[byte_index] |= bit << bit_index;
        }
    }

    let fs = StdFilesystem::new_with_base_path(".".into());
    let mut out = fs
        .open_file(out_dir, Mode::Write)
        .expect("Failed to create output XT file");

    xt::write_xtg(&mut out, &buffer.try_into().unwrap()).expect("Failed to write XT file");
}
