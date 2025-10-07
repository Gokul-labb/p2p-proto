//! Simple sender example
//! 
//! Demonstrates how to send a file to a P2P peer.

use anyhow::Result;
use p2p_file_converter::prelude::*;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,libp2p=debug")
        .init();

    println!("📤 P2P File Converter - Sender Mode Example");

    // Parse command line arguments
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!("Usage: {} <target_multiaddr> <file_path> [format]", args[0]);
        eprintln!("Example: {} /ip4/127.0.0.1/tcp/8080/p2p/12D3K... document.txt pdf", args[0]);
        std::process::exit(1);
    }

    let target_addr: Multiaddr = args[1].parse()
        .context("Invalid multiaddress format")?;
    let file_path = PathBuf::from(&args[2]);
    let target_format = args.get(3).map(|s| s.to_string());

    // Extract peer ID
    let peer_id = extract_peer_id(&target_addr)?;

    println!("🎯 Target: {}", target_addr);
    println!("📄 File: {}", file_path.display());
    if let Some(ref format) = target_format {
        println!("🔄 Convert to: {}", format);
    }

    // Create file sender
    let retry_config = RetryConfig {
        max_attempts: 5,
        initial_delay: std::time::Duration::from_millis(500),
        max_delay: std::time::Duration::from_secs(30),
        backoff_multiplier: 2.0,
        connection_timeout: std::time::Duration::from_secs(15),
    };

    let mut sender = FileSender::new(Some(retry_config)).await?;

    // Set up progress callback
    sender.set_progress_callback(|progress| {
        match &progress.status {
            TransferStatus::Connecting => {
                println!("🔄 Connecting to peer (attempt {})...", progress.connection_attempts);
            }
            TransferStatus::Negotiating => {
                println!("🤝 Negotiating protocol...");
            }
            TransferStatus::Sending => {
                if progress.chunks_sent % 5 == 0 || progress.chunks_sent == progress.total_chunks {
                    println!("📤 Progress: {:.1}% ({}/{} chunks, {:.1} KB/s)", 
                             progress.percentage(),
                             progress.chunks_sent, 
                             progress.total_chunks,
                             progress.speed_bps() / 1024.0);
                }
            }
            TransferStatus::WaitingResponse => {
                println!("⏳ Waiting for server response...");
            }
            TransferStatus::Completed => {
                println!("✅ Transfer completed successfully!");
            }
            TransferStatus::Failed(error) => {
                println!("❌ Transfer failed: {}", error);
            }
            TransferStatus::Cancelled => {
                println!("🚫 Transfer cancelled");
            }
        }
    });

    // Start sender event loop in background
    let sender_task = tokio::spawn(async move {
        if let Err(e) = sender.run().await {
            eprintln!("Sender error: {}", e);
        }
    });

    // Give sender time to initialize
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send the file
    println!("🚀 Starting file transfer...");
    let transfer_id = sender.send_file(
        peer_id,
        target_addr,
        &file_path,
        target_format,
        false, // Don't return result
    ).await?;

    println!("📋 Transfer ID: {}", transfer_id);

    // Wait for completion
    match sender.wait_for_completion(&transfer_id).await {
        Ok(result) if result.success => {
            println!("🎉 Success! Sent {} bytes in {:?}", 
                     result.bytes_sent, result.duration);

            if let Some(response) = result.response {
                println!("⚡ Server processing time: {}ms", response.processing_time_ms);
            }
        }
        Ok(result) => {
            println!("❌ Transfer failed: {}", 
                     result.error.unwrap_or_else(|| "Unknown error".to_string()));
            println!("📊 Partial transfer: {} bytes", result.bytes_sent);
            std::process::exit(1);
        }
        Err(e) => {
            println!("💥 Error: {}", e);
            std::process::exit(1);
        }
    }

    // Cleanup
    sender_task.abort();
    println!("👋 Sender finished");
    Ok(())
}

/// Extract peer ID from multiaddr
fn extract_peer_id(addr: &Multiaddr) -> Result<PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Ok(peer_id);
        }
    }

    Err(anyhow::anyhow!("No peer ID found in multiaddr: {}", addr))
}
