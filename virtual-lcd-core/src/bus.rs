use super::*;
use instant::Instant;

#[derive(Debug)]
pub(crate) struct RegisterWrite {
    pub(crate) register: RegisterKind,
    pub(crate) allowed_lengths: &'static [usize],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RegisterKind {
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
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) pixels: Vec<Color>,
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
    pub(crate) fn index_of(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(y as usize * self.width as usize + x as usize)
    }
}

#[derive(Clone, Debug)]
pub struct PinBank {
    pub(crate) levels: [bool; 9],
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
    pub(crate) fn set(&mut self, pin: PinId, value: bool) {
        self.levels[pin.index()] = value;
    }
}

#[derive(Debug)]
pub(crate) struct TimingEngine {
    pub(crate) frame_interval: Duration,
    pub(crate) bus_hz: u32,
    pub(crate) last_visible_at: Instant,
    pub(crate) pending_ready_at: Option<Instant>,
}

impl TimingEngine {
    pub(crate) fn new(config: &LcdConfig) -> Self {
        let frame_interval = config.frame_interval();
        Self {
            frame_interval,
            bus_hz: config.bus_hz,
            last_visible_at: Instant::now() - frame_interval,
            pending_ready_at: None,
        }
    }
    pub(crate) fn schedule_transfer(&mut self, bytes: usize, vsync: bool) -> Result<Instant> {
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
    pub(crate) fn tick(&mut self) -> bool {
        match self.pending_ready_at {
            Some(ready_at) if Instant::now() >= ready_at => {
                self.last_visible_at = ready_at;
                self.pending_ready_at = None;
                true
            }
            _ => false,
        }
    }
    pub(crate) fn time_until_ready(&self) -> Option<Duration> {
        self.pending_ready_at.map(|ready_at| ready_at.saturating_duration_since(Instant::now()))
    }
    pub(crate) fn clear_pending(&mut self) {
        self.pending_ready_at = None;
    }
}

#[derive(Debug)]
pub(crate) enum PendingWrite {
    None,
    Column(AddressAccumulator),
    Row(AddressAccumulator),
    Register(RegisterWrite),
    MemoryWrite(MemoryWriteProgress),
}

#[derive(Debug)]
pub(crate) struct AddressAccumulator {
    pub(crate) bytes: [u8; 4],
    pub(crate) len: usize,
}

impl AddressAccumulator {
    pub(crate) fn new() -> Self {
        Self {
            bytes: [0; 4],
            len: 0,
        }
    }
    pub(crate) fn push(&mut self, data: &[u8]) -> usize {
        let available = 4 - self.len;
        let take = available.min(data.len());
        self.bytes[self.len..self.len + take].copy_from_slice(&data[..take]);
        self.len += take;
        take
    }
    pub(crate) fn complete(&self) -> bool {
        self.len == 4
    }
    pub(crate) fn decode(&self) -> (u16, u16) {
        let start = u16::from_be_bytes([self.bytes[0], self.bytes[1]]);
        let end = u16::from_be_bytes([self.bytes[2], self.bytes[3]]);
        (start, end)
    }
}

#[derive(Debug)]
pub(crate) struct MemoryWriteProgress {
    pub(crate) window: DrawWindow,
    pub(crate) next_pixel: usize,
    pub(crate) partial_pixel: [u8; 3],
    pub(crate) partial_len: usize,
    pub(crate) transferred_bytes: usize,
}

impl MemoryWriteProgress {
    pub(crate) fn new(window: DrawWindow) -> Self {
        Self {
            window,
            next_pixel: 0,
            partial_pixel: [0; 3],
            partial_len: 0,
            transferred_bytes: 0,
        }
    }
    pub(crate) fn total_pixels(&self) -> usize {
        self.window.area()
    }
    pub(crate) fn remaining_bytes(&self, bytes_per_pixel: usize) -> usize {
        (self.total_pixels() - self.next_pixel) * bytes_per_pixel - self.partial_len
    }
    pub(crate) fn finished(&self) -> bool {
        self.next_pixel == self.total_pixels() && self.partial_len == 0
    }
    pub(crate) fn current_coords(&self) -> (u16, u16) {
        let dx = (self.next_pixel % self.window.width as usize) as u16;
        let dy = (self.next_pixel / self.window.width as usize) as u16;
        (self.window.x + dx, self.window.y + dy)
    }
}
    pub(crate) fn max_instant(left: Instant, right: Instant) -> Instant {
    if left >= right { left } else { right }
}

