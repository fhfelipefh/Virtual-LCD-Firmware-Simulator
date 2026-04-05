use std::env;
use std::fs;
use std::thread;
use std::time::Duration;

use lcd_core::{BufferingMode, InterfaceType, LcdConfig, PixelFormat, VirtualLcd};
use lcd_examples::script::ScriptProgram;
use lcd_renderer::{SvgFrame, WindowRenderer};
use lcd_sdk::Lcd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "lcd-examples/scripts/panel.lcd".to_string());
    let source = fs::read_to_string(&path)?;
    let program = ScriptProgram::parse(&source)?;
    let (frame_path, screen_rect) = program.frame_asset();

    let config = LcdConfig {
        width: program.width,
        height: program.height,
        pixel_format: PixelFormat::Rgb565,
        fps: 30,
        interface: InterfaceType::Spi4Wire,
        orientation: 0,
        vsync: true,
        buffering: BufferingMode::Double,
        backlight: true,
        tearing_effect: false,
        bus_hz: 24_000_000,
    };

    let mut lcd = VirtualLcd::new(config)?;
    lcd.init()?;

    let frame = SvgFrame::load(frame_path, screen_rect)?;
    let mut renderer = WindowRenderer::new("LCD Script Runner", frame)?;

    while renderer.is_open() {
        program.execute(&mut lcd)?;
        lcd.present()?;

        while !lcd.tick() {
            if let Some(wait) = lcd.time_until_ready() {
                thread::sleep(wait.min(Duration::from_millis(4)));
            } else {
                break;
            }
        }

        renderer.update(lcd.visible_frame())?;
        thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
