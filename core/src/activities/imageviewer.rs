use alloc::vec;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::DrawTarget};

use crate::{
    activities::ApplicationState,
    container::{image, tbmp},
    display::{Display, RefreshMode},
    framebuffer::{DisplayBuffers, Rotation},
    fs,
    input::Buttons,
    res::font::Mode,
};

pub struct ImageViewerActivity<Filesystem: fs::Filesystem> {
    image: image::Image,
    file: Option<Filesystem::File>,
}

impl<Filesystem: fs::Filesystem> ImageViewerActivity<Filesystem> {
    pub fn new(fs: &Filesystem, path: &str) -> Self {
        let mut file = fs.open_file(path, fs::Mode::Read).unwrap();

        let header = tbmp::parse_header(&mut file).unwrap();
        let image = image::Image::Tbmp(header);

        ImageViewerActivity { image, file: Some(file) }
    }
}

impl<Filesystem: fs::Filesystem> super::Activity for ImageViewerActivity<Filesystem> {
    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        let Some(file) = &mut self.file else {
            return;
        };
        let header = match self.image {
            image::Image::Tbmp(header) => header,
            _ => return,
        };
        let clear = match header.background {
            tbmp::Background::White => BinaryColor::On,
            tbmp::Background::Black => BinaryColor::Off,
        };
        let mut buffer = vec![0u8; header.buffer_size()];

        buffers.clear(clear).ok();
        tbmp::load_buffer(file, &header, Mode::Bw, &mut buffer).ok();
        buffers.blit(&buffer, header.width, header.height);
        display.display(buffers, RefreshMode::Fast);

        buffers.clear(BinaryColor::Off).ok();
        tbmp::load_buffer(file, &header, Mode::Msb, &mut buffer).ok();
        buffers.blit(&buffer, header.width, header.height);
        display.copy_to_msb(buffers.get_active_buffer());

        buffers.clear(BinaryColor::Off).ok();
        tbmp::load_buffer(file, &header, Mode::Lsb, &mut buffer).ok();
        buffers.blit(&buffer, header.width, header.height);
        display.copy_to_lsb(buffers.get_active_buffer());
        display.display_differential_grayscale(false);
    }

    fn update(&mut self, state: &ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;
        if buttons.is_pressed(Buttons::Back) {
            super::UpdateResult::PopActivity
        } else if buttons.is_pressed(Buttons::Right) {
            super::UpdateResult::SetRotation(match state.rotation {
                Rotation::Rotate0 => Rotation::Rotate90,
                Rotation::Rotate90 => Rotation::Rotate180,
                Rotation::Rotate180 => Rotation::Rotate270,
                Rotation::Rotate270 => Rotation::Rotate0,
            })
        } else {
            super::UpdateResult::None
        }
    }
}
