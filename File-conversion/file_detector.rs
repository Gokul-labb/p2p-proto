// Example: Detect file types using magic numbers
use file_converter::{FileConverter, FileType};
use std::env;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file1> [file2] [file3] ...", args[0]);
        std::process::exit(1);
    }

    let converter = FileConverter::new();

    println!("🔍 File Type Detection Results:");
    println!("{:-<50}", "");

    for file_path in &args[1..] {
        print!("📁 {:<30}", file_path);

        match converter.detect_file_type(file_path) {
            Ok(file_type) => {
                let icon = match file_type {
                    FileType::Pdf => "📕",
                    FileType::Text => "📝",
                    FileType::Unknown => "❓",
                };
                println!(" → {} {}", icon, file_type);
            }
            Err(e) => {
                println!(" → ❌ Error: {}", e);
            }
        }
    }

    println!("{:-<50}", "");
    println!("📋 Legend: 📕 PDF  📝 Text  ❓ Unknown");

    Ok(())
}
