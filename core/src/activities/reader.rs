pub struct ReaderActivity {}

impl super::Activity for ReaderActivity {
    fn start(&mut self) {}

    fn update(&mut self, _state: &super::ApplicationState) -> crate::activities::UpdateResult {
        crate::activities::UpdateResult::PopActivity
    }
    fn draw(&mut self, _display: &mut dyn crate::display::Display, _buffers: &mut crate::framebuffer::DisplayBuffers) {
    }
}