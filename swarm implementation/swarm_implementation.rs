use anyhow::{Context, Result};
use futures::prelude::*;
use libp2p::{
    core::upgrade,
    identity::Keypair,
    noise, swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder, Transport,
};
use std::error::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

// Import stream protocol and related types
use libp2p::{
    swarm::{
        handler::ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr, KeepAlive,
        NegotiatedSubstream, SubstreamProtocol,
    },
    swarm::{NetworkBehaviour, StreamProtocol},
    InboundUpgrade, OutboundUpgrade,
};

/// Custom protocol identifier for file conversion
const CONVERT_PROTOCOL: StreamProtocol = StreamProtocol::new("/convert/1.0.0");

/// Custom protocol implementation for file conversion
#[derive(Clone)]
pub struct ConvertProtocol;

impl InboundUpgrade<NegotiatedSubstream> for ConvertProtocol {
    type Output = NegotiatedSubstream;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, stream: NegotiatedSubstream, _: Self::Info) -> Self::Future {
        future::ready(Ok(stream))
    }
}

impl OutboundUpgrade<NegotiatedSubstream> for ConvertProtocol {
    type Output = NegotiatedSubstream;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, stream: NegotiatedSubstream, _: Self::Info) -> Self::Future {
        future::ready(Ok(stream))
    }
}

impl libp2p::core::upgrade::UpgradeInfo for ConvertProtocol {
    type Info = StreamProtocol;
    type InfoIter = std::iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once(CONVERT_PROTOCOL)
    }
}

/// Connection handler for the convert protocol
pub struct ConvertHandler {
    keep_alive: KeepAlive,
    inbound_streams: Vec<NegotiatedSubstream>,
    outbound_streams: Vec<NegotiatedSubstream>,
}

impl ConvertHandler {
    pub fn new() -> Self {
        Self {
            keep_alive: KeepAlive::Yes,
            inbound_streams: Vec::new(),
            outbound_streams: Vec::new(),
        }
    }

    async fn handle_inbound_stream(&mut self, mut stream: NegotiatedSubstream) -> Result<()> {
        info!("Handling new inbound stream on convert protocol");

        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer).await?;

        let request = String::from_utf8(buffer)?;
        info!("Received conversion request: {}", request);

        // Simple echo response for demonstration
        let response = format!("Processed: {}", request);
        stream.write_all(response.as_bytes()).await?;
        stream.close().await?;

        Ok(())
    }
}

impl ConnectionHandler for ConvertHandler {
    type InEvent = ();
    type OutEvent = ();
    type Error = std::io::Error;
    type InboundProtocol = ConvertProtocol;
    type OutboundProtocol = ConvertProtocol;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(ConvertProtocol, ())
    }

    fn on_behaviour_event(&mut self, _event: Self::InEvent) {}

    fn connection_keep_alive(&self) -> KeepAlive {
        self.keep_alive
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::OutEvent,
            Self::Error,
        >,
    > {
        // Handle any pending inbound streams
        while let Some(stream) = self.inbound_streams.pop() {
            let mut handler = self.clone();
            tokio::spawn(async move {
                if let Err(e) = handler.handle_inbound_stream(stream).await {
                    error!("Error handling inbound stream: {}", e);
                }
            });
        }

        std::task::Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: libp2p::swarm::handler::ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        use libp2p::swarm::handler::ConnectionEvent;

        match event {
            ConnectionEvent::FullyNegotiatedInbound(fully_negotiated_inbound) => {
                info!("Inbound stream fully negotiated for convert protocol");
                self.inbound_streams.push(fully_negotiated_inbound.protocol);
            }
            ConnectionEvent::FullyNegotiatedOutbound(fully_negotiated_outbound) => {
                info!("Outbound stream fully negotiated for convert protocol");
                self.outbound_streams.push(fully_negotiated_outbound.protocol);
            }
            ConnectionEvent::DialUpgradeError(dial_upgrade_error) => {
                error!("Dial upgrade error: {:?}", dial_upgrade_error.error);
            }
            ConnectionEvent::ListenUpgradeError(listen_upgrade_error) => {
                error!("Listen upgrade error: {:?}", listen_upgrade_error.error);
            }
            ConnectionEvent::AddressChange(_) => {
                debug!("Address change event received");
            }
        }
    }
}

impl Clone for ConvertHandler {
    fn clone(&self) -> Self {
        Self {
            keep_alive: self.keep_alive,
            inbound_streams: Vec::new(),
            outbound_streams: Vec::new(),
        }
    }
}

/// Network behavior for our P2P file converter with custom protocol
#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    convert: libp2p::swarm::dummy::Behaviour,
    identify: libp2p::identify::Behaviour,
    ping: libp2p::ping::Behaviour,
}

/// Configuration for the P2P swarm
#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub listen_addr: Multiaddr,
    pub enable_mdns: bool,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
            enable_mdns: true,
        }
    }
}

/// P2P Swarm manager with custom protocol support
pub struct P2PSwarmManager {
    swarm: Swarm<P2PBehaviour>,
    local_peer_id: PeerId,
    config: SwarmConfig,
}

impl P2PSwarmManager {
    /// Create a new P2P swarm with custom protocol handler
    pub async fn new(config: SwarmConfig) -> Result<Self> {
        info!("Creating P2P swarm with custom convert protocol");

        // Generate a new identity keypair
        let local_key = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        info!("Generated peer ID: {}", local_peer_id);

        // Build the swarm using SwarmBuilder
        let swarm = SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().port_reuse(true).nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .context("Failed to configure TCP transport with Noise and Yamux")?
            .with_behaviour(|key| {
                let peer_id = key.public().to_peer_id();

                Ok(P2PBehaviour {
                    convert: libp2p::swarm::dummy::Behaviour,
                    identify: libp2p::identify::Behaviour::new(
                        libp2p::identify::Config::new("/convert-p2p/1.0.0".to_string(), key.public())
                            .with_agent_version("rust-p2p-converter/0.1.0".to_string()),
                    ),
                    ping: libp2p::ping::Behaviour::new(
                        libp2p::ping::Config::new()
                            .with_interval(std::time::Duration::from_secs(30))
                            .with_timeout(std::time::Duration::from_secs(10)),
                    ),
                })
            })
            .context("Failed to configure swarm behaviour")?
            .build();

        Ok(Self {
            swarm,
            local_peer_id,
            config,
        })
    }

    /// Start listening on the configured address
    pub async fn start_listening(&mut self) -> Result<Vec<Multiaddr>> {
        info!("Starting to listen on: {}", self.config.listen_addr);

        self.swarm
            .listen_on(self.config.listen_addr.clone())
            .context("Failed to start listening")?;

        // Wait for the first NewListenAddr event to get the actual addresses
        let mut listening_addresses = Vec::new();

        while let Some(event) = self.swarm.next().await {
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Now listening on: {}", address);
                    listening_addresses.push(address);

                    // Print peer information
                    self.print_peer_info(&listening_addresses);
                    break;
                }
                SwarmEvent::ListenerError { error, .. } => {
                    error!("Listener error: {}", error);
                    return Err(anyhow::anyhow!("Listener failed to start: {}", error));
                }
                _ => {
                    debug!("Received other swarm event while starting: {:?}", event);
                }
            }
        }

        Ok(listening_addresses)
    }

    /// Print peer ID and listening addresses
    fn print_peer_info(&self, addresses: &[Multiaddr]) {
        println!("🚀 P2P File Converter Node Started!");
        println!("📋 Peer ID: {}", self.local_peer_id);
        println!("🌐 Listening addresses:");

        for addr in addresses {
            println!("   - {}/p2p/{}", addr, self.local_peer_id);
        }

        println!("🔧 Supported protocols:");
        println!("   - /convert/1.0.0 (File conversion protocol)");
        println!("   - /ipfs/id/1.0.0 (Identity protocol)");
        println!("   - /ipfs/ping/1.0.0 (Ping protocol)");
        println!();
    }

    /// Dial a peer by their multiaddress
    pub async fn dial(&mut self, addr: Multiaddr) -> Result<()> {
        info!("Dialing peer at: {}", addr);

        self.swarm
            .dial(addr.clone())
            .context(format!("Failed to dial peer at {}", addr))?;

        Ok(())
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    /// Get connected peers
    pub fn connected_peers(&self) -> impl Iterator<Item = &PeerId> {
        self.swarm.connected_peers()
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting P2P swarm event loop");

        let mut stdin = BufReader::new(tokio::io::stdin()).lines();

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

    /// Handle user input commands
    async fn handle_user_input(&mut self, input: &str) -> Result<()> {
        match input {
            "help" => {
                println!("Available commands:");
                println!("  help        - Show this help message");
                println!("  peers       - List connected peers");
                println!("  listen      - Show listening addresses");
                println!("  dial <addr> - Connect to a peer");
                println!("  quit        - Exit the application");
            }
            "peers" => {
                let peers: Vec<_> = self.connected_peers().collect();
                if peers.is_empty() {
                    println!("No connected peers");
                } else {
                    println!("Connected peers:");
                    for peer in peers {
                        println!("  {}", peer);
                    }
                }
            }
            "listen" => {
                println!("Listening on:");
                for addr in self.swarm.listeners() {
                    println!("  {}/p2p/{}", addr, self.local_peer_id);
                }
            }
            "quit" | "exit" => {
                info!("Shutting down...");
                return Err(anyhow::anyhow!("User requested shutdown"));
            }
            _ if input.starts_with("dial ") => {
                let addr_str = input.trim_start_matches("dial ");
                match addr_str.parse::<Multiaddr>() {
                    Ok(addr) => {
                        if let Err(e) = self.dial(addr.clone()).await {
                            error!("Failed to dial {}: {}", addr, e);
                        } else {
                            info!("Attempting to connect to {}", addr);
                        }
                    }
                    Err(e) => {
                        error!("Invalid multiaddress '{}': {}", addr_str, e);
                    }
                }
            }
            _ => {
                warn!("Unknown command: '{}'. Type 'help' for available commands.", input);
            }
        }
        Ok(())
    }

    /// Handle swarm events
    async fn handle_swarm_event(&mut self, event: SwarmEvent<P2PBehaviourEvent>) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("New listen address: {}", address);
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!(
                    "Connection established with peer {} via {}",
                    peer_id, endpoint.get_remote_address()
                );
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                cause,
                ..
            } => {
                info!(
                    "Connection closed with peer {} via {} (cause: {:?})",
                    peer_id,
                    endpoint.get_remote_address(),
                    cause
                );
            }
            SwarmEvent::IncomingConnection { connection_id, .. } => {
                debug!("Incoming connection: {:?}", connection_id);
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {}", error);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("Outgoing connection error to {:?}: {}", peer_id, error);
            }
            SwarmEvent::Behaviour(P2PBehaviourEvent::Identify(event)) => {
                debug!("Identify event: {:?}", event);
            }
            SwarmEvent::Behaviour(P2PBehaviourEvent::Ping(event)) => {
                debug!("Ping event: {:?}", event);
            }
            _ => {
                debug!("Unhandled swarm event: {:?}", event);
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,libp2p=debug")),
        )
        .init();

    info!("Starting P2P File Converter with custom protocol support");

    // Create swarm configuration
    let config = SwarmConfig {
        listen_addr: "/ip4/0.0.0.0/tcp/0".parse()?,
        enable_mdns: true,
    };

    // Create and configure the swarm
    let mut swarm_manager = P2PSwarmManager::new(config).await?;

    // Start listening
    let _listening_addresses = swarm_manager.start_listening().await?;

    println!("Type 'help' for available commands or 'quit' to exit.");

    // Run the event loop
    if let Err(e) = swarm_manager.run().await {
        if e.to_string().contains("User requested shutdown") {
            info!("Graceful shutdown completed");
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
    async fn test_swarm_creation() {
        let config = SwarmConfig::default();
        let result = P2PSwarmManager::new(config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_protocol() {
        let protocol = ConvertProtocol;
        let protocols: Vec<_> = protocol.protocol_info().collect();
        assert_eq!(protocols.len(), 1);
        assert_eq!(protocols[0], CONVERT_PROTOCOL);
    }

    #[test]
    fn test_swarm_config_default() {
        let config = SwarmConfig::default();
        assert_eq!(config.listen_addr.to_string(), "/ip4/0.0.0.0/tcp/0");
        assert!(config.enable_mdns);
    }
}
