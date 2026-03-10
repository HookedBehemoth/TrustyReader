use std::hint::black_box;

use trusty_core::{container::{epub, image}, fs::Filesystem};

use crate::std_fs::StdFilesystem;


mod std_fs;

/// open options
#[derive(argh::FromArgs)]
struct Args {
    /// file to open on startup (relative to the base path)
    #[argh(positional)]
    file_to_open: String,
}

fn main() {
    let args: Args = argh::from_env();

    let fs = StdFilesystem::new_with_base_path("".into());
    let mut file = fs.open_file(&args.file_to_open, trusty_core::fs::Mode::Read).unwrap();
    
    let start = std::time::Instant::now();
    let book = epub::parse(&mut file).unwrap();
    let parse_duration = std::time::Instant::now() - start;
    println!("Parsed book '{}' in {} us", book.metadata.title, parse_duration.as_micros());

    // Parse all chapters
    let start = std::time::Instant::now();
    for i in 0..book.spine.len() {
        let _ = black_box(epub::parse_chapter(&book, i, &mut file));
    }
    let chapters_duration = std::time::Instant::now() - start;
    println!("Parsed {} chapters in {} us", book.spine.len(), chapters_duration.as_micros());

    let max = (800u16, 480u16);

    // Decode all images
    let entries = book.file_resolver.entries();
    let mut image_count = 0;
    let mut total_image_duration = std::time::Duration::ZERO;
    for idx in 0..entries.len() {
        if image::Format::guess_from_filename(&entries[idx].name).is_some() {
            let start = std::time::Instant::now();
            let _ = black_box(epub::parse_image(
                &book,
                idx as u16,
                max,
                &mut file,
            ));
            total_image_duration += std::time::Instant::now() - start;
            image_count += 1;
        }
    }
    let avg = total_image_duration.as_micros() / image_count;
    println!("Parsed {image_count} in {} ms (AVG.: {} us)", total_image_duration.as_millis(), avg);
}