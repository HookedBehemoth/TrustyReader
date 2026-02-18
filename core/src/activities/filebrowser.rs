use alloc::vec::Vec;
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::Point,
    text::Text,
};
use log::info;

use crate::{
    activities::Path,
    display::{Display, RefreshMode},
    framebuffer::DisplayBuffers,
    input::Buttons,
};

struct WrappingNumber {
    value: u8,
    max: u8,
}

impl core::ops::Deref for WrappingNumber {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl WrappingNumber {
    fn next(&self) -> Self {
        if self.value < self.max {
            Self {
                value: self.value + 1,
                max: self.max,
            }
        } else {
            Self { value: 0, max: self.max }
        }
    }

    fn prev(&self) -> Self {
        if self.value > 0 {
            Self {
                value: self.value - 1,
                max: self.max,
            }
        } else {
            Self { value: self.max, max: self.max }
        }
    }
}

pub struct FileBrowser<Entry: crate::fs::DirEntry> {
    path: Path,
    entries: Vec<Entry>,
    focus: WrappingNumber,
}

impl<FileEntry: crate::fs::DirEntry> FileBrowser<FileEntry> {
    pub fn new(path: Path, entries: Vec<FileEntry>, focus: u8) -> Self {
        let focus = WrappingNumber {
            value: focus,
            max: entries.len().saturating_sub(1) as u8,
        };
        Self { path, entries, focus }
    }
}

impl<FileEntry: crate::fs::DirEntry> super::Activity for FileBrowser<FileEntry> {
    fn start(&mut self) {
        log::info!("FileBrowser started");
    }

    fn update(&mut self, state: &super::ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;
        if buttons.is_pressed(Buttons::Back) {
            super::UpdateResult::PopActivity
        } else if buttons.any_pressed(&[Buttons::Up, Buttons::Right]) {
            self.focus = self.focus.prev();
            super::UpdateResult::Redraw
        } else if buttons.any_pressed(&[Buttons::Down, Buttons::Left]) {
            self.focus = self.focus.next();
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Confirm) {
            let Some(entry) = &self.entries.get(*self.focus as usize) else { return super::UpdateResult::None; };
            let separator = if !self.path.is_empty() { "/" } else { "" };
            let Ok(path) = heapless::format!("{}{separator}{}", self.path, entry.name()) else {
                info!(
                    "Failed to construct path for {} + {}",
                    self.path,
                    entry.name()
                );
                return super::UpdateResult::None;
            };
            let current = super::ActivityType::FileBrowser {
                focus: *self.focus,
                path: self.path.clone(),
            };
            if entry.is_directory() {
                let next = super::ActivityType::FileBrowser { focus: 0, path };
                super::UpdateResult::PushActivity { current, next }
            } else {
                let next = super::ActivityType::Reader { path };
                super::UpdateResult::PushActivity { current, next }
            }
        } else {
            super::UpdateResult::None
        }
    }

    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers.clear_screen(0xFF);

        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        Text::new("File Browser", Point::new(20, 30), text_style)
            .draw(buffers)
            .ok();

        for (i, entry) in self.entries.iter().enumerate() {
            let pos = Text::new(
                entry.name(),
                Point::new(20, 60 + (i as i32) * 30),
                text_style,
            )
            .draw(buffers)
            .unwrap();
            if entry.is_directory() {
                Text::new("/", pos, text_style).draw(buffers).ok();
            }

            if i as u8 == *self.focus {
                Text::new(">", Point::new(5, 60 + (i as i32) * 30), text_style)
                    .draw(buffers)
                    .ok();
            }
        }

        display.display(buffers, RefreshMode::Fast);
    }
}
