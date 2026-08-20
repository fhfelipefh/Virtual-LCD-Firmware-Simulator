use std::fs;
use virtual_lcd_core::{
    BufferingMode, Color, ControllerModel, InterfaceType, LcdConfig, PixelFormat, VirtualLcd,
};
use virtual_lcd_sdk::{LcdBus, PinId};

fn save_snapshot(lcd: &VirtualLcd, name: &str) {
    let fb = lcd.working_frame(); // back_buffer: has pixels written immediately
    let width = config_width(lcd.config().controller) as u32;
    let height = config_height(lcd.config().controller) as u32;

    let mut img = image::RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let color = fb.get_pixel(x as u16, y as u16).unwrap_or(Color::BLACK);
            img.put_pixel(x, y, image::Rgb([color.r, color.g, color.b]));
        }
    }

    let path = format!("tests/snapshots/{}.png", name);
    fs::create_dir_all("tests/snapshots").unwrap();
    img.save(path).unwrap();
}

fn config_width(controller: ControllerModel) -> u16 {
    match controller {
        ControllerModel::Ili9341 => 240,
        ControllerModel::Ssd1306 => 128,
        ControllerModel::St7789 => 240,
        ControllerModel::GenericMipiDcs => 240,
    }
}

fn config_height(controller: ControllerModel) -> u16 {
    match controller {
        ControllerModel::Ili9341 => 320,
        ControllerModel::Ssd1306 => 64,
        ControllerModel::St7789 => 240,
        ControllerModel::GenericMipiDcs => 320,
    }
}

fn create_lcd(controller: ControllerModel) -> VirtualLcd {
    VirtualLcd::new(LcdConfig {
        controller,
        width: config_width(controller),
        height: config_height(controller),
        pixel_format: PixelFormat::Rgb565,
        fps: 60,
        interface: InterfaceType::Spi4Wire,
        bus_hz: 10_000_000,
        orientation: 0,
        vsync: false,
        buffering: BufferingMode::Single,
        backlight: true,
        tearing_effect: false,
    })
    .unwrap()
}

#[test]
fn test_ili9341_snapshot() {
    let mut lcd = create_lcd(ControllerModel::Ili9341);

    lcd.set_pin(PinId::Cs, false).unwrap();
    lcd.set_pin(PinId::Rst, true).unwrap();

    let init_cmds = [
        (0x01, vec![]),     // SWRESET
        (0x11, vec![]),     // SLPOUT
        (0x29, vec![]),     // DISPON
        (0x36, vec![0x48]), // MADCTL
        (0x3A, vec![0x55]), // COLMOD
    ];

    for (cmd, data) in init_cmds.iter() {
        lcd.write_command(*cmd).unwrap();
        if !data.is_empty() {
            lcd.write_data(data).unwrap();
        }
    }

    // Draw a red rectangle
    lcd.write_command(0x2A).unwrap();
    lcd.write_data(&[
        (10 >> 8) as u8,
        (10 & 0xFF) as u8,
        (50 >> 8) as u8,
        (50 & 0xFF) as u8,
    ])
    .unwrap();
    lcd.write_command(0x2B).unwrap();
    lcd.write_data(&[
        (10 >> 8) as u8,
        (10 & 0xFF) as u8,
        (50 >> 8) as u8,
        (50 & 0xFF) as u8,
    ])
    .unwrap();

    let color: u16 = 0xF800; // Red
    let mut pixels = Vec::new();
    for _ in 0..((41 * 41) as usize) {
        pixels.push((color >> 8) as u8);
        pixels.push((color & 0xFF) as u8);
    }
    lcd.write_command(0x2C).unwrap();
    lcd.write_data(&pixels).unwrap();

    lcd.set_pin(PinId::Cs, true).unwrap();

    lcd.tick();
    lcd.tick();

    save_snapshot(&lcd, "ili9341_test_rect");
}

#[test]
fn test_st7789_snapshot() {
    let mut lcd = create_lcd(ControllerModel::St7789);

    lcd.set_pin(PinId::Cs, false).unwrap();
    lcd.set_pin(PinId::Rst, true).unwrap();

    let init_cmds = [
        (0x01, vec![]),     // SWRESET
        (0x11, vec![]),     // SLPOUT
        (0x29, vec![]),     // DISPON
        (0x36, vec![0x00]), // MADCTL
        (0x3A, vec![0x55]), // COLMOD
    ];

    for (cmd, data) in init_cmds.iter() {
        lcd.write_command(*cmd).unwrap();
        if !data.is_empty() {
            lcd.write_data(data).unwrap();
        }
    }

    // Draw a green rectangle
    lcd.write_command(0x2A).unwrap();
    lcd.write_data(&[
        (20 >> 8) as u8,
        (20 & 0xFF) as u8,
        (60 >> 8) as u8,
        (60 & 0xFF) as u8,
    ])
    .unwrap();
    lcd.write_command(0x2B).unwrap();
    lcd.write_data(&[
        (20 >> 8) as u8,
        (20 & 0xFF) as u8,
        (60 >> 8) as u8,
        (60 & 0xFF) as u8,
    ])
    .unwrap();

    let color: u16 = 0x07E0; // Green
    let mut pixels = Vec::new();
    for _ in 0..((41 * 41) as usize) {
        pixels.push((color >> 8) as u8);
        pixels.push((color & 0xFF) as u8);
    }
    lcd.write_command(0x2C).unwrap();
    lcd.write_data(&pixels).unwrap();

    lcd.set_pin(PinId::Cs, true).unwrap();

    lcd.tick();
    lcd.tick();

    save_snapshot(&lcd, "st7789_test_rect");
}
