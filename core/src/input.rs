use crate::framebuffer::Rotation;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Buttons {
    Back,
    Confirm,
    Left,
    Right,
    Up,
    Down,
    Power,
}

#[derive(Clone, Copy, Default)]
pub struct ButtonState {
    current: u8,
    previous: u8,
}

impl ButtonState {
    pub fn update(&mut self, current: u8) {
        self.previous = self.current;
        self.current = current;
    }

    fn rotate_once(val: u8) -> u8 {
        let right = (val >> Buttons::Right as u8) & 1;
        let left = (val >> Buttons::Left as u8) & 1;
        let up = (val >> Buttons::Up as u8) & 1;
        let down = (val >> Buttons::Down as u8) & 1;
        (val & 0b11000011) |
        (right << Buttons::Down as u8) |
        (down << Buttons::Left as u8) |
        (left << Buttons::Up as u8) |
        (up << Buttons::Right as u8)
    }

    fn rotate(mut val: u8, count: u8) -> u8 {
        for _ in 0..count {
            val = Self::rotate_once(val);
        }
        val
    }

    pub fn translated(&self, rotation: Rotation) -> Self {
        match rotation {
            Rotation::Rotate0 => Self {
                current: Self::rotate(self.current, 3),
                previous: Self::rotate(self.previous, 3),
            },
            Rotation::Rotate90 => *self,
            Rotation::Rotate180 => Self {
                current: Self::rotate(self.current, 1),
                previous: Self::rotate(self.previous, 1),
            },
            Rotation::Rotate270 => Self {
                current: Self::rotate(self.current, 2),
                previous: Self::rotate(self.previous, 2),
            },
        }
    }

    fn held(&self) -> u8 {
        self.current & self.previous
    }

    fn pressed(&self) -> u8 {
        self.current & !self.previous
    }

    fn released(&self) -> u8 {
        !self.current & self.previous
    }

    pub fn is_held(&self, button: Buttons) -> bool {
        let mask = 1 << (button as u8);
        (self.held() & mask) != 0
    }

    pub fn is_pressed(&self, button: Buttons) -> bool {
        let mask = 1 << (button as u8);
        (self.pressed() & mask) != 0
    }

    pub fn is_released(&self, button: Buttons) -> bool {
        let mask = 1 << (button as u8);
        (self.released() & mask) != 0
    }

    pub fn any_pressed(&self, buttons: &[Buttons]) -> bool {
        let mask = buttons
            .iter()
            .fold(0, |acc, &button| acc | (1 << (button as u8)));
        (self.pressed() & mask) != 0
    }
}
