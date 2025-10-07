# P2P File Transfer System Usage

## Quick Start

### 1. Start a File Receiver Node

```bash
# Basic receiver (listens on default port)
cargo run --bin p2p-file-node

# Custom configuration
cargo run --bin p2p-file-node -- \
  --listen /ip4/0.0.0.0/tcp/8080 \
  --output-dir ./my_files \
  --auto-convert \
  --verbose
```

### 2. Send Files Using Client

```bash
# Send a single file
cargo run --bin p2p-file-client -- \
  --peer /ip4/127.0.0.1/tcp/8080/p2p/12D3K... \
  send --file document.txt --format pdf

# Send multiple files
cargo run --bin p2p-file-client -- \
  --peer /ip4/127.0.0.1/tcp/8080/p2p/12D3K... \
  batch --dir ./documents --pattern "*.txt" --format pdf
```

## Architecture Overview

```
┌─────────────────┐    File Transfer     ┌─────────────────┐
│   File Client   │ ──────────────────► │   File Node     │
│   (Sender)      │    /convert/1.0.0    │   (Receiver)    │
└─────────────────┘                      └─────────────────┘
         │                                         │
         │ 1. FileTransferRequest                  │ 2. Detect File Type
         │ 2. FileChunk[]                          │ 3. Save Original
         │ 3. Wait for Response                    │ 4. Convert (optional)
         │                                         │ 5. Send Response
         └◄────────── FileTransferResponse ────────┘
```

## Protocol Flow

### 1. Request Phase
- Client sends `FileTransferRequest` with metadata
- Server validates request (size limits, concurrent transfers)
- Server responds with accept/reject

### 2. Transfer Phase  
- Client sends file in chunks (`FileChunk[]`)
- Server tracks progress and assembles chunks
- Progress is logged and monitored

### 3. Processing Phase
- Server detects file type using magic numbers
- Automatic conversion (if enabled and requested)
- Files saved to configured output directory

### 4. Response Phase
- Server sends `FileTransferResponse` with results
- Optional: converted file data returned to client

## Configuration

Create `config.toml`:

```toml
[conversion]
max_concurrent_transfers = 5
output_dir = "./received_files"
auto_convert = true
return_results = false

[conversion.pdf_config]
title = "Converted Document"
margins = 20
font_size = 12
line_spacing = 1.2

[network]
max_file_size_mb = 100
chunk_size_kb = 1024
transfer_timeout_seconds = 300
```

## Examples

### Run Simple Transfer Example
```bash
cargo run --example simple_transfer
```

### Run Batch Conversion Example  
```bash
cargo run --example batch_conversion
```

## Features

✅ **Chunked File Transfer** - Large files split into manageable chunks  
✅ **Progress Tracking** - Real-time transfer progress with ETA  
✅ **File Type Detection** - Automatic detection via magic numbers  
✅ **Format Conversion** - Text ↔ PDF conversion with configurable styling  
✅ **Error Handling** - Comprehensive error recovery and reporting  
✅ **Concurrent Transfers** - Multiple simultaneous file transfers  
✅ **Timeout Management** - Automatic cleanup of stalled transfers  
✅ **CLI Interface** - Easy-to-use command-line tools  

## Protocol Specification

### Message Types

#### FileTransferRequest
```rust
{
    transfer_id: String,     // Unique transfer identifier
    filename: String,        // Original filename
    file_size: u64,         // Size in bytes
    file_type: String,      // Detected file type
    target_format: Option<String>, // Requested conversion
    return_result: bool,    // Whether to return converted data
    chunk_count: usize,     // Number of chunks to follow
}
```

#### FileChunk
```rust
{
    transfer_id: String,    // Links to original request
    chunk_index: usize,     // 0-based sequence number
    data: Vec<u8>,         // Chunk data
    is_final: bool,        // Last chunk indicator
}
```

#### FileTransferResponse  
```rust
{
    transfer_id: String,           // Links to original request
    success: bool,                 // Transfer success status
    error_message: Option<String>, // Error details if failed
    converted_data: Option<Vec<u8>>, // Converted file (if requested)
    converted_filename: Option<String>, // New filename
    processing_time_ms: u64,       // Processing duration
}
```

## Error Handling

The system handles various error conditions:

- **File Size Limits** - Rejects files exceeding configured maximum
- **Network Timeouts** - Automatic cleanup of stalled transfers  
- **Conversion Failures** - Graceful handling of unsupported formats
- **Concurrent Limits** - Queue management for busy servers
- **Disk Space** - Checks available space before transfers
- **Corrupted Data** - Validates file integrity and chunks

## Monitoring

### Progress Tracking
- Real-time transfer speed (KB/s)
- Percentage completion  
- Estimated time remaining (ETA)
- Peer identification

### Logging Levels
- **ERROR** - Critical failures and errors
- **WARN** - Non-fatal issues and warnings
- **INFO** - General operation information (default)
- **DEBUG** - Detailed debugging information

## Security Considerations

- **File Size Limits** - Prevents resource exhaustion attacks
- **Type Validation** - Only processes supported file types
- **Timeout Protection** - Prevents indefinite resource consumption
- **Path Sanitization** - Prevents directory traversal attacks
- **Memory Management** - Streaming prevents memory exhaustion

## Performance

### Optimizations
- Chunked transfer reduces memory usage
- Concurrent processing improves throughput  
- Progress tracking minimizes blocking operations
- Async I/O maximizes CPU utilization

### Benchmarks
- **Small Files** (<1MB): ~50ms processing time
- **Medium Files** (1-10MB): ~200-500ms processing time
- **Large Files** (10-100MB): Streaming with progress tracking
- **Concurrent Transfers**: Up to 5 simultaneous (configurable)

## Troubleshooting

### Common Issues

**Connection Refused**
- Verify the receiver node is running
- Check firewall settings
- Confirm multiaddr format is correct

**Transfer Timeouts**  
- Check network connectivity
- Increase timeout in configuration
- Monitor system resources

**Conversion Failures**
- Verify file format is supported
- Check font availability for PDF generation
- Review error logs for specific details

**Memory Issues**
- Reduce concurrent transfer limit
- Check available disk space
- Monitor chunk size configuration
