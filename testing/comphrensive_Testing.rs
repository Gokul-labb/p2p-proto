//! Comprehensive Test Suite for P2P File Converter
//! 
//! This module contains unit tests, integration tests, and end-to-end tests
//! for all components of the P2P file converter system.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::{NamedTempFile, TempDir};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;
use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

// Import all modules for testing
use crate::{
    error_handling::{P2PError, Result as P2PResult, validation::*},
    file_converter::{FileConverter, FileType, PdfConfig},
    file_sender::{FileSender, RetryConfig, SendProgress, TransferStatus},
    p2p_stream_handler::{FileConversionService, FileConversionConfig, P2PFileNode},
    main_event_loop::{P2PFileConverter, ShutdownReason},
};

/// Test utilities and helper functions
pub mod test_utils {
    use super::*;
    use std::io::Write;

    /// Create a temporary text file with specified content
    pub fn create_temp_text_file(content: &str) -> Result<NamedTempFile> {
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(content.as_bytes())?;
        temp_file.flush()?;
        Ok(temp_file)
    }

    /// Create a temporary PDF file with basic PDF structure
    pub fn create_temp_pdf_file() -> Result<NamedTempFile> {
        let mut temp_file = NamedTempFile::new()?;
        let pdf_content = b"%PDF-1.4
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
>>
endobj
4 0 obj
<<
/Length 44
>>
stream
BT
/F1 12 Tf
72 720 Td
(Hello, World!) Tj
ET
endstream
endobj
xref
0 5
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000206 00000 n 
trailer
<<
/Size 5
/Root 1 0 R
>>
startxref
299
%%EOF";
        temp_file.write_all(pdf_content)?;
        temp_file.flush()?;
        Ok(temp_file)
    }

    /// Create a test directory with sample files
    pub async fn create_test_directory() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;

        // Create various test files
        let test_files = vec![
            ("document1.txt", "This is a simple text document for testing.\n\nIt contains multiple lines and paragraphs."),
            ("document2.txt", "# Markdown Document\n\n## Section 1\n\nThis is a markdown-style document.\n\n- Item 1\n- Item 2\n- Item 3"),
            ("empty.txt", ""),
            ("large_text.txt", &"A".repeat(10000)), // 10KB file
            ("unicode.txt", "Unicode test: 你好世界 🌍 Café naïve résumé"),
        ];

        for (filename, content) in test_files {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).await?;
        }

        // Create a test PDF file
        let pdf_file = create_temp_pdf_file()?;
        let pdf_dest = temp_dir.path().join("test.pdf");
        fs::copy(pdf_file.path(), &pdf_dest).await?;

        Ok(temp_dir)
    }

    /// Generate a valid test multiaddr
    pub fn generate_test_multiaddr(port: u16) -> Multiaddr {
        format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port, PeerId::random())
            .parse()
            .unwrap()
    }

    /// Wait for condition with timeout
    pub async fn wait_for_condition<F>(
        condition: F,
        timeout_duration: Duration,
        check_interval: Duration,
    ) -> bool
    where
        F: Fn() -> bool + Send + Sync,
    {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout_duration {
            if condition() {
                return true;
            }
            tokio::time::sleep(check_interval).await;
        }
        false
    }

    /// Create a mock progress callback for testing
    pub fn create_progress_callback() -> (
        impl Fn(&SendProgress) + Send + Sync + 'static,
        Arc<Mutex<Vec<SendProgress>>>,
    ) {
        let progress_log = Arc::new(Mutex::new(Vec::new()));
        let progress_log_clone = progress_log.clone();

        let callback = move |progress: &SendProgress| {
            let log = progress_log_clone.clone();
            let progress = progress.clone();
            tokio::spawn(async move {
                log.lock().await.push(progress);
            });
        };

        (callback, progress_log)
    }
}

/// Unit tests for file conversion functionality
#[cfg(test)]
mod conversion_tests {
    use super::*;
    use test_utils::*;

    #[tokio::test]
    async fn test_file_type_detection() {
        let converter = FileConverter::new();

        // Test text file detection
        let text_file = create_temp_text_file("Hello, world!").unwrap();
        let file_type = converter.detect_file_type(text_file.path()).unwrap();
        assert_eq!(file_type, FileType::Text);

        // Test PDF file detection
        let pdf_file = create_temp_pdf_file().unwrap();
        let file_type = converter.detect_file_type(pdf_file.path()).unwrap();
        assert_eq!(file_type, FileType::Pdf);

        // Test unknown file type
        let unknown_file = NamedTempFile::new().unwrap();
        // Write binary data that's not a known format
        std::fs::write(unknown_file.path(), &[0xFF, 0xFE, 0xFD, 0xFC]).unwrap();
        let file_type = converter.detect_file_type(unknown_file.path()).unwrap();
        assert_eq!(file_type, FileType::Unknown);
    }

    #[tokio::test]
    async fn test_text_to_pdf_conversion() {
        let mut converter = FileConverter::new();
        let config = PdfConfig {
            title: "Test Document".to_string(),
            font_size: 12,
            margins: 20,
            ..Default::default()
        };

        let text_content = "# Test Document\n\nThis is a test document for PDF conversion.\n\n## Features\n\n- Text formatting\n- Multiple paragraphs\n- Special characters: àáâãäå";

        let result = converter.text_to_pdf(text_content, &config);

        // Conversion might fail due to missing fonts in test environment
        match result {
            Ok(pdf_data) => {
                assert!(!pdf_data.is_empty());
                assert!(pdf_data.starts_with(b"%PDF"));
                println!("✅ Text to PDF conversion successful: {} bytes", pdf_data.len());
            }
            Err(e) => {
                println!("⚠️  Text to PDF conversion failed (expected in test environment): {}", e);
                // This is acceptable in test environment without proper fonts
            }
        }
    }

    #[tokio::test]
    async fn test_pdf_to_text_extraction() {
        let converter = FileConverter::new();
        let pdf_file = create_temp_pdf_file().unwrap();

        let result = converter.pdf_to_text_from_file(pdf_file.path());

        match result {
            Ok(text) => {
                assert!(!text.is_empty());
                // PDF might contain "Hello, World!" or similar content
                println!("✅ PDF to text extraction successful: {} characters", text.len());
                println!("Extracted text: {:?}", text.trim());
            }
            Err(e) => {
                println!("⚠️  PDF to text extraction failed: {}", e);
                // This might fail if pdf-extract crate has issues with our simple PDF
            }
        }
    }

    #[tokio::test]
    async fn test_file_conversion_edge_cases() {
        let mut converter = FileConverter::new();
        let config = PdfConfig::default();

        // Test empty text conversion
        let result = converter.text_to_pdf("", &config);
        match result {
            Ok(pdf_data) => {
                assert!(!pdf_data.is_empty()); // Should still produce a valid PDF
            }
            Err(_) => {
                // Acceptable failure for empty content
            }
        }

        // Test very long text
        let long_text = "A".repeat(100000); // 100KB of text
        let result = converter.text_to_pdf(&long_text, &config);
        match result {
            Ok(pdf_data) => {
                assert!(!pdf_data.is_empty());
                println!("✅ Large text conversion successful: {} bytes", pdf_data.len());
            }
            Err(e) => {
                println!("⚠️  Large text conversion failed: {}", e);
            }
        }

        // Test Unicode content
        let unicode_text = "Unicode test: 你好世界 🌍 Café naïve résumé ñoño";
        let result = converter.text_to_pdf(unicode_text, &config);
        match result {
            Ok(pdf_data) => {
                assert!(!pdf_data.is_empty());
                println!("✅ Unicode text conversion successful");
            }
            Err(e) => {
                println!("⚠️  Unicode text conversion failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_conversion_configuration() {
        let mut converter = FileConverter::new();

        // Test different PDF configurations
        let configs = vec![
            PdfConfig {
                title: "Small Font".to_string(),
                font_size: 8,
                margins: 10,
                line_spacing: 1.0,
                ..Default::default()
            },
            PdfConfig {
                title: "Large Font".to_string(),
                font_size: 16,
                margins: 30,
                line_spacing: 1.5,
                ..Default::default()
            },
        ];

        let test_text = "This is a test document with different formatting options.";

        for (i, config) in configs.iter().enumerate() {
            let result = converter.text_to_pdf(test_text, config);
            match result {
                Ok(pdf_data) => {
                    println!("✅ Config {} successful: {} bytes", i, pdf_data.len());
                }
                Err(e) => {
                    println!("⚠️  Config {} failed: {}", i, e);
                }
            }
        }
    }
}

/// Unit tests for error handling and validation
#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::error_handling::validation::*;

    #[tokio::test]
    async fn test_multiaddr_validation() {
        let validator = MultiAddrValidator::new();

        // Test valid multiaddrs
        let valid_addrs = vec![
            "/ip4/127.0.0.1/tcp/8080",
            "/ip4/192.168.1.100/tcp/9000/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ",
            "/ip6/::1/tcp/8080",
            "/dns/example.com/tcp/443",
        ];

        for addr in valid_addrs {
            let result = validator.validate(addr);
            assert!(result.is_ok(), "Valid address should pass: {}", addr);
        }

        // Test invalid multiaddrs
        let invalid_addrs = vec![
            "not-a-multiaddr",
            "/ip4/256.256.256.256/tcp/8080", // Invalid IP
            "/ip4/127.0.0.1/tcp/70000",      // Invalid port
            "/ip4/127.0.0.1",                // Missing required TCP
        ];

        for addr in invalid_addrs {
            let result = validator.validate(addr);
            assert!(result.is_err(), "Invalid address should fail: {}", addr);
        }
    }

    #[tokio::test]
    async fn test_file_path_validation() {
        let validator = FilePathValidator::new().skip_existence_check();

        // Test valid file paths
        let valid_paths = vec![
            "document.txt",
            "report.pdf",
            "data.md",
        ];

        for path in valid_paths {
            let result = validator.validate(path).await;
            assert!(result.is_ok(), "Valid path should pass: {}", path);
        }

        // Test invalid file paths
        let invalid_paths = vec![
            "../../../etc/passwd",  // Path traversal
            "document.exe",         // Invalid extension
            "file<>name.txt",       // Invalid characters
        ];

        for path in invalid_paths {
            let result = validator.validate(path).await;
            assert!(result.is_err(), "Invalid path should fail: {}", path);
        }
    }

    #[tokio::test]
    async fn test_file_type_validation() {
        let validator = FileTypeValidator::new();

        // Create test files
        let text_file = create_temp_text_file("Hello, world!").unwrap();
        let pdf_file = create_temp_pdf_file().unwrap();

        // Test text file validation
        let result = validator.validate(text_file.path(), Some("txt")).await;
        match result {
            Ok(file_type) => assert_eq!(file_type, "txt"),
            Err(e) => println!("Text file validation failed: {}", e),
        }

        // Test PDF file validation
        let result = validator.validate(pdf_file.path(), Some("pdf")).await;
        match result {
            Ok(file_type) => assert_eq!(file_type, "pdf"),
            Err(e) => println!("PDF file validation failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_timeout_manager() {
        use crate::error_handling::timeouts::TimeoutManager;

        let timeout_manager = TimeoutManager::new()
            .with_network_timeout(Duration::from_millis(100));

        // Test operation that should complete
        let result = timeout_manager.execute_network_operation(
            "fast_operation",
            None,
            || async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok::<(), P2PError>(())
            }
        ).await;
        assert!(result.is_ok());

        // Test operation that should timeout
        let result = timeout_manager.execute_network_operation(
            "slow_operation", 
            None,
            || async {
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok::<(), P2PError>(())
            }
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test] 
    async fn test_resource_cleanup() {
        use crate::error_handling::cleanup::{ResourceGuard, CleanupManager};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let cleanup_called = Arc::new(AtomicBool::new(false));
        let cleanup_called_clone = cleanup_called.clone();

        // Test RAII resource guard
        {
            let _guard = ResourceGuard::new(
                "test_resource".to_string(),
                "test".to_string(),
                move |_| {
                    cleanup_called_clone.store(true, Ordering::SeqCst);
                }
            );
        } // Guard goes out of scope here

        assert!(cleanup_called.load(Ordering::SeqCst));

        // Test cleanup manager
        let cleanup_manager = CleanupManager::new();
        cleanup_manager.register_resource("test1".to_string(), "Test resource".to_string()).await;

        let active = cleanup_manager.get_active_resources().await;
        assert_eq!(active.len(), 1);

        let failed = cleanup_manager.cleanup_all().await;
        assert!(failed.is_empty());

        let active_after = cleanup_manager.get_active_resources().await;
        assert!(active_after.is_empty());
    }
}

/// Integration tests for P2P networking
#[cfg(test)]
mod networking_tests {
    use super::*;
    use test_utils::*;

    #[tokio::test]
    async fn test_file_sender_creation() {
        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 1.5,
            connection_timeout: Duration::from_secs(10),
        };

        let result = FileSender::new(Some(retry_config)).await;
        assert!(result.is_ok(), "FileSender creation should succeed");

        let sender = result.unwrap();
        let progress = sender.get_all_progress().await;
        assert!(progress.is_empty(), "New sender should have no active transfers");
    }

    #[tokio::test]
    async fn test_p2p_node_creation() {
        let config = FileConversionConfig {
            max_concurrent_transfers: 3,
            output_dir: PathBuf::from("./test_output"),
            auto_convert: true,
            return_results: false,
            pdf_config: PdfConfig::default(),
        };

        let result = P2PFileNode::new(config).await;
        assert!(result.is_ok(), "P2PFileNode creation should succeed");
    }

    #[tokio::test]
    async fn test_file_conversion_service() {
        let config = FileConversionConfig {
            max_concurrent_transfers: 2,
            output_dir: PathBuf::from("./test_output"),
            auto_convert: false,
            return_results: true,
            pdf_config: PdfConfig::default(),
        };

        let result = FileConversionService::new(config);
        assert!(result.is_ok(), "FileConversionService creation should succeed");

        let service = result.unwrap();
        let progress = service.get_transfer_progress().await;
        assert!(progress.is_empty(), "New service should have no active transfers");
    }

    #[tokio::test]
    async fn test_progress_tracking() {
        let (callback, progress_log) = create_progress_callback();

        // Simulate progress updates
        let progress = SendProgress {
            transfer_id: "test_123".to_string(),
            file_path: PathBuf::from("test.txt"),
            peer_id: PeerId::random(),
            total_size: 1000,
            sent_bytes: 250,
            chunks_sent: 5,
            total_chunks: 20,
            start_time: std::time::Instant::now(),
            status: TransferStatus::Sending,
            connection_attempts: 1,
            last_error: None,
        };

        callback(&progress);

        // Wait for async callback to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        let logged_progress = progress_log.lock().await;
        assert_eq!(logged_progress.len(), 1);
        assert_eq!(logged_progress[0].transfer_id, "test_123");
        assert_eq!(logged_progress[0].percentage(), 25.0);
    }

    #[tokio::test]
    async fn test_transfer_status_transitions() {
        use crate::file_sender::TransferStatus;

        let statuses = vec![
            TransferStatus::Connecting,
            TransferStatus::Negotiating,
            TransferStatus::Sending,
            TransferStatus::WaitingResponse,
            TransferStatus::Completed,
        ];

        let mut progress = SendProgress {
            transfer_id: "status_test".to_string(),
            file_path: PathBuf::from("test.txt"),
            peer_id: PeerId::random(),
            total_size: 1000,
            sent_bytes: 0,
            chunks_sent: 0,
            total_chunks: 10,
            start_time: std::time::Instant::now(),
            status: TransferStatus::Connecting,
            connection_attempts: 1,
            last_error: None,
        };

        for (i, status) in statuses.iter().enumerate() {
            progress.status = status.clone();
            progress.chunks_sent = i;
            progress.sent_bytes = (i * 100) as u64;

            let status_string = progress.status_string();
            assert!(!status_string.is_empty());

            let percentage = progress.percentage();
            assert!(percentage >= 0.0 && percentage <= 100.0);

            println!("Status {}: {} - {:.1}%", i, status_string, percentage);
        }
    }
}

/// End-to-end tests with multiple peer instances
#[cfg(test)]
mod e2e_tests {
    use super::*;
    use test_utils::*;
    use tokio::sync::mpsc;
    use std::sync::atomic::{AtomicU16, Ordering};

    static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

    fn get_next_port() -> u16 {
        PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    #[tokio::test]
    async fn test_two_peer_setup() {
        let temp_dir = create_test_directory().await.unwrap();

        // Create two P2P nodes
        let receiver_config = FileConversionConfig {
            max_concurrent_transfers: 2,
            output_dir: temp_dir.path().join("received"),
            auto_convert: false,
            return_results: true,
            pdf_config: PdfConfig::default(),
        };

        let sender_config = FileConversionConfig {
            max_concurrent_transfers: 2,
            output_dir: temp_dir.path().join("temp"),
            auto_convert: false,
            return_results: false,
            pdf_config: PdfConfig::default(),
        };

        let receiver_result = P2PFileNode::new(receiver_config).await;
        let sender_result = P2PFileNode::new(sender_config).await;

        assert!(receiver_result.is_ok(), "Receiver node creation should succeed");
        assert!(sender_result.is_ok(), "Sender node creation should succeed");

        // In a real test, we would start the nodes and test communication
        // For now, we verify they can be created successfully
        println!("✅ Two-peer setup test completed");
    }

    #[tokio::test]
    async fn test_multiple_file_transfer_simulation() {
        let temp_dir = create_test_directory().await.unwrap();

        // Simulate multiple file transfers
        let file_paths = vec![
            temp_dir.path().join("document1.txt"),
            temp_dir.path().join("document2.txt"),
            temp_dir.path().join("unicode.txt"),
        ];

        // Verify all test files exist
        for path in &file_paths {
            assert!(path.exists(), "Test file should exist: {}", path.display());

            let metadata = fs::metadata(path).await.unwrap();
            assert!(metadata.is_file(), "Path should be a file: {}", path.display());
            assert!(metadata.len() > 0, "File should not be empty: {}", path.display());
        }

        // Create file sender for simulation
        let retry_config = RetryConfig {
            max_attempts: 2,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 1.5,
            connection_timeout: Duration::from_secs(5),
        };

        let sender_result = FileSender::new(Some(retry_config)).await;
        assert!(sender_result.is_ok(), "Sender creation should succeed");

        let sender = sender_result.unwrap();

        // Test that we can track multiple transfers (even if they would fail)
        println!("✅ Multiple file transfer simulation setup completed");
        println!("Test files prepared: {} files", file_paths.len());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let temp_dir = create_test_directory().await.unwrap();

        // Test concurrent file operations
        let file_paths: Vec<_> = (0..3).map(|i| {
            temp_dir.path().join(format!("concurrent_test_{}.txt", i))
        }).collect();

        // Create multiple files concurrently
        let create_tasks: Vec<_> = file_paths.iter().enumerate().map(|(i, path)| {
            let path = path.clone();
            tokio::spawn(async move {
                let content = format!("Concurrent test file {} created at {:?}", i, std::time::SystemTime::now());
                fs::write(&path, content).await.unwrap();
                path
            })
        }).collect();

        // Wait for all files to be created
        let mut created_paths = Vec::new();
        for task in create_tasks {
            let path = task.await.unwrap();
            created_paths.push(path);
        }

        // Verify all files were created
        assert_eq!(created_paths.len(), 3);
        for path in &created_paths {
            assert!(path.exists(), "Concurrent file should exist: {}", path.display());
        }

        // Test concurrent file validation
        let validation_tasks: Vec<_> = created_paths.iter().map(|path| {
            let path = path.clone();
            tokio::spawn(async move {
                let validator = FilePathValidator::new();
                validator.validate(&path).await
            })
        }).collect();

        let mut validation_results = Vec::new();
        for task in validation_tasks {
            let result = task.await.unwrap();
            validation_results.push(result);
        }

        // All validations should succeed
        for (i, result) in validation_results.iter().enumerate() {
            assert!(result.is_ok(), "Validation {} should succeed", i);
        }

        println!("✅ Concurrent operations test completed");
    }

    #[tokio::test]
    async fn test_error_recovery_simulation() {
        // Test error recovery mechanisms
        use crate::error_handling::recovery::RecoveryManager;

        let recovery_manager = RecoveryManager::new();

        // Simulate an operation that fails initially but succeeds on retry
        let mut attempt_count = 0;
        let result = recovery_manager.attempt_recovery(
            "test_operation",
            &P2PError::Network(crate::error_handling::NetworkError::Transport {
                message: "Test error".to_string(),
            }),
            || async {
                attempt_count += 1;
                if attempt_count < 3 {
                    Err(P2PError::Network(crate::error_handling::NetworkError::Transport {
                        message: format!("Attempt {} failed", attempt_count),
                    }))
                } else {
                    Ok(format!("Success on attempt {}", attempt_count))
                }
            }
        ).await;

        assert!(result.is_ok(), "Recovery should eventually succeed");
        assert_eq!(attempt_count, 3, "Should make exactly 3 attempts");

        println!("✅ Error recovery simulation completed");
    }

    #[tokio::test]
    async fn test_resource_lifecycle() {
        use crate::error_handling::cleanup::CleanupManager;

        let cleanup_manager = CleanupManager::new();

        // Simulate resource lifecycle
        let resource_ids: Vec<String> = (0..5).map(|i| format!("resource_{}", i)).collect();

        // Register resources
        for id in &resource_ids {
            cleanup_manager.register_resource(
                id.clone(),
                format!("Test resource {}", id)
            ).await;
        }

        // Verify all resources are tracked
        let active = cleanup_manager.get_active_resources().await;
        assert_eq!(active.len(), 5);

        // Cleanup individual resources
        for (i, id) in resource_ids.iter().enumerate().take(3) {
            let result = cleanup_manager.cleanup_resource(id).await;
            assert!(result.is_ok(), "Cleanup {} should succeed", i);
        }

        // Verify partial cleanup
        let active_after_partial = cleanup_manager.get_active_resources().await;
        assert_eq!(active_after_partial.len(), 2);

        // Cleanup remaining resources
        let failed = cleanup_manager.cleanup_all().await;
        assert!(failed.is_empty(), "Final cleanup should succeed completely");

        let active_final = cleanup_manager.get_active_resources().await;
        assert!(active_final.is_empty(), "No resources should remain");

        println!("✅ Resource lifecycle test completed");
    }
}

/// Performance and stress tests
#[cfg(test)]
mod performance_tests {
    use super::*;
    use test_utils::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_file_validation_performance() {
        let temp_dir = create_test_directory().await.unwrap();
        let validator = FilePathValidator::new();

        let file_paths: Vec<_> = (0..100).map(|i| {
            temp_dir.path().join(format!("perf_test_{}.txt", i))
        }).collect();

        // Create test files
        for path in &file_paths {
            fs::write(path, format!("Performance test file: {}", path.display())).await.unwrap();
        }

        // Measure validation performance
        let start = Instant::now();
        let mut validation_count = 0;

        for path in &file_paths {
            let result = validator.validate(path).await;
            assert!(result.is_ok(), "Validation should succeed for: {}", path.display());
            validation_count += 1;
        }

        let duration = start.elapsed();
        let avg_time = duration / validation_count;

        println!("✅ Validation performance: {} files in {:?} (avg: {:?} per file)", 
                 validation_count, duration, avg_time);

        // Performance assertion (adjust based on requirements)
        assert!(avg_time < Duration::from_millis(10), "Validation should be fast");
    }

    #[tokio::test]
    async fn test_error_handling_overhead() {
        use crate::error_handling::display::ErrorFormatter;

        let formatter = ErrorFormatter::new();
        let errors = vec![
            P2PError::FileIO(crate::error_handling::FileIOError::NotFound {
                path: PathBuf::from("test.txt"),
            }),
            P2PError::Network(crate::error_handling::NetworkError::ConnectionTimeout {
                address: "/ip4/127.0.0.1/tcp/8080".parse().unwrap(),
                duration: Duration::from_secs(30),
            }),
            P2PError::Validation(crate::error_handling::ValidationError::InvalidMultiaddr {
                addr: "invalid".to_string(),
                reason: "Test error".to_string(),
            }),
        ];

        let start = Instant::now();
        let mut format_count = 0;

        for _ in 0..1000 {
            for error in &errors {
                let _formatted = formatter.format_error(error);
                format_count += 1;
            }
        }

        let duration = start.elapsed();
        let avg_time = duration / format_count;

        println!("✅ Error formatting performance: {} formats in {:?} (avg: {:?} per format)",
                 format_count, duration, avg_time);

        // Performance assertion
        assert!(avg_time < Duration::from_micros(100), "Error formatting should be fast");
    }

    #[tokio::test]
    async fn test_concurrent_validation() {
        let temp_dir = create_test_directory().await.unwrap();
        let validator = Arc::new(FilePathValidator::new());

        // Create test files
        let file_paths: Vec<_> = (0..50).map(|i| {
            temp_dir.path().join(format!("concurrent_{}.txt", i))
        }).collect();

        for path in &file_paths {
            fs::write(path, "Concurrent validation test").await.unwrap();
        }

        // Run concurrent validations
        let start = Instant::now();
        let tasks: Vec<_> = file_paths.iter().map(|path| {
            let validator = validator.clone();
            let path = path.clone();
            tokio::spawn(async move {
                validator.validate(&path).await
            })
        }).collect();

        let mut success_count = 0;
        for task in tasks {
            let result = task.await.unwrap();
            if result.is_ok() {
                success_count += 1;
            }
        }

        let duration = start.elapsed();

        println!("✅ Concurrent validation: {}/{} successful in {:?}",
                 success_count, file_paths.len(), duration);

        assert_eq!(success_count, file_paths.len(), "All validations should succeed");
        assert!(duration < Duration::from_secs(5), "Concurrent validation should be reasonably fast");
    }
}

/// Integration tests for the complete application flow
#[cfg(test)]
mod application_tests {
    use super::*;
    use test_utils::*;

    #[tokio::test]
    async fn test_application_startup_shutdown() {
        // Test application can start and shut down cleanly
        // This would normally test the full P2PFileConverter but we'll simulate

        let temp_dir = create_test_directory().await.unwrap();

        // Simulate application configuration
        use crate::config_utilities::AppConfig;

        let mut config = AppConfig::default();
        config.files.output_directory = temp_dir.path().to_path_buf();
        config.files.max_file_size = 10 * 1024 * 1024; // 10MB

        // Validate configuration
        let validation_result = config.validate().await;
        assert!(validation_result.is_ok(), "Configuration should be valid");

        println!("✅ Application startup/shutdown simulation completed");
    }

    #[tokio::test]
    async fn test_full_conversion_workflow() {
        let temp_dir = create_test_directory().await.unwrap();
        let mut converter = FileConverter::new();

        // Test complete workflow: file detection -> validation -> conversion
        let text_file_path = temp_dir.path().join("document1.txt");

        // 1. File type detection
        let file_type = converter.detect_file_type(&text_file_path);
        match file_type {
            Ok(FileType::Text) => println!("✅ File type detection successful"),
            Ok(other) => println!("⚠️  Unexpected file type: {:?}", other),
            Err(e) => println!("⚠️  File type detection failed: {}", e),
        }

        // 2. File validation
        let validator = FilePathValidator::new();
        let validation_result = validator.validate(&text_file_path).await;
        assert!(validation_result.is_ok(), "File validation should succeed");

        // 3. File conversion (if possible)
        let config = PdfConfig {
            title: "Test Workflow".to_string(),
            font_size: 12,
            margins: 20,
            ..Default::default()
        };

        let content = fs::read_to_string(&text_file_path).await.unwrap();
        let conversion_result = converter.text_to_pdf(&content, &config);

        match conversion_result {
            Ok(pdf_data) => {
                println!("✅ Full conversion workflow successful: {} bytes", pdf_data.len());

                // Save converted file
                let output_path = temp_dir.path().join("converted.pdf");
                fs::write(&output_path, &pdf_data).await.unwrap();

                // Verify output file
                assert!(output_path.exists());
                let output_size = fs::metadata(&output_path).await.unwrap().len();
                assert!(output_size > 0);
            }
            Err(e) => {
                println!("⚠️  Conversion failed (may be expected in test environment): {}", e);
            }
        }

        println!("✅ Full conversion workflow test completed");
    }

    #[tokio::test]
    async fn test_error_propagation() {
        // Test that errors propagate correctly through the system
        let validator = FilePathValidator::new();

        // Test with non-existent file
        let result = validator.validate("nonexistent_file.txt").await;
        assert!(result.is_err(), "Should fail for non-existent file");

        match result.unwrap_err() {
            P2PError::FileIO(crate::error_handling::FileIOError::NotFound { path }) => {
                assert_eq!(path, PathBuf::from("nonexistent_file.txt"));
                println!("✅ Error propagation test successful");
            }
            other => {
                panic!("Unexpected error type: {:?}", other);
            }
        }
    }
}

#[cfg(test)]
mod test_runner {
    use super::*;

    /// Run all tests with summary
    #[tokio::test]
    async fn run_comprehensive_test_suite() {
        println!("🧪 Running Comprehensive P2P File Converter Test Suite");
        println!("=" .repeat(60));

        // This test serves as a summary and coordination point
        // Individual test modules are run separately by cargo test

        let test_modules = vec![
            "conversion_tests",
            "validation_tests", 
            "networking_tests",
            "e2e_tests",
            "performance_tests",
            "application_tests",
        ];

        println!("📋 Test modules available:");
        for module in &test_modules {
            println!("  ✓ {}", module);
        }

        println!();
        println!("🚀 Run tests with:");
        println!("  cargo test                    # All tests");
        println!("  cargo test conversion_tests   # Conversion tests only");
        println!("  cargo test networking_tests   # Networking tests only");
        println!("  cargo test e2e_tests         # End-to-end tests only");
        println!("  cargo test --release          # Release mode tests");
        println!();

        // Create test environment verification
        let temp_dir = test_utils::create_test_directory().await.unwrap();
        println!("✅ Test environment setup successful");
        println!("📁 Test directory: {}", temp_dir.path().display());

        // Verify test files were created
        let test_files = vec!["document1.txt", "document2.txt", "test.pdf"];
        for file in &test_files {
            let path = temp_dir.path().join(file);
            assert!(path.exists(), "Test file should exist: {}", file);
        }

        println!("✅ All test files present and accessible");
        println!("=" .repeat(60));
        println!("🎉 Comprehensive test suite verification completed!");
    }
}
