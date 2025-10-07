use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use libp2p::Multiaddr;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{debug, error, info};

/// CLI arguments for P2P file converter
#[derive(Parser, Debug, Clone)]
#[command(
    name = "p2p-converter",
    version = "1.0.0",
    author = "Your Name <your.email@example.com>",
    about = "A peer-to-peer file converter using libp2p",
    long_about = "
P2P File Converter allows you to send and receive files over a peer-to-peer network.

MODES:
  Receiver Mode: Run without arguments to listen for incoming files
  Sender Mode:   Provide --target and --file to send a file to a peer

EXAMPLES:
  Receiver mode:
    p2p-converter
    p2p-converter --listen /ip4/0.0.0.0/tcp/8080

  Sender mode:
    p2p-converter -t /ip4/127.0.0.1/tcp/8080/p2p/12D3K... -f document.pdf
    p2p-converter --target /ip4/192.168.1.100/tcp/9000/p2p/12D3K... --file image.jpg
"
)]
pub struct CliArgs {
    /// Target peer address to send file to (multiaddr format)
    /// 
    /// Example: /ip4/127.0.0.1/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ
    #[arg(
        short = 't',
        long = "target",
        value_name = "MULTIADDR",
        help = "Target peer multiaddress for sending files"
    )]
    pub target_peer: Option<ValidatedMultiaddr>,

    /// Path to the file to send
    #[arg(
        short = 'f',
        long = "file",
        value_name = "FILE_PATH",
        help = "Path to the file to send to the target peer"
    )]
    pub file_path: Option<ValidatedFilePath>,

    /// Address to listen on for incoming connections
    #[arg(
        short = 'l',
        long = "listen",
        value_name = "LISTEN_ADDR",
        default_value = "/ip4/0.0.0.0/tcp/0",
        help = "Multiaddress to listen on for incoming connections"
    )]
    pub listen_address: ValidatedMultiaddr,

    /// Output directory for received files
    #[arg(
        short = 'o',
        long = "output",
        value_name = "OUTPUT_DIR",
        default_value = "./received",
        help = "Directory to save received files"
    )]
    pub output_dir: PathBuf,

    /// Verbose logging
    #[arg(
        short = 'v',
        long = "verbose",
        help = "Enable verbose logging"
    )]
    pub verbose: bool,

    /// Log level
    #[arg(
        long = "log-level",
        value_enum,
        default_value_t = LogLevel::Info,
        help = "Set the logging level"
    )]
    pub log_level: LogLevel,

    /// Maximum file size to accept (in MB)
    #[arg(
        long = "max-size",
        value_name = "SIZE_MB",
        default_value_t = 100,
        help = "Maximum file size to accept in megabytes"
    )]
    pub max_file_size_mb: u64,
}

/// Log level enumeration
#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum LogLevel {
    /// Show error messages only
    Error,
    /// Show warnings and errors
    Warn,
    /// Show info, warnings, and errors (default)
    Info,
    /// Show debug information
    Debug,
    /// Show all log messages including trace
    Trace,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

/// Validated multiaddr wrapper for CLI parsing
#[derive(Debug, Clone)]
pub struct ValidatedMultiaddr(pub Multiaddr);

impl FromStr for ValidatedMultiaddr {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.parse::<Multiaddr>() {
            Ok(addr) => {
                // Validate that the multiaddr has required components
                let protocols: Vec<_> = addr.iter().collect();

                // Check for at least IP and transport protocol
                let has_ip = protocols.iter().any(|p| {
                    matches!(p, 
                        libp2p::multiaddr::Protocol::Ip4(_) | 
                        libp2p::multiaddr::Protocol::Ip6(_) |
                        libp2p::multiaddr::Protocol::Dns(_) |
                        libp2p::multiaddr::Protocol::Dns4(_) |
                        libp2p::multiaddr::Protocol::Dns6(_)
                    )
                });

                let has_transport = protocols.iter().any(|p| {
                    matches!(p, 
                        libp2p::multiaddr::Protocol::Tcp(_) |
                        libp2p::multiaddr::Protocol::Udp(_) |
                        libp2p::multiaddr::Protocol::Quic |
                        libp2p::multiaddr::Protocol::QuicV1
                    )
                });

                if !has_ip {
                    return Err(format!(
                        "Multiaddr must contain an IP address component (ip4, ip6, dns, etc.): {}",
                        s
                    ));
                }

                if !has_transport {
                    return Err(format!(
                        "Multiaddr must contain a transport protocol component (tcp, udp, quic): {}",
                        s
                    ));
                }

                Ok(ValidatedMultiaddr(addr))
            }
            Err(e) => Err(format!("Invalid multiaddr format: {}", e)),
        }
    }
}

impl std::ops::Deref for ValidatedMultiaddr {
    type Target = Multiaddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Validated file path wrapper for CLI parsing
#[derive(Debug, Clone)]
pub struct ValidatedFilePath(pub PathBuf);

impl FromStr for ValidatedFilePath {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let path = PathBuf::from(s);

        // Check if file exists
        if !path.exists() {
            return Err(format!(
                "File does not exist: '{}'\nPlease check the file path and try again.",
                path.display()
            ));
        }

        // Check if it's actually a file (not a directory)
        if !path.is_file() {
            return Err(format!(
                "Path exists but is not a regular file: '{}'\nPlease provide a path to a file, not a directory.",
                path.display()
            ));
        }

        // Check if we can read the file
        match std::fs::metadata(&path) {
            Ok(metadata) => {
                // Check file size (warn if very large)
                const MAX_SIZE_BYTES: u64 = 1024 * 1024 * 1024; // 1GB
                if metadata.len() > MAX_SIZE_BYTES {
                    return Err(format!(
                        "File is too large: {} bytes (max: {} bytes)\nFile: '{}'",
                        metadata.len(),
                        MAX_SIZE_BYTES,
                        path.display()
                    ));
                }
            }
            Err(e) => {
                return Err(format!(
                    "Cannot read file metadata for '{}': {}\nCheck file permissions.",
                    path.display(),
                    e
                ));
            }
        }

        Ok(ValidatedFilePath(path))
    }
}

impl std::ops::Deref for ValidatedFilePath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Application mode determined from CLI arguments
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Listen for incoming files and connections
    Receiver {
        listen_addr: Multiaddr,
        output_dir: PathBuf,
    },
    /// Send a file to a target peer
    Sender {
        target_addr: Multiaddr,
        file_path: PathBuf,
        listen_addr: Multiaddr,
    },
}

impl CliArgs {
    /// Parse CLI arguments and determine application mode
    pub fn parse_args() -> Result<(Self, AppMode)> {
        let args = Self::parse();
        let mode = args.determine_mode()?;
        Ok((args, mode))
    }

    /// Determine application mode from parsed arguments
    pub fn determine_mode(&self) -> Result<AppMode> {
        match (&self.target_peer, &self.file_path) {
            (None, None) => {
                // Receiver mode
                info!("Starting in receiver mode");

                // Ensure output directory exists or can be created
                if !self.output_dir.exists() {
                    std::fs::create_dir_all(&self.output_dir)
                        .with_context(|| {
                            format!("Failed to create output directory: {}", self.output_dir.display())
                        })?;
                    info!("Created output directory: {}", self.output_dir.display());
                }

                Ok(AppMode::Receiver {
                    listen_addr: self.listen_address.0.clone(),
                    output_dir: self.output_dir.clone(),
                })
            }
            (Some(target), Some(file)) => {
                // Sender mode
                info!("Starting in sender mode");
                Ok(AppMode::Sender {
                    target_addr: target.0.clone(),
                    file_path: file.0.clone(),
                    listen_addr: self.listen_address.0.clone(),
                })
            }
            (Some(_), None) => {
                Err(anyhow::anyhow!(
                    "Target peer specified but no file path provided.\n\
                    When sending files, both --target and --file are required.\n\
                    Usage: {} --target <MULTIADDR> --file <FILE_PATH>",
                    env!("CARGO_PKG_NAME")
                ))
            }
            (None, Some(_)) => {
                Err(anyhow::anyhow!(
                    "File path specified but no target peer provided.\n\
                    When sending files, both --target and --file are required.\n\
                    Usage: {} --target <MULTIADDR> --file <FILE_PATH>",
                    env!("CARGO_PKG_NAME")
                ))
            }
        }
    }

    /// Initialize logging based on CLI arguments
    pub fn setup_logging(&self) -> Result<()> {
        let level = if self.verbose {
            "debug"
        } else {
            self.log_level.as_str()
        };

        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| {
                        tracing_subscriber::EnvFilter::new(format!("{}={},libp2p=info", 
                            env!("CARGO_PKG_NAME").replace('-', "_"), level))
                    })
            )
            .with_target(false)
            .with_thread_ids(true)
            .with_level(true)
            .init();

        Ok(())
    }

    /// Validate all arguments and display helpful information
    pub fn validate(&self) -> Result<()> {
        // Check if output directory can be created (for receiver mode)
        if self.target_peer.is_none() {
            if let Some(parent) = self.output_dir.parent() {
                if !parent.exists() {
                    return Err(anyhow::anyhow!(
                        "Parent directory of output path does not exist: {}\n\
                        Please create the parent directory or choose a different output path.",
                        parent.display()
                    ));
                }
            }
        }

        // Validate max file size
        if self.max_file_size_mb == 0 {
            return Err(anyhow::anyhow!(
                "Maximum file size must be greater than 0 MB"
            ));
        }

        if self.max_file_size_mb > 10000 {
            return Err(anyhow::anyhow!(
                "Maximum file size is too large: {} MB (max: 10000 MB)",
                self.max_file_size_mb
            ));
        }

        Ok(())
    }

    /// Print configuration summary
    pub fn print_config(&self, mode: &AppMode) {
        println!("🚀 P2P File Converter Configuration");
        println!("📝 Mode: {}", match mode {
            AppMode::Receiver { .. } => "Receiver (waiting for files)",
            AppMode::Sender { .. } => "Sender (sending file)",
        });

        match mode {
            AppMode::Receiver { listen_addr, output_dir } => {
                println!("🌐 Listen Address: {}", listen_addr);
                println!("📁 Output Directory: {}", output_dir.display());
            }
            AppMode::Sender { target_addr, file_path, listen_addr } => {
                println!("🎯 Target Peer: {}", target_addr);
                println!("📄 File to Send: {}", file_path.display());
                println!("🌐 Listen Address: {}", listen_addr);

                // Show file info
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    println!("📏 File Size: {} bytes", metadata.len());
                }
            }
        }

        println!("📊 Max File Size: {} MB", self.max_file_size_mb);
        println!("🔧 Log Level: {:?}", self.log_level);
        println!();
    }
}

/// Custom validation functions for use with clap value_parser
pub mod validators {
    use super::*;

    /// Validate multiaddr format and structure
    pub fn validate_multiaddr(addr_str: &str) -> std::result::Result<ValidatedMultiaddr, String> {
        ValidatedMultiaddr::from_str(addr_str)
    }

    /// Validate file exists and is readable
    pub fn validate_file_path(path_str: &str) -> std::result::Result<ValidatedFilePath, String> {
        ValidatedFilePath::from_str(path_str)
    }

    /// Validate directory exists or can be created
    pub fn validate_output_dir(path_str: &str) -> std::result::Result<PathBuf, String> {
        let path = PathBuf::from(path_str);

        if path.exists() && !path.is_dir() {
            return Err(format!(
                "Output path exists but is not a directory: '{}'",
                path.display()
            ));
        }

        // Check if parent directory exists (if path doesn't exist)
        if !path.exists() {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    return Err(format!(
                        "Parent directory does not exist: '{}'\n\
                        Cannot create output directory.",
                        parent.display()
                    ));
                }
            }
        }

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_multiaddr() {
        let addr = ValidatedMultiaddr::from_str("/ip4/127.0.0.1/tcp/8080");
        assert!(addr.is_ok());
    }

    #[test]
    fn test_invalid_multiaddr_no_transport() {
        let addr = ValidatedMultiaddr::from_str("/ip4/127.0.0.1");
        assert!(addr.is_err());
        assert!(addr.unwrap_err().contains("transport protocol"));
    }

    #[test]
    fn test_invalid_multiaddr_no_ip() {
        let addr = ValidatedMultiaddr::from_str("/tcp/8080");
        assert!(addr.is_err());
        assert!(addr.unwrap_err().contains("IP address"));
    }

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(LogLevel::Debug.as_str(), "debug");
        assert_eq!(LogLevel::Info.as_str(), "info");
    }

    #[test]
    fn test_app_mode_receiver() {
        let args = CliArgs {
            target_peer: None,
            file_path: None,
            listen_address: ValidatedMultiaddr::from_str("/ip4/0.0.0.0/tcp/0").unwrap(),
            output_dir: PathBuf::from("./test_output"),
            verbose: false,
            log_level: LogLevel::Info,
            max_file_size_mb: 100,
        };

        // Create test directory
        std::fs::create_dir_all("./test_output").unwrap();

        let mode = args.determine_mode().unwrap();
        assert!(matches!(mode, AppMode::Receiver { .. }));

        // Clean up
        std::fs::remove_dir_all("./test_output").ok();
    }
}

/// Example usage function
pub fn print_usage_examples() {
    println!("📖 Usage Examples:");
    println!();
    println!("1. Start in receiver mode (default):");
    println!("   p2p-converter");
    println!("   p2p-converter --listen /ip4/0.0.0.0/tcp/8080");
    println!();
    println!("2. Send a file to a peer:");
    println!("   p2p-converter \\");
    println!("     --target /ip4/192.168.1.100/tcp/8080/p2p/12D3KooW... \\");
    println!("     --file document.pdf");
    println!();
    println!("3. With custom settings:");
    println!("   p2p-converter \\");
    println!("     --target /ip4/example.com/tcp/9000/p2p/12D3KooW... \\");
    println!("     --file large_video.mp4 \\");
    println!("     --max-size 500 \\");
    println!("     --verbose");
    println!();
    println!("4. Custom output directory:");
    println!("   p2p-converter --output /home/user/Downloads");
    println!();
}

fn main() -> Result<()> {
    // Parse command line arguments
    let (args, mode) = CliArgs::parse_args()?;

    // Validate arguments
    args.validate()?;

    // Setup logging
    args.setup_logging()?;

    // Print configuration
    args.print_config(&mode);

    // Log the determined mode
    match &mode {
        AppMode::Receiver { listen_addr, output_dir } => {
            info!("Starting receiver mode on {}", listen_addr);
            info!("Output directory: {}", output_dir.display());
        }
        AppMode::Sender { target_addr, file_path, .. } => {
            info!("Starting sender mode");
            info!("Target: {}", target_addr);
            info!("File: {}", file_path.display());
        }
    }

    // TODO: Initialize P2P swarm and start appropriate mode
    println!("✅ CLI parsing complete. Ready to start P2P operations.");
    println!("Press Ctrl+C to exit.");

    // Simulate running
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Shutting down...");

    Ok(())
}
