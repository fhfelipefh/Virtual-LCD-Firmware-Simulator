use virtual_lcd_examples::{run_scene_with, scenes, RuntimeOptions};
use virtual_lcd_renderer::ScreenRect;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_scene_with(
        RuntimeOptions {
            title: "Gameboy Boot",
            width: 160,
            height: 144,
            fps: 30,
            frame_path: "frames/handheld_classic.svg",
            screen_rect: ScreenRect::new(32, 34, 496, 432),
        },
        scenes::gameboy_boot,
    )
}
