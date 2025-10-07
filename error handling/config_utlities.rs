//! Configuration and utility modules for enhanced error handling

use crate::error_handling::{P2PError, Result, ConfigurationError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tracing::{info, warn};

/// Application configuration with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Network configuration
    pub network: NetworkConfig,
    /// File handling configuration
    pub files: FileConfig,
    /// Conversion configuration
    pub conversion: ConversionConfig,
    /// Error handling configuration
    pub error_handling: ErrorHandlingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Default connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retry_attempts: usize,
    /// Enable connection keep-alive
    #[serde(default = "default_true")]
    pub keep_alive: bool,
    /// Bandwidth limit in bytes per second (0 = unlimited)
    #[serde(default)]
    pub bandwidth_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    /// Maximum file size in bytes
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Allowed file extensions
    #[serde(default = "default_extensions")]
    pub allowed_extensions: Vec<String>,
    /// Default output directory
    #[serde(default = "default_output_dir")]
    pub output_directory: PathBuf,
    /// Enable file integrity checking
    #[serde(default = "default_true")]
    pub integrity_check: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionConfig {
    /// Conversion timeout in seconds
    #[serde(default = "default_conversion_timeout")]
    pub timeout_secs: u64,
    /// Enable parallel conversions
    #[serde(default = "default_true")]
    pub parallel_processing: bool,
    /// Maximum memory usage for conversions in MB
    #[serde(default = "default_max_memory")]
    pub max_memory_mb: u64,
    /// Font directory path
    #[serde(default = "default_font_dir")]
    pub font_directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    /// Enable verbose error messages
    #[serde(default)]
    pub verbose_errors: bool,
    /// Log errors to file
    #[serde(default = "default_true")]
    pub log_errors: bool,
    /// Error log file path
    #[serde(default = "default_error_log")]
    pub error_log_path: PathBuf,
    /// Enable recovery mechanisms
    #[serde(default = "default_true")]
    pub enable_recovery: bool,
}

// Default value functions
fn default_connection_timeout() -> u64 { 30 }
fn default_max_retries() -> usize { 5 }
fn default_true() -> bool { true }
fn default_max_file_size() -> u64 { 100 * 1024 * 1024 } // 100MB
fn default_extensions() -> Vec<String> { 
    vec!["txt".to_string(), "pdf".to_string(), "md".to_string()] 
}
fn default_output_dir() -> PathBuf { PathBuf::from("./output") }
fn default_conversion_timeout() -> u64 { 300 } // 5 minutes
fn default_max_memory() -> u64 { 1024 } // 1GB
fn default_font_dir() -> PathBuf { PathBuf::from("./fonts") }
fn default_error_log() -> PathBuf { PathBuf::from("./error.log") }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig {
                connection_timeout_secs: default_connection_timeout(),
                max_retry_attempts: default_max_retries(),
                keep_alive: default_true(),
                bandwidth_limit: 0,
            },
            files: FileConfig {
                max_file_size: default_max_file_size(),
                allowed_extensions: default_extensions(),
                output_directory: default_output_dir(),
                integrity_check: default_true(),
            },
            conversion: ConversionConfig {
                timeout_secs: default_conversion_timeout(),
                parallel_processing: default_true(),
                max_memory_mb: default_max_memory(),
                font_directory: default_font_dir(),
            },
            error_handling: ErrorHandlingConfig {
                verbose_errors: false,
                log_errors: default_true(),
                error_log_path: default_error_log(),
                enable_recovery: default_true(),
            },
        }
    }
}

impl AppConfig {
    /// Load configuration from file with validation
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        let content = fs::read_to_string(path).await
            .map_err(|e| P2PError::Configuration(ConfigurationError::FileNotFound {
                path: path.to_path_buf(),
            }))?;

        let config: AppConfig = toml::from_str(&content)
            .map_err(|e| P2PError::Configuration(ConfigurationError::InvalidFormat {
                path: path.to_path_buf(),
                reason: e.to_string(),
            }))?;

        config.validate().await?;
        info!("✅ Configuration loaded from: {}", path.display());
        Ok(config)
    }

    /// Save configuration to file
    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        let content = toml::to_string_pretty(self)
            .map_err(|e| P2PError::Configuration(ConfigurationError::InvalidFormat {
                path: path.to_path_buf(),
                reason: format!("Serialization error: {}", e),
            }))?;

        fs::write(path, content).await
            .map_err(|e| P2PError::FileIO(crate::error_handling::FileIOError::PermissionDenied {
                path: path.to_path_buf(),
                operation: "write config".to_string(),
            }))?;

        info!("💾 Configuration saved to: {}", path.display());
        Ok(())
    }

    /// Validate configuration values
    pub async fn validate(&self) -> Result<()> {
        // Validate network settings
        if self.network.connection_timeout_secs == 0 {
            return Err(P2PError::Configuration(ConfigurationError::ValidationFailed {
                section: "network".to_string(),
                reason: "Connection timeout cannot be zero".to_string(),
            }));
        }

        if self.network.max_retry_attempts == 0 {
            return Err(P2PError::Configuration(ConfigurationError::ValidationFailed {
                section: "network".to_string(),
                reason: "Max retry attempts cannot be zero".to_string(),
            }));
        }

        // Validate file settings
        if self.files.max_file_size == 0 {
            return Err(P2PError::Configuration(ConfigurationError::ValidationFailed {
                section: "files".to_string(),
                reason: "Max file size cannot be zero".to_string(),
            }));
        }

        if self.files.allowed_extensions.is_empty() {
            warn!("⚠️  No file extensions specified, all files will be allowed");
        }

        // Validate output directory
        if !self.files.output_directory.exists() {
            fs::create_dir_all(&self.files.output_directory).await
                .map_err(|e| P2PError::FileIO(crate::error_handling::FileIOError::DirectoryCreation {
                    path: self.files.output_directory.clone(),
                    reason: e.to_string(),
                }))?;
            info!("📁 Created output directory: {}", self.files.output_directory.display());
        }

        // Validate conversion settings
        if self.conversion.timeout_secs == 0 {
            return Err(P2PError::Configuration(ConfigurationError::ValidationFailed {
                section: "conversion".to_string(),
                reason: "Conversion timeout cannot be zero".to_string(),
            }));
        }

        Ok(())
    }

    /// Get network timeout as Duration
    pub fn network_timeout(&self) -> Duration {
        Duration::from_secs(self.network.connection_timeout_secs)
    }

    /// Get conversion timeout as Duration
    pub fn conversion_timeout(&self) -> Duration {
        Duration::from_secs(self.conversion.timeout_secs)
    }

    /// Check if file extension is allowed
    pub fn is_extension_allowed(&self, extension: &str) -> bool {
        self.files.allowed_extensions.is_empty() ||
        self.files.allowed_extensions.iter().any(|ext| ext.eq_ignore_ascii_case(extension))
    }
}

/// Utility functions for common operations
pub mod utils {
    use super::*;
    use std::fs::Metadata;

    /// Format file size in human-readable format
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
    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
    }

    /// Check available disk space
    pub async fn check_disk_space<P: AsRef<Path>>(path: P) -> Result<u64> {
        // Platform-specific disk space checking would go here
        // For now, return a large value
        Ok(u64::MAX)
    }

    /// Generate unique operation ID
    pub fn generate_operation_id(prefix: &str) -> String {
        format!("{}_{}", prefix, chrono::Utc::now().timestamp_nanos())
    }

    /// Sanitize filename for cross-platform compatibility
    pub fn sanitize_filename(filename: &str) -> String {
        filename
            .chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
                '/' | '\' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect()
    }

    /// Extract file metadata safely
    pub async fn safe_metadata<P: AsRef<Path>>(path: P) -> Result<Metadata> {
        let path = path.as_ref();
        fs::metadata(path).await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    P2PError::FileIO(crate::error_handling::FileIOError::NotFound {
                        path: path.to_path_buf(),
                    })
                }
                std::io::ErrorKind::PermissionDenied => {
                    P2PError::FileIO(crate::error_handling::FileIOError::PermissionDenied {
                        path: path.to_path_buf(),
                        operation: "read metadata".to_string(),
                    })
                }
                _ => {
                    P2PError::FileIO(crate::error_handling::FileIOError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: e.to_string(),
                    })
                }
            })
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_config_load_save() {
        let config = AppConfig::default();

        // Create temporary config file
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = toml::to_string_pretty(&config).unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        // Load config from file
        let loaded_config = AppConfig::load_from_file(temp_file.path()).await.unwrap();

        // Verify loaded config matches original
        assert_eq!(loaded_config.network.connection_timeout_secs, config.network.connection_timeout_secs);
        assert_eq!(loaded_config.files.max_file_size, config.files.max_file_size);
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(utils::format_file_size(512), "512 B");
        assert_eq!(utils::format_file_size(1536), "1.5 KB");
        assert_eq!(utils::format_file_size(2048 * 1024), "2.0 MB");
        assert_eq!(utils::format_file_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(utils::format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(utils::format_duration(Duration::from_secs(125)), "2m 5s");
        assert_eq!(utils::format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(utils::sanitize_filename("test<>file.txt"), "test__file.txt");
        assert_eq!(utils::sanitize_filename("path/to\file.pdf"), "path_to_file.pdf");
        assert_eq!(utils::sanitize_filename("normal_file.txt"), "normal_file.txt");
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = AppConfig::default();

        // Valid config should pass
        assert!(config.validate().await.is_ok());

        // Invalid config should fail
        config.network.connection_timeout_secs = 0;
        assert!(config.validate().await.is_err());
    }
}
