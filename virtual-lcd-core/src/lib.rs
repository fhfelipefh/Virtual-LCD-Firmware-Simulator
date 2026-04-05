#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::{Duration, Instant};

pub use virtual_lcd_sdk::{Color, Lcd, LcdBus, PinId};

pub type Result<T> = std::result::Result<T, LcdError>;

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
        let timing = TimingEngine::new(&config);

        Ok(Self {
            config,
            state,
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
            self.back_buffer
                .set_pixel(window.x + dx, window.y + dy, color)?;
        }

        self.schedule_visible_update(expected * self.config.pixel_format.bytes_per_pixel())
    }

    pub fn tick(&mut self) -> bool {
        if self.timing.tick() {
            self.front_buffer.copy_from(&self.back_buffer);
            if self.config.buffering == BufferingMode::Single {
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
        self.ensure_ready_for_graphics()?;

        let bytes_per_pixel = self.config.pixel_format.bytes_per_pixel();
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
                let color = self
                    .config
                    .pixel_format
                    .decode_color(&progress.partial_pixel[..bytes_per_pixel]);
                let (x, y) = progress.current_coords();
                self.back_buffer.set_pixel(x, y, color)?;
                progress.partial_len = 0;
                progress.next_pixel += 1;
            }
        }

        Ok(data.len())
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
        Ok(())
    }

    fn clear(&mut self, color: Color) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        self.back_buffer.clear(color);
        Ok(())
    }

    fn draw_pixel(&mut self, x: u16, y: u16, color: Color) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        self.back_buffer.set_pixel(x, y, color)
    }

    fn fill_rect(&mut self, x: u16, y: u16, width: u16, height: u16, color: Color) -> Result<()> {
        self.ensure_ready_for_graphics()?;
        let window = DrawWindow::from_origin(x, y, width, height, &self.config)?;
        self.back_buffer.fill_rect(window, color)
    }

    fn present(&mut self) -> Result<()> {
        self.ensure_ready_for_graphics()?;

        if !matches!(self.pending_write, PendingWrite::None) {
            return Err(LcdError::BusViolation("cannot present while a bus transaction is active"));
        }

        self.schedule_visible_update(self.config.full_frame_bytes())
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

        match cmd {
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
                self.ensure_ready_for_graphics()?;
                self.pending_write = PendingWrite::MemoryWrite(MemoryWriteProgress::new(self.state.current_window));
            }
            _ => return Err(LcdError::InvalidCommand(cmd)),
        }

        Ok(())
    }

    fn write_data(&mut self, data: &[u8]) -> Result<()> {
        self.validate_bus_access()?;

        let pending = std::mem::replace(&mut self.pending_write, PendingWrite::None);
        match pending {
            PendingWrite::None => Err(LcdError::BusViolation("data write without an active command")),
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

        let response = match self.state.current_command {
            Some(0x04) => {
                let mut id = vec![0x00, 0x93, 0x41];
                id.resize(len, 0x00);
                id
            }
            _ => vec![0x00; len],
        };

        Ok(response)
    }
}

impl VirtualLcd {
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
        let mut lcd = VirtualLcd::new(fast_config()).expect("config should be valid");
        lcd.set_pin(PinId::Cs, false).expect("CS should be writable");
        lcd.write_command(0x11).expect("sleep out should succeed");
        lcd.write_command(0x29).expect("display on should succeed");

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
    fn invalid_config_rejects_zero_dimensions() {
        let mut config = fast_config();
        config.width = 0;

        assert!(matches!(
            VirtualLcd::new(config),
            Err(LcdError::InvalidConfig("display dimensions must be non-zero"))
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
