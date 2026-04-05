#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use lcd_core::Framebuffer;
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl ScreenRect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Debug)]
pub enum RendererError {
    Window(minifb::Error),
    Io(std::io::Error),
    SvgParse(String),
    SvgRender(String),
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Window(error) => write!(f, "window error: {error}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::SvgParse(error) => write!(f, "svg parse error: {error}"),
            Self::SvgRender(error) => write!(f, "svg render error: {error}"),
        }
    }
}

impl std::error::Error for RendererError {}

impl From<minifb::Error> for RendererError {
    fn from(value: minifb::Error) -> Self {
        Self::Window(value)
    }
}

impl From<std::io::Error> for RendererError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, RendererError>;

#[derive(Debug)]
pub struct SvgFrame {
    width: usize,
    height: usize,
    base_buffer: Vec<u32>,
    screen: ScreenRect,
}

impl SvgFrame {
    pub fn load(path: impl AsRef<Path>, screen: ScreenRect) -> Result<Self> {
        let data = fs::read(path)?;
        let options = Options::default();
        let tree =
            Tree::from_data(&data, &options).map_err(|error| RendererError::SvgParse(error.to_string()))?;
        let size = tree.size().to_int_size();
        let mut pixmap = Pixmap::new(size.width(), size.height())
            .ok_or_else(|| RendererError::SvgRender("unable to allocate svg pixmap".to_string()))?;
        let mut pixmap_mut = pixmap.as_mut();
        resvg::render(&tree, Transform::identity(), &mut pixmap_mut);

        let width = size.width() as usize;
        let height = size.height() as usize;
        if screen.x + screen.width > width || screen.y + screen.height > height {
            return Err(RendererError::SvgRender(
                "screen rect exceeds rendered svg bounds".to_string(),
            ));
        }

        let base_buffer = pixmap
            .data()
            .chunks_exact(4)
            .map(|rgba| ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | rgba[2] as u32)
            .collect();

        Ok(Self {
            width,
            height,
            base_buffer,
            screen,
        })
    }
}

#[derive(Debug)]
pub struct WindowRenderer {
    window: Window,
    frame: SvgFrame,
    buffer: Vec<u32>,
}

impl WindowRenderer {
    pub fn new(title: &str, frame: SvgFrame) -> Result<Self> {
        let mut window = Window::new(
            title,
            frame.width,
            frame.height,
            WindowOptions {
                resize: false,
                scale: Scale::X1,
                scale_mode: ScaleMode::Center,
                ..WindowOptions::default()
            },
        )?;
        window.set_target_fps(60);

        Ok(Self {
            buffer: frame.base_buffer.clone(),
            window,
            frame,
        })
    }

    pub fn is_open(&self) -> bool {
        self.window.is_open() && !self.window.is_key_down(Key::Escape)
    }

    pub fn update(&mut self, lcd_frame: &Framebuffer) -> Result<()> {
        self.buffer.clone_from(&self.frame.base_buffer);
        composite_framebuffer(
            &mut self.buffer,
            self.frame.width,
            self.frame.height,
            lcd_frame,
            self.frame.screen,
        );
        self.window
            .update_with_buffer(&self.buffer, self.frame.width, self.frame.height)?;
        Ok(())
    }
}

fn composite_framebuffer(
    output: &mut [u32],
    output_width: usize,
    output_height: usize,
    framebuffer: &Framebuffer,
    screen: ScreenRect,
) {
    let fit_width = screen.width as f32 / framebuffer.width() as f32;
    let fit_height = screen.height as f32 / framebuffer.height() as f32;
    let scale = fit_width.min(fit_height);

    let draw_width = ((framebuffer.width() as f32 * scale).round() as usize).max(1);
    let draw_height = ((framebuffer.height() as f32 * scale).round() as usize).max(1);
    let offset_x = screen.x + (screen.width.saturating_sub(draw_width)) / 2;
    let offset_y = screen.y + (screen.height.saturating_sub(draw_height)) / 2;

    for y in 0..screen.height {
        let row = (screen.y + y) * output_width;
        for x in 0..screen.width {
            output[row + screen.x + x] = 0x000000;
        }
    }

    for y in 0..draw_height {
        let src_y = ((y as f32 / draw_height as f32) * framebuffer.height() as f32).floor() as u16;
        let dst_y = offset_y + y;
        if dst_y >= output_height {
            continue;
        }

        let row = dst_y * output_width;
        for x in 0..draw_width {
            let src_x =
                ((x as f32 / draw_width as f32) * framebuffer.width() as f32).floor() as u16;
            let dst_x = offset_x + x;
            if dst_x >= output_width {
                continue;
            }

            if let Some(pixel) = framebuffer.get_pixel(
                src_x.min(framebuffer.width() - 1),
                src_y.min(framebuffer.height() - 1),
            ) {
                output[row + dst_x] =
                    ((pixel.r as u32) << 16) | ((pixel.g as u32) << 8) | pixel.b as u32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{composite_framebuffer, ScreenRect};
    use lcd_core::{Color, Framebuffer};

    #[test]
    fn composite_writes_inside_screen_rect() {
        let mut output = vec![0x112233; 16 * 16];
        let mut frame = Framebuffer::new(2, 2);
        frame.set_pixel(0, 0, Color::RED).expect("pixel should be valid");
        frame.set_pixel(1, 0, Color::GREEN).expect("pixel should be valid");
        frame.set_pixel(0, 1, Color::BLUE).expect("pixel should be valid");
        frame.set_pixel(1, 1, Color::WHITE).expect("pixel should be valid");

        composite_framebuffer(&mut output, 16, 16, &frame, ScreenRect::new(4, 4, 8, 8));

        assert_eq!(output[0], 0x112233);
        assert_eq!(output[4 * 16 + 4], 0xFF0000);
        assert_eq!(output[4 * 16 + 11], 0x00FF00);
        assert_eq!(output[11 * 16 + 4], 0x0000FF);
    }
}
