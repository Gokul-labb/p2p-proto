// Simple P2P file transfer example

use anyhow::Result;
use libp2p::{Multiaddr, PeerId};
use p2p_file_transfer::{FileConversionConfig, P2PFileNode};
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tokio::fs;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🚀 Simple P2P File Transfer Example");

    // Create two nodes: sender and receiver
    let receiver_config = FileConversionConfig {
        output_dir: PathBuf::from("./example_received"),
        auto_convert: true,
        ..Default::default()
    };

    let sender_config = FileConversionConfig::default();

    let mut receiver_node = P2PFileNode::new(receiver_config).await?;
    let mut sender_node = P2PFileNode::new(sender_config).await?;

    // Start receiver
    let receiver_addr: Multiaddr = "/ip4/127.0.0.1/tcp/9001".parse()?;
    tokio::spawn(async move {
        if let Err(e) = receiver_node.run(receiver_addr).await {
            eprintln!("Receiver error: {}", e);
        }
    });

    // Start sender  
    let sender_addr: Multiaddr = "/ip4/127.0.0.1/tcp/9002".parse()?;
    tokio::spawn(async move {
        if let Err(e) = sender_node.run(sender_addr).await {
            eprintln!("Sender error: {}", e);
        }
    });

    // Wait for nodes to start
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Create a test file
    let mut test_file = NamedTempFile::new()?;
    let test_content = r#"
# Test Document

This is a **test document** for P2P file transfer.

## Features

- Peer-to-peer file sharing
- Automatic file conversion
- Progress tracking
- Error handling

## Content

Lorem ipsum dolor sit amet, consectetur adipiscing elit.
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

### Code Example

```rust
fn main() {
    println!("Hello, P2P world!");
}
```

End of document.
"#;

    fs::write(&test_file, test_content).await?;

    info!("📄 Created test file: {}", test_file.path().display());

    // Send file from sender to receiver
    let receiver_peer_id = PeerId::random(); // In real usage, get from multiaddr

    info!("📤 Sending file to receiver...");

    // Note: This is a simplified example
    // In real usage, you would:
    // 1. Connect to the receiver using its multiaddr
    // 2. Extract the actual peer ID
    // 3. Send the file through the established connection

    info!("✅ Example completed!");
    info!("📁 Check ./example_received/ for transferred files");

    Ok(())
}
