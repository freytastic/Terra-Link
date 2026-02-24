use futures::StreamExt;
use libp2p::{
    identify, identity, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, SwarmBuilder,
};
use std::error::Error;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(NetworkBehaviour)]
struct RelayBehaviour {
    relay: relay::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    println!("Starting Terra-Link Dedicated Relay Server...");

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id();

    println!("Local Peer ID: {}", local_peer_id);

    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key| {
            let identify = identify::Behaviour::new(identify::Config::new(
                "/terra-link/0.1.0".into(),
                key.public(),
            ));

            let relay_config = relay::Config {
                max_reservations: 128,        // Allow many concurrent clients to hold a slot
                max_reservations_per_peer: 4, // Prevent one client from hoarding slots
                reservation_duration: Duration::from_secs(60 * 60), // 1 Hour
                reservation_rate_limiters: vec![],
                max_circuits: 16, // Active data-transfer circuits simultaneously
                max_circuits_per_peer: 4,
                max_circuit_duration: Duration::from_secs(60 * 2), // Keep open for 2m max for hole punching
                max_circuit_bytes: 1024 * 1024 * 10,               // 10 MB limit through the relay
                circuit_src_rate_limiters: vec![],
            };

            RelayBehaviour {
                relay: relay::Behaviour::new(local_peer_id, relay_config),
                ping: ping::Behaviour::default(),
                identify,
            }
        })?
        // Note: Ping determines if the connection is dead. We do not want an arbitrary idle timeout closing active relayed tunnels.
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60 * 60)))
        .build();

    // Listen on all interfaces on port 4001
    let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/4001".parse()?;
    swarm.listen_on(listen_addr.clone())?;

    // Also listen on QUIC
    let quic_addr: Multiaddr = "/ip4/0.0.0.0/udp/4002/quic-v1".parse()?;
    swarm.listen_on(quic_addr)?;

    println!("Relay listening for UDP (QUIC) and TCP connections on port 4001...");
    println!(
        "Run this exactly on ur relay node server, and place the Peer ID in your local .env file."
    );
    println!(
        "RELAY_NODE=\"/ip4/46.62.175.35/tcp/4001/p2p/{}\"",
        local_peer_id
    );

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Listening on {:?}", address);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Relay(event)) => {
                println!("Relay circuit event: {:?}", event);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Ping(event)) => {
                // Commenting out to avoid log spam, but ping keeps the connection alive
                // println!("Ping event: {:?}", event);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("Connected to {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("Disconnected from {} (cause: {:?})", peer_id, cause);
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                println!("Incoming connection error: {:?}", error);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                println!("Outgoing connection error to {:?}: {:?}", peer_id, error);
            }
            event => {
                // Print all other untracked events for deep debugging
                println!("Unhandled SwarmEvent: {:?}", event);
            }
        }
    }
}
