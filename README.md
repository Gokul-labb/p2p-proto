# P2P File Converter

A peer-to-peer file converter built with Rust and libp2p that allows distributed file format conversion across a decentralized network.

## Features

- **Peer-to-peer networking** using libp2p with TCP transport, Noise encryption, and Yamux multiplexing
- **File format conversion** supporting PDF and text formats
- **Automatic peer discovery** via mDNS (Multicast DNS)
- **CLI interface** with command parsing using clap
- **Async runtime** powered by Tokio
- **Comprehensive logging** using tracing and tracing-subscriber
- **Error handling** with anyhow and thiserror

## Dependencies

- `libp2p` - P2P networking framework with TCP, Noise, and Yamux features
- `tokio` - Async runtime for Rust
- `clap` - Command line argument parser
- `genpdf` - PDF generation library
- `pdf-extract` - PDF text extraction
- `tracing` & `tracing-subscriber` - Logging and observability
- `anyhow` & `thiserror` - Error handling
- `serde` & `serde_json` - Serialization
- `futures` - Futures utilities
- `uuid` - UUID generation

## Project Structure

```
p2p-file-converter/
├── Cargo.toml          # Project dependencies and metadata
├── main.rs             # Main application entry point
├── README.md           # Project documentation
└── fonts/              # Font files for PDF generation (optional)
```

## Building and Running

### Prerequisites

1. **Rust toolchain** (install from [rustup.rs](https://rustup.rs/))
2. **Font files** (optional, for PDF generation):
   ```bash
   mkdir fonts
   # Add LiberationSans font files to fonts/ directory
   ```

### Build the project

```bash
cargo build
```

### Run the application

```bash
# Start with default settings
cargo run

# Specify a custom listen address
cargo run -- --listen /ip4/0.0.0.0/tcp/8080

# Connect to a specific peer
cargo run -- --peer /ip4/127.0.0.1/tcp/8080
```

## Usage

Once running, the application accepts the following commands:

- `peers` - List all connected peers
- `connect <multiaddr>` - Connect to a peer at the specified address
- `quit` or `exit` - Shutdown the application

Example multiaddresses:
- `/ip4/127.0.0.1/tcp/8080` - Local TCP connection
- `/ip4/192.168.1.100/tcp/9000` - Remote TCP connection

## File Conversion

The application includes a file conversion module that supports:

- **PDF to Text**: Extract text content from PDF files
- **Text to PDF**: Convert plain text files to PDF format

*Note: The current implementation provides the foundation for file conversion. Additional features like P2P file transfer and distributed conversion requests can be built on top of this base.*

## Architecture

### Network Behavior

The P2P network behavior combines:
- **Identity Protocol**: Peer identification and capability exchange
- **mDNS**: Automatic local network peer discovery  
- **Ping Protocol**: Connection health monitoring

### Event Loop

The application runs an async event loop that handles:
- User input from the CLI
- Network events from the libp2p swarm
- File conversion requests (extensible)

### Error Handling

Comprehensive error handling using:
- `anyhow::Result` for application-level errors
- `thiserror` for custom error types
- Proper error propagation throughout the codebase

## Logging

Structured logging with different levels:
- `ERROR`: Critical errors
- `WARN`: Warning messages  
- `INFO`: General information (default level)
- `DEBUG`: Detailed debugging information

Set log level with the `RUST_LOG` environment variable:
```bash
RUST_LOG=debug cargo run
```

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Future Enhancements

- **Distributed file processing**: Split large file conversions across multiple peers
- **File transfer protocol**: Direct P2P file sharing
- **Additional format support**: Images, documents, media files
- **Web interface**: Browser-based UI for easier interaction
- **Persistent storage**: Save conversion history and peer information
- **Authentication**: Secure peer verification and access control
