// Batch file conversion example

use anyhow::Result;
use p2p_file_transfer::{FileConversionConfig, FileConverter, FileType, PdfConfig};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🔄 Batch File Conversion Example");

    // Create temporary directory for test files
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    info!("📁 Working directory: {}", temp_path.display());

    // Create test files
    create_test_files(&temp_path).await?;

    // Setup file converter
    let mut converter = FileConverter::new();
    let pdf_config = PdfConfig {
        title: "Batch Converted Document".to_string(),
        font_size: 12,
        margins: 20,
        ..Default::default()
    };

    // Process all files in the directory
    let mut entries = fs::read_dir(&temp_path).await?;
    let mut conversion_count = 0;

    while let Some(entry) = entries.next_entry().await? {
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        info!("🔍 Processing: {}", file_name);

        // Detect file type
        let file_type = match converter.detect_file_type(&file_path) {
            Ok(ft) => ft,
            Err(e) => {
                info!("⚠️  Could not detect type for {}: {}", file_name, e);
                continue;
            }
        };

        info!("📋 Detected type: {}", file_type);

        // Convert based on type
        match file_type {
            FileType::Text => {
                let output_path = file_path.with_extension("pdf");

                match converter.text_file_to_pdf(&file_path, &output_path, &pdf_config) {
                    Ok(()) => {
                        info!("✅ Converted {} to PDF", file_name);
                        conversion_count += 1;
                    }
                    Err(e) => {
                        info!("❌ Failed to convert {} to PDF: {}", file_name, e);
                    }
                }
            }

            FileType::Pdf => {
                let output_path = file_path.with_extension("txt");

                match converter.pdf_file_to_text(&file_path, &output_path) {
                    Ok(()) => {
                        info!("✅ Extracted text from {}", file_name);
                        conversion_count += 1;
                    }
                    Err(e) => {
                        info!("❌ Failed to extract text from {}: {}", file_name, e);
                    }
                }
            }

            FileType::Unknown => {
                info!("⚠️  Unknown file type for {}, skipping", file_name);
            }
        }
    }

    info!("🎉 Batch conversion completed!");
    info!("📊 Converted {} files", conversion_count);
    info!("📁 Results saved in: {}", temp_path.display());

    // Keep temp directory for inspection
    temp_dir.into_path();

    Ok(())
}

async fn create_test_files(dir: &std::path::Path) -> Result<()> {
    // Create text files
    let text_files = vec![
        ("document1.txt", "This is the first test document.
It contains multiple lines.

And paragraphs."),
        ("document2.txt", "# Markdown Document

## Section 1

This is a markdown-style document.

- Item 1
- Item 2
- Item 3"),
        ("notes.txt", "Quick notes:

1. Remember to test P2P transfers
2. Check file conversion
3. Verify progress tracking"),
    ];

    for (filename, content) in text_files {
        let file_path = dir.join(filename);
        fs::write(&file_path, content).await?;
        info!("📄 Created: {}", filename);
    }

    // Note: Creating actual PDF files would require more complex setup
    // For this example, we'll just work with text files

    Ok(())
}
