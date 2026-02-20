use alloc::{
    string::String,
    vec::Vec,
};
use embedded_graphics::{
    Drawable, mono_font::{MonoTextStyle, ascii::FONT_10X20}, pixelcolor::BinaryColor, prelude::{DrawTarget, OriginDimensions, Point, Primitive, Size}, primitives::{Line, PrimitiveStyle, Rectangle}, text::Text
};
use log::{info, warn};

use crate::{
    container::book,
    display::RefreshMode,
    framebuffer::DisplayBuffers,
    input::Buttons,
    layout,
    res::font,
};
pub struct ReaderActivity<Filesystem>
where
    Filesystem: crate::fs::Filesystem,
{
    filesystem: Filesystem,
    file_path: String,
    show_settings: bool,
    settings_cursor: usize,
    font_size: font::FontSize,
    alignment: layout::Alignment,
    justify: bool,
    language: hypher::Lang,
    debug_width: bool,
    file: Filesystem::File,
    book: Option<book::Book>,
    chapter_idx: usize,
    chapter: Option<book::Chapter>,
    progress: Page,
}

struct Page {
    start: Progress,
    end: Progress,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            start: Progress { paragraph: 0, line: 0 },
            end: Progress { paragraph: 0, line: 0 },
        }
    }
}

#[derive(Clone, Copy)]
struct Progress {
    paragraph: u16,
    line: u16,
}

impl<Filesystem: crate::fs::Filesystem> ReaderActivity<Filesystem> {
    pub fn new(filesystem: Filesystem, file_path: String) -> Self {
        info!("Opening EPUB reader for path: {}", file_path);
        let mut file = filesystem
            .open_file(&file_path, crate::fs::Mode::Read)
            .unwrap();

        let book = book::Book::from_file(&file_path, &mut file);
        let language = book.as_ref().and_then(|book| book.language()).unwrap_or(hypher::Lang::English);

        let chapter = book.as_ref().and_then(|b| b.chapter(0, &mut file));

        ReaderActivity {
            filesystem,
            file_path,
            show_settings: false,
            settings_cursor: 0,
            font_size: font::FontSize::Size26,
            alignment: layout::Alignment::Start,
            justify: true,
            language,
            debug_width: false,
            file,
            book,
            chapter_idx: 0,
            chapter,
            progress: Page::default(),
        }
    }

    fn draw_layed_out_text(
        &self,
        font: font::Font,
        lines: &[layout::Line],
        y_offsets: &[u16],
        x_start: u16,
        y_base: u16,
        mode: font::Mode,
        display_buffers: &mut DisplayBuffers,
    ) {
        let size = display_buffers.size();
        let font = font.definition(font::FontStyle::Regular);

        for (line, y_offset) in lines.iter().zip(y_offsets) {
            let y = y_base + y_offset;
            if y as u32 >= size.height {
                return;
            }
            let mut x_advance = 0u16;
            for word in line.words.iter() {
                x_advance = x_start + word.x;
                for codepoint in word.text.chars() {
                    if let Ok(glyph_width) = font::draw_glyph(
                        font,
                        codepoint as _,
                        display_buffers,
                        x_advance as isize,
                        y as isize,
                        mode,
                    ) {
                        self.print_debug_line(x_advance, y, glyph_width as u16, display_buffers);
                        x_advance += glyph_width as u16;
                    }
                }
            }
            if line.hyphenated {
                if let Ok(glyph_width) = font::draw_glyph(
                    font,
                    '-' as _,
                    display_buffers,
                    x_advance as isize,
                    y as isize,
                    font::Mode::Bw,
                ) {
                    self.print_debug_line(x_advance, y, glyph_width as u16, display_buffers);
                }
            }
        }
    }

    fn print_debug_line(&self, x: u16, y: u16, width: u16, display_buffers: &mut DisplayBuffers,) {
        if !self.debug_width {
            return;
        }

        Line::new(
            Point {
                x: x as _,
                y: (y + 3) as _,
            },
            Point {
                x: (x + width) as _,
                y: (y + 3) as _,
            },
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 1))
        .draw(display_buffers);
    }

    fn next_page(&mut self, _: Size) {
        let Some(chapter) = &self.chapter else {
            self.next_chapter();
            return;
        };
        let end = &self.progress.end;
        let at_end = end.paragraph as usize >= chapter.paragraphs.len();
        if !at_end {
            self.progress.start = Progress {
                paragraph: end.paragraph,
                line: end.line,
            };
        } else {
            self.next_chapter();
        }
    }

    fn next_chapter(&mut self) {
        let Some(book) = &self.book else { return; };
        if self.chapter_idx + 1 >= book.chapter_count() {
            return;
        }
        self.chapter_idx += 1;
        self.chapter = book.chapter(self.chapter_idx, &mut self.file);
        self.progress.start = Progress { paragraph: 0, line: 0 };
    }

    fn prev_page(&mut self, Size { width, height }: Size) {
        let padding = 10u32;
        let font = font::Font::new(font::FontFamily::Bookerly, self.font_size);
        let options = layout::Options::new(
            (width - 2 * padding) as _,
            self.alignment,
            self.justify,
            self.language,
            font,
        );
        let page_height = (height - padding - 10) as u16;
        let Some(chapter) = &self.chapter else {
            self.prev_chapter(options, page_height);
            return;
        };
        if let Some(progress) = Self::compute_prev_page(
            chapter,
            self.progress.start,
            options,
            page_height,
        ) {
            self.progress.start = progress;
        } else {
            self.prev_chapter(options, page_height);
        };
    }

    fn prev_chapter(&mut self, options: layout::Options, page_height: u16) {
        let Some(book) = &self.book else { return; };
        if self.chapter_idx == 0 {
            return;
        }
        self.chapter_idx -= 1;
        let Some(chapter) = book.chapter(self.chapter_idx, &mut self.file) else { return; };
        let last_para = chapter.paragraphs.len() - 1;
        let lines = layout::layout_text(options, &chapter.paragraphs[last_para].text);
        // Try to show the last 10 lines
        // NOTE: unless we lay out the entire chapter, there doesn't seem to be a sane way of getting
        // the correct line number. Fill the entire page :(
        self.progress.start = Self::compute_prev_page(
            &chapter,
            Progress { paragraph: last_para as u16, line: lines.len() as u16 },
            options,
            page_height,
        ).unwrap_or(Progress { paragraph: last_para as u16, line: 0 });
        self.chapter = Some(chapter);
    }

    /// Compute the previous page start by laying out paragraphs backwards
    /// from the given position until the page is filled from the bottom.
    fn compute_prev_page(
        chapter: &book::Chapter,
        current: Progress,
        options: layout::Options,
        page_height: u16,
    ) -> Option<Progress> {
        let y_advance = options.font.y_advance();
        let para_spacing = y_advance / 2;
        let mut remaining = page_height;

        let cur_para = current.paragraph as usize;
        let cur_line = current.line as usize;

        // Walk backwards through paragraphs
        // Start with the current paragraph (lines before cur_line)
        let mut result_para = 0usize;
        let mut result_line = 0usize;

        // If we're at (0, 0), nothing to go back to
        if cur_para == 0 && cur_line == 0 {
            return None;
        }

        // Determine the first paragraph to consider and how many lines from it
        // We iterate from cur_para down to 0
        // Start from the paragraph just before current position
        let mut first_iter = true;
        let mut para_idx = if cur_line > 0 { cur_para } else { cur_para.saturating_sub(1) };
        let at_line = if cur_line > 0 { cur_line } else { usize::MAX };

        loop {
            let paragraph = &chapter.paragraphs[para_idx];

            // Add paragraph spacing (between paragraphs, not before the bottom-most)
            if !first_iter {
                if remaining < para_spacing {
                    // Can't fit the spacing; previous result stands
                    break;
                }
                remaining -= para_spacing;
            }
            first_iter = false;

            if paragraph.text.is_empty() {
                result_para = para_idx;
                result_line = 0;
                if para_idx == 0 {
                    break;
                }
                para_idx -= 1;
                continue;
            }

            let para_lines = layout::layout_text(options, &paragraph.text);
            // How many lines from this paragraph are available
            let available = if para_idx == cur_para && at_line != usize::MAX {
                at_line
            } else {
                para_lines.len()
            };

            // Try to fit lines from the end backwards
            let mut fitted = 0usize;
            for _ in (0..available).rev() {
                if remaining < y_advance {
                    break;
                }
                remaining -= y_advance;
                fitted += 1;
            }

            if fitted > 0 {
                result_para = para_idx;
                result_line = available - fitted;
            }

            if remaining < y_advance {
                // Page full
                break;
            }

            if para_idx == 0 {
                break;
            }
            para_idx -= 1;
        }


        Some(Progress {
            paragraph: result_para as u16,
            line: result_line as u16,
        })
    }

    fn display_settings(&self, buffers: &mut DisplayBuffers) {
        if !self.show_settings {
            return;
        }

        let size = buffers.size();
        Rectangle::new(
            Point { x: 5, y: size.height as i32 / 2 },
            Size { width: size.width - 10, height: size.height / 2 - 5 },
        )
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 1))
            .draw(buffers);
        Rectangle::new(
            Point { x: 8, y: size.height as i32 / 2 + 3 },
            Size { width: size.width - 16, height: size.height / 2 - 11 },
        )
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 1))
            .draw(buffers);

        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        Text::new("Settings", Point::new(20, size.height as i32 / 2 + 20), text_style)
            .draw(buffers)
            .ok();

        let cursor_pos = 10;
        let desc_pos = 20;
        let value_pos = 200;

        let cursor_offset = self.settings_cursor as i32 * 30;
        Text::new(">", Point::new(cursor_pos, size.height as i32 / 2 + 50 + cursor_offset), text_style)
            .draw(buffers)
            .ok();
        
        // Text Size
        Text::new("Font Size:", Point::new(desc_pos, size.height as i32 / 2 + 50), text_style)
            .draw(buffers)
            .ok();
        Text::new(self.font_size.repr(), Point::new(value_pos, size.height as i32 / 2 + 50), text_style)
            .draw(buffers)
            .ok();

        // Alignment
        Text::new("Alignment:", Point::new(desc_pos, size.height as i32 / 2 + 80), text_style)
            .draw(buffers)
            .ok();
        Text::new(self.alignment.repr(), Point::new(value_pos, size.height as i32 / 2 + 80), text_style)
            .draw(buffers)
            .ok();
        
        // Justify
        Text::new("Justify:", Point::new(desc_pos, size.height as i32 / 2 + 110), text_style)
            .draw(buffers)
            .ok();
        Text::new(if self.justify { "On" } else { "Off" }, Point::new(value_pos, size.height as i32 / 2 + 110), text_style)
            .draw(buffers)
            .ok();

        // Rotation
        Text::new("Rotation:", Point::new(desc_pos, size.height as i32 / 2 + 140), text_style)
            .draw(buffers)
            .ok();
        Text::new(buffers.rotation().repr(), Point::new(value_pos, size.height as i32 / 2 + 140), text_style)
            .draw(buffers)
            .ok();

        // Language
        Text::new("Language:", Point::new(desc_pos, size.height as i32 / 2 + 170), text_style)
            .draw(buffers)
            .ok();
        Text::new(&alloc::format!("{:?}", self.language), Point::new(value_pos, size.height as i32 / 2 + 170), text_style)
            .draw(buffers)
            .ok();

        // Justification debug lines
        Text::new("Debug Justify:", Point::new(desc_pos, size.height as i32 / 2 + 200), text_style)
            .draw(buffers)
            .ok();
        Text::new(if self.debug_width { "On" } else { "Off" }, Point::new(value_pos, size.height as i32 / 2 + 200), text_style)
            .draw(buffers)
            .ok();
    }

    fn display_footer(&self, buffers: &mut DisplayBuffers) {
        if self.show_settings {
            return;
        }
        let size = buffers.size();
        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        if let Some(title) = &self.chapter.as_ref().and_then(|c| c.title.as_deref()) {
            Text::new(&title, Point::new(10, size.height as i32 - 10), text_style)
                .draw(buffers)
                .ok();
        }
    }

    fn update_settings(&mut self, state: &super::ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;

        if buttons.is_pressed(Buttons::Back) {
            self.show_settings = false;
            return super::UpdateResult::Redraw;
        } else if buttons.is_pressed(Buttons::Confirm) {
            use font::FontSize::*;
            use crate::framebuffer::Rotation::*;
            use layout::Alignment::*;
            match self.settings_cursor {
                0 => self.font_size = match self.font_size {
                    Size26 => Size30,
                    Size28 => Size26,
                    Size30 => Size28,
                },
                1 => self.alignment = match self.alignment {
                    Start => Center,
                    Center => End,
                    End => Start,
                },
                2 => self.justify = !self.justify,
                3 => {
                    let new_rotation = match state.rotation {
                        Rotate0 => Rotate90,
                        Rotate90 => Rotate180,
                        Rotate180 => Rotate270,
                        Rotate270 => Rotate0,
                    };
                    return super::UpdateResult::SetRotation(new_rotation);
                },
                4 => self.language = match self.language {
                    hypher::Lang::English => hypher::Lang::French,
                    hypher::Lang::French => hypher::Lang::German,
                    hypher::Lang::German => hypher::Lang::Spanish,
                    hypher::Lang::Spanish => hypher::Lang::English,
                    _ => self.language, // Don't cycle unsupported languages
                },
                5 => self.debug_width = !self.debug_width,
                _ => return super::UpdateResult::None
            }
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Up) {
            self.settings_cursor = if self.settings_cursor > 0 { self.settings_cursor - 1 } else { 0 };
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Down) {
            self.settings_cursor = if self.settings_cursor < 5 { self.settings_cursor + 1 } else { 5 };
            super::UpdateResult::Redraw
        } else {
            super::UpdateResult::None
        }
    }
}

impl<Filesystem: crate::fs::Filesystem> super::Activity for ReaderActivity<Filesystem> {
    fn start(&mut self) {}

    fn update(&mut self, state: &super::ApplicationState) -> super::UpdateResult {
        if self.show_settings {
            return self.update_settings(state);
        }

        let buttons = &state.input;

        if buttons.is_pressed(Buttons::Up) {
            self.prev_page(state.rotation.size());
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Down) {
            self.next_page(state.rotation.size());
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Left) {
            super::UpdateResult::None
        } else if buttons.is_pressed(Buttons::Right) {
            super::UpdateResult::None
        } else if buttons.is_pressed(Buttons::Confirm) {
            self.show_settings = !self.show_settings;
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(Buttons::Back) {
            super::UpdateResult::PopActivity
        } else {
            super::UpdateResult::None
        }
    }

    fn draw(&mut self, display: &mut dyn crate::display::Display, buffers: &mut DisplayBuffers) {
        let Some(chapter) = &self.chapter else {
            warn!("No chapter");
            return;
        };
        let padding = 10;
        let Size { width, height } = buffers.size();

        let font = font::Font::new(font::FontFamily::Bookerly, self.font_size);
        let options = layout::Options::new(
            (width - 2 * padding) as _,
            self.alignment,
            self.justify,
            self.language,
            font,
        );

        let x_start = padding as u16;
        let y_advance = font.y_advance();
        let para_spacing = y_advance / 2;
        let y_start = y_advance / 2 + padding as u16;
        let page_height = if self.show_settings {
            (height / 2 - padding) as u16
        } else {
            (height - padding - 10) as u16
        };

        // Collect lines forward from start, tracking pixel height with paragraph spacing
        let start_paragraph = self.progress.start.paragraph as usize;
        let start_line = self.progress.start.line as usize;

        let mut all_lines: Vec<layout::Line> = Vec::new();
        let mut y_offsets: Vec<u16> = Vec::new();
        let mut end_paragraph = start_paragraph;
        let mut end_line: usize = 0;
        let mut y_cursor: u16 = 0;

        'outer: for para_idx in start_paragraph..chapter.paragraphs.len() {
            let paragraph = &chapter.paragraphs[para_idx];

            // Add paragraph spacing before each paragraph (except the first on the page)
            if para_idx > start_paragraph || start_line == 0 {
                if !all_lines.is_empty() {
                    y_cursor += para_spacing;
                }
            }

            if paragraph.text.is_empty() {
                end_paragraph = para_idx + 1;
                end_line = 0;
                continue;
            }

            let para_lines = layout::layout_text(options, &paragraph.text);
            let skip = if para_idx == start_paragraph { start_line } else { 0 };

            for (line_idx, line) in para_lines.into_iter().enumerate() {
                if line_idx < skip {
                    continue;
                }

                if y_cursor + y_advance > page_height {
                    end_paragraph = para_idx;
                    end_line = line_idx;
                    break 'outer;
                }

                y_offsets.push(y_cursor);
                all_lines.push(line);
                y_cursor += y_advance;
                end_paragraph = para_idx;
                end_line = line_idx + 1;
            }

            // Finished this paragraph entirely
            if end_paragraph == para_idx {
                end_paragraph = para_idx + 1;
                end_line = 0;
            }
        }

        self.progress.end = Progress {
            paragraph: end_paragraph as u16,
            line: end_line as u16,
        };

        buffers.clear(BinaryColor::On).ok();
        self.draw_layed_out_text(font, &all_lines, &y_offsets, x_start, y_start, font::Mode::Bw, buffers);
        self.display_settings(buffers);
        self.display_footer(buffers);
        display.display(buffers, RefreshMode::Fast);

        buffers.clear(BinaryColor::Off).ok();
        self.draw_layed_out_text(font, &all_lines, &y_offsets, x_start, y_start, font::Mode::Msb, buffers);
        display.copy_to_msb(buffers.get_active_buffer());

        buffers.clear(BinaryColor::Off).ok();
        self.draw_layed_out_text(font, &all_lines, &y_offsets, x_start, y_start, font::Mode::Lsb, buffers);
        display.copy_to_lsb(buffers.get_active_buffer());
        display.display_differential_grayscale(false);
    }
}
