[package]
name = "p2p-file-sender"
version = "1.0.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
description = "P2P file sender with retry logic, progress tracking, and comprehensive error handling"
license = "MIT"
repository = "https://github.com/username/p2p-file-sender"
readme = "README.md"
keywords = ["p2p", "libp2p", "file-transfer", "networking", "async"]
categories = ["network-programming", "asynchronous"]

[features]
default = ["progress-bars"]
progress-bars = ["indicatif"]
benchmarks = []

[dependencies]
# Core libp2p
libp2p = { version = "0.56", features = [
    "tcp", 
    "noise", 
    "yamux", 
    "swarm", 
    "request-response",
    "macros"
] }

# Async runtime and futures
tokio = { version = "1.0", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
uuid = { version = "1.0", features = ["v4"] }

# Error handling and logging
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI support
clap = { version = "4.5", features = ["derive"] }

# File operations
tokio-util = { version = "0.7", features = ["codec"] }

# Progress bars (optional)
indicatif = { version = "0.17", optional = true }

# File conversion (from previous modules)
genpdf = "0.2"
pdf-extract = "0.7"

[dev-dependencies]
tempfile = "3.0"
tokio-test = "0.4"
criterion = { version = "0.5", features = ["html_reports"] }

[[bin]]
name = "p2p-send"
path = "cli_sender_tool.rs"

[[example]]
name = "simple_send"
path = "examples/simple_send.rs"

[[example]]
name = "batch_send"
path = "examples/batch_send.rs"

[[example]]
name = "resilient_send"
path = "examples/resilient_send.rs"

[[example]]
name = "progress_monitoring"
path = "examples/progress_monitoring.rs"

[[bench]]
name = "sender_benchmarks"
path = "benches/sender_benchmarks.rs"
harness = false

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
