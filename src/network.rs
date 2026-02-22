use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, SwarmBuilder, gossipsub, identify, identity, kad, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

#[derive(Debug)]
pub enum NetworkCommand {
    Listen(Multiaddr),
    Dial(Multiaddr),
    PublishMessage(String),
}

#[derive(Debug)]
pub enum NetworkEvent {
    Listening(Multiaddr),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    MessageReceived { sender: PeerId, text: String },
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
            // Setup Gossipsub config
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Map config builder error

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?;

            let identify = identify::Behaviour::new(identify::Config::new(
                "/terra-link/0.1.0".into(),
                key.public(),
            ));

            let kad_config = kad::Config::new(StreamProtocol::new("/terra-link/kad/1.0.0"));
            let store = kad::store::MemoryStore::new(local_peer_id);
            let kademlia = kad::Behaviour::with_config(local_peer_id, store, kad_config);

            Ok(AppBehaviour {
                gossipsub,
                identify,
                kademlia,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let topic = gossipsub::IdentTopic::new("/world");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

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
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id: _id,
                        message,
                    })) => {
                        if let Ok(text) = String::from_utf8(message.data) {
                            let _ = event_sender.send(NetworkEvent::MessageReceived {
                                sender: peer_id,
                                text,
                            }).await;
                        }
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
                            NetworkCommand::PublishMessage(text) => {
                                let topic = gossipsub::IdentTopic::new("/world");
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, text.into_bytes()) {
                                    eprintln!("Publish error: {:?}", e);
                                }
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
