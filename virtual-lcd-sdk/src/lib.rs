#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn luminance(self) -> u8 {
        ((self.r as u16 * 30 + self.g as u16 * 59 + self.b as u16 * 11) / 100) as u8
    }

    pub fn to_rgb565(self) -> u16 {
        let r = (self.r as u16 >> 3) & 0x1F;
        let g = (self.g as u16 >> 2) & 0x3F;
        let b = (self.b as u16 >> 3) & 0x1F;
        (r << 11) | (g << 5) | b
    }

    pub fn from_rgb565(value: u16) -> Self {
        let r = ((value >> 11) & 0x1F) as u8;
        let g = ((value >> 5) & 0x3F) as u8;
        let b = (value & 0x1F) as u8;

        Self {
            r: (r << 3) | (r >> 2),
            g: (g << 2) | (g >> 4),
            b: (b << 3) | (b >> 2),
        }
    }

    pub fn to_gray8(self) -> u8 {
        self.luminance()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinId {
    Cs,
    Dc,
    Rst,
    Wr,
    Rd,
    Clk,
    Mosi,
    Miso,
    Bl,
}

impl PinId {
    pub const ALL: [Self; 9] = [
        Self::Cs,
        Self::Dc,
        Self::Rst,
        Self::Wr,
        Self::Rd,
        Self::Clk,
        Self::Mosi,
        Self::Miso,
        Self::Bl,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Cs => 0,
            Self::Dc => 1,
            Self::Rst => 2,
            Self::Wr => 3,
            Self::Rd => 4,
            Self::Clk => 5,
            Self::Mosi => 6,
            Self::Miso => 7,
            Self::Bl => 8,
        }
    }
}

pub trait Lcd {
    type Error;

    fn init(&mut self) -> Result<(), Self::Error>;
    fn clear(&mut self, color: Color) -> Result<(), Self::Error>;
    fn draw_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<(), Self::Error>;
    fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: Color,
    ) -> Result<(), Self::Error>;
    fn present(&mut self) -> Result<(), Self::Error>;
}

pub trait LcdBus {
    type Error;

    fn set_pin(&mut self, pin: PinId, value: bool) -> Result<(), Self::Error>;
    fn write_command(&mut self, cmd: u8) -> Result<(), Self::Error>;
    fn write_data(&mut self, data: &[u8]) -> Result<(), Self::Error>;
    fn read_data(&mut self, len: usize) -> Result<Vec<u8>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn rgb565_roundtrip_preserves_primary_signal() {
        let red = Color::RED;
        let encoded = red.to_rgb565();
        let decoded = Color::from_rgb565(encoded);

        assert!(decoded.r > 200);
        assert!(decoded.g < 20);
        assert!(decoded.b < 20);
    }
}
