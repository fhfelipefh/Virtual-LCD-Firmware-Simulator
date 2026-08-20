use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ssd1306AddressingMode {
    Horizontal,
    Vertical,
    Page,
}

#[derive(Debug)]
pub(crate) struct Ssd1306State {
    pub(crate) gddram: Vec<u8>,
    pub(crate) memory_mode: Ssd1306AddressingMode,
    pub(crate) column_start: u8,
    pub(crate) column_end: u8,
    pub(crate) page_start: u8,
    pub(crate) page_end: u8,
    pub(crate) column: u8,
    pub(crate) page: u8,
    pub(crate) start_line: u8,
    pub(crate) display_offset: u8,
    pub(crate) contrast: u8,
    pub(crate) multiplex_ratio: u8,
    pub(crate) clock_div: u8,
    pub(crate) precharge: u8,
    pub(crate) com_pins: u8,
    pub(crate) vcomh: u8,
    pub(crate) charge_pump: u8,
    pub(crate) segment_remap: bool,
    pub(crate) com_scan_reverse: bool,
    pub(crate) entire_display_on: bool,
    pub(crate) inverse_display: bool,
    pub(crate) scroll_enabled: bool,
    pub(crate) raw_registers: BTreeMap<u8, Vec<u8>>,
}

impl Ssd1306State {
    pub(crate) fn new(config: &LcdConfig) -> Self {
        let pages = (config.height / 8).max(1);
        Self {
            gddram: vec![0x00; config.width as usize * pages as usize],
            memory_mode: Ssd1306AddressingMode::Page,
            column_start: 0,
            column_end: config.width.saturating_sub(1) as u8,
            page_start: 0,
            page_end: pages.saturating_sub(1) as u8,
            column: 0,
            page: 0,
            start_line: 0,
            display_offset: 0,
            contrast: 0x7F,
            multiplex_ratio: config.height.saturating_sub(1) as u8,
            clock_div: 0x80,
            precharge: 0xF1,
            com_pins: if config.height > 32 { 0x12 } else { 0x02 },
            vcomh: 0x20,
            charge_pump: 0x14,
            segment_remap: false,
            com_scan_reverse: false,
            entire_display_on: false,
            inverse_display: false,
            scroll_enabled: false,
            raw_registers: BTreeMap::new(),
        }
    }
    pub(crate) fn normalize_color(&self, color: Color) -> Color {
        if color.luminance() >= 128 {
            Color::WHITE
        } else {
            Color::BLACK
        }
    }
    pub(crate) fn pages(&self, config: &LcdConfig) -> u8 {
        (config.height / 8).max(1) as u8
    }
    pub(crate) fn clamp_column(&self, column: u8, config: &LcdConfig) -> u8 {
        column.min(config.width.saturating_sub(1) as u8)
    }
    pub(crate) fn clamp_page(&self, page: u8, config: &LcdConfig) -> u8 {
        page.min(self.pages(config).saturating_sub(1))
    }
    pub(crate) fn gddram_index(&self, x: u16, page: u8, config: &LcdConfig) -> Option<usize> {
        if x >= config.width || page >= self.pages(config) {
            return None;
        }

        Some(page as usize * config.width as usize + x as usize)
    }
    pub(crate) fn sync_gddram_byte_to_frame(
        &self,
        frame: &mut Framebuffer,
        column: u8,
        page: u8,
        config: &LcdConfig,
    ) -> Result<()> {
        let x = column as u16;
        let Some(index) = self.gddram_index(x, page, config) else {
            return Ok(());
        };
        let byte = self.gddram[index];
        let base_y = page as u16 * 8;

        for bit in 0..8u16 {
            let y = base_y + bit;
            if y >= config.height {
                break;
            }

            let color = if (byte >> bit) & 0x01 != 0 {
                Color::WHITE
            } else {
                Color::BLACK
            };
            frame.set_pixel(x, y, color)?;
        }

        Ok(())
    }
    pub(crate) fn set_native_pixel(&mut self, x: u16, y: u16, on: bool, config: &LcdConfig) -> Result<()> {
        let page = (y / 8) as u8;
        let bit = (y % 8) as u8;
        let index = self
            .gddram_index(x, page, config)
            .ok_or(LcdError::OutOfBounds)?;

        if on {
            self.gddram[index] |= 1 << bit;
        } else {
            self.gddram[index] &= !(1 << bit);
        }

        Ok(())
    }
    pub(crate) fn sync_pixel_from_color(
        &mut self,
        frame: &mut Framebuffer,
        x: u16,
        y: u16,
        color: Color,
        config: &LcdConfig,
    ) -> Result<()> {
        let mono = self.normalize_color(color);
        frame.set_pixel(x, y, mono)?;
        self.set_native_pixel(x, y, mono == Color::WHITE, config)
    }
    pub(crate) fn sync_window_from_frame(
        &mut self,
        frame: &mut Framebuffer,
        window: DrawWindow,
        config: &LcdConfig,
    ) -> Result<()> {
        for y in window.y..window.y + window.height {
            for x in window.x..window.x + window.width {
                let color = frame.get_pixel(x, y).unwrap_or(Color::BLACK);
                self.sync_pixel_from_color(frame, x, y, color, config)?;
            }
        }

        Ok(())
    }
    pub(crate) fn set_column_address(&mut self, start: u8, end: u8, config: &LcdConfig) {
        self.column_start = self.clamp_column(start, config);
        self.column_end = self.clamp_column(end, config).max(self.column_start);
        self.column = self.column_start;
    }
    pub(crate) fn set_page_address(&mut self, start: u8, end: u8, config: &LcdConfig) {
        self.page_start = self.clamp_page(start, config);
        self.page_end = self.clamp_page(end, config).max(self.page_start);
        self.page = self.page_start;
    }
    pub(crate) fn set_page_mode_page(&mut self, page: u8, config: &LcdConfig) {
        self.page = self.clamp_page(page, config);
    }
    pub(crate) fn set_page_mode_lower_column(&mut self, lower: u8, config: &LcdConfig) {
        self.column = self.clamp_column((self.column & 0xF0) | (lower & 0x0F), config);
    }
    pub(crate) fn set_page_mode_upper_column(&mut self, upper: u8, config: &LcdConfig) {
        self.column = self.clamp_column((self.column & 0x0F) | ((upper & 0x0F) << 4), config);
    }
    pub(crate) fn advance_address(&mut self, config: &LcdConfig) {
        match self.memory_mode {
            Ssd1306AddressingMode::Horizontal => {
                if self.column >= self.column_end {
                    self.column = self.column_start;
                    if self.page >= self.page_end {
                        self.page = self.page_start;
                    } else {
                        self.page += 1;
                    }
                } else {
                    self.column += 1;
                }
            }
            Ssd1306AddressingMode::Vertical => {
                if self.page >= self.page_end {
                    self.page = self.page_start;
                    if self.column >= self.column_end {
                        self.column = self.column_start;
                    } else {
                        self.column += 1;
                    }
                } else {
                    self.page += 1;
                }
            }
            Ssd1306AddressingMode::Page => {
                let max_column = config.width.saturating_sub(1) as u8;
                if self.column >= max_column {
                    self.column = 0;
                } else {
                    self.column += 1;
                }
            }
        }
    }
    pub(crate) fn write_ram_bytes(
        &mut self,
        frame: &mut Framebuffer,
        data: &[u8],
        config: &LcdConfig,
    ) -> Result<usize> {
        for byte in data.iter().copied() {
            let column = self.clamp_column(self.column, config);
            let page = self.clamp_page(self.page, config);
            if let Some(index) = self.gddram_index(column as u16, page, config) {
                self.gddram[index] = byte;
                self.sync_gddram_byte_to_frame(frame, column, page, config)?;
            }
            self.advance_address(config);
        }

        Ok(data.len())
    }
    pub(crate) fn apply_visible_transform(
        &self,
        visible: &mut Framebuffer,
        state: &LcdState,
        config: &LcdConfig,
    ) -> Result<()> {
        if !state.display_on || state.backlight == 0 {
            visible.clear(Color::BLACK);
            return Ok(());
        }

        let height = config.height;
        let width = config.width;

        for y in 0..height {
            let logical_y = if self.com_scan_reverse {
                height - 1 - y
            } else {
                y
            };
            let memory_y =
                (logical_y + self.start_line as u16 + self.display_offset as u16) % height.max(1);

            for x in 0..width {
                let memory_x = if self.segment_remap {
                    width - 1 - x
                } else {
                    x
                };

                let pixel_on = if self.entire_display_on {
                    true
                } else {
                    let page = (memory_y / 8) as u8;
                    let bit = (memory_y % 8) as u8;
                    let Some(index) = self.gddram_index(memory_x, page, config) else {
                        continue;
                    };
                    let mut on = (self.gddram[index] >> bit) & 0x01 != 0;
                    if self.inverse_display {
                        on = !on;
                    }
                    on
                };

                visible.set_pixel(x, y, if pixel_on { Color::WHITE } else { Color::BLACK })?;
            }
        }

        Ok(())
    }
}
