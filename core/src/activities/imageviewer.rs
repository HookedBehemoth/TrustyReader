use embedded_io::Seek;

use crate::{
    activities::ApplicationState,
    container::image::{self, Format},
    display::{Display, RefreshMode},
    framebuffer::{DisplayBuffers, Rotation},
    fs::{self, File},
    input::Buttons,
    res::font::Mode,
};

pub struct ImageViewerActivity<Filesystem: fs::Filesystem> {
    format: Format,
    image: Option<Result<image::Image, &'static str>>,
    file: Option<Filesystem::File>,
}

impl<Filesystem: fs::Filesystem> ImageViewerActivity<Filesystem> {
    pub fn new(fs: &Filesystem, path: &str, format: Format) -> Self {
        let file = fs.open_file(path, fs::Mode::Read).ok();

        ImageViewerActivity { format, image: None, file }
    }

    fn load_image(&mut self, rotation: Rotation) {
        let Some(file) = &mut self.file else {
            return;
        };
        file.seek(embedded_io::SeekFrom::Start(0)).ok();
        let sz = rotation.size();
        let file_size = file.size() as _;
        self.image = match image::decode(self.format, file, file_size, sz.width as _, sz.height as _) {
            Ok(img) => Some(Ok(img)),
            Err(e) => {
                log::error!("Failed to decode image: {}", e);
                Some(Err(e))
            },
        }
    }
}

impl<Filesystem: fs::Filesystem> super::Activity for ImageViewerActivity<Filesystem> {
    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        log::info!("Drawing ImageViewerActivity");
        let Some(file) = &mut self.file else {
            return;
        };
        let Some(Ok(image)) = &self.image else {
            return;
        };
        log::info!("Blitting image to display");

        buffers.clear_screen(0xFF);
        image.blit_bw(file, 0, buffers);
        display.display(buffers, RefreshMode::Fast);

        if image.has_grayscale() {
            buffers.clear_screen(0x00);
            image.blit_gray(file, 0, Mode::Msb, buffers);
            display.copy_to_msb(buffers.get_active_buffer());

            buffers.clear_screen(0x00);
            image.blit_gray(file, 0, Mode::Lsb, buffers);
            display.copy_to_lsb(buffers.get_active_buffer());
            display.display_differential_grayscale(false);
        }
    }

    fn update(&mut self, state: &ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;
        if self.image.is_none() {
            self.load_image(state.rotation);
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Back) {
            super::UpdateResult::PopActivity
        } else if buttons.is_pressed(Buttons::Right) {
            let rotation = match state.rotation {
                Rotation::Rotate0 => Rotation::Rotate90,
                Rotation::Rotate90 => Rotation::Rotate180,
                Rotation::Rotate180 => Rotation::Rotate270,
                Rotation::Rotate270 => Rotation::Rotate0,
            };
            self.load_image(rotation);
            super::UpdateResult::SetRotation(rotation)
        } else {
            super::UpdateResult::None
        }
    }
}
