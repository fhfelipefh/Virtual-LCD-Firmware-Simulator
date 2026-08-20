use super::*;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct St7789State {
    pub(crate) madctl: u8,
    pub(crate) colmod: u8,
    pub(crate) inversion_on: bool,
    pub(crate) tearing_enabled: bool,
    pub(crate) tearing_mode: u8,
    pub(crate) brightness: u8,
    pub(crate) control_display: u8,
    pub(crate) scroll: VerticalScrollState,
    pub(crate) raw_registers: BTreeMap<u8, Vec<u8>>,
}

impl St7789State {
    const MADCTL_MY: u8 = 0x80;
    const MADCTL_MX: u8 = 0x40;
    const MADCTL_MV: u8 = 0x20;
    const MADCTL_BGR: u8 = 0x08;

    pub(crate) fn new(config: &LcdConfig) -> Self {
        Self {
            madctl: 0x00,
            colmod: 0x55, // default to rgb565
            inversion_on: true, // st7789 typically uses inversion by default
            tearing_enabled: config.tearing_effect,
            tearing_mode: 0x00,
            brightness: if config.backlight { 0xFF } else { 0x00 },
            control_display: 0x24,
            scroll: VerticalScrollState::new(config.height),
            raw_registers: BTreeMap::new(),
        }
    }

    pub(crate) fn interface_pixel_format(&self) -> PixelFormat {
        match self.colmod & 0x77 {
            0x55 => PixelFormat::Rgb565,
            0x66 => PixelFormat::Rgb888,
            _ => PixelFormat::Rgb565,
        }
    }

    pub(crate) fn decode_interface_color(&self, bytes: &[u8]) -> Color {
        match self.interface_pixel_format() {
            PixelFormat::Rgb565 => PixelFormat::Rgb565.decode_color(bytes),
            PixelFormat::Rgb888 => {
                let expand = |value: u8| (value << 2) | (value >> 4);
                Color::rgb(expand(bytes[0]), expand(bytes[1]), expand(bytes[2]))
            }
            other => other.decode_color(bytes),
        }
    }

    pub(crate) fn map_logical_to_memory(&self, x: u16, y: u16, config: &LcdConfig) -> Result<(u16, u16)> {
        let width = config.width;
        let height = config.height;

        let logical_y = self.scroll.map_visible_row(y, height);
        let mx = self.madctl & Self::MADCTL_MX != 0;
        let my = self.madctl & Self::MADCTL_MY != 0;
        let mv = self.madctl & Self::MADCTL_MV != 0;

        let (mut mem_x, mut mem_y) = if mv {
            let mem_x = if mx {
                width.checked_sub(logical_y + 1).unwrap_or(0)
            } else {
                logical_y
            };
            let mem_y = if my {
                height.checked_sub(x + 1).unwrap_or(0)
            } else {
                x
            };
            (mem_x, mem_y)
        } else {
            let mem_x = if mx {
                width.checked_sub(x + 1).unwrap_or(0)
            } else {
                x
            };
            let mem_y = if my {
                height.checked_sub(logical_y + 1).unwrap_or(0)
            } else {
                logical_y
            };
            (mem_x, mem_y)
        };

        // ST7789 is typically 240x320 GRAM.
        // For smaller screens (e.g., 240x240, 135x240), there are physical offsets.
        let (offset_x, offset_y) = match (config.width, config.height) {
            (240, 240) => (0, 80),
            (135, 240) => (52, 40),
            _ => (0, 0),
        };

        mem_x = mem_x.saturating_add(offset_x);
        mem_y = mem_y.saturating_add(offset_y);

        if mem_x >= 240 || mem_y >= 320 {
            // Out of GRAM bounds
            return Err(LcdError::OutOfBounds);
        }

        Ok((mem_x, mem_y))
    }

    pub(crate) fn write_pixel_coords(
        &self,
        window: DrawWindow,
        next_pixel: usize,
        config: &LcdConfig,
    ) -> Result<(u16, u16)> {
        let dx = (next_pixel % window.width as usize) as u16;
        let dy = (next_pixel / window.width as usize) as u16;
        self.map_logical_to_memory(window.x + dx, window.y + dy, config)
    }

    pub(crate) fn apply_visible_transform(
        &self,
        memory: &Framebuffer,
        visible: &mut Framebuffer,
        state: &LcdState,
        config: &LcdConfig,
    ) -> Result<()> {
        if !state.display_on || state.sleeping || state.backlight == 0 || self.brightness == 0 {
            visible.clear(Color::BLACK);
            return Ok(());
        }

        let is_bgr = self.madctl & Self::MADCTL_BGR != 0;

        for y in 0..config.height {
            for x in 0..config.width {
                let (mem_x, mem_y) = self.map_logical_to_memory(x, y, config)?;
                let mut color = memory.get_pixel(mem_x, mem_y).unwrap_or(Color::BLACK);

                if is_bgr {
                    color = Color::rgb(color.b, color.g, color.r);
                }
                
                // Typically ST7789 uses inversion by default for correct colors on some panels
                if self.inversion_on {
                    color = Color::rgb(255 - color.r, 255 - color.g, 255 - color.b);
                }

                visible.set_pixel(x, y, color)?;
            }
        }

        Ok(())
    }

    pub(crate) fn power_mode(&self, state: &LcdState) -> u8 {
        let mut mode = 0u8;
        if !state.sleeping {
            mode |= 0x08;
        }
        if state.display_on {
            mode |= 0x04;
        }
        if self.interface_pixel_format() == PixelFormat::Rgb565 {
            mode |= 0x02;
        }
        if state.initialized {
            mode |= 0x80;
        }
        mode
    }
}
