use crate::{
    display::{Display, GrayscaleMode, RefreshMode},
    framebuffer::{BUFFER_SIZE, DisplayBuffers, Rotation},
    input, layout,
    res::{
        font,
        img::{bebop, test_image},
        xt,
    },
};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, OriginDimensions, Point, Primitive, Size},
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};
use log::info;

pub struct DemoActivity {
    screen: usize,
    full_refresh: bool,
}

impl DemoActivity {
    pub fn new() -> Self {
        Self { screen: 0, full_refresh: true }
    }

    fn draw_bebop(&self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers
            .get_active_buffer_mut()
            .copy_from_slice(bebop::BEBOP);
        display.display(
            buffers,
            if self.full_refresh {
                RefreshMode::Full
            } else {
                RefreshMode::Fast
            },
        );
        display.copy_grayscale_buffers(bebop::BEBOP_LSB, bebop::BEBOP_MSB);
        display.display_differential_grayscale(false);
    }

    fn draw_test_image(&self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers
            .get_active_buffer_mut()
            .copy_from_slice(test_image::TEST_IMAGE);
        display.display(
            buffers,
            if self.full_refresh {
                RefreshMode::Full
            } else {
                RefreshMode::Fast
            },
        );
        display.copy_grayscale_buffers(test_image::TEST_IMAGE_LSB, test_image::TEST_IMAGE_MSB);
        display.display_differential_grayscale(false);
    }

    fn draw_shapes(&self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        // Clear and redraw with new rotation
        buffers.clear(BinaryColor::On).ok();

        // Get the current display size (changes with rotation)
        let size = buffers.size() - Size::new(20, 20);

        // Draw a border rectangle that fits the rotated display
        Rectangle::new(Point::new(10, 10), size)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 2))
            .draw(buffers)
            .ok();

        // Draw some circles
        Circle::new(Point::new(100, 100), 80)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 3))
            .draw(buffers)
            .ok();

        Circle::new(Point::new(200, 100), 60)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(buffers)
            .ok();

        // Draw text
        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        Text::new("Hello from rust", Point::new(20, 30), text_style)
            .draw(buffers)
            .ok();

        display.display(
            buffers,
            if self.full_refresh {
                RefreshMode::Full
            } else {
                RefreshMode::Fast
            },
        );
    }

    fn draw_grayscale(&self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers.clear(BinaryColor::On).ok();
        let size = buffers.size() - Size::new(20, 20);

        let width = size.width as i32 - 200;
        // Black
        Rectangle::new(Point::new(100, 50), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(buffers)
            .ok();
        Rectangle::new(Point::new(100, 150), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(buffers)
            .ok();
        Rectangle::new(Point::new(100, 250), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(buffers)
            .ok();

        display.display(
            buffers,
            if self.full_refresh {
                RefreshMode::Full
            } else {
                RefreshMode::Fast
            },
        );

        buffers.clear(BinaryColor::Off).ok();

        // Dark Gray
        Rectangle::new(Point::new(100, 150), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(buffers)
            .ok();

        // Gray
        Rectangle::new(Point::new(100, 250), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(buffers)
            .ok();

        display.copy_to_msb(buffers.get_active_buffer());

        buffers.clear(BinaryColor::Off).ok();

        // Dark Gray
        Rectangle::new(Point::new(100, 150), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(buffers)
            .ok();

        // Light Gray
        Rectangle::new(Point::new(100, 350), Size::new(width as _, 100))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(buffers)
            .ok();

        display.copy_to_lsb(buffers.get_active_buffer());
        display.display_differential_grayscale(false);
    }

    fn draw_xth(
        &self,
        display: &mut dyn Display,
        _buffers: &mut DisplayBuffers,
        mode: GrayscaleMode,
    ) {
        let lsb = &xt::XTH_DATA[0x16..(0x16 + BUFFER_SIZE)];
        let msb = &xt::XTH_DATA[(0x16 + BUFFER_SIZE)..(0x16 + 2 * BUFFER_SIZE)];
        display.copy_grayscale_buffers(lsb.try_into().unwrap(), msb.try_into().unwrap());
        display.display_absolute_grayscale(mode);
    }

    fn draw_xtg(&self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        let buffer = xt::XTG_DATA;
        buffers
            .get_active_buffer_mut()
            .copy_from_slice(&buffer[0x16..]);
        display.display(buffers, RefreshMode::Full);
    }

    fn draw_text(&self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        buffers.clear(BinaryColor::On).ok();

        let font = &font::bookerly_26::FONT;

        let size = buffers.size();

        let x_start = 10usize;
        let x_end = size.width as usize - 10usize;
        let mut x_advance = x_start;
        let mut y_advance = 0usize;
        y_advance += font.y_advance as usize;
        for glyph in font.glyphs {
            if (x_advance + glyph.x_advance() as usize) > x_end {
                x_advance = x_start;
                y_advance += font.y_advance as usize;
            }
            font::draw_glyph(
                font,
                glyph.codepoint,
                buffers,
                x_advance as isize,
                y_advance as isize,
                font::Mode::Bw,
            )
            .expect("Glyph not found");
            x_advance += glyph.x_advance() as usize;
        }

        display.display(
            buffers,
            if self.full_refresh {
                RefreshMode::Full
            } else {
                RefreshMode::Fast
            },
        );
        buffers.clear(BinaryColor::Off).ok();

        let mut x_advance = x_start;
        let mut y_advance = 0usize;
        y_advance += font.y_advance as usize;
        for glyph in font.glyphs {
            if (x_advance + glyph.x_advance() as usize) > x_end {
                x_advance = x_start;
                y_advance += font.y_advance as usize;
            }
            font::draw_glyph(
                font,
                glyph.codepoint,
                buffers,
                x_advance as isize,
                y_advance as isize,
                font::Mode::Msb,
            )
            .expect("Glyph not found");
            x_advance += glyph.x_advance() as usize;
        }

        display.copy_to_msb(buffers.get_active_buffer());
        buffers.clear(BinaryColor::Off).ok();

        let mut x_advance = x_start;
        let mut y_advance = 0usize;
        y_advance += font.y_advance as usize;
        for glyph in font.glyphs {
            if (x_advance + glyph.x_advance() as usize) > x_end {
                x_advance = x_start;
                y_advance += font.y_advance as usize;
            }
            font::draw_glyph(
                font,
                glyph.codepoint,
                buffers,
                x_advance as isize,
                y_advance as isize,
                font::Mode::Lsb,
            )
            .expect("Glyph not found");
            x_advance += glyph.x_advance() as usize;
        }

        display.copy_to_lsb(buffers.get_active_buffer());
        display.display_differential_grayscale(false);
    }

    fn draw_layouted_text(
        &self,
        display: &mut dyn Display,
        buffers: &mut DisplayBuffers,
        font: &font::FontDefinition,
    ) {
        let size = buffers.size();
        info!(
            "Display size: {:?}, rotation: {:?}",
            size,
            buffers.rotation()
        );

        let x_start = 20u16;
        let options = crate::layout::Options::new(
            size.width as u16 - 40,
            crate::layout::Alignment::Start,
            true,
            hypher::Lang::English,
            font,
        );

        let text = "The Watergate scandal, or simply Watergate, was a political scandal in the United States involving the administration of President Richard Nixon. On June 17, 1972, operatives associated with Nixon's 1972 re-election campaign were caught burglarizing and planting listening devices in the Democratic National Committee headquarters at Washington, D.C.'s Watergate complex. Nixon's efforts to conceal his administration's involvement led to an impeachment process and his resignation in August 1974.\n\
        Emerging from the White House's efforts to stop leaks, the break-in was an implementation of Operation Gemstone, enacted by mostly Cuban burglars led by former intelligence agents E. Howard Hunt and G. Gordon Liddy. After the arrests, investigators and reporters like The Washington Post's Bob Woodward and Carl Bernstein—guided by the source \"Deep Throat\"—exposed a White House political espionage program illegally funded by donor contributions. Nixon denied involvement but his administration destroyed evidence, obstructed investigators, and bribed the burglars. This cover-up initially worked, helping Nixon win a landslide re-election, until revelations from the burglars' 1973 trial led to a Senate investigation.\n\
        Mounting pressure led Attorney General Elliot Richardson to appoint Archibald Cox as Watergate special prosecutor. Cox subpoenaed Nixon's Oval Office tapes—suspected to include Watergate conversations—but Nixon invoked executive privilege to block their release, triggering a constitutional crisis. In the \"Saturday Night Massacre\", Nixon fired Cox, forcing the resignations of the attorney general and his deputy and fueling suspicions of Nixon's involvement. Nixon released select tapes, although one was partially erased and two others disappeared. In April 1974, Cox's replacement Leon Jaworski reissued the subpoena, but Nixon provided only redacted transcripts. In July, the Supreme Court ordered the tapes' release, and the House Judiciary Committee recommended impeachment for obstructing justice, abuse of power, and contempt of Congress. The White House released the \"Smoking Gun\" tape, showing that Nixon ordered the CIA to stop the FBI's investigation. Facing impeachment, on August 9, 1974, Nixon became the first U.S. president to resign. In total, 69 people were charged for Watergate—including two cabinet members—and most pleaded guilty or were convicted. Nixon was pardoned by his successor, Gerald Ford.\n\
        Watergate, often considered the greatest presidential scandal, tarnished Nixon's legacy and had electoral ramifications for the Republican Party: heavy losses in the 1974 midterm elections and Ford's failed 1976 reelection bid. Despite significant coverage, no consensus exists on the motive for the break-in or who specifically ordered it. Theories range from an incompetent break-in by rogue campaign officials to a sexpionage operation or CIA plot. The scandal generated over 30 memoirs and left such an impression that it is common for scandals, even outside politics or the United States, to be named with the suffix \"-gate\".";

        let lines = crate::layout::layout_text(options, text);

        buffers.clear(BinaryColor::On).ok();
        Self::draw_layed_out_text(font, &lines, x_start, font::Mode::Bw, buffers);
        display.display(
            buffers,
            if self.full_refresh {
                RefreshMode::Full
            } else {
                RefreshMode::Fast
            },
        );

        buffers.clear(BinaryColor::Off).ok();
        Self::draw_layed_out_text(font, &lines, x_start, font::Mode::Msb, buffers);
        display.copy_to_msb(buffers.get_active_buffer());

        buffers.clear(BinaryColor::Off).ok();
        Self::draw_layed_out_text(font, &lines, x_start, font::Mode::Lsb, buffers);
        display.copy_to_lsb(buffers.get_active_buffer());
        display.display_differential_grayscale(false);
    }

    fn draw_layed_out_text(
        font: &font::FontDefinition,
        lines: &[layout::Line],
        x_start: u16,
        mode: font::Mode,
        display_buffers: &mut DisplayBuffers,
    ) {
        let size = display_buffers.size();

        for line in lines.iter() {
            if line.y as u32 >= size.height {
                break;
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
                        line.y as isize,
                        mode,
                    ) {
                        // Line::new(
                        //     Point {
                        //         x: x_advance as _,
                        //         y: (line.y + 3) as _,
                        //     },
                        //     Point {
                        //         x: (x_advance + glyph_width as u16) as _,
                        //         y: (line.y + 3) as _,
                        //     },
                        // )
                        // .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 1))
                        // .draw(display_buffers);
                        x_advance += glyph_width as u16;
                    }
                }
            }
            if line.hyphenated {
                font::draw_glyph(
                    font,
                    '-' as _,
                    display_buffers,
                    x_advance as isize,
                    line.y as isize,
                    font::Mode::Bw,
                )
                .unwrap();
            }
        }
    }
}

impl super::Activity for DemoActivity {
    fn start(&mut self) {
        log::info!("DemoActivity started");
    }

    fn update(&mut self, state: &super::ApplicationState) -> super::UpdateResult {
        let buttons = &state.input;
        // self.dirty |= buttons.is_pressed(input::Buttons::Confirm);
        if buttons.is_pressed(input::Buttons::Left) {
            self.full_refresh = !self.full_refresh;
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(input::Buttons::Right) {
            super::UpdateResult::SetRotation(match state.rotation {
                Rotation::Rotate0 => Rotation::Rotate90,
                Rotation::Rotate90 => Rotation::Rotate180,
                Rotation::Rotate180 => Rotation::Rotate270,
                Rotation::Rotate270 => Rotation::Rotate0,
            })
        } else if buttons.is_pressed(input::Buttons::Up) {
            self.screen = if self.screen == 0 {
                19
            } else {
                self.screen - 1
            };
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(input::Buttons::Down) {
            self.screen = if self.screen == 19 {
                0
            } else {
                self.screen + 1
            };
            super::UpdateResult::Redraw
        } else if buttons.is_pressed(input::Buttons::Back) {
            super::UpdateResult::PopActivity
        } else if buttons.is_pressed(input::Buttons::Confirm) {
            super::UpdateResult::Redraw
        } else {
            super::UpdateResult::None
        }
    }

    fn draw(&mut self, display: &mut dyn Display, buffers: &mut DisplayBuffers) {
        match self.screen {
            1 => self.draw_test_image(display, buffers),
            2 => self.draw_bebop(display, buffers),
            3 => self.draw_grayscale(display, buffers),
            4 => self.draw_xth(display, buffers, GrayscaleMode::Standard),
            5 => self.draw_xth(display, buffers, GrayscaleMode::Fast),
            6 => self.draw_xtg(display, buffers),
            7 => self.draw_text(display, buffers),
            8 => self.draw_layouted_text(display, buffers, &font::bookerly_26::FONT),
            9 => self.draw_layouted_text(display, buffers, &font::bookerly_28::FONT),
            10 => self.draw_layouted_text(display, buffers, &font::bookerly_30::FONT),
            11 => self.draw_layouted_text(display, buffers, &font::bookerly_italic_26::FONT),
            12 => self.draw_layouted_text(display, buffers, &font::bookerly_italic_28::FONT),
            13 => self.draw_layouted_text(display, buffers, &font::bookerly_italic_30::FONT),
            14 => self.draw_layouted_text(display, buffers, &font::bookerly_bold_26::FONT),
            15 => self.draw_layouted_text(display, buffers, &font::bookerly_bold_28::FONT),
            16 => self.draw_layouted_text(display, buffers, &font::bookerly_bold_30::FONT),
            17 => self.draw_layouted_text(display, buffers, &font::bookerly_bold_italic_26::FONT),
            18 => self.draw_layouted_text(display, buffers, &font::bookerly_bold_italic_28::FONT),
            19 => self.draw_layouted_text(display, buffers, &font::bookerly_bold_italic_30::FONT),
            _ => self.draw_shapes(display, buffers),
        }
        self.full_refresh = false;
    }
}
