// Complete CLI tool for file conversion
use anyhow::Result;
use clap::{Parser, Subcommand};
use file_converter::{FileConverter, PdfConfig, FileType};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "file-converter")]
#[command(about = "Convert between text and PDF files")]
#[command(version = "1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, help = "Enable verbose output")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert text file to PDF
    TextToPdf {
        /// Input text file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output PDF file path
        #[arg(short, long)]
        output: PathBuf,

        /// Document title
        #[arg(short, long, default_value = "Converted Document")]
        title: String,

        /// Font size in points
        #[arg(long, default_value = "12")]
        font_size: u8,

        /// Page margins in points
        #[arg(long, default_value = "20")]
        margins: u8,
    },

    /// Extract text from PDF file
    PdfToText {
        /// Input PDF file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output text file path
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Detect file type
    Detect {
        /// File paths to analyze
        files: Vec<PathBuf>,
    },

    /// Auto-convert based on file extensions
    Convert {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_level))
        .init();

    let mut converter = FileConverter::new();

    match cli.command {
        Commands::TextToPdf { input, output, title, font_size, margins } => {
            let config = PdfConfig {
                title,
                font_size,
                margins,
                ..Default::default()
            };

            println!("📝 → 📕 Converting text to PDF...");
            converter.text_file_to_pdf(&input, &output, &config)?;
            println!("✅ Success: {} → {}", input.display(), output.display());
        }

        Commands::PdfToText { input, output } => {
            println!("📕 → 📝 Extracting text from PDF...");
            converter.pdf_file_to_text(&input, &output)?;
            println!("✅ Success: {} → {}", input.display(), output.display());

            // Show statistics
            let text = std::fs::read_to_string(&output)?;
            println!("📊 Extracted {} characters, {} lines", 
                     text.len(), text.lines().count());
        }

        Commands::Detect { files } => {
            println!("🔍 File type detection:");
            for file in files {
                match converter.detect_file_type(&file) {
                    Ok(file_type) => {
                        let icon = match file_type {
                            FileType::Pdf => "📕",
                            FileType::Text => "📝", 
                            FileType::Unknown => "❓",
                        };
                        println!("  {} {} → {}", icon, file.display(), file_type);
                    }
                    Err(e) => println!("  ❌ {} → Error: {}", file.display(), e),
                }
            }
        }

        Commands::Convert { input, output } => {
            println!("🔄 Auto-converting based on file extensions...");
            converter.convert_file(&input, &output, None)?;
            println!("✅ Success: {} → {}", input.display(), output.display());
        }
    }

    Ok(())
}
