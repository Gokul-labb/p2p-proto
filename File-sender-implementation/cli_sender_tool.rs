// CLI tool for sending files using P2P file sender
use anyhow::Result;
use clap::{Parser, Subcommand};
use file_sender::{FileSender, RetryConfig, progress::ProgressReporter};
use libp2p::{Multiaddr, PeerId};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "p2p-send")]
#[command(about = "P2P file sender with retry logic and progress tracking")]
#[command(version = "1.0.0")]
struct Args {
    /// Target peer multiaddress (must include peer ID)
    #[arg(short, long)]
    target: Multiaddr,

    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Maximum retry attempts
    #[arg(long, default_value = "5")]
    max_retries: usize,

    /// Connection timeout in seconds
    #[arg(long, default_value = "10")]
    timeout: u64,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a single file
    File {
        /// Path to file to send
        #[arg(short, long)]
        path: PathBuf,

        /// Target conversion format
        #[arg(short, long)]
        format: Option<String>,

        /// Request converted result back
        #[arg(long)]
        return_result: bool,
    },

    /// Send multiple files
    Batch {
        /// Directory containing files
        #[arg(short, long)]
        dir: PathBuf,

        /// File pattern (e.g., "*.txt")
        #[arg(short, long, default_value = "*")]
        pattern: String,

        /// Target conversion format for all files
        #[arg(long)]
        format: Option<String>,

        /// Maximum concurrent transfers
        #[arg(long, default_value = "3")]
        concurrent: usize,
    },

    /// Test connection to peer
    Ping {
        /// Number of ping attempts
        #[arg(short, long, default_value = "3")]
        count: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("{},libp2p=info", log_level)))
        )
        .init();

    info!("🚀 P2P File Sender v1.0.0");

    // Extract peer ID from multiaddr
    let peer_id = extract_peer_id(&args.target)?;
    info!("🎯 Target peer: {}", peer_id);
    info!("📡 Target address: {}", args.target);

    // Configure retry settings
    let retry_config = RetryConfig {
        max_attempts: args.max_retries,
        connection_timeout: Duration::from_secs(args.timeout),
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(30),
        backoff_multiplier: 2.0,
    };

    // Create file sender
    let mut sender = FileSender::new(Some(retry_config)).await?;

    // Set up progress reporting  
    let mut progress_reporter = ProgressReporter::new(Duration::from_secs(1));
    sender.set_progress_callback(move |progress| {
        progress_reporter.report(progress);
    });

    // Start sender event loop in background
    let sender_handle = tokio::spawn(async move {
        if let Err(e) = sender.run().await {
            warn!("Sender event loop error: {}", e);
        }
    });

    // Wait for sender to initialize
    sleep(Duration::from_millis(100)).await;

    // Execute command
    match args.command {
        Commands::File { path, format, return_result } => {
            send_single_file(&mut sender, peer_id, &args.target, path, format, return_result).await?;
        }

        Commands::Batch { dir, pattern, format, concurrent } => {
            send_batch_files(&mut sender, peer_id, &args.target, dir, pattern, format, concurrent).await?;
        }

        Commands::Ping { count } => {
            ping_peer(&mut sender, peer_id, &args.target, count).await?;
        }
    }

    // Cleanup
    sender_handle.abort();
    info!("👋 File sender stopped");
    Ok(())
}

/// Send a single file
async fn send_single_file(
    sender: &mut FileSender,
    peer_id: PeerId,
    target_addr: &Multiaddr,
    file_path: PathBuf,
    target_format: Option<String>,
    return_result: bool,
) -> Result<()> {
    info!("📤 Sending file: {}", file_path.display());

    if let Some(ref format) = target_format {
        info!("🔄 Requesting conversion to: {}", format);
    }

    let transfer_id = sender.send_file(
        peer_id,
        target_addr.clone(),
        &file_path,
        target_format,
        return_result,
    ).await?;

    info!("✅ Transfer initiated: {}", transfer_id);

    // Wait for completion
    let result = sender.wait_for_completion(&transfer_id).await?;

    if result.success {
        info!("🎉 Transfer completed successfully!");
        info!("📊 Sent {} bytes in {:?}", result.bytes_sent, result.duration);

        if let Some(response) = result.response {
            info!("📝 Server processing time: {}ms", response.processing_time_ms);
            if let Some(converted_filename) = response.converted_filename {
                info!("🔄 Converted to: {}", converted_filename);
            }
        }
    } else {
        warn!("❌ Transfer failed: {}", result.error.unwrap_or_else(|| "Unknown error".to_string()));
        info!("📊 Partial transfer: {} bytes in {:?}", result.bytes_sent, result.duration);
    }

    Ok(())
}

/// Send multiple files in batch
async fn send_batch_files(
    sender: &mut FileSender,
    peer_id: PeerId,
    target_addr: &Multiaddr,
    dir: PathBuf,
    pattern: String,
    target_format: Option<String>,
    max_concurrent: usize,
) -> Result<()> {
    use tokio::fs;
    use futures::stream::{self, StreamExt};

    info!("📁 Scanning directory: {}", dir.display());

    // Find matching files
    let mut files = Vec::new();
    let mut dir_entries = fs::read_dir(&dir).await?;

    while let Some(entry) = dir_entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");

            if pattern == "*" || filename.contains(&pattern) {
                files.push(path);
            }
        }
    }

    if files.is_empty() {
        info!("ℹ️  No files found matching pattern: {}", pattern);
        return Ok(());
    }

    info!("📦 Found {} files to transfer", files.len());

    // Process files with limited concurrency
    let results: Vec<Result<String>> = stream::iter(files)
        .map(|file_path| {
            let target_format = target_format.clone();
            async move {
                info!("📤 Starting: {}", file_path.display());
                sender.send_file(
                    peer_id,
                    target_addr.clone(),
                    &file_path,
                    target_format,
                    false,
                ).await
            }
        })
        .buffer_unordered(max_concurrent)
        .collect()
        .await;

    // Collect transfer IDs
    let mut transfer_ids = Vec::new();
    let mut failed_starts = 0;

    for result in results {
        match result {
            Ok(transfer_id) => transfer_ids.push(transfer_id),
            Err(e) => {
                warn!("❌ Failed to start transfer: {}", e);
                failed_starts += 1;
            }
        }
    }

    info!("📊 Started {} transfers, {} failed to start", transfer_ids.len(), failed_starts);

    // Wait for all transfers to complete
    let mut successful = 0;
    let mut failed = 0;
    let mut total_bytes = 0;

    for transfer_id in transfer_ids {
        match sender.wait_for_completion(&transfer_id).await {
            Ok(result) => {
                total_bytes += result.bytes_sent;
                if result.success {
                    successful += 1;
                    info!("✅ Transfer {} completed", transfer_id[..8].to_string());
                } else {
                    failed += 1;
                    warn!("❌ Transfer {} failed: {}", 
                          transfer_id[..8].to_string(),
                          result.error.unwrap_or_else(|| "Unknown error".to_string()));
                }
            }
            Err(e) => {
                failed += 1;
                warn!("❌ Error waiting for transfer {}: {}", transfer_id[..8].to_string(), e);
            }
        }
    }

    info!("🎉 Batch transfer completed!");
    info!("📊 Results: {} successful, {} failed", successful, failed);
    info!("📊 Total bytes sent: {}", total_bytes);

    Ok(())
}

/// Test connection to peer
async fn ping_peer(
    sender: &mut FileSender,
    peer_id: PeerId,
    target_addr: &Multiaddr,
    count: usize,
) -> Result<()> {
    info!("🏓 Pinging peer {} ({} attempts)", peer_id, count);

    for attempt in 1..=count {
        info!("🏓 Ping attempt {}/{}", attempt, count);

        // Create a small test file
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp_file = NamedTempFile::new()?;
        write!(temp_file, "ping test data for attempt {}", attempt)?;

        let start_time = std::time::Instant::now();

        match sender.send_file(
            peer_id,
            target_addr.clone(),
            temp_file.path(),
            None,
            false,
        ).await {
            Ok(transfer_id) => {
                match sender.wait_for_completion(&transfer_id).await {
                    Ok(result) if result.success => {
                        let rtt = start_time.elapsed();
                        info!("✅ Ping {}: RTT = {:?}", attempt, rtt);
                    }
                    Ok(result) => {
                        warn!("❌ Ping {} failed: {}", attempt, 
                              result.error.unwrap_or_else(|| "Unknown error".to_string()));
                    }
                    Err(e) => {
                        warn!("❌ Ping {} error: {}", attempt, e);
                    }
                }
            }
            Err(e) => {
                warn!("❌ Ping {} connection failed: {}", attempt, e);
            }
        }

        // Wait between attempts
        if attempt < count {
            sleep(Duration::from_secs(1)).await;
        }
    }

    info!("🏓 Ping test completed");
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
