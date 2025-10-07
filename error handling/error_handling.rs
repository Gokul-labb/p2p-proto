//! Comprehensive Error Handling and Validation System for P2P File Converter
//! 
//! This module provides a complete error handling infrastructure including:
//! - Custom error types for different failure scenarios
//! - Input validation for multiaddrs, file paths, and file types
//! - Timeout handling for network operations and conversion processes
//! - Recovery mechanisms for temporary failures
//! - Resource cleanup in all error scenarios
//! - User-friendly error messages and diagnostics

use anyhow::{Context, Result as AnyhowResult};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Display},
    fs::Metadata,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
    fs,
    time::{timeout, sleep},
    sync::{Mutex, RwLock},
};
use tracing::{debug, error, info, warn};

/// Result type alias for P2P file converter operations
pub type Result<T> = std::result::Result<T, P2PError>;

/// Main error type for P2P file converter operations
#[derive(Error, Debug, Clone)]
pub enum P2PError {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    /// File conversion errors
    #[error("Conversion error: {0}")]
    Conversion(#[from] ConversionError),

    /// File I/O errors
    #[error("File I/O error: {0}")]
    FileIO(#[from] FileIOError),

    /// Input validation errors
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Protocol handling errors
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Timeout errors
    #[error("Timeout error: {0}")]
    Timeout(#[from] TimeoutError),

    /// Resource management errors
    #[error("Resource error: {0}")]
    Resource(#[from] ResourceError),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigurationError),
}

/// Network-specific error types
#[derive(Error, Debug, Clone)]
pub enum NetworkError {
    /// Connection failed
    #[error("Failed to connect to peer {peer_id} at {address}: {reason}")]
    ConnectionFailed {
        peer_id: Option<PeerId>,
        address: Multiaddr,
        reason: String,
    },

    /// Connection timeout
    #[error("Connection timeout after {duration:?} to {address}")]
    ConnectionTimeout {
        address: Multiaddr,
        duration: Duration,
    },

    /// Peer unreachable
    #[error("Peer {peer_id} is unreachable at {address}")]
    PeerUnreachable {
        peer_id: PeerId,
        address: Multiaddr,
    },

    /// Network transport error
    #[error("Transport error: {message}")]
    Transport { message: String },

    /// DNS resolution failed
    #[error("DNS resolution failed for {hostname}: {reason}")]
    DnsResolution { hostname: String, reason: String },

    /// Network interface error
    #[error("Network interface error: {message}")]
    Interface { message: String },

    /// Bandwidth limit exceeded
    #[error("Bandwidth limit exceeded: {current}/{limit} bytes")]
    BandwidthLimit { current: u64, limit: u64 },
}

/// File conversion error types
#[derive(Error, Debug, Clone)]
pub enum ConversionError {
    /// Unsupported file format
    #[error("Unsupported file format: {format} (supported: {supported:?})")]
    UnsupportedFormat {
        format: String,
        supported: Vec<String>,
    },

    /// PDF generation failed
    #[error("PDF generation failed: {reason}")]
    PdfGeneration { reason: String },

    /// Text extraction failed
    #[error("Text extraction from PDF failed: {reason}")]
    TextExtraction { reason: String },

    /// Font loading error
    #[error("Font loading error for '{font_name}': {reason}")]
    FontLoading { font_name: String, reason: String },

    /// Invalid document structure
    #[error("Invalid document structure: {details}")]
    InvalidDocument { details: String },

    /// Conversion timeout
    #[error("Conversion timeout after {duration:?}")]
    ConversionTimeout { duration: Duration },

    /// Memory limit exceeded during conversion
    #[error("Memory limit exceeded during conversion: {used}/{limit} MB")]
    MemoryLimit { used: u64, limit: u64 },
}

/// File I/O error types
#[derive(Error, Debug, Clone)]
pub enum FileIOError {
    /// File not found
    #[error("File not found: '{path}'")]
    NotFound { path: PathBuf },

    /// Permission denied
    #[error("Permission denied accessing '{path}': {operation}")]
    PermissionDenied { path: PathBuf, operation: String },

    /// Disk space insufficient
    #[error("Insufficient disk space: need {needed} bytes, available {available} bytes")]
    InsufficientSpace { needed: u64, available: u64 },

    /// File too large
    #[error("File too large: {size} bytes exceeds maximum {max_size} bytes")]
    FileTooLarge { size: u64, max_size: u64 },

    /// Invalid file path
    #[error("Invalid file path: '{path}' - {reason}")]
    InvalidPath { path: PathBuf, reason: String },

    /// File locked by another process
    #[error("File locked: '{path}' is being used by another process")]
    FileLocked { path: PathBuf },

    /// Directory creation failed
    #[error("Failed to create directory '{path}': {reason}")]
    DirectoryCreation { path: PathBuf, reason: String },

    /// File corruption detected
    #[error("File corruption detected in '{path}': {details}")]
    FileCorruption { path: PathBuf, details: String },
}

/// Input validation error types
#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    /// Invalid multiaddr format
    #[error("Invalid multiaddr '{addr}': {reason}")]
    InvalidMultiaddr { addr: String, reason: String },

    /// Missing required multiaddr component
    #[error("Missing required component in multiaddr '{addr}': {component}")]
    MissingComponent { addr: String, component: String },

    /// Invalid peer ID
    #[error("Invalid peer ID '{peer_id}': {reason}")]
    InvalidPeerId { peer_id: String, reason: String },

    /// Invalid file extension
    #[error("Invalid file extension '{extension}' for file '{filename}' (expected: {expected:?})")]
    InvalidExtension {
        filename: String,
        extension: String,
        expected: Vec<String>,
    },

    /// Invalid configuration value
    #[error("Invalid configuration value for '{key}': {value} ({reason})")]
    InvalidConfigValue {
        key: String,
        value: String,
        reason: String,
    },

    /// Input out of range
    #[error("Value {value} is out of range for '{field}' (min: {min}, max: {max})")]
    OutOfRange {
        field: String,
        value: i64,
        min: i64,
        max: i64,
    },

    /// Required field missing
    #[error("Required field '{field}' is missing")]
    RequiredField { field: String },
}

/// Protocol error types
#[derive(Error, Debug, Clone)]
pub enum ProtocolError {
    /// Protocol negotiation failed
    #[error("Protocol negotiation failed with {peer_id}: expected {expected}, got {actual}")]
    NegotiationFailed {
        peer_id: PeerId,
        expected: String,
        actual: String,
    },

    /// Unsupported protocol version
    #[error("Unsupported protocol version: {version} (supported: {supported:?})")]
    UnsupportedVersion {
        version: String,
        supported: Vec<String>,
    },

    /// Message serialization failed
    #[error("Message serialization failed: {reason}")]
    SerializationFailed { reason: String },

    /// Message deserialization failed
    #[error("Message deserialization failed: {reason}")]
    DeserializationFailed { reason: String },

    /// Stream closed unexpectedly
    #[error("Stream closed unexpectedly with {peer_id}")]
    StreamClosed { peer_id: PeerId },

    /// Protocol state error
    #[error("Invalid protocol state: expected {expected}, current {current}")]
    InvalidState { expected: String, current: String },
}

/// Timeout error types
#[derive(Error, Debug, Clone)]
pub enum TimeoutError {
    /// Operation timeout
    #[error("Operation '{operation}' timed out after {duration:?}")]
    Operation { operation: String, duration: Duration },

    /// Network operation timeout
    #[error("Network operation '{operation}' with {peer_id} timed out after {duration:?}")]
    NetworkOperation {
        operation: String,
        peer_id: PeerId,
        duration: Duration,
    },

    /// File operation timeout
    #[error("File operation '{operation}' on '{path}' timed out after {duration:?}")]
    FileOperation {
        operation: String,
        path: PathBuf,
        duration: Duration,
    },

    /// User input timeout
    #[error("User input timeout after {duration:?}")]
    UserInput { duration: Duration },
}

/// Resource management error types
#[derive(Error, Debug, Clone)]
pub enum ResourceError {
    /// Resource limit exceeded
    #[error("Resource limit exceeded for '{resource}': {current}/{limit}")]
    LimitExceeded {
        resource: String,
        current: u64,
        limit: u64,
    },

    /// Resource cleanup failed
    #[error("Resource cleanup failed for '{resource}': {reason}")]
    CleanupFailed { resource: String, reason: String },

    /// Resource leak detected
    #[error("Resource leak detected: {count} '{resource}' instances not cleaned up")]
    LeakDetected { resource: String, count: usize },

    /// Resource unavailable
    #[error("Resource '{resource}' is unavailable: {reason}")]
    Unavailable { resource: String, reason: String },
}

/// Configuration error types
#[derive(Error, Debug, Clone)]
pub enum ConfigurationError {
    /// Missing configuration file
    #[error("Configuration file not found: '{path}'")]
    FileNotFound { path: PathBuf },

    /// Invalid configuration format
    #[error("Invalid configuration format in '{path}': {reason}")]
    InvalidFormat { path: PathBuf, reason: String },

    /// Missing required configuration
    #[error("Missing required configuration: '{key}'")]
    MissingRequired { key: String },

    /// Configuration validation failed
    #[error("Configuration validation failed for '{section}': {reason}")]
    ValidationFailed { section: String, reason: String },
}

/// Error context for better error reporting
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub component: String,
    pub peer_id: Option<PeerId>,
    pub file_path: Option<PathBuf>,
    pub timestamp: Instant,
    pub additional_info: HashMap<String, String>,
}

impl ErrorContext {
    pub fn new(operation: &str, component: &str) -> Self {
        Self {
            operation: operation.to_string(),
            component: component.to_string(),
            peer_id: None,
            file_path: None,
            timestamp: Instant::now(),
            additional_info: HashMap::new(),
        }
    }

    pub fn with_peer(mut self, peer_id: PeerId) -> Self {
        self.peer_id = Some(peer_id);
        self
    }

    pub fn with_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.file_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_info<K: ToString, V: ToString>(mut self, key: K, value: V) -> Self {
        self.additional_info.insert(key.to_string(), value.to_string());
        self
    }
}

/// Input validation utilities
pub mod validation {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use regex::Regex;

    /// Multiaddr validator
    pub struct MultiAddrValidator {
        required_protocols: Vec<String>,
        allowed_protocols: Vec<String>,
        max_length: usize,
    }

    impl MultiAddrValidator {
        pub fn new() -> Self {
            Self {
                required_protocols: vec!["ip4".to_string(), "tcp".to_string()],
                allowed_protocols: vec![
                    "ip4".to_string(), "ip6".to_string(), "dns".to_string(),
                    "tcp".to_string(), "udp".to_string(), "quic".to_string(),
                    "p2p".to_string(), "tls".to_string(), "ws".to_string(),
                ],
                max_length: 1024,
            }
        }

        pub fn with_required_protocols(mut self, protocols: Vec<String>) -> Self {
            self.required_protocols = protocols;
            self
        }

        pub fn with_max_length(mut self, max_length: usize) -> Self {
            self.max_length = max_length;
            self
        }

        /// Validate multiaddr format and components
        pub fn validate(&self, addr_str: &str) -> Result<Multiaddr> {
            // Check length
            if addr_str.len() > self.max_length {
                return Err(P2PError::Validation(ValidationError::InvalidMultiaddr {
                    addr: addr_str.to_string(),
                    reason: format!("Address too long: {} chars (max: {})", 
                                  addr_str.len(), self.max_length),
                }));
            }

            // Parse multiaddr
            let multiaddr: Multiaddr = addr_str.parse()
                .map_err(|e| P2PError::Validation(ValidationError::InvalidMultiaddr {
                    addr: addr_str.to_string(),
                    reason: format!("Parse error: {}", e),
                }))?;

            // Extract protocols
            let protocols: Vec<String> = multiaddr.iter()
                .map(|p| match p {
                    Protocol::Ip4(_) => "ip4",
                    Protocol::Ip6(_) => "ip6",
                    Protocol::Dns(_) | Protocol::Dns4(_) | Protocol::Dns6(_) => "dns",
                    Protocol::Tcp(_) => "tcp",
                    Protocol::Udp(_) => "udp",
                    Protocol::Quic(_) => "quic",
                    Protocol::P2p(_) => "p2p",
                    Protocol::Tls(_) => "tls",
                    Protocol::Ws(_) => "ws",
                    _ => "unknown",
                })
                .map(|s| s.to_string())
                .collect();

            // Check required protocols
            for required in &self.required_protocols {
                if !protocols.contains(required) {
                    return Err(P2PError::Validation(ValidationError::MissingComponent {
                        addr: addr_str.to_string(),
                        component: required.clone(),
                    }));
                }
            }

            // Check allowed protocols
            for protocol in &protocols {
                if !self.allowed_protocols.contains(protocol) && protocol != "unknown" {
                    return Err(P2PError::Validation(ValidationError::InvalidMultiaddr {
                        addr: addr_str.to_string(),
                        reason: format!("Unsupported protocol: {}", protocol),
                    }));
                }
            }

            // Validate specific protocol components
            self.validate_protocol_components(&multiaddr, addr_str)?;

            Ok(multiaddr)
        }

        fn validate_protocol_components(&self, multiaddr: &Multiaddr, addr_str: &str) -> Result<()> {
            for protocol in multiaddr.iter() {
                match protocol {
                    Protocol::Ip4(ip) => {
                        if ip.is_unspecified() && !addr_str.contains("0.0.0.0") {
                            return Err(P2PError::Validation(ValidationError::InvalidMultiaddr {
                                addr: addr_str.to_string(),
                                reason: "Unspecified IPv4 address".to_string(),
                            }));
                        }
                    }
                    Protocol::Ip6(ip) => {
                        if ip.is_unspecified() && !addr_str.contains("::") {
                            return Err(P2PError::Validation(ValidationError::InvalidMultiaddr {
                                addr: addr_str.to_string(),
                                reason: "Unspecified IPv6 address".to_string(),
                            }));
                        }
                    }
                    Protocol::Tcp(port) => {
                        if *port == 0 {
                            return Err(P2PError::Validation(ValidationError::InvalidMultiaddr {
                                addr: addr_str.to_string(),
                                reason: "Invalid TCP port: 0".to_string(),
                            }));
                        }
                    }
                    Protocol::Udp(port) => {
                        if *port == 0 {
                            return Err(P2PError::Validation(ValidationError::InvalidMultiaddr {
                                addr: addr_str.to_string(),
                                reason: "Invalid UDP port: 0".to_string(),
                            }));
                        }
                    }
                    Protocol::P2p(peer_id) => {
                        // Validate peer ID format
                        if peer_id.to_string().len() < 46 {  // Minimum valid peer ID length
                            return Err(P2PError::Validation(ValidationError::InvalidPeerId {
                                peer_id: peer_id.to_string(),
                                reason: "Peer ID too short".to_string(),
                            }));
                        }
                    }
                    _ => {} // Other protocols are accepted if in allowed list
                }
            }
            Ok(())
        }

        /// Extract peer ID from multiaddr
        pub fn extract_peer_id(&self, multiaddr: &Multiaddr) -> Result<Option<PeerId>> {
            for protocol in multiaddr.iter() {
                if let Protocol::P2p(peer_id) = protocol {
                    return Ok(Some(peer_id));
                }
            }
            Ok(None)
        }
    }

    impl Default for MultiAddrValidator {
        fn default() -> Self {
            Self::new()
        }
    }

    /// File path validator
    pub struct FilePathValidator {
        max_path_length: usize,
        allowed_extensions: Vec<String>,
        forbidden_patterns: Vec<Regex>,
        check_existence: bool,
        check_permissions: bool,
    }

    impl FilePathValidator {
        pub fn new() -> Self {
            Self {
                max_path_length: 4096,
                allowed_extensions: vec![
                    "txt".to_string(), "pdf".to_string(), "md".to_string(),
                    "rtf".to_string(), "doc".to_string(), "docx".to_string(),
                ],
                forbidden_patterns: vec![
                    Regex::new(r"\.\.").unwrap(), // Path traversal
                    Regex::new(r"[<>:"|?*]").unwrap(), // Invalid filename chars
                ],
                check_existence: true,
                check_permissions: true,
            }
        }

        pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
            self.allowed_extensions = extensions;
            self
        }

        pub fn skip_existence_check(mut self) -> Self {
            self.check_existence = false;
            self
        }

        /// Validate file path format and accessibility
        pub async fn validate<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
            let path = path.as_ref();
            let path_str = path.to_string_lossy();

            // Check path length
            if path_str.len() > self.max_path_length {
                return Err(P2PError::Validation(ValidationError::InvalidConfigValue {
                    key: "file_path".to_string(),
                    value: path_str.to_string(),
                    reason: format!("Path too long: {} chars (max: {})", 
                                  path_str.len(), self.max_path_length),
                }));
            }

            // Check forbidden patterns
            for pattern in &self.forbidden_patterns {
                if pattern.is_match(&path_str) {
                    return Err(P2PError::FileIO(FileIOError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: format!("Path contains forbidden pattern: {}", pattern.as_str()),
                    }));
                }
            }

            // Check file extension
            if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                if !self.allowed_extensions.is_empty() && 
                   !self.allowed_extensions.contains(&extension.to_lowercase()) {
                    return Err(P2PError::Validation(ValidationError::InvalidExtension {
                        filename: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        extension: extension.to_string(),
                        expected: self.allowed_extensions.clone(),
                    }));
                }
            }

            // Check file existence and permissions
            if self.check_existence {
                let metadata = fs::metadata(path).await
                    .map_err(|e| match e.kind() {
                        ErrorKind::NotFound => P2PError::FileIO(FileIOError::NotFound {
                            path: path.to_path_buf(),
                        }),
                        ErrorKind::PermissionDenied => P2PError::FileIO(FileIOError::PermissionDenied {
                            path: path.to_path_buf(),
                            operation: "read metadata".to_string(),
                        }),
                        _ => P2PError::FileIO(FileIOError::InvalidPath {
                            path: path.to_path_buf(),
                            reason: e.to_string(),
                        }),
                    })?;

                // Check if it's a file (not directory)
                if !metadata.is_file() {
                    return Err(P2PError::FileIO(FileIOError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: "Path is not a regular file".to_string(),
                    }));
                }

                // Check file permissions
                if self.check_permissions {
                    self.validate_permissions(path, &metadata).await?;
                }
            }

            Ok(path.to_path_buf())
        }

        async fn validate_permissions(&self, path: &Path, metadata: &Metadata) -> Result<()> {
            // Check if file is readable
            match fs::File::open(path).await {
                Ok(_) => {},
                Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                    return Err(P2PError::FileIO(FileIOError::PermissionDenied {
                        path: path.to_path_buf(),
                        operation: "read".to_string(),
                    }));
                }
                Err(e) => {
                    return Err(P2PError::FileIO(FileIOError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: format!("Cannot access file: {}", e),
                    }));
                }
            }

            Ok(())
        }

        /// Validate file size limits
        pub async fn validate_size<P: AsRef<Path>>(&self, path: P, max_size: u64) -> Result<u64> {
            let path = path.as_ref();
            let metadata = fs::metadata(path).await
                .map_err(|e| P2PError::FileIO(FileIOError::NotFound {
                    path: path.to_path_buf(),
                }))?;

            let size = metadata.len();
            if size > max_size {
                return Err(P2PError::FileIO(FileIOError::FileTooLarge {
                    size,
                    max_size,
                }));
            }

            Ok(size)
        }
    }

    impl Default for FilePathValidator {
        fn default() -> Self {
            Self::new()
        }
    }

    /// File type validator using magic numbers and heuristics
    pub struct FileTypeValidator {
        strict_mode: bool,
        magic_signatures: HashMap<Vec<u8>, String>,
    }

    impl FileTypeValidator {
        pub fn new() -> Self {
            let mut magic_signatures = HashMap::new();

            // PDF signatures
            magic_signatures.insert(vec![0x25, 0x50, 0x44, 0x46], "pdf".to_string()); // %PDF

            // Text file indicators (UTF-8 BOM)
            magic_signatures.insert(vec![0xEF, 0xBB, 0xBF], "txt".to_string());

            // Add more magic numbers as needed

            Self {
                strict_mode: false,
                magic_signatures,
            }
        }

        pub fn strict(mut self) -> Self {
            self.strict_mode = true;
            self
        }

        /// Validate file type based on content and extension
        pub async fn validate<P: AsRef<Path>>(&self, path: P, expected_type: Option<&str>) -> Result<String> {
            let path = path.as_ref();

            // Read file header for magic number detection
            let header = self.read_file_header(path).await?;

            // Detect type from magic numbers
            let detected_type = self.detect_from_header(&header);

            // If no magic number match, use heuristics
            let file_type = if detected_type.is_empty() {
                self.detect_from_heuristics(&header, path).await?
            } else {
                detected_type
            };

            // Validate against expected type if provided
            if let Some(expected) = expected_type {
                if file_type != expected && self.strict_mode {
                    return Err(P2PError::Conversion(ConversionError::UnsupportedFormat {
                        format: file_type,
                        supported: vec![expected.to_string()],
                    }));
                }
            }

            Ok(file_type)
        }

        async fn read_file_header<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>> {
            let mut file = fs::File::open(path.as_ref()).await
                .map_err(|e| P2PError::FileIO(FileIOError::NotFound {
                    path: path.as_ref().to_path_buf(),
                }))?;

            let mut header = vec![0u8; 1024]; // Read first 1KB
            let bytes_read = tokio::io::AsyncReadExt::read(&mut file, &mut header).await
                .map_err(|e| P2PError::FileIO(FileIOError::InvalidPath {
                    path: path.as_ref().to_path_buf(),
                    reason: format!("Failed to read file header: {}", e),
                }))?;

            header.truncate(bytes_read);
            Ok(header)
        }

        fn detect_from_header(&self, header: &[u8]) -> String {
            for (signature, file_type) in &self.magic_signatures {
                if header.len() >= signature.len() && header.starts_with(signature) {
                    return file_type.clone();
                }
            }
            String::new()
        }

        async fn detect_from_heuristics(&self, header: &[u8], path: &Path) -> Result<String> {
            // Check for text content
            if self.is_likely_text(header) {
                return Ok("txt".to_string());
            }

            // Fall back to file extension
            if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                return Ok(extension.to_lowercase());
            }

            // Unknown type
            Ok("unknown".to_string())
        }

        fn is_likely_text(&self, data: &[u8]) -> bool {
            if data.is_empty() {
                return false;
            }

            // Check for null bytes (strong indicator of binary)
            if data.contains(&0) {
                return false;
            }

            // Check UTF-8 validity
            if let Ok(text) = std::str::from_utf8(data) {
                let printable_count = text.chars()
                    .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                    .count();

                let total_chars = text.chars().count();
                if total_chars > 0 {
                    let printable_ratio = printable_count as f64 / total_chars as f64;
                    return printable_ratio > 0.7; // 70% printable threshold
                }
            }

            false
        }
    }

    impl Default for FileTypeValidator {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Timeout handling utilities
pub mod timeouts {
    use super::*;

    /// Timeout manager for various operations
    pub struct TimeoutManager {
        default_network_timeout: Duration,
        default_file_timeout: Duration,
        default_conversion_timeout: Duration,
        max_retries: usize,
    }

    impl TimeoutManager {
        pub fn new() -> Self {
            Self {
                default_network_timeout: Duration::from_secs(30),
                default_file_timeout: Duration::from_secs(60),
                default_conversion_timeout: Duration::from_secs(120),
                max_retries: 3,
            }
        }

        pub fn with_network_timeout(mut self, timeout: Duration) -> Self {
            self.default_network_timeout = timeout;
            self
        }

        pub fn with_file_timeout(mut self, timeout: Duration) -> Self {
            self.default_file_timeout = timeout;
            self
        }

        pub fn with_conversion_timeout(mut self, timeout: Duration) -> Self {
            self.default_conversion_timeout = timeout;
            self
        }

        /// Execute network operation with timeout and retries
        pub async fn execute_network_operation<F, Fut, T>(
            &self,
            operation_name: &str,
            peer_id: Option<PeerId>,
            operation: F,
        ) -> Result<T>
        where
            F: Fn() -> Fut,
            Fut: std::future::Future<Output = Result<T>>,
        {
            let mut last_error = None;

            for attempt in 1..=self.max_retries {
                debug!("Network operation '{}' attempt {}/{}", operation_name, attempt, self.max_retries);

                match timeout(self.default_network_timeout, operation()).await {
                    Ok(Ok(result)) => {
                        if attempt > 1 {
                            info!("Network operation '{}' succeeded on attempt {}", operation_name, attempt);
                        }
                        return Ok(result);
                    }
                    Ok(Err(e)) => {
                        warn!("Network operation '{}' failed on attempt {}: {}", operation_name, attempt, e);
                        last_error = Some(e);
                    }
                    Err(_) => {
                        let timeout_error = P2PError::Timeout(TimeoutError::NetworkOperation {
                            operation: operation_name.to_string(),
                            peer_id: peer_id.unwrap_or_else(|| PeerId::random()),
                            duration: self.default_network_timeout,
                        });
                        warn!("Network operation '{}' timed out on attempt {}", operation_name, attempt);
                        last_error = Some(timeout_error);
                    }
                }

                // Wait before retry (exponential backoff)
                if attempt < self.max_retries {
                    let delay = Duration::from_millis(100 * (2_u64.pow(attempt as u32 - 1)));
                    debug!("Retrying network operation '{}' in {:?}", operation_name, delay);
                    sleep(delay).await;
                }
            }

            Err(last_error.unwrap_or_else(|| P2PError::Network(NetworkError::Transport {
                message: format!("Operation '{}' failed after {} attempts", operation_name, self.max_retries),
            })))
        }

        /// Execute file operation with timeout
        pub async fn execute_file_operation<F, Fut, T, P>(
            &self,
            operation_name: &str,
            path: P,
            operation: F,
        ) -> Result<T>
        where
            F: FnOnce() -> Fut,
            Fut: std::future::Future<Output = Result<T>>,
            P: AsRef<Path>,
        {
            let path_buf = path.as_ref().to_path_buf();

            match timeout(self.default_file_timeout, operation()).await {
                Ok(result) => result,
                Err(_) => Err(P2PError::Timeout(TimeoutError::FileOperation {
                    operation: operation_name.to_string(),
                    path: path_buf,
                    duration: self.default_file_timeout,
                })),
            }
        }

        /// Execute conversion operation with timeout
        pub async fn execute_conversion_operation<F, Fut, T>(
            &self,
            operation_name: &str,
            operation: F,
        ) -> Result<T>
        where
            F: FnOnce() -> Fut,
            Fut: std::future::Future<Output = Result<T>>,
        {
            match timeout(self.default_conversion_timeout, operation()).await {
                Ok(result) => result,
                Err(_) => Err(P2PError::Timeout(TimeoutError::Operation {
                    operation: operation_name.to_string(),
                    duration: self.default_conversion_timeout,
                })),
            }
        }
    }

    impl Default for TimeoutManager {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Recovery mechanisms for handling failures
pub mod recovery {
    use super::*;

    /// Recovery strategy for different types of failures
    #[derive(Debug, Clone)]
    pub enum RecoveryStrategy {
        /// Retry with exponential backoff
        RetryWithBackoff {
            max_attempts: usize,
            initial_delay: Duration,
            max_delay: Duration,
        },
        /// Fallback to alternative method
        Fallback { alternative: String },
        /// Skip and continue
        Skip,
        /// Fail immediately
        Fail,
    }

    /// Recovery manager
    pub struct RecoveryManager {
        strategies: HashMap<String, RecoveryStrategy>,
        active_recoveries: Arc<RwLock<HashMap<String, RecoveryState>>>,
    }

    #[derive(Debug, Clone)]
    struct RecoveryState {
        attempts: usize,
        last_attempt: Instant,
        last_error: String,
    }

    impl RecoveryManager {
        pub fn new() -> Self {
            let mut strategies = HashMap::new();

            // Default strategies
            strategies.insert("network_connection".to_string(), RecoveryStrategy::RetryWithBackoff {
                max_attempts: 5,
                initial_delay: Duration::from_millis(500),
                max_delay: Duration::from_secs(30),
            });

            strategies.insert("file_conversion".to_string(), RecoveryStrategy::RetryWithBackoff {
                max_attempts: 3,
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(10),
            });

            strategies.insert("file_io".to_string(), RecoveryStrategy::RetryWithBackoff {
                max_attempts: 2,
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(5),
            });

            Self {
                strategies,
                active_recoveries: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        /// Attempt recovery for a failed operation
        pub async fn attempt_recovery<F, Fut, T>(
            &self,
            operation_id: &str,
            error: &P2PError,
            operation: F,
        ) -> Result<T>
        where
            F: Fn() -> Fut,
            Fut: std::future::Future<Output = Result<T>>,
        {
            let strategy = self.determine_strategy(error);

            match strategy {
                RecoveryStrategy::RetryWithBackoff { max_attempts, initial_delay, max_delay } => {
                    self.retry_with_backoff(operation_id, max_attempts, initial_delay, max_delay, operation).await
                }
                RecoveryStrategy::Fallback { alternative } => {
                    warn!("Attempting fallback strategy: {}", alternative);
                    // Implement fallback logic based on alternative strategy
                    Err(error.clone())
                }
                RecoveryStrategy::Skip => {
                    warn!("Skipping failed operation: {}", operation_id);
                    Err(error.clone())
                }
                RecoveryStrategy::Fail => {
                    error!("No recovery strategy for operation: {}", operation_id);
                    Err(error.clone())
                }
            }
        }

        async fn retry_with_backoff<F, Fut, T>(
            &self,
            operation_id: &str,
            max_attempts: usize,
            initial_delay: Duration,
            max_delay: Duration,
            operation: F,
        ) -> Result<T>
        where
            F: Fn() -> Fut,
            Fut: std::future::Future<Output = Result<T>>,
        {
            let mut current_delay = initial_delay;
            let mut last_error = None;

            for attempt in 1..=max_attempts {
                debug!("Recovery attempt {}/{} for operation: {}", attempt, max_attempts, operation_id);

                match operation().await {
                    Ok(result) => {
                        if attempt > 1 {
                            info!("Operation {} recovered successfully on attempt {}", operation_id, attempt);
                            // Clean up recovery state
                            self.active_recoveries.write().await.remove(operation_id);
                        }
                        return Ok(result);
                    }
                    Err(e) => {
                        last_error = Some(e);

                        // Update recovery state
                        let recovery_state = RecoveryState {
                            attempts: attempt,
                            last_attempt: Instant::now(),
                            last_error: last_error.as_ref().unwrap().to_string(),
                        };
                        self.active_recoveries.write().await.insert(operation_id.to_string(), recovery_state);

                        if attempt < max_attempts {
                            warn!("Operation {} failed on attempt {}, retrying in {:?}", 
                                  operation_id, attempt, current_delay);
                            sleep(current_delay).await;

                            // Exponential backoff
                            current_delay = std::cmp::min(
                                Duration::from_millis((current_delay.as_millis() as f64 * 1.5) as u64),
                                max_delay
                            );
                        }
                    }
                }
            }

            error!("Operation {} failed after {} recovery attempts", operation_id, max_attempts);
            Err(last_error.unwrap())
        }

        fn determine_strategy(&self, error: &P2PError) -> RecoveryStrategy {
            match error {
                P2PError::Network(NetworkError::ConnectionTimeout { .. }) => {
                    self.strategies.get("network_connection")
                        .cloned()
                        .unwrap_or(RecoveryStrategy::Fail)
                }
                P2PError::Network(NetworkError::ConnectionFailed { .. }) => {
                    self.strategies.get("network_connection")
                        .cloned()
                        .unwrap_or(RecoveryStrategy::Fail)
                }
                P2PError::Conversion(_) => {
                    self.strategies.get("file_conversion")
                        .cloned()
                        .unwrap_or(RecoveryStrategy::Fail)
                }
                P2PError::FileIO(_) => {
                    self.strategies.get("file_io")
                        .cloned()
                        .unwrap_or(RecoveryStrategy::Fail)
                }
                _ => RecoveryStrategy::Fail,
            }
        }

        /// Get current recovery statistics
        pub async fn get_recovery_stats(&self) -> HashMap<String, RecoveryState> {
            self.active_recoveries.read().await.clone()
        }
    }

    impl Default for RecoveryManager {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Resource cleanup utilities with RAII patterns
pub mod cleanup {
    use super::*;
    use std::sync::Arc;

    /// RAII guard for automatic resource cleanup
    pub struct ResourceGuard<T> {
        resource: Option<T>,
        cleanup_fn: Box<dyn FnOnce(T) + Send + 'static>,
        name: String,
    }

    impl<T> ResourceGuard<T> {
        pub fn new<F>(resource: T, name: String, cleanup_fn: F) -> Self
        where
            F: FnOnce(T) + Send + 'static,
        {
            Self {
                resource: Some(resource),
                cleanup_fn: Box::new(cleanup_fn),
                name,
            }
        }

        /// Take ownership of the resource (prevents cleanup)
        pub fn take(mut self) -> T {
            self.resource.take().expect("Resource already taken")
        }

        /// Get reference to the resource
        pub fn get(&self) -> &T {
            self.resource.as_ref().expect("Resource already taken")
        }

        /// Get mutable reference to the resource
        pub fn get_mut(&mut self) -> &mut T {
            self.resource.as_mut().expect("Resource already taken")
        }
    }

    impl<T> Drop for ResourceGuard<T> {
        fn drop(&mut self) {
            if let Some(resource) = self.resource.take() {
                debug!("Cleaning up resource: {}", self.name);
                (self.cleanup_fn)(resource);
            }
        }
    }

    /// Cleanup manager for tracking and managing resources
    pub struct CleanupManager {
        active_resources: Arc<RwLock<HashMap<String, String>>>,
        cleanup_callbacks: Arc<RwLock<HashMap<String, Box<dyn Fn() + Send + Sync>>>>,
    }

    impl CleanupManager {
        pub fn new() -> Self {
            Self {
                active_resources: Arc::new(RwLock::new(HashMap::new())),
                cleanup_callbacks: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        /// Register a resource for tracking
        pub async fn register_resource(&self, id: String, description: String) {
            self.active_resources.write().await.insert(id, description);
        }

        /// Unregister a resource
        pub async fn unregister_resource(&self, id: &str) {
            self.active_resources.write().await.remove(id);
        }

        /// Register cleanup callback for a resource
        pub async fn register_cleanup<F>(&self, id: String, callback: F)
        where
            F: Fn() + Send + Sync + 'static,
        {
            self.cleanup_callbacks.write().await.insert(id, Box::new(callback));
        }

        /// Execute cleanup for a specific resource
        pub async fn cleanup_resource(&self, id: &str) -> Result<()> {
            if let Some(callback) = self.cleanup_callbacks.write().await.remove(id) {
                callback();
                self.unregister_resource(id).await;
                debug!("Cleaned up resource: {}", id);
                Ok(())
            } else {
                Err(P2PError::Resource(ResourceError::Unavailable {
                    resource: id.to_string(),
                    reason: "No cleanup callback registered".to_string(),
                }))
            }
        }

        /// Clean up all registered resources
        pub async fn cleanup_all(&self) -> Vec<String> {
            let mut failed_cleanups = Vec::new();
            let callbacks = self.cleanup_callbacks.write().await.drain().collect::<Vec<_>>();

            for (id, callback) in callbacks {
                callback();
                debug!("Cleaned up resource: {}", id);
            }

            self.active_resources.write().await.clear();
            failed_cleanups
        }

        /// Get list of active resources
        pub async fn get_active_resources(&self) -> HashMap<String, String> {
            self.active_resources.read().await.clone()
        }

        /// Check for resource leaks
        pub async fn check_leaks(&self) -> Vec<String> {
            let active = self.active_resources.read().await;
            if !active.is_empty() {
                warn!("Potential resource leaks detected: {} active resources", active.len());
                active.keys().cloned().collect()
            } else {
                Vec::new()
            }
        }
    }

    impl Default for CleanupManager {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// User-friendly error formatting and display
pub mod display {
    use super::*;

    /// Error formatter for user-friendly messages
    pub struct ErrorFormatter {
        show_technical_details: bool,
        show_recovery_suggestions: bool,
    }

    impl ErrorFormatter {
        pub fn new() -> Self {
            Self {
                show_technical_details: false,
                show_recovery_suggestions: true,
            }
        }

        pub fn technical(mut self) -> Self {
            self.show_technical_details = true;
            self
        }

        pub fn simple(mut self) -> Self {
            self.show_technical_details = false;
            self.show_recovery_suggestions = false;
            self
        }

        /// Format error for end user display
        pub fn format_error(&self, error: &P2PError) -> String {
            let mut message = String::new();

            // Main error message
            message.push_str(&self.format_main_message(error));

            // Technical details if enabled
            if self.show_technical_details {
                message.push_str(&format!("\n\nTechnical details: {}", error));
            }

            // Recovery suggestions if enabled
            if self.show_recovery_suggestions {
                if let Some(suggestion) = self.get_recovery_suggestion(error) {
                    message.push_str(&format!("\n\nSuggestion: {}", suggestion));
                }
            }

            message
        }

        fn format_main_message(&self, error: &P2PError) -> String {
            match error {
                P2PError::Network(NetworkError::ConnectionFailed { address, reason, .. }) => {
                    format!("Unable to connect to peer at {}. {}", address, reason)
                }
                P2PError::Network(NetworkError::ConnectionTimeout { address, duration }) => {
                    format!("Connection to {} timed out after {:?}", address, duration)
                }
                P2PError::FileIO(FileIOError::NotFound { path }) => {
                    format!("File not found: {}", path.display())
                }
                P2PError::FileIO(FileIOError::PermissionDenied { path, operation }) => {
                    format!("Permission denied: cannot {} file {}", operation, path.display())
                }
                P2PError::FileIO(FileIOError::FileTooLarge { size, max_size }) => {
                    format!("File is too large: {:.1} MB exceeds maximum {:.1} MB", 
                           *size as f64 / 1_000_000.0, *max_size as f64 / 1_000_000.0)
                }
                P2PError::Validation(ValidationError::InvalidMultiaddr { addr, reason }) => {
                    format!("Invalid peer address '{}': {}", addr, reason)
                }
                P2PError::Conversion(ConversionError::UnsupportedFormat { format, supported }) => {
                    format!("Unsupported file format '{}'. Supported formats: {}", 
                           format, supported.join(", "))
                }
                P2PError::Timeout(TimeoutError::Operation { operation, duration }) => {
                    format!("Operation '{}' timed out after {:?}", operation, duration)
                }
                _ => error.to_string(),
            }
        }

        fn get_recovery_suggestion(&self, error: &P2PError) -> Option<String> {
            match error {
                P2PError::Network(NetworkError::ConnectionFailed { .. }) => {
                    Some("Check the peer address and ensure the peer is running and accessible".to_string())
                }
                P2PError::Network(NetworkError::ConnectionTimeout { .. }) => {
                    Some("Check your network connection and try again. The peer may be overloaded".to_string())
                }
                P2PError::FileIO(FileIOError::NotFound { .. }) => {
                    Some("Verify the file path is correct and the file exists".to_string())
                }
                P2PError::FileIO(FileIOError::PermissionDenied { .. }) => {
                    Some("Check file permissions or run as administrator/root if necessary".to_string())
                }
                P2PError::FileIO(FileIOError::FileTooLarge { .. }) => {
                    Some("Try splitting the file into smaller parts or increase the size limit".to_string())
                }
                P2PError::Validation(ValidationError::InvalidMultiaddr { .. }) => {
                    Some("Ensure the address follows the format: /ip4/127.0.0.1/tcp/8080/p2p/12D3K...".to_string())
                }
                P2PError::Conversion(ConversionError::UnsupportedFormat { .. }) => {
                    Some("Convert the file to a supported format first, or check file extension".to_string())
                }
                _ => None,
            }
        }
    }

    impl Default for ErrorFormatter {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};
    use std::io::Write;

    #[tokio::test]
    async fn test_multiaddr_validation() {
        let validator = validation::MultiAddrValidator::new();

        // Valid multiaddr
        let valid_addr = "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ";
        assert!(validator.validate(valid_addr).is_ok());

        // Invalid multiaddr (missing TCP)
        let invalid_addr = "/ip4/127.0.0.1/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ";
        assert!(validator.validate(invalid_addr).is_err());
    }

    #[tokio::test]
    async fn test_file_path_validation() {
        let validator = validation::FilePathValidator::new().skip_existence_check();

        // Valid file path
        assert!(validator.validate("test.txt").await.is_ok());

        // Invalid extension
        assert!(validator.validate("test.exe").await.is_err());

        // Path traversal attempt
        assert!(validator.validate("../etc/passwd").await.is_err());
    }

    #[tokio::test]
    async fn test_timeout_manager() {
        let timeout_manager = timeouts::TimeoutManager::new()
            .with_network_timeout(Duration::from_millis(100));

        // Operation that should timeout
        let result = timeout_manager.execute_network_operation(
            "test_operation",
            None,
            || async {
                sleep(Duration::from_millis(200)).await;
                Ok::<(), P2PError>(())
            }
        ).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), P2PError::Timeout(_)));
    }

    #[tokio::test]
    async fn test_resource_guard() {
        use cleanup::ResourceGuard;

        let cleanup_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cleanup_called_clone = cleanup_called.clone();

        {
            let _guard = ResourceGuard::new(
                "test_resource".to_string(),
                "test".to_string(),
                move |_| {
                    cleanup_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            );
        }

        assert!(cleanup_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_error_formatting() {
        let formatter = display::ErrorFormatter::new();

        let error = P2PError::FileIO(FileIOError::NotFound {
            path: PathBuf::from("test.txt"),
        });

        let formatted = formatter.format_error(&error);
        assert!(formatted.contains("File not found"));
        assert!(formatted.contains("Suggestion"));
    }

    #[tokio::test]
    async fn test_file_type_validation() {
        let validator = validation::FileTypeValidator::new();

        // Create a temporary PDF file with proper header
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"%PDF-1.4\ntest content").unwrap();

        let file_type = validator.validate(temp_file.path(), Some("pdf")).await.unwrap();
        assert_eq!(file_type, "pdf");

        // Create a text file
        let mut text_file = NamedTempFile::new().unwrap();
        text_file.write_all(b"This is plain text content").unwrap();

        let file_type = validator.validate(text_file.path(), Some("txt")).await.unwrap();
        assert_eq!(file_type, "txt");
    }

    #[tokio::test]
    async fn test_recovery_manager() {
        let recovery_manager = recovery::RecoveryManager::new();

        let mut attempt_count = 0;
        let result = recovery_manager.attempt_recovery(
            "test_operation",
            &P2PError::Network(NetworkError::Transport {
                message: "Test error".to_string(),
            }),
            || async {
                attempt_count += 1;
                if attempt_count < 3 {
                    Err(P2PError::Network(NetworkError::Transport {
                        message: "Still failing".to_string(),
                    }))
                } else {
                    Ok(())
                }
            }
        ).await;

        assert!(result.is_ok());
        assert_eq!(attempt_count, 3);
    }
}
