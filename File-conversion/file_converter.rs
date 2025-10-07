use anyhow::{Context, Result};
use genpdf::{
    elements::{Paragraph, Text, LinearLayout, TableLayout, StyledElement},
    fonts::{self, FontData, FontFamily},
    style::{Color, Style},
    Document, Element, Alignment, SimplePageDecorator,
};
use pdf_extract::extract_text;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Custom error types for file conversion operations
#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("PDF generation failed: {0}")]
    PdfGenerationFailed(String),

    #[error("PDF text extraction failed: {0}")]
    PdfExtractionFailed(String),

    #[error("File type detection failed: {0}")]
    FileTypeDetectionFailed(String),

    #[error("Invalid input data: {0}")]
    InvalidInput(String),

    #[error("Font loading failed: {0}")]
    FontLoadingFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Supported file types based on magic number detection
#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    /// PDF document (%PDF signature)
    Pdf,
    /// Plain text file (UTF-8, ASCII, or other text encoding)
    Text,
    /// Unknown or unsupported file type
    Unknown,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Pdf => write!(f, "PDF"),
            FileType::Text => write!(f, "Text"),
            FileType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Magic number signatures for file type detection
pub struct MagicNumbers {
    signatures: HashMap<Vec<u8>, FileType>,
}

impl MagicNumbers {
    /// Create a new MagicNumbers instance with predefined signatures
    pub fn new() -> Self {
        let mut signatures = HashMap::new();

        // PDF signatures - %PDF- (0x25, 0x50, 0x44, 0x46, 0x2D)
        signatures.insert(vec![0x25, 0x50, 0x44, 0x46], FileType::Pdf); // %PDF

        Self { signatures }
    }

    /// Add a custom signature for detection
    pub fn add_signature(&mut self, signature: Vec<u8>, file_type: FileType) {
        self.signatures.insert(signature, file_type);
    }

    /// Detect file type by checking magic numbers
    pub fn detect_from_bytes(&self, bytes: &[u8]) -> FileType {
        // Check for PDF signature first (most specific)
        for (signature, file_type) in &self.signatures {
            if bytes.len() >= signature.len() && bytes.starts_with(signature) {
                return file_type.clone();
            }
        }

        // If no magic number matches, try to detect if it's text
        if self.is_likely_text(bytes) {
            return FileType::Text;
        }

        FileType::Unknown
    }

    /// Heuristic to determine if bytes represent text content
    fn is_likely_text(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // Check for UTF-8 BOM
        if bytes.len() >= 3 && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return true;
        }

        // Sample first 1024 bytes or entire content if smaller
        let sample_size = std::cmp::min(1024, bytes.len());
        let sample = &bytes[0..sample_size];

        // Check for null bytes (strong indicator of binary content)
        if sample.contains(&0) {
            return false;
        }

        // Check if valid UTF-8
        if let Ok(text) = std::str::from_utf8(sample) {
            // Count printable characters
            let printable_count = text.chars()
                .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                .count();

            let total_chars = text.chars().count();

            // If more than 70% are printable ASCII characters, likely text
            if total_chars > 0 {
                let printable_ratio = printable_count as f64 / total_chars as f64;
                return printable_ratio > 0.7;
            }
        }

        false
    }
}

impl Default for MagicNumbers {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for PDF generation
#[derive(Debug, Clone)]
pub struct PdfConfig {
    /// Document title
    pub title: String,
    /// Page margins in points
    pub margins: u8,
    /// Font size in points
    pub font_size: u8,
    /// Line spacing multiplier
    pub line_spacing: f64,
    /// Text color
    pub text_color: Color,
    /// Font family name
    pub font_family: String,
    /// Maximum characters per line (for text wrapping)
    pub max_chars_per_line: Option<usize>,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            title: "Converted Document".to_string(),
            margins: 20,
            font_size: 12,
            line_spacing: 1.2,
            text_color: Color::Rgb(0, 0, 0), // Black
            font_family: "LiberationSans".to_string(),
            max_chars_per_line: Some(80),
        }
    }
}

/// File converter with support for text-to-PDF and PDF-to-text
pub struct FileConverter {
    magic_numbers: MagicNumbers,
    font_cache: HashMap<String, FontFamily<FontData>>,
}

impl FileConverter {
    /// Create a new file converter instance
    pub fn new() -> Self {
        Self {
            magic_numbers: MagicNumbers::new(),
            font_cache: HashMap::new(),
        }
    }

    /// Detect file type from file path
    pub fn detect_file_type<P: AsRef<Path>>(&self, path: P) -> Result<FileType> {
        let bytes = fs::read(&path)
            .with_context(|| format!("Failed to read file: {}", path.as_ref().display()))?;

        Ok(self.magic_numbers.detect_from_bytes(&bytes))
    }

    /// Detect file type from byte content
    pub fn detect_file_type_from_bytes(&self, bytes: &[u8]) -> FileType {
        self.magic_numbers.detect_from_bytes(bytes)
    }

    /// Convert text content to PDF bytes
    pub fn text_to_pdf(&mut self, text: &str, config: &PdfConfig) -> Result<Vec<u8>> {
        info!("Converting text to PDF with title: '{}'", config.title);

        // Load or get cached font family
        let font_family = self.get_or_load_font(&config.font_family)?;

        // Create document
        let mut doc = Document::new(font_family);
        doc.set_title(&config.title);
        doc.set_line_spacing(config.line_spacing);

        // Set up page decorator with margins
        let mut decorator = SimplePageDecorator::new();
        decorator.set_margins(config.margins as i32);
        doc.set_page_decorator(decorator);

        // Process text content
        let processed_text = self.process_text_for_pdf(text, config);

        // Add content to document
        for paragraph_text in processed_text {
            if paragraph_text.trim().is_empty() {
                // Add empty paragraph for spacing
                doc.push(Paragraph::new(""));
            } else {
                // Create styled text
                let mut paragraph = Paragraph::new(&paragraph_text);

                // Apply styling
                let style = Style::new()
                    .with_font_size(config.font_size)
                    .with_color(config.text_color);

                paragraph = paragraph.styled(style);
                doc.push(paragraph);
            }
        }

        // Render to bytes
        let mut buffer = Vec::new();
        doc.render(&mut buffer)
            .map_err(|e| ConversionError::PdfGenerationFailed(e.to_string()))?;

        info!("Successfully generated PDF with {} bytes", buffer.len());
        Ok(buffer)
    }

    /// Convert text file to PDF file
    pub fn text_file_to_pdf<P: AsRef<Path>>(
        &mut self, 
        input_path: P, 
        output_path: P, 
        config: &PdfConfig
    ) -> Result<()> {
        let input_path = input_path.as_ref();
        let output_path = output_path.as_ref();

        info!("Converting text file {} to PDF {}", 
              input_path.display(), output_path.display());

        // Verify input file is text
        let file_type = self.detect_file_type(input_path)?;
        if file_type != FileType::Text {
            return Err(ConversionError::UnsupportedFileType(
                format!("Expected text file, found: {}", file_type)
            ));
        }

        // Read text content
        let text_content = fs::read_to_string(input_path)
            .with_context(|| format!("Failed to read text file: {}", input_path.display()))?;

        // Convert to PDF
        let pdf_bytes = self.text_to_pdf(&text_content, config)?;

        // Write PDF file
        fs::write(output_path, pdf_bytes)
            .with_context(|| format!("Failed to write PDF file: {}", output_path.display()))?;

        info!("Successfully converted {} to {}", 
              input_path.display(), output_path.display());
        Ok(())
    }

    /// Extract text content from PDF bytes
    pub fn pdf_to_text(&self, pdf_bytes: &[u8]) -> Result<String> {
        info!("Extracting text from PDF ({} bytes)", pdf_bytes.len());

        // Verify it's a PDF file
        let file_type = self.detect_file_type_from_bytes(pdf_bytes);
        if file_type != FileType::Pdf {
            return Err(ConversionError::UnsupportedFileType(
                format!("Expected PDF file, found: {}", file_type)
            ));
        }

        // Extract text using pdf-extract
        let text = extract_text(pdf_bytes)
            .map_err(|e| ConversionError::PdfExtractionFailed(e.to_string()))?;

        let text = text.trim().to_string();
        info!("Successfully extracted {} characters of text from PDF", text.len());

        Ok(text)
    }

    /// Extract text from PDF file to text file
    pub fn pdf_file_to_text<P: AsRef<Path>>(
        &self, 
        input_path: P, 
        output_path: P
    ) -> Result<()> {
        let input_path = input_path.as_ref();
        let output_path = output_path.as_ref();

        info!("Converting PDF file {} to text {}", 
              input_path.display(), output_path.display());

        // Read PDF file
        let pdf_bytes = fs::read(input_path)
            .with_context(|| format!("Failed to read PDF file: {}", input_path.display()))?;

        // Extract text
        let text_content = self.pdf_to_text(&pdf_bytes)?;

        // Write text file
        fs::write(output_path, text_content)
            .with_context(|| format!("Failed to write text file: {}", output_path.display()))?;

        info!("Successfully converted {} to {}", 
              input_path.display(), output_path.display());
        Ok(())
    }

    /// Generic file conversion - automatically detects input type and converts
    pub fn convert_file<P: AsRef<Path>>(
        &mut self,
        input_path: P,
        output_path: P,
        config: Option<&PdfConfig>,
    ) -> Result<()> {
        let input_path = input_path.as_ref();
        let output_path = output_path.as_ref();

        // Detect input file type
        let input_type = self.detect_file_type(input_path)?;

        // Determine output type from extension
        let output_extension = output_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match (input_type, output_extension.as_str()) {
            (FileType::Text, "pdf") => {
                let config = config.unwrap_or(&PdfConfig::default());
                self.text_file_to_pdf(input_path, output_path, config)
            }
            (FileType::Pdf, "txt") => {
                self.pdf_file_to_text(input_path, output_path)
            }
            (input_type, output_ext) => {
                Err(ConversionError::UnsupportedFileType(
                    format!("Conversion from {} to {} is not supported", input_type, output_ext)
                ))
            }
        }
    }

    /// Load or get cached font family
    fn get_or_load_font(&mut self, font_name: &str) -> Result<FontFamily<FontData>> {
        if let Some(font_family) = self.font_cache.get(font_name) {
            return Ok(font_family.clone());
        }

        // Try to load font from system or embedded fonts
        let font_family = self.load_font_family(font_name)?;
        self.font_cache.insert(font_name.to_string(), font_family.clone());

        Ok(font_family)
    }

    /// Load font family (tries multiple approaches)
    fn load_font_family(&self, font_name: &str) -> Result<FontFamily<FontData>> {
        // Try to load from fonts directory
        if let Ok(font_family) = fonts::from_files("./fonts", font_name, None) {
            debug!("Loaded font '{}' from ./fonts directory", font_name);
            return Ok(font_family);
        }

        // Try to load from system fonts (common paths)
        let system_font_paths = [
            "/usr/share/fonts",
            "/System/Library/Fonts",
            "C:\Windows\Fonts",
        ];

        for path in &system_font_paths {
            if let Ok(font_family) = fonts::from_files(path, font_name, None) {
                debug!("Loaded font '{}' from system path: {}", font_name, path);
                return Ok(font_family);
            }
        }

        // Fallback to built-in font or error
        warn!("Could not load font '{}', falling back to built-in font", font_name);

        // Use DejaVu Sans as fallback (commonly available)
        if font_name != "DejaVuSans" {
            if let Ok(font_family) = self.load_font_family("DejaVuSans") {
                return Ok(font_family);
            }
        }

        Err(ConversionError::FontLoadingFailed(
            format!("Could not load font '{}' or any fallback fonts", font_name)
        ))
    }

    /// Process text for PDF conversion (handle line wrapping, etc.)
    fn process_text_for_pdf(&self, text: &str, config: &PdfConfig) -> Vec<String> {
        let mut paragraphs = Vec::new();

        for line in text.lines() {
            if let Some(max_chars) = config.max_chars_per_line {
                if line.len() > max_chars {
                    // Wrap long lines
                    let wrapped_lines = self.wrap_text(line, max_chars);
                    paragraphs.extend(wrapped_lines);
                } else {
                    paragraphs.push(line.to_string());
                }
            } else {
                paragraphs.push(line.to_string());
            }
        }

        paragraphs
    }

    /// Simple text wrapping at word boundaries
    fn wrap_text(&self, text: &str, max_chars: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                result.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }

        result
    }
}

impl Default for FileConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for file type detection
pub mod detection {
    use super::*;

    /// Quick file type detection from file path (reads only header)
    pub fn detect_file_type_quick<P: AsRef<Path>>(path: P) -> Result<FileType> {
        let mut file = fs::File::open(&path)
            .with_context(|| format!("Failed to open file: {}", path.as_ref().display()))?;

        // Read first 32 bytes for magic number detection
        let mut buffer = [0u8; 32];
        let bytes_read = file.read(&mut buffer)?;

        let magic = MagicNumbers::new();
        Ok(magic.detect_from_bytes(&buffer[..bytes_read]))
    }

    /// Validate that a file is the expected type
    pub fn validate_file_type<P: AsRef<Path>>(path: P, expected_type: FileType) -> Result<()> {
        let detected_type = detect_file_type_quick(&path)?;

        if detected_type != expected_type {
            return Err(ConversionError::InvalidInput(
                format!(
                    "File {} is type {}, expected {}",
                    path.as_ref().display(),
                    detected_type,
                    expected_type
                )
            ).into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_pdf_magic_number_detection() {
        let pdf_header = b"%PDF-1.4\n";
        let magic = MagicNumbers::new();

        assert_eq!(magic.detect_from_bytes(pdf_header), FileType::Pdf);
    }

    #[test]
    fn test_text_detection() {
        let text_content = b"Hello, this is a text file with normal content.";
        let magic = MagicNumbers::new();

        assert_eq!(magic.detect_from_bytes(text_content), FileType::Text);
    }

    #[test]
    fn test_binary_detection() {
        let binary_content = b"\x00\x01\x02\x03\xFF\xFE\xFD";
        let magic = MagicNumbers::new();

        assert_eq!(magic.detect_from_bytes(binary_content), FileType::Unknown);
    }

    #[test]
    fn test_text_to_pdf_conversion() {
        let mut converter = FileConverter::new();
        let config = PdfConfig::default();
        let test_text = "This is a test document.\nWith multiple lines.\n\nAnd paragraphs.";

        let result = converter.text_to_pdf(test_text, &config);
        assert!(result.is_ok());

        let pdf_bytes = result.unwrap();
        assert!(!pdf_bytes.is_empty());
        assert!(pdf_bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_file_type_detection_from_file() -> Result<()> {
        // Create temporary text file
        let mut text_file = NamedTempFile::new()?;
        writeln!(text_file, "This is a test text file.")?;

        let converter = FileConverter::new();
        let file_type = converter.detect_file_type(text_file.path())?;

        assert_eq!(file_type, FileType::Text);
        Ok(())
    }

    #[test]
    fn test_text_wrapping() {
        let converter = FileConverter::new();
        let long_text = "This is a very long line that should be wrapped at word boundaries when it exceeds the maximum character limit";

        let wrapped = converter.wrap_text(long_text, 20);

        // Each line should be <= 20 characters
        for line in &wrapped {
            assert!(line.len() <= 20);
        }

        // Should have multiple lines
        assert!(wrapped.len() > 1);
    }
}

/// Example usage and CLI interface
pub mod examples {
    use super::*;

    /// Example: Convert text file to PDF
    pub fn example_text_to_pdf() -> Result<()> {
        let mut converter = FileConverter::new();

        // Custom PDF configuration
        let config = PdfConfig {
            title: "My Document".to_string(),
            font_size: 14,
            margins: 25,
            line_spacing: 1.5,
            ..Default::default()
        };

        converter.text_file_to_pdf(
            "input.txt",
            "output.pdf",
            &config
        )?;

        println!("Converted input.txt to output.pdf");
        Ok(())
    }

    /// Example: Extract text from PDF
    pub fn example_pdf_to_text() -> Result<()> {
        let converter = FileConverter::new();

        converter.pdf_file_to_text(
            "document.pdf",
            "extracted.txt"
        )?;

        println!("Extracted text from document.pdf to extracted.txt");
        Ok(())
    }

    /// Example: Auto-detect and convert
    pub fn example_auto_convert() -> Result<()> {
        let mut converter = FileConverter::new();

        // Will automatically detect input type and convert based on output extension
        converter.convert_file(
            "input.txt",      // Text file
            "output.pdf",     // PDF output -> text-to-PDF conversion
            None              // Use default config
        )?;

        converter.convert_file(
            "document.pdf",   // PDF file  
            "extracted.txt",  // Text output -> PDF-to-text conversion
            None
        )?;

        println!("Auto-converted files based on extensions");
        Ok(())
    }

    /// Example: File type detection
    pub fn example_file_detection() -> Result<()> {
        let converter = FileConverter::new();

        let files = ["document.pdf", "readme.txt", "image.jpg"];

        for file in &files {
            match converter.detect_file_type(file) {
                Ok(file_type) => println!("{}: {}", file, file_type),
                Err(e) => println!("{}: Error - {}", file, e),
            }
        }

        Ok(())
    }
}
