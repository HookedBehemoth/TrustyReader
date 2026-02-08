use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::{OriginDimensions, Point},
    text::Text,
};

use crate::{
    battery::ChargeState, display::{Display, RefreshMode}, framebuffer::DisplayBuffers, input::Buttons
};

#[derive(Clone, Copy, PartialEq, Eq, rotate_enum::RotateEnum, strum_macros::EnumIter)]
pub enum Focus {
    FileBrowser,
    Demo,
    Settings,
}

impl Focus {
    fn label(&self) -> &'static str {
        match self {
            Focus::FileBrowser => "File Browser",
            Focus::Demo => "Demo",
            Focus::Settings => "Settings",
        }
    }
}

pub struct HomeActivity {
    focus: Focus,
    charge: ChargeState,
}

impl HomeActivity {
    pub fn new(focus: Focus) -> Self {
        Self { focus, charge: ChargeState::default() }
    }
}

impl super::Activity for HomeActivity {
    fn start(&mut self) {
        log::info!("HomeActivity started");
    }

    fn update(&mut self, state: &super::ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;
        if buttons.any_pressed(&[Buttons::Up, Buttons::Right]) {
            self.focus = self.focus.prev();
            super::UpdateResult::Redraw
        } else if buttons.any_pressed(&[Buttons::Down, Buttons::Left]) {
            self.focus = self.focus.next();
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Confirm) {
            let current = super::ActivityType::Home { state: self.focus };
            match self.focus {
                Focus::FileBrowser => super::UpdateResult::PushActivity { current, next: super::ActivityType::file_browser() },
                Focus::Demo => super::UpdateResult::PushActivity { current, next: super::ActivityType::Demo },
                Focus::Settings => super::UpdateResult::PushActivity { current, next: super::ActivityType::Settings },
            }
        } else if state.charge != self.charge {
            self.charge = state.charge;
            super::UpdateResult::Redraw
        } else {
            // No interaction for now
            super::UpdateResult::None
        }
    }

    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers.clear_screen(0xFF);

        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        Text::new("Home", Point::new(20, 30), text_style)
            .draw(buffers)
            .ok();

        for option in [ Focus::FileBrowser, Focus::Demo, Focus::Settings] {
            Text::new(
                option.label(),
                Point::new(20, 60 + (option as i32) * 30),
                text_style,
            )
            .draw(buffers)
            .ok();
            if option == self.focus {
                Text::new(">", Point::new(5, 60 + (option as i32) * 30), text_style)
                    .draw(buffers)
                    .ok();
            }
        }

        let size = buffers.size();
        let battery_pos = Point::new(size.width as i32 - 55, size.height as i32 - 20);

        let charge = self.charge.format();
        Text::new(&charge, battery_pos, text_style)
            .draw(buffers)
            .ok();

        display.display(buffers, RefreshMode::Fast);
    }
}
