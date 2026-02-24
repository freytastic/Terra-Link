mod app;
mod geo;
mod globe;
mod network;
mod proto;
mod tui;
mod ui;

use app::App;
use network::{NetworkCommand, NetworkEvent};
use std::env;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut listen_addr = None;
    let mut dial_addr = None;

    if args.len() >= 3 {
        match args[1].as_str() {
            "listen" => {
                listen_addr = Some(
                    args[2]
                        .parse::<libp2p::Multiaddr>()
                        .expect("Invalid Multiaddr"),
                );
            }
            "dial" => {
                dial_addr = Some(
                    args[2]
                        .parse::<libp2p::Multiaddr>()
                        .expect("Invalid Multiaddr"),
                );
            }
            _ => {
                println!("Usage: {} [listen|dial] <multiaddr>", args[0]);
                return Ok(());
            }
        }
    } else if args.len() == 2 {
        println!("Usage: {} [listen|dial] <multiaddr>", args[0]);
        return Ok(());
    }

    let _ = dotenvy::dotenv();

    ensure_geolite_db().await?;

    let mut terminal = tui::init()?;
    let mut app = App::new();

    let (cmd_sender, cmd_receiver) = mpsc::channel(32);
    let (event_sender, mut event_receiver) = mpsc::channel(32);

    let local_peer_id = network::start_network(cmd_receiver, event_sender)
        .await
        .expect("Failed to start network");

    app.local_peer_id = Some(local_peer_id);

    if let Some(ref addr) = listen_addr {
        cmd_sender
            .send(NetworkCommand::Listen(addr.clone()))
            .await
            .expect("Failed to send listen command: network thread died");
    }
    if let Some(ref addr) = dial_addr {
        // Listen on random port first
        cmd_sender
            .send(NetworkCommand::Listen(
                "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
            ))
            .await
            .expect("Failed to initialize random listen port");
        cmd_sender
            .send(NetworkCommand::Dial(addr.clone()))
            .await
            .expect("Failed to send dial command: network thread died");
    }

    // If RELAY_NODE is defined in .env, automatically dial it!
    if let Ok(relay_str) = std::env::var("RELAY_NODE") {
        if let Ok(relay_addr) = relay_str.parse::<libp2p::Multiaddr>() {
            // Need a listen port open to perform NAT hole punching
            if listen_addr.is_none() && dial_addr.is_none() {
                cmd_sender
                    .send(NetworkCommand::Listen(
                        "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
                    ))
                    .await
                    .expect("Failed to initialize random listen port for relay");
            }

            // libp2p queuing , dispatch ListenOnRelay immediately
            cmd_sender
                .send(NetworkCommand::Dial(relay_addr.clone()))
                .await
                .expect("Failed to dial relay node");

            //  must wait for the Identify protocol to complete before reserving the circuit!
            // If send ListenOnRelay immediately, libp2p may try to open the circuit stream
            let cmd_sender_clone = cmd_sender.clone();
            tokio::spawn(async move {
                // Give the connection and Identify exchange 2 seconds to complete
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _ = cmd_sender_clone
                    .send(NetworkCommand::ListenOnRelay(relay_addr))
                    .await;
            });
        }
    }

    let res = run_app(&mut terminal, &mut app, &mut event_receiver, cmd_sender).await;

    tui::restore()?;
    res
}

async fn run_app(
    terminal: &mut tui::Tui,
    app: &mut App,
    event_receiver: &mut mpsc::Receiver<NetworkEvent>,
    mut cmd_sender: mpsc::Sender<NetworkCommand>,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut last_presence_broadcast = Instant::now();
    let mut needs_render = true;

    while !app.should_quit {
        if needs_render {
            terminal.draw(|f| ui::render(f, app))?;
            needs_render = false;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        // Sleep to avoid spinning the CPU aggressively
        if timeout > Duration::from_millis(10) {
            std::thread::sleep(timeout - Duration::from_millis(10));
        }

        // Periodically broadcast presence if we have local addresses
        if last_presence_broadcast.elapsed() > Duration::from_secs(15) {
            if let Some(me) = app.local_peer_id {
                let addrs: Vec<String> = app.listen_addrs.iter().map(|a| a.to_string()).collect();
                if !addrs.is_empty() {
                    let _ = cmd_sender.try_send(NetworkCommand::BroadcastPresence {
                        sender_id: me.to_string(),
                        listen_addrs: addrs,
                    });
                }
            }
            last_presence_broadcast = Instant::now();
        }

        // Process network events non-blocking
        while let Ok(event) = event_receiver.try_recv() {
            match &event {
                NetworkEvent::PeerConnected(peer, ip) => {
                    println!("[CONNECTED] Peer: {} | IP: {}", peer, ip);
                }
                NetworkEvent::PeerDisconnected(peer) => {
                    println!("[DISCONNECTED] Peer: {}", peer);
                }
                NetworkEvent::Listening(addr) => {
                    println!("[LISTENING] {}", addr);
                }
                NetworkEvent::PeerDiscovered(peer, addr) => {
                    // Try to autodial the discovered peer if we aren't connected!
                    if let Ok(peer_id) = peer.parse::<libp2p::PeerId>() {
                        if !app.peers.contains(&peer_id) && Some(peer_id) != app.local_peer_id {
                            let _ = cmd_sender.try_send(NetworkCommand::Dial(addr.clone()));
                        }
                    }
                }
                NetworkEvent::Error(msg) => {
                    eprintln!("[ERROR] {}", msg);
                }
                _ => {}
            }

            app.handle_network_event(event);
            needs_render = true;
        }

        if ratatui::crossterm::event::poll(Duration::from_millis(10))? {
            app.handle_events(&mut cmd_sender)?;
            needs_render = true; // Input might change state
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
            needs_render = true; // Globe rotated, we must render
        }
    }
    Ok(())
}

async fn ensure_geolite_db() -> io::Result<()> {
    let db_path = "GeoLite2-City.mmdb";
    if !std::path::Path::new(db_path).exists() {
        println!("GeoLite2-City database not found. Downloading (60MB+)...");
        let url = "https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-City.mmdb";

        let mut response = reqwest::get(url)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Download failed: {}", e)))?;

        let mut file = std::fs::File::create(db_path)?;
        while let Some(chunk) = response.chunk().await.map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("Failed to read chunk: {}", e))
        })? {
            use std::io::Write;
            file.write_all(&chunk)?;
        }
        println!("Download complete!");
    }
    Ok(())
}
