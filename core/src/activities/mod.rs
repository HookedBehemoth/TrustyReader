use crate::{
    battery::ChargeState,
    display::Display,
    framebuffer::{DisplayBuffers, Rotation},
    input::ButtonState,
};

pub mod demo;
pub mod filebrowser;
pub mod home;
pub mod imageviewer;
pub mod reader;
pub mod settings;

pub type Path = heapless::String<256>;

#[derive(Clone)]
pub enum ActivityType {
    Home { state: home::Focus },
    FileBrowser { focus: u8, path: Path },
    Settings,
    Demo,
    Reader { path: Path },
}

impl ActivityType {
    pub fn home() -> Self {
        ActivityType::Home {
            state: home::Focus::FileBrowser,
        }
    }
    pub fn file_browser() -> Self {
        ActivityType::FileBrowser {
            focus: 0,
            path: heapless::String::new(),
        }
    }
    pub fn reader(path: &str) -> Self {
        ActivityType::Reader { path: path.try_into().unwrap() }
    }
}

pub enum UpdateResult {
    None,
    Redraw,
    SetRotation(Rotation),
    PopActivity,
    PushActivity {
        current: ActivityType,
        next: ActivityType,
    },
    Ota,
}

pub struct ApplicationState {
    pub input: ButtonState,
    pub charge: ChargeState,
    pub rotation: Rotation,
}

pub trait Activity {
    fn start(&mut self) {}
    fn close(&mut self) {}
    fn update(&mut self, state: &ApplicationState) -> UpdateResult;
    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers);
}
