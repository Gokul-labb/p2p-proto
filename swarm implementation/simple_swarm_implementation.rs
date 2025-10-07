// Simplified example showing core swarm configuration
use anyhow::Result;
use libp2p::{
    identity::Keypair, noise, swarm::SwarmEvent, tcp, yamux, 
    Multiaddr, PeerId, Swarm, SwarmBuilder
};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,libp2p=debug")
        .init();

    // Step 1: Generate new identity
    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    info!("🔑 Local Peer ID: {}", local_peer_id);

    // Step 2: Build swarm with TCP + Noise + Yamux
    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            Ok(libp2p::ping::Behaviour::new(
                libp2p::ping::Config::default()
            ))
        })?
        .build();

    // Step 3: Listen on all interfaces, port 0 (auto-select)
    let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
    swarm.listen_on(listen_addr)?;

    // Step 4: Event loop - print peer ID and addresses
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("🌐 Listening on: {}/p2p/{}", address, local_peer_id);
                println!("Node ready! Connect with:");
                println!("  {}/p2p/{}", address, local_peer_id);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("✅ Connected to: {}", peer_id);
            }
            SwarmEvent::Behaviour(event) => {
                info!("📡 Ping event: {:?}", event);
            }
            _ => {}
        }
    }
}
