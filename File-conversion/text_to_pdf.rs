// Example: Convert text file to PDF
use file_converter::{FileConverter, PdfConfig};
use std::env;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input.txt> <output.pdf>", args[0]);
        std::process::exit(1);
    }

    let input_file = &args[1];
    let output_file = &args[2];

    let mut converter = FileConverter::new();

    // Create custom PDF configuration
    let config = PdfConfig {
        title: format!("Converted from {}", input_file),
        font_size: 12,
        margins: 20,
        line_spacing: 1.2,
        max_chars_per_line: Some(80),
        ..Default::default()
    };

    println!("Converting {} to {}...", input_file, output_file);

    converter.text_file_to_pdf(input_file, output_file, &config)?;

    println!("✅ Successfully converted to PDF!");
    println!("📄 Output file: {}", output_file);

    Ok(())
}
