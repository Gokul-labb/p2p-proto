// Example usage scenarios for the file sender

use anyhow::Result;
use file_sender::{FileSender, RetryConfig, progress::ProgressReporter};
use libp2p::{Multiaddr, PeerId};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

/// Example 1: Simple file sending with progress tracking
pub async fn example_simple_send() -> Result<()> {
    info!("📤 Example: Simple file sending");

    // Create sender with custom retry config
    let retry_config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(200),
        max_delay: Duration::from_secs(10),
        backoff_multiplier: 1.5,
        connection_timeout: Duration::from_secs(15),
    };

    let mut sender = FileSender::new(Some(retry_config)).await?;

    // Set up simple progress callback
    sender.set_progress_callback(|progress| {
        println!("📊 {}: {:.1}% complete - {}", 
                 progress.file_path.file_name().unwrap().to_str().unwrap(),
                 progress.percentage(),
                 progress.status_string());
    });

    // Start sender background task
    let sender_task = tokio::spawn(async move {
        if let Err(e) = sender.run().await {
            eprintln!("Sender error: {}", e);
        }
    });

    sleep(Duration::from_millis(100)).await;

    // Send a file
    let peer_id = PeerId::random(); // In practice, extract from multiaddr
    let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080/p2p/12D3K...".parse()?;

    let transfer_id = sender.send_file(
        peer_id,
        target_addr,
        "document.pdf",
        Some("txt".to_string()), // Convert PDF to text
        true, // Request result back
    ).await?;

    // Monitor progress
    loop {
        if let Some(progress) = sender.get_progress(&transfer_id).await {
            match progress.status {
                file_sender::TransferStatus::Completed => {
                    info!("✅ Transfer completed successfully!");
                    break;
                }
                file_sender::TransferStatus::Failed(ref error) => {
                    info!("❌ Transfer failed: {}", error);
                    break;
                }
                _ => {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        } else {
            break;
        }
    }

    sender_task.abort();
    Ok(())
}

/// Example 2: Batch file sending with concurrent transfers
pub async fn example_batch_send() -> Result<()> {
    info!("📦 Example: Batch file sending");

    let mut sender = FileSender::new(None).await?;

    // Advanced progress reporter
    let mut reporter = ProgressReporter::new(Duration::from_millis(500));

    sender.set_progress_callback(move |progress| {
        // Only report significant progress changes
        if matches!(progress.status, 
                   file_sender::TransferStatus::Sending | 
                   file_sender::TransferStatus::Completed |
                   file_sender::TransferStatus::Failed(_)) {
            reporter.maybe_report(progress);
        }
    });

    let peer_id = PeerId::random();
    let target_addr: Multiaddr = "/ip4/192.168.1.100/tcp/9000/p2p/12D3K...".parse()?;

    // List of files to send
    let files = vec![
        ("report.txt", Some("pdf".to_string())),
        ("data.csv", None),
        ("presentation.pdf", Some("txt".to_string())),
        ("image.jpg", None),
    ];

    let mut transfer_ids = Vec::new();

    // Start all transfers
    for (file_path, format) in &files {
        match sender.send_file(
            peer_id,
            target_addr.clone(),
            file_path,
            format.clone(),
            false,
        ).await {
            Ok(transfer_id) => {
                transfer_ids.push(transfer_id);
                info!("🚀 Started transfer for {}", file_path);
            }
            Err(e) => {
                info!("❌ Failed to start transfer for {}: {}", file_path, e);
            }
        }

        // Small delay between starts to avoid overwhelming
        sleep(Duration::from_millis(100)).await;
    }

    // Wait for all transfers to complete
    let mut results = Vec::new();
    for transfer_id in transfer_ids {
        match sender.wait_for_completion(&transfer_id).await {
            Ok(result) => results.push(result),
            Err(e) => info!("❌ Error waiting for {}: {}", transfer_id, e),
        }
    }

    // Report final statistics
    let successful = results.iter().filter(|r| r.success).count();
    let total_bytes: u64 = results.iter().map(|r| r.bytes_sent).sum();
    let total_duration = results.iter().map(|r| r.duration).max().unwrap_or_default();

    info!("📊 Batch completed: {}/{} successful", successful, results.len());
    info!("📊 Total bytes sent: {} bytes", total_bytes);
    info!("📊 Total time: {:?}", total_duration);

    Ok(())
}

/// Example 3: Resilient sending with error handling
pub async fn example_resilient_send() -> Result<()> {
    info!("🛡️  Example: Resilient file sending");

    // Configure aggressive retry settings
    let retry_config = RetryConfig {
        max_attempts: 10,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(60),
        backoff_multiplier: 2.0,
        connection_timeout: Duration::from_secs(5),
    };

    let mut sender = FileSender::new(Some(retry_config)).await?;

    // Detailed progress tracking
    sender.set_progress_callback(|progress| {
        match &progress.status {
            file_sender::TransferStatus::Connecting => {
                info!("🔄 Connecting to peer (attempt {})", progress.connection_attempts);
            }
            file_sender::TransferStatus::Sending => {
                if progress.chunks_sent % 10 == 0 || progress.chunks_sent == progress.total_chunks {
                    info!("📤 Sent chunk {}/{} ({:.1}%)", 
                          progress.chunks_sent, progress.total_chunks, progress.percentage());
                }
            }
            file_sender::TransferStatus::Failed(error) => {
                info!("❌ Transfer failed: {}", error);
                if let Some(ref last_error) = progress.last_error {
                    info!("🔍 Last error: {}", last_error);
                }
            }
            file_sender::TransferStatus::Completed => {
                info!("✅ Transfer completed in {:?}", progress.start_time.elapsed());
            }
            _ => {}
        }
    });

    // Try to send to multiple potential peers
    let potential_peers = vec![
        "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWPeer1...".parse()?,
        "/ip4/192.168.1.100/tcp/9000/p2p/12D3KooWPeer2...".parse()?,
        "/ip4/10.0.0.50/tcp/7000/p2p/12D3KooWPeer3...".parse()?,
    ];

    let file_path = "important_document.pdf";

    for (i, target_addr) in potential_peers.iter().enumerate() {
        info!("🎯 Attempting peer {} of {}", i + 1, potential_peers.len());

        let peer_id = extract_peer_id_from_multiaddr(target_addr)?;

        match sender.send_file(
            peer_id,
            target_addr.clone(),
            file_path,
            Some("txt".to_string()),
            true,
        ).await {
            Ok(transfer_id) => {
                info!("🚀 Transfer started: {}", transfer_id);

                // Wait for completion with timeout
                match tokio::time::timeout(
                    Duration::from_secs(300), // 5 minute timeout
                    sender.wait_for_completion(&transfer_id)
                ).await {
                    Ok(Ok(result)) if result.success => {
                        info!("🎉 Successfully sent to peer {} in {:?}", 
                              i + 1, result.duration);
                        return Ok(());
                    }
                    Ok(Ok(result)) => {
                        info!("❌ Transfer to peer {} failed: {}", 
                              i + 1, result.error.unwrap_or_else(|| "Unknown".to_string()));
                    }
                    Ok(Err(e)) => {
                        info!("❌ Error with peer {}: {}", i + 1, e);
                    }
                    Err(_) => {
                        info!("⏰ Timeout waiting for peer {}", i + 1);
                        sender.cancel_transfer(&transfer_id).await?;
                    }
                }
            }
            Err(e) => {
                info!("❌ Failed to start transfer to peer {}: {}", i + 1, e);
            }
        }

        // Wait before trying next peer
        if i < potential_peers.len() - 1 {
            info!("⏳ Waiting before trying next peer...");
            sleep(Duration::from_secs(2)).await;
        }
    }

    info!("❌ All peers failed, giving up");
    Ok(())
}

/// Example 4: Real-time progress monitoring
pub async fn example_progress_monitoring() -> Result<()> {
    info!("📊 Example: Real-time progress monitoring");

    let mut sender = FileSender::new(None).await?;

    // Spawn a separate task for progress monitoring
    let sender_progress = Arc::clone(&sender.active_sends);
    let monitor_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            interval.tick().await;

            let active_sends = sender_progress.read().await;
            if active_sends.is_empty() {
                continue;
            }

            println!("\n📊 === Transfer Status ===");
            for (transfer_id, active_send) in active_sends.iter() {
                let progress = &active_send.progress;
                let short_id = &transfer_id[..8];

                println!("Transfer {}: {} -> {}", 
                         short_id, 
                         progress.file_path.file_name().unwrap().to_str().unwrap(),
                         progress.peer_id);
                println!("  Status: {}", progress.status_string());
                println!("  Progress: {:.1}% ({}/{} bytes)", 
                         progress.percentage(), progress.sent_bytes, progress.total_size);

                if progress.sent_bytes > 0 {
                    println!("  Speed: {:.1} KB/s", progress.speed_bps() / 1024.0);
                    if let Some(eta) = progress.eta_seconds() {
                        println!("  ETA: {:.0} seconds", eta);
                    }
                }
                println!();
            }
        }
    });

    // Send large file to demonstrate progress
    let peer_id = PeerId::random();
    let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080/p2p/12D3K...".parse()?;

    let transfer_id = sender.send_file(
        peer_id,
        target_addr,
        "large_video.mp4", // Simulated large file
        None,
        false,
    ).await?;

    // Wait for completion
    let result = sender.wait_for_completion(&transfer_id).await?;

    monitor_task.abort();

    info!("📊 Final result: {} bytes in {:?}", result.bytes_sent, result.duration);
    Ok(())
}

// Helper function to extract peer ID from multiaddr
fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Result<PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Ok(peer_id);
        }
    }

    Err(anyhow::anyhow!("No peer ID found in multiaddr"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_example_setup() {
        // Test that we can create a sender for examples
        let sender = FileSender::new(None).await;
        assert!(sender.is_ok());
    }
}
