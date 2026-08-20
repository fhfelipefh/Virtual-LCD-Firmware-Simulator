use super::*;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerModel {
    GenericMipiDcs,
    Ili9341,
    Ssd1306,
    St7789,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LcdConfig {
    pub width: u16,
    pub height: u16,
    pub pixel_format: PixelFormat,
    pub fps: u16,
    pub interface: InterfaceType,
    pub orientation: u16,
    pub vsync: bool,
    pub buffering: BufferingMode,
    pub backlight: bool,
    pub tearing_effect: bool,
    pub bus_hz: u32,
    pub controller: ControllerModel,
}

impl Default for LcdConfig {
    fn default() -> Self {
        Self {
            width: 320,
            height: 240,
            pixel_format: PixelFormat::Rgb565,
            fps: 30,
            interface: InterfaceType::Spi4Wire,
            orientation: 0,
            vsync: true,
            buffering: BufferingMode::Double,
            backlight: true,
            tearing_effect: false,
            bus_hz: 8_000_000,
            controller: ControllerModel::Ili9341,
        }
    }
}

impl LcdConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(LcdError::InvalidConfig("display dimensions must be non-zero"));
        }

        if self.fps == 0 {
            return Err(LcdError::InvalidConfig("fps must be non-zero"));
        }

        if self.bus_hz == 0 {
            return Err(LcdError::InvalidConfig("bus_hz must be non-zero"));
        }

        if matches!(self.controller, ControllerModel::Ssd1306) {
            if self.width > 128 {
                return Err(LcdError::InvalidConfig("ssd1306 width must be 128 pixels or smaller"));
            }

            if self.height > 64 {
                return Err(LcdError::InvalidConfig("ssd1306 height must be 64 pixels or smaller"));
            }

            if self.height % 8 != 0 {
                return Err(LcdError::InvalidConfig("ssd1306 height must be a multiple of 8"));
            }
        }

        Ok(())
    }

    pub fn frame_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.fps as f64)
    }

    pub fn full_frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * self.pixel_format.bytes_per_pixel()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Mono1,
    Gray8,
    Rgb565,
    Rgb888,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Mono1 | Self::Gray8 => 1,
            Self::Rgb565 => 2,
            Self::Rgb888 => 3,
        }
    }
    pub(crate) fn decode_color(self, bytes: &[u8]) -> Color {
        match self {
            Self::Mono1 => {
                if bytes[0] == 0 {
                    Color::BLACK
                } else {
                    Color::WHITE
                }
            }
            Self::Gray8 => Color::rgb(bytes[0], bytes[0], bytes[0]),
            Self::Rgb565 => {
                let value = u16::from_be_bytes([bytes[0], bytes[1]]);
                Color::from_rgb565(value)
            }
            Self::Rgb888 => Color::rgb(bytes[0], bytes[1], bytes[2]),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceType {
    Spi4Wire,
    Spi3Wire,
    Parallel8080,
    MemoryMapped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferingMode {
    Single,
    Double,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawWindow {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl DrawWindow {
    pub fn full(config: &LcdConfig) -> Self {
        Self {
            x: 0,
            y: 0,
            width: config.width,
            height: config.height,
        }
    }

    pub fn from_origin(x: u16, y: u16, width: u16, height: u16, config: &LcdConfig) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(LcdError::InvalidWindow);
        }

        let x_end = x
            .checked_add(width - 1)
            .ok_or(LcdError::OutOfBounds)?;
        let y_end = y
            .checked_add(height - 1)
            .ok_or(LcdError::OutOfBounds)?;

        if x_end >= config.width || y_end >= config.height {
            return Err(LcdError::OutOfBounds);
        }

        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub fn from_inclusive(x0: u16, y0: u16, x1: u16, y1: u16, config: &LcdConfig) -> Result<Self> {
        if x1 < x0 || y1 < y0 {
            return Err(LcdError::InvalidWindow);
        }

        Self::from_origin(x0, y0, x1 - x0 + 1, y1 - y0 + 1, config)
    }

    pub fn area(self) -> usize {
        self.width as usize * self.height as usize
    }
}
