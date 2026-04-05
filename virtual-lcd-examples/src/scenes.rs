use virtual_lcd_core::{Result as LcdResult, VirtualLcd};
use virtual_lcd_sdk::{Color, Lcd};

use crate::draw::{
    draw_blip, draw_circle, draw_line, draw_panel, draw_particles, draw_rect_outline,
    draw_scanlines, draw_text, fill_rect_safe, fill_vertical_gradient, measure_text, mix_color,
};
use crate::{LCD_HEIGHT, LCD_WIDTH};

pub fn dashboard(lcd: &mut VirtualLcd, frame: u32) -> LcdResult<()> {
    fill_vertical_gradient(
        lcd,
        0,
        0,
        LCD_WIDTH,
        LCD_HEIGHT,
        Color::rgb(4, 14, 20),
        Color::rgb(3, 32, 40),
    )?;
    draw_scanlines(lcd, 3, Color::rgb(2, 10, 14))?;
    draw_particles(lcd, frame, 28)?;
    draw_status_bar(lcd, frame)?;
    draw_level_bank(lcd, frame, 14, 28, 68, 142)?;
    draw_radar(lcd, frame, 90, 28, 146, 142)?;
    draw_telemetry(lcd, frame, 244, 28, 62, 142)?;
    draw_footer_cards(lcd, frame, 14, 178, 292, 48)?;
    Ok(())
}

pub fn oscilloscope(lcd: &mut VirtualLcd, frame: u32) -> LcdResult<()> {
    fill_vertical_gradient(
        lcd,
        0,
        0,
        LCD_WIDTH,
        LCD_HEIGHT,
        Color::rgb(8, 6, 18),
        Color::rgb(18, 10, 36),
    )?;
    draw_scanlines(lcd, 4, Color::rgb(10, 6, 18))?;
    draw_scope_header(lcd, frame)?;
    draw_scope_grid(lcd, frame, 14, 34, 292, 146)?;
    draw_scope_footer(lcd, frame, 14, 190, 292, 36)?;
    Ok(())
}

pub fn startup(lcd: &mut VirtualLcd, frame: u32) -> LcdResult<()> {
    fill_vertical_gradient(
        lcd,
        0,
        0,
        LCD_WIDTH,
        LCD_HEIGHT,
        Color::rgb(12, 16, 22),
        Color::rgb(6, 8, 12),
    )?;

    let cx = LCD_WIDTH as i32 / 2;
    let cy = LCD_HEIGHT as i32 / 2 - 18;
    for radius in (16..90).step_by(14) {
        let glow = ((frame as f32) * 0.05 + radius as f32 * 0.08).sin() * 0.5 + 0.5;
        let color = mix_color(Color::rgb(24, 60, 92), Color::rgb(88, 255, 228), glow * 0.65);
        draw_circle(lcd, cx, cy, radius, color)?;
    }

    for spoke in 0..6 {
        let angle = frame as f32 * 0.04 + spoke as f32 * 1.047;
        let x1 = cx + (angle.cos() * 78.0) as i32;
        let y1 = cy + (angle.sin() * 78.0) as i32;
        draw_line(lcd, cx, cy, x1, y1, Color::rgb(44, 160, 140))?;
    }

    let progress = ((frame % 220) as f32 / 219.0).clamp(0.0, 1.0);
    draw_panel(
        lcd,
        52,
        172,
        216,
        28,
        Color::rgb(28, 42, 48),
        Color::rgb(8, 14, 16),
        Color::rgb(48, 196, 170),
    )?;
    lcd.fill_rect(66, 183, 188, 6, Color::rgb(16, 26, 28))?;
    fill_rect_safe(
        lcd,
        66,
        183,
        (188.0 * progress) as u16,
        6,
        Color::rgb(78, 240, 210),
    )?;

    for index in 0..8u16 {
        let pulse = ((frame / 6 + index as u32) % 8) < 4;
        let color = if pulse {
            Color::rgb(255, 198, 104)
        } else {
            Color::rgb(44, 52, 60)
        };
        draw_blip(lcd, 84 + index as i32 * 20, 214, 3, color)?;
    }

    for index in 0..18u32 {
        let angle = index as f32 * 0.349 + frame as f32 * 0.03;
        let orbit = 36.0 + (index % 3) as f32 * 18.0;
        let px = cx + (angle.cos() * orbit) as i32;
        let py = cy + (angle.sin() * orbit) as i32;
        draw_blip(lcd, px, py, 1 + (index % 2) as i32, Color::rgb(96, 214, 255))?;
    }

    Ok(())
}

pub fn gameboy_boot(lcd: &mut VirtualLcd, frame: u32) -> LcdResult<()> {
    let screen = Color::rgb(198, 207, 163);
    let band = Color::rgb(191, 200, 156);
    let text = Color::rgb(48, 56, 32);

    lcd.clear(screen)?;
    lcd.fill_rect(0, 0, 160, 144, screen)?;

    for row in (0..144).step_by(2) {
        lcd.fill_rect(0, row, 160, 1, band)?;
    }

    let logo_text = "NINTENDO";
    let logo_scale = 3;
    let (logo_w, logo_h) = measure_text(logo_text, logo_scale);
    let logo_x = ((160 - logo_w) / 2) as u16;

    let start_y = -((logo_h as i32) + 12);
    let end_y = 58i32;
    let travel_frames = 84.0f32;
    let t = (frame as f32 / travel_frames).clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t).powf(3.0);
    let logo_y = (start_y as f32 + (end_y - start_y) as f32 * eased).round() as i32;

    if logo_y < 144 {
        draw_text(lcd, logo_x, logo_y.max(0) as u16, logo_scale, text, logo_text)?;
    }

    Ok(())
}

fn draw_status_bar(lcd: &mut VirtualLcd, frame: u32) -> LcdResult<()> {
    draw_panel(
        lcd,
        12,
        10,
        296,
        12,
        Color::rgb(18, 54, 62),
        Color::rgb(7, 18, 22),
        Color::rgb(35, 120, 122),
    )?;

    for segment in 0..13u16 {
        let active = ((frame / 5 + segment as u32 * 2) % 13) < 6;
        let color = if active {
            Color::rgb(90, 255, 208)
        } else {
            Color::rgb(16, 44, 44)
        };
        lcd.fill_rect(22 + segment * 21, 13, 14, 4, color)?;
    }

    let pulse = ((frame as f32) * 0.12).sin() * 0.5 + 0.5;
    let width = 36 + (pulse * 72.0) as u16;
    lcd.fill_rect(218, 13, width.min(78), 4, Color::rgb(255, 196, 88))?;
    Ok(())
}

fn draw_level_bank(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> LcdResult<()> {
    draw_panel(
        lcd,
        x,
        y,
        width,
        height,
        Color::rgb(24, 58, 68),
        Color::rgb(6, 16, 20),
        Color::rgb(58, 190, 190),
    )?;

    let inner_x = x + 9;
    let inner_y = y + 10;
    let inner_w = width - 18;
    let inner_h = height - 20;

    for row in 0..6u16 {
        let y_line = inner_y + row * 20;
        lcd.fill_rect(inner_x, y_line, inner_w, 1, Color::rgb(10, 42, 44))?;
    }

    let bar_w = 6u16;
    let gap = 3u16;
    for index in 0..6u16 {
        let phase = frame as f32 * 0.14 + index as f32 * 0.8;
        let level = ((phase.sin() * 0.5 + 0.5).powf(1.5) * (inner_h as f32 - 14.0)) as u16 + 10;
        let bar_x = inner_x + 3 + index * (bar_w + gap);
        let bar_y = inner_y + inner_h - level;

        lcd.fill_rect(bar_x, bar_y, bar_w, level, Color::rgb(18, 74, 60))?;

        let hot = level.min(18);
        lcd.fill_rect(bar_x, bar_y, bar_w, hot, Color::rgb(246, 194, 82))?;
        fill_rect_safe(
            lcd,
            bar_x,
            bar_y + hot,
            bar_w,
            (level - hot).min(24),
            Color::rgb(82, 230, 162),
        )?;
    }

    let marker_y = inner_y + ((frame * 3) % (inner_h as u32 - 4)) as u16;
    lcd.fill_rect(inner_x + inner_w - 6, marker_y, 4, 4, Color::rgb(255, 120, 120))?;
    Ok(())
}

fn draw_radar(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> LcdResult<()> {
    draw_panel(
        lcd,
        x,
        y,
        width,
        height,
        Color::rgb(18, 60, 56),
        Color::rgb(4, 18, 16),
        Color::rgb(72, 255, 198),
    )?;

    let inner_x = x + 8;
    let inner_y = y + 8;
    let inner_w = width - 16;
    let inner_h = height - 16;
    lcd.fill_rect(inner_x, inner_y, inner_w, inner_h, Color::rgb(3, 14, 12))?;

    for step in 0..8u16 {
        let vx = inner_x + step * (inner_w / 7);
        let vy = inner_y + step * (inner_h / 7);
        lcd.fill_rect(vx, inner_y, 1, inner_h, Color::rgb(8, 36, 34))?;
        lcd.fill_rect(inner_x, vy, inner_w, 1, Color::rgb(8, 36, 34))?;
    }

    let cx = x as i32 + width as i32 / 2;
    let cy = y as i32 + height as i32 / 2;
    for radius in [18, 34, 50, 62] {
        draw_circle(lcd, cx, cy, radius, Color::rgb(18, 68, 58))?;
    }
    draw_line(lcd, cx - 62, cy, cx + 62, cy, Color::rgb(18, 68, 58))?;
    draw_line(lcd, cx, cy - 62, cx, cy + 62, Color::rgb(18, 68, 58))?;

    let base_angle = frame as f32 * 0.09;
    for (offset, color) in [
        (0.0f32, Color::rgb(120, 255, 208)),
        (-0.16, Color::rgb(48, 170, 132)),
        (-0.32, Color::rgb(20, 88, 68)),
    ] {
        let angle = base_angle + offset;
        let x1 = cx + (angle.cos() * 62.0) as i32;
        let y1 = cy + (angle.sin() * 62.0) as i32;
        draw_line(lcd, cx, cy, x1, y1, color)?;
    }

    for (index, (bx, by)) in [(36, -22), (-30, 18), (10, 28), (42, 24), (-12, -34)]
        .into_iter()
        .enumerate()
    {
        let pulse = (((frame as f32) * 0.11) + index as f32).sin() * 0.5 + 0.5;
        let color = if pulse > 0.78 {
            Color::rgb(255, 212, 112)
        } else {
            Color::rgb(88, 255, 186)
        };
        draw_blip(lcd, cx + bx, cy + by, 2 + (pulse * 2.0) as i32, color)?;
    }

    Ok(())
}

fn draw_telemetry(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> LcdResult<()> {
    draw_panel(
        lcd,
        x,
        y,
        width,
        height,
        Color::rgb(54, 58, 68),
        Color::rgb(8, 12, 18),
        Color::rgb(120, 150, 255),
    )?;

    let graph_x = x + 7;
    let graph_y = y + 10;
    let graph_w = width - 14;
    let graph_h = 84u16;

    for row in 0..5u16 {
        let y_line = graph_y + row * 16;
        lcd.fill_rect(graph_x, y_line, graph_w, 1, Color::rgb(18, 24, 40))?;
    }

    let mut prev = None;
    for sample in 0..graph_w {
        let t = frame as f32 * 0.08 + sample as f32 * 0.23;
        let signal = (t.sin() * 0.7 + (t * 0.37).cos() * 0.3) * 0.5 + 0.5;
        let py = graph_y + graph_h - 1 - (signal * (graph_h - 4) as f32) as u16;
        let px = graph_x + sample;

        if let Some((last_x, last_y)) = prev {
            draw_line(
                lcd,
                last_x as i32,
                last_y as i32,
                px as i32,
                py as i32,
                Color::rgb(126, 170, 255),
            )?;
        }
        prev = Some((px, py));

        if sample % 9 == 0 {
            lcd.fill_rect(px.saturating_sub(1), py.saturating_sub(1), 3, 3, Color::rgb(255, 210, 120))?;
        }
    }

    for meter in 0..3u16 {
        let meter_y = y + 108 + meter * 16;
        let fill = 14 + (((frame as f32 * 0.09) + meter as f32 * 1.2).sin() * 0.5 + 0.5) as u16 * 28;
        lcd.fill_rect(x + 7, meter_y, width - 14, 8, Color::rgb(14, 20, 30))?;
        lcd.fill_rect(x + 7, meter_y, fill.min(width - 14), 8, Color::rgb(92, 124, 255))?;
        lcd.fill_rect(x + 7, meter_y, fill.min(18), 8, Color::rgb(255, 186, 88))?;
    }

    Ok(())
}

fn draw_footer_cards(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> LcdResult<()> {
    draw_panel(
        lcd,
        x,
        y,
        width,
        height,
        Color::rgb(28, 52, 64),
        Color::rgb(6, 15, 18),
        Color::rgb(74, 170, 190),
    )?;

    for card in 0..3u16 {
        let card_x = x + 8 + card * 94;
        let card_w = if card == 2 { 86 } else { 88 };
        lcd.fill_rect(card_x, y + 8, card_w, height - 16, Color::rgb(8, 18, 24))?;
        draw_rect_outline(lcd, card_x, y + 8, card_w, height - 16, Color::rgb(24, 58, 70))?;

        for row in 0..3u16 {
            let base_y = y + 12 + row * 10;
            let level = (((frame as f32) * 0.1 + card as f32 * 0.6 + row as f32 * 0.8).sin() * 0.5
                + 0.5)
                * (card_w as f32 - 16.0);
            lcd.fill_rect(card_x + 6, base_y, card_w - 12, 5, Color::rgb(10, 24, 28))?;
            lcd.fill_rect(card_x + 6, base_y, level as u16 + 10, 5, Color::rgb(58, 224, 184))?;
        }
    }

    let indicator_x = x + width - 48;
    for dot in 0..5u16 {
        let pulse = (((frame / 3 + dot as u32 * 2) % 10) as u16) < 5;
        let color = if pulse {
            Color::rgb(255, 196, 92)
        } else {
            Color::rgb(42, 52, 58)
        };
        draw_blip(lcd, indicator_x as i32 + dot as i32 * 8, y as i32 + 14, 2, color)?;
    }

    Ok(())
}

fn draw_scope_header(lcd: &mut VirtualLcd, frame: u32) -> LcdResult<()> {
    draw_panel(
        lcd,
        14,
        10,
        292,
        14,
        Color::rgb(38, 54, 92),
        Color::rgb(8, 12, 22),
        Color::rgb(108, 146, 255),
    )?;

    for step in 0..10u16 {
        let lit = ((frame / 4 + step as u32) % 10) < 4;
        let color = if lit {
            Color::rgb(255, 196, 108)
        } else {
            Color::rgb(28, 36, 60)
        };
        lcd.fill_rect(24 + step * 27, 14, 18, 5, color)?;
    }

    Ok(())
}

fn draw_scope_grid(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> LcdResult<()> {
    draw_panel(
        lcd,
        x,
        y,
        width,
        height,
        Color::rgb(46, 54, 108),
        Color::rgb(8, 10, 20),
        Color::rgb(120, 150, 255),
    )?;

    let gx = x + 8;
    let gy = y + 8;
    let gw = width - 16;
    let gh = height - 16;

    for col in 0..10u16 {
        lcd.fill_rect(gx + col * (gw / 9), gy, 1, gh, Color::rgb(22, 28, 54))?;
    }
    for row in 0..7u16 {
        lcd.fill_rect(gx, gy + row * (gh / 6), gw, 1, Color::rgb(22, 28, 54))?;
    }

    draw_scope_wave(lcd, frame, gx, gy, gw, gh, 0.11, Color::rgb(88, 232, 196), 0.0)?;
    draw_scope_wave(lcd, frame, gx, gy, gw, gh, 0.16, Color::rgb(255, 198, 92), 1.2)?;
    draw_scope_wave(lcd, frame, gx, gy, gw, gh, 0.07, Color::rgb(120, 160, 255), 2.4)?;

    Ok(())
}

fn draw_scope_wave(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    speed: f32,
    color: Color,
    phase: f32,
) -> LcdResult<()> {
    let mut prev = None;
    for sample in 0..width {
        let t = frame as f32 * speed + sample as f32 * 0.09 + phase;
        let envelope = (t * 0.17).cos() * 0.18 + 0.82;
        let signal = ((t.sin() * 0.7) + ((t * 0.37).cos() * 0.3)) * 0.5 * envelope;
        let py = y as i32 + height as i32 / 2 - (signal * (height as f32 * 0.38)) as i32;
        let px = x as i32 + sample as i32;

        if let Some((last_x, last_y)) = prev {
            draw_line(lcd, last_x, last_y, px, py, color)?;
        }
        prev = Some((px, py));
    }
    Ok(())
}

fn draw_scope_footer(
    lcd: &mut VirtualLcd,
    frame: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> LcdResult<()> {
    draw_panel(
        lcd,
        x,
        y,
        width,
        height,
        Color::rgb(34, 40, 70),
        Color::rgb(8, 10, 18),
        Color::rgb(112, 144, 255),
    )?;

    for slot in 0..4u16 {
        let sx = x + 12 + slot * 70;
        let value = (((frame as f32) * 0.07 + slot as f32).sin() * 0.5 + 0.5) * 44.0;
        lcd.fill_rect(sx, y + 12, 50, 6, Color::rgb(18, 22, 34))?;
        lcd.fill_rect(sx, y + 12, value as u16 + 6, 6, Color::rgb(92, 214, 255))?;
    }

    Ok(())
}
