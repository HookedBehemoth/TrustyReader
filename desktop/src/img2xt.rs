use argh::FromArgs;
use image::DynamicImage;
use trusty_core::{
    container::xt,
    framebuffer::{BUFFER_SIZE, HEIGHT, WIDTH},
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
}

fn main() {
    let args: Args = argh::from_env();

    let image = image::open(&args.input_path).expect("Failed to open input image");

    let width = image.width() as usize;
    let height = image.height() as usize;
    if width > HEIGHT as _ || height > WIDTH as _ {
        panic!("Input image is too large (max {}x{})", WIDTH, HEIGHT);
    }
    if args.monochrome {
        encode_xtg(image, &args.output_path);
    } else {
        encode_xth(image, &args.output_path);
    }
}

fn encode_xth(img: DynamicImage, out_dir: &str) {
    let image = img.into_luma8();
    let mut buffer1 = vec![0u8; BUFFER_SIZE];
    let mut buffer2 = vec![0u8; BUFFER_SIZE];
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            let pixel = image.get_pixel(y as u32, x as u32);
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
            let byte_index = y * (WIDTH as usize / 8) + (x / 8);
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
    let image = img.into_luma8();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            let pixel = image.get_pixel(y as u32, x as u32);
            let luma = pixel[0];
            let bit = if luma < 128 { 0 } else { 1 };
            let byte_index = y * (WIDTH as usize / 8) + (x / 8);
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
