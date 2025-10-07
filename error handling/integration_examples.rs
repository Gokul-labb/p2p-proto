//! Integration examples showing how to use the error handling system
//! throughout the P2P file converter application

use crate::error_handling::{
    self, P2PError, Result, 
    validation::{MultiAddrValidator, FilePathValidator, FileTypeValidator},
    timeouts::TimeoutManager,
    recovery::RecoveryManager,
    cleanup::{ResourceGuard, CleanupManager},
    display::ErrorFormatter,
};
use libp2p::{Multiaddr, PeerId};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tracing::{error, info, warn};

/// Enhanced CLI argument parser with comprehensive validation
pub mod enhanced_cli {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    #[command(name = "p2p-converter")]
    #[command(about = "P2P file converter with comprehensive error handling")]
    pub struct ValidatedArgs {
        /// Target peer multiaddress
        #[arg(short, long)]
        pub target: Option<String>,

        /// File to send
        #[arg(short, long)]
        pub file: Option<String>,

        /// Listen address for receiver mode
        #[arg(short, long)]
        pub listen: Option<String>,

        /// Output directory
        #[arg(short, long, default_value = "./output")]
        pub output_dir: String,

        /// Target format for conversion
        #[arg(long)]
        pub format: Option<String>,

        /// Maximum file size in MB
        #[arg(long, default_value = "100")]
        pub max_size: u64,

        /// Verbose output
        #[arg(short, long)]
        pub verbose: bool,
    }

    impl ValidatedArgs {
        /// Parse and validate CLI arguments
        pub async fn parse_and_validate() -> Result<ValidatedArgs> {
            let args = ValidatedArgs::parse();
            let validator = ArgumentValidator::new();
            validator.validate(&args).await?;
            Ok(args)
        }
    }

    /// Validator for CLI arguments
    pub struct ArgumentValidator {
        multiaddr_validator: MultiAddrValidator,
        file_validator: FilePathValidator,
        timeout_manager: TimeoutManager,
    }

    impl ArgumentValidator {
        pub fn new() -> Self {
            Self {
                multiaddr_validator: MultiAddrValidator::new(),
                file_validator: FilePathValidator::new(),
                timeout_manager: TimeoutManager::new(),
            }
        }

        pub async fn validate(&self, args: &ValidatedArgs) -> Result<()> {
            // Validate target multiaddr if provided
            if let Some(ref target) = args.target {
                let multiaddr = self.timeout_manager.execute_network_operation(
                    "validate_multiaddr",
                    None,
                    || async {
                        self.multiaddr_validator.validate(target)
                    }
                ).await?;

                info!("✅ Valid target address: {}", multiaddr);
            }

            // Validate listen address if provided
            if let Some(ref listen) = args.listen {
                let listen_validator = MultiAddrValidator::new()
                    .with_required_protocols(vec!["ip4".to_string(), "tcp".to_string()]);

                let multiaddr = listen_validator.validate(listen)?;
                info!("✅ Valid listen address: {}", multiaddr);
            }

            // Validate file path if provided
            if let Some(ref file_path) = args.file {
                let validated_path = self.file_validator.validate(file_path).await?;

                // Check file size
                let file_size = self.file_validator.validate_size(&validated_path, args.max_size * 1_000_000).await?;
                info!("✅ Valid file: {} ({} bytes)", validated_path.display(), file_size);
            }

            // Validate output directory
            self.validate_output_directory(&args.output_dir).await?;

            Ok(())
        }

        async fn validate_output_directory(&self, dir: &str) -> Result<()> {
            let path = Path::new(dir);

            // Create directory if it doesn't exist
            if !path.exists() {
                fs::create_dir_all(path).await
                    .map_err(|e| P2PError::FileIO(error_handling::FileIOError::DirectoryCreation {
                        path: path.to_path_buf(),
                        reason: e.to_string(),
                    }))?;
                info!("📁 Created output directory: {}", path.display());
            }

            // Check if it's a directory and writable
            let metadata = fs::metadata(path).await
                .map_err(|e| P2PError::FileIO(error_handling::FileIOError::NotFound {
                    path: path.to_path_buf(),
                }))?;

            if !metadata.is_dir() {
                return Err(P2PError::FileIO(error_handling::FileIOError::InvalidPath {
                    path: path.to_path_buf(),
                    reason: "Path is not a directory".to_string(),
                }));
            }

            Ok(())
        }
    }
}

/// Enhanced file conversion with comprehensive error handling
pub mod enhanced_conversion {
    use super::*;
    use crate::file_converter::{FileConverter, FileType, PdfConfig};

    /// File converter with integrated error handling
    pub struct EnhancedFileConverter {
        converter: FileConverter,
        type_validator: FileTypeValidator,
        timeout_manager: TimeoutManager,
        recovery_manager: RecoveryManager,
        cleanup_manager: CleanupManager,
    }

    impl EnhancedFileConverter {
        pub fn new() -> Self {
            Self {
                converter: FileConverter::new(),
                type_validator: FileTypeValidator::new(),
                timeout_manager: TimeoutManager::new()
                    .with_conversion_timeout(Duration::from_secs(300)), // 5 minutes
                recovery_manager: RecoveryManager::new(),
                cleanup_manager: CleanupManager::new(),
            }
        }

        /// Convert file with comprehensive error handling and recovery
        pub async fn convert_file_with_recovery<P: AsRef<Path>>(
            &self,
            input_path: P,
            output_path: P,
            config: &PdfConfig,
        ) -> Result<()> {
            let input_path = input_path.as_ref();
            let output_path = output_path.as_ref();

            // Create operation ID for tracking
            let operation_id = format!("convert_{}_{}", 
                                     input_path.file_name().unwrap_or_default().to_string_lossy(),
                                     chrono::Utc::now().timestamp());

            // Register resources for cleanup
            self.cleanup_manager.register_resource(
                operation_id.clone(),
                format!("File conversion: {} -> {}", input_path.display(), output_path.display())
            ).await;

            // Validate input file type
            let file_type = self.timeout_manager.execute_file_operation(
                "validate_file_type",
                input_path,
                || async {
                    self.type_validator.validate(input_path, None).await
                }
            ).await?;

            info!("📄 Detected file type: {}", file_type);

            // Perform conversion with recovery
            let result = self.recovery_manager.attempt_recovery(
                &operation_id,
                &P2PError::Conversion(error_handling::ConversionError::PdfGeneration {
                    reason: "Initial attempt".to_string(),
                }),
                || async {
                    self.perform_conversion_with_timeout(
                        input_path,
                        output_path,
                        &file_type,
                        config
                    ).await
                }
            ).await;

            // Cleanup regardless of result
            if let Err(cleanup_error) = self.cleanup_manager.cleanup_resource(&operation_id).await {
                warn!("Cleanup warning for {}: {}", operation_id, cleanup_error);
            }

            result
        }

        async fn perform_conversion_with_timeout<P: AsRef<Path>>(
            &self,
            input_path: P,
            output_path: P,
            file_type: &str,
            config: &PdfConfig,
        ) -> Result<()> {
            let input_path = input_path.as_ref();
            let output_path = output_path.as_ref();

            self.timeout_manager.execute_conversion_operation(
                "file_conversion",
                || async {
                    match file_type {
                        "txt" => {
                            self.converter.text_file_to_pdf(input_path, output_path, config)
                                .map_err(|e| P2PError::Conversion(
                                    error_handling::ConversionError::PdfGeneration {
                                        reason: e.to_string(),
                                    }
                                ))
                        }
                        "pdf" => {
                            self.converter.pdf_file_to_text(input_path, output_path)
                                .map_err(|e| P2PError::Conversion(
                                    error_handling::ConversionError::TextExtraction {
                                        reason: e.to_string(),
                                    }
                                ))
                        }
                        _ => {
                            Err(P2PError::Conversion(
                                error_handling::ConversionError::UnsupportedFormat {
                                    format: file_type.to_string(),
                                    supported: vec!["txt".to_string(), "pdf".to_string()],
                                }
                            ))
                        }
                    }
                }
            ).await
        }

        /// Get conversion statistics and health check
        pub async fn get_health_status(&self) -> ConversionHealthStatus {
            let active_resources = self.cleanup_manager.get_active_resources().await;
            let recovery_stats = self.recovery_manager.get_recovery_stats().await;
            let leaks = self.cleanup_manager.check_leaks().await;

            ConversionHealthStatus {
                active_conversions: active_resources.len(),
                active_recoveries: recovery_stats.len(),
                potential_leaks: leaks.len(),
                is_healthy: leaks.is_empty() && recovery_stats.len() < 5,
            }
        }
    }

    #[derive(Debug)]
    pub struct ConversionHealthStatus {
        pub active_conversions: usize,
        pub active_recoveries: usize,
        pub potential_leaks: usize,
        pub is_healthy: bool,
    }
}

/// Enhanced networking with comprehensive error handling
pub mod enhanced_networking {
    use super::*;
    use crate::file_sender::{FileSender, RetryConfig, SendProgress};

    /// Network manager with integrated error handling
    pub struct EnhancedNetworkManager {
        multiaddr_validator: MultiAddrValidator,
        timeout_manager: TimeoutManager,
        recovery_manager: RecoveryManager,
        cleanup_manager: CleanupManager,
        error_formatter: ErrorFormatter,
    }

    impl EnhancedNetworkManager {
        pub fn new() -> Self {
            Self {
                multiaddr_validator: MultiAddrValidator::new(),
                timeout_manager: TimeoutManager::new()
                    .with_network_timeout(Duration::from_secs(30)),
                recovery_manager: RecoveryManager::new(),
                cleanup_manager: CleanupManager::new(),
                error_formatter: ErrorFormatter::new(),
            }
        }

        /// Connect to peer with comprehensive error handling
        pub async fn connect_to_peer_with_recovery(&self, addr_str: &str) -> Result<(PeerId, Multiaddr)> {
            // Validate multiaddr format
            let multiaddr = self.multiaddr_validator.validate(addr_str)?;

            // Extract peer ID
            let peer_id = self.multiaddr_validator.extract_peer_id(&multiaddr)?
                .ok_or_else(|| P2PError::Validation(error_handling::ValidationError::MissingComponent {
                    addr: addr_str.to_string(),
                    component: "p2p peer ID".to_string(),
                }))?;

            // Attempt connection with recovery
            let connection_id = format!("connect_{}", peer_id);

            self.recovery_manager.attempt_recovery(
                &connection_id,
                &P2PError::Network(error_handling::NetworkError::ConnectionFailed {
                    peer_id: Some(peer_id),
                    address: multiaddr.clone(),
                    reason: "Initial connection attempt".to_string(),
                }),
                || async {
                    self.attempt_connection(peer_id, multiaddr.clone()).await
                }
            ).await?;

            Ok((peer_id, multiaddr))
        }

        async fn attempt_connection(&self, peer_id: PeerId, multiaddr: Multiaddr) -> Result<()> {
            // Register connection resource
            let resource_id = format!("connection_{}", peer_id);
            self.cleanup_manager.register_resource(
                resource_id.clone(),
                format!("Connection to {} at {}", peer_id, multiaddr)
            ).await;

            // Simulate connection attempt with timeout
            self.timeout_manager.execute_network_operation(
                "establish_connection",
                Some(peer_id),
                || async {
                    // In real implementation, this would use libp2p to establish connection
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    info!("🔗 Successfully connected to peer {}", peer_id);
                    Ok(())
                }
            ).await?;

            Ok(())
        }

        /// Send file with comprehensive error handling and progress tracking
        pub async fn send_file_with_monitoring<P: AsRef<Path>>(
            &self,
            peer_id: PeerId,
            multiaddr: Multiaddr,
            file_path: P,
            progress_callback: impl Fn(&SendProgress) + Send + Sync + 'static,
        ) -> Result<String> {
            let file_path = file_path.as_ref();

            // Validate file before sending
            let file_validator = FilePathValidator::new();
            let validated_path = file_validator.validate(file_path).await?;
            let file_size = file_validator.validate_size(&validated_path, 100 * 1_000_000).await?; // 100MB limit

            info!("📤 Preparing to send file: {} ({} bytes)", validated_path.display(), file_size);

            // Create enhanced file sender with integrated error handling
            let retry_config = RetryConfig {
                max_attempts: 5,
                initial_delay: Duration::from_millis(500),
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 2.0,
                connection_timeout: Duration::from_secs(15),
            };

            let mut sender = FileSender::new(Some(retry_config)).await
                .map_err(|e| P2PError::Network(error_handling::NetworkError::Transport {
                    message: format!("Failed to create file sender: {}", e),
                }))?;

            // Set up enhanced progress callback with error handling
            sender.set_progress_callback(move |progress| {
                // Log progress with error context
                match &progress.status {
                    crate::file_sender::TransferStatus::Failed(error) => {
                        let formatter = ErrorFormatter::new();
                        let user_error = P2PError::Network(error_handling::NetworkError::Transport {
                            message: error.clone(),
                        });
                        error!("Transfer error: {}", formatter.format_error(&user_error));
                    }
                    _ => {
                        progress_callback(progress);
                    }
                }
            });

            // Send file with recovery mechanisms
            let transfer_id = format!("transfer_{}_{}", peer_id, chrono::Utc::now().timestamp());

            self.recovery_manager.attempt_recovery(
                &transfer_id,
                &P2PError::Network(error_handling::NetworkError::ConnectionFailed {
                    peer_id: Some(peer_id),
                    address: multiaddr.clone(),
                    reason: "File transfer initiation".to_string(),
                }),
                || async {
                    sender.send_file(
                        peer_id,
                        multiaddr.clone(),
                        &validated_path,
                        None, // No format conversion for this example
                        false, // Don't return result
                    ).await.map_err(|e| P2PError::Network(error_handling::NetworkError::Transport {
                        message: e.to_string(),
                    }))
                }
            ).await
        }

        /// Format network error for user display
        pub fn format_network_error(&self, error: &P2PError) -> String {
            self.error_formatter.format_error(error)
        }
    }
}

/// Enhanced main application with comprehensive error handling
pub mod enhanced_application {
    use super::*;
    use enhanced_cli::{ValidatedArgs, ArgumentValidator};
    use enhanced_conversion::EnhancedFileConverter;
    use enhanced_networking::EnhancedNetworkManager;

    /// Main application with comprehensive error handling
    pub struct EnhancedP2PApplication {
        args: ValidatedArgs,
        converter: EnhancedFileConverter,
        network_manager: EnhancedNetworkManager,
        cleanup_manager: CleanupManager,
        error_formatter: ErrorFormatter,
    }

    impl EnhancedP2PApplication {
        /// Create new application instance with validation
        pub async fn new() -> Result<Self> {
            // Parse and validate CLI arguments
            let args = ValidatedArgs::parse_and_validate().await?;

            Ok(Self {
                args,
                converter: EnhancedFileConverter::new(),
                network_manager: EnhancedNetworkManager::new(),
                cleanup_manager: CleanupManager::new(),
                error_formatter: ErrorFormatter::new(),
            })
        }

        /// Run the enhanced application
        pub async fn run(&mut self) -> Result<i32> {
            // Register application-level cleanup
            self.setup_cleanup_handlers().await?;

            let result = match (&self.args.target, &self.args.file) {
                (Some(target), Some(file)) => {
                    self.run_sender_mode(target, file).await
                }
                _ => {
                    self.run_receiver_mode().await
                }
            };

            // Ensure cleanup on exit
            self.cleanup_on_exit().await;

            result
        }

        async fn run_sender_mode(&self, target: &str, file_path: &str) -> Result<i32> {
            info!("🚀 Starting enhanced sender mode");

            // Connect to peer with comprehensive error handling
            let (peer_id, multiaddr) = match self.network_manager.connect_to_peer_with_recovery(target).await {
                Ok(connection) => connection,
                Err(e) => {
                    let user_message = self.error_formatter.format_error(&e);
                    eprintln!("❌ Connection failed: {}", user_message);
                    return Ok(1);
                }
            };

            // Send file with monitoring
            let progress_callback = |progress: &SendProgress| {
                match &progress.status {
                    crate::file_sender::TransferStatus::Sending => {
                        if progress.chunks_sent % 10 == 0 {
                            println!("📤 Progress: {:.1}% ({:.1} KB/s)", 
                                   progress.percentage(), progress.speed_bps() / 1024.0);
                        }
                    }
                    crate::file_sender::TransferStatus::Completed => {
                        println!("✅ Transfer completed successfully!");
                    }
                    crate::file_sender::TransferStatus::Failed(error) => {
                        println!("❌ Transfer failed: {}", error);
                    }
                    _ => {}
                }
            };

            match self.network_manager.send_file_with_monitoring(
                peer_id,
                multiaddr,
                file_path,
                progress_callback
            ).await {
                Ok(transfer_id) => {
                    info!("✅ File sent successfully: {}", transfer_id);
                    Ok(0)
                }
                Err(e) => {
                    let user_message = self.error_formatter.format_error(&e);
                    eprintln!("❌ File transfer failed: {}", user_message);
                    Ok(1)
                }
            }
        }

        async fn run_receiver_mode(&self) -> Result<i32> {
            info!("📥 Starting enhanced receiver mode");

            // In a real implementation, this would start the P2P node
            // and listen for incoming connections

            println!("🌐 Receiver mode started");
            println!("📁 Output directory: {}", self.args.output_dir);
            println!("💾 Maximum file size: {} MB", self.args.max_size);

            // Simulate running receiver
            tokio::time::sleep(Duration::from_secs(1)).await;

            Ok(0)
        }

        async fn setup_cleanup_handlers(&self) -> Result<()> {
            // Register signal handlers for graceful shutdown
            let cleanup_manager = self.cleanup_manager.clone();

            tokio::spawn(async move {
                match tokio::signal::ctrl_c().await {
                    Ok(()) => {
                        warn!("🛑 Received Ctrl+C, cleaning up...");
                        let failed_cleanups = cleanup_manager.cleanup_all().await;
                        if !failed_cleanups.is_empty() {
                            error!("Failed to cleanup resources: {:?}", failed_cleanups);
                        } else {
                            info!("✅ All resources cleaned up successfully");
                        }
                    }
                    Err(err) => {
                        error!("Unable to listen for shutdown signal: {}", err);
                    }
                }
            });

            Ok(())
        }

        async fn cleanup_on_exit(&self) {
            info!("🧹 Performing final cleanup");

            // Check for resource leaks
            let leaks = self.cleanup_manager.check_leaks().await;
            if !leaks.is_empty() {
                warn!("⚠️  Potential resource leaks detected: {:?}", leaks);
            }

            // Cleanup all resources
            let failed_cleanups = self.cleanup_manager.cleanup_all().await;
            if !failed_cleanups.is_empty() {
                error!("❌ Failed to cleanup some resources: {:?}", failed_cleanups);
            } else {
                info!("✅ Application cleanup completed successfully");
            }
        }

        /// Get application health status
        pub async fn get_health_status(&self) -> ApplicationHealthStatus {
            let conversion_health = self.converter.get_health_status().await;
            let active_resources = self.cleanup_manager.get_active_resources().await;

            ApplicationHealthStatus {
                is_healthy: conversion_health.is_healthy && active_resources.len() < 10,
                active_resources: active_resources.len(),
                conversion_health,
                uptime: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default(),
            }
        }
    }

    #[derive(Debug)]
    pub struct ApplicationHealthStatus {
        pub is_healthy: bool,
        pub active_resources: usize,
        pub conversion_health: enhanced_conversion::ConversionHealthStatus,
        pub uptime: Duration,
    }
}

/// Example usage and integration tests
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};
    use std::io::Write;

    #[tokio::test]
    async fn test_enhanced_cli_validation() {
        // Test multiaddr validation
        let validator = enhanced_cli::ArgumentValidator::new();

        // This would normally be done through CLI parsing
        let args = enhanced_cli::ValidatedArgs {
            target: Some("/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ".to_string()),
            file: None,
            listen: None,
            output_dir: "./test_output".to_string(),
            format: None,
            max_size: 100,
            verbose: false,
        };

        // Validation should succeed for valid multiaddr
        assert!(validator.validate(&args).await.is_ok());
    }

    #[tokio::test]
    async fn test_enhanced_conversion() {
        let converter = enhanced_conversion::EnhancedFileConverter::new();

        // Create a temporary text file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Test content for conversion").unwrap();

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("output.pdf");

        let config = crate::file_converter::PdfConfig::default();

        // Conversion should handle errors gracefully
        let result = converter.convert_file_with_recovery(
            temp_file.path(),
            &output_path,
            &config
        ).await;

        // The conversion might fail due to missing fonts, but it should not panic
        match result {
            Ok(()) => println!("✅ Conversion succeeded"),
            Err(e) => println!("⚠️  Conversion failed gracefully: {}", e),
        }
    }

    #[tokio::test]
    async fn test_enhanced_networking() {
        let network_manager = enhanced_networking::EnhancedNetworkManager::new();

        // Test with invalid multiaddr
        let result = network_manager.connect_to_peer_with_recovery("invalid-addr").await;
        assert!(result.is_err());

        // Check that error is user-friendly
        if let Err(e) = result {
            let formatted = network_manager.format_network_error(&e);
            assert!(formatted.len() > 0);
            println!("User-friendly error: {}", formatted);
        }
    }

    #[tokio::test]
    async fn test_resource_cleanup() {
        let cleanup_manager = cleanup::CleanupManager::new();

        // Register some test resources
        cleanup_manager.register_resource("test1".to_string(), "Test resource 1".to_string()).await;
        cleanup_manager.register_resource("test2".to_string(), "Test resource 2".to_string()).await;

        // Verify resources are tracked
        let active = cleanup_manager.get_active_resources().await;
        assert_eq!(active.len(), 2);

        // Cleanup all resources
        let failed = cleanup_manager.cleanup_all().await;
        assert!(failed.is_empty());

        // Verify all resources are cleaned up
        let active_after = cleanup_manager.get_active_resources().await;
        assert!(active_after.is_empty());
    }

    #[tokio::test]
    async fn test_error_formatting() {
        let formatter = display::ErrorFormatter::new();

        let error = P2PError::FileIO(error_handling::FileIOError::NotFound {
            path: std::path::PathBuf::from("/nonexistent/file.txt"),
        });

        let formatted = formatter.format_error(&error);

        // Should contain user-friendly message
        assert!(formatted.contains("File not found"));
        // Should contain suggestion
        assert!(formatted.contains("Suggestion"));
        // Should be reasonably readable
        assert!(formatted.len() > 50);

        println!("Formatted error: {}", formatted);
    }
}
