use alloc::boxed::Box;

use embedded_graphics::prelude::OriginDimensions;
use log::info;

use crate::activities::ActivityType;
use crate::activities::demo::DemoActivity;
use crate::activities::filebrowser::FileBrowser;
use crate::activities::home::HomeActivity;
use crate::activities::imageviewer::ImageViewerActivity;
use crate::activities::reader::ReaderActivity;
use crate::activities::settings::SettingsActivity;

use crate::container::image;
use crate::display::RefreshMode;
use crate::fs::DirEntry;
use crate::res::img::bebop;

use crate::{
    activities::{Activity, ApplicationState},
    battery::ChargeState,
    framebuffer::DisplayBuffers,
    fs::Directory,
    input,
};

type Stack = heapless::Vec<ActivityType, 8>;

pub struct Application<'a, Filesystem> 
where
    Filesystem: crate::fs::Filesystem + Clone + 'static,
{
    dirty: bool,
    display_buffers: &'a mut DisplayBuffers,
    filesystem: Filesystem,
    stack: Stack,
    activity: Option<Box<dyn Activity>>,
    sleep: bool,
    ota: bool,
}

const STACK_PATH: &str = ".trusty/stack";

impl<'a, Filesystem> Application<'a, Filesystem>
where
    Filesystem: crate::fs::Filesystem + Clone + 'static,
{
    pub fn new(display_buffers: &'a mut DisplayBuffers, filesystem: Filesystem) -> Self {
        if let Ok(stack) = filesystem.open_file(STACK_PATH, crate::fs::Mode::Read) {
            let mut buff = [0u8; 0x100];
            if let Ok((mut stack, _)) = postcard::from_eio::<'_, Stack, _>((stack, &mut buff)) {
                info!("Restored activity stack from previous session: {:?}", stack);
                if let Some(activity_type) = stack.pop() {
                    info!("Resuming last activity: {:?}", activity_type);
                    return Self::with_intent_and_stack(display_buffers, filesystem, activity_type, stack);
                }
                info!("No activity to resume, starting at home screen");
            } else {
                info!("Failed to restore activity stack from previous session");
            }
        }
        Self::with_intent(display_buffers, filesystem, ActivityType::home())
    }

    pub fn with_intent(
        display_buffers: &'a mut DisplayBuffers,
        filesystem: Filesystem,
        activity_type: ActivityType,
    ) -> Self {
        Self::with_intent_and_stack(display_buffers, filesystem, activity_type, Stack::new())
    }

    fn with_intent_and_stack(
        display_buffers: &'a mut DisplayBuffers,
        filesystem: Filesystem,
        activity_type: ActivityType,
        stack: Stack,
    ) -> Self {
        let mut activity = Self::create_activity(&activity_type, &filesystem);
        activity.start();

        Application {
            dirty: true,
            display_buffers,
            filesystem,
            stack,
            activity: Some(activity),
            sleep: false,
            ota: false,
        }
    }

    pub fn sleeping(&self) -> bool {
        self.sleep
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

        let rotation = self.display_buffers.rotation();
        let input = buttons.translated(rotation);
        let state = ApplicationState { input, charge, rotation };

        if self.activity.is_none() {
            return;
        }

        match self.activity.as_mut().unwrap().update(&state) {
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
                    self.open_activity(ActivityType::home());
                    return;
                };
                info!("Opening previous activity: {:?}", prev_activity);
                self.open_activity(prev_activity);
            }
            crate::activities::UpdateResult::PushActivity(next) => {
                self.flush_activity_to_stack();
                self.open_activity(next);
            }
            crate::activities::UpdateResult::Ota => self.ota = true,
        }
    }

    pub fn draw(&mut self, display: &mut impl crate::display::Display) {
        if self.sleep {
            self.draw_sleep(display);
            return;
        }
        if !self.dirty {
            return;
        }
        if let Some(activity) = &mut self.activity {
            info!("Drawing activity");
            activity.draw(display, self.display_buffers);
        }
        self.dirty = false;
    }

    fn open_activity(&mut self, activity_type: ActivityType) {
        self.close_activity();
        let mut activity = Self::create_activity(&activity_type, &self.filesystem);
        activity.start();
        self.activity = Some(activity);
        self.dirty = true;
    }

    fn close_activity(&mut self) {
        if let Some(mut current) = self.activity.take() {
            current.close();
        }
    }

    fn flush_activity_to_stack(&mut self) {
        if let Some(current) = &self.activity {
            let activity_type = current.to_activity_type();
            info!("Pushing activity to stack: {:?}", activity_type);
            let _ = self.stack.push(activity_type);
        }
        self.close_activity();
    }

    fn create_activity(activity_type: &ActivityType, filesystem: &Filesystem) -> Box<dyn Activity> {
        match activity_type {
            ActivityType::Home { state } => Box::new(HomeActivity::new(*state)),
            ActivityType::FileBrowser { focus, path } => {
                let dir = filesystem.open_directory(path).unwrap();
                let entries = dir.list().unwrap();
                Box::new(FileBrowser::new(path.clone(), entries, *focus))
            }
            ActivityType::Settings => Box::new(SettingsActivity::new()),
            ActivityType::Demo { screen } => Box::new(DemoActivity::new(*screen)),
            ActivityType::Reader { path } => {
                if let Some(format) = image::Format::guess_from_filename(path) {
                    Box::new(ImageViewerActivity::new(filesystem, path, format))
                } else {
                    Box::new(ReaderActivity::new(filesystem.clone(), path))
                }
            }
        }
    }

    fn draw_sleep(&mut self, display: &mut impl crate::display::Display) {
        // TODO: should this be an activity?
        // free all resources so we don't have to worry about memory
        self.flush_activity_to_stack();

        // attempt to draw custom sleep screen if available, otherwise fall back to static one
        if let Some(()) = self.draw_custom_sleep(display) {
            return;
        }

        self.draw_static_sleep(display);
    }

    fn draw_custom_sleep(&mut self, display: &mut impl crate::display::Display) -> Option<()> {
        let sleep_path = ".sleep";
        let fb_size = self.display_buffers.size();
        let sleep_dir = self.filesystem.open_directory(sleep_path).ok()?;
        let entries = sleep_dir.list().ok()?;
        for entry in entries.iter().filter(|e| !e.is_directory()) {
            let name = entry.name();

            log::debug!("Attempting to load sleep image: {}", name);

            let Some(image) = image::load_and_cache(&self.filesystem, sleep_path, entry, fb_size) else {
                continue;
            };

            let y_offset = fb_size.height.saturating_sub(image.height as _) / 2;
            log::info!("Loaded sleep screen from cache: {}", name);
            self.display_buffers.clear_screen(0xff);
            image.blit(y_offset as _, self.display_buffers);
            display.display(self.display_buffers, RefreshMode::Full);
            return Some(());
        }

        // no image found
        None
    }

    fn draw_static_sleep(&mut self, display: &mut impl crate::display::Display) {
        self.display_buffers
            .get_active_buffer_mut()
            .copy_from_slice(bebop::BEBOP);
        display.display(self.display_buffers, RefreshMode::Full);
        display.copy_grayscale_buffers(bebop::BEBOP_LSB, bebop::BEBOP_MSB);
        display.display_differential_grayscale(true);
    }
}

impl<'a, Filesystem> Drop for Application<'a, Filesystem>
where
    Filesystem: crate::fs::Filesystem + Clone + 'static,
{
    fn drop(&mut self) {
        self.flush_activity_to_stack();
        if let Ok(mut stack_file) = self.filesystem.open_file(STACK_PATH, crate::fs::Mode::Write) {
            postcard::to_eio(&self.stack, &mut stack_file).ok();
        }
    }
}
