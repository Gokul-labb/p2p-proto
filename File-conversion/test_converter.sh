#!/bin/bash
# Test script for file converter functionality

set -e

echo "=== File Converter Test Suite ==="

# Create test directory
mkdir -p test_files
cd test_files

# Create a test text file
cat > sample.txt << 'EOF'
# Sample Document

This is a sample text document for testing the file converter.

## Features

- Text to PDF conversion
- PDF to text extraction  
- File type detection using magic numbers
- Proper error handling

## Content

Lorem ipsum dolor sit amet, consectetur adipiscing elit. 
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

Ut enim ad minim veniam, quis nostrud exercitation ullamco 
laboris nisi ut aliquip ex ea commodo consequat.

### Code Example

```rust
let converter = FileConverter::new();
let result = converter.text_to_pdf("Hello, World!", &config)?;
```

This document tests various formatting scenarios including:
- Multiple paragraphs
- Long lines that need wrapping
- Special characters: äöü ñ é à
- Numbers: 123, 456.78, 9.99

End of document.
EOF

echo "📝 Created sample text file"

# Test 1: File type detection
echo "🔍 Testing file type detection..."
cargo run --example detect_file_type sample.txt

# Test 2: Text to PDF conversion
echo "📝 → 📕 Testing text to PDF conversion..."
cargo run --example convert_text_to_pdf sample.txt sample.pdf

# Test 3: Verify PDF was created and detect its type
if [ -f sample.pdf ]; then
    echo "✅ PDF file created successfully"
    echo "🔍 Detecting PDF file type..."
    cargo run --example detect_file_type sample.pdf
else
    echo "❌ PDF file was not created"
    exit 1
fi

# Test 4: PDF to text extraction
echo "📕 → 📝 Testing PDF to text extraction..."
cargo run --example extract_pdf_text sample.pdf extracted.txt

# Test 5: Compare original and extracted text
if [ -f extracted.txt ]; then
    echo "✅ Text extraction successful"
    echo "📊 Original text: $(wc -c < sample.txt) characters"
    echo "📊 Extracted text: $(wc -c < extracted.txt) characters"

    # Show first few lines of extracted text
    echo "📄 First 3 lines of extracted text:"
    head -3 extracted.txt
else
    echo "❌ Text extraction failed"
    exit 1
fi

# Clean up
cd ..

echo "🎉 All tests completed successfully!"
echo "📁 Test files are in ./test_files/ directory"
