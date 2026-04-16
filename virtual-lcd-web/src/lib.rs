#![forbid(unsafe_code)]

use wasm_bindgen::prelude::*;

use virtual_lcd_core::{
    BufferingMode, ControllerModel, InterfaceType, LcdConfig, PixelFormat, VirtualLcd,
};
use virtual_lcd_examples::draw::{draw_blip, draw_text, fill_rect_safe};
use virtual_lcd_examples::script::ScriptProgram;
use virtual_lcd_examples::scenes;
use virtual_lcd_sdk::{Color, Lcd};

const DEFAULT_FPS: u16 = 60;
const DEFAULT_BUS_HZ: u32 = 64_000_000;

#[derive(Clone, Copy)]
enum SceneKind {
    Dashboard,
    Oscilloscope,
    Startup,
    Gameboy,
}

impl SceneKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "dashboard" => Some(Self::Dashboard),
            "oscilloscope" => Some(Self::Oscilloscope),
            "startup" => Some(Self::Startup),
            "gameboy" => Some(Self::Gameboy),
            _ => None,
        }
    }

    fn as_name(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Oscilloscope => "oscilloscope",
            Self::Startup => "startup",
            Self::Gameboy => "gameboy",
        }
    }

    fn config(self) -> (u16, u16, ControllerModel) {
        match self {
            Self::Gameboy => (160, 144, ControllerModel::Ili9341),
            Self::Dashboard | Self::Oscilloscope | Self::Startup => {
                (320, 240, ControllerModel::Ili9341)
            }
        }
    }

    fn run(self, lcd: &mut VirtualLcd, tick: u32) -> virtual_lcd_core::Result<()> {
        match self {
            Self::Dashboard => scenes::dashboard(lcd, tick),
            Self::Oscilloscope => scenes::oscilloscope(lcd, tick),
            Self::Startup => scenes::startup(lcd, tick),
            Self::Gameboy => scenes::gameboy_boot(lcd, tick),
        }
    }
}

enum Mode {
    Scene(SceneKind),
    Script(ScriptProgram),
}

#[wasm_bindgen]
pub struct WebSimulator {
    lcd: VirtualLcd,
    mode: Mode,
    tick: u32,
    pointer_x: u16,
    pointer_y: u16,
    pointer_down: bool,
    buttons: u8,
}

#[wasm_bindgen]
impl WebSimulator {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WebSimulator, JsValue> {
        console_error_panic_hook::set_once();
        let mode = Mode::Scene(SceneKind::Dashboard);
        let lcd = Self::build_lcd_for_mode(&mode)?;
        Ok(Self {
            lcd,
            mode,
            tick: 0,
            pointer_x: 0,
            pointer_y: 0,
            pointer_down: false,
            buttons: 0,
        })
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        self.lcd = Self::build_lcd_for_mode(&self.mode)?;
        self.tick = 0;
        Ok(())
    }

    pub fn set_scene(&mut self, name: &str) -> Result<(), JsValue> {
        let scene = SceneKind::from_name(name)
            .ok_or_else(|| JsValue::from_str("Cena desconhecida. Use: dashboard|oscilloscope|startup|gameboy"))?;
        self.mode = Mode::Scene(scene);
        self.reset()
    }

    pub fn load_script(&mut self, source: &str) -> Result<(), JsValue> {
        let program = ScriptProgram::parse(source).map_err(to_js_error)?;
        self.mode = Mode::Script(program);
        self.reset()
    }

    pub fn step(&mut self) -> Result<(), JsValue> {
        match &self.mode {
            Mode::Scene(scene) => scene.run(&mut self.lcd, self.tick).map_err(to_js_error)?,
            Mode::Script(program) => {
                self.lcd.clear(Color::rgb(8, 12, 18)).map_err(to_js_error)?;
                program.execute(&mut self.lcd).map_err(to_js_error)?;
            }
        }

        self.apply_interaction_overlay().map_err(to_js_error)?;
        self.tick = self.tick.wrapping_add(1);
        Ok(())
    }

    pub fn frame_rgba(&self) -> Vec<u8> {
        let frame = self.lcd.working_frame();
        let mut out = Vec::with_capacity(frame.pixels().len() * 4);

        for pixel in frame.pixels().iter().copied() {
            out.push(pixel.r);
            out.push(pixel.g);
            out.push(pixel.b);
            out.push(255);
        }

        out
    }

    pub fn width(&self) -> u16 {
        self.lcd.working_frame().width()
    }

    pub fn height(&self) -> u16 {
        self.lcd.working_frame().height()
    }

    pub fn mode_name(&self) -> String {
        match &self.mode {
            Mode::Scene(scene) => scene.as_name().to_string(),
            Mode::Script(_) => "script".to_string(),
        }
    }

    pub fn controller_name(&self) -> String {
        match self.lcd.controller_model() {
            ControllerModel::GenericMipiDcs => "generic".to_string(),
            ControllerModel::Ili9341 => "ili9341".to_string(),
            ControllerModel::Ssd1306 => "ssd1306".to_string(),
        }
    }

    pub fn set_pointer(&mut self, x: u16, y: u16, down: bool) {
        self.pointer_x = x.min(self.width().saturating_sub(1));
        self.pointer_y = y.min(self.height().saturating_sub(1));
        self.pointer_down = down;
    }

    pub fn set_button(&mut self, name: &str, pressed: bool) {
        let mask = match name {
            "up" => 0b0000_0001,
            "down" => 0b0000_0010,
            "left" => 0b0000_0100,
            "right" => 0b0000_1000,
            "a" => 0b0001_0000,
            "b" => 0b0010_0000,
            "start" => 0b0100_0000,
            "select" => 0b1000_0000,
            _ => 0,
        };

        if mask == 0 {
            return;
        }

        if pressed {
            self.buttons |= mask;
        } else {
            self.buttons &= !mask;
        }
    }

    pub fn default_script(&self) -> String {
        include_str!("../../virtual-lcd-examples/scripts/panel.lcd").to_string()
    }

    fn build_lcd_for_mode(mode: &Mode) -> Result<VirtualLcd, JsValue> {
        let (width, height, controller) = match mode {
            Mode::Scene(scene) => scene.config(),
            Mode::Script(program) => (program.width, program.height, program.controller),
        };

        let pixel_format = match controller {
            ControllerModel::Ssd1306 => PixelFormat::Mono1,
            ControllerModel::GenericMipiDcs | ControllerModel::Ili9341 => PixelFormat::Rgb565,
        };

        let config = LcdConfig {
            width,
            height,
            pixel_format,
            fps: DEFAULT_FPS,
            interface: InterfaceType::Spi4Wire,
            orientation: 0,
            vsync: false,
            buffering: BufferingMode::Single,
            backlight: true,
            tearing_effect: false,
            bus_hz: DEFAULT_BUS_HZ,
            controller,
        };

        let mut lcd = VirtualLcd::new(config).map_err(to_js_error)?;
        lcd.init().map_err(to_js_error)?;
        Ok(lcd)
    }

    fn apply_interaction_overlay(&mut self) -> virtual_lcd_core::Result<()> {
        if self.pointer_down {
            let px = self.pointer_x as i32;
            let py = self.pointer_y as i32;
            draw_blip(&mut self.lcd, px, py, 3, Color::rgb(255, 220, 120))?;
            draw_blip(&mut self.lcd, px, py, 1, Color::rgb(255, 255, 255))?;
        }

        if self.buttons != 0 {
            let h = self.height();
            let w = self.width();
            fill_rect_safe(&mut self.lcd, 0, h.saturating_sub(16), w, 16, Color::rgb(8, 18, 28))?;

            let labels = [
                ("U", 0b0000_0001),
                ("D", 0b0000_0010),
                ("L", 0b0000_0100),
                ("R", 0b0000_1000),
                ("A", 0b0001_0000),
                ("B", 0b0010_0000),
                ("ST", 0b0100_0000),
                ("SE", 0b1000_0000),
            ];

            let mut x = 4u16;
            for (label, mask) in labels {
                let on = self.buttons & mask != 0;
                let fg = if on {
                    Color::rgb(110, 250, 220)
                } else {
                    Color::rgb(56, 80, 92)
                };
                draw_text(&mut self.lcd, x, h.saturating_sub(12), 1, fg, label)?;
                x = x.saturating_add(20);
            }
        }

        Ok(())
    }
}

fn to_js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
