use trusty_core::fs::Filesystem;
use trusty_core::{application::Application, framebuffer::DisplayBuffers, io, zip};

use crate::minifb_display::MinifbDisplay;
use crate::std_fs::{StdFileReader, StdFilesystem};

mod minifb_display;
mod std_fs;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Trusty desktop application started");

    let mut display_buffers = Box::new(DisplayBuffers::default());
    let mut display = MinifbDisplay::default();
    let mut application = Application::new(&mut display_buffers);

    while display.is_open() {
        display.update();
        application.update(&display.get_buttons());
        application.draw(&mut display);
    }

    if (false) {
        let mut fs = StdFilesystem::new_with_base_path("sd".into());
        let mut reader = fs.open("ohler.epub").unwrap();
        let entries = zip::parse_zip(&mut reader).unwrap();
        for entry in &entries {
            log::info!("Found zip entry: {} (sz: {})", entry.name, entry.size);
            let reader = zip::ZipEntryReader::new(&mut reader, entry).unwrap();
            let contents = reader.read_to_end().unwrap();
            log::info!("  Entry data size: {}", contents.len());
        }
    }
}
