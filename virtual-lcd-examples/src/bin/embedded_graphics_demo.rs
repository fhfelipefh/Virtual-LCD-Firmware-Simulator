use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyleBuilder, Rectangle, Triangle},
    text::Text,
};
use virtual_lcd_core::VirtualLcd;
use virtual_lcd_embedded_graphics::VirtualLcdDrawTarget;

fn eg_scene(lcd: &mut VirtualLcd, _tick: u32) -> virtual_lcd_core::Result<()> {
    // Wrap the LCD to use with embedded-graphics
    let mut display = VirtualLcdDrawTarget::<&mut VirtualLcd, Rgb565>::new(lcd, 320, 240);

    // Clear the display with black
    let _ = display.clear(Rgb565::BLACK);

    // Draw some shapes
    let style = PrimitiveStyleBuilder::new()
        .stroke_width(2)
        .stroke_color(Rgb565::RED)
        .fill_color(Rgb565::GREEN)
        .build();

    let _ = Circle::new(Point::new(100, 50), 60)
        .into_styled(style)
        .draw(&mut display);

    let _ = Rectangle::new(Point::new(20, 150), Size::new(80, 60))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::BLUE)
                .build(),
        )
        .draw(&mut display);

    let _ = Triangle::new(
        Point::new(200, 120),
        Point::new(250, 200),
        Point::new(150, 200),
    )
    .into_styled(
        PrimitiveStyleBuilder::new()
            .stroke_width(3)
            .stroke_color(Rgb565::YELLOW)
            .build(),
    )
    .draw(&mut display);

    // Draw some text
    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let _ = Text::new(
        "Virtual LCD Simulator\nembedded-graphics",
        Point::new(120, 30),
        text_style,
    )
    .draw(&mut display);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    virtual_lcd_examples::run_scene("Embedded Graphics Demo", eg_scene)
}
