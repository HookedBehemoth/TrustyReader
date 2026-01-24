#![feature(generic_const_exprs)]

use microreader_core::{
    application::{self, Application},
    display::{Display, HEIGHT, WIDTH},
};

use crate::display::MinifbDisplay;

mod display;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Microreader desktop application started");

    // const WIDTH: u32 = 800;
    // const HEIGHT: u32 = 480;
    // let mut buffer: Vec<u32> = vec![0; (WIDTH * HEIGHT) as usize];

    let mut window = minifb::Window::new(
        "Microreader Desktop",
        // swapped
        WIDTH as usize,
        HEIGHT as usize,
        minifb::WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("Unable to open window: {}", e);
    });

    window.set_target_fps(2);

    let mut display = Box::new(MinifbDisplay::new(window));
    let mut application = Application::new();

    while display.is_open() {
        application.draw(&mut *display);
    }
}
