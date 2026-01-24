use embedded_graphics::{Drawable, mono_font::{MonoTextStyle, ascii::FONT_10X20}, pixelcolor::BinaryColor, prelude::{DrawTarget, OriginDimensions, Point, Primitive, Size}, primitives::{Circle, Line, PrimitiveStyle, Rectangle}, text::Text};

use crate::display::RefreshMode;


pub struct Application {

}

impl Application {
    pub fn new() -> Self {
        Application {

        }
    }

    pub fn update(&mut self) {

    }

    pub fn draw(&self, display: &mut impl crate::display::Display) {
        let mut fb = display.get_framebuffer_mut();
        // for byte in fb.iter_mut() {
        //     *byte = 0xFF;
        // }
        // Clear and redraw with new rotation
        fb.clear(BinaryColor::Off).ok();
        
        // Get the current display size (changes with rotation)
        let size = fb.size() - Size::new(20, 20);
        
        // Draw a border rectangle that fits the rotated display
        Rectangle::new(Point::new(10, 10), size)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(&mut fb)
            .ok();

        // Draw some circles
        Circle::new(Point::new(100, 100), 80)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 3))
            .draw(&mut fb)
            .ok();

        Circle::new(Point::new(200, 100), 60)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(&mut fb)
            .ok();

        // Draw text
        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        Text::new("Hello from rust", Point::new(20, 30), text_style)
            .draw(&mut fb)
            .ok();

        // Black
        Line::new(Point::new(100, 100), Point::new(700, 100))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(&mut fb)
            .ok();

        // display.copy_to_msb();
        display.display(RefreshMode::Full);

        let mut fb = display.get_framebuffer_mut();

        // Dark Gray
        Line::new(Point::new(100, 200), Point::new(700, 200))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(&mut fb)
            .ok();

        // Gray
        Line::new(Point::new(100, 300), Point::new(700, 300))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(&mut fb)
            .ok();

        // Rectangle::new(Point::new(14, 14), size - Size::new(8, 8))
        //     .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        //     .draw(&mut fb)
        //     .ok();
        // Rectangle::new(Point::new(18, 18), size - Size::new(14, 14))
        //     .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        //     .draw(&mut fb)
        //     .ok();
        display.copy_to_msb();
        
        let mut fb = display.get_framebuffer_mut();

        // Dark Gray
        Line::new(Point::new(100, 200), Point::new(700, 200))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(&mut fb)
            .ok();

        // Light Gray
        Line::new(Point::new(100, 400), Point::new(700, 400))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(&mut fb)
            .ok();

        // Rectangle::new(Point::new(14, 14), size - Size::new(6, 6))
        //     .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        //     .draw(&mut fb)
        //     .ok();
        // Rectangle::new(Point::new(22, 22), size - Size::new(14, 14))
        //     .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        //     .draw(&mut fb)
        //     .ok();
        display.copy_to_lsb();
        display.display_grayscale();
    }
}
