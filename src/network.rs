use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, SwarmBuilder, identify, identity, kad, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use std::error::Error;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

#[derive(Debug)]
pub enum NetworkCommand {
    Listen(Multiaddr),
    Dial(Multiaddr),
}

#[derive(Debug)]
pub enum NetworkEvent {
    Listening(Multiaddr),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

pub async fn start_network(
    mut cmd_receiver: mpsc::Receiver<NetworkCommand>,
    event_sender: mpsc::Sender<NetworkEvent>,
) -> Result<PeerId, Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    // Setup swarm
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
            let store = kad::store::MemoryStore::new(local_peer_id);
            let kademlia = kad::Behaviour::new(local_peer_id, store);

            AppBehaviour { identify, kademlia }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = swarm.select_next_some() => match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let _ = event_sender.send(NetworkEvent::Listening(address)).await;
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        let _ = event_sender.send(NetworkEvent::PeerConnected(peer_id)).await;
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        let _ = event_sender.send(NetworkEvent::PeerDisconnected(peer_id)).await;
                    }
                    _ => {}
                },
                cmd = cmd_receiver.recv() => {
                    if let Some(command) = cmd {
                        match command {
                            NetworkCommand::Listen(addr) => {
                                let _ = swarm.listen_on(addr);
                            }
                            NetworkCommand::Dial(addr) => {
                                let _ = swarm.dial(addr);
                            }
                        }
                    } else {
                        // Channel closed by UI
                        break;
                    }
                }
            }
        }
    });

    Ok(local_peer_id)
}
