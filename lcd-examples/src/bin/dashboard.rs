fn main() -> Result<(), Box<dyn std::error::Error>> {
    lcd_examples::run_scene("LCD Dashboard", lcd_examples::scenes::dashboard)
}
