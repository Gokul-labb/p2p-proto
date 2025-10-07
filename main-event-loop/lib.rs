//! P2P File Converter - Complete Implementation
//! 
//! A peer-to-peer file converter built with Rust and libp2p that allows
//! distributed file format conversion across a decentralized network.

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod cli;
pub mod config;
pub mod error;
pub mod file_converter;
pub mod file_sender;
pub mod p2p_stream_handler;
pub mod main_event_loop;

// Re-export commonly used types
pub use cli::{CliArgs, AppMode};
pub use config::Config;
pub use error::{P2PError, Result};
pub use file_converter::{FileConverter, FileType, PdfConfig};
pub use file_sender::{FileSender, RetryConfig, SendProgress, TransferStatus};
pub use p2p_stream_handler::{
    FileConversionService, FileConversionConfig, P2PFileNode, 
    TransferProgress, FileTransferRequest, FileTransferResponse
};
pub use main_event_loop::{P2PFileConverter, ShutdownReason, AppState};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        CliArgs, AppMode, Config, P2PError, Result,
        FileConverter, FileType, PdfConfig,
        FileSender, RetryConfig, SendProgress, TransferStatus,
        FileConversionService, P2PFileNode, TransferProgress,
        P2PFileConverter, ShutdownReason, AppState,
    };

    pub use libp2p::{PeerId, Multiaddr};
    pub use tokio;
    pub use tracing::{debug, error, info, warn};
    pub use anyhow::{Context, Result as AnyhowResult};
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

/// Protocol version
pub const PROTOCOL_VERSION: &str = "/convert/1.0.0";
