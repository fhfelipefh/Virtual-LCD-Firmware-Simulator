fn main() -> Result<(), Box<dyn std::error::Error>> {
    virtual_lcd_examples::run_scene(
        "LCD Dashboard",
        virtual_lcd_examples::scenes::dashboard,
    )
}
