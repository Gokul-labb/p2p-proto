//! Interactive client example
//! 
//! Demonstrates an interactive P2P file transfer client.

use anyhow::Result;
use p2p_file_converter::prelude::*;
use std::collections::HashMap;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

struct InteractiveClient {
    sender: FileSender,
    connected_peers: HashMap<String, (PeerId, Multiaddr)>,
    active_transfers: HashMap<String, String>, // transfer_id -> description
}

impl InteractiveClient {
    async fn new() -> Result<Self> {
        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay: std::time::Duration::from_millis(200),
            max_delay: std::time::Duration::from_secs(10),
            backoff_multiplier: 1.5,
            connection_timeout: std::time::Duration::from_secs(10),
        };

        let mut sender = FileSender::new(Some(retry_config)).await?;

        // Set up progress tracking
        sender.set_progress_callback(|progress| {
            match &progress.status {
                TransferStatus::Connecting => {
                    print!("\r🔄 Connecting... (attempt {})    ", progress.connection_attempts);
                }
                TransferStatus::Sending => {
                    if progress.chunks_sent % 3 == 0 {
                        print!("\r📤 {:.1}% ({:.1} KB/s)     ", 
                               progress.percentage(), progress.speed_bps() / 1024.0);
                    }
                }
                TransferStatus::Completed => {
                    println!("\r✅ Transfer completed!                ");
                }
                TransferStatus::Failed(e) => {
                    println!("\r❌ Transfer failed: {}     ", e);
                }
                _ => {}
            }
            io::stdout().flush().unwrap();
        });

        Ok(Self {
            sender,
            connected_peers: HashMap::new(),
            active_transfers: HashMap::new(),
        })
    }

    async fn run(&mut self) -> Result<()> {
        println!("🚀 Interactive P2P File Transfer Client");
        println!("Type 'help' for commands or 'quit' to exit");

        // Start sender event loop
        let sender_task = tokio::spawn(async move {
            // Note: In a real implementation, we'd need to handle this properly
            println!("Sender event loop would run here");
        });

        let mut reader = BufReader::new(tokio::io::stdin());
        let mut line = String::new();

        loop {
            print!("p2p> ");
            io::stdout().flush()?;

            line.clear();
            if reader.read_line(&mut line).await? == 0 {
                break; // EOF
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Err(e) = self.handle_command(trimmed).await {
                println!("❌ Error: {}", e);
            }

            if trimmed == "quit" || trimmed == "exit" {
                break;
            }
        }

        sender_task.abort();
        println!("👋 Goodbye!");
        Ok(())
    }

    async fn handle_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "help" => {
                self.show_help();
            }
            "connect" => {
                if parts.len() < 3 {
                    println!("Usage: connect <name> <multiaddr>");
                    return Ok(());
                }
                self.connect_peer(parts[1], parts[2]).await?;
            }
            "peers" => {
                self.list_peers();
            }
            "send" => {
                if parts.len() < 3 {
                    println!("Usage: send <peer_name> <file_path> [format]");
                    return Ok(());
                }
                let format = parts.get(3).map(|s| s.to_string());
                self.send_file(parts[1], parts[2], format).await?;
            }
            "status" => {
                self.show_status().await;
            }
            "transfers" => {
                self.list_transfers().await;
            }
            "quit" | "exit" => {
                // Handled in main loop
            }
            _ => {
                println!("Unknown command: '{}'. Type 'help' for available commands.", parts[0]);
            }
        }

        Ok(())
    }

    fn show_help(&self) {
        println!("📋 Available commands:");
        println!("  help                           - Show this help");
        println!("  connect <name> <multiaddr>     - Connect to a peer");
        println!("  peers                          - List connected peers");
        println!("  send <peer> <file> [format]    - Send file to peer");
        println!("  status                         - Show application status");
        println!("  transfers                      - List active transfers");
        println!("  quit                           - Exit the client");
        println!();
        println!("Examples:");
        println!("  connect alice /ip4/192.168.1.100/tcp/8080/p2p/12D3K...");
        println!("  send alice document.txt pdf");
    }

    async fn connect_peer(&mut self, name: &str, addr_str: &str) -> Result<()> {
        let addr: Multiaddr = addr_str.parse()
            .context("Invalid multiaddress format")?;

        // Extract peer ID
        let peer_id = self.extract_peer_id(&addr)?;

        println!("🔄 Connecting to {} at {}", name, addr);

        // Store peer info
        self.connected_peers.insert(name.to_string(), (peer_id, addr.clone()));

        println!("✅ Peer '{}' added ({})", name, peer_id);
        Ok(())
    }

    fn list_peers(&self) {
        if self.connected_peers.is_empty() {
            println!("🌐 No peers configured");
        } else {
            println!("🌐 Configured peers:");
            for (name, (peer_id, addr)) in &self.connected_peers {
                println!("  {} -> {} ({})", name, peer_id, addr);
            }
        }
    }

    async fn send_file(&mut self, peer_name: &str, file_path: &str, format: Option<String>) -> Result<()> {
        let (peer_id, addr) = self.connected_peers.get(peer_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown peer: '{}'", peer_name))?
            .clone();

        println!("📤 Sending {} to {} ({})", file_path, peer_name, peer_id);
        if let Some(ref fmt) = format {
            println!("🔄 Requesting conversion to: {}", fmt);
        }

        let transfer_id = self.sender.send_file(
            peer_id,
            addr,
            file_path,
            format,
            false,
        ).await?;

        let description = format!("{} -> {}", file_path, peer_name);
        self.active_transfers.insert(transfer_id.clone(), description);

        println!("🚀 Transfer started: {}", transfer_id[..8].to_string());
        Ok(())
    }

    async fn show_status(&self) {
        println!("📊 Client Status:");
        println!("  Connected peers: {}", self.connected_peers.len());
        println!("  Active transfers: {}", self.active_transfers.len());

        let all_progress = self.sender.get_all_progress().await;
        let active_count = all_progress.iter()
            .filter(|p| !matches!(p.status, TransferStatus::Completed | TransferStatus::Failed(_)))
            .count();

        println!("  Transfers in progress: {}", active_count);
    }

    async fn list_transfers(&self) {
        let all_progress = self.sender.get_all_progress().await;

        if all_progress.is_empty() {
            println!("📊 No transfers");
            return;
        }

        println!("📊 Transfers:");
        for progress in &all_progress {
            let short_id = &progress.transfer_id[..8];
            let description = self.active_transfers.get(&progress.transfer_id)
                .map(|s| s.as_str())
                .unwrap_or("Unknown");

            println!("  {} - {} ({:.1}%) - {}", 
                     short_id, description, progress.percentage(), progress.status_string());
        }
    }

    fn extract_peer_id(&self, addr: &Multiaddr) -> Result<PeerId> {
        use libp2p::multiaddr::Protocol;

        for protocol in addr.iter() {
            if let Protocol::P2p(peer_id) = protocol {
                return Ok(peer_id);
            }
        }

        Err(anyhow::anyhow!("No peer ID found in multiaddr"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let mut client = InteractiveClient::new().await?;
    client.run().await
}
