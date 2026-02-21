use trusty_core::activities::ActivityType;
use trusty_core::battery::ChargeState;
use trusty_core::fs::Filesystem;
use trusty_core::{application::Application, framebuffer::DisplayBuffers, zip};

use crate::minifb_display::MinifbDisplay;
use crate::std_fs::StdFilesystem;

mod minifb_display;
mod std_fs;

/// Launch args
#[derive(argh::FromArgs)]
struct Args {
    /// path to the base directory to use for the filesystem (e.g. SD card mount point)
    #[argh(option, default = "\"sd\".to_string()")]
    fs_base_path: String,

    /// file to open on startup (relative to the base path)
    #[argh(option, short = 'f')]
    file_to_open: Option<String>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Args = argh::from_env();

    log::info!("Trusty desktop application started");

    let intent = if let Some(file) = args.file_to_open.clone() {
        log::info!("Opening file on startup: {}", file);
        ActivityType::reader(&file)
    } else {
        ActivityType::home()
    };

    let mut display_buffers = Box::new(DisplayBuffers::default());
    let mut display = MinifbDisplay::default();
    let fs = StdFilesystem::new_with_base_path(args.fs_base_path.into());
    let mut application = Application::with_intent(&mut display_buffers, fs, intent);
    let charge = ChargeState { level: 75, charging: true };

    while display.is_open() {
        display.update();
        application.update(&display.get_buttons(), charge);
        application.draw(&mut display);
    }

    if false {
        let fs = StdFilesystem::new_with_base_path("sd".into());
        let mut reader = fs
            .open_file("ohler.epub", trusty_core::fs::Mode::Read)
            .unwrap();
        let entries = zip::parse_zip(&mut reader).unwrap();
        for entry in &entries {
            log::info!("Found zip entry: {} (sz: {})", entry.name, entry.size);
            let reader = zip::ZipEntryReader::new(&mut reader, entry).unwrap();
            let contents = reader.read_to_end().unwrap();
            log::info!("  Entry data size: {}", contents.len());
        }
    }
}
