// Example: Extract text from PDF file
use file_converter::FileConverter;
use std::env;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input.pdf> <output.txt>", args[0]);
        std::process::exit(1);
    }

    let input_file = &args[1];
    let output_file = &args[2];

    let converter = FileConverter::new();

    println!("Extracting text from {} to {}...", input_file, output_file);

    converter.pdf_file_to_text(input_file, output_file)?;

    println!("✅ Successfully extracted text from PDF!");
    println!("📄 Output file: {}", output_file);

    // Display some statistics
    let extracted_text = std::fs::read_to_string(output_file)?;
    println!("📊 Extracted {} characters", extracted_text.len());
    println!("📊 {} lines of text", extracted_text.lines().count());

    Ok(())
}
