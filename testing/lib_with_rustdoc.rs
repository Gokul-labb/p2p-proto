//! # P2P File Converter
//!
//! A high-performance, peer-to-peer file converter built with Rust and libp2p.
//! This crate provides a complete solution for converting files between different 
//! formats (text ↔ PDF) using a distributed, decentralized network.
//!
//! ## Quick Start
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! use p2p_file_converter::{P2PFileConverter, AppMode};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create and run the P2P file converter
//!     let mut app = P2PFileConverter::new().await?;
//!     let exit_code = app.run().await?;
//!     
//!     std::process::exit(exit_code);
//! }
//! ```
//!
//! ### File Conversion
//!
//! ```rust,no_run
//! use p2p_file_converter::file_converter::{FileConverter, PdfConfig};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut converter = FileConverter::new();
//!     
//!     // Configure PDF output
//!     let config = PdfConfig {
//!         title: "My Document".to_string(),
//!         font_size: 12,
//!         margins: 20,
//!         ..Default::default()
//!     };
//!     
//!     // Convert text to PDF
//!     let text_content = "Hello, World!\n\nThis is a test document.";
//!     let pdf_data = converter.text_to_pdf(text_content, &config)?;
//!     
//!     // Save PDF file
//!     std::fs::write("output.pdf", pdf_data)?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### P2P File Transfer
//!
//! ```rust,no_run
//! use p2p_file_converter::file_sender::{FileSender, RetryConfig};
//! use libp2p::{Multiaddr, PeerId};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure retry behavior
//!     let retry_config = RetryConfig {
//!         max_attempts: 5,
//!         initial_delay: Duration::from_millis(500),
//!         max_delay: Duration::from_secs(30),
//!         backoff_multiplier: 2.0,
//!         connection_timeout: Duration::from_secs(15),
//!     };
//!     
//!     let mut sender = FileSender::new(Some(retry_config)).await?;
//!     
//!     // Set up progress callback
//!     sender.set_progress_callback(|progress| {
//!         println!("Progress: {:.1}% - {}", 
//!                  progress.percentage(), 
//!                  progress.status_string());
//!     });
//!     
//!     // Send file to peer
//!     let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080/p2p/12D3K...".parse()?;
//!     let peer_id = extract_peer_id(&target_addr)?;
//!     
//!     let transfer_id = sender.send_file(
//!         peer_id,
//!         target_addr,
//!         "document.txt",
//!         Some("pdf".to_string()), // Convert to PDF
//!         false, // Don't return result
//!     ).await?;
//!     
//!     // Wait for completion
//!     let result = sender.wait_for_completion(&transfer_id).await?;
//!     println!("Transfer completed: {} bytes sent", result.bytes_sent);
//!     
//!     Ok(())
//! }
//!
//! fn extract_peer_id(addr: &Multiaddr) -> Result<PeerId, Box<dyn std::error::Error>> {
//!     use libp2p::multiaddr::Protocol;
//!     
//!     for protocol in addr.iter() {
//!         if let Protocol::P2p(peer_id) = protocol {
//!             return Ok(peer_id);
//!         }
//!     }
//!     
//!     Err("No peer ID found in multiaddr".into())
//! }
//! ```
//!
//! ## Architecture Overview
//!
//! The P2P File Converter consists of several key components:
//!
//! - **[`file_converter`]**: Core file conversion functionality (text ↔ PDF)
//! - **[`file_sender`]**: P2P file transfer with retry logic and progress tracking
//! - **[`p2p_stream_handler`]**: Protocol handling for incoming file transfers
//! - **[`error_handling`]**: Comprehensive error management and recovery
//! - **[`main_event_loop`]**: Central event coordination and application lifecycle
//!
//! ## Protocol Design
//!
//! The system uses a custom protocol (`/convert/1.0.0`) built on top of libp2p:
//!
//! 1. **Connection Establishment**: Peers connect using libp2p with Noise encryption
//! 2. **Protocol Negotiation**: Both peers agree on the `/convert/1.0.0` protocol
//! 3. **File Transfer Request**: Sender transmits file metadata and conversion options
//! 4. **Chunked Transfer**: File data is sent in configurable chunks (default 1MB)
//! 5. **Processing**: Receiver validates, converts (if requested), and stores the file
//! 6. **Response**: Receiver sends completion status and optional converted data
//!
//! ## Security Considerations
//!
//! The system implements multiple security layers:
//!
//! - **Transport Security**: All communication encrypted with Noise protocol
//! - **Input Validation**: Comprehensive validation of all inputs and file paths
//! - **Resource Limits**: Configurable limits prevent resource exhaustion attacks
//! - **Path Sanitization**: Prevents directory traversal and other path-based attacks
//! - **Memory Safety**: Rust's memory safety prevents buffer overflows and use-after-free
//!
//! ## Error Handling
//!
//! The crate provides comprehensive error handling through the [`error_handling`] module:
//!
//! ```rust,no_run
//! use p2p_file_converter::error_handling::{P2PError, Result};
//!
//! fn example_operation() -> Result<()> {
//!     // Operations return Result<T, P2PError> for consistent error handling
//!     validate_input("some_input")?;
//!     process_file("file.txt")?;
//!     Ok(())
//! }
//!
//! # fn validate_input(_: &str) -> Result<()> { Ok(()) }
//! # fn process_file(_: &str) -> Result<()> { Ok(()) }
//! ```
//!
//! ## Configuration
//!
//! The system supports extensive configuration through TOML files and environment variables.
//! See the [`config_utilities`] module for details.
//!
//! ## Testing
//!
//! Comprehensive test coverage includes:
//!
//! - **Unit Tests**: Individual component testing
//! - **Integration Tests**: Component interaction testing  
//! - **End-to-End Tests**: Full system testing with multiple peers
//! - **Performance Tests**: Load testing and benchmarking
//!
//! Run tests with: `cargo test`
//!
//! ## Feature Flags
//!
//! - `full` (default): All features enabled
//! - `cli`: Command-line interface support
//! - `conversion`: File conversion functionality
//! - `networking`: P2P networking capabilities

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_code)]

pub mod cli;
pub mod config_utilities;
pub mod error_handling;
pub mod file_converter;
pub mod file_sender;
pub mod p2p_stream_handler;
pub mod main_event_loop;

// Re-export commonly used types for convenience
pub use cli::{CliArgs, AppMode};
pub use config_utilities::{AppConfig, NetworkConfig, FileConfig, ConversionConfig};
pub use error_handling::{P2PError, Result};
pub use file_converter::{FileConverter, FileType, PdfConfig};
pub use file_sender::{FileSender, RetryConfig, SendProgress, TransferStatus, SendResult};
pub use p2p_stream_handler::{
    FileConversionService, FileConversionConfig, P2PFileNode, 
    TransferProgress, FileTransferRequest, FileTransferResponse
};
pub use main_event_loop::{P2PFileConverter, ShutdownReason, AppState};

/// Prelude module for convenient imports
///
/// This module re-exports the most commonly used types and traits,
/// allowing users to quickly get started with a single import:
///
/// ```rust
/// use p2p_file_converter::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        CliArgs, AppMode, AppConfig, P2PError, Result,
        FileConverter, FileType, PdfConfig,
        FileSender, RetryConfig, SendProgress, TransferStatus,
        FileConversionService, P2PFileNode, TransferProgress,
        P2PFileConverter, ShutdownReason,
    };

    pub use libp2p::{PeerId, Multiaddr};
    pub use tokio;
    pub use anyhow::{Context, Result as AnyhowResult};
    pub use tracing::{debug, error, info, warn};
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

/// Protocol version used for peer-to-peer communication
pub const PROTOCOL_VERSION: &str = "/convert/1.0.0";

/// Default chunk size for file transfers (1MB)
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// Maximum supported file size (100MB by default)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Default connection timeout (30 seconds)
pub const DEFAULT_CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Utility functions for common operations
pub mod utils {
    use super::*;
    use libp2p::multiaddr::Protocol;

    /// Extract peer ID from a multiaddr
    ///
    /// # Arguments
    /// 
    /// * `addr` - The multiaddr to extract peer ID from
    ///
    /// # Returns
    ///
    /// Returns the peer ID if found, or an error if no peer ID is present
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use p2p_file_converter::utils::extract_peer_id;
    /// use libp2p::Multiaddr;
    ///
    /// let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWExample".parse()?;
    /// let peer_id = extract_peer_id(&addr)?;
    /// println!("Peer ID: {}", peer_id);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn extract_peer_id(addr: &libp2p::Multiaddr) -> Result<libp2p::PeerId> {
        for protocol in addr.iter() {
            if let Protocol::P2p(peer_id) = protocol {
                return Ok(peer_id);
            }
        }

        Err(crate::error_handling::P2PError::Validation(
            crate::error_handling::ValidationError::MissingComponent {
                addr: addr.to_string(),
                component: "p2p peer ID".to_string(),
            }
        ))
    }

    /// Format file size in human-readable format
    ///
    /// # Arguments
    ///
    /// * `bytes` - Size in bytes
    ///
    /// # Returns
    ///
    /// Human-readable size string (e.g., "1.5 MB", "512 KB")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use p2p_file_converter::utils::format_file_size;
    ///
    /// assert_eq!(format_file_size(1024), "1.0 KB");
    /// assert_eq!(format_file_size(1536), "1.5 KB");
    /// assert_eq!(format_file_size(2048 * 1024), "2.0 MB");
    /// ```
    pub fn format_file_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit = 0;

        while size >= 1024.0 && unit < UNITS.len() - 1 {
            size /= 1024.0;
            unit += 1;
        }

        if unit == 0 {
            format!("{} {}", bytes, UNITS[unit])
        } else {
            format!("{:.1} {}", size, UNITS[unit])
        }
    }

    /// Format duration in human-readable format
    ///
    /// # Arguments
    ///
    /// * `duration` - Duration to format
    ///
    /// # Returns
    ///
    /// Human-readable duration string (e.g., "1m 30s", "2h 15m 45s")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use p2p_file_converter::utils::format_duration;
    /// use std::time::Duration;
    ///
    /// assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
    /// assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    /// ```
    pub fn format_duration(duration: std::time::Duration) -> String {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
    }

    /// Validate that a file extension is supported
    ///
    /// # Arguments
    ///
    /// * `extension` - File extension to check (without the dot)
    /// * `allowed` - List of allowed extensions
    ///
    /// # Returns
    ///
    /// True if the extension is allowed, false otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// use p2p_file_converter::utils::is_extension_allowed;
    ///
    /// let allowed = vec!["txt", "pdf", "md"];
    /// assert!(is_extension_allowed("txt", &allowed));
    /// assert!(is_extension_allowed("PDF", &allowed)); // Case insensitive
    /// assert!(!is_extension_allowed("exe", &allowed));
    /// ```
    pub fn is_extension_allowed(extension: &str, allowed: &[impl AsRef<str>]) -> bool {
        if allowed.is_empty() {
            return true; // If no restrictions, allow all
        }

        allowed.iter().any(|ext| ext.as_ref().eq_ignore_ascii_case(extension))
    }

    /// Generate a unique operation ID
    ///
    /// Creates a unique identifier for tracking operations, useful for
    /// logging and debugging distributed operations.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Prefix for the operation ID
    ///
    /// # Returns
    ///
    /// Unique operation ID string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use p2p_file_converter::utils::generate_operation_id;
    ///
    /// let id = generate_operation_id("transfer");
    /// assert!(id.starts_with("transfer_"));
    /// assert!(id.len() > 10);
    /// ```
    pub fn generate_operation_id(prefix: &str) -> String {
        format!("{}_{}", prefix, uuid::Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constants() {
        assert!(!VERSION.is_empty());
        assert!(!APP_NAME.is_empty());
        assert_eq!(PROTOCOL_VERSION, "/convert/1.0.0");
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_CHUNK_SIZE, 1024 * 1024);
        assert_eq!(DEFAULT_MAX_FILE_SIZE, 100 * 1024 * 1024);
        assert_eq!(DEFAULT_CONNECTION_TIMEOUT.as_secs(), 30);
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(utils::format_file_size(512), "512 B");
        assert_eq!(utils::format_file_size(1536), "1.5 KB");
        assert_eq!(utils::format_file_size(2048 * 1024), "2.0 MB");
    }

    #[test]
    fn test_format_duration() {
        use std::time::Duration;

        assert_eq!(utils::format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(utils::format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(utils::format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    }

    #[test]
    fn test_extension_validation() {
        let allowed = vec!["txt", "pdf", "md"];

        assert!(utils::is_extension_allowed("txt", &allowed));
        assert!(utils::is_extension_allowed("PDF", &allowed)); // Case insensitive
        assert!(!utils::is_extension_allowed("exe", &allowed));

        // Empty allowed list should allow everything
        assert!(utils::is_extension_allowed("anything", &Vec::<String>::new()));
    }

    #[test]
    fn test_operation_id_generation() {
        let id1 = utils::generate_operation_id("test");
        let id2 = utils::generate_operation_id("test");

        assert!(id1.starts_with("test_"));
        assert!(id2.starts_with("test_"));
        assert_ne!(id1, id2); // Should be unique
    }
}
