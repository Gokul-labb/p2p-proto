//! Main event loop for the P2P file converter integrating all components
//! 
//! This module provides the central event loop that orchestrates:
//! - libp2p swarm events and peer discovery
//! - Incoming stream connections and protocol negotiations  
//! - User command input and CLI interactions
//! - File conversion and transfer operations
//! - Graceful shutdown and cleanup operations

use anyhow::{Context, Result};
use clap::Parser;
use futures::{
    future::{select, Either},
    stream::{StreamExt, FuturesUnordered},
    Future, FutureExt,
};
use libp2p::{
    swarm::{SwarmEvent, dial_opts::DialOpts},
    Multiaddr, PeerId, Swarm,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    select,
    signal,
    sync::{broadcast, mpsc, RwLock},
    task::JoinHandle,
    time::{interval, sleep},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Import all our components
use crate::{
    cli::{CliArgs, AppMode},
    file_converter::{FileConverter, FileType, PdfConfig},
    file_sender::{FileSender, RetryConfig, SendProgress, SendResult, TransferStatus},
    p2p_stream_handler::{
        FileConversionService, FileConversionConfig, FileTransferRequest, 
        FileTransferResponse, P2PFileNode, TransferProgress,
    },
};

/// Shutdown signal types
#[derive(Debug, Clone)]
pub enum ShutdownReason {
    /// User requested shutdown (Ctrl+C)
    UserInterrupt,
    /// CLI command to exit
    UserCommand,
    /// Transfer completed (sender mode)
    TransferComplete,
    /// Fatal error occurred
    Error(String),
    /// Timeout reached
    Timeout,
}

/// Event types in the main loop
#[derive(Debug)]
pub enum EventLoopEvent {
    /// User input from stdin
    UserInput(String),
    /// libp2p swarm event
    SwarmEvent(SwarmEvent<libp2p::swarm::behaviour::toggle::Toggle<libp2p::ping::Behaviour>>),
    /// File transfer progress update
    TransferProgress(TransferProgress),
    /// File conversion completed
    ConversionComplete {
        transfer_id: String,
        success: bool,
        output_path: Option<PathBuf>,
    },
    /// Peer discovery event
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    /// Connection status change
    ConnectionStatusChange {
        peer_id: PeerId,
        connected: bool,
        endpoint: Option<libp2p::core::ConnectedPoint>,
    },
    /// Shutdown signal received
    Shutdown(ShutdownReason),
}

/// Application state shared across components
#[derive(Debug)]
pub struct AppState {
    /// Current application mode
    pub mode: AppMode,
    /// CLI arguments
    pub args: CliArgs,
    /// Active file transfers (sender mode)
    pub active_transfers: Arc<RwLock<HashMap<String, SendProgress>>>,
    /// Connected peers
    pub connected_peers: Arc<RwLock<HashMap<PeerId, Vec<Multiaddr>>>>,
    /// Transfer statistics
    pub transfer_stats: Arc<RwLock<TransferStats>>,
    /// Shutdown flag
    pub shutdown_requested: Arc<RwLock<Option<ShutdownReason>>>,
    /// Start time for statistics
    pub start_time: Instant,
}

/// Transfer statistics
#[derive(Debug, Default)]
pub struct TransferStats {
    pub files_sent: u64,
    pub files_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub successful_transfers: u64,
    pub failed_transfers: u64,
    pub conversion_count: u64,
}

/// Main P2P file converter application
pub struct P2PFileConverter {
    /// Application state
    state: Arc<AppState>,
    /// File sender (for sender mode)
    file_sender: Option<FileSender>,
    /// P2P node (for receiver mode)
    p2p_node: Option<P2PFileNode>,
    /// File conversion service
    conversion_service: Arc<FileConversionService>,
    /// Event broadcast channel
    event_tx: broadcast::Sender<EventLoopEvent>,
    /// Shutdown sender
    shutdown_tx: mpsc::Sender<ShutdownReason>,
    shutdown_rx: mpsc::Receiver<ShutdownReason>,
    /// Background tasks
    background_tasks: Vec<JoinHandle<()>>,
}

impl P2PFileConverter {
    /// Create a new P2P file converter application
    pub async fn new() -> Result<Self> {
        // Parse CLI arguments and determine mode
        let (args, mode) = CliArgs::parse_args()?;

        // Setup logging
        args.setup_logging()?;

        info!("🚀 Starting P2P File Converter");
        args.print_config(&mode);

        // Create application state
        let state = Arc::new(AppState {
            mode: mode.clone(),
            args: args.clone(),
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            connected_peers: Arc::new(RwLock::new(HashMap::new())),
            transfer_stats: Arc::new(RwLock::new(TransferStats::default())),
            shutdown_requested: Arc::new(RwLock::new(None)),
            start_time: Instant::now(),
        });

        // Create event broadcast channel
        let (event_tx, _) = broadcast::channel(1000);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(10);

        // Create file conversion service
        let conversion_config = FileConversionConfig {
            max_concurrent_transfers: 5,
            output_dir: args.output_dir.clone(),
            auto_convert: true,
            return_results: false,
            pdf_config: PdfConfig::default(),
        };
        let conversion_service = Arc::new(FileConversionService::new(conversion_config)?);

        // Initialize sender or receiver based on mode
        let (file_sender, p2p_node) = match &mode {
            AppMode::Sender { .. } => {
                info!("📤 Initializing sender mode");
                let retry_config = RetryConfig {
                    max_attempts: 5,
                    initial_delay: Duration::from_millis(500),
                    max_delay: Duration::from_secs(30),
                    backoff_multiplier: 2.0,
                    connection_timeout: Duration::from_secs(15),
                };
                let sender = FileSender::new(Some(retry_config)).await?;
                (Some(sender), None)
            }
            AppMode::Receiver { .. } => {
                info!("📥 Initializing receiver mode");
                let node = P2PFileNode::new(conversion_config).await?;
                (None, Some(node))
            }
        };

        Ok(Self {
            state,
            file_sender,
            p2p_node,
            conversion_service,
            event_tx,
            shutdown_tx,
            shutdown_rx,
            background_tasks: Vec::new(),
        })
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<i32> {
        info!("🔄 Starting main event loop");

        // Start background tasks
        self.start_background_tasks().await?;

        // Setup shutdown signal handlers
        let shutdown_tx = self.shutdown_tx.clone();
        tokio::spawn(async move {
            if let Ok(()) = signal::ctrl_c().await {
                info!("📶 Received Ctrl+C, initiating shutdown");
                let _ = shutdown_tx.send(ShutdownReason::UserInterrupt).await;
            }
        });

        // Run mode-specific initialization
        match &self.state.mode {
            AppMode::Sender { target_addr, file_path, .. } => {
                self.run_sender_mode(target_addr.clone(), file_path.clone()).await
            }
            AppMode::Receiver { listen_addr, .. } => {
                self.run_receiver_mode(listen_addr.clone()).await
            }
        }
    }

    /// Run sender mode - send file and exit
    async fn run_sender_mode(&mut self, target_addr: Multiaddr, file_path: PathBuf) -> Result<i32> {
        info!("📤 Running in sender mode");

        // Extract peer ID from target address
        let peer_id = self.extract_peer_id(&target_addr)?;

        // Start file sender if available
        let mut sender = self.file_sender.take()
            .ok_or_else(|| anyhow::anyhow!("File sender not initialized"))?;

        // Setup progress callback
        let event_tx = self.event_tx.clone();
        let state = Arc::clone(&self.state);
        sender.set_progress_callback(move |progress| {
            // Update state
            let state = Arc::clone(&state);
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                state.active_transfers.write().await.insert(
                    progress.transfer_id.clone(),
                    progress.clone().into(), // Convert to our progress type
                );
                let _ = event_tx.send(EventLoopEvent::TransferProgress(progress.into()));
            });
        });

        // Start sender event loop in background
        let sender_handle = tokio::spawn(async move {
            if let Err(e) = sender.run().await {
                error!("Sender event loop error: {}", e);
            }
        });

        // Allow sender to initialize
        sleep(Duration::from_millis(100)).await;

        // Initiate file transfer
        let transfer_id = match sender.send_file(
            peer_id,
            target_addr.clone(),
            &file_path,
            self.state.args.target_format.clone(),
            false, // Don't return result for CLI mode
        ).await {
            Ok(id) => {
                info!("✅ Transfer initiated: {}", id);
                id
            }
            Err(e) => {
                error!("❌ Failed to initiate transfer: {}", e);
                sender_handle.abort();
                return Ok(1);
            }
        };

        // Main event loop for sender mode
        let mut exit_code = 0;
        let mut transfer_completed = false;

        loop {
            select! {
                // Handle shutdown signals
                shutdown_reason = self.shutdown_rx.recv() => {
                    if let Some(reason) = shutdown_reason {
                        info!("🛑 Shutdown requested: {:?}", reason);
                        match reason {
                            ShutdownReason::UserInterrupt => {
                                warn!("Cancelling transfer due to user interrupt");
                                if let Err(e) = sender.cancel_transfer(&transfer_id).await {
                                    warn!("Failed to cancel transfer: {}", e);
                                }
                                exit_code = 130; // Standard exit code for Ctrl+C
                            }
                            ShutdownReason::TransferComplete => {
                                info!("✅ Transfer completed successfully");
                                exit_code = 0;
                            }
                            ShutdownReason::Error(msg) => {
                                error!("❌ Transfer failed: {}", msg);
                                exit_code = 1;
                            }
                            _ => exit_code = 0,
                        }
                        break;
                    }
                }

                // Handle user input (for interactive commands during transfer)
                line = self.read_user_input() => {
                    if let Some(input) = line {
                        match input.trim() {
                            "status" => self.print_transfer_status().await,
                            "cancel" => {
                                info!("🚫 Cancelling transfer by user request");
                                if let Err(e) = sender.cancel_transfer(&transfer_id).await {
                                    warn!("Failed to cancel transfer: {}", e);
                                }
                                let _ = self.shutdown_tx.send(ShutdownReason::UserCommand).await;
                            }
                            "quit" | "exit" => {
                                let _ = self.shutdown_tx.send(ShutdownReason::UserCommand).await;
                            }
                            _ => {
                                info!("Available commands: status, cancel, quit");
                            }
                        }
                    }
                }

                // Check transfer completion periodically
                _ = sleep(Duration::from_secs(1)) => {
                    if !transfer_completed {
                        match sender.wait_for_completion(&transfer_id).await {
                            Ok(result) => {
                                transfer_completed = true;
                                self.handle_transfer_result(result).await;

                                if result.success {
                                    let _ = self.shutdown_tx.send(ShutdownReason::TransferComplete).await;
                                } else {
                                    let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                                    let _ = self.shutdown_tx.send(ShutdownReason::Error(error_msg)).await;
                                }
                            }
                            Err(e) => {
                                debug!("Transfer still in progress: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Cleanup
        sender_handle.abort();
        self.cleanup_background_tasks().await;

        info!("👋 Sender mode completed with exit code: {}", exit_code);
        Ok(exit_code)
    }

    /// Run receiver mode - listen indefinitely
    async fn run_receiver_mode(&mut self, listen_addr: Multiaddr) -> Result<i32> {
        info!("📥 Running in receiver mode");

        // Start P2P node if available
        let mut p2p_node = self.p2p_node.take()
            .ok_or_else(|| anyhow::anyhow!("P2P node not initialized"))?;

        // Start P2P node event loop in background
        let node_handle = tokio::spawn(async move {
            if let Err(e) = p2p_node.run(listen_addr).await {
                error!("P2P node error: {}", e);
            }
        });

        // Allow node to initialize
        sleep(Duration::from_millis(500)).await;

        info!("🌐 P2P node listening for incoming connections");
        info!("📋 Commands: status, peers, stats, quit");

        // Main event loop for receiver mode
        let mut exit_code = 0;

        loop {
            select! {
                // Handle shutdown signals
                shutdown_reason = self.shutdown_rx.recv() => {
                    if let Some(reason) = shutdown_reason {
                        info!("🛑 Shutdown requested: {:?}", reason);
                        match reason {
                            ShutdownReason::UserInterrupt | ShutdownReason::UserCommand => {
                                info!("👋 Graceful shutdown initiated");
                                exit_code = 0;
                            }
                            ShutdownReason::Error(msg) => {
                                error!("❌ Fatal error: {}", msg);
                                exit_code = 1;
                            }
                            _ => exit_code = 0,
                        }
                        break;
                    }
                }

                // Handle user input (interactive commands)
                line = self.read_user_input() => {
                    if let Some(input) = line {
                        if let Err(e) = self.handle_user_command(input.trim()).await {
                            error!("Command error: {}", e);
                        }
                    }
                }

                // Periodic maintenance tasks
                _ = sleep(Duration::from_secs(30)) => {
                    self.perform_maintenance().await;
                }
            }
        }

        // Cleanup
        node_handle.abort();
        self.cleanup_background_tasks().await;

        info!("👋 Receiver mode completed with exit code: {}", exit_code);
        Ok(exit_code)
    }

    /// Start background tasks
    async fn start_background_tasks(&mut self) -> Result<()> {
        info!("🔧 Starting background tasks");

        // Progress monitoring task
        let state = Arc::clone(&self.state);
        let progress_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                // Check if shutdown requested
                if state.shutdown_requested.read().await.is_some() {
                    break;
                }

                // Print active transfer progress
                let transfers = state.active_transfers.read().await;
                if !transfers.is_empty() {
                    info!("📊 Active transfers: {}", transfers.len());
                    for (id, progress) in transfers.iter() {
                        info!("  {} -> {:.1}% complete ({} KB/s)", 
                              &id[..8], 
                              progress.percentage(),
                              progress.speed_bps() / 1024.0);
                    }
                }
            }
        });
        self.background_tasks.push(progress_task);

        // Statistics collection task
        let state = Arc::clone(&self.state);
        let stats_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                if state.shutdown_requested.read().await.is_some() {
                    break;
                }

                // Log periodic statistics
                let stats = state.transfer_stats.read().await;
                let uptime = state.start_time.elapsed();

                info!("📈 Statistics (uptime: {:?})", uptime);
                info!("  Files sent: {}, received: {}", stats.files_sent, stats.files_received);
                info!("  Bytes sent: {}, received: {}", stats.bytes_sent, stats.bytes_received);
                info!("  Success rate: {}/{} transfers", 
                      stats.successful_transfers, 
                      stats.successful_transfers + stats.failed_transfers);
            }
        });
        self.background_tasks.push(stats_task);

        // Peer discovery monitoring
        let state = Arc::clone(&self.state);
        let peer_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                if state.shutdown_requested.read().await.is_some() {
                    break;
                }

                let peer_count = state.connected_peers.read().await.len();
                if peer_count > 0 {
                    debug!("🌐 Connected to {} peers", peer_count);
                }
            }
        });
        self.background_tasks.push(peer_task);

        info!("✅ Background tasks started");
        Ok(())
    }

    /// Read user input asynchronously
    async fn read_user_input(&self) -> Option<String> {
        // Use a separate task to handle blocking stdin read
        match tokio::task::spawn_blocking(|| {
            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(_) => Some(line.trim().to_string()),
                Err(_) => None,
            }
        }).await {
            Ok(Some(line)) if !line.is_empty() => Some(line),
            _ => None,
        }
    }

    /// Handle user commands in receiver mode
    async fn handle_user_command(&self, command: &str) -> Result<()> {
        match command {
            "help" => {
                println!("📋 Available commands:");
                println!("  help     - Show this help message");
                println!("  status   - Show current status");
                println!("  peers    - List connected peers");
                println!("  stats    - Show transfer statistics");
                println!("  quit     - Exit the application");
            }
            "status" => {
                self.print_status().await;
            }
            "peers" => {
                self.print_connected_peers().await;
            }
            "stats" => {
                self.print_statistics().await;
            }
            "quit" | "exit" => {
                let _ = self.shutdown_tx.send(ShutdownReason::UserCommand).await;
            }
            _ => {
                warn!("Unknown command: '{}'. Type 'help' for available commands.", command);
            }
        }
        Ok(())
    }

    /// Print current application status
    async fn print_status(&self) -> () {
        let uptime = self.state.start_time.elapsed();
        let peer_count = self.state.connected_peers.read().await.len();
        let transfer_count = self.state.active_transfers.read().await.len();

        println!("📊 Application Status:");
        println!("  Mode: {:?}", self.state.mode);
        println!("  Uptime: {:?}", uptime);
        println!("  Connected peers: {}", peer_count);
        println!("  Active transfers: {}", transfer_count);
        println!("  Output directory: {}", self.state.args.output_dir.display());
    }

    /// Print connected peers
    async fn print_connected_peers(&self) {
        let peers = self.state.connected_peers.read().await;

        if peers.is_empty() {
            println!("🌐 No peers currently connected");
        } else {
            println!("🌐 Connected peers ({}):", peers.len());
            for (peer_id, addresses) in peers.iter() {
                println!("  {} ({})", peer_id, addresses.len());
                for addr in addresses.iter().take(3) {
                    println!("    {}", addr);
                }
                if addresses.len() > 3 {
                    println!("    ... and {} more", addresses.len() - 3);
                }
            }
        }
    }

    /// Print transfer statistics
    async fn print_statistics(&self) {
        let stats = self.state.transfer_stats.read().await;
        let uptime = self.state.start_time.elapsed();

        println!("📈 Transfer Statistics:");
        println!("  Uptime: {:?}", uptime);
        println!("  Files sent: {}", stats.files_sent);
        println!("  Files received: {}", stats.files_received);
        println!("  Bytes sent: {} ({:.1} MB)", stats.bytes_sent, stats.bytes_sent as f64 / 1024.0 / 1024.0);
        println!("  Bytes received: {} ({:.1} MB)", stats.bytes_received, stats.bytes_received as f64 / 1024.0 / 1024.0);
        println!("  Successful transfers: {}", stats.successful_transfers);
        println!("  Failed transfers: {}", stats.failed_transfers);
        println!("  Conversions performed: {}", stats.conversion_count);

        let total_transfers = stats.successful_transfers + stats.failed_transfers;
        if total_transfers > 0 {
            let success_rate = (stats.successful_transfers as f64 / total_transfers as f64) * 100.0;
            println!("  Success rate: {:.1}%", success_rate);
        }
    }

    /// Print transfer status (sender mode)
    async fn print_transfer_status(&self) {
        let transfers = self.state.active_transfers.read().await;

        if transfers.is_empty() {
            println!("📊 No active transfers");
        } else {
            println!("📊 Active transfers ({}):", transfers.len());
            for (id, progress) in transfers.iter() {
                println!("  Transfer {}", &id[..8]);
                println!("    File: {}", progress.file_path.display());
                println!("    Progress: {:.1}% ({}/{} bytes)", 
                         progress.percentage(), progress.sent_bytes, progress.total_size);
                println!("    Speed: {:.1} KB/s", progress.speed_bps() / 1024.0);
                println!("    Status: {}", progress.status_string());

                if let Some(eta) = progress.eta_seconds() {
                    println!("    ETA: {:.0} seconds", eta);
                }
            }
        }
    }

    /// Handle transfer result
    async fn handle_transfer_result(&self, result: SendResult) {
        let mut stats = self.state.transfer_stats.write().await;

        if result.success {
            stats.successful_transfers += 1;
            stats.bytes_sent += result.bytes_sent;
            stats.files_sent += 1;

            info!("✅ Transfer {} completed successfully", result.transfer_id);
            info!("📊 Sent {} bytes in {:?}", result.bytes_sent, result.duration);
        } else {
            stats.failed_transfers += 1;

            let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
            warn!("❌ Transfer {} failed: {}", result.transfer_id, error_msg);
            warn!("📊 Partial transfer: {} bytes in {:?}", result.bytes_sent, result.duration);
        }
    }

    /// Perform periodic maintenance
    async fn perform_maintenance(&self) {
        debug!("🔧 Performing maintenance tasks");

        // Clean up completed transfers
        let mut transfers = self.state.active_transfers.write().await;
        let mut to_remove = Vec::new();

        for (id, progress) in transfers.iter() {
            if matches!(progress.status, TransferStatus::Completed | TransferStatus::Failed(_) | TransferStatus::Cancelled) {
                if progress.start_time.elapsed() > Duration::from_secs(300) {
                    to_remove.push(id.clone());
                }
            }
        }

        for id in to_remove {
            transfers.remove(&id);
            debug!("🧹 Cleaned up completed transfer: {}", id);
        }
    }

    /// Extract peer ID from multiaddr
    fn extract_peer_id(&self, addr: &Multiaddr) -> Result<PeerId> {
        use libp2p::multiaddr::Protocol;

        for protocol in addr.iter() {
            if let Protocol::P2p(peer_id) = protocol {
                return Ok(peer_id);
            }
        }

        Err(anyhow::anyhow!("No peer ID found in multiaddr: {}", addr))
    }

    /// Cleanup background tasks
    async fn cleanup_background_tasks(&mut self) {
        info!("🧹 Cleaning up background tasks");

        // Signal shutdown to all tasks
        *self.state.shutdown_requested.write().await = Some(ShutdownReason::UserCommand);

        // Wait for tasks to complete or abort them
        for task in self.background_tasks.drain(..) {
            task.abort();
        }

        // Give tasks time to cleanup
        sleep(Duration::from_millis(100)).await;

        info!("✅ Background tasks cleaned up");
    }
}

// Convert between different progress types
impl From<crate::file_sender::SendProgress> for TransferProgress {
    fn from(send_progress: crate::file_sender::SendProgress) -> Self {
        TransferProgress {
            transfer_id: send_progress.transfer_id,
            filename: send_progress.file_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            total_size: send_progress.total_size,
            transferred: send_progress.sent_bytes,
            start_time: send_progress.start_time,
            peer_id: send_progress.peer_id,
        }
    }
}

impl From<TransferProgress> for crate::file_sender::SendProgress {
    fn from(transfer_progress: TransferProgress) -> Self {
        crate::file_sender::SendProgress {
            transfer_id: transfer_progress.transfer_id,
            file_path: PathBuf::from(&transfer_progress.filename),
            peer_id: transfer_progress.peer_id,
            total_size: transfer_progress.total_size,
            sent_bytes: transfer_progress.transferred,
            chunks_sent: 0, // Not available in TransferProgress
            total_chunks: 0, // Not available in TransferProgress
            start_time: transfer_progress.start_time,
            status: TransferStatus::Sending, // Default status
            connection_attempts: 1,
            last_error: None,
        }
    }
}

/// Main application entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Create and run the P2P file converter
    let mut app = P2PFileConverter::new().await?;
    let exit_code = app.run().await?;

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_app_creation() {
        // Test that we can create the application
        // Note: This would need proper CLI args in a real test
        // let app = P2PFileConverter::new().await;
        // assert!(app.is_ok());
    }

    #[test]
    fn test_shutdown_reason_debug() {
        let reason = ShutdownReason::UserInterrupt;
        assert!(format!("{:?}", reason).contains("UserInterrupt"));
    }

    #[test]
    fn test_transfer_stats_default() {
        let stats = TransferStats::default();
        assert_eq!(stats.files_sent, 0);
        assert_eq!(stats.files_received, 0);
    }

    #[test]
    fn test_event_loop_event_debug() {
        let event = EventLoopEvent::UserInput("test".to_string());
        assert!(format!("{:?}", event).contains("UserInput"));
    }
}
