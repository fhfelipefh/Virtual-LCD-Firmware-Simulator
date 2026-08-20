use super::*;

pub mod ili9341;
pub mod ssd1306;
pub mod st7789;
pub(crate) use ili9341::*;
pub(crate) use ssd1306::*;
pub(crate) use st7789::*;

#[derive(Debug)]
pub(crate) enum ControllerRuntime {
    Generic,
    Ili9341(Ili9341State),
    Ssd1306(Ssd1306State),
    St7789(St7789State),
}

impl ControllerRuntime {
    pub(crate) fn new(model: ControllerModel, config: &LcdConfig) -> Self {
        match model {
            ControllerModel::GenericMipiDcs => Self::Generic,
            ControllerModel::Ili9341 => Self::Ili9341(Ili9341State::new(config)),
            ControllerModel::Ssd1306 => Self::Ssd1306(Ssd1306State::new(config)),
            ControllerModel::St7789 => Self::St7789(St7789State::new(config)),
        }
    }
    pub(crate) fn reset(&mut self, config: &LcdConfig) {
        match self {
            Self::Generic => {}
            Self::Ili9341(state) => *state = Ili9341State::new(config),
            Self::Ssd1306(state) => *state = Ssd1306State::new(config),
            Self::St7789(state) => *state = St7789State::new(config),
        }
    }
    pub(crate) fn visible_bytes_per_pixel(&self, fallback: PixelFormat) -> usize {
        match self {
            Self::Generic => fallback.bytes_per_pixel(),
            Self::Ili9341(state) => state.interface_pixel_format().bytes_per_pixel(),
            Self::Ssd1306(_) => PixelFormat::Mono1.bytes_per_pixel(),
            Self::St7789(state) => state.interface_pixel_format().bytes_per_pixel(),
        }
    }
    pub(crate) fn native_frame_bytes(&self, config: &LcdConfig) -> usize {
        match self {
            Self::Generic | Self::Ili9341(_) | Self::St7789(_) => config.full_frame_bytes(),
            Self::Ssd1306(state) => state.gddram.len(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct VerticalScrollState {
    pub(crate) top_fixed_area: u16,
    pub(crate) scroll_area: u16,
    pub(crate) bottom_fixed_area: u16,
    pub(crate) start_address: u16,
}

impl VerticalScrollState {
    pub(crate) fn new(height: u16) -> Self {
        Self {
            top_fixed_area: 0,
            scroll_area: height,
            bottom_fixed_area: 0,
            start_address: 0,
        }
    }
    pub(crate) fn map_visible_row(&self, row: u16, total_height: u16) -> u16 {
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
