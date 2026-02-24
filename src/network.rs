use futures::StreamExt;
use libp2p::{
    gossipsub, identify, identity, kad, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
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
    pub relay_client: libp2p::relay::client::Behaviour,
    pub dcutr: libp2p::dcutr::Behaviour,
    pub autonat: libp2p::autonat::Behaviour,
    pub ping: libp2p::ping::Behaviour,
}

#[derive(Debug)]
pub enum NetworkCommand {
    Listen(Multiaddr),
    Dial(Multiaddr),
    ListenOnRelay(Multiaddr),
    PublishMessage { sender_id: String, text: String },
}

#[derive(Debug)]
pub enum NetworkEvent {
    Listening(Multiaddr),
    PeerConnected(PeerId, std::net::IpAddr),
    PeerDisconnected(PeerId),
    MessageReceived { sender_id: String, text: String },
    Error(String),
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
        .with_relay_client(noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key, relay_client| {
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

            let dcutr = libp2p::dcutr::Behaviour::new(local_peer_id);
            let autonat = libp2p::autonat::Behaviour::new(local_peer_id, Default::default());

            let ping = libp2p::ping::Behaviour::default();

            Ok(AppBehaviour {
                gossipsub,
                identify,
                kademlia,
                relay_client,
                dcutr,
                autonat,
                ping,
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
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        let mut ip = None;
                        for protocol in endpoint.get_remote_address().iter() {
                            match protocol {
                                libp2p::multiaddr::Protocol::Ip4(ipv4) => ip = Some(std::net::IpAddr::V4(ipv4)),
                                libp2p::multiaddr::Protocol::Ip6(ipv6) => ip = Some(std::net::IpAddr::V6(ipv6)),
                                _ => {}
                            }
                        }
                        if let Some(ip) = ip {
                            let _ = event_sender.send(NetworkEvent::PeerConnected(peer_id, ip)).await;
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        let _ = event_sender.send(NetworkEvent::PeerDisconnected(peer_id)).await;
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: _peer_id,
                        message_id: _id,
                        message,
                    })) => {
                        use prost::Message;
                        if let Ok(global_chat) = crate::proto::messages::GlobalChat::decode(message.data.as_slice()) {
                            let _ = event_sender.send(NetworkEvent::MessageReceived {
                                sender_id: global_chat.sender_id,
                                text: global_chat.text,
                            }).await;
                        }
                    }
                    other => {
                        use std::io::Write;
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("debug.log") {
                            let _ = writeln!(f, "Unhandled event: {:?}", other);
                        }
                    }
                },
                cmd = cmd_receiver.recv() => {
                    if let Some(command) = cmd {
                        match command {
                            NetworkCommand::Listen(addr) => {
                                if let Err(e) = swarm.listen_on(addr.clone()) {
                                    let _ = event_sender.send(NetworkEvent::Error(format!("Listen error on {}: {}", addr, e))).await;
                                }
                            }
                            NetworkCommand::Dial(addr) => {
                                if let Err(e) = swarm.dial(addr.clone()) {
                                    let _ = event_sender.send(NetworkEvent::Error(format!("Dial error for {}: {}", addr, e))).await;
                                }
                            }
                            NetworkCommand::ListenOnRelay(mut addr) => {
                                // To reserve the circuit
                                addr.push(libp2p::multiaddr::Protocol::P2pCircuit);
                                if let Err(e) = swarm.listen_on(addr.clone()) {
                                    let _ = event_sender.send(NetworkEvent::Error(format!("Relay Reservation error for {}: {}", addr, e))).await;
                                }
                            }
                            NetworkCommand::PublishMessage { sender_id, text } => {
                                use prost::Message;
                                use std::time::{SystemTime, UNIX_EPOCH};
                                let topic = gossipsub::IdentTopic::new("/world");

                                let timestamp = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;

                                let msg = crate::proto::messages::GlobalChat {
                                    sender_id,
                                    text,
                                    timestamp,
                                };

                                let mut buf = Vec::new();
                                msg.encode(&mut buf).unwrap();

                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, buf) {
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
