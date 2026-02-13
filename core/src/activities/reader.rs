use alloc::string::String;
use log::info;

use crate::container::epub;

pub struct ReaderActivity<Filesystem>
where
    Filesystem: crate::fs::Filesystem,
{
    filesystem: Filesystem,
    file_path: String,
    file: Option<<Filesystem as crate::fs::Filesystem>::File>,
}

impl<Filesystem: crate::fs::Filesystem> ReaderActivity<Filesystem> {
    pub fn new(filesystem: Filesystem, file_path: String) -> Self {
        ReaderActivity {
            filesystem,
            file_path,
            file: None,
        }
    }
}

impl<Filesystem: crate::fs::Filesystem> super::Activity for ReaderActivity<Filesystem> {
    fn start(&mut self) {
        info!("Opening EPUB reader for path: {}", self.file_path);
        let mut file = self
            .filesystem
            .open_file(&self.file_path, crate::fs::Mode::Read)
            .unwrap();

        let epub = epub::parse(&mut file).unwrap();
        let meta = &epub.metadata;
        info!("Parsed EPUB: title={}, author={:?} ({:?})", meta.title, meta.author, meta.language);
    }

    fn update(&mut self, _state: &super::ApplicationState) -> crate::activities::UpdateResult {
        crate::activities::UpdateResult::PopActivity
    }
    fn draw(
        &mut self,
        _display: &mut dyn crate::display::Display,
        _buffers: &mut crate::framebuffer::DisplayBuffers,
    ) {
    }
}
