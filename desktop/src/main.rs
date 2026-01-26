use trusty_core::{
    application::Application,
    framebuffer::DisplayBuffers,
};

use crate::display::MinifbDisplay;

mod display;

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
}
