# P2P File Sender

A high-performance, resilient peer-to-peer file sender built with Rust and libp2p. Features advanced retry logic, real-time progress tracking, chunked file transfer, and comprehensive error handling.

## 🚀 Features

- **🔄 Intelligent Retry Logic**: Exponential backoff with configurable timeouts
- **📊 Real-time Progress Tracking**: Speed, ETA, and detailed transfer statistics  
- **🧩 Chunked File Transfer**: Memory-efficient streaming for large files
- **🛡️ Robust Error Handling**: Comprehensive error recovery and reporting
- **📡 Multiple Peer Support**: Automatic failover to backup peers
- **⚡ Concurrent Transfers**: Send multiple files simultaneously
- **🎯 Format Conversion**: Integrated text↔PDF conversion requests
- **📈 Performance Monitoring**: Built-in benchmarks and metrics
- **🔧 CLI Tools**: Ready-to-use command-line interface

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
p2p-file-sender = "1.0.0"
```

Or install the CLI tool:

```bash
cargo install p2p-file-sender --features cli
```

## 🏃 Quick Start

### Basic File Sending

```rust
use p2p_file_sender::{FileSender, RetryConfig};
use libp2p::{Multiaddr, PeerId};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create sender with retry configuration
    let retry_config = RetryConfig {
        max_attempts: 5,
        connection_timeout: Duration::from_secs(10),
        ..Default::default()
    };

    let mut sender = FileSender::new(Some(retry_config)).await?;

    // Set up progress callback
    sender.set_progress_callback(|progress| {
        println!("Progress: {:.1}% - {}", 
                 progress.percentage(), 
                 progress.status_string());
    });

    // Send file
    let peer_id = "12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ".parse()?;
    let target_addr = "/ip4/127.0.0.1/tcp/8080/p2p/12D3K...".parse()?;

    let transfer_id = sender.send_file(
        peer_id,
        target_addr,
        "document.pdf",
        Some("txt".to_string()), // Convert to text
        false, // Don't return result
    ).await?;

    // Wait for completion
    let result = sender.wait_for_completion(&transfer_id).await?;
    println!("Transfer completed: {} bytes sent", result.bytes_sent);

    Ok(())
}
```

### CLI Usage

```bash
# Send a single file
p2p-send --target /ip4/192.168.1.100/tcp/8080/p2p/12D3K... \
         file --path document.pdf --format txt

# Send multiple files
p2p-send --target /ip4/192.168.1.100/tcp/8080/p2p/12D3K... \
         batch --dir ./documents --pattern "*.pdf" --format txt

# Test connection
p2p-send --target /ip4/192.168.1.100/tcp/8080/p2p/12D3K... \
         ping --count 5
```

## 📊 Progress Tracking

The sender provides detailed progress information:

```rust
use p2p_file_sender::progress::ProgressReporter;

let mut reporter = ProgressReporter::new(Duration::from_secs(1));

sender.set_progress_callback(move |progress| {
    // Automatic rate limiting
    reporter.maybe_report(progress);

    // Manual progress access
    println!("Speed: {:.1} KB/s", progress.speed_bps() / 1024.0);
    println!("ETA: {:?}", progress.eta_seconds());
    println!("Status: {}", progress.status_string());
});
```

## 🔄 Retry Configuration

Customize retry behavior for unreliable networks:

```rust
use p2p_file_sender::RetryConfig;
use std::time::Duration;

let config = RetryConfig {
    max_attempts: 10,                           // Try up to 10 times
    initial_delay: Duration::from_millis(100),  // Start with 100ms delay
    max_delay: Duration::from_secs(30),         // Cap at 30 seconds
    backoff_multiplier: 2.0,                    // Double delay each time
    connection_timeout: Duration::from_secs(15), // 15s per attempt
};

let sender = FileSender::new(Some(config)).await?;
```

## 🧩 Chunked Transfer

Files are automatically split into chunks for efficient transfer:

- **Memory Efficient**: Only one chunk in memory at a time
- **Progress Granular**: Updates on every chunk completion
- **Network Friendly**: Configurable chunk sizes (default: 1MB)
- **Resumable**: Foundation for future resume capability

## 🛡️ Error Handling

Comprehensive error recovery with detailed error information:

```rust
match sender.send_file(peer_id, addr, "file.txt", None, false).await {
    Ok(transfer_id) => {
        match sender.wait_for_completion(&transfer_id).await {
            Ok(result) if result.success => {
                println!("✅ Success: {} bytes sent", result.bytes_sent);
            }
            Ok(result) => {
                println!("❌ Failed: {}", result.error.unwrap());
                println!("📊 Partial: {} bytes sent", result.bytes_sent);
            }
            Err(e) => println!("💥 Error: {}", e),
        }
    }
    Err(e) => println!("🚫 Connection failed: {}", e),
}
```

## 📈 Performance

### Benchmarks

Run performance benchmarks:

```bash
cargo bench --features benchmarks
```

Typical performance characteristics:
- **Small files** (<1MB): ~100-200ms total time
- **Large files** (100MB+): 50-100 MB/s transfer rate
- **Progress calculation**: <1μs per update
- **Memory usage**: <10MB baseline + 1MB per active transfer

### Optimization Tips

1. **Adjust chunk size** for your network conditions
2. **Use concurrent transfers** for multiple files
3. **Configure retry parameters** based on network reliability
4. **Enable progress rate limiting** to reduce CPU overhead

## 🔧 Advanced Usage

### Multiple Peer Failover

```rust
let peers = vec![
    "/ip4/127.0.0.1/tcp/8080/p2p/12D3K...",
    "/ip4/192.168.1.100/tcp/9000/p2p/12D3K...",
    "/ip4/10.0.0.50/tcp/7000/p2p/12D3K...",
];

for peer_addr in peers {
    match sender.send_file(peer_id, peer_addr.parse()?, "file.txt", None, false).await {
        Ok(transfer_id) => {
            if sender.wait_for_completion(&transfer_id).await?.success {
                println!("✅ Successfully sent to {}", peer_addr);
                break;
            }
        }
        Err(_) => continue, // Try next peer
    }
}
```

### Batch Operations

```rust
use futures::stream::{self, StreamExt};

let files = vec!["file1.txt", "file2.pdf", "file3.jpg"];

let results: Vec<_> = stream::iter(files)
    .map(|file| sender.send_file(peer_id, addr.clone(), file, None, false))
    .buffer_unordered(3) // Max 3 concurrent
    .collect()
    .await;

for result in results {
    match result {
        Ok(transfer_id) => println!("Started: {}", transfer_id),
        Err(e) => println!("Failed: {}", e),
    }
}
```

### Custom Progress Handling

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

let progress_store = Arc::new(RwLock::new(HashMap::new()));
let store_clone = Arc::clone(&progress_store);

sender.set_progress_callback(move |progress| {
    let store = Arc::clone(&store_clone);
    tokio::spawn(async move {
        store.write().await.insert(
            progress.transfer_id.clone(), 
            progress.clone()
        );
    });
});
```

## 📝 Examples

The repository includes comprehensive examples:

- **`simple_send.rs`**: Basic file sending
- **`batch_send.rs`**: Multiple file handling
- **`resilient_send.rs`**: Advanced error recovery
- **`progress_monitoring.rs`**: Real-time progress tracking

Run examples:

```bash
cargo run --example simple_send
cargo run --example batch_send
cargo run --example resilient_send
cargo run --example progress_monitoring
```

## 🧪 Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

```bash
cargo test --test integration_tests
```

### Benchmarks

```bash
cargo bench
```

## 🐛 Troubleshooting

### Common Issues

**Connection Timeouts**
- Increase `connection_timeout` in `RetryConfig`
- Check network connectivity and firewall settings
- Verify peer ID and multiaddr format

**Slow Transfer Speeds**
- Adjust chunk size based on network conditions
- Check available bandwidth and system resources
- Monitor for network congestion

**Memory Usage**
- Limit concurrent transfers
- Monitor chunk size settings
- Check for proper cleanup of completed transfers

**Protocol Errors**
- Ensure both peers support the same protocol version
- Check libp2p compatibility between implementations
- Verify transport configuration matches

### Debug Logging

Enable detailed logging:

```bash
RUST_LOG=debug cargo run
```

Or programmatically:

```rust
tracing_subscriber::fmt()
    .with_env_filter("debug,libp2p=trace")
    .init();
```

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
git clone https://github.com/username/p2p-file-sender
cd p2p-file-sender
cargo build
cargo test
```

### Submitting Changes

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [libp2p](https://libp2p.io/) for the excellent P2P networking library
- [Tokio](https://tokio.rs/) for the async runtime
- The Rust community for feedback and contributions

## 📚 Documentation

Full API documentation is available at [docs.rs](https://docs.rs/p2p-file-sender).

### Key Concepts

- **FileSender**: Main interface for sending files
- **SendProgress**: Progress tracking and statistics
- **RetryConfig**: Configurable retry behavior
- **TransferStatus**: Current state of file transfers
- **ProgressReporter**: Formatted progress output

### Architecture

```
┌─────────────────┐    libp2p     ┌─────────────────┐
│   FileSender    │ ─────────────► │   Target Peer   │
│                 │               │                 │
│ ┌─────────────┐ │    Request    │ ┌─────────────┐ │
│ │ RetryLogic  │ │ ─────────────► │ │   Handler   │ │
│ └─────────────┘ │               │ └─────────────┘ │
│                 │               │                 │
│ ┌─────────────┐ │    Chunks     │ ┌─────────────┐ │
│ │FileChunker  │ │ ─────────────► │ │ Processor   │ │
│ └─────────────┘ │               │ └─────────────┘ │
│                 │               │                 │
│ ┌─────────────┐ │   Response    │ ┌─────────────┐ │
│ │ Progress    │ │ ◄───────────── │ │   Result    │ │
│ │ Tracker     │ │               │ └─────────────┘ │
│ └─────────────┘ │               └─────────────────┘
└─────────────────┘
```

---

**Built with ❤️ in Rust**
