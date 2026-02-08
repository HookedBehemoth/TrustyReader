use alloc::boxed::Box;

use log::info;

use crate::activities::{ActivityType, home};
use crate::activities::demo::DemoActivity;
use crate::activities::filebrowser::FileBrowser;
use crate::activities::home::HomeActivity;
use crate::activities::settings::SettingsActivity;

use crate::display::RefreshMode;
use crate::res::img::bebop;
use crate::{
    activities::{Activity, ApplicationState},
    battery::ChargeState,
    framebuffer::DisplayBuffers,
    fs::Directory,
    input,
};

pub struct Application<'a, Filesystem>
where
    Filesystem: crate::fs::Filesystem,
{
    dirty: bool,
    display_buffers: &'a mut DisplayBuffers,
    filesystem: Filesystem,
    stack: heapless::Vec<ActivityType, 4>,
    activity: Box<dyn Activity>,
    activity_type: ActivityType,
    sleep: bool,
    ota: bool,
}

impl<'a, Filesystem> Application<'a, Filesystem>
where
    Filesystem: crate::fs::Filesystem,
{
    pub fn new(display_buffers: &'a mut DisplayBuffers, filesystem: Filesystem) -> Self {

        let activity_type = ActivityType::home();
        let activity = Box::new(HomeActivity::new(home::Focus::FileBrowser));

        Application {
            dirty: true,
            display_buffers,
            filesystem,
            stack: heapless::Vec::new(),
            activity,
            activity_type,
            sleep: false,
            ota: false,
        }
    }

    pub fn running(&self) -> bool {
        !self.sleep && !self.ota
    }

    pub fn ota_running(&self) -> bool {
        self.ota
    }

    pub fn update(&mut self, buttons: &input::ButtonState, charge: ChargeState) {
        if buttons.is_pressed(input::Buttons::Power) {
            self.sleep = true;
            return;
        }

        let state = ApplicationState {
            input: buttons.clone(),
            charge,
            rotation: self.display_buffers.rotation(),
        };

        match self.activity.update(&state) {
            crate::activities::UpdateResult::None => {}
            crate::activities::UpdateResult::Redraw => self.dirty = true,
            crate::activities::UpdateResult::SetRotation(rotation) => {
                self.display_buffers.set_rotation(rotation);
                self.dirty = true;
            }
            crate::activities::UpdateResult::PopActivity => {
                info!("Going back to previous activity");
                let Some(prev_activity) = self.stack.pop() else {
                    info!("No previous activity to go back to");
                    self.open(ActivityType::home());
                    return;
                };
                self.open(prev_activity);
            }
            crate::activities::UpdateResult::PushActivity { current, next } => {
                self.stack.push(current).ok();
                self.open(next);
            }
            crate::activities::UpdateResult::Ota => self.ota = true,
        }
    }

    pub fn draw(&mut self, display: &mut impl crate::display::Display) {
        if self.sleep {
            self.display_buffers
                .get_active_buffer_mut()
                .copy_from_slice(bebop::BEBOP);
            display.display(self.display_buffers, RefreshMode::Full);
            display.copy_grayscale_buffers(bebop::BEBOP_LSB, bebop::BEBOP_MSB);
            display.display_differential_grayscale(true);
            return;
        }
        if !self.dirty {
            return;
        }
        info!("Drawing activity");
        self.activity.draw(display, &mut self.display_buffers);
        self.dirty = false;
    }

    fn open(&mut self, activity_type: ActivityType) {
        self.activity = self.create_activity(&activity_type);
        self.activity.start();
        self.dirty = true;
        self.activity_type = activity_type;
    }

    fn create_activity(&self, activity_type: &ActivityType) -> Box<dyn Activity> {
        match activity_type {
            ActivityType::Home { state } => Box::new(HomeActivity::new(*state)),
            ActivityType::FileBrowser { focus, path } => {
                let dir = self.filesystem.open_directory(&path).unwrap();
                let entries = dir.list().unwrap();
                Box::new(FileBrowser::new(path.clone(), entries, *focus))
            }
            ActivityType::Settings => Box::new(SettingsActivity::new()),
            ActivityType::Demo => Box::new(DemoActivity::new()),
            // ActivityType::Reader { path } => Box::new(crate::activities::reader::ReaderActivity::new(path)),
            ActivityType::Reader { path } => unreachable!("TODO: Reader ({path})"),
        }
    }

}
