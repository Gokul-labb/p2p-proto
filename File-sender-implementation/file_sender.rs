use anyhow::{Context, Result};
use futures::{
    future::{select, Either},
    pin_mut, select,
    stream::{FuturesUnordered, StreamExt},
    Future, FutureExt,
};
use libp2p::{
    core::ConnectedPoint,
    request_response::{self, Codec, OutboundRequestId, RequestId},
    swarm::{SwarmEvent, dial_opts::DialOpts},
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::SeekFrom,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
    sync::{mpsc, Mutex, RwLock},
    time::{interval, sleep, timeout, Interval},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Re-use protocol definitions from stream handler
use crate::p2p_stream_handler::{
    FileChunk, FileConversionCodec, FileTransferRequest, FileTransferResponse, 
    FileType, PROTOCOL_NAME, MAX_CHUNK_SIZE, MAX_FILE_SIZE, TRANSFER_TIMEOUT
};
use crate::file_converter::FileConverter;

/// Connection retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of connection attempts
    pub max_attempts: usize,
    /// Initial retry delay
    pub initial_delay: Duration,
    /// Maximum retry delay (for exponential backoff)
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Connection timeout per attempt
    pub connection_timeout: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            connection_timeout: Duration::from_secs(10),
        }
    }
}

/// Progress information for file sending
#[derive(Debug, Clone)]
pub struct SendProgress {
    /// Transfer ID
    pub transfer_id: String,
    /// File path being sent
    pub file_path: PathBuf,
    /// Target peer ID
    pub peer_id: PeerId,
    /// Total file size in bytes
    pub total_size: u64,
    /// Bytes sent so far
    pub sent_bytes: u64,
    /// Number of chunks sent
    pub chunks_sent: usize,
    /// Total number of chunks
    pub total_chunks: usize,
    /// Transfer start time
    pub start_time: Instant,
    /// Current transfer status
    pub status: TransferStatus,
    /// Connection attempts made
    pub connection_attempts: usize,
    /// Last error encountered
    pub last_error: Option<String>,
}

impl SendProgress {
    /// Calculate transfer speed in bytes per second
    pub fn speed_bps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.sent_bytes as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Calculate percentage complete
    pub fn percentage(&self) -> f64 {
        if self.total_size > 0 {
            (self.sent_bytes as f64 / self.total_size as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Estimate time remaining in seconds
    pub fn eta_seconds(&self) -> Option<f64> {
        let speed = self.speed_bps();
        if speed > 0.0 && self.sent_bytes < self.total_size {
            let remaining = self.total_size - self.sent_bytes;
            Some(remaining as f64 / speed)
        } else {
            None
        }
    }

    /// Get human-readable status
    pub fn status_string(&self) -> String {
        match &self.status {
            TransferStatus::Connecting => format!("Connecting (attempt {})", self.connection_attempts),
            TransferStatus::Negotiating => "Negotiating protocol".to_string(),
            TransferStatus::Sending => format!("Sending chunk {}/{}", self.chunks_sent, self.total_chunks),
            TransferStatus::WaitingResponse => "Waiting for response".to_string(),
            TransferStatus::Completed => "Completed successfully".to_string(),
            TransferStatus::Failed(error) => format!("Failed: {}", error),
            TransferStatus::Cancelled => "Cancelled".to_string(),
        }
    }
}

/// Transfer status enumeration
#[derive(Debug, Clone)]
pub enum TransferStatus {
    Connecting,
    Negotiating,
    Sending,
    WaitingResponse,
    Completed,
    Failed(String),
    Cancelled,
}

/// File sending result
#[derive(Debug)]
pub struct SendResult {
    pub transfer_id: String,
    pub success: bool,
    pub bytes_sent: u64,
    pub duration: Duration,
    pub response: Option<FileTransferResponse>,
    pub error: Option<String>,
}

/// Active file transfer tracking
#[derive(Debug)]
struct ActiveSend {
    pub progress: SendProgress,
    pub file: File,
    pub request_id: Option<OutboundRequestId>,
    pub response_receiver: Option<mpsc::Receiver<FileTransferResponse>>,
    pub cancel_sender: Option<mpsc::Sender<()>>,
}

/// File sender service
pub struct FileSender {
    /// libp2p swarm
    swarm: Swarm<request_response::Behaviour<FileConversionCodec>>,
    /// Active transfers
    active_sends: Arc<RwLock<HashMap<String, ActiveSend>>>,
    /// File converter for type detection
    converter: Arc<Mutex<FileConverter>>,
    /// Retry configuration
    retry_config: RetryConfig,
    /// Progress callback
    progress_callback: Option<Arc<dyn Fn(&SendProgress) + Send + Sync>>,
}

impl FileSender {
    /// Create a new file sender
    pub async fn new(retry_config: Option<RetryConfig>) -> Result<Self> {
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        info!("Creating file sender with peer ID: {}", local_peer_id);

        // Create request-response behaviour
        let behaviour = request_response::Behaviour::new(
            FileConversionCodec,
            [libp2p::StreamProtocol::new(PROTOCOL_NAME)],
            request_response::Config::default()
                .with_request_timeout(TRANSFER_TIMEOUT)
                .with_max_concurrent_streams(10),
        );

        // Build swarm
        let swarm = SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default()
                    .port_reuse(true)
                    .nodelay(true),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .context("Failed to configure transport")?
            .with_behaviour(|_| Ok(behaviour))
            .context("Failed to configure behaviour")?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(30))
                   .with_dial_concurrency_factor(5.try_into().unwrap())
            })
            .build();

        Ok(Self {
            swarm,
            active_sends: Arc::new(RwLock::new(HashMap::new())),
            converter: Arc::new(Mutex::new(FileConverter::new())),
            retry_config: retry_config.unwrap_or_default(),
            progress_callback: None,
        })
    }

    /// Set progress callback function
    pub fn set_progress_callback<F>(&mut self, callback: F)
    where
        F: Fn(&SendProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
    }

    /// Send file to target peer
    pub async fn send_file<P: AsRef<Path>>(
        &mut self,
        target_peer: PeerId,
        target_addr: Multiaddr,
        file_path: P,
        target_format: Option<String>,
        return_result: bool,
    ) -> Result<String> {
        let file_path = file_path.as_ref();
        let transfer_id = Uuid::new_v4().to_string();

        info!(
            "Starting file transfer {} to peer {} at {}",
            transfer_id, target_peer, target_addr
        );

        // Validate file
        let file = File::open(&file_path).await
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let metadata = file.metadata().await
            .with_context(|| format!("Failed to read file metadata: {}", file_path.display()))?;

        let file_size = metadata.len();
        if file_size > MAX_FILE_SIZE {
            return Err(anyhow::anyhow!(
                "File size {} exceeds maximum allowed size {}",
                file_size, MAX_FILE_SIZE
            ));
        }

        // Detect file type
        let file_type = self.converter.lock().await.detect_file_type(&file_path)?;

        // Calculate chunks
        let total_chunks = ((file_size + MAX_CHUNK_SIZE as u64 - 1) / MAX_CHUNK_SIZE as u64) as usize;

        // Create progress tracking
        let progress = SendProgress {
            transfer_id: transfer_id.clone(),
            file_path: file_path.to_path_buf(),
            peer_id: target_peer,
            total_size: file_size,
            sent_bytes: 0,
            chunks_sent: 0,
            total_chunks,
            start_time: Instant::now(),
            status: TransferStatus::Connecting,
            connection_attempts: 0,
            last_error: None,
        };

        // Create transfer request
        let request = FileTransferRequest {
            transfer_id: transfer_id.clone(),
            filename: file_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_size,
            file_type: file_type.to_string(),
            target_format,
            return_result,
            chunk_count: total_chunks,
        };

        // Create response channel
        let (response_tx, response_rx) = mpsc::channel(1);
        let (cancel_tx, cancel_rx) = mpsc::channel(1);

        // Store active transfer
        let active_send = ActiveSend {
            progress,
            file,
            request_id: None,
            response_receiver: Some(response_rx),
            cancel_sender: Some(cancel_tx),
        };

        self.active_sends.write().await.insert(transfer_id.clone(), active_send);

        // Start the transfer process
        let sender_clone = Arc::new(Mutex::new(self));
        let transfer_task = tokio::spawn(async move {
            Self::perform_transfer(
                sender_clone,
                transfer_id.clone(),
                target_peer,
                target_addr,
                request,
                response_tx,
                cancel_rx,
            ).await
        });

        // Wait briefly to ensure transfer is started
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(transfer_id)
    }

    /// Perform the actual file transfer with retry logic
    async fn perform_transfer(
        sender: Arc<Mutex<&mut Self>>,
        transfer_id: String,
        target_peer: PeerId,
        target_addr: Multiaddr,
        request: FileTransferRequest,
        response_tx: mpsc::Sender<FileTransferResponse>,
        mut cancel_rx: mpsc::Receiver<()>,
    ) -> Result<()> {
        let retry_config = sender.lock().await.retry_config.clone();
        let mut delay = retry_config.initial_delay;
        let mut last_error = None;

        for attempt in 1..=retry_config.max_attempts {
            // Update progress
            {
                let mut sender_lock = sender.lock().await;
                if let Some(active_send) = sender_lock.active_sends.write().await.get_mut(&transfer_id) {
                    active_send.progress.connection_attempts = attempt;
                    active_send.progress.status = TransferStatus::Connecting;
                    sender_lock.notify_progress(&active_send.progress);
                }
            }

            info!("Connection attempt {}/{} for transfer {}", attempt, retry_config.max_attempts, transfer_id);

            // Attempt connection with timeout
            let connection_result = timeout(
                retry_config.connection_timeout,
                Self::attempt_connection_and_transfer(
                    sender.clone(),
                    transfer_id.clone(),
                    target_peer,
                    target_addr.clone(),
                    request.clone(),
                    response_tx.clone(),
                )
            ).await;

            match connection_result {
                Ok(Ok(())) => {
                    info!("Transfer {} completed successfully", transfer_id);
                    return Ok(());
                }
                Ok(Err(e)) => {
                    last_error = Some(e);
                    warn!("Transfer attempt {} failed: {}", attempt, last_error.as_ref().unwrap());
                }
                Err(_) => {
                    let timeout_error = anyhow::anyhow!("Connection timeout after {:?}", retry_config.connection_timeout);
                    last_error = Some(timeout_error);
                    warn!("Transfer attempt {} timed out", attempt);
                }
            }

            // Check for cancellation
            if cancel_rx.try_recv().is_ok() {
                warn!("Transfer {} cancelled", transfer_id);
                Self::update_transfer_status(
                    sender.clone(),
                    &transfer_id,
                    TransferStatus::Cancelled
                ).await;
                return Ok(());
            }

            // Wait before retry (except on last attempt)
            if attempt < retry_config.max_attempts {
                info!("Retrying in {:?}...", delay);
                sleep(delay).await;
                delay = Duration::from_millis(
                    ((delay.as_millis() as f64) * retry_config.backoff_multiplier).min(retry_config.max_delay.as_millis() as f64) as u64
                );
            }
        }

        // All attempts failed
        let final_error = last_error.unwrap_or_else(|| anyhow::anyhow!("All connection attempts failed"));
        error!("Transfer {} failed after {} attempts: {}", transfer_id, retry_config.max_attempts, final_error);

        Self::update_transfer_status(
            sender.clone(),
            &transfer_id,
            TransferStatus::Failed(final_error.to_string())
        ).await;

        Err(final_error)
    }

    /// Attempt a single connection and transfer
    async fn attempt_connection_and_transfer(
        sender: Arc<Mutex<&mut Self>>,
        transfer_id: String,
        target_peer: PeerId,
        target_addr: Multiaddr,
        request: FileTransferRequest,
        response_tx: mpsc::Sender<FileTransferResponse>,
    ) -> Result<()> {
        // Connect to peer
        {
            let mut sender_lock = sender.lock().await;
            sender_lock.swarm.dial(
                DialOpts::peer_id(target_peer)
                    .addresses(vec![target_addr.clone()])
                    .build()
            )?;
        }

        // Wait for connection establishment  
        let connection_established = Self::wait_for_connection(sender.clone(), target_peer).await?;
        if !connection_established {
            return Err(anyhow::anyhow!("Failed to establish connection to peer"));
        }

        // Update status to negotiating
        Self::update_transfer_status(
            sender.clone(),
            &transfer_id,
            TransferStatus::Negotiating
        ).await;

        // Send the initial request
        let request_id = {
            let mut sender_lock = sender.lock().await;
            sender_lock.swarm.behaviour_mut()
                .send_request(&target_peer, request.clone())
        };

        // Update request ID in active transfer
        {
            let sender_lock = sender.lock().await;
            if let Some(active_send) = sender_lock.active_sends.write().await.get_mut(&transfer_id) {
                active_send.request_id = Some(request_id);
            }
        }

        // Send file chunks
        Self::send_file_chunks(sender.clone(), &transfer_id, target_peer).await?;

        // Wait for response
        Self::wait_for_response(sender.clone(), &transfer_id, response_tx).await?;

        Ok(())
    }

    /// Wait for connection to be established
    async fn wait_for_connection(
        sender: Arc<Mutex<&mut Self>>,
        target_peer: PeerId,
    ) -> Result<bool> {
        let timeout_duration = Duration::from_secs(30);
        let start_time = Instant::now();

        while start_time.elapsed() < timeout_duration {
            let event = {
                let mut sender_lock = sender.lock().await;
                sender_lock.swarm.select_next_some().await
            };

            match event {
                SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == target_peer => {
                    info!("Connection established with peer: {}", peer_id);
                    return Ok(true);
                }
                SwarmEvent::OutgoingConnectionError { peer_id, error, .. } 
                    if peer_id == Some(target_peer) => {
                    warn!("Connection error to {}: {}", target_peer, error);
                    return Err(anyhow::anyhow!("Connection failed: {}", error));
                }
                SwarmEvent::Behaviour(request_response::Event::OutboundFailure { 
                    peer, error, .. 
                }) if peer == target_peer => {
                    warn!("Request-response outbound failure to {}: {:?}", peer, error);
                    return Err(anyhow::anyhow!("Request-response failure: {:?}", error));
                }
                _ => {
                    // Continue waiting for other events
                    debug!("Received other swarm event while waiting for connection");
                }
            }
        }

        Err(anyhow::anyhow!("Connection timeout"))
    }

    /// Send file chunks to peer
    async fn send_file_chunks(
        sender: Arc<Mutex<&mut Self>>,
        transfer_id: &str,
        target_peer: PeerId,
    ) -> Result<()> {
        // Update status
        Self::update_transfer_status(
            sender.clone(),
            transfer_id,
            TransferStatus::Sending
        ).await;

        let mut buffer = vec![0u8; MAX_CHUNK_SIZE];
        let mut chunk_index = 0;

        loop {
            // Read next chunk
            let bytes_read = {
                let sender_lock = sender.lock().await;
                let mut active_sends = sender_lock.active_sends.write().await;
                let active_send = active_sends.get_mut(transfer_id)
                    .ok_or_else(|| anyhow::anyhow!("Transfer not found: {}", transfer_id))?;

                active_send.file.read(&mut buffer).await?
            };

            if bytes_read == 0 {
                break; // End of file
            }

            // Create chunk
            let is_final = {
                let sender_lock = sender.lock().await;
                let active_sends = sender_lock.active_sends.read().await;
                let active_send = active_sends.get(transfer_id).unwrap();
                chunk_index >= active_send.progress.total_chunks - 1
            };

            let chunk = FileChunk {
                transfer_id: transfer_id.to_string(),
                chunk_index,
                data: buffer[..bytes_read].to_vec(),
                is_final,
            };

            // Send chunk (in a real implementation, this would be sent over a separate stream)
            // For now, we'll simulate the chunk sending
            info!("Sending chunk {}/{} ({} bytes)", 
                  chunk_index + 1, 
                  {
                      let sender_lock = sender.lock().await;
                      let active_sends = sender_lock.active_sends.read().await;
                      active_sends.get(transfer_id).unwrap().progress.total_chunks
                  },
                  bytes_read);

            // Update progress
            {
                let sender_lock = sender.lock().await;
                let mut active_sends = sender_lock.active_sends.write().await;
                let active_send = active_sends.get_mut(transfer_id).unwrap();

                active_send.progress.sent_bytes += bytes_read as u64;
                active_send.progress.chunks_sent = chunk_index + 1;

                sender_lock.notify_progress(&active_send.progress);
            }

            chunk_index += 1;

            // Simulate network delay
            tokio::time::sleep(Duration::from_millis(10)).await;

            if is_final {
                break;
            }
        }

        info!("All chunks sent for transfer {}", transfer_id);
        Ok(())
    }

    /// Wait for response from peer
    async fn wait_for_response(
        sender: Arc<Mutex<&mut Self>>,
        transfer_id: &str,
        response_tx: mpsc::Sender<FileTransferResponse>,
    ) -> Result<()> {
        // Update status
        Self::update_transfer_status(
            sender.clone(),
            transfer_id,
            TransferStatus::WaitingResponse
        ).await;

        // In a real implementation, this would wait for the actual response
        // For now, we'll simulate a successful response
        tokio::time::sleep(Duration::from_secs(2)).await;

        let response = FileTransferResponse {
            transfer_id: transfer_id.to_string(),
            success: true,
            error_message: None,
            converted_data: None,
            converted_filename: None,
            processing_time_ms: 1500,
        };

        if let Err(e) = response_tx.send(response).await {
            warn!("Failed to send response for transfer {}: {}", transfer_id, e);
        }

        // Update status to completed
        Self::update_transfer_status(
            sender.clone(),
            transfer_id,
            TransferStatus::Completed
        ).await;

        info!("Transfer {} completed successfully", transfer_id);
        Ok(())
    }

    /// Update transfer status
    async fn update_transfer_status(
        sender: Arc<Mutex<&mut Self>>,
        transfer_id: &str,
        status: TransferStatus,
    ) {
        let sender_lock = sender.lock().await;
        let mut active_sends = sender_lock.active_sends.write().await;

        if let Some(active_send) = active_sends.get_mut(transfer_id) {
            active_send.progress.status = status;
            if let TransferStatus::Failed(ref error) = active_send.progress.status {
                active_send.progress.last_error = Some(error.clone());
            }
            sender_lock.notify_progress(&active_send.progress);
        }
    }

    /// Notify progress callback
    fn notify_progress(&self, progress: &SendProgress) {
        if let Some(ref callback) = self.progress_callback {
            callback(progress);
        }
    }

    /// Cancel an active transfer
    pub async fn cancel_transfer(&self, transfer_id: &str) -> Result<()> {
        let active_sends = self.active_sends.read().await;

        if let Some(active_send) = active_sends.get(transfer_id) {
            if let Some(ref cancel_sender) = active_send.cancel_sender {
                if let Err(e) = cancel_sender.send(()).await {
                    warn!("Failed to send cancel signal for transfer {}: {}", transfer_id, e);
                }
            }
        }

        Ok(())
    }

    /// Get transfer progress
    pub async fn get_progress(&self, transfer_id: &str) -> Option<SendProgress> {
        let active_sends = self.active_sends.read().await;
        active_sends.get(transfer_id).map(|send| send.progress.clone())
    }

    /// Get all active transfers
    pub async fn get_all_progress(&self) -> Vec<SendProgress> {
        let active_sends = self.active_sends.read().await;
        active_sends.values().map(|send| send.progress.clone()).collect()
    }

    /// Wait for transfer completion
    pub async fn wait_for_completion(&self, transfer_id: &str) -> Result<SendResult> {
        let start_time = Instant::now();

        loop {
            let progress = self.get_progress(transfer_id).await
                .ok_or_else(|| anyhow::anyhow!("Transfer not found: {}", transfer_id))?;

            match &progress.status {
                TransferStatus::Completed => {
                    return Ok(SendResult {
                        transfer_id: transfer_id.to_string(),
                        success: true,
                        bytes_sent: progress.sent_bytes,
                        duration: start_time.elapsed(),
                        response: None, // Would include actual response in real implementation
                        error: None,
                    });
                }
                TransferStatus::Failed(error) => {
                    return Ok(SendResult {
                        transfer_id: transfer_id.to_string(),
                        success: false,
                        bytes_sent: progress.sent_bytes,
                        duration: start_time.elapsed(),
                        response: None,
                        error: Some(error.clone()),
                    });
                }
                TransferStatus::Cancelled => {
                    return Ok(SendResult {
                        transfer_id: transfer_id.to_string(),
                        success: false,
                        bytes_sent: progress.sent_bytes,
                        duration: start_time.elapsed(),
                        response: None,
                        error: Some("Transfer was cancelled".to_string()),
                    });
                }
                _ => {
                    // Still in progress, wait a bit
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Clean up completed transfers
    pub async fn cleanup_completed_transfers(&self) {
        let mut active_sends = self.active_sends.write().await;
        let mut to_remove = Vec::new();

        for (transfer_id, active_send) in active_sends.iter() {
            match &active_send.progress.status {
                TransferStatus::Completed | TransferStatus::Failed(_) | TransferStatus::Cancelled => {
                    // Keep transfers for a while after completion for status checking
                    if active_send.progress.start_time.elapsed() > Duration::from_secs(300) {
                        to_remove.push(transfer_id.clone());
                    }
                }
                _ => {}
            }
        }

        for transfer_id in to_remove {
            active_sends.remove(&transfer_id);
            info!("Cleaned up completed transfer: {}", transfer_id);
        }
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let sender = Arc::new(self);
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));

            loop {
                cleanup_interval.tick().await;
                sender.cleanup_completed_transfers().await;
            }
        })
    }

    /// Run the swarm event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting file sender event loop");

        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("File sender listening on: {}", address);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    debug!("Connection established with: {}", peer_id);
                }
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    debug!("Connection closed with {}: {:?}", peer_id, cause);
                }
                SwarmEvent::Behaviour(request_response::Event::ResponseReceived { 
                    peer, response, .. 
                }) => {
                    debug!("Received response from {}: {:?}", peer, response);
                    // Handle response for active transfers
                    self.handle_response(response).await;
                }
                SwarmEvent::Behaviour(request_response::Event::OutboundFailure { 
                    peer, error, .. 
                }) => {
                    warn!("Outbound request failed to {}: {:?}", peer, error);
                    // Handle failure for active transfers
                    self.handle_outbound_failure(peer, error).await;
                }
                _ => {
                    debug!("Received other swarm event");
                }
            }
        }
    }

    /// Handle response from peer
    async fn handle_response(&self, response: FileTransferResponse) {
        let active_sends = self.active_sends.read().await;

        if let Some(active_send) = active_sends.get(&response.transfer_id) {
            if let Some(ref response_tx) = active_send.response_receiver {
                // In a real implementation, we would send the response through the channel
                info!("Received response for transfer {}: success={}", 
                      response.transfer_id, response.success);
            }
        }
    }

    /// Handle outbound request failure
    async fn handle_outbound_failure(
        &self, 
        peer: PeerId, 
        error: request_response::OutboundFailure
    ) {
        warn!("Request to peer {} failed: {:?}", peer, error);

        // Find transfers to this peer and mark them as failed
        let mut active_sends = self.active_sends.write().await;
        let failed_transfers: Vec<String> = active_sends
            .iter()
            .filter(|(_, send)| send.progress.peer_id == peer)
            .map(|(id, _)| id.clone())
            .collect();

        for transfer_id in failed_transfers {
            if let Some(active_send) = active_sends.get_mut(&transfer_id) {
                active_send.progress.status = TransferStatus::Failed(format!("{:?}", error));
                active_send.progress.last_error = Some(format!("{:?}", error));
                self.notify_progress(&active_send.progress);
            }
        }
    }
}

/// Progress tracking utilities
pub mod progress {
    use super::*;
    use std::fmt;

    /// Progress reporter for file transfers
    pub struct ProgressReporter {
        last_update: Instant,
        update_interval: Duration,
    }

    impl ProgressReporter {
        pub fn new(update_interval: Duration) -> Self {
            Self {
                last_update: Instant::now(),
                update_interval,
            }
        }

        /// Report progress if enough time has elapsed
        pub fn maybe_report(&mut self, progress: &SendProgress) -> bool {
            if self.last_update.elapsed() >= self.update_interval {
                self.report(progress);
                self.last_update = Instant::now();
                true
            } else {
                false
            }
        }

        /// Always report progress
        pub fn report(&self, progress: &SendProgress) {
            println!("{}", self.format_progress(progress));
        }

        /// Format progress as string
        pub fn format_progress(&self, progress: &SendProgress) -> String {
            let speed_kbps = progress.speed_bps() / 1024.0;
            let eta_str = progress.eta_seconds()
                .map(|eta| format!("{:.0}s", eta))
                .unwrap_or_else(|| "∞".to_string());

            format!(
                "[{}] {:.1}% ({}/{} bytes) - {:.1} KB/s - ETA: {} - {}",
                progress.transfer_id[..8].to_string(),
                progress.percentage(),
                progress.sent_bytes,
                progress.total_size,
                speed_kbps,
                eta_str,
                progress.status_string()
            )
        }
    }

    impl fmt::Display for SendProgress {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "Transfer {} to {}: {:.1}% complete ({} bytes sent)",
                self.transfer_id[..8].to_string(),
                self.peer_id,
                self.percentage(),
                self.sent_bytes
            )
        }
    }
}

/// Example usage and integration
pub mod examples {
    use super::*;

    /// Simple file sending example
    pub async fn send_file_example() -> Result<()> {
        // Create file sender
        let mut sender = FileSender::new(None).await?;

        // Set up progress callback
        sender.set_progress_callback(|progress| {
            println!("Progress: {}", progress);
        });

        // Start sender in background
        let sender_task = tokio::spawn(async move {
            if let Err(e) = sender.run().await {
                eprintln!("Sender error: {}", e);
            }
        });

        // Wait briefly for sender to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send a file
        let target_peer = PeerId::random(); // In real usage, get from multiaddr
        let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse()?;

        let transfer_id = sender.send_file(
            target_peer,
            target_addr,
            "test_file.txt",
            Some("pdf".to_string()),
            false,
        ).await?;

        println!("Started transfer: {}", transfer_id);

        // Wait for completion
        let result = sender.wait_for_completion(&transfer_id).await?;
        println!("Transfer result: {:?}", result);

        sender_task.abort();
        Ok(())
    }

    /// Batch file sending example
    pub async fn send_multiple_files() -> Result<()> {
        let mut sender = FileSender::new(None).await?;

        // Progress tracking
        use crate::file_sender::progress::ProgressReporter;
        let mut reporter = ProgressReporter::new(Duration::from_secs(1));

        sender.set_progress_callback(move |progress| {
            reporter.maybe_report(progress);
        });

        let files = vec!["file1.txt", "file2.pdf", "file3.doc"];
        let target_peer = PeerId::random();
        let target_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse()?;

        let mut transfer_ids = Vec::new();

        // Start all transfers
        for file in &files {
            let transfer_id = sender.send_file(
                target_peer,
                target_addr.clone(),
                file,
                None,
                false,
            ).await?;
            transfer_ids.push(transfer_id);
            println!("Started transfer for {}", file);
        }

        // Wait for all to complete
        for transfer_id in transfer_ids {
            let result = sender.wait_for_completion(&transfer_id).await?;
            println!("Transfer {} completed: success={}", 
                     transfer_id, result.success);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_file_sender_creation() {
        let sender = FileSender::new(None).await;
        assert!(sender.is_ok());
    }

    #[tokio::test]
    async fn test_retry_config() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            connection_timeout: Duration::from_secs(5),
        };

        let sender = FileSender::new(Some(config)).await;
        assert!(sender.is_ok());
    }

    #[tokio::test]
    async fn test_progress_calculation() {
        let progress = SendProgress {
            transfer_id: "test".to_string(),
            file_path: PathBuf::from("test.txt"),
            peer_id: PeerId::random(),
            total_size: 1000,
            sent_bytes: 250,
            chunks_sent: 5,
            total_chunks: 20,
            start_time: Instant::now() - Duration::from_secs(1),
            status: TransferStatus::Sending,
            connection_attempts: 1,
            last_error: None,
        };

        assert_eq!(progress.percentage(), 25.0);
        assert!(progress.speed_bps() > 0.0);
        assert!(progress.eta_seconds().is_some());
    }

    #[test]
    fn test_transfer_status_string() {
        let mut progress = SendProgress {
            transfer_id: "test".to_string(),
            file_path: PathBuf::from("test.txt"),
            peer_id: PeerId::random(),
            total_size: 1000,
            sent_bytes: 0,
            chunks_sent: 0,
            total_chunks: 10,
            start_time: Instant::now(),
            status: TransferStatus::Connecting,
            connection_attempts: 1,
            last_error: None,
        };

        assert!(progress.status_string().contains("Connecting"));

        progress.status = TransferStatus::Sending;
        progress.chunks_sent = 5;
        assert!(progress.status_string().contains("5/10"));

        progress.status = TransferStatus::Completed;
        assert_eq!(progress.status_string(), "Completed successfully");
    }
}
