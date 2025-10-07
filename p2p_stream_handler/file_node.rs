// P2P File Transfer Node - Server/Receiver Implementation

use anyhow::Result;
use clap::Parser;
use libp2p::{Multiaddr, PeerId};
use p2p_file_transfer::{
    FileConversionConfig, FileConversionService, 
    P2PFileNode, TransferProgress
};
use std::path::PathBuf;
use tokio::{signal, time::{interval, Duration}};
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "p2p-file-node")]
#[command(about = "P2P file transfer and conversion node")]
#[command(version = "0.2.0")]
struct Args {
    /// Address to listen on
    #[arg(short, long, default_value = "/ip4/0.0.0.0/tcp/0")]
    listen: Multiaddr,

    /// Output directory for received files
    #[arg(short, long, default_value = "./received")]
    output_dir: PathBuf,

    /// Maximum concurrent transfers
    #[arg(long, default_value = "5")]
    max_transfers: usize,

    /// Enable auto-conversion of received files
    #[arg(long)]
    auto_convert: bool,

    /// Return conversion results to sender
    #[arg(long)]
    return_results: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,
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

    info!("🚀 Starting P2P File Transfer Node v0.2.0");

    // Load or create configuration
    let config = if let Some(config_path) = args.config {
        load_config(&config_path).await?
    } else {
        FileConversionConfig {
            max_concurrent_transfers: args.max_transfers,
            output_dir: args.output_dir,
            auto_convert: args.auto_convert,
            return_results: args.return_results,
            ..Default::default()
        }
    };

    info!("📁 Output directory: {}", config.output_dir.display());
    info!("🔄 Auto-conversion: {}", config.auto_convert);
    info!("📊 Max concurrent transfers: {}", config.max_concurrent_transfers);

    // Create and start the P2P node
    let mut node = P2PFileNode::new(config).await?;

    // Start progress reporting task
    let progress_service = node.service.clone();
    let progress_task = tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            let transfers = progress_service.get_transfer_progress().await;

            if !transfers.is_empty() {
                info!("📈 Active Transfers:");
                for transfer in transfers {
                    info!(
                        "  {} -> {} ({:.1}% complete, {:.1} KB/s)",
                        transfer.peer_id,
                        transfer.filename,
                        transfer.percentage(),
                        transfer.speed_bps() / 1024.0
                    );

                    if let Some(eta) = transfer.eta_seconds() {
                        info!("    ETA: {:.0} seconds", eta);
                    }
                }
            }
        }
    });

    // Handle shutdown gracefully
    let shutdown_signal = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        info!("🛑 Shutdown signal received");
    };

    // Run the node or handle shutdown
    tokio::select! {
        result = node.run(args.listen) => {
            if let Err(e) = result {
                error!("❌ Node error: {}", e);
                return Err(e);
            }
        }
        _ = shutdown_signal => {
            info!("👋 Shutting down gracefully...");
            progress_task.abort();
        }
    }

    info!("✅ P2P File Transfer Node stopped");
    Ok(())
}

/// Load configuration from file
async fn load_config(config_path: &PathBuf) -> Result<FileConversionConfig> {
    use std::fs;

    let config_content = fs::read_to_string(config_path)?;
    let config: FileConversionConfig = toml::from_str(&config_content)?;

    info!("📄 Loaded configuration from {}", config_path.display());
    Ok(config)
}
