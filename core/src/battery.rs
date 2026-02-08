#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChargeState {
    pub level: u8,
    pub charging: bool,
}

impl ChargeState {
    pub fn format(&self) -> heapless::String<8> {
        heapless::format!("{}{}%", if self.charging { "+" } else { "" }, self.level).unwrap()
    }
}
