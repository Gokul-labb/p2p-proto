# File Converter Library

A comprehensive Rust library for converting between text and PDF files with advanced file type detection.

## Features

- **Text to PDF Conversion**: Convert plain text files to properly formatted PDF documents
- **PDF Text Extraction**: Extract text content from PDF files  
- **File Type Detection**: Detect file types using magic number signatures
- **Configurable PDF Generation**: Custom fonts, margins, styling, and layout
- **Error Handling**: Comprehensive error types with detailed messages
- **Cross-platform**: Works on Windows, macOS, and Linux

## Magic Number Detection

The library detects file types by examining magic number signatures:

- **PDF Files**: `%PDF` signature (bytes: `0x25, 0x50, 0x44, 0x46`)
- **Text Files**: Heuristic detection based on UTF-8 validity and printable character ratio
- **Binary Files**: Detected by presence of null bytes or non-printable content

## PDF Generation Features

- Professional page layout with configurable margins
- Font loading from system fonts or custom font directories
- Text wrapping at word boundaries
- Configurable font size, line spacing, and colors
- Document metadata (title, author, etc.)
- Support for multi-page documents

## Quick Start

```rust
use file_converter::{FileConverter, PdfConfig};

// Create converter instance
let mut converter = FileConverter::new();

// Convert text to PDF
let config = PdfConfig {
    title: "My Document".to_string(),
    font_size: 12,
    margins: 20,
    ..Default::default()
};
converter.text_file_to_pdf("input.txt", "output.pdf", &config)?;

// Extract text from PDF
converter.pdf_file_to_text("document.pdf", "extracted.txt")?;

// Detect file type
let file_type = converter.detect_file_type("unknown_file")?;
println!("File type: {}", file_type);
```

## Error Handling

The library provides detailed error types for different failure scenarios:

```rust
match converter.text_to_pdf(text, &config) {
    Ok(pdf_bytes) => println!("Success: {} bytes generated", pdf_bytes.len()),
    Err(ConversionError::FontLoadingFailed(msg)) => eprintln!("Font error: {}", msg),
    Err(ConversionError::PdfGenerationFailed(msg)) => eprintln!("PDF error: {}", msg),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
file-converter = "0.1.0"
genpdf = "0.2"
pdf-extract = "0.7"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
```

## Font Requirements

For PDF generation, you need font files available. The library searches for fonts in:

1. `./fonts/` directory (relative to working directory)
2. System font directories:
   - Linux: `/usr/share/fonts`
   - macOS: `/System/Library/Fonts`  
   - Windows: `C:\Windows\Fonts`

### Setting up fonts

Create a fonts directory and add TrueType fonts:

```bash
mkdir fonts
# Copy font files like LiberationSans-Regular.ttf to fonts/
```

## Examples

See the `examples/` directory for complete usage examples:

- `text_to_pdf.rs`: Convert text files to PDF
- `pdf_to_text.rs`: Extract text from PDF files
- `file_detection.rs`: Detect file types
- `cli_tool.rs`: Complete CLI application

## Testing

Run the test suite:

```bash
cargo test
./test_converter.sh  # Integration tests
```

## License

MIT License - see LICENSE file for details.
