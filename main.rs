use anyhow::Result;
use clap::{Arg, Command};
use futures::prelude::*;
use libp2p::{
    identify, mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use std::error::Error;
use tokio::io::{self, AsyncBufReadExt};
use tracing::{debug, error, info, warn};

/// Network behaviour for our P2P file converter
#[derive(NetworkBehaviour)]
struct P2PBehaviour {
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
    ping: libp2p::ping::Behaviour,
}

/// Configuration for the P2P file converter
#[derive(Debug)]
struct Config {
    listen_addr: Multiaddr,
    peer_id: PeerId,
}

impl Default for Config {
    fn default() -> Self {
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
            peer_id: local_peer_id,
        }
    }
}

/// Main application structure
struct P2PFileConverter {
    swarm: Swarm<P2PBehaviour>,
    config: Config,
}

impl P2PFileConverter {
    /// Initialize a new P2P file converter instance
    async fn new(config: Config) -> Result<Self> {
        info!("Initializing P2P File Converter with peer ID: {}", config.peer_id);

        // Generate a keypair for authentication
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // Set up the transport
        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        // Create network behaviour
        let behaviour = P2PBehaviour {
            identify: identify::Behaviour::new(identify::Config::new(
                "/p2p-file-converter/1.0.0".to_string(),
                local_key.public(),
            )),
            mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?,
            ping: libp2p::ping::Behaviour::new(libp2p::ping::Config::new()),
        };

        // Create swarm
        let swarm = Swarm::with_tokio_executor(transport, behaviour, local_peer_id);

        Ok(Self { swarm, config })
    }

    /// Start listening for connections
    async fn start_listening(&mut self) -> Result<()> {
        self.swarm.listen_on(self.config.listen_addr.clone())?;
        info!("Started listening on {}", self.config.listen_addr);
        Ok(())
    }

    /// Main event loop for handling P2P events
    async fn run(&mut self) -> Result<()> {
        info!("Starting P2P File Converter event loop");

        let mut stdin = io::BufReader::new(io::stdin()).lines();

        loop {
            tokio::select! {
                line = stdin.next_line() => {
                    if let Ok(Some(line)) = line {
                        self.handle_user_input(line.trim()).await?;
                    }
                }
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
            }
        }
    }

    /// Handle user input from CLI
    async fn handle_user_input(&mut self, input: &str) -> Result<()> {
        match input {
            "peers" => {
                info!("Connected peers:");
                for peer in self.swarm.connected_peers() {
                    info!("  {}", peer);
                }
            }
            "quit" | "exit" => {
                info!("Shutting down...");
                return Err(anyhow::anyhow!("User requested shutdown"));
            }
            _ if input.starts_with("connect ") => {
                let addr = input.trim_start_matches("connect ");
                match addr.parse::<Multiaddr>() {
                    Ok(multiaddr) => {
                        if let Err(e) = self.swarm.dial(multiaddr.clone()) {
                            error!("Failed to connect to {}: {}", multiaddr, e);
                        } else {
                            info!("Attempting to connect to {}", multiaddr);
                        }
                    }
                    Err(e) => {
                        error!("Invalid multiaddress '{}': {}", addr, e);
                    }
                }
            }
            _ => {
                warn!("Unknown command: {}", input);
                info!("Available commands: peers, connect <multiaddr>, quit/exit");
            }
        }
        Ok(())
    }

    /// Handle swarm events
    async fn handle_swarm_event(&mut self, event: SwarmEvent<P2PBehaviourEvent>) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connected to peer: {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Connection to peer {} closed: {:?}", peer_id, cause);
            }
            SwarmEvent::Behaviour(P2PBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    debug!("Discovered peer {} at {}", peer_id, multiaddr);
                    if let Err(e) = self.swarm.dial(multiaddr.clone()) {
                        debug!("Failed to dial discovered peer {}: {}", peer_id, e);
                    }
                }
            }
            SwarmEvent::Behaviour(P2PBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _multiaddr) in list {
                    debug!("Peer {} expired from mDNS", peer_id);
                }
            }
            SwarmEvent::Behaviour(P2PBehaviourEvent::Identify(identify::Event::Received { 
                peer_id, 
                info 
            })) => {
                debug!(
                    "Received identify info from {}: protocol_version={}, agent_version={}",
                    peer_id, info.protocol_version, info.agent_version
                );
            }
            SwarmEvent::Behaviour(P2PBehaviourEvent::Ping(ping_event)) => {
                debug!("Ping event: {:?}", ping_event);
            }
            _ => {
                debug!("Unhandled swarm event: {:?}", event);
            }
        }
        Ok(())
    }
}

/// File conversion utilities
mod file_converter {
    use super::*;
    use pdf_extract::extract_text;
    use std::path::Path;

    /// Convert file to different formats
    pub async fn convert_file(input_path: &Path, output_format: &str) -> Result<Vec<u8>> {
        match output_format.to_lowercase().as_str() {
            "txt" => convert_to_text(input_path).await,
            "pdf" => convert_to_pdf(input_path).await,
            _ => Err(anyhow::anyhow!("Unsupported output format: {}", output_format)),
        }
    }

    /// Extract text from PDF and return as bytes
    async fn convert_to_text(input_path: &Path) -> Result<Vec<u8>> {
        let bytes = tokio::fs::read(input_path).await?;
        let text = extract_text(&bytes)?;
        Ok(text.into_bytes())
    }

    /// Convert text file to PDF
    async fn convert_to_pdf(input_path: &Path) -> Result<Vec<u8>> {
        let text = tokio::fs::read_to_string(input_path).await?;

        // Create a new PDF document
        let mut doc = genpdf::Document::new(genpdf::fonts::from_files("./fonts", "LiberationSans", None)?);

        // Set document metadata
        doc.set_title("Converted Document");
        doc.set_minimal_conformance();
        doc.set_line_spacing(1.25);

        // Add content
        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(10);
        doc.set_page_decorator(decorator);

        doc.push(genpdf::elements::Paragraph::new(text));

        // Render to bytes
        let mut buf = Vec::new();
        doc.render(&mut buf)?;

        Ok(buf)
    }
}

/// Set up logging and tracing
fn setup_logging() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    Ok(())
}

/// Parse command line arguments
fn parse_args() -> Command {
    Command::new("p2p-file-converter")
        .about("A peer-to-peer file converter using libp2p")
        .version("0.1.0")
        .author("Your Name")
        .arg(
            Arg::new("listen-addr")
                .long("listen")
                .value_name("MULTIADDR")
                .help("Address to listen on")
                .default_value("/ip4/0.0.0.0/tcp/0")
        )
        .arg(
            Arg::new("peer")
                .long("peer")
                .value_name("MULTIADDR")
                .help("Address of a peer to connect to")
        )
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging
    setup_logging()?;

    info!("Starting P2P File Converter");

    // Parse command line arguments
    let matches = parse_args().get_matches();

    // Create configuration
    let mut config = Config::default();

    if let Some(listen_addr) = matches.get_one::<String>("listen-addr") {
        config.listen_addr = listen_addr.parse()
            .map_err(|e| anyhow::anyhow!("Invalid listen address: {}", e))?;
    }

    // Initialize P2P file converter
    let mut converter = P2PFileConverter::new(config).await?;

    // Start listening
    converter.start_listening().await?;

    // Connect to peer if specified
    if let Some(peer_addr) = matches.get_one::<String>("peer") {
        let multiaddr: Multiaddr = peer_addr.parse()
            .map_err(|e| anyhow::anyhow!("Invalid peer address: {}", e))?;

        info!("Connecting to peer: {}", multiaddr);
        converter.swarm.dial(multiaddr)?;
    }

    info!("P2P File Converter started successfully!");
    info!("Commands: peers, connect <multiaddr>, quit/exit");

    // Run the main event loop
    if let Err(e) = converter.run().await {
        if e.to_string().contains("User requested shutdown") {
            info!("Shutting down gracefully");
            return Ok(());
        }
        return Err(e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.listen_addr.to_string(), "/ip4/0.0.0.0/tcp/0");
    }

    #[test]
    fn test_parse_args() {
        let cmd = parse_args();
        assert_eq!(cmd.get_name(), "p2p-file-converter");
    }
}
