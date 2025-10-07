//! Sample file creator for P2P file converter testing
//! 
//! This module creates various sample files for testing the conversion
//! and transfer functionality.

use std::fs;
use std::path::Path;
use anyhow::Result;

/// Create all sample files for testing
pub fn create_sample_files<P: AsRef<Path>>(output_dir: P) -> Result<()> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;

    // Create text samples
    create_text_samples(output_dir)?;

    // Create PDF samples  
    create_pdf_samples(output_dir)?;

    // Create markdown samples
    create_markdown_samples(output_dir)?;

    // Create test data files
    create_test_data_files(output_dir)?;

    println!("✅ All sample files created in: {}", output_dir.display());
    Ok(())
}

fn create_text_samples<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    // Simple text file
    fs::write(
        dir.join("simple.txt"),
        "Hello, World!\n\nThis is a simple text file for testing the P2P file converter."
    )?;

    // Multi-paragraph document
    fs::write(
        dir.join("document.txt"),
        r#"# P2P File Converter Test Document

This is a test document for the P2P file converter system. It demonstrates
various text formatting and content that should be properly handled during
the conversion process.

## Features to Test

The following features should be tested:

1. **Text Formatting**
   - Bold and italic text
   - Multiple paragraphs
   - Line breaks and spacing

2. **Special Characters**
   - Unicode: 你好世界 🌍
   - Accented characters: Café, naïve, résumé
   - Mathematical symbols: α, β, γ, π, Σ
   - Currency: $, €, £, ¥

3. **Long Lines**
   This is a very long line that should test the text wrapping functionality of the PDF conversion system and ensure that it handles line breaks appropriately without cutting off words in the middle.

## Code Examples

Here's a simple Rust code example:

```rust
fn main() {
    println!("Hello, P2P world!");

    let message = "File conversion successful";
    println!("{}", message);
}
```

## Lists and Structure

### Unordered List
- First item
- Second item with more text
- Third item

### Ordered List
1. Initialize the system
2. Connect to peers
3. Transfer files
4. Convert formats
5. Verify results

## Conclusion

This document tests various text elements to ensure proper conversion
from text to PDF format while maintaining readability and structure.

---
End of test document.
"#
    )?;

    // Large text file for performance testing
    let large_content = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(1000);
    fs::write(dir.join("large_text.txt"), large_content)?;

    // Unicode test file
    fs::write(
        dir.join("unicode_test.txt"),
        r#"Unicode Test File

This file contains various Unicode characters to test proper handling:

## Latin Scripts
- English: Hello World
- French: Bonjour le monde
- Spanish: Hola Mundo  
- German: Hallo Welt
- Italian: Ciao Mondo

## Non-Latin Scripts
- Chinese: 你好世界
- Japanese: こんにちは世界
- Korean: 안녕하세요 세계
- Arabic: مرحبا بالعالم
- Russian: Привет мир
- Hebrew: שלום עולם

## Symbols and Emojis
- Mathematical: ∑, ∏, ∫, ∂, ∇, ∞
- Currency: $, €, £, ¥, ₹, ₿
- Arrows: ←, →, ↑, ↓, ↔, ⇒
- Emojis: 🌍, 🚀, 📄, 💻, 🔄, ✅

## Special Cases
- Combining characters: é (e + ´), ñ (n + ~)
- Zero-width characters and spaces
- Right-to-left text: العربية
- Bidirectional text: Hello العربية World

This tests the robustness of Unicode handling in the conversion process.
"#
    )?;

    // Empty file
    fs::write(dir.join("empty.txt"), "")?;

    // File with only whitespace
    fs::write(dir.join("whitespace_only.txt"), "   \n\t\n   \n")?;

    println!("✅ Text sample files created");
    Ok(())
}

fn create_pdf_samples<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    // Simple PDF file with basic structure
    let simple_pdf = b"%PDF-1.4
1 0 obj
<<
/Type /Catalog
/Pages 2 0 R
>>
endobj

2 0 obj
<<
/Type /Pages
/Kids [3 0 R]
/Count 1
>>
endobj

3 0 obj
<<
/Type /Page
/Parent 2 0 R
/MediaBox [0 0 612 792]
/Contents 4 0 R
/Resources <<
  /Font <<
    /F1 5 0 R
  >>
>>
>>
endobj

4 0 obj
<<
/Length 125
>>
stream
BT
/F1 12 Tf
72 720 Td
(P2P File Converter Test PDF) Tj
0 -20 Td
(This is a simple test PDF document.) Tj
0 -20 Td
(It should be convertible to text format.) Tj
ET
endstream
endobj

5 0 obj
<<
/Type /Font
/Subtype /Type1
/BaseFont /Helvetica
>>
endobj

xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000258 00000 n 
0000000434 00000 n 
trailer
<<
/Size 6
/Root 1 0 R
>>
startxref
528
%%EOF";

    fs::write(dir.join("simple.pdf"), simple_pdf)?;

    // Multi-page PDF
    let multipage_pdf = b"%PDF-1.4
1 0 obj
<<
/Type /Catalog
/Pages 2 0 R
>>
endobj

2 0 obj
<<
/Type /Pages
/Kids [3 0 R 6 0 R]
/Count 2
>>
endobj

3 0 obj
<<
/Type /Page
/Parent 2 0 R
/MediaBox [0 0 612 792]
/Contents 4 0 R
/Resources <<
  /Font <<
    /F1 5 0 R
  >>
>>
>>
endobj

4 0 obj
<<
/Length 89
>>
stream
BT
/F1 14 Tf
72 720 Td
(Multi-page PDF Test) Tj
0 -30 Td
(This is page 1 of 2) Tj
ET
endstream
endobj

5 0 obj
<<
/Type /Font
/Subtype /Type1
/BaseFont /Helvetica
>>
endobj

6 0 obj
<<
/Type /Page
/Parent 2 0 R
/MediaBox [0 0 612 792]
/Contents 7 0 R
/Resources <<
  /Font <<
    /F1 5 0 R
  >>
>>
>>
endobj

7 0 obj
<<
/Length 89
>>
stream
BT
/F1 14 Tf
72 720 Td
(Page 2 Content) Tj
0 -30 Td
(This is page 2 of 2) Tj
ET
endstream
endobj

xref
0 8
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000120 00000 n 
0000000263 00000 n 
0000000403 00000 n 
0000000497 00000 n 
0000000640 00000 n 
trailer
<<
/Size 8
/Root 1 0 R
>>
startxref
780
%%EOF";

    fs::write(dir.join("multipage.pdf"), multipage_pdf)?;

    println!("✅ PDF sample files created");
    Ok(())
}

fn create_markdown_samples<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    fs::write(
        dir.join("sample.md"),
        r#"# P2P File Converter Documentation

## Overview

The P2P File Converter is a distributed system for converting files between different formats using peer-to-peer networking.

### Key Features

- **Distributed Architecture**: No central server required
- **Format Support**: Text, PDF, and Markdown files
- **Real-time Progress**: Live transfer and conversion status
- **Error Recovery**: Automatic retry with exponential backoff
- **Security**: Encrypted peer-to-peer communication

## Getting Started

### Installation

```bash
# Install from source
git clone https://github.com/user/p2p-file-converter
cd p2p-file-converter
cargo build --release
```

### Basic Usage

#### Receiver Mode
```bash
p2p-converter --listen /ip4/0.0.0.0/tcp/8080
```

#### Sender Mode
```bash
p2p-converter --target /ip4/peer-ip/tcp/8080/p2p/peer-id \
              --file document.txt --format pdf
```

## Architecture

The system consists of several key components:

1. **File Converter**: Handles format conversion
2. **P2P Network**: Manages peer connections
3. **Stream Handler**: Processes file transfers
4. **Event Loop**: Coordinates all operations

### Protocol Flow

```
Sender                    Receiver
  |                         |
  |--- FileTransferRequest->|
  |                         |
  |<-- Acknowledgment ------|
  |                         |
  |--- File Chunks -------->|
  |                         |
  |<-- Progress Updates ----|
  |                         |
  |<-- Final Response ------|
```

## Configuration

Create a `config.toml` file:

```toml
[network]
connection_timeout_secs = 30
max_retry_attempts = 5

[files]
max_file_size = 104857600  # 100MB
allowed_extensions = ["txt", "pdf", "md"]

[conversion]
timeout_secs = 300
parallel_processing = true
```

## Error Handling

The system provides comprehensive error handling:

- **Network Errors**: Connection failures, timeouts
- **File Errors**: Permission denied, file not found
- **Conversion Errors**: Unsupported formats, processing failures
- **Validation Errors**: Invalid addresses, malformed data

## Security Considerations

- All network communication is encrypted using Noise protocol
- File paths are validated to prevent directory traversal
- File size limits prevent resource exhaustion
- Peer authentication prevents unauthorized access

## Troubleshooting

### Connection Issues
- Verify peer addresses and network connectivity
- Check firewall settings
- Ensure peers are running and accessible

### Conversion Problems
- Verify file format support
- Check available fonts for PDF generation
- Review error logs for specific details

### Performance Optimization
- Adjust chunk sizes for network conditions
- Configure concurrent transfer limits
- Monitor system resources

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a pull request

## License

MIT License - see LICENSE file for details.
"#
    )?;

    fs::write(
        dir.join("tutorial.md"),
        r#"# P2P File Converter Tutorial

This tutorial will guide you through using the P2P file converter step by step.

## Step 1: Setup

First, ensure you have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Step 2: Build the Application

```bash
cargo build --release
```

## Step 3: Start a Receiver

Open a terminal and start the receiver:

```bash
./target/release/p2p-converter --listen /ip4/0.0.0.0/tcp/8080
```

## Step 4: Send a File

In another terminal, send a file:

```bash
./target/release/p2p-converter \
  --target /ip4/127.0.0.1/tcp/8080/p2p/PEER_ID \
  --file sample.txt \
  --format pdf
```

## Step 5: Verify Results

Check the receiver's output directory for the converted file.

## Advanced Usage

### Batch Operations

Send multiple files at once:

```bash
for file in *.txt; do
  p2p-converter --target ADDRESS --file "$file" --format pdf
done
```

### Custom Configuration

Create a config file for persistent settings:

```toml
[files]
output_directory = "./converted_files"
max_file_size = 52428800  # 50MB

[network]
connection_timeout_secs = 60
```

## Tips and Tricks

1. **Monitor Progress**: Use `--verbose` for detailed output
2. **Check Logs**: Review error logs for troubleshooting
3. **Network Testing**: Use `ping` command to test connectivity
4. **Performance**: Adjust concurrent transfer limits

Happy converting! 🚀
"#
    )?;

    println!("✅ Markdown sample files created");
    Ok(())
}

fn create_test_data_files<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();

    // Create test configuration file
    fs::write(
        dir.join("test_config.toml"),
        r#"# Test configuration for P2P File Converter

[network]
connection_timeout_secs = 30
max_retry_attempts = 3
keep_alive = true
bandwidth_limit = 0

[files]
max_file_size = 10485760  # 10MB for testing
allowed_extensions = ["txt", "pdf", "md", "rtf"]
output_directory = "./test_output"
integrity_check = true

[conversion]
timeout_secs = 120  # 2 minutes for testing
parallel_processing = true
max_memory_mb = 512
font_directory = "./test_fonts"

[error_handling]
verbose_errors = true
log_errors = true
error_log_path = "./test_errors.log"
enable_recovery = true
"#
    )?;

    // Create test addresses file
    fs::write(
        dir.join("test_addresses.txt"),
        r#"# Test peer addresses for P2P file converter

# Local testing addresses
/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ
/ip4/127.0.0.1/tcp/8081/p2p/12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X
/ip4/127.0.0.1/tcp/8082/p2p/12D3KooWQYErvNPJAeNSZDAyYk7dxGy6PqLEKrVpfnFrL7bKzGNs

# Network testing addresses (examples)
/ip4/192.168.1.100/tcp/9000/p2p/12D3KooWExample1
/ip4/192.168.1.101/tcp/9000/p2p/12D3KooWExample2

# IPv6 addresses (examples)
/ip6/::1/tcp/8080/p2p/12D3KooWExample3
/ip6/2001:db8::1/tcp/8080/p2p/12D3KooWExample4
"#
    )?;

    // Create test script
    fs::write(
        dir.join("run_tests.sh"),
        r#"#!/bin/bash
# Test script for P2P File Converter

set -e

echo "🧪 P2P File Converter Test Suite"
echo "=================================="

# Run different test categories
echo "📋 Running unit tests..."
cargo test --lib

echo "🔗 Running integration tests..."
cargo test --test integration_tests

echo "🌐 Running network tests..."
cargo test networking_tests

echo "📄 Running conversion tests..."
cargo test conversion_tests

echo "⚡ Running performance tests..."
cargo test performance_tests --release

echo "🎯 Running end-to-end tests..."
cargo test e2e_tests

echo ""
echo "✅ All tests completed!"
echo "📊 View detailed results with: cargo test -- --nocapture"
echo "🔍 Run specific tests with: cargo test <test_name>"
echo "📈 Run benchmarks with: cargo bench"
"#
    )?;

    // Make test script executable (on Unix systems)
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("chmod")
            .args(["+x", &dir.join("run_tests.sh").to_string_lossy()])
            .output()
            .ok(); // Ignore errors - might not be on Unix
    }

    // Create performance test data
    let perf_data = "Performance test data line.\n".repeat(10000); // ~300KB
    fs::write(dir.join("performance_test.txt"), perf_data)?;

    // Create binary test file (should be detected as unknown)
    let binary_data: Vec<u8> = (0..=255).cycle().take(1024).collect();
    fs::write(dir.join("binary_test.bin"), binary_data)?;

    println!("✅ Test data files created");
    Ok(())
}

/// Main function to create all sample files
fn main() -> Result<()> {
    println!("🏗️  Creating sample files for P2P File Converter testing...");

    let sample_dir = "sample_files";
    create_sample_files(sample_dir)?;

    println!();
    println!("📁 Sample files created in: {}/", sample_dir);
    println!("📋 Contents:");

    // List all created files
    if let Ok(entries) = fs::read_dir(sample_dir) {
        let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        files.sort_by_key(|e| e.file_name());

        for entry in files {
            let path = entry.path();
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let size_str = if size < 1024 {
                format!("{} B", size)
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)
            };

            println!("  📄 {} ({}))", 
                     path.file_name().unwrap().to_string_lossy(),
                     size_str);
        }
    }

    println!();
    println!("🚀 Usage:");
    println!("  cp sample_files/* ./");
    println!("  ./run_tests.sh");
    println!("  cargo test");

    Ok(())
}
