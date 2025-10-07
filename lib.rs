//! P2P File Converter Library
//! 
//! This crate provides peer-to-peer file conversion capabilities using libp2p.

pub mod network;
pub mod conversion;
pub mod error;
pub mod config;

pub use error::{P2PError, Result};
pub use config::Config;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::{
        network::P2PBehaviour,
        conversion::FileConverter,
        error::{P2PError, Result},
        config::Config,
    };

    pub use libp2p::{PeerId, Multiaddr};
    pub use tokio;
    pub use tracing::{debug, error, info, warn};
}
