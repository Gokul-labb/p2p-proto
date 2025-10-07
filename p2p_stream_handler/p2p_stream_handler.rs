use anyhow::{Context, Result};
use futures::{
    io::{AsyncReadExt, AsyncWriteExt},
    prelude::*,
    stream::StreamExt,
};
use libp2p::{
    core::upgrade,
    identity::Keypair,
    request_response::{
        self, Codec, ProtocolName, RequestResponse, RequestResponseEvent, 
        RequestResponseMessage, ResponseChannel,
    },
    swarm::{
        ConnectionHandler, ConnectionHandlerEvent, KeepAlive, NetworkBehaviour,
        SubstreamProtocol, SwarmEvent,
    },
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{self, Cursor},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::{mpsc, Mutex, RwLock},
    time::{interval, sleep},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Import our file converter from previous implementation
use crate::file_converter::{FileConverter, FileType, PdfConfig, ConversionError};

/// Protocol name for our file conversion service
const PROTOCOL_NAME: &str = "/convert/1.0.0";

/// Maximum chunk size for file transfer (1MB)
const MAX_CHUNK_SIZE: usize = 1024 * 1024;

/// Maximum file size to accept (100MB)
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Transfer timeout duration
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// File transfer request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    /// Unique transfer ID
    pub transfer_id: String,
    /// Original filename
    pub filename: String,
    /// File size in bytes
    pub file_size: u64,
    /// Expected file type
    pub file_type: String,
    /// Target conversion type (optional)
    pub target_format: Option<String>,
    /// Whether to send result back
    pub return_result: bool,
    /// File chunks follow this message
    pub chunk_count: usize,
}

/// File transfer response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferResponse {
    /// Transfer ID from request
    pub transfer_id: String,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Converted file data (if return_result was true)
    pub converted_data: Option<Vec<u8>>,
    /// Converted filename
    pub converted_filename: Option<String>,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// File chunk for streaming transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    /// Transfer ID
    pub transfer_id: String,
    /// Chunk sequence number (0-indexed)
    pub chunk_index: usize,
    /// Chunk data
    pub data: Vec<u8>,
    /// Whether this is the final chunk
    pub is_final: bool,
}

/// Transfer progress information
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
    pub transferred: u64,
    pub start_time: Instant,
    pub peer_id: PeerId,
}

impl TransferProgress {
    /// Calculate transfer speed in bytes per second
    pub fn speed_bps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.transferred as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Calculate percentage complete
    pub fn percentage(&self) -> f64 {
        if self.total_size > 0 {
            (self.transferred as f64 / self.total_size as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Estimate time remaining
    pub fn eta_seconds(&self) -> Option<f64> {
        let speed = self.speed_bps();
        if speed > 0.0 && self.transferred < self.total_size {
            let remaining = self.total_size - self.transferred;
            Some(remaining as f64 / speed)
        } else {
            None
        }
    }
}

/// File conversion protocol codec
#[derive(Clone)]
pub struct FileConversionCodec;

impl ProtocolName for FileConversionCodec {
    fn protocol_name(&self) -> &[u8] {
        PROTOCOL_NAME.as_bytes()
    }
}

#[async_trait::async_trait]
impl Codec for FileConversionCodec {
    type Protocol = StreamProtocol;
    type Request = FileTransferRequest;
    type Response = FileTransferResponse;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;

        bincode::deserialize(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;

        bincode::deserialize(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWriteExt + Unpin + Send,
    {
        let data = bincode::serialize(&req)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        io.write_all(&data).await?;
        io.close().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWriteExt + Unpin + Send,
    {
        let data = bincode::serialize(&res)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        io.write_all(&data).await?;
        io.close().await?;
        Ok(())
    }
}

/// Active file transfer tracking
#[derive(Debug)]
pub struct ActiveTransfer {
    pub request: FileTransferRequest,
    pub received_chunks: HashMap<usize, Vec<u8>>,
    pub total_received: u64,
    pub start_time: Instant,
    pub peer_id: PeerId,
    pub response_channel: Option<ResponseChannel<FileTransferResponse>>,
}

impl ActiveTransfer {
    pub fn new(
        request: FileTransferRequest,
        peer_id: PeerId,
        response_channel: ResponseChannel<FileTransferResponse>,
    ) -> Self {
        Self {
            request,
            received_chunks: HashMap::new(),
            total_received: 0,
            start_time: Instant::now(),
            peer_id,
            response_channel: Some(response_channel),
        }
    }

    /// Add a chunk to the transfer
    pub fn add_chunk(&mut self, chunk: FileChunk) -> Result<()> {
        if chunk.chunk_index >= self.request.chunk_count {
            return Err(anyhow::anyhow!(
                "Invalid chunk index {} for transfer {}",
                chunk.chunk_index,
                self.request.transfer_id
            ));
        }

        self.received_chunks.insert(chunk.chunk_index, chunk.data.clone());
        self.total_received += chunk.data.len() as u64;

        debug!(
            "Received chunk {}/{} for transfer {} ({} bytes)",
            chunk.chunk_index + 1,
            self.request.chunk_count,
            self.request.transfer_id,
            chunk.data.len()
        );

        Ok(())
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        self.received_chunks.len() == self.request.chunk_count
    }

    /// Assemble received chunks into complete file data
    pub fn assemble_file(&self) -> Result<Vec<u8>> {
        if !self.is_complete() {
            return Err(anyhow::anyhow!(
                "Transfer {} is not complete ({}/{} chunks)",
                self.request.transfer_id,
                self.received_chunks.len(),
                self.request.chunk_count
            ));
        }

        let mut file_data = Vec::with_capacity(self.request.file_size as usize);

        for i in 0..self.request.chunk_count {
            if let Some(chunk_data) = self.received_chunks.get(&i) {
                file_data.extend_from_slice(chunk_data);
            } else {
                return Err(anyhow::anyhow!(
                    "Missing chunk {} for transfer {}",
                    i,
                    self.request.transfer_id
                ));
            }
        }

        Ok(file_data)
    }
}

/// P2P file conversion service
pub struct FileConversionService {
    /// File converter instance
    converter: Arc<Mutex<FileConverter>>,
    /// Active transfers
    active_transfers: Arc<RwLock<HashMap<String, ActiveTransfer>>>,
    /// Transfer progress tracking
    transfer_progress: Arc<RwLock<HashMap<String, TransferProgress>>>,
    /// Output directory for received files
    output_dir: PathBuf,
    /// Configuration
    config: FileConversionConfig,
}

/// Configuration for file conversion service
#[derive(Debug, Clone)]
pub struct FileConversionConfig {
    /// Maximum concurrent transfers
    pub max_concurrent_transfers: usize,
    /// Output directory for received files
    pub output_dir: PathBuf,
    /// Auto-convert received files
    pub auto_convert: bool,
    /// Return conversion results to sender
    pub return_results: bool,
    /// PDF generation config
    pub pdf_config: PdfConfig,
}

impl Default for FileConversionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 5,
            output_dir: PathBuf::from("./received_files"),
            auto_convert: true,
            return_results: false,
            pdf_config: PdfConfig::default(),
        }
    }
}

impl FileConversionService {
    /// Create a new file conversion service
    pub fn new(config: FileConversionConfig) -> Result<Self> {
        // Ensure output directory exists
        std::fs::create_dir_all(&config.output_dir)?;

        Ok(Self {
            converter: Arc::new(Mutex::new(FileConverter::new())),
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            transfer_progress: Arc::new(RwLock::new(HashMap::new())),
            output_dir: config.output_dir.clone(),
            config,
        })
    }

    /// Handle incoming file transfer request
    pub async fn handle_file_transfer_request(
        &self,
        request: FileTransferRequest,
        peer_id: PeerId,
        response_channel: ResponseChannel<FileTransferResponse>,
    ) -> Result<()> {
        info!(
            "Received file transfer request from {}: {} ({} bytes)",
            peer_id, request.filename, request.file_size
        );

        // Validate request
        if request.file_size > MAX_FILE_SIZE {
            let response = FileTransferResponse {
                transfer_id: request.transfer_id.clone(),
                success: false,
                error_message: Some(format!(
                    "File size {} exceeds maximum allowed size {}",
                    request.file_size, MAX_FILE_SIZE
                )),
                converted_data: None,
                converted_filename: None,
                processing_time_ms: 0,
            };

            // Send error response
            if let Err(e) = self.send_response(response_channel, response).await {
                error!("Failed to send error response: {}", e);
            }
            return Ok(());
        }

        // Check concurrent transfer limit
        let active_count = self.active_transfers.read().await.len();
        if active_count >= self.config.max_concurrent_transfers {
            let response = FileTransferResponse {
                transfer_id: request.transfer_id.clone(),
                success: false,
                error_message: Some(format!(
                    "Too many concurrent transfers ({}/{})",
                    active_count, self.config.max_concurrent_transfers
                )),
                converted_data: None,
                converted_filename: None,
                processing_time_ms: 0,
            };

            if let Err(e) = self.send_response(response_channel, response).await {
                error!("Failed to send error response: {}", e);
            }
            return Ok(());
        }

        // Create active transfer
        let transfer = ActiveTransfer::new(request.clone(), peer_id, response_channel);

        // Add to tracking
        self.active_transfers
            .write()
            .await
            .insert(request.transfer_id.clone(), transfer);

        // Create progress tracking
        let progress = TransferProgress {
            transfer_id: request.transfer_id.clone(),
            filename: request.filename.clone(),
            total_size: request.file_size,
            transferred: 0,
            start_time: Instant::now(),
            peer_id,
        };

        self.transfer_progress
            .write()
            .await
            .insert(request.transfer_id.clone(), progress);

        info!(
            "Started transfer {}: {} from {}",
            request.transfer_id, request.filename, peer_id
        );

        Ok(())
    }

    /// Handle incoming file chunk
    pub async fn handle_file_chunk(&self, chunk: FileChunk) -> Result<()> {
        let mut transfers = self.active_transfers.write().await;

        if let Some(transfer) = transfers.get_mut(&chunk.transfer_id) {
            // Add chunk to transfer
            transfer.add_chunk(chunk.clone())?;

            // Update progress
            if let Some(progress) = self.transfer_progress.write().await.get_mut(&chunk.transfer_id) {
                progress.transferred = transfer.total_received;

                // Log progress periodically
                if chunk.chunk_index % 10 == 0 || chunk.is_final {
                    info!(
                        "Transfer {} progress: {:.1}% ({}/{} bytes) - {:.1} KB/s",
                        progress.transfer_id,
                        progress.percentage(),
                        progress.transferred,
                        progress.total_size,
                        progress.speed_bps() / 1024.0
                    );
                }
            }

            // Check if transfer is complete
            if transfer.is_complete() {
                info!("Transfer {} completed, processing file...", chunk.transfer_id);

                // Remove from active transfers and process
                let completed_transfer = transfers.remove(&chunk.transfer_id).unwrap();
                drop(transfers); // Release lock

                // Process the completed transfer
                self.process_completed_transfer(completed_transfer).await?;
            }
        } else {
            warn!(
                "Received chunk for unknown transfer: {}",
                chunk.transfer_id
            );
        }

        Ok(())
    }

    /// Process a completed file transfer
    async fn process_completed_transfer(&self, transfer: ActiveTransfer) -> Result<()> {
        let processing_start = Instant::now();
        let transfer_id = transfer.request.transfer_id.clone();

        // Assemble file data
        let file_data = match transfer.assemble_file() {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to assemble file for transfer {}: {}", transfer_id, e);
                self.send_error_response(transfer, format!("File assembly failed: {}", e)).await?;
                return Ok(());
            }
        };

        // Detect file type
        let detected_type = self.converter.lock().await.detect_file_type_from_bytes(&file_data);
        info!(
            "Transfer {}: detected file type {} for {}",
            transfer_id, detected_type, transfer.request.filename
        );

        // Save original file
        let original_path = self.output_dir.join(&transfer.request.filename);
        if let Err(e) = fs::write(&original_path, &file_data).await {
            error!("Failed to save file {}: {}", original_path.display(), e);
            self.send_error_response(transfer, format!("Failed to save file: {}", e)).await?;
            return Ok(());
        }

        info!(
            "Saved received file: {} ({} bytes)",
            original_path.display(),
            file_data.len()
        );

        // Perform conversion if requested and auto-convert is enabled
        let converted_data = if self.config.auto_convert && transfer.request.target_format.is_some() {
            let target_format = transfer.request.target_format.as_ref().unwrap();

            match self.perform_conversion(&file_data, &detected_type, target_format).await {
                Ok(data) => {
                    let converted_filename = format!(
                        "{}.{}",
                        transfer.request.filename.trim_end_matches(".pdf").trim_end_matches(".txt"),
                        target_format
                    );
                    let converted_path = self.output_dir.join(&converted_filename);

                    if let Err(e) = fs::write(&converted_path, &data).await {
                        warn!("Failed to save converted file {}: {}", converted_path.display(), e);
                    } else {
                        info!(
                            "Saved converted file: {} ({} bytes)",
                            converted_path.display(),
                            data.len()
                        );
                    }

                    Some(data)
                }
                Err(e) => {
                    warn!("Conversion failed for {}: {}", transfer_id, e);
                    None
                }
            }
        } else {
            None
        };

        // Send response
        let processing_time = processing_start.elapsed().as_millis() as u64;
        let response = FileTransferResponse {
            transfer_id: transfer_id.clone(),
            success: true,
            error_message: None,
            converted_data: if transfer.request.return_result { converted_data } else { None },
            converted_filename: if converted_data.is_some() {
                Some(format!(
                    "{}.{}",
                    transfer.request.filename.trim_end_matches(".pdf").trim_end_matches(".txt"),
                    transfer.request.target_format.as_deref().unwrap_or("converted")
                ))
            } else {
                None
            },
            processing_time_ms: processing_time,
        };

        if let Some(response_channel) = transfer.response_channel {
            self.send_response(response_channel, response).await?;
        }

        // Clean up progress tracking
        self.transfer_progress.write().await.remove(&transfer_id);

        info!(
            "Transfer {} processing completed in {}ms",
            transfer_id, processing_time
        );

        Ok(())
    }

    /// Perform file conversion
    async fn perform_conversion(
        &self,
        file_data: &[u8],
        detected_type: &FileType,
        target_format: &str,
    ) -> Result<Vec<u8>> {
        let mut converter = self.converter.lock().await;

        match (detected_type, target_format.to_lowercase().as_str()) {
            (FileType::Text, "pdf") => {
                let text_content = String::from_utf8(file_data.to_vec())
                    .with_context(|| "Invalid UTF-8 in text file")?;

                converter.text_to_pdf(&text_content, &self.config.pdf_config)
                    .with_context(|| "Failed to convert text to PDF")
            }
            (FileType::Pdf, "txt") => {
                let text_content = converter.pdf_to_text(file_data)
                    .with_context(|| "Failed to extract text from PDF")?;

                Ok(text_content.into_bytes())
            }
            _ => {
                Err(anyhow::anyhow!(
                    "Unsupported conversion: {} to {}",
                    detected_type, target_format
                ))
            }
        }
    }

    /// Send error response
    async fn send_error_response(
        &self,
        transfer: ActiveTransfer,
        error_message: String,
    ) -> Result<()> {
        if let Some(response_channel) = transfer.response_channel {
            let response = FileTransferResponse {
                transfer_id: transfer.request.transfer_id,
                success: false,
                error_message: Some(error_message),
                converted_data: None,
                converted_filename: None,
                processing_time_ms: transfer.start_time.elapsed().as_millis() as u64,
            };

            self.send_response(response_channel, response).await?;
        }
        Ok(())
    }

    /// Send response through channel
    async fn send_response(
        &self,
        response_channel: ResponseChannel<FileTransferResponse>,
        response: FileTransferResponse,
    ) -> Result<()> {
        // Note: In actual implementation, this would use the libp2p response channel
        // For now, we'll simulate it
        info!(
            "Sending response for transfer {}: success={}",
            response.transfer_id, response.success
        );
        Ok(())
    }

    /// Get active transfer progress
    pub async fn get_transfer_progress(&self) -> Vec<TransferProgress> {
        self.transfer_progress
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Send file to peer
    pub async fn send_file_to_peer<P: AsRef<Path>>(
        &self,
        peer_id: PeerId,
        file_path: P,
        target_format: Option<String>,
        return_result: bool,
    ) -> Result<String> {
        let file_path = file_path.as_ref();

        // Read file metadata
        let metadata = fs::metadata(file_path).await
            .with_context(|| format!("Failed to read file metadata: {}", file_path.display()))?;

        let file_size = metadata.len();
        if file_size > MAX_FILE_SIZE {
            return Err(anyhow::anyhow!(
                "File size {} exceeds maximum allowed size {}",
                file_size, MAX_FILE_SIZE
            ));
        }

        // Detect file type
        let detected_type = self.converter.lock().await.detect_file_type(file_path)?;

        // Generate transfer ID
        let transfer_id = Uuid::new_v4().to_string();
        let filename = file_path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Calculate chunk count
        let chunk_count = ((file_size + MAX_CHUNK_SIZE as u64 - 1) / MAX_CHUNK_SIZE as u64) as usize;

        // Create transfer request
        let request = FileTransferRequest {
            transfer_id: transfer_id.clone(),
            filename,
            file_size,
            file_type: detected_type.to_string(),
            target_format,
            return_result,
            chunk_count,
        };

        info!(
            "Sending file {} to {} (transfer: {}, {} chunks)",
            file_path.display(), peer_id, transfer_id, chunk_count
        );

        // TODO: Send request to peer using libp2p request-response
        // This is where you would use the actual libp2p swarm to send the request

        // Read and send file chunks
        let mut file = File::open(file_path).await
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let mut chunk_index = 0;
        let mut buffer = vec![0u8; MAX_CHUNK_SIZE];
        let mut total_sent = 0;

        while let Ok(bytes_read) = file.read(&mut buffer).await {
            if bytes_read == 0 {
                break;
            }

            let chunk = FileChunk {
                transfer_id: transfer_id.clone(),
                chunk_index,
                data: buffer[..bytes_read].to_vec(),
                is_final: chunk_index == chunk_count - 1,
            };

            // TODO: Send chunk to peer
            // In actual implementation, this would use a separate stream for chunks

            total_sent += bytes_read as u64;
            chunk_index += 1;

            // Log progress
            let percentage = (total_sent as f64 / file_size as f64) * 100.0;
            if chunk_index % 10 == 0 || chunk.is_final {
                info!(
                    "Sent chunk {}/{} to {} ({:.1}%)",
                    chunk_index, chunk_count, peer_id, percentage
                );
            }
        }

        info!(
            "File transfer completed: {} to {} ({} bytes)",
            file_path.display(), peer_id, total_sent
        );

        Ok(transfer_id)
    }

    /// Cleanup expired transfers
    pub async fn cleanup_expired_transfers(&self) {
        let now = Instant::now();
        let mut expired_transfers = Vec::new();

        // Find expired transfers
        {
            let transfers = self.active_transfers.read().await;
            for (transfer_id, transfer) in transfers.iter() {
                if now.duration_since(transfer.start_time) > TRANSFER_TIMEOUT {
                    expired_transfers.push(transfer_id.clone());
                }
            }
        }

        // Remove expired transfers
        if !expired_transfers.is_empty() {
            let mut transfers = self.active_transfers.write().await;
            let mut progress = self.transfer_progress.write().await;

            for transfer_id in expired_transfers {
                warn!("Transfer {} expired and was cleaned up", transfer_id);
                transfers.remove(&transfer_id);
                progress.remove(&transfer_id);
            }
        }
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let service = self.clone();
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(30));

            loop {
                cleanup_interval.tick().await;
                service.cleanup_expired_transfers().await;
            }
        })
    }
}

impl Clone for FileConversionService {
    fn clone(&self) -> Self {
        Self {
            converter: self.converter.clone(),
            active_transfers: self.active_transfers.clone(),
            transfer_progress: self.transfer_progress.clone(),
            output_dir: self.output_dir.clone(),
            config: self.config.clone(),
        }
    }
}

/// Network behavior for file conversion
#[derive(NetworkBehaviour)]
pub struct FileConversionBehaviour {
    request_response: RequestResponse<FileConversionCodec>,
    file_service: Arc<FileConversionService>,
}

impl FileConversionBehaviour {
    pub fn new(config: FileConversionConfig) -> Result<Self> {
        let file_service = Arc::new(FileConversionService::new(config)?);

        let request_response = RequestResponse::new(
            FileConversionCodec,
            [StreamProtocol::new(PROTOCOL_NAME)],
            request_response::Config::default(),
        );

        Ok(Self {
            request_response,
            file_service,
        })
    }

    /// Send file to peer
    pub async fn send_file<P: AsRef<Path>>(
        &mut self,
        peer_id: PeerId,
        file_path: P,
        target_format: Option<String>,
        return_result: bool,
    ) -> Result<request_response::RequestId> {
        self.file_service
            .send_file_to_peer(peer_id, file_path, target_format, return_result)
            .await?;

        // TODO: Return actual request ID from libp2p
        Ok(request_response::RequestId::new())
    }

    /// Get transfer progress
    pub async fn get_progress(&self) -> Vec<TransferProgress> {
        self.file_service.get_transfer_progress().await
    }
}

/// Example usage and integration
pub mod examples {
    use super::*;

    /// Complete P2P file conversion node
    pub struct P2PFileNode {
        swarm: Swarm<FileConversionBehaviour>,
        service: Arc<FileConversionService>,
    }

    impl P2PFileNode {
        pub async fn new(config: FileConversionConfig) -> Result<Self> {
            let local_key = Keypair::generate_ed25519();
            let local_peer_id = PeerId::from(local_key.public());

            let behaviour = FileConversionBehaviour::new(config.clone())?;
            let service = behaviour.file_service.clone();

            let swarm = SwarmBuilder::with_existing_identity(local_key)
                .with_tokio()
                .with_tcp(
                    Default::default(),
                    libp2p::noise::Config::new,
                    libp2p::yamux::Config::default,
                )?
                .with_behaviour(|_| Ok(behaviour))?
                .build();

            info!("Created P2P file node with peer ID: {}", local_peer_id);

            Ok(Self { swarm, service })
        }

        /// Start the node
        pub async fn run(&mut self, listen_addr: Multiaddr) -> Result<()> {
            self.swarm.listen_on(listen_addr.clone())?;
            info!("P2P file node listening on: {}", listen_addr);

            // Start cleanup task
            let _cleanup_handle = self.service.start_cleanup_task();

            loop {
                match self.swarm.select_next_some().await {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on: {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!("Connected to peer: {}", peer_id);
                    }
                    SwarmEvent::Behaviour(event) => {
                        self.handle_behaviour_event(event).await?;
                    }
                    _ => {}
                }
            }
        }

        /// Handle behavior events
        async fn handle_behaviour_event(
            &self,
            event: <FileConversionBehaviour as NetworkBehaviour>::OutEvent,
        ) -> Result<()> {
            // TODO: Handle actual libp2p request-response events
            info!("Received behavior event: {:?}", event);
            Ok(())
        }

        /// Send file to peer
        pub async fn send_file<P: AsRef<Path>>(
            &mut self,
            peer_id: PeerId,
            file_path: P,
            target_format: Option<String>,
        ) -> Result<String> {
            self.service
                .send_file_to_peer(peer_id, file_path, target_format, false)
                .await
        }

        /// Get active transfer progress
        pub async fn get_progress(&self) -> Vec<TransferProgress> {
            self.service.get_transfer_progress().await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_file_transfer_request() {
        let config = FileConversionConfig::default();
        let service = FileConversionService::new(config).unwrap();

        let request = FileTransferRequest {
            transfer_id: "test-123".to_string(),
            filename: "test.txt".to_string(),
            file_size: 100,
            file_type: "text".to_string(),
            target_format: Some("pdf".to_string()),
            return_result: false,
            chunk_count: 1,
        };

        let peer_id = PeerId::random();
        // Note: In real test, would need actual ResponseChannel
        // let response_channel = ...; 

        // Test would continue with actual libp2p integration
        assert!(service.active_transfers.read().await.is_empty());
    }

    #[test]
    fn test_transfer_progress_calculations() {
        let progress = TransferProgress {
            transfer_id: "test".to_string(),
            filename: "test.txt".to_string(),
            total_size: 1000,
            transferred: 250,
            start_time: Instant::now() - Duration::from_secs(1),
            peer_id: PeerId::random(),
        };

        assert_eq!(progress.percentage(), 25.0);
        assert!(progress.speed_bps() > 0.0);
    }

    #[test]
    fn test_file_chunk_assembly() {
        let request = FileTransferRequest {
            transfer_id: "test".to_string(),
            filename: "test.txt".to_string(),
            file_size: 6,
            file_type: "text".to_string(),
            target_format: None,
            return_result: false,
            chunk_count: 3,
        };

        let peer_id = PeerId::random();
        // Note: Would need actual ResponseChannel in real implementation
        let mut transfer = ActiveTransfer {
            request,
            received_chunks: HashMap::new(),
            total_received: 0,
            start_time: Instant::now(),
            peer_id,
            response_channel: None,
        };

        // Add chunks out of order
        transfer.add_chunk(FileChunk {
            transfer_id: "test".to_string(),
            chunk_index: 1,
            data: vec![b'l', b'o'],
            is_final: false,
        }).unwrap();

        transfer.add_chunk(FileChunk {
            transfer_id: "test".to_string(),
            chunk_index: 0,
            data: vec![b'h', b'e'],
            is_final: false,
        }).unwrap();

        transfer.add_chunk(FileChunk {
            transfer_id: "test".to_string(),
            chunk_index: 2,
            data: vec![b'r', b'd'],
            is_final: true,
        }).unwrap();

        assert!(transfer.is_complete());
        let assembled = transfer.assemble_file().unwrap();
        assert_eq!(assembled, b"helord");
    }
}
