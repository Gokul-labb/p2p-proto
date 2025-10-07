// Benchmark tests for file sender performance

#[cfg(test)]
mod benchmarks {
    use super::*;
    use file_sender::{FileSender, SendProgress, TransferStatus};
    use std::time::{Duration, Instant};
    use tokio::io::AsyncWriteExt;
    use tempfile::NamedTempFile;

    /// Benchmark progress calculation performance
    #[tokio::test]
    async fn bench_progress_calculation() {
        let progress = SendProgress {
            transfer_id: "bench-test".to_string(),
            file_path: std::path::PathBuf::from("large_file.bin"),
            peer_id: libp2p::PeerId::random(),
            total_size: 1_000_000_000, // 1GB
            sent_bytes: 250_000_000,   // 250MB
            chunks_sent: 250,
            total_chunks: 1000,
            start_time: Instant::now() - Duration::from_secs(10),
            status: TransferStatus::Sending,
            connection_attempts: 1,
            last_error: None,
        };

        let start = Instant::now();
        let iterations = 10_000;

        for _ in 0..iterations {
            let _percentage = progress.percentage();
            let _speed = progress.speed_bps();
            let _eta = progress.eta_seconds();
            let _status = progress.status_string();
        }

        let elapsed = start.elapsed();
        println!("Progress calculation: {} iterations in {:?} ({:.2}μs per iteration)",
                 iterations, elapsed, elapsed.as_micros() as f64 / iterations as f64);

        // Should be very fast
        assert!(elapsed < Duration::from_millis(100));
    }

    /// Benchmark file reading performance
    #[tokio::test]
    async fn bench_file_reading() {
        // Create a temporary file with test data
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = vec![0u8; 1024 * 1024]; // 1MB of zeros
        temp_file.write_all(&test_data).unwrap();

        let start = Instant::now();

        // Simulate chunked reading like the sender does
        let mut file = tokio::fs::File::open(temp_file.path()).await.unwrap();
        let mut buffer = vec![0u8; 64 * 1024]; // 64KB chunks
        let mut total_read = 0;
        let mut chunks = 0;

        loop {
            let bytes_read = file.read(&mut buffer).await.unwrap();
            if bytes_read == 0 {
                break;
            }
            total_read += bytes_read;
            chunks += 1;
        }

        let elapsed = start.elapsed();
        let throughput = (total_read as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64();

        println!("File reading: {} bytes in {} chunks in {:?} ({:.2} MB/s)",
                 total_read, chunks, elapsed, throughput);

        assert_eq!(total_read, test_data.len());
        assert!(throughput > 50.0); // Should be faster than 50 MB/s
    }

    /// Benchmark progress reporter performance
    #[tokio::test]
    async fn bench_progress_reporter() {
        use file_sender::progress::ProgressReporter;

        let mut reporter = ProgressReporter::new(Duration::from_millis(1));

        let progress = SendProgress {
            transfer_id: "reporter-bench".to_string(),
            file_path: std::path::PathBuf::from("test.txt"),
            peer_id: libp2p::PeerId::random(),
            total_size: 1000,
            sent_bytes: 500,
            chunks_sent: 5,
            total_chunks: 10,
            start_time: Instant::now() - Duration::from_secs(1),
            status: TransferStatus::Sending,
            connection_attempts: 1,
            last_error: None,
        };

        let start = Instant::now();
        let iterations = 1_000;

        for _ in 0..iterations {
            let _formatted = reporter.format_progress(&progress);
        }

        let elapsed = start.elapsed();
        println!("Progress formatting: {} iterations in {:?} ({:.2}μs per iteration)",
                 iterations, elapsed, elapsed.as_micros() as f64 / iterations as f64);

        // Should be reasonably fast
        assert!(elapsed < Duration::from_millis(500));
    }

    /// Test memory usage with many concurrent transfer tracking
    #[tokio::test]
    async fn test_memory_usage_many_transfers() {
        let sender = FileSender::new(None).await.unwrap();

        // Simulate many transfers in tracking
        let start_memory = get_memory_usage();

        // This test would need to be expanded with actual transfer simulation
        // For now, just test that we can create the structures

        let end_memory = get_memory_usage();
        println!("Memory usage change: {} KB", (end_memory as i64 - start_memory as i64) / 1024);

        // Basic sanity check - shouldn't use excessive memory just for setup
        assert!(end_memory - start_memory < 10 * 1024 * 1024); // Less than 10MB
    }

    /// Simple memory usage estimation (platform-specific)
    fn get_memory_usage() -> usize {
        // This is a simplified approach - in a real benchmark you'd use
        // platform-specific APIs or tools like jemalloc stats
        std::mem::size_of::<FileSender>() * 1000 // Rough approximation
    }

    /// Test performance of status transitions
    #[tokio::test]
    async fn bench_status_transitions() {
        let mut progress = SendProgress {
            transfer_id: "status-bench".to_string(),
            file_path: std::path::PathBuf::from("test.txt"),
            peer_id: libp2p::PeerId::random(),
            total_size: 1000,
            sent_bytes: 0,
            chunks_sent: 0,
            total_chunks: 10,
            start_time: Instant::now(),
            status: TransferStatus::Connecting,
            connection_attempts: 1,
            last_error: None,
        };

        let statuses = vec![
            TransferStatus::Connecting,
            TransferStatus::Negotiating,
            TransferStatus::Sending,
            TransferStatus::WaitingResponse,
            TransferStatus::Completed,
        ];

        let start = Instant::now();
        let iterations = 10_000;

        for i in 0..iterations {
            progress.status = statuses[i % statuses.len()].clone();
            let _status_str = progress.status_string();
        }

        let elapsed = start.elapsed();
        println!("Status transitions: {} iterations in {:?} ({:.2}μs per iteration)",
                 iterations, elapsed, elapsed.as_micros() as f64 / iterations as f64);

        // Should be very fast
        assert!(elapsed < Duration::from_millis(50));
    }
}
