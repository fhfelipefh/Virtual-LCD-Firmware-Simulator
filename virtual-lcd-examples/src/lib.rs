use std::thread;
use std::time::Duration;

use virtual_lcd_core::{
    BufferingMode, ControllerModel, InterfaceType, LcdConfig, PixelFormat, Result as LcdResult,
    VirtualLcd,
};
use virtual_lcd_renderer::{ScreenRect, SvgFrame, WindowRenderer};
use virtual_lcd_sdk::Lcd;

pub mod draw;
pub mod font;
pub mod script;
pub mod scenes;

pub const LCD_WIDTH: u16 = 320;
pub const LCD_HEIGHT: u16 = 240;

pub type Scene = fn(&mut VirtualLcd, u32) -> LcdResult<()>;

pub fn run_scene(title: &str, scene: Scene) -> Result<(), Box<dyn std::error::Error>> {
    let (frame_path, screen_rect) = frame_asset_for(LCD_WIDTH as usize, LCD_HEIGHT as usize);
    run_scene_with(
        RuntimeOptions {
            title,
            width: LCD_WIDTH,
            height: LCD_HEIGHT,
            fps: 30,
            frame_path,
            screen_rect,
            controller: ControllerModel::Ili9341,
        },
        scene,
    )
}

pub struct RuntimeOptions<'a> {
    pub title: &'a str,
    pub width: u16,
    pub height: u16,
    pub fps: u16,
    pub frame_path: &'a str,
    pub screen_rect: ScreenRect,
    pub controller: ControllerModel,
}

pub fn run_scene_with(
    options: RuntimeOptions<'_>,
    scene: Scene,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = LcdConfig {
        width: options.width,
        height: options.height,
        pixel_format: PixelFormat::Rgb565,
        fps: options.fps,
        interface: InterfaceType::Spi4Wire,
        orientation: 0,
        vsync: true,
        buffering: BufferingMode::Double,
        backlight: true,
        tearing_effect: false,
        bus_hz: 24_000_000,
        controller: options.controller,
    };

    let mut lcd = VirtualLcd::new(config)?;
    lcd.init()?;

    let frame = SvgFrame::load(options.frame_path, options.screen_rect)?;
    let mut renderer = WindowRenderer::new(options.title, frame)?;

    let mut tick = 0u32;
    while renderer.is_open() {
        scene(&mut lcd, tick)?;
        lcd.present()?;

        while !lcd.tick() {
            if let Some(wait) = lcd.time_until_ready() {
                thread::sleep(wait.min(Duration::from_millis(4)));
            } else {
                break;
            }
        }

        renderer.update(lcd.visible_frame())?;
        tick = tick.wrapping_add(1);
        thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

pub fn frame_asset_for(width: usize, height: usize) -> (&'static str, ScreenRect) {
    match (width, height) {
        (w, h) if w * 3 == h * 4 => ("frames/lcd_frame_4_3.svg", ScreenRect::new(80, 80, 960, 660)),
        (w, h) if w * 9 == h * 16 => ("frames/lcd_frame_16_9.svg", ScreenRect::new(80, 80, 1360, 660)),
        (w, h) if w * 9 == h * 21 => ("frames/lcd_frame_21_9.svg", ScreenRect::new(80, 80, 1860, 660)),
        (w, h) if w == h => ("frames/lcd_frame_1_1.svg", ScreenRect::new(80, 80, 760, 760)),
        (w, h) if w * 16 == h * 9 => ("frames/lcd_frame_9_16.svg", ScreenRect::new(80, 80, 660, 1360)),
        _ => ("frames/lcd_frame_4_3.svg", ScreenRect::new(80, 80, 960, 660)),
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use virtual_lcd_core::{
        BufferingMode, ControllerModel, InterfaceType, LcdConfig, PixelFormat, VirtualLcd,
    };
    use virtual_lcd_renderer::ScreenRect;
    use virtual_lcd_sdk::{Color, Lcd};

    use super::{frame_asset_for, scenes, script};

    fn fast_config(width: u16, height: u16) -> LcdConfig {
        LcdConfig {
            width,
            height,
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

    fn present_and_wait(lcd: &mut VirtualLcd) {
        lcd.present().expect("present should succeed");
        for _ in 0..16 {
            if lcd.tick() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("frame was not made visible in time");
    }

    #[test]
    fn frame_asset_selection_matches_supported_ratios() {
        assert_eq!(
            frame_asset_for(320, 240),
            ("frames/lcd_frame_4_3.svg", ScreenRect::new(80, 80, 960, 660))
        );
        assert_eq!(
            frame_asset_for(240, 240),
            ("frames/lcd_frame_1_1.svg", ScreenRect::new(80, 80, 760, 760))
        );
        assert_eq!(
            frame_asset_for(160, 90),
            ("frames/lcd_frame_16_9.svg", ScreenRect::new(80, 80, 1360, 660))
        );
    }

    #[test]
    fn script_program_parses_and_executes_draw_commands() {
        let program = script::ScriptProgram::parse(
            "\
controller ili9341
canvas 16 12
frame handheld
gradient 0 0 16 12 0 0 0 70 80 90
fill_rect 2 3 2 2 10 20 30
rect 0 0 16 12 50 60 70
line 0 11 15 11 90 100 110
circle 6 1 1 120 130 140
text 1 1 1 200 210 220 HI
",
        )
        .expect("script should parse");

        assert_eq!(program.width, 16);
        assert_eq!(program.height, 12);
        assert_eq!(program.controller, ControllerModel::Ili9341);
        assert_eq!(
            program.frame_asset(),
            ("frames/handheld_classic.svg", ScreenRect::new(32, 34, 496, 432))
        );

        let mut lcd = VirtualLcd::new(fast_config(16, 12)).expect("config should be valid");
        lcd.init().expect("init should succeed");
        program.execute(&mut lcd).expect("script should execute");
        present_and_wait(&mut lcd);

        assert_eq!(lcd.visible_frame().get_pixel(0, 0), Some(Color::rgb(50, 60, 70)));
        assert_eq!(lcd.visible_frame().get_pixel(2, 3), Some(Color::rgb(10, 20, 30)));
        assert_eq!(lcd.visible_frame().get_pixel(0, 11), Some(Color::rgb(90, 100, 110)));
        assert_eq!(lcd.visible_frame().get_pixel(6, 0), Some(Color::rgb(120, 130, 140)));
    }

    #[test]
    fn script_program_rejects_unknown_frame_preset() {
        assert!(matches!(
            script::ScriptProgram::parse("frame arcade"),
            Err(script::ScriptError::InvalidFramePreset(value)) if value == "arcade"
        ));
    }

    #[test]
    fn script_program_rejects_unknown_controller() {
        assert!(matches!(
            script::ScriptProgram::parse("controller hx8357"),
            Err(script::ScriptError::InvalidController(value)) if value == "hx8357"
        ));
    }

    #[test]
    fn gameboy_scene_draws_logo_using_dark_monochrome_pixels() {
        let mut lcd = VirtualLcd::new(fast_config(160, 144)).expect("config should be valid");
        lcd.init().expect("init should succeed");

        scenes::gameboy_boot(&mut lcd, 120).expect("scene should render");
        present_and_wait(&mut lcd);

        let dark_logo = Color::rgb(48, 56, 32);
        let dark_pixels = lcd
            .visible_frame()
            .pixels()
            .iter()
            .filter(|&&pixel| pixel == dark_logo)
            .count();

        assert!(dark_pixels > 400, "expected visible logo pixels, got {dark_pixels}");
    }
}
