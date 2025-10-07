use thiserror::Error;

/// Custom error types for the P2P file converter
#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Network error: {0}")]
    Network(#[from] libp2p::swarm::SwarmError),

    #[error("Transport error: {0}")]
    Transport(#[from] libp2p::TransportError<std::io::Error>),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PDF processing error: {0}")]
    Pdf(String),

    #[error("File conversion error: {0}")]
    Conversion(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Peer connection error: {0}")]
    PeerConnection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// Convenience Result type with P2PError
pub type Result<T> = std::result::Result<T, P2PError>;
