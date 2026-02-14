use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::Point,
    text::Text,
};
use strum::IntoEnumIterator;

use crate::{
    display::{Display, RefreshMode},
    framebuffer::DisplayBuffers,
    input::Buttons,
};

#[derive(Clone, Copy, PartialEq, Eq, rotate_enum::RotateEnum, strum_macros::EnumIter)]
pub enum Focus {
    SwitchOta,
}

impl Focus {
    pub fn label(&self) -> &'static str {
        match self {
            Focus::SwitchOta => "Switch OTA Partition",
        }
    }
}

pub struct SettingsActivity {
    focus: Focus,
}

impl SettingsActivity {
    pub fn new() -> Self {
        Self { focus: Focus::SwitchOta }
    }

    pub fn select(&mut self) -> super::UpdateResult {
        match self.focus {
            Focus::SwitchOta => super::UpdateResult::Ota,
        }
    }
}

impl super::Activity for SettingsActivity {
    fn start(&mut self) {
        log::info!("SettingsActivity started");
    }

    fn update(&mut self, state: &super::ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;
        if buttons.is_pressed(Buttons::Confirm) {
            self.select()
        } else if buttons.any_pressed(&[Buttons::Up, Buttons::Right]) {
            self.focus = self.focus.prev();
            super::UpdateResult::Redraw
        } else if buttons.any_pressed(&[Buttons::Down, Buttons::Left]) {
            self.focus = self.focus.next();
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Back) {
            super::UpdateResult::PopActivity
        } else {
            super::UpdateResult::None
        }
    }

    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers.clear_screen(0xFF);

        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        Text::new("Settings", Point::new(20, 30), text_style)
            .draw(buffers)
            .ok();

        for (i, entry) in Focus::iter().enumerate() {
            Text::new(
                entry.label(),
                Point::new(20, 60 + (i as i32) * 30),
                text_style,
            )
            .draw(buffers)
            .ok();

            if self.focus == entry {
                Text::new(">", Point::new(5, 60 + (i as i32) * 30), text_style)
                    .draw(buffers)
                    .ok();
            }
        }

        display.display(buffers, RefreshMode::Fast);
    }
}
