use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the P2P file converter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Address to listen on for incoming connections
    pub listen_addr: Multiaddr,

    /// Local peer ID
    pub peer_id: Option<PeerId>,

    /// Bootstrap peers to connect to
    pub bootstrap_peers: Vec<Multiaddr>,

    /// Maximum number of concurrent connections
    pub max_connections: usize,

    /// File conversion settings
    pub conversion: ConversionConfig,

    /// Network settings
    pub network: NetworkConfig,
}

/// File conversion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionConfig {
    /// Maximum file size for conversion (in bytes)
    pub max_file_size: usize,

    /// Supported input formats
    pub supported_inputs: Vec<String>,

    /// Supported output formats
    pub supported_outputs: Vec<String>,

    /// Temporary directory for file processing
    pub temp_dir: PathBuf,

    /// Font directory for PDF generation
    pub font_dir: Option<PathBuf>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable mDNS discovery
    pub enable_mdns: bool,

    /// Connection timeout in seconds
    pub connection_timeout: u64,

    /// Keep-alive interval in seconds
    pub keep_alive_interval: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
            peer_id: None,
            bootstrap_peers: Vec::new(),
            max_connections: 50,
            conversion: ConversionConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10 MB
            supported_inputs: vec!["txt".to_string(), "pdf".to_string()],
            supported_outputs: vec!["txt".to_string(), "pdf".to_string()],
            temp_dir: std::env::temp_dir(),
            font_dir: None,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            connection_timeout: 30,
            keep_alive_interval: 60,
        }
    }
}
