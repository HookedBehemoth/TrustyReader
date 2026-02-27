use trusty_core::activities::ActivityType;
use trusty_core::battery::ChargeState;
use trusty_core::framebuffer;
use trusty_core::{application::Application, framebuffer::DisplayBuffers};

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

    /// starting rotation (0, 90, 180, 270)
    #[argh(option, short = 'r', default = "90")]
    rotation: u16,

    /// scale factor (1, 2, 4, 8)
    #[argh(option, short = 's', default = "1")]
    scale: u8,
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

    let rotation = match args.rotation {
        0 => framebuffer::Rotation::Rotate0,
        90 => framebuffer::Rotation::Rotate90,
        180 => framebuffer::Rotation::Rotate180,
        270 => framebuffer::Rotation::Rotate270,
        _ => panic!("Invalid rotation")
    };
    let scale = match args.scale {
        1 => minifb::Scale::X1,
        2 => minifb::Scale::X2,
        4 => minifb::Scale::X4,
        8 => minifb::Scale::X8,
        _ => panic!("Invalid scale")
    };
    let mut display_buffers = Box::new(DisplayBuffers::with_rotation(rotation));
    let mut display = MinifbDisplay::new(rotation, scale);
    let fs = StdFilesystem::new_with_base_path(args.fs_base_path.into());
    let mut application = Application::with_intent(&mut display_buffers, fs, intent);
    let charge = ChargeState { level: 75, charging: true };

    while display.is_open() {
        display.update();
        application.update(&display.get_buttons(), charge);
        application.draw(&mut display);
    }
}
