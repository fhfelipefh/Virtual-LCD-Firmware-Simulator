use super::*;

#[derive(Clone, Debug)]
pub struct LcdState {
    pub initialized: bool,
    pub sleeping: bool,
    pub display_on: bool,
    pub backlight: u8,
    pub current_window: DrawWindow,
    pub current_command: Option<u8>,
    pub(crate) column_range: (u16, u16),
    pub(crate) row_range: (u16, u16),
    pub(crate) drawing: bool,
    pub(crate) touch: crate::touch::TouchState,
}

impl LcdState {
    pub(crate) fn new(config: &LcdConfig) -> Self {
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
            drawing: false,
            touch: crate::touch::TouchState::default(),
        }
    }
    pub(crate) fn set_column_range(&mut self, start: u16, end: u16) {
        self.column_range = (start, end);
        self.sync_window();
    }
    pub(crate) fn set_row_range(&mut self, start: u16, end: u16) {
        self.row_range = (start, end);
        self.sync_window();
    }
    pub(crate) fn sync_window(&mut self) {
        self.current_window = DrawWindow {
            x: self.column_range.0,
            y: self.row_range.0,
            width: self.column_range.1 - self.column_range.0 + 1,
            height: self.row_range.1 - self.row_range.0 + 1,
        };
    }
}
