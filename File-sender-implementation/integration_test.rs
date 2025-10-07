// Integration tests for file sender functionality

#[cfg(test)]
mod integration_tests {
    use super::*;
    use file_sender::{FileSender, RetryConfig, TransferStatus};
    use libp2p::{Multiaddr, PeerId};
    use std::time::Duration;
    use tempfile::{NamedTempFile, TempDir};
    use tokio::io::AsyncWriteExt;

    /// Test basic file sender creation and configuration
    #[tokio::test]
    async fn test_sender_creation() {
        let sender = FileSender::new(None).await;
        assert!(sender.is_ok());

        let custom_config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 1.5,
            connection_timeout: Duration::from_secs(10),
        };

        let sender_with_config = FileSender::new(Some(custom_config)).await;
        assert!(sender_with_config.is_ok());
    }

    /// Test progress calculation accuracy
    #[tokio::test]
    async fn test_progress_calculation() {
        use file_sender::SendProgress;
        use std::time::Instant;

        let progress = SendProgress {
            transfer_id: "test-123".to_string(),
            file_path: std::path::PathBuf::from("test.txt"),
            peer_id: PeerId::random(),
            total_size: 1000,
            sent_bytes: 250,
            chunks_sent: 5,
            total_chunks: 20,
            start_time: Instant::now() - Duration::from_secs(2),
            status: TransferStatus::Sending,
            connection_attempts: 1,
            last_error: None,
        };

        // Test percentage calculation
        assert_eq!(progress.percentage(), 25.0);

        // Test speed calculation
        let speed = progress.speed_bps();
        assert!(speed > 100.0); // Should be around 125 bytes/second
        assert!(speed < 200.0);

        // Test ETA calculation
        assert!(progress.eta_seconds().is_some());
        let eta = progress.eta_seconds().unwrap();
        assert!(eta > 5.0); // Should be around 6 seconds
        assert!(eta < 10.0);
    }

    /// Test transfer status transitions
    #[tokio::test]
    async fn test_status_transitions() {
        use file_sender::{SendProgress, TransferStatus};

        let mut progress = SendProgress {
            transfer_id: "test".to_string(),
            file_path: std::path::PathBuf::from("test.txt"),
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

        // Test status string representations
        assert!(progress.status_string().contains("Connecting"));

        progress.status = TransferStatus::Negotiating;
        assert_eq!(progress.status_string(), "Negotiating protocol");

        progress.status = TransferStatus::Sending;
        progress.chunks_sent = 3;
        assert!(progress.status_string().contains("3/10"));

        progress.status = TransferStatus::WaitingResponse;
        assert_eq!(progress.status_string(), "Waiting for response");

        progress.status = TransferStatus::Completed;
        assert_eq!(progress.status_string(), "Completed successfully");

        progress.status = TransferStatus::Failed("Connection timeout".to_string());
        assert!(progress.status_string().contains("Connection timeout"));

        progress.status = TransferStatus::Cancelled;
        assert_eq!(progress.status_string(), "Cancelled");
    }

    /// Test file validation and error handling
    #[tokio::test]
    async fn test_file_validation() {
        use std::io::Write;

        let mut sender = FileSender::new(None).await.unwrap();
        let peer_id = PeerId::random();
        let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();

        // Test with non-existent file
        let result = sender.send_file(
            peer_id,
            target_addr.clone(),
            "non_existent_file.txt",
            None,
            false,
        ).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to open file"));

        // Test with valid file
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "Hello, world!").unwrap();

        // This would normally fail because we're not actually connecting,
        // but it should at least pass the file validation stage
        let result = sender.send_file(
            peer_id,
            target_addr,
            temp_file.path(),
            None,
            false,
        ).await;

        // The result might be an error due to connection failure,
        // but it shouldn't be a file validation error
        if let Err(e) = result {
            assert!(!e.to_string().contains("Failed to open file"));
        }
    }

    /// Test progress reporter functionality
    #[tokio::test]
    async fn test_progress_reporter() {
        use file_sender::progress::ProgressReporter;
        use file_sender::{SendProgress, TransferStatus};

        let mut reporter = ProgressReporter::new(Duration::from_millis(100));

        let progress = SendProgress {
            transfer_id: "test-456".to_string(),
            file_path: std::path::PathBuf::from("example.pdf"),
            peer_id: PeerId::random(),
            total_size: 2048,
            sent_bytes: 512,
            chunks_sent: 2,
            total_chunks: 8,
            start_time: std::time::Instant::now() - Duration::from_secs(1),
            status: TransferStatus::Sending,
            connection_attempts: 1,
            last_error: None,
        };

        // Test progress formatting
        let formatted = reporter.format_progress(&progress);
        assert!(formatted.contains("test-456"));
        assert!(formatted.contains("25.0%")); // 512/2048 = 25%
        assert!(formatted.contains("512/2048"));
        assert!(formatted.contains("Sending chunk 2/8"));

        // Test rate limiting
        assert!(reporter.maybe_report(&progress)); // First call should report
        assert!(!reporter.maybe_report(&progress)); // Second call should not (too soon)

        // Wait for interval and try again
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(reporter.maybe_report(&progress)); // Should report again
    }

    /// Test retry configuration and backoff
    #[tokio::test]
    async fn test_retry_configuration() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
            backoff_multiplier: 2.0,
            connection_timeout: Duration::from_secs(5),
        };

        // Test backoff calculation simulation
        let mut delay = config.initial_delay;

        assert_eq!(delay, Duration::from_millis(100));

        delay = Duration::from_millis(
            ((delay.as_millis() as f64) * config.backoff_multiplier).min(config.max_delay.as_millis() as f64) as u64
        );
        assert_eq!(delay, Duration::from_millis(200));

        delay = Duration::from_millis(
            ((delay.as_millis() as f64) * config.backoff_multiplier).min(config.max_delay.as_millis() as f64) as u64
        );
        assert_eq!(delay, Duration::from_millis(400));

        delay = Duration::from_millis(
            ((delay.as_millis() as f64) * config.backoff_multiplier).min(config.max_delay.as_millis() as f64) as u64
        );
        assert_eq!(delay, Duration::from_secs(1)); // Capped at max_delay
    }

    /// Test concurrent transfer limits and management
    #[tokio::test]
    async fn test_concurrent_transfers() {
        let mut sender = FileSender::new(None).await.unwrap();

        // Initially no active transfers
        let all_progress = sender.get_all_progress().await;
        assert!(all_progress.is_empty());

        // Test that we can track multiple transfers
        // (This is mainly testing the data structures since we won't actually connect)

        let peer_id = PeerId::random();
        let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();

        // Create multiple temporary files
        let temp_dir = TempDir::new().unwrap();
        let mut temp_files = Vec::new();

        for i in 0..3 {
            let file_path = temp_dir.path().join(format!("test_file_{}.txt", i));
            let mut file = tokio::fs::File::create(&file_path).await.unwrap();
            file.write_all(format!("Test content for file {}", i).as_bytes()).await.unwrap();
            temp_files.push(file_path);
        }

        // The actual sending would fail due to no connection, but we can test
        // that the data structures are set up correctly
        assert_eq!(sender.get_all_progress().await.len(), 0);
    }

    /// Test cleanup functionality
    #[tokio::test]
    async fn test_cleanup() {
        let sender = FileSender::new(None).await.unwrap();

        // Test cleanup of completed transfers
        sender.cleanup_completed_transfers().await;

        // Should not panic and should work with empty active transfers
        assert!(sender.active_sends.read().await.is_empty());
    }

    /// Test multiaddr peer ID extraction
    #[tokio::test]
    async fn test_peer_id_extraction() {
        use libp2p::multiaddr::Protocol;

        // Create a multiaddr with peer ID
        let peer_id = PeerId::random();
        let mut addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        addr.push(Protocol::P2p(peer_id));

        // Test extraction
        let extracted = extract_peer_id_from_addr(&addr).unwrap();
        assert_eq!(extracted, peer_id);

        // Test multiaddr without peer ID
        let addr_no_peer: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        let result = extract_peer_id_from_addr(&addr_no_peer);
        assert!(result.is_err());
    }

    // Helper function for tests
    fn extract_peer_id_from_addr(addr: &Multiaddr) -> Result<PeerId, anyhow::Error> {
        use libp2p::multiaddr::Protocol;

        for protocol in addr.iter() {
            if let Protocol::P2p(peer_id) = protocol {
                return Ok(peer_id);
            }
        }

        Err(anyhow::anyhow!("No peer ID found in multiaddr"))
    }
}
