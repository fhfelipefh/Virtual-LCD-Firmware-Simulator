fn main() -> Result<(), Box<dyn std::error::Error>> {
    lcd_examples::run_scene("LCD Startup Sequence", lcd_examples::scenes::startup)
}
