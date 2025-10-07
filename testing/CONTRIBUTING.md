# Contributing to P2P File Converter

Thank you for your interest in contributing to the P2P File Converter project! This guide will help you get started with contributing code, documentation, tests, and other improvements.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Contributing Process](#contributing-process)
5. [Code Style and Standards](#code-style-and-standards)
6. [Testing Guidelines](#testing-guidelines)
7. [Documentation](#documentation)
8. [Performance Considerations](#performance-considerations)
9. [Security Guidelines](#security-guidelines)
10. [Community and Communication](#community-and-communication)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/conduct.html). By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

## Getting Started

### Prerequisites

- **Rust 1.70+**: Install from [rustup.rs](https://rustup.rs/)
- **Git**: For version control
- **IDE/Editor**: VS Code with rust-analyzer, or your preferred Rust development environment

### Areas for Contribution

We welcome contributions in several areas:

- **Core Features**: File conversion, P2P networking, protocol improvements
- **Performance**: Optimization, benchmarking, profiling
- **Testing**: Unit tests, integration tests, end-to-end testing
- **Documentation**: API docs, tutorials, examples, guides
- **Tooling**: Development tools, CI/CD improvements, automation
- **Platform Support**: Windows, macOS, different Linux distributions
- **Bug Fixes**: Identifying and fixing issues
- **Security**: Security audits, vulnerability fixes

## Development Setup

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/p2p-file-converter.git
cd p2p-file-converter

# Add upstream remote
git remote add upstream https://github.com/ORIGINAL_OWNER/p2p-file-converter.git
```

### 2. Install Development Dependencies

```bash
# Install useful development tools
cargo install cargo-watch      # Auto-rebuild on changes
cargo install cargo-tarpaulin  # Code coverage
cargo install cargo-audit      # Security audit
cargo install cargo-outdated   # Check for outdated dependencies
cargo install cargo-expand     # Expand macros for debugging
cargo install cargo-flamegraph # Performance profiling
```

### 3. Build and Test

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run with coverage
cargo tarpaulin --out Html

# Security audit
cargo audit

# Check for outdated dependencies
cargo outdated
```

### 4. IDE Setup

#### VS Code
Install these extensions:
- **rust-analyzer**: Rust language server
- **CodeLLDB**: Debugging support
- **Better TOML**: TOML syntax highlighting
- **GitLens**: Git integration

#### Vim/Neovim
Configure with rust-analyzer LSP support.

#### IntelliJ IDEA/CLion
Install the Rust plugin.

## Contributing Process

### 1. Create an Issue

Before starting work, create or find an existing issue describing:
- The problem you're solving
- Your proposed approach
- Any breaking changes

For larger features, consider creating a design document or RFC.

### 2. Create a Branch

```bash
# Create and switch to a new branch
git checkout -b feature/your-feature-name

# Or for bug fixes
git checkout -b fix/issue-number-description
```

### 3. Make Changes

Follow the code style guidelines and write tests for your changes.

### 4. Test Your Changes

```bash
# Run all tests
cargo test

# Run specific tests
cargo test conversion_tests

# Run with coverage
cargo tarpaulin

# Test documentation examples
cargo test --doc

# Integration testing
cargo test --test integration_tests

# Performance tests
cargo test --release performance_tests
```

### 5. Commit and Push

```bash
# Stage your changes
git add .

# Commit with a descriptive message
git commit -m "feat: add support for DOCX file conversion

- Implement DOCX to PDF conversion
- Add DOCX file type detection
- Update supported formats list
- Add comprehensive tests"

# Push to your fork
git push origin feature/your-feature-name
```

### 6. Create Pull Request

1. Go to GitHub and create a pull request
2. Fill out the PR template completely
3. Link to related issues
4. Add reviewers if you know who should review
5. Respond to feedback promptly

## Code Style and Standards

### Rust Code Style

We follow standard Rust conventions:

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings

# Check code without building
cargo check
```

### Code Organization

```
src/
├── lib.rs                 # Public API and re-exports
├── main.rs               # Binary entry point
├── cli/                  # Command-line interface
├── error_handling/       # Error types and utilities
├── file_converter/       # File conversion logic
├── file_sender/          # P2P file sending
├── p2p_stream_handler/   # Protocol handling
├── config_utilities/     # Configuration management
└── main_event_loop/      # Application coordination
```

### Naming Conventions

- **Functions**: `snake_case`
- **Types**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`
- **Files**: `snake_case.rs`

### Documentation Standards

#### Public APIs

All public functions, types, and modules must have documentation:

```rust
/// Converts a text file to PDF format.
///
/// This function takes text content and converts it to a PDF document
/// using the specified configuration options.
///
/// # Arguments
///
/// * `text` - The text content to convert
/// * `config` - PDF configuration options
///
/// # Returns
///
/// Returns the PDF data as a byte vector on success, or an error if
/// the conversion fails.
///
/// # Errors
///
/// This function will return an error if:
/// - The text contains unsupported Unicode characters
/// - Font loading fails
/// - PDF generation encounters an internal error
///
/// # Examples
///
/// ```rust
/// use p2p_file_converter::{FileConverter, PdfConfig};
///
/// let mut converter = FileConverter::new();
/// let config = PdfConfig::default();
/// let pdf_data = converter.text_to_pdf("Hello, World!", &config)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn text_to_pdf(&mut self, text: &str, config: &PdfConfig) -> Result<Vec<u8>> {
    // Implementation...
}
```

#### Error Handling

Use the `?` operator and provide context:

```rust
use anyhow::Context;

pub fn process_file(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    validate_content(&content)
        .context("File content validation failed")?;

    Ok(())
}
```

### Performance Guidelines

#### Memory Management

```rust
// Good: Use references when possible
fn process_data(data: &[u8]) -> Result<ProcessedData> { 
    // ...
}

// Good: Use Arc for shared data
let shared_config = Arc::new(config);

// Avoid: Unnecessary cloning
fn bad_example(data: Vec<u8>) -> Vec<u8> {
    data.clone() // Unnecessary
}
```

#### Async Best Practices

```rust
// Good: Use async/await properly
async fn fetch_data() -> Result<Data> {
    let response = http_client.get("/api/data").await?;
    let data = response.json().await?;
    Ok(data)
}

// Good: Use select! for concurrent operations
tokio::select! {
    result = operation_a() => handle_a(result),
    result = operation_b() => handle_b(result),
}
```

## Testing Guidelines

### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_basic_functionality() {
        // Unit test
    }

    #[tokio::test]
    async fn test_async_functionality() {
        // Async unit test
    }
}
```

### Integration Tests

Place integration tests in `tests/` directory:

```rust
// tests/file_conversion.rs
use p2p_file_converter::{FileConverter, PdfConfig};

#[tokio::test]
async fn test_text_to_pdf_conversion() {
    let mut converter = FileConverter::new();
    let config = PdfConfig::default();

    let result = converter.text_to_pdf("Test content", &config);
    assert!(result.is_ok());

    let pdf_data = result.unwrap();
    assert!(!pdf_data.is_empty());
    assert!(pdf_data.starts_with(b"%PDF"));
}
```

### Test Utilities

Create helper functions for common test scenarios:

```rust
// tests/common/mod.rs
use tempfile::NamedTempFile;
use std::io::Write;

pub fn create_test_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

pub fn create_test_pdf() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"%PDF-1.4\n...").unwrap();
    file
}
```

### Property-Based Testing

For complex algorithms, consider property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_file_size_calculation(
        file_size in 0u64..1_000_000_000u64
    ) {
        let chunks = calculate_chunks(file_size, CHUNK_SIZE);
        let reconstructed_size = chunks * CHUNK_SIZE;
        prop_assert!(reconstructed_size >= file_size);
    }
}
```

### Performance Testing

```rust
#[tokio::test]
async fn bench_large_file_conversion() {
    let large_text = "A".repeat(1_000_000); // 1MB text
    let mut converter = FileConverter::new();
    let config = PdfConfig::default();

    let start = std::time::Instant::now();
    let result = converter.text_to_pdf(&large_text, &config);
    let duration = start.elapsed();

    assert!(result.is_ok());
    assert!(duration < std::time::Duration::from_secs(30));
    println!("Conversion took: {:?}", duration);
}
```

## Documentation

### Types of Documentation

1. **API Documentation**: Rustdoc comments for public APIs
2. **User Guides**: README, usage examples, tutorials  
3. **Developer Guides**: Architecture, design decisions
4. **Specifications**: Protocol documentation, data formats

### Writing Good Documentation

#### Be Clear and Concise

```rust
// Good
/// Validates that a file path is safe to use.
/// Returns an error if the path contains ".." or other unsafe components.

// Less clear
/// Does validation stuff on paths and things
```

#### Provide Examples

```rust
/// Formats a file size in human-readable format.
///
/// # Examples
///
/// ```
/// use p2p_file_converter::utils::format_file_size;
///
/// assert_eq!(format_file_size(1024), "1.0 KB");
/// assert_eq!(format_file_size(1536), "1.5 KB");
/// ```
pub fn format_file_size(bytes: u64) -> String {
    // Implementation...
}
```

#### Document Error Conditions

```rust
/// Connects to a peer at the specified address.
///
/// # Errors
///
/// Returns an error if:
/// - The address is malformed
/// - The peer is unreachable
/// - The connection times out
/// - Protocol negotiation fails
pub async fn connect_to_peer(addr: &str) -> Result<PeerConnection> {
    // Implementation...
}
```

## Performance Considerations

### Benchmarking

Add benchmarks for performance-critical code:

```rust
// benches/conversion_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use p2p_file_converter::{FileConverter, PdfConfig};

fn bench_text_to_pdf(c: &mut Criterion) {
    let mut converter = FileConverter::new();
    let config = PdfConfig::default();
    let text = "Sample text content for benchmarking".repeat(1000);

    c.bench_function("text_to_pdf_1kb", |b| {
        b.iter(|| {
            converter.text_to_pdf(black_box(&text), black_box(&config))
        })
    });
}

criterion_group!(benches, bench_text_to_pdf);
criterion_main!(benches);
```

### Memory Profiling

Use tools like `heaptrack` or `valgrind` for memory analysis:

```bash
# Install heaptrack (Linux)
sudo apt install heaptrack

# Profile memory usage
heaptrack ./target/release/p2p-converter --file large_file.txt

# Analyze results
heaptrack_gui heaptrack.p2p-converter.*.gz
```

### CPU Profiling

Use `flamegraph` for CPU profiling:

```bash
# Profile with flamegraph
cargo flamegraph --bin p2p-converter -- --file test.txt

# This generates a flamegraph.svg file
```

## Security Guidelines

### Input Validation

Always validate inputs:

```rust
pub fn process_filename(filename: &str) -> Result<PathBuf> {
    // Validate filename doesn't contain path traversal
    if filename.contains("..") || filename.contains('/') || filename.contains('\') {
        return Err(anyhow::anyhow!("Invalid filename: contains path separators"));
    }

    // Validate length
    if filename.len() > 255 {
        return Err(anyhow::anyhow!("Filename too long"));
    }

    Ok(PathBuf::from(filename))
}
```

### Resource Limits

Implement resource limits:

```rust
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB
const MAX_CONCURRENT_TRANSFERS: usize = 10;

pub fn validate_file_size(size: u64) -> Result<()> {
    if size > MAX_FILE_SIZE {
        return Err(anyhow::anyhow!("File too large: {} bytes", size));
    }
    Ok(())
}
```

### Dependency Management

- Regularly update dependencies
- Audit dependencies for vulnerabilities
- Use minimal feature sets
- Pin versions for reproducible builds

```bash
# Security audit
cargo audit

# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated
```

## Community and Communication

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: General questions, ideas
- **Pull Requests**: Code review and discussion

### Getting Help

1. **Search existing issues** before creating new ones
2. **Check documentation** and examples first
3. **Provide minimal reproducible examples** when reporting bugs
4. **Be respectful** and patient with maintainers and contributors

### Issue Templates

When reporting bugs, include:

- Rust version
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Error messages or logs
- Minimal code example

### Feature Requests

When proposing features:

- Explain the use case
- Consider alternatives
- Discuss implementation approach
- Consider backward compatibility

## Release Process

### Version Management

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Changelog

All changes are documented in `CHANGELOG.md`:

```markdown
## [1.2.0] - 2025-10-07

### Added
- Support for DOCX file conversion
- IPv6 address support in peer discovery
- Configuration validation improvements

### Changed
- Improved error messages for network failures
- Updated dependency versions

### Fixed
- Fixed memory leak in large file processing
- Resolved race condition in concurrent transfers
```

### Release Checklist

Before releasing:

- [ ] All tests pass
- [ ] Documentation is updated
- [ ] CHANGELOG.md is updated
- [ ] Version numbers are bumped
- [ ] Security audit passes
- [ ] Performance regressions checked
- [ ] Breaking changes documented

## Recognition

Contributors are recognized in:

- **AUTHORS.md**: List of all contributors
- **CHANGELOG.md**: Attribution for specific changes
- **GitHub releases**: Thanks to contributors

Thank you for contributing to P2P File Converter! Your efforts help make this project better for everyone.
