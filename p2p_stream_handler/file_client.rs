// P2P File Transfer Client - Sender Implementation

use anyhow::Result;
use clap::{Parser, Subcommand};
use libp2p::{Multiaddr, PeerId};
use p2p_file_transfer::{FileConversionConfig, P2PFileNode};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "p2p-file-client")]
#[command(about = "P2P file transfer client")]
#[command(version = "0.2.0")]
struct Args {
    /// Peer address to connect to
    #[arg(short, long)]
    peer: Multiaddr,

    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a file to the peer
    Send {
        /// File path to send
        #[arg(short, long)]
        file: PathBuf,

        /// Target conversion format (pdf, txt)
        #[arg(short, long)]
        format: Option<String>,

        /// Request result back from peer
        #[arg(long)]
        return_result: bool,
    },

    /// Send multiple files
    Batch {
        /// Directory containing files to send
        #[arg(short, long)]
        dir: PathBuf,

        /// File pattern to match (e.g., "*.txt")
        #[arg(short, long, default_value = "*")]
        pattern: String,

        /// Target conversion format
        #[arg(long)]
        format: Option<String>,

        /// Maximum concurrent transfers
        #[arg(long, default_value = "3")]
        concurrent: usize,
    },

    /// Get peer information
    Info,
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

    info!("📡 P2P File Transfer Client v0.2.0");

    // Create client node
    let config = FileConversionConfig::default();
    let mut node = P2PFileNode::new(config).await?;

    // Start the node in background
    let listen_addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    tokio::spawn(async move {
        if let Err(e) = node.run(listen_addr).await {
            error!("Node error: {}", e);
        }
    });

    // Give node time to start
    sleep(Duration::from_secs(1)).await;

    // Extract peer ID from multiaddr
    let peer_id = extract_peer_id(&args.peer)?;

    match args.command {
        Commands::Send { file, format, return_result } => {
            send_single_file(&mut node, peer_id, file, format, return_result).await?;
        }

        Commands::Batch { dir, pattern, format, concurrent } => {
            send_batch_files(&mut node, peer_id, dir, pattern, format, concurrent).await?;
        }

        Commands::Info => {
            get_peer_info(&mut node, peer_id).await?;
        }
    }

    Ok(())
}

/// Send a single file to peer
async fn send_single_file(
    node: &mut P2PFileNode,
    peer_id: PeerId,
    file_path: PathBuf,
    target_format: Option<String>,
    return_result: bool,
) -> Result<()> {
    info!("📤 Sending file: {} to {}", file_path.display(), peer_id);

    if let Some(ref format) = target_format {
        info!("🔄 Requesting conversion to: {}", format);
    }

    let transfer_id = node.send_file(peer_id, &file_path, target_format).await?;

    info!("✅ File transfer initiated: {}", transfer_id);
    info!("📊 Monitoring progress...");

    // Monitor progress
    for _ in 0..60 { // Monitor for up to 5 minutes
        sleep(Duration::from_secs(5)).await;

        let progress_list = node.get_progress().await;
        if let Some(progress) = progress_list.iter().find(|p| p.transfer_id == transfer_id) {
            info!(
                "📈 Progress: {:.1}% ({}/{} bytes) - {:.1} KB/s",
                progress.percentage(),
                progress.transferred,
                progress.total_size,
                progress.speed_bps() / 1024.0
            );
        } else {
            info!("✅ Transfer completed!");
            break;
        }
    }

    Ok(())
}

/// Send multiple files in batch
async fn send_batch_files(
    node: &mut P2PFileNode,
    peer_id: PeerId,
    dir: PathBuf,
    pattern: String,
    target_format: Option<String>,
    max_concurrent: usize,
) -> Result<()> {
    use tokio::fs;
    use futures::stream::{self, StreamExt};

    info!("📁 Scanning directory: {}", dir.display());
    info!("🔍 Pattern: {}", pattern);

    // Find matching files
    let mut files = Vec::new();
    let mut dir_entries = fs::read_dir(&dir).await?;

    while let Some(entry) = dir_entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if pattern == "*" || path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.contains(&pattern))
                .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }

    info!("📦 Found {} files to transfer", files.len());

    if files.is_empty() {
        info!("ℹ️  No files found matching pattern");
        return Ok(());
    }

    // Send files concurrently
    let results: Vec<Result<String>> = stream::iter(files)
        .map(|file_path| {
            let target_format = target_format.clone();
            async move {
                info!("📤 Sending: {}", file_path.display());
                node.send_file(peer_id, &file_path, target_format).await
            }
        })
        .buffer_unordered(max_concurrent)
        .collect()
        .await;

    // Report results
    let mut successful = 0;
    let mut failed = 0;

    for result in results {
        match result {
            Ok(transfer_id) => {
                successful += 1;
                info!("✅ Transfer started: {}", transfer_id);
            }
            Err(e) => {
                failed += 1;
                error!("❌ Transfer failed: {}", e);
            }
        }
    }

    info!("📊 Batch complete: {} successful, {} failed", successful, failed);
    Ok(())
}

/// Get information about peer
async fn get_peer_info(node: &mut P2PFileNode, peer_id: PeerId) -> Result<()> {
    info!("ℹ️  Peer ID: {}", peer_id);
    info!("🔍 Attempting to connect and gather information...");

    // TODO: Implement peer info gathering
    // This could include supported protocols, node version, etc.

    Ok(())
}

/// Extract PeerId from multiaddr
fn extract_peer_id(addr: &Multiaddr) -> Result<PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Ok(peer_id);
        }
    }

    Err(anyhow::anyhow!("No peer ID found in multiaddr: {}", addr))
}
