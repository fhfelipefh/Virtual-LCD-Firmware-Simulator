#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use std::collections::BTreeMap;
use instant::Instant;

pub use virtual_lcd_sdk::{Color, Lcd, LcdBus, PinId};

pub type Result<T> = std::result::Result<T, LcdError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerModel {
    GenericMipiDcs,
    Ili9341,
    Ssd1306,
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
    fn validate(&self) -> Result<()> {
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

    fn decode_color(self, bytes: &[u8]) -> Color {
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

#[derive(Clone, Debug)]
pub struct LcdState {
    pub initialized: bool,
    pub sleeping: bool,
    pub display_on: bool,
    pub backlight: u8,
    pub current_window: DrawWindow,
    pub current_command: Option<u8>,
    column_range: (u16, u16),
    row_range: (u16, u16),
}

impl LcdState {
    fn new(config: &LcdConfig) -> Self {
        let full = DrawWindow::full(config);
        Self {
            initialized: false,
            sleeping: true,
            display_on: false,
            backlight: if config.backlight { 100 } else { 0 },
            current_window: full,
            current_command: None,
            column_range: (0, config.width - 1),
            row_range: (0, config.height - 1),
        }
    }

    fn set_column_range(&mut self, start: u16, end: u16) {
        self.column_range = (start, end);
        self.sync_window();
    }

    fn set_row_range(&mut self, start: u16, end: u16) {
        self.row_range = (start, end);
        self.sync_window();
    }

    fn sync_window(&mut self) {
        self.current_window = DrawWindow {
            x: self.column_range.0,
            y: self.row_range.0,
            width: self.column_range.1 - self.column_range.0 + 1,
            height: self.row_range.1 - self.row_range.0 + 1,
        };
    }
}

#[derive(Debug)]
enum ControllerRuntime {
    Generic,
    Ili9341(Ili9341State),
    Ssd1306(Ssd1306State),
}

impl ControllerRuntime {
    fn new(model: ControllerModel, config: &LcdConfig) -> Self {
        match model {
            ControllerModel::GenericMipiDcs => Self::Generic,
            ControllerModel::Ili9341 => Self::Ili9341(Ili9341State::new(config)),
            ControllerModel::Ssd1306 => Self::Ssd1306(Ssd1306State::new(config)),
        }
    }

    fn reset(&mut self, config: &LcdConfig) {
        match self {
            Self::Generic => {}
            Self::Ili9341(state) => *state = Ili9341State::new(config),
            Self::Ssd1306(state) => *state = Ssd1306State::new(config),
        }
    }

    fn visible_bytes_per_pixel(&self, fallback: PixelFormat) -> usize {
        match self {
            Self::Generic => fallback.bytes_per_pixel(),
            Self::Ili9341(state) => state.interface_pixel_format().bytes_per_pixel(),
            Self::Ssd1306(_) => PixelFormat::Mono1.bytes_per_pixel(),
        }
    }

    fn native_frame_bytes(&self, config: &LcdConfig) -> usize {
        match self {
            Self::Generic | Self::Ili9341(_) => config.full_frame_bytes(),
            Self::Ssd1306(state) => state.gddram.len(),
        }
    }
}

#[derive(Debug)]
struct Ili9341State {
    madctl: u8,
    colmod: u8,
    inversion_on: bool,
    tearing_enabled: bool,
    tearing_mode: u8,
    brightness: u8,
    control_display: u8,
    scroll: VerticalScrollState,
    interface_control: [u8; 3],
    raw_registers: BTreeMap<u8, Vec<u8>>,
}

impl Ili9341State {
    const MADCTL_MY: u8 = 0x80;
    const MADCTL_MX: u8 = 0x40;
    const MADCTL_MV: u8 = 0x20;
    const MADCTL_BGR: u8 = 0x08;

    fn new(config: &LcdConfig) -> Self {
        Self {
            madctl: 0x00,
            colmod: 0x66,
            inversion_on: false,
            tearing_enabled: config.tearing_effect,
            tearing_mode: 0x00,
            brightness: if config.backlight { 0xFF } else { 0x00 },
            control_display: 0x24,
            scroll: VerticalScrollState::new(config.height),
            interface_control: [0x01, 0x00, 0x00],
            raw_registers: BTreeMap::new(),
        }
    }

    fn interface_pixel_format(&self) -> PixelFormat {
        match self.colmod & 0x07 {
            0x05 => PixelFormat::Rgb565,
            0x06 => PixelFormat::Rgb888,
            _ => PixelFormat::Rgb565,
        }
    }

    fn decode_interface_color(&self, bytes: &[u8]) -> Color {
        match self.interface_pixel_format() {
            PixelFormat::Rgb565 => PixelFormat::Rgb565.decode_color(bytes),
            PixelFormat::Rgb888 => {
                let expand = |value: u8| (value << 2) | (value >> 4);
                Color::rgb(expand(bytes[0]), expand(bytes[1]), expand(bytes[2]))
            }
            other => other.decode_color(bytes),
        }
    }

    fn map_logical_to_memory(&self, x: u16, y: u16, config: &LcdConfig) -> Result<(u16, u16)> {
        let width = config.width;
        let height = config.height;

        let logical_y = self.scroll.map_visible_row(y, height);
        let mx = self.madctl & Self::MADCTL_MX != 0;
        let my = self.madctl & Self::MADCTL_MY != 0;
        let mv = self.madctl & Self::MADCTL_MV != 0;

        let (mem_x, mem_y) = if mv {
            let mem_x = if mx {
                width
                    .checked_sub(logical_y + 1)
                    .ok_or(LcdError::OutOfBounds)?
            } else {
                logical_y
            };
            let mem_y = if my {
                height.checked_sub(x + 1).ok_or(LcdError::OutOfBounds)?
            } else {
                x
            };
            (mem_x, mem_y)
        } else {
            let mem_x = if mx {
                width.checked_sub(x + 1).ok_or(LcdError::OutOfBounds)?
            } else {
                x
            };
            let mem_y = if my {
                height
                    .checked_sub(logical_y + 1)
                    .ok_or(LcdError::OutOfBounds)?
            } else {
                logical_y
            };
            (mem_x, mem_y)
        };

        if mem_x >= width || mem_y >= height {
            return Err(LcdError::OutOfBounds);
        }

        Ok((mem_x, mem_y))
    }

    fn write_pixel_coords(
        &self,
        window: DrawWindow,
        next_pixel: usize,
        config: &LcdConfig,
    ) -> Result<(u16, u16)> {
        let dx = (next_pixel % window.width as usize) as u16;
        let dy = (next_pixel / window.width as usize) as u16;
        self.map_logical_to_memory(window.x + dx, window.y + dy, config)
    }

    fn apply_visible_transform(
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

        for y in 0..config.height {
            for x in 0..config.width {
                let (mem_x, mem_y) = self.map_logical_to_memory(x, y, config)?;
                let mut color = memory.get_pixel(mem_x, mem_y).unwrap_or(Color::BLACK);
                if self.madctl & Self::MADCTL_BGR != 0 {
                    color = Color::rgb(color.b, color.g, color.r);
                }
                visible.set_pixel(x, y, color)?;
            }
        }

        Ok(())
    }

    fn power_mode(&self, state: &LcdState) -> u8 {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ssd1306AddressingMode {
    Horizontal,
    Vertical,
    Page,
}

#[derive(Debug)]
struct Ssd1306State {
    gddram: Vec<u8>,
    memory_mode: Ssd1306AddressingMode,
    column_start: u8,
    column_end: u8,
    page_start: u8,
    page_end: u8,
    column: u8,
    page: u8,
    start_line: u8,
    display_offset: u8,
    contrast: u8,
    multiplex_ratio: u8,
    clock_div: u8,
    precharge: u8,
    com_pins: u8,
    vcomh: u8,
    charge_pump: u8,
    segment_remap: bool,
    com_scan_reverse: bool,
    entire_display_on: bool,
    inverse_display: bool,
    scroll_enabled: bool,
    raw_registers: BTreeMap<u8, Vec<u8>>,
}

impl Ssd1306State {
    fn new(config: &LcdConfig) -> Self {
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

    fn normalize_color(&self, color: Color) -> Color {
        if color.luminance() >= 128 {
            Color::WHITE
        } else {
            Color::BLACK
        }
    }

    fn pages(&self, config: &LcdConfig) -> u8 {
        (config.height / 8).max(1) as u8
    }

    fn clamp_column(&self, column: u8, config: &LcdConfig) -> u8 {
        column.min(config.width.saturating_sub(1) as u8)
    }

    fn clamp_page(&self, page: u8, config: &LcdConfig) -> u8 {
        page.min(self.pages(config).saturating_sub(1))
    }

    fn gddram_index(&self, x: u16, page: u8, config: &LcdConfig) -> Option<usize> {
        if x >= config.width || page >= self.pages(config) {
            return None;
        }

        Some(page as usize * config.width as usize + x as usize)
    }

    fn sync_gddram_byte_to_frame(
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

    fn set_native_pixel(&mut self, x: u16, y: u16, on: bool, config: &LcdConfig) -> Result<()> {
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

    fn sync_pixel_from_color(
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

    fn sync_window_from_frame(
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

    fn set_column_address(&mut self, start: u8, end: u8, config: &LcdConfig) {
        self.column_start = self.clamp_column(start, config);
        self.column_end = self.clamp_column(end, config).max(self.column_start);
        self.column = self.column_start;
    }

    fn set_page_address(&mut self, start: u8, end: u8, config: &LcdConfig) {
        self.page_start = self.clamp_page(start, config);
        self.page_end = self.clamp_page(end, config).max(self.page_start);
        self.page = self.page_start;
    }

    fn set_page_mode_page(&mut self, page: u8, config: &LcdConfig) {
        self.page = self.clamp_page(page, config);
    }

    fn set_page_mode_lower_column(&mut self, lower: u8, config: &LcdConfig) {
        self.column = self.clamp_column((self.column & 0xF0) | (lower & 0x0F), config);
    }

    fn set_page_mode_upper_column(&mut self, upper: u8, config: &LcdConfig) {
        self.column = self.clamp_column((self.column & 0x0F) | ((upper & 0x0F) << 4), config);
    }

    fn advance_address(&mut self, config: &LcdConfig) {
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

    fn write_ram_bytes(
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

    fn apply_visible_transform(
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

#[derive(Debug)]
struct VerticalScrollState {
    top_fixed_area: u16,
    scroll_area: u16,
    bottom_fixed_area: u16,
    start_address: u16,
}

impl VerticalScrollState {
    fn new(height: u16) -> Self {
        Self {
            top_fixed_area: 0,
            scroll_area: height,
            bottom_fixed_area: 0,
            start_address: 0,
        }
    }

    fn map_visible_row(&self, row: u16, total_height: u16) -> u16 {
        if self.top_fixed_area + self.scroll_area + self.bottom_fixed_area != total_height {
            return row;
        }

        if row < self.top_fixed_area {
            return row;
        }

        if row >= self.top_fixed_area + self.scroll_area {
            return row;
        }

        if self.scroll_area == 0 {
            return row;
        }

        let offset = row - self.top_fixed_area;
        self.top_fixed_area + ((offset + self.start_address) % self.scroll_area)
    }
}

#[derive(Debug)]
struct RegisterWrite {
    register: RegisterKind,
    allowed_lengths: &'static [usize],
}

#[derive(Debug, Clone, Copy)]
enum RegisterKind {
    Madctl,
    Colmod,
    VerticalScrollDefinition,
    VerticalScrollStart,
    Brightness,
    ControlDisplay,
    InterfaceControl,
    Ssd1306MemoryMode,
    Ssd1306ColumnAddress,
    Ssd1306PageAddress,
    Ssd1306Contrast,
    Ssd1306MultiplexRatio,
    Ssd1306DisplayOffset,
    Ssd1306ClockDiv,
    Ssd1306Precharge,
    Ssd1306Compins,
    Ssd1306Vcomh,
    Ssd1306ChargePump,
    Raw(u8),
}

#[derive(Clone, Debug)]
pub struct Framebuffer {
    width: u16,
    height: u16,
    pixels: Vec<Color>,
}

impl Framebuffer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            pixels: vec![Color::BLACK; width as usize * height as usize],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    pub fn clear(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    pub fn copy_from(&mut self, other: &Self) {
        self.pixels.clone_from_slice(&other.pixels);
    }

    pub fn get_pixel(&self, x: u16, y: u16) -> Option<Color> {
        let index = self.index_of(x, y)?;
        Some(self.pixels[index])
    }

    pub fn set_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        let index = self.index_of(x, y).ok_or(LcdError::OutOfBounds)?;
        self.pixels[index] = color;
        Ok(())
    }

    pub fn fill_rect(&mut self, window: DrawWindow, color: Color) -> Result<()> {
        for y in window.y..window.y + window.height {
            for x in window.x..window.x + window.width {
                self.set_pixel(x, y, color)?;
            }
        }
        Ok(())
    }

    fn index_of(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(y as usize * self.width as usize + x as usize)
    }
}

#[derive(Clone, Debug)]
pub struct PinBank {
    levels: [bool; 9],
}

impl Default for PinBank {
    fn default() -> Self {
        let mut levels = [false; 9];
        levels[PinId::Cs.index()] = true;
        levels[PinId::Rst.index()] = true;
        levels[PinId::Wr.index()] = true;
        levels[PinId::Rd.index()] = true;
        levels[PinId::Bl.index()] = true;
        Self { levels }
    }
}

impl PinBank {
    pub fn level(&self, pin: PinId) -> bool {
        self.levels[pin.index()]
    }

    fn set(&mut self, pin: PinId, value: bool) {
        self.levels[pin.index()] = value;
    }
}

#[derive(Debug)]
struct TimingEngine {
    frame_interval: Duration,
    bus_hz: u32,
    last_visible_at: Instant,
    pending_ready_at: Option<Instant>,
}

impl TimingEngine {
    fn new(config: &LcdConfig) -> Self {
        let frame_interval = config.frame_interval();
        Self {
            frame_interval,
            bus_hz: config.bus_hz,
            last_visible_at: Instant::now() - frame_interval,
            pending_ready_at: None,
        }
    }

    fn schedule_transfer(&mut self, bytes: usize, vsync: bool) -> Result<Instant> {
        let now = Instant::now();

        if let Some(ready_at) = self.pending_ready_at {
            if ready_at > now {
                return Err(LcdError::FrameRateExceeded);
            }
        }

        let transfer_secs = (bytes as f64 * 8.0) / self.bus_hz as f64;
        let bus_time = Duration::from_secs_f64(transfer_secs.max(0.0));
        let earliest = if vsync {
            self.last_visible_at + self.frame_interval
        } else {
            now
        };
        let ready_at = max_instant(now + bus_time, earliest);

        self.pending_ready_at = Some(ready_at);
        Ok(ready_at)
    }

    fn tick(&mut self) -> bool {
        match self.pending_ready_at {
            Some(ready_at) if Instant::now() >= ready_at => {
                self.last_visible_at = ready_at;
                self.pending_ready_at = None;
                true
            }
            _ => false,
        }
    }

    fn time_until_ready(&self) -> Option<Duration> {
        self.pending_ready_at.map(|ready_at| ready_at.saturating_duration_since(Instant::now()))
    }

    fn clear_pending(&mut self) {
        self.pending_ready_at = None;
    }
}

#[derive(Debug)]
enum PendingWrite {
    None,
    Column(AddressAccumulator),
    Row(AddressAccumulator),
    Register(RegisterWrite),
    MemoryWrite(MemoryWriteProgress),
}

#[derive(Debug)]
struct AddressAccumulator {
    bytes: [u8; 4],
    len: usize,
}

impl AddressAccumulator {
    fn new() -> Self {
        Self {
            bytes: [0; 4],
            len: 0,
        }
    }

    fn push(&mut self, data: &[u8]) -> usize {
        let available = 4 - self.len;
        let take = available.min(data.len());
        self.bytes[self.len..self.len + take].copy_from_slice(&data[..take]);
        self.len += take;
        take
    }

    fn complete(&self) -> bool {
        self.len == 4
    }

    fn decode(&self) -> (u16, u16) {
        let start = u16::from_be_bytes([self.bytes[0], self.bytes[1]]);
        let end = u16::from_be_bytes([self.bytes[2], self.bytes[3]]);
        (start, end)
    }
}

#[derive(Debug)]
struct MemoryWriteProgress {
    window: DrawWindow,
    next_pixel: usize,
    partial_pixel: [u8; 3],
    partial_len: usize,
    transferred_bytes: usize,
}

impl MemoryWriteProgress {
    fn new(window: DrawWindow) -> Self {
        Self {
            window,
            next_pixel: 0,
            partial_pixel: [0; 3],
            partial_len: 0,
            transferred_bytes: 0,
        }
    }

    fn total_pixels(&self) -> usize {
        self.window.area()
    }

    fn remaining_bytes(&self, bytes_per_pixel: usize) -> usize {
        (self.total_pixels() - self.next_pixel) * bytes_per_pixel - self.partial_len
    }

    fn finished(&self) -> bool {
        self.next_pixel == self.total_pixels() && self.partial_len == 0
    }

    fn current_coords(&self) -> (u16, u16) {
        let dx = (self.next_pixel % self.window.width as usize) as u16;
        let dy = (self.next_pixel / self.window.width as usize) as u16;
        (self.window.x + dx, self.window.y + dy)
    }
}

#[derive(Debug)]
pub struct VirtualLcd {
    config: LcdConfig,
    state: LcdState,
    controller: ControllerRuntime,
    front_buffer: Framebuffer,
    back_buffer: Framebuffer,
    pins: PinBank,
    timing: TimingEngine,
    pending_write: PendingWrite,
}

impl VirtualLcd {
    pub fn new(config: LcdConfig) -> Result<Self> {
        config.validate()?;

        let front_buffer = Framebuffer::new(config.width, config.height);
        let back_buffer = Framebuffer::new(config.width, config.height);
        let state = LcdState::new(&config);
        let controller = ControllerRuntime::new(config.controller, &config);
        let timing = TimingEngine::new(&config);

        Ok(Self {
            config,
            state,
            controller,
            front_buffer,
            back_buffer,
            pins: PinBank::default(),
            timing,
            pending_write: PendingWrite::None,
        })
    }

    pub fn config(&self) -> &LcdConfig {
        &self.config
    }

    pub fn state(&self) -> &LcdState {
        &self.state
    }

    pub fn pins(&self) -> &PinBank {
        &self.pins
    }

    pub fn visible_frame(&self) -> &Framebuffer {
        &self.front_buffer
    }

    pub fn working_frame(&self) -> &Framebuffer {
        &self.back_buffer
    }

    pub fn controller_model(&self) -> ControllerModel {
        self.config.controller
    }

    pub fn set_window(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        let window = DrawWindow::from_origin(x, y, width, height, &self.config)?;
        self.state.set_column_range(window.x, window.x + window.width - 1);
        self.state.set_row_range(window.y, window.y + window.height - 1);
        Ok(())
    }

    pub fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        let window = DrawWindow::from_inclusive(x0, y0, x1, y1, &self.config)?;
        self.state.set_column_range(window.x, window.x + window.width - 1);
        self.state.set_row_range(window.y, window.y + window.height - 1);
        Ok(())
    }

    pub fn write_pixels(&mut self, pixels: &[Color]) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        let expected = self.state.current_window.area();
        if pixels.len() != expected {
            return Err(LcdError::InvalidDataLength {
                expected,
                got: pixels.len(),
            });
        }

        let window = self.state.current_window;
        for (index, color) in pixels.iter().copied().enumerate() {
            let dx = (index % window.width as usize) as u16;
            let dy = (index / window.width as usize) as u16;
            let color = self.normalize_high_level_color(color);
            self.back_buffer
                .set_pixel(window.x + dx, window.y + dy, color)?;
            self.sync_controller_pixel(window.x + dx, window.y + dy, color)?;
        }

        self.schedule_visible_update(expected * self.config.pixel_format.bytes_per_pixel())
    }

    pub fn tick(&mut self) -> bool {
        if self.timing.tick() {
            let _ = self.rebuild_visible_frame();
            if self.config.buffering == BufferingMode::Single
                && matches!(self.controller, ControllerRuntime::Generic)
            {
                self.back_buffer.copy_from(&self.front_buffer);
            }
            return true;
        }

        false
    }

    pub fn time_until_ready(&self) -> Option<Duration> {
        self.timing.time_until_ready()
    }

    pub fn has_pending_frame(&self) -> bool {
        self.timing.pending_ready_at.is_some()
    }

    fn hardware_reset(&mut self) {
        self.front_buffer.clear(Color::BLACK);
        self.back_buffer.clear(Color::BLACK);
        self.state = LcdState::new(&self.config);
        self.controller.reset(&self.config);
        self.pending_write = PendingWrite::None;
        self.timing.clear_pending();
    }

    fn ensure_ready_for_graphics(&self) -> Result<()> {
        if !self.state.initialized {
            return Err(LcdError::NotInitialized);
        }

        if self.state.sleeping {
            return Err(LcdError::SleepMode);
        }

        if !self.state.display_on {
            return Err(LcdError::DisplayOff);
        }

        Ok(())
    }

    fn ensure_memory_access(&self) -> Result<()> {
        if !self.state.initialized {
            return Err(LcdError::NotInitialized);
        }

        if self.state.sleeping {
            return Err(LcdError::SleepMode);
        }

        Ok(())
    }

    fn validate_bus_access(&self) -> Result<()> {
        if self.pins.level(PinId::Cs) {
            return Err(LcdError::BusViolation("cannot access bus while CS is high"));
        }

        if !self.pins.level(PinId::Rst) {
            return Err(LcdError::BusViolation("cannot access bus while reset is asserted"));
        }

        Ok(())
    }

    fn schedule_visible_update(&mut self, bytes: usize) -> Result<()> {
        self.timing.schedule_transfer(bytes, self.config.vsync)?;
        Ok(())
    }

    fn rebuild_visible_frame(&mut self) -> Result<()> {
        match &self.controller {
            ControllerRuntime::Generic => {
                if self.state.display_on && !self.state.sleeping && self.state.backlight > 0 {
                    self.front_buffer.copy_from(&self.back_buffer);
                } else {
                    self.front_buffer.clear(Color::BLACK);
                }
            }
            ControllerRuntime::Ili9341(controller) => {
                controller.apply_visible_transform(
                    &self.back_buffer,
                    &mut self.front_buffer,
                    &self.state,
                    &self.config,
                )?;
            }
            ControllerRuntime::Ssd1306(controller) => {
                controller.apply_visible_transform(
                    &mut self.front_buffer,
                    &self.state,
                    &self.config,
                )?;
            }
        }
        Ok(())
    }

    fn normalize_high_level_color(&self, color: Color) -> Color {
        match &self.controller {
            ControllerRuntime::Ssd1306(controller) => controller.normalize_color(color),
            _ => color,
        }
    }

    fn sync_controller_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
            controller.sync_pixel_from_color(&mut self.back_buffer, x, y, color, &self.config)?;
        }
        Ok(())
    }

    fn sync_controller_window(&mut self, window: DrawWindow) -> Result<()> {
        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
            controller.sync_window_from_frame(&mut self.back_buffer, window, &self.config)?;
        }
        Ok(())
    }

    fn process_address_data(&mut self, accumulator: &mut AddressAccumulator, data: &[u8], is_column: bool) -> Result<usize> {
        let consumed = accumulator.push(data);
        if accumulator.complete() {
            let (start, end) = accumulator.decode();
            let window = if is_column {
                DrawWindow::from_inclusive(
                    start,
                    self.state.current_window.y,
                    end,
                    self.state.current_window.y + self.state.current_window.height - 1,
                    &self.config,
                )?
            } else {
                DrawWindow::from_inclusive(
                    self.state.current_window.x,
                    start,
                    self.state.current_window.x + self.state.current_window.width - 1,
                    end,
                    &self.config,
                )?
            };

            self.state.set_column_range(window.x, window.x + window.width - 1);
            self.state.set_row_range(window.y, window.y + window.height - 1);
        }

        Ok(consumed)
    }

    fn process_memory_write(&mut self, progress: &mut MemoryWriteProgress, data: &[u8]) -> Result<usize> {
        self.ensure_memory_access()?;

        let bytes_per_pixel = self.controller.visible_bytes_per_pixel(self.config.pixel_format);
        if data.len() > progress.remaining_bytes(bytes_per_pixel) {
            return Err(LcdError::InvalidDataLength {
                expected: progress.remaining_bytes(bytes_per_pixel),
                got: data.len(),
            });
        }

        for byte in data.iter().copied() {
            progress.partial_pixel[progress.partial_len] = byte;
            progress.partial_len += 1;
            progress.transferred_bytes += 1;

            if progress.partial_len == bytes_per_pixel {
                let color = match &self.controller {
                    ControllerRuntime::Generic => self
                        .config
                        .pixel_format
                        .decode_color(&progress.partial_pixel[..bytes_per_pixel]),
                    ControllerRuntime::Ili9341(controller) => {
                        controller.decode_interface_color(&progress.partial_pixel[..bytes_per_pixel])
                    }
                    ControllerRuntime::Ssd1306(_) => {
                        unreachable!("ssd1306 does not use MIPI-style memory write sequencing")
                    }
                };
                let (x, y) = match &self.controller {
                    ControllerRuntime::Generic => progress.current_coords(),
                    ControllerRuntime::Ili9341(controller) => {
                        controller.write_pixel_coords(progress.window, progress.next_pixel, &self.config)?
                    }
                    ControllerRuntime::Ssd1306(_) => {
                        unreachable!("ssd1306 does not use MIPI-style memory write sequencing")
                    }
                };
                self.back_buffer.set_pixel(x, y, color)?;
                progress.partial_len = 0;
                progress.next_pixel += 1;
            }
        }

        Ok(data.len())
    }

    fn process_ssd1306_ram_write(&mut self, data: &[u8]) -> Result<usize> {
        self.ensure_memory_access()?;

        match &mut self.controller {
            ControllerRuntime::Ssd1306(controller) => {
                controller.write_ram_bytes(&mut self.back_buffer, data, &self.config)
            }
            _ => Err(LcdError::BusViolation(
                "ssd1306 RAM write requested for a non-ssd1306 controller",
            )),
        }
    }

    fn process_register_write(&mut self, write: RegisterWrite, data: &[u8]) -> Result<()> {
        if !write.allowed_lengths.contains(&data.len()) {
            return Err(LcdError::InvalidDataLength {
                expected: *write.allowed_lengths.first().unwrap_or(&0),
                got: data.len(),
            });
        }

        let mut refresh_visible = false;
        match (&mut self.controller, write.register) {
            (ControllerRuntime::Generic, RegisterKind::Raw(_)) => {}
            (ControllerRuntime::Ili9341(controller), RegisterKind::Madctl) => {
                controller.madctl = data[0];
                refresh_visible = true;
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::Colmod) => {
                controller.colmod = data[0];
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::VerticalScrollDefinition) => {
                controller.scroll.top_fixed_area = u16::from_be_bytes([data[0], data[1]]);
                controller.scroll.scroll_area = u16::from_be_bytes([data[2], data[3]]);
                controller.scroll.bottom_fixed_area = u16::from_be_bytes([data[4], data[5]]);
                refresh_visible = true;
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::VerticalScrollStart) => {
                controller.scroll.start_address =
                    u16::from_be_bytes([data[0], data[1]]) % controller.scroll.scroll_area.max(1);
                refresh_visible = true;
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::Brightness) => {
                controller.brightness = data[0];
                refresh_visible = true;
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::ControlDisplay) => {
                controller.control_display = data[0];
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::InterfaceControl) => {
                controller.interface_control.copy_from_slice(&data[..3]);
            }
            (ControllerRuntime::Ili9341(controller), RegisterKind::Raw(cmd)) => {
                controller.raw_registers.insert(cmd, data.to_vec());
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306MemoryMode) => {
                controller.memory_mode = match data[0] & 0x03 {
                    0x00 => Ssd1306AddressingMode::Horizontal,
                    0x01 => Ssd1306AddressingMode::Vertical,
                    _ => Ssd1306AddressingMode::Page,
                };
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306ColumnAddress) => {
                controller.set_column_address(data[0], data[1], &self.config);
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306PageAddress) => {
                controller.set_page_address(data[0], data[1], &self.config);
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306Contrast) => {
                controller.contrast = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306MultiplexRatio) => {
                controller.multiplex_ratio = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306DisplayOffset) => {
                controller.display_offset = data[0] & 0x3F;
                refresh_visible = true;
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306ClockDiv) => {
                controller.clock_div = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306Precharge) => {
                controller.precharge = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306Compins) => {
                controller.com_pins = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306Vcomh) => {
                controller.vcomh = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Ssd1306ChargePump) => {
                controller.charge_pump = data[0];
            }
            (ControllerRuntime::Ssd1306(controller), RegisterKind::Raw(cmd)) => {
                controller.raw_registers.insert(cmd, data.to_vec());
            }
            (ControllerRuntime::Generic, _) => {}
            (ControllerRuntime::Ili9341(_), _) => {}
            (ControllerRuntime::Ssd1306(_), _) => {}
        }

        if refresh_visible {
            self.rebuild_visible_frame()?;
        }

        Ok(())
    }
}

impl Lcd for VirtualLcd {
    type Error = LcdError;

    fn init(&mut self) -> Result<()> {
        self.hardware_reset();
        self.state.initialized = true;
        self.state.sleeping = false;
        self.state.display_on = true;
        self.state.backlight = if self.config.backlight { 100 } else { 0 };
        if let ControllerRuntime::Ili9341(controller) = &mut self.controller {
            controller.brightness = if self.config.backlight { 0xFF } else { 0x00 };
        }
        self.rebuild_visible_frame()?;
        Ok(())
    }

    fn clear(&mut self, color: Color) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        self.back_buffer.clear(self.normalize_high_level_color(color));
        self.sync_controller_window(DrawWindow::full(&self.config))?;
        Ok(())
    }

    fn draw_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        let color = self.normalize_high_level_color(color);
        self.back_buffer.set_pixel(x, y, color)?;
        self.sync_controller_pixel(x, y, color)
    }

    fn fill_rect(&mut self, x: u16, y: u16, width: u16, height: u16, color: Color) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        let window = DrawWindow::from_origin(x, y, width, height, &self.config)?;
        self.back_buffer
            .fill_rect(window, self.normalize_high_level_color(color))?;
        self.sync_controller_window(window)
    }

    fn present(&mut self) -> Result<()> {
        self.ensure_ready_for_graphics()?;

        if !matches!(self.pending_write, PendingWrite::None) {
            return Err(LcdError::BusViolation("cannot present while a bus transaction is active"));
        }

        self.schedule_visible_update(self.controller.native_frame_bytes(&self.config))
    }
}

impl LcdBus for VirtualLcd {
    type Error = LcdError;

    fn set_pin(&mut self, pin: PinId, value: bool) -> Result<()> {
        self.pins.set(pin, value);

        match pin {
            PinId::Rst if !value => self.hardware_reset(),
            PinId::Bl => {
                self.state.backlight = if value { 100 } else { 0 };
                self.rebuild_visible_frame()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn write_command(&mut self, cmd: u8) -> Result<()> {
        self.validate_bus_access()?;

        if !matches!(self.pending_write, PendingWrite::None) {
            return Err(LcdError::BusViolation("cannot start a new command before finishing data phase"));
        }

        self.state.current_command = Some(cmd);

        match self.config.controller {
            ControllerModel::GenericMipiDcs => match cmd {
                0x01 => {
                    self.hardware_reset();
                    self.state.current_command = Some(cmd);
                }
                0x11 => {
                    self.state.initialized = true;
                    self.state.sleeping = false;
                }
                0x28 => {
                    self.ensure_initialized_only()?;
                    self.state.display_on = false;
                }
                0x29 => {
                    self.ensure_initialized_only()?;
                    self.state.display_on = true;
                }
                0x2A => {
                    self.ensure_initialized_only()?;
                    self.pending_write = PendingWrite::Column(AddressAccumulator::new());
                }
                0x2B => {
                    self.ensure_initialized_only()?;
                    self.pending_write = PendingWrite::Row(AddressAccumulator::new());
                }
                0x2C => {
                    self.ensure_memory_access()?;
                    self.pending_write =
                        PendingWrite::MemoryWrite(MemoryWriteProgress::new(self.state.current_window));
                }
                _ => return Err(LcdError::InvalidCommand(cmd)),
            },
            ControllerModel::Ili9341 => match cmd {
                0x01 => {
                    self.hardware_reset();
                    self.state.current_command = Some(cmd);
                }
                0x04 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0F | 0x2E | 0x45 | 0x52 | 0x54 | 0xDA
                | 0xDB | 0xDC => {}
                0x10 => {
                    self.ensure_initialized_only()?;
                    self.state.sleeping = true;
                    self.rebuild_visible_frame()?;
                }
                0x11 => {
                    self.state.initialized = true;
                    self.state.sleeping = false;
                    self.rebuild_visible_frame()?;
                }
                0x13 => {
                    self.state.initialized = true;
                }
                0x20 => {
                    if let ControllerRuntime::Ili9341(controller) = &mut self.controller {
                        controller.inversion_on = false;
                    }
                }
                0x21 => {
                    if let ControllerRuntime::Ili9341(controller) = &mut self.controller {
                        controller.inversion_on = true;
                    }
                }
                0x28 => {
                    self.ensure_initialized_only()?;
                    self.state.display_on = false;
                    self.rebuild_visible_frame()?;
                }
                0x29 => {
                    self.ensure_initialized_only()?;
                    self.state.display_on = true;
                    self.rebuild_visible_frame()?;
                }
                0x2A => {
                    self.ensure_initialized_only()?;
                    self.pending_write = PendingWrite::Column(AddressAccumulator::new());
                }
                0x2B => {
                    self.ensure_initialized_only()?;
                    self.pending_write = PendingWrite::Row(AddressAccumulator::new());
                }
                0x2C => {
                    self.ensure_memory_access()?;
                    self.pending_write =
                        PendingWrite::MemoryWrite(MemoryWriteProgress::new(self.state.current_window));
                }
                0x34 => {
                    if let ControllerRuntime::Ili9341(controller) = &mut self.controller {
                        controller.tearing_enabled = false;
                    }
                }
                0x35 => {
                    if let ControllerRuntime::Ili9341(controller) = &mut self.controller {
                        controller.tearing_enabled = true;
                        controller.tearing_mode = 0x00;
                    }
                }
                other => {
                    if let Some(write) = self.ili9341_register_write_for_command(other) {
                        self.pending_write = PendingWrite::Register(write);
                    } else {
                        return Err(LcdError::InvalidCommand(other));
                    }
                }
            },
            ControllerModel::Ssd1306 => {
                self.state.initialized = true;
                self.state.sleeping = false;

                match cmd {
                    0x00..=0x0F => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.set_page_mode_lower_column(cmd & 0x0F, &self.config);
                        }
                    }
                    0x10..=0x1F => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.set_page_mode_upper_column(cmd & 0x0F, &self.config);
                        }
                    }
                    0x20 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306MemoryMode,
                            allowed_lengths: &[1],
                        });
                    }
                    0x21 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306ColumnAddress,
                            allowed_lengths: &[2],
                        });
                    }
                    0x22 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306PageAddress,
                            allowed_lengths: &[2],
                        });
                    }
                    0x26 | 0x27 | 0x29 | 0x2A => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Raw(cmd),
                            allowed_lengths: &[6],
                        });
                    }
                    0x2E => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.scroll_enabled = false;
                        }
                    }
                    0x2F => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.scroll_enabled = true;
                        }
                    }
                    0x40..=0x7F => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.start_line = cmd & 0x3F;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0x81 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306Contrast,
                            allowed_lengths: &[1],
                        });
                    }
                    0x8D => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306ChargePump,
                            allowed_lengths: &[1],
                        });
                    }
                    0xA0 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.segment_remap = false;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xA1 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.segment_remap = true;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xA3 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Raw(cmd),
                            allowed_lengths: &[2],
                        });
                    }
                    0xA4 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.entire_display_on = false;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xA5 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.entire_display_on = true;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xA6 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.inverse_display = false;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xA7 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.inverse_display = true;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xA8 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306MultiplexRatio,
                            allowed_lengths: &[1],
                        });
                    }
                    0xAE => {
                        self.state.display_on = false;
                        self.rebuild_visible_frame()?;
                    }
                    0xAF => {
                        self.state.display_on = true;
                        self.rebuild_visible_frame()?;
                    }
                    0xB0..=0xB7 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.set_page_mode_page(cmd & 0x0F, &self.config);
                        }
                    }
                    0xC0 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.com_scan_reverse = false;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xC8 => {
                        if let ControllerRuntime::Ssd1306(controller) = &mut self.controller {
                            controller.com_scan_reverse = true;
                        }
                        self.rebuild_visible_frame()?;
                    }
                    0xD3 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306DisplayOffset,
                            allowed_lengths: &[1],
                        });
                    }
                    0xD5 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306ClockDiv,
                            allowed_lengths: &[1],
                        });
                    }
                    0xD9 => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306Precharge,
                            allowed_lengths: &[1],
                        });
                    }
                    0xDA => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306Compins,
                            allowed_lengths: &[1],
                        });
                    }
                    0xDB => {
                        self.pending_write = PendingWrite::Register(RegisterWrite {
                            register: RegisterKind::Ssd1306Vcomh,
                            allowed_lengths: &[1],
                        });
                    }
                    0xE3 => {}
                    other => return Err(LcdError::InvalidCommand(other)),
                }
            }
        }

        Ok(())
    }

    fn write_data(&mut self, data: &[u8]) -> Result<()> {
        self.validate_bus_access()?;

        let pending = std::mem::replace(&mut self.pending_write, PendingWrite::None);
        match pending {
            PendingWrite::None => {
                if matches!(self.controller, ControllerRuntime::Ssd1306(_)) {
                    let transferred = self.process_ssd1306_ram_write(data)?;
                    if !self.has_pending_frame() {
                        self.schedule_visible_update(transferred)?;
                    }
                    Ok(())
                } else {
                    Err(LcdError::BusViolation("data write without an active command"))
                }
            }
            PendingWrite::Column(mut accumulator) => {
                let consumed = self.process_address_data(&mut accumulator, data, true)?;
                if consumed != data.len() {
                    return Err(LcdError::InvalidDataLength {
                        expected: 4 - accumulator.len,
                        got: data.len() - consumed,
                    });
                }

                if !accumulator.complete() {
                    self.pending_write = PendingWrite::Column(accumulator);
                }

                Ok(())
            }
            PendingWrite::Row(mut accumulator) => {
                let consumed = self.process_address_data(&mut accumulator, data, false)?;
                if consumed != data.len() {
                    return Err(LcdError::InvalidDataLength {
                        expected: 4 - accumulator.len,
                        got: data.len() - consumed,
                    });
                }

                if !accumulator.complete() {
                    self.pending_write = PendingWrite::Row(accumulator);
                }

                Ok(())
            }
            PendingWrite::Register(write) => self.process_register_write(write, data),
            PendingWrite::MemoryWrite(mut progress) => {
                self.process_memory_write(&mut progress, data)?;
                if progress.finished() {
                    self.schedule_visible_update(progress.transferred_bytes)?;
                } else {
                    self.pending_write = PendingWrite::MemoryWrite(progress);
                }
                Ok(())
            }
        }
    }

    fn read_data(&mut self, len: usize) -> Result<Vec<u8>> {
        self.validate_bus_access()?;
        self.build_read_response(len)
    }
}

impl VirtualLcd {
    fn ili9341_register_write_for_command(&self, cmd: u8) -> Option<RegisterWrite> {
        let allowed_lengths: &'static [usize] = match cmd {
            0x26 | 0x36 | 0x3A | 0x51 | 0x53 | 0x55 | 0x56 | 0xB0 | 0xB7 | 0xC0 | 0xC1
            | 0xC7 | 0xF2 | 0xF7 => &[1],
            0x37 | 0x44 | 0xB1 | 0xC5 | 0xEA => &[2],
            0xE8 | 0xF6 => &[3],
            0xB5 | 0xED => &[4],
            0xCB => &[5],
            0x33 => &[6],
            0xCF => &[3],
            0xB6 => &[3, 4],
            0xE0 | 0xE1 => &[15],
            _ => return None,
        };

        let register = match cmd {
            0x36 => RegisterKind::Madctl,
            0x3A => RegisterKind::Colmod,
            0x33 => RegisterKind::VerticalScrollDefinition,
            0x37 => RegisterKind::VerticalScrollStart,
            0x51 => RegisterKind::Brightness,
            0x53 => RegisterKind::ControlDisplay,
            0xF6 => RegisterKind::InterfaceControl,
            other => RegisterKind::Raw(other),
        };

        Some(RegisterWrite {
            register,
            allowed_lengths,
        })
    }

    fn build_read_response(&self, len: usize) -> Result<Vec<u8>> {
        let mut response = match (&self.controller, self.state.current_command) {
            (_, Some(0x04)) => vec![0x00, 0x00, 0x93, 0x41],
            (ControllerRuntime::Ili9341(controller), Some(0x09)) => {
                vec![0x00, 0x00, controller.power_mode(&self.state), controller.madctl, controller.colmod]
            }
            (ControllerRuntime::Ili9341(controller), Some(0x0A)) => {
                vec![0x00, controller.power_mode(&self.state)]
            }
            (ControllerRuntime::Ili9341(controller), Some(0x0B)) => vec![0x00, controller.madctl],
            (ControllerRuntime::Ili9341(controller), Some(0x0C)) => vec![0x00, controller.colmod],
            (ControllerRuntime::Ili9341(_), Some(0x0F)) => vec![0x00, 0xC0],
            (ControllerRuntime::Ili9341(_), Some(0x45)) => vec![0x00, 0x00, 0x00],
            (ControllerRuntime::Ili9341(controller), Some(0x52)) => vec![0x00, controller.brightness],
            (ControllerRuntime::Ili9341(controller), Some(0x54)) => {
                vec![0x00, controller.control_display]
            }
            (_, Some(0xDA)) => vec![0x00],
            (_, Some(0xDB)) => vec![0x93],
            (_, Some(0xDC)) => vec![0x41],
            (ControllerRuntime::Ili9341(controller), Some(0x2E)) => {
                self.build_ili9341_memory_read(controller, len)
            }
            _ => vec![0x00; len],
        };

        response.resize(len, 0x00);
        Ok(response)
    }

    fn build_ili9341_memory_read(&self, controller: &Ili9341State, len: usize) -> Vec<u8> {
        let window = self.state.current_window;
        let bytes_per_pixel = controller.interface_pixel_format().bytes_per_pixel();
        let mut out = Vec::with_capacity(len.max(1));
        out.push(0x00);

        for index in 0..window.area() {
            if out.len() >= len {
                break;
            }

            if let Ok((x, y)) = controller.write_pixel_coords(window, index, &self.config) {
                let color = self.back_buffer.get_pixel(x, y).unwrap_or(Color::BLACK);
                match controller.interface_pixel_format() {
                    PixelFormat::Rgb565 => {
                        let bytes = color.to_rgb565().to_be_bytes();
                        out.extend_from_slice(&bytes);
                    }
                    PixelFormat::Rgb888 => {
                        out.push(color.r & 0xFC);
                        out.push(color.g & 0xFC);
                        out.push(color.b & 0xFC);
                    }
                    format => {
                        let mut raw = [0u8; 3];
                        raw[..format.bytes_per_pixel()]
                            .copy_from_slice(&[color.r, color.g, color.b][..format.bytes_per_pixel()]);
                        out.extend_from_slice(&raw[..bytes_per_pixel]);
                    }
                }
            }
        }

        out.truncate(len);
        out
    }

    fn ensure_initialized_only(&self) -> Result<()> {
        if !self.state.initialized {
            return Err(LcdError::NotInitialized);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LcdError {
    InvalidConfig(&'static str),
    NotInitialized,
    DisplayOff,
    SleepMode,
    InvalidWindow,
    OutOfBounds,
    InvalidCommand(u8),
    InvalidDataLength { expected: usize, got: usize },
    BusViolation(&'static str),
    FrameRateExceeded,
}

impl Display for LcdError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid config: {message}"),
            Self::NotInitialized => f.write_str("display is not initialized"),
            Self::DisplayOff => f.write_str("display is off"),
            Self::SleepMode => f.write_str("display is in sleep mode"),
            Self::InvalidWindow => f.write_str("invalid address window"),
            Self::OutOfBounds => f.write_str("coordinates are out of bounds"),
            Self::InvalidCommand(cmd) => write!(f, "invalid command 0x{cmd:02X}"),
            Self::InvalidDataLength { expected, got } => {
                write!(f, "invalid data length: expected {expected} bytes, got {got}")
            }
            Self::BusViolation(message) => write!(f, "bus violation: {message}"),
            Self::FrameRateExceeded => f.write_str("frame submitted before the previous transfer completed"),
        }
    }
}

impl Error for LcdError {}

fn max_instant(left: Instant, right: Instant) -> Instant {
    if left >= right { left } else { right }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn fast_config() -> LcdConfig {
        LcdConfig {
            width: 4,
            height: 4,
            pixel_format: PixelFormat::Rgb565,
            fps: 1_000,
            interface: InterfaceType::Spi4Wire,
            orientation: 0,
            vsync: false,
            buffering: BufferingMode::Double,
            backlight: true,
            tearing_effect: false,
            bus_hz: 32_000_000,
            controller: ControllerModel::Ili9341,
        }
    }

    fn fast_ssd1306_config() -> LcdConfig {
        LcdConfig {
            width: 8,
            height: 8,
            pixel_format: PixelFormat::Mono1,
            fps: 1_000,
            interface: InterfaceType::Spi4Wire,
            orientation: 0,
            vsync: false,
            buffering: BufferingMode::Double,
            backlight: true,
            tearing_effect: false,
            bus_hz: 32_000_000,
            controller: ControllerModel::Ssd1306,
        }
    }

    fn wait_until_visible(lcd: &mut VirtualLcd) {
        for _ in 0..16 {
            if lcd.tick() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn bus_ready_ili9341() -> VirtualLcd {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");
        lcd.write_command(0x11).expect("sleep out should succeed");
        lcd.write_command(0x29).expect("display on should succeed");
        lcd
    }

    fn bus_ready_ssd1306() -> VirtualLcd {
        let mut lcd = VirtualLcd::new(fast_ssd1306_config()).expect("config should be valid");
        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");
        lcd.write_command(0xAE).expect("display off should succeed");
        write_command_with_data(&mut lcd, 0x20, &[0x02]);
        lcd.write_command(0xAF).expect("display on should succeed");
        lcd
    }

    fn write_command_with_data(lcd: &mut VirtualLcd, cmd: u8, data: &[u8]) {
        lcd.write_command(cmd).expect("command should succeed");
        lcd.write_data(data)
            .unwrap_or_else(|error| panic!("data for command 0x{cmd:02X} should succeed: {error:?}"));
    }

    #[test]
    fn high_level_draw_requires_present() {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.init().expect("init should succeed");
        lcd.draw_pixel(1, 2, Color::WHITE)
            .expect("pixel draw should succeed");

        assert_eq!(lcd.visible_frame().get_pixel(1, 2), Some(Color::BLACK));

        lcd.present().expect("present should schedule a frame");
        wait_until_visible(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(1, 2), Some(Color::WHITE));
    }

    #[test]
    fn low_level_memory_write_updates_window() {
        let mut lcd = bus_ready_ili9341();
        write_command_with_data(&mut lcd, 0x3A, &[0x55]);

        lcd.write_command(0x2A).expect("column command should succeed");
        lcd.write_data(&[0x00, 0x00, 0x00, 0x01])
            .expect("column data should succeed");
        lcd.write_command(0x2B).expect("row command should succeed");
        lcd.write_data(&[0x00, 0x00, 0x00, 0x00])
            .expect("row data should succeed");
        lcd.write_command(0x2C).expect("memory write should start");

        let red = Color::RED.to_rgb565().to_be_bytes();
        let green = Color::GREEN.to_rgb565().to_be_bytes();
        let mut pixels = Vec::new();
        pixels.extend_from_slice(&red);
        pixels.extend_from_slice(&green);
        lcd.write_data(&pixels).expect("pixel payload should succeed");

        wait_until_visible(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::RED));
        assert_eq!(lcd.visible_frame().get_pixel(1, 0), Some(Color::GREEN));
    }

    #[test]
    fn ili9341_common_init_sequence_is_accepted() {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");

        write_command_with_data(&mut lcd, 0xCB, &[0x39, 0x2C, 0x00, 0x34, 0x02]);
        write_command_with_data(&mut lcd, 0xCF, &[0x00, 0xC1, 0x30]);
        write_command_with_data(&mut lcd, 0xE8, &[0x85, 0x00, 0x78]);
        write_command_with_data(&mut lcd, 0xEA, &[0x00, 0x00]);
        write_command_with_data(&mut lcd, 0xED, &[0x64, 0x03, 0x12, 0x81]);
        write_command_with_data(&mut lcd, 0xF7, &[0x20]);
        write_command_with_data(&mut lcd, 0xC0, &[0x23]);
        write_command_with_data(&mut lcd, 0xC1, &[0x10]);
        write_command_with_data(&mut lcd, 0xC5, &[0x3E, 0x28]);
        write_command_with_data(&mut lcd, 0xC7, &[0x86]);
        write_command_with_data(&mut lcd, 0xB1, &[0x00, 0x18]);
        write_command_with_data(&mut lcd, 0xB6, &[0x08, 0x82, 0x27]);
        write_command_with_data(&mut lcd, 0xF2, &[0x00]);
        write_command_with_data(&mut lcd, 0x26, &[0x01]);
        write_command_with_data(
            &mut lcd,
            0xE0,
            &[0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09, 0x00],
        );
        write_command_with_data(
            &mut lcd,
            0xE1,
            &[0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36, 0x0F],
        );
        lcd.write_command(0x11).expect("sleep out should succeed");
        write_command_with_data(&mut lcd, 0x3A, &[0x55]);
        write_command_with_data(&mut lcd, 0x36, &[0x48]);
        lcd.write_command(0x29).expect("display on should succeed");
    }

    #[test]
    fn ili9341_read_commands_expose_id_and_pixel_format() {
        let mut lcd = bus_ready_ili9341();
        write_command_with_data(&mut lcd, 0x3A, &[0x55]);

        lcd.write_command(0x04).expect("read id command should succeed");
        assert_eq!(lcd.read_data(4).expect("id read should succeed"), vec![0x00, 0x00, 0x93, 0x41]);

        lcd.write_command(0x0C).expect("read colmod should succeed");
        assert_eq!(lcd.read_data(2).expect("colmod read should succeed"), vec![0x00, 0x55]);
    }

    #[test]
    fn ili9341_madctl_rotation_changes_visible_mapping() {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.init().expect("init should succeed");
        lcd.draw_pixel(1, 0, Color::RED).expect("pixel draw should succeed");
        lcd.present().expect("present should succeed");
        wait_until_visible(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(1, 0), Some(Color::RED));

        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");
        write_command_with_data(&mut lcd, 0x36, &[0x20]);

        assert_eq!(lcd.visible_frame().get_pixel(1, 0), Some(Color::BLACK));
        assert_eq!(lcd.visible_frame().get_pixel(0, 1), Some(Color::RED));
    }

    #[test]
    fn ili9341_vertical_scroll_repositions_visible_rows() {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.init().expect("init should succeed");

        lcd.fill_rect(0, 0, 4, 1, Color::RED).expect("row 0");
        lcd.fill_rect(0, 1, 4, 1, Color::GREEN).expect("row 1");
        lcd.fill_rect(0, 2, 4, 1, Color::BLUE).expect("row 2");
        lcd.fill_rect(0, 3, 4, 1, Color::WHITE).expect("row 3");
        lcd.present().expect("present should succeed");
        wait_until_visible(&mut lcd);

        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");
        write_command_with_data(&mut lcd, 0x33, &[0x00, 0x00, 0x00, 0x04, 0x00, 0x00]);
        write_command_with_data(&mut lcd, 0x37, &[0x00, 0x01]);

        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::GREEN));
        assert_eq!(lcd.visible_frame().get_pixel(0, 1), Some(Color::BLUE));
        assert_eq!(lcd.visible_frame().get_pixel(0, 2), Some(Color::WHITE));
        assert_eq!(lcd.visible_frame().get_pixel(0, 3), Some(Color::RED));
    }

    #[test]
    fn ssd1306_common_init_sequence_is_accepted() {
        let mut lcd = VirtualLcd::new(fast_ssd1306_config()).expect("config should be valid");
        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");

        lcd.write_command(0xAE).expect("display off should succeed");
        write_command_with_data(&mut lcd, 0xD5, &[0x80]);
        write_command_with_data(&mut lcd, 0xA8, &[0x3F]);
        write_command_with_data(&mut lcd, 0xD3, &[0x00]);
        lcd.write_command(0x40).expect("start line should succeed");
        write_command_with_data(&mut lcd, 0x8D, &[0x14]);
        write_command_with_data(&mut lcd, 0x20, &[0x00]);
        lcd.write_command(0xA1).expect("segment remap should succeed");
        lcd.write_command(0xC8).expect("com scan reverse should succeed");
        write_command_with_data(&mut lcd, 0xDA, &[0x12]);
        write_command_with_data(&mut lcd, 0x81, &[0xCF]);
        write_command_with_data(&mut lcd, 0xD9, &[0xF1]);
        write_command_with_data(&mut lcd, 0xDB, &[0x40]);
        lcd.write_command(0xA4).expect("display follow ram should succeed");
        lcd.write_command(0xA6).expect("normal display should succeed");
        lcd.write_command(0xAF).expect("display on should succeed");
    }

    #[test]
    fn ssd1306_page_writes_update_mono_pixels() {
        let mut lcd = bus_ready_ssd1306();

        lcd.write_command(0xB0).expect("page select should succeed");
        lcd.write_command(0x00).expect("lower column should succeed");
        lcd.write_command(0x10).expect("upper column should succeed");
        lcd.write_data(&[0b0000_0011, 0b0000_0100])
            .expect("gddram write should succeed");

        wait_until_visible(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(lcd.visible_frame().get_pixel(0, 1), Some(Color::WHITE));
        assert_eq!(lcd.visible_frame().get_pixel(1, 2), Some(Color::WHITE));
        assert_eq!(lcd.visible_frame().get_pixel(1, 1), Some(Color::BLACK));
    }

    #[test]
    fn ssd1306_high_level_drawing_quantizes_to_monochrome() {
        let mut lcd = VirtualLcd::new(fast_ssd1306_config()).expect("config should be valid");
        lcd.init().expect("init should succeed");

        lcd.draw_pixel(0, 0, Color::rgb(240, 240, 240))
            .expect("bright pixel should succeed");
        lcd.draw_pixel(1, 0, Color::rgb(20, 20, 20))
            .expect("dark pixel should succeed");
        lcd.present().expect("present should succeed");
        wait_until_visible(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(lcd.visible_frame().get_pixel(1, 0), Some(Color::BLACK));
    }

    #[test]
    fn ssd1306_display_start_line_and_remap_affect_visible_output() {
        let mut lcd = bus_ready_ssd1306();
        lcd.write_command(0xB0).expect("page select should succeed");
        lcd.write_command(0x00).expect("lower column should succeed");
        lcd.write_command(0x10).expect("upper column should succeed");
        lcd.write_data(&[0b0000_0001]).expect("gddram write should succeed");
        wait_until_visible(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::WHITE));

        lcd.write_command(0x41).expect("start line shift should succeed");
        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::BLACK));
        assert_eq!(lcd.visible_frame().get_pixel(0, 7), Some(Color::WHITE));

        lcd.write_command(0xA1).expect("segment remap should succeed");
        assert_eq!(lcd.visible_frame().get_pixel(7, 7), Some(Color::WHITE));
    }

    #[test]
    fn invalid_config_rejects_zero_dimensions() {
        let mut config = fast_config();
        config.width = 0;

        assert!(matches!(
            VirtualLcd::new(config),
            Err(LcdError::InvalidConfig("display dimensions must be non-zero"))
        ));
    }

    #[test]
    fn invalid_ssd1306_config_rejects_non_paged_height() {
        let mut config = fast_ssd1306_config();
        config.height = 7;

        assert!(matches!(
            VirtualLcd::new(config),
            Err(LcdError::InvalidConfig("ssd1306 height must be a multiple of 8"))
        ));
    }

    #[test]
    fn present_rejects_new_frame_while_previous_one_is_pending() {
        let mut config = fast_config();
        config.bus_hz = 1;

        let mut lcd = VirtualLcd::new(config).expect("config should be valid");
        lcd.init().expect("init should succeed");
        lcd.present().expect("first frame should be scheduled");

        assert!(lcd.has_pending_frame());
        assert_eq!(lcd.present(), Err(LcdError::FrameRateExceeded));
    }

    #[test]
    fn write_data_without_command_reports_bus_violation() {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");

        assert_eq!(
            lcd.write_data(&[0x12]),
            Err(LcdError::BusViolation("data write without an active command"))
        );
    }

    #[test]
    fn write_pixels_requires_window_sized_payload() {
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.init().expect("init should succeed");
        lcd.set_window(0, 0, 2, 2).expect("window should be valid");

        assert_eq!(
            lcd.write_pixels(&[Color::WHITE; 3]),
            Err(LcdError::InvalidDataLength {
                expected: 4,
                got: 3,
            })
        );
    }
}
