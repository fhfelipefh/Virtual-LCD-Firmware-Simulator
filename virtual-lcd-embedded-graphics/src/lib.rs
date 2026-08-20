#![no_std]

use core::marker::PhantomData;
use embedded_graphics_core::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::{PixelColor, Rgb565, Rgb888, RgbColor},
    primitives::Rectangle,
};
use virtual_lcd_sdk::{Color, Lcd};

/// A trait for converting embedded-graphics colors to the virtual LCD color format.
pub trait IntoVirtualColor {
    fn into_virtual(self) -> Color;
}

impl IntoVirtualColor for Rgb565 {
    fn into_virtual(self) -> Color {
        let r = (self.r() << 3) | (self.r() >> 2);
        let g = (self.g() << 2) | (self.g() >> 4);
        let b = (self.b() << 3) | (self.b() >> 2);
        Color::rgb(r, g, b)
    }
}

impl IntoVirtualColor for Rgb888 {
    fn into_virtual(self) -> Color {
        Color::rgb(self.r(), self.g(), self.b())
    }
}

/// A wrapper that implements `DrawTarget` for any type that implements `Lcd`.
pub struct VirtualLcdDrawTarget<T, C> {
    lcd: T,
    width: u16,
    height: u16,
    _color: PhantomData<C>,
}

impl<T: Lcd, C> VirtualLcdDrawTarget<T, C> {
    /// Creates a new draw target for the given LCD.
    pub fn new(lcd: T, width: u16, height: u16) -> Self {
        Self {
            lcd,
            width,
            height,
            _color: PhantomData,
        }
    }

    /// Consumes the wrapper and returns the underlying LCD instance.
    pub fn into_inner(self) -> T {
        self.lcd
    }

    /// Get a mutable reference to the underlying LCD instance.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.lcd
    }
}

impl<T: Lcd, C> OriginDimensions for VirtualLcdDrawTarget<T, C> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl<T: Lcd, C> DrawTarget for VirtualLcdDrawTarget<T, C>
where
    C: PixelColor + IntoVirtualColor,
{
    type Color = C;
    type Error = T::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if coord.x >= 0
                && coord.y >= 0
                && coord.x < self.width as i32
                && coord.y < self.height as i32
            {
                self.lcd
                    .draw_pixel(coord.x as u16, coord.y as u16, color.into_virtual())?;
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let intersection = area.intersection(&Rectangle::new(
            embedded_graphics_core::geometry::Point::zero(),
            self.size(),
        ));

        if intersection.size.width > 0 && intersection.size.height > 0 {
            self.lcd.fill_rect(
                intersection.top_left.x as u16,
                intersection.top_left.y as u16,
                intersection.size.width as u16,
                intersection.size.height as u16,
                color.into_virtual(),
            )?;
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.lcd.clear(color.into_virtual())
    }
}
