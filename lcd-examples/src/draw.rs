use lcd_core::{Result as LcdResult, VirtualLcd};
use lcd_sdk::{Color, Lcd};

use crate::font;
use crate::{LCD_HEIGHT, LCD_WIDTH};

pub fn fill_vertical_gradient(
    lcd: &mut VirtualLcd,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    top: Color,
    bottom: Color,
) -> LcdResult<()> {
    for row in 0..height {
        let t = row as f32 / (height.max(1) - 1) as f32;
        let color = mix_color(top, bottom, t);
        lcd.fill_rect(x, y + row, width, 1, color)?;
    }
    Ok(())
}

pub fn draw_scanlines(lcd: &mut VirtualLcd, spacing: u16, color: Color) -> LcdResult<()> {
    let mut y = 0u16;
    while y < LCD_HEIGHT {
        lcd.fill_rect(0, y, LCD_WIDTH, 1, color)?;
        y = y.saturating_add(spacing);
    }
    Ok(())
}

pub fn draw_particles(lcd: &mut VirtualLcd, frame: u32, count: u32) -> LcdResult<()> {
    for index in 0..count {
        let seed = hash(index.wrapping_mul(0x9E37_79B9));
        let speed = 1 + (seed & 0x3);
        let x = ((seed >> 8).wrapping_add(frame.wrapping_mul(speed)) % LCD_WIDTH as u32) as u16;
        let y = (((seed >> 18).wrapping_mul(3)).wrapping_add(frame.wrapping_mul(speed / 2 + 1))
            % (LCD_HEIGHT as u32 - 56))
            as u16
            + 28;
        let color = if index % 5 == 0 {
            Color::rgb(255, 186, 104)
        } else {
            Color::rgb(56, 120, 140)
        };
        lcd.draw_pixel(x, y, color)?;
        if index % 4 == 0 && x + 1 < LCD_WIDTH {
            lcd.draw_pixel(x + 1, y, Color::rgb(18, 54, 70))?;
        }
    }
    Ok(())
}

pub fn draw_rect_outline(
    lcd: &mut VirtualLcd,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    color: Color,
) -> LcdResult<()> {
    lcd.fill_rect(x, y, width, 1, color)?;
    lcd.fill_rect(x, y + height - 1, width, 1, color)?;
    lcd.fill_rect(x, y, 1, height, color)?;
    lcd.fill_rect(x + width - 1, y, 1, height, color)?;
    Ok(())
}

pub fn fill_rect_safe(
    lcd: &mut VirtualLcd,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    color: Color,
) -> LcdResult<()> {
    if width == 0 || height == 0 {
        return Ok(());
    }
    lcd.fill_rect(x, y, width, height, color)
}

pub fn draw_line(
    lcd: &mut VirtualLcd,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
) -> LcdResult<()> {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        draw_pixel_checked(lcd, x0, y0, color)?;
        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }

    Ok(())
}

pub fn draw_circle(lcd: &mut VirtualLcd, cx: i32, cy: i32, radius: i32, color: Color) -> LcdResult<()> {
    let mut x = radius;
    let mut y = 0;
    let mut err = 1 - x;

    while x >= y {
        for (px, py) in [
            (cx + x, cy + y),
            (cx + y, cy + x),
            (cx - y, cy + x),
            (cx - x, cy + y),
            (cx - x, cy - y),
            (cx - y, cy - x),
            (cx + y, cy - x),
            (cx + x, cy - y),
        ] {
            draw_pixel_checked(lcd, px, py, color)?;
        }

        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x + 1);
        }
    }

    Ok(())
}

pub fn draw_blip(lcd: &mut VirtualLcd, cx: i32, cy: i32, radius: i32, color: Color) -> LcdResult<()> {
    for y in -radius..=radius {
        for x in -radius..=radius {
            if x * x + y * y <= radius * radius {
                draw_pixel_checked(lcd, cx + x, cy + y, color)?;
            }
        }
    }
    Ok(())
}

pub fn draw_panel(
    lcd: &mut VirtualLcd,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    frame_color: Color,
    body_color: Color,
    accent: Color,
) -> LcdResult<()> {
    lcd.fill_rect(x, y, width, height, frame_color)?;
    lcd.fill_rect(x + 2, y + 2, width - 4, height - 4, Color::rgb(3, 8, 12))?;
    lcd.fill_rect(x + 4, y + 4, width - 8, height - 8, body_color)?;
    draw_rect_outline(lcd, x + 1, y + 1, width - 2, height - 2, accent)?;
    Ok(())
}

pub fn draw_text(
    lcd: &mut VirtualLcd,
    x: u16,
    y: u16,
    scale: u16,
    color: Color,
    text: &str,
) -> LcdResult<()> {
    let scale = scale.max(1);
    let mut cursor_x = x;
    for ch in text.chars() {
        let glyph = font::glyph(ch);
        for (row, bits) in glyph.iter().copied().enumerate() {
            for col in 0..font::WIDTH {
                if bits & (1 << (font::WIDTH - 1 - col)) != 0 {
                    fill_rect_safe(
                        lcd,
                        cursor_x + col * scale,
                        y + row as u16 * scale,
                        scale,
                        scale,
                        color,
                    )?;
                }
            }
        }
        cursor_x += (font::WIDTH + 1) * scale;
    }
    Ok(())
}

pub fn measure_text(text: &str, scale: u16) -> (u16, u16) {
    let scale = scale.max(1);
    let chars = text.chars().count() as u16;
    let width = if chars == 0 {
        0
    } else {
        chars * (font::WIDTH + 1) * scale - scale
    };
    let height = font::HEIGHT * scale;
    (width, height)
}

pub fn draw_pixel_checked(lcd: &mut VirtualLcd, x: i32, y: i32, color: Color) -> LcdResult<()> {
    if x >= 0 && y >= 0 && x < LCD_WIDTH as i32 && y < LCD_HEIGHT as i32 {
        lcd.draw_pixel(x as u16, y as u16, color)?;
    }
    Ok(())
}

pub fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        (a.r as f32 + (b.r as f32 - a.r as f32) * t) as u8,
        (a.g as f32 + (b.g as f32 - a.g as f32) * t) as u8,
        (a.b as f32 + (b.b as f32 - a.b as f32) * t) as u8,
    )
}

pub fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
