fn main() -> Result<(), Box<dyn std::error::Error>> {
    lcd_examples::run_scene("LCD Oscilloscope", lcd_examples::scenes::oscilloscope)
}
