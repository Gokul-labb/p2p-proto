# Usage Examples and Scenarios

This document provides comprehensive examples for using the P2P File Converter in various scenarios.

## Table of Contents

1. [Basic Usage](#basic-usage)
2. [Network Configurations](#network-configurations)
3. [File Conversion Examples](#file-conversion-examples)
4. [Advanced Scenarios](#advanced-scenarios)
5. [Automation and Scripting](#automation-and-scripting)
6. [Troubleshooting Examples](#troubleshooting-examples)

## Basic Usage

### Simple File Transfer

**Scenario**: Transfer a text file from one machine to another without conversion.

**Receiver (Machine A)**:
```bash
# Start receiver on default port
p2p-converter

# Output:
# 🌐 Listening on: /ip4/192.168.1.100/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ
# 📁 Output directory: ./output
# 📋 Commands: status, peers, stats, quit
```

**Sender (Machine B)**:
```bash
# Send file to receiver
p2p-converter --target /ip4/192.168.1.100/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ \
              --file document.txt

# Output:
# 📤 Connecting to peer...
# 🤝 Protocol negotiation successful
# 📊 Progress: 100% - Transfer completed!
# ✅ File sent successfully: 1.2 MB in 3.4s
```

### File Conversion During Transfer

**Scenario**: Convert a text document to PDF during transfer.

```bash
# Send with conversion
p2p-converter --target /ip4/192.168.1.100/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ \
              --file report.txt \
              --format pdf

# Output:
# 📤 Connecting to peer...
# 🔄 Requesting conversion to PDF
# 📊 Progress: 45.2% (1.2 MB/s) - ETA: 12s
# 🔄 File conversion in progress...
# ✅ Transfer and conversion completed!
# 📄 Converted: report.txt → report.pdf
```

## Network Configurations

### Local Network Setup

**Scenario**: Set up P2P file sharing within a local network.

**Configuration File** (`local_config.toml`):
```toml
[network]
connection_timeout_secs = 15
max_retry_attempts = 3
keep_alive = true

[files]
max_file_size = 52428800  # 50MB
output_directory = "./shared_files"
allowed_extensions = ["txt", "pdf", "md", "doc", "docx"]

[conversion]
timeout_secs = 180
parallel_processing = true
```

**Node 1** (192.168.1.100):
```bash
p2p-converter --config local_config.toml \
              --listen /ip4/192.168.1.100/tcp/8080 \
              --verbose
```

**Node 2** (192.168.1.101):
```bash
p2p-converter --config local_config.toml \
              --listen /ip4/192.168.1.101/tcp/8080 \
              --verbose
```

**File Transfer Between Nodes**:
```bash
# From Node 2 to Node 1
p2p-converter --config local_config.toml \
              --target /ip4/192.168.1.100/tcp/8080/p2p/PEER_ID \
              --file important_document.txt \
              --format pdf
```

### Internet Setup with Port Forwarding

**Scenario**: Set up P2P file sharing over the internet.

**Server Setup** (Public IP: 203.0.113.1):
```bash
# Configure firewall
sudo ufw allow 8080/tcp

# Start receiver with public binding
p2p-converter --listen /ip4/0.0.0.0/tcp/8080 \
              --output-dir ~/received_files \
              --max-size 100 \
              --verbose

# Router: Forward port 8080 to server's internal IP
```

**Client Usage**:
```bash
# Connect from anywhere on the internet
p2p-converter --target /ip4/203.0.113.1/tcp/8080/p2p/PEER_ID \
              --file presentation.pdf \
              --timeout 60 \
              --max-retries 5
```

### IPv6 Configuration

**Scenario**: Use IPv6 for peer communication.

```bash
# IPv6 receiver
p2p-converter --listen /ip6/::/tcp/8080 \
              --output-dir ./ipv6_files

# IPv6 sender
p2p-converter --target /ip6/2001:db8:85a3::8a2e:370:7334/tcp/8080/p2p/PEER_ID \
              --file data.csv
```

## File Conversion Examples

### Text to PDF Conversion

**Scenario**: Convert various text formats to PDF with custom styling.

**Configuration** (`pdf_config.toml`):
```toml
[conversion.pdf_config]
title = "Converted Document"
font_size = 11
margins = 25
line_spacing = 1.3
font_family = "Liberation Sans"
```

**Markdown to PDF**:
```bash
p2p-converter --config pdf_config.toml \
              --target PEER_ADDRESS \
              --file README.md \
              --format pdf
```

**Large Text File**:
```bash
# For large files, increase timeout
p2p-converter --target PEER_ADDRESS \
              --file large_report.txt \
              --format pdf \
              --timeout 300  # 5 minutes
```

### PDF to Text Extraction

**Scenario**: Extract text from PDF documents.

```bash
# Simple PDF to text
p2p-converter --target PEER_ADDRESS \
              --file document.pdf \
              --format txt

# Batch PDF processing
for pdf in *.pdf; do
    echo "Processing $pdf..."
    p2p-converter --target PEER_ADDRESS \
                  --file "$pdf" \
                  --format txt \
                  --verbose
done
```

### Unicode and Special Characters

**Scenario**: Handle files with Unicode content.

```bash
# Unicode text file
p2p-converter --target PEER_ADDRESS \
              --file unicode_content.txt \
              --format pdf

# Content example:
# Chinese: 你好世界
# Arabic: مرحبا بالعالم  
# Emoji: 🌍🚀📄💻
```

## Advanced Scenarios

### High-Volume File Processing

**Scenario**: Process many files efficiently with resource management.

**Configuration** (`high_volume.toml`):
```toml
[network]
connection_timeout_secs = 45
max_retry_attempts = 3

[files]
max_file_size = 209715200  # 200MB
output_directory = "./processed"

[conversion]
timeout_secs = 600  # 10 minutes
parallel_processing = true
max_memory_mb = 2048  # 2GB

[error_handling]
enable_recovery = true
log_errors = true
```

**Batch Processing Script**:
```bash
#!/bin/bash
# high_volume_process.sh

CONFIG="high_volume.toml"
TARGET_PEER="$1"
INPUT_DIR="$2"
OUTPUT_FORMAT="$3"

if [ $# -ne 3 ]; then
    echo "Usage: $0 <target_peer> <input_dir> <format>"
    exit 1
fi

echo "🚀 Starting high-volume processing..."
echo "📁 Input directory: $INPUT_DIR"
echo "🎯 Target format: $OUTPUT_FORMAT"
echo "📡 Target peer: $TARGET_PEER"

# Process files in batches
BATCH_SIZE=5
count=0
batch=1

for file in "$INPUT_DIR"/*; do
    if [ -f "$file" ]; then
        echo "📤 Processing batch $batch, file $(($count % $BATCH_SIZE + 1)): $(basename "$file")"

        p2p-converter --config "$CONFIG" \
                      --target "$TARGET_PEER" \
                      --file "$file" \
                      --format "$OUTPUT_FORMAT" &

        count=$((count + 1))

        # Wait for batch completion
        if [ $(($count % $BATCH_SIZE)) -eq 0 ]; then
            echo "⏳ Waiting for batch $batch to complete..."
            wait
            batch=$((batch + 1))
            sleep 2  # Brief pause between batches
        fi
    fi
done

# Wait for final batch
echo "⏳ Waiting for final batch to complete..."
wait

echo "✅ High-volume processing completed!"
echo "📊 Processed $count files total"
```

### Load Balancing Across Multiple Receivers

**Scenario**: Distribute files across multiple receiver nodes.

```bash
#!/bin/bash
# load_balance.sh

RECEIVERS=(
    "/ip4/192.168.1.100/tcp/8080/p2p/12D3KooWReceiver1"
    "/ip4/192.168.1.101/tcp/8080/p2p/12D3KooWReceiver2"  
    "/ip4/192.168.1.102/tcp/8080/p2p/12D3KooWReceiver3"
)

FILES_DIR="$1"
FORMAT="$2"

receiver_index=0
for file in "$FILES_DIR"/*; do
    if [ -f "$file" ]; then
        receiver="${RECEIVERS[$receiver_index]}"

        echo "📤 Sending $(basename "$file") to receiver $((receiver_index + 1))"

        p2p-converter --target "$receiver" \
                      --file "$file" \
                      --format "$FORMAT" &

        # Round-robin to next receiver
        receiver_index=$(((receiver_index + 1) % ${#RECEIVERS[@]}))
    fi
done

wait
echo "✅ Load balancing completed!"
```

### Fault-Tolerant Transfer

**Scenario**: Robust file transfer with multiple fallback peers.

```bash
#!/bin/bash
# fault_tolerant_send.sh

PRIMARY_PEER="$1"
FALLBACK_PEERS=("$2" "$3" "$4")  # Up to 3 fallback peers
FILE="$5"
FORMAT="$6"

send_file_with_fallback() {
    local peer="$1"
    local file="$2"
    local format="$3"

    echo "🎯 Attempting transfer to: $peer"

    if p2p-converter --target "$peer" \
                     --file "$file" \
                     --format "$format" \
                     --timeout 60 \
                     --max-retries 3; then
        echo "✅ Transfer successful to: $peer"
        return 0
    else
        echo "❌ Transfer failed to: $peer"
        return 1
    fi
}

# Try primary peer first
if send_file_with_fallback "$PRIMARY_PEER" "$FILE" "$FORMAT"; then
    exit 0
fi

# Try fallback peers
for fallback in "${FALLBACK_PEERS[@]}"; do
    if [ -n "$fallback" ]; then
        echo "🔄 Trying fallback peer..."
        if send_file_with_fallback "$fallback" "$FILE" "$FORMAT"; then
            exit 0
        fi
    fi
done

echo "💥 All transfer attempts failed!"
exit 1
```

## Automation and Scripting

### Automated Document Processing Workflow

**Scenario**: Set up automated processing of incoming documents.

**Receiver Script** (`auto_processor.sh`):
```bash
#!/bin/bash
# auto_processor.sh - Automated document processing

WATCH_DIR="./incoming"
PROCESSED_DIR="./processed"
CONFIG_FILE="./auto_config.toml"
PEER_ADDRESS="$1"

# Create directories
mkdir -p "$WATCH_DIR" "$PROCESSED_DIR"

# Start P2P receiver in background
p2p-converter --config "$CONFIG_FILE" \
              --listen /ip4/0.0.0.0/tcp/8080 \
              --output-dir "$WATCH_DIR" \
              --auto-convert &

RECEIVER_PID=$!

# Monitor for new files and process them
inotifywait -m -e create -e moved_to "$WATCH_DIR" |
while read dir action file; do
    if [[ "$file" == *.txt ]]; then
        echo "📄 New text file: $file"

        # Convert to PDF and send to processing peer
        p2p-converter --target "$PEER_ADDRESS" \
                      --file "$WATCH_DIR/$file" \
                      --format pdf

        # Move original to processed directory
        mv "$WATCH_DIR/$file" "$PROCESSED_DIR/"

        echo "✅ Processed: $file"
    fi
done

# Cleanup on script termination
trap "kill $RECEIVER_PID" EXIT
```

### Scheduled Backup System

**Scenario**: Automated daily backup of documents to remote peer.

**Cron Job** (`backup_cron.sh`):
```bash
#!/bin/bash
# Daily backup script - Add to crontab: 0 2 * * * /path/to/backup_cron.sh

BACKUP_SOURCE="$HOME/documents"
REMOTE_PEER="/ip4/backup-server.example.com/tcp/8080/p2p/BACKUP_PEER_ID"
LOG_FILE="/var/log/p2p_backup.log"
DATE=$(date +%Y%m%d)

echo "[$DATE] Starting daily backup..." >> "$LOG_FILE"

# Create daily archive
ARCHIVE_NAME="backup_$DATE.tar.gz"
tar -czf "/tmp/$ARCHIVE_NAME" -C "$BACKUP_SOURCE" .

# Send archive to backup peer
if p2p-converter --target "$REMOTE_PEER" \
                 --file "/tmp/$ARCHIVE_NAME" \
                 --timeout 1800 \
                 --max-retries 5 >> "$LOG_FILE" 2>&1; then
    echo "[$DATE] Backup completed successfully" >> "$LOG_FILE"
    rm "/tmp/$ARCHIVE_NAME"
else
    echo "[$DATE] Backup failed!" >> "$LOG_FILE"
    exit 1
fi
```

### Integration with File Managers

**Scenario**: Add P2P transfer to file manager context menu.

**Nautilus Script** (`~/.local/share/nautilus/scripts/Send via P2P`):
```bash
#!/bin/bash
# Nautilus context menu script

# Get default peer from config or prompt user
DEFAULT_PEER=$(grep -o 'default_peer.*' ~/.p2p_converter.conf | cut -d= -f2)

if [ -z "$DEFAULT_PEER" ]; then
    PEER=$(zenity --entry --title="P2P File Transfer" \
                  --text="Enter target peer address:")
else
    PEER="$DEFAULT_PEER"
fi

if [ -n "$PEER" ]; then
    for file in "$@"; do
        (
            echo "# Sending $(basename "$file")..."
            p2p-converter --target "$PEER" --file "$file" --verbose
        ) | zenity --progress --title="P2P Transfer" --text="Transferring files..." --pulsate
    done
fi
```

## Troubleshooting Examples

### Connection Debugging

**Scenario**: Diagnose connection issues between peers.

```bash
#!/bin/bash
# connection_debug.sh

TARGET="$1"
TEST_FILE="test.txt"

# Create test file
echo "Connection test - $(date)" > "$TEST_FILE"

echo "🔍 P2P Connection Diagnostics"
echo "=============================="

# Extract IP and port from multiaddr
IP=$(echo "$TARGET" | grep -o '/ip4/[^/]*' | cut -d/ -f3)
PORT=$(echo "$TARGET" | grep -o '/tcp/[^/]*' | cut -d/ -f3)

echo "📡 Target IP: $IP"
echo "🔌 Target Port: $PORT"

# Test basic connectivity
echo "🏓 Testing ping..."
if ping -c 3 "$IP" > /dev/null 2>&1; then
    echo "✅ Ping successful"
else
    echo "❌ Ping failed - check network connectivity"
    exit 1
fi

# Test port connectivity
echo "🔌 Testing port connectivity..."
if timeout 5 bash -c "</dev/tcp/$IP/$PORT" 2>/dev/null; then
    echo "✅ Port $PORT is open"
else
    echo "❌ Port $PORT is not accessible"
    echo "💡 Check firewall settings and port forwarding"
    exit 1
fi

# Test P2P transfer
echo "📤 Testing P2P transfer..."
if p2p-converter --target "$TARGET" \
                 --file "$TEST_FILE" \
                 --timeout 30 \
                 --verbose; then
    echo "✅ P2P transfer successful"
else
    echo "❌ P2P transfer failed"
    echo "💡 Check peer ID and protocol compatibility"
fi

# Cleanup
rm -f "$TEST_FILE"
```

### Performance Monitoring

**Scenario**: Monitor transfer performance and identify bottlenecks.

```bash
#!/bin/bash
# performance_monitor.sh

TARGET="$1"
TEST_FILES_DIR="$2"
LOG_FILE="performance_$(date +%Y%m%d_%H%M%S).log"

echo "📊 P2P Performance Monitoring" | tee "$LOG_FILE"
echo "=============================" | tee -a "$LOG_FILE"
echo "Target: $TARGET" | tee -a "$LOG_FILE"
echo "Test files: $TEST_FILES_DIR" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# System info
echo "System Information:" | tee -a "$LOG_FILE"
echo "CPU: $(nproc) cores" | tee -a "$LOG_FILE"
echo "Memory: $(free -h | grep Mem | awk '{print $2}')" | tee -a "$LOG_FILE"
echo "Network: $(ip route get 8.8.8.8 | grep -oP 'dev \K\S+')" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Test different file sizes
total_files=0
total_bytes=0
total_time=0

for file in "$TEST_FILES_DIR"/*; do
    if [ -f "$file" ]; then
        file_size=$(stat -c%s "$file")
        file_name=$(basename "$file")

        echo "📤 Testing: $file_name ($(numfmt --to=iec $file_size))" | tee -a "$LOG_FILE"

        start_time=$(date +%s.%N)

        if p2p-converter --target "$TARGET" \
                         --file "$file" \
                         --verbose >> "$LOG_FILE" 2>&1; then
            end_time=$(date +%s.%N)
            duration=$(echo "$end_time - $start_time" | bc)
            throughput=$(echo "scale=2; $file_size / $duration / 1024 / 1024" | bc)

            echo "✅ Success: ${duration}s, ${throughput} MB/s" | tee -a "$LOG_FILE"

            total_files=$((total_files + 1))
            total_bytes=$((total_bytes + file_size))
            total_time=$(echo "$total_time + $duration" | bc)
        else
            echo "❌ Failed: $file_name" | tee -a "$LOG_FILE"
        fi

        echo "" | tee -a "$LOG_FILE"
    fi
done

# Summary statistics
if [ $total_files -gt 0 ]; then
    avg_throughput=$(echo "scale=2; $total_bytes / $total_time / 1024 / 1024" | bc)
    avg_time=$(echo "scale=2; $total_time / $total_files" | bc)

    echo "📊 Performance Summary:" | tee -a "$LOG_FILE"
    echo "Files transferred: $total_files" | tee -a "$LOG_FILE"
    echo "Total data: $(numfmt --to=iec $total_bytes)" | tee -a "$LOG_FILE"
    echo "Total time: ${total_time}s" | tee -a "$LOG_FILE"
    echo "Average throughput: ${avg_throughput} MB/s" | tee -a "$LOG_FILE"
    echo "Average time per file: ${avg_time}s" | tee -a "$LOG_FILE"
fi

echo "End time: $(date)" | tee -a "$LOG_FILE"
echo "📄 Full log saved to: $LOG_FILE"
```

### Error Recovery Testing

**Scenario**: Test error recovery mechanisms under various failure conditions.

```bash
#!/bin/bash
# error_recovery_test.sh

TARGET="$1"
TEST_FILE="recovery_test.txt"

# Create test file
echo "Error recovery test data" > "$TEST_FILE"

echo "🧪 Error Recovery Testing"
echo "========================="

# Test 1: Network interruption simulation
echo "Test 1: Network interruption"
p2p-converter --target "$TARGET" --file "$TEST_FILE" &
TRANSFER_PID=$!

sleep 2
echo "🔌 Simulating network interruption..."
# Block network traffic to target (requires sudo)
# sudo iptables -A OUTPUT -d TARGET_IP -j DROP

sleep 5
echo "🔌 Restoring network..."
# sudo iptables -D OUTPUT -d TARGET_IP -j DROP

wait $TRANSFER_PID
if [ $? -eq 0 ]; then
    echo "✅ Recovered from network interruption"
else
    echo "❌ Failed to recover from network interruption"
fi

# Test 2: High retry scenario
echo "Test 2: High retry scenario"
p2p-converter --target "invalid_peer_address" \
              --file "$TEST_FILE" \
              --max-retries 3 \
              --timeout 5

# Test 3: Large file timeout
echo "Test 3: Timeout handling"
# Create large file
dd if=/dev/zero of=large_test.bin bs=1M count=100 2>/dev/null

p2p-converter --target "$TARGET" \
              --file large_test.bin \
              --timeout 10  # Short timeout to trigger timeout handling

rm -f large_test.bin

# Cleanup
rm -f "$TEST_FILE"

echo "🧪 Error recovery testing completed"
```

These examples cover a wide range of real-world scenarios and provide practical solutions for using the P2P File Converter effectively. Each example includes proper error handling, logging, and best practices for production use.
