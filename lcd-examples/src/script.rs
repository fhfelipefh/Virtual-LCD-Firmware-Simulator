use std::fmt::{Display, Formatter};

use lcd_core::{Result as LcdResult, VirtualLcd};
use lcd_renderer::ScreenRect;
use lcd_sdk::{Color, Lcd};

use crate::draw::{
    draw_circle, draw_line, draw_rect_outline, draw_text, fill_vertical_gradient, fill_rect_safe,
};
use crate::frame_asset_for;

#[derive(Debug, Clone)]
pub struct ScriptProgram {
    pub width: u16,
    pub height: u16,
    pub frame: FramePreset,
    commands: Vec<Command>,
}

#[derive(Debug, Clone, Copy)]
pub enum FramePreset {
    Auto,
    Handheld,
}

#[derive(Debug, Clone)]
enum Command {
    Clear(Color),
    FillRect(u16, u16, u16, u16, Color),
    Rect(u16, u16, u16, u16, Color),
    Line(i32, i32, i32, i32, Color),
    Circle(i32, i32, i32, Color),
    Gradient(u16, u16, u16, u16, Color, Color),
    Text(u16, u16, u16, Color, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    UnknownCommand(String),
    MissingArgument(&'static str),
    InvalidNumber(String),
    InvalidFramePreset(String),
}

impl Display for ScriptError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCommand(cmd) => write!(f, "unknown command: {cmd}"),
            Self::MissingArgument(arg) => write!(f, "missing argument: {arg}"),
            Self::InvalidNumber(value) => write!(f, "invalid number: {value}"),
            Self::InvalidFramePreset(value) => write!(f, "invalid frame preset: {value}"),
        }
    }
}

impl std::error::Error for ScriptError {}

impl ScriptProgram {
    pub fn parse(source: &str) -> Result<Self, ScriptError> {
        let mut width = crate::LCD_WIDTH;
        let mut height = crate::LCD_HEIGHT;
        let mut frame = FramePreset::Auto;
        let mut commands = Vec::new();

        for raw_line in source.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.split_whitespace();
            let command = parts
                .next()
                .ok_or(ScriptError::MissingArgument("command"))?;

            match command {
                "canvas" => {
                    width = parse_u16(parts.next(), "canvas width")?;
                    height = parse_u16(parts.next(), "canvas height")?;
                }
                "frame" => {
                    frame = match parts.next().ok_or(ScriptError::MissingArgument("frame preset"))? {
                        "auto" => FramePreset::Auto,
                        "handheld" => FramePreset::Handheld,
                        other => return Err(ScriptError::InvalidFramePreset(other.to_string())),
                    };
                }
                "clear" => {
                    commands.push(Command::Clear(parse_color(&mut parts)?));
                }
                "fill_rect" => {
                    let x = parse_u16(parts.next(), "x")?;
                    let y = parse_u16(parts.next(), "y")?;
                    let w = parse_u16(parts.next(), "width")?;
                    let h = parse_u16(parts.next(), "height")?;
                    let color = parse_color(&mut parts)?;
                    commands.push(Command::FillRect(x, y, w, h, color));
                }
                "rect" => {
                    let x = parse_u16(parts.next(), "x")?;
                    let y = parse_u16(parts.next(), "y")?;
                    let w = parse_u16(parts.next(), "width")?;
                    let h = parse_u16(parts.next(), "height")?;
                    let color = parse_color(&mut parts)?;
                    commands.push(Command::Rect(x, y, w, h, color));
                }
                "line" => {
                    let x0 = parse_i32(parts.next(), "x0")?;
                    let y0 = parse_i32(parts.next(), "y0")?;
                    let x1 = parse_i32(parts.next(), "x1")?;
                    let y1 = parse_i32(parts.next(), "y1")?;
                    let color = parse_color(&mut parts)?;
                    commands.push(Command::Line(x0, y0, x1, y1, color));
                }
                "circle" => {
                    let cx = parse_i32(parts.next(), "cx")?;
                    let cy = parse_i32(parts.next(), "cy")?;
                    let radius = parse_i32(parts.next(), "radius")?;
                    let color = parse_color(&mut parts)?;
                    commands.push(Command::Circle(cx, cy, radius, color));
                }
                "gradient" => {
                    let x = parse_u16(parts.next(), "x")?;
                    let y = parse_u16(parts.next(), "y")?;
                    let w = parse_u16(parts.next(), "width")?;
                    let h = parse_u16(parts.next(), "height")?;
                    let top = parse_color(&mut parts)?;
                    let bottom = parse_color(&mut parts)?;
                    commands.push(Command::Gradient(x, y, w, h, top, bottom));
                }
                "text" => {
                    let x = parse_u16(parts.next(), "x")?;
                    let y = parse_u16(parts.next(), "y")?;
                    let scale = parse_u16(parts.next(), "scale")?;
                    let color = parse_color(&mut parts)?;
                    let text = parts.collect::<Vec<_>>().join(" ");
                    if text.is_empty() {
                        return Err(ScriptError::MissingArgument("text"));
                    }
                    commands.push(Command::Text(x, y, scale, color, text.replace('_', " ")));
                }
                other => return Err(ScriptError::UnknownCommand(other.to_string())),
            }
        }

        Ok(Self {
            width,
            height,
            frame,
            commands,
        })
    }

    pub fn execute(&self, lcd: &mut VirtualLcd) -> LcdResult<()> {
        for command in &self.commands {
            match command {
                Command::Clear(color) => lcd.clear(*color)?,
                Command::FillRect(x, y, w, h, color) => fill_rect_safe(lcd, *x, *y, *w, *h, *color)?,
                Command::Rect(x, y, w, h, color) => draw_rect_outline(lcd, *x, *y, *w, *h, *color)?,
                Command::Line(x0, y0, x1, y1, color) => draw_line(lcd, *x0, *y0, *x1, *y1, *color)?,
                Command::Circle(cx, cy, radius, color) => draw_circle(lcd, *cx, *cy, *radius, *color)?,
                Command::Gradient(x, y, w, h, top, bottom) => {
                    fill_vertical_gradient(lcd, *x, *y, *w, *h, *top, *bottom)?
                }
                Command::Text(x, y, scale, color, text) => draw_text(lcd, *x, *y, *scale, *color, text)?,
            }
        }
        Ok(())
    }

    pub fn frame_asset(&self) -> (&'static str, ScreenRect) {
        match self.frame {
            FramePreset::Auto => frame_asset_for(self.width as usize, self.height as usize),
            FramePreset::Handheld => (
                "frames/handheld_classic.svg",
                ScreenRect::new(32, 34, 496, 432),
            ),
        }
    }
}

fn parse_color<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Result<Color, ScriptError> {
    let r = parse_u8(parts.next(), "r")?;
    let g = parse_u8(parts.next(), "g")?;
    let b = parse_u8(parts.next(), "b")?;
    Ok(Color::rgb(r, g, b))
}

fn parse_u8(value: Option<&str>, name: &'static str) -> Result<u8, ScriptError> {
    value
        .ok_or(ScriptError::MissingArgument(name))?
        .parse()
        .map_err(|_| ScriptError::InvalidNumber(name.to_string()))
}

fn parse_u16(value: Option<&str>, name: &'static str) -> Result<u16, ScriptError> {
    value
        .ok_or(ScriptError::MissingArgument(name))?
        .parse()
        .map_err(|_| ScriptError::InvalidNumber(name.to_string()))
}

fn parse_i32(value: Option<&str>, name: &'static str) -> Result<i32, ScriptError> {
    value
        .ok_or(ScriptError::MissingArgument(name))?
        .parse()
        .map_err(|_| ScriptError::InvalidNumber(name.to_string()))
}
