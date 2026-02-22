mod app;
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
                listen_addr = Some(args[2].parse::<libp2p::Multiaddr>().expect("Invalid Multiaddr"));
            }
            "dial" => {
                dial_addr = Some(args[2].parse::<libp2p::Multiaddr>().expect("Invalid Multiaddr"));
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

    let mut terminal = tui::init()?;
    let mut app = App::new();

    let (cmd_sender, cmd_receiver) = mpsc::channel(32);
    let (event_sender, mut event_receiver) = mpsc::channel(32);

    let local_peer_id = network::start_network(cmd_receiver, event_sender)
        .await
        .expect("Failed to start network");

    app.local_peer_id = Some(local_peer_id);

    if let Some(addr) = listen_addr {
        cmd_sender.send(NetworkCommand::Listen(addr)).await.unwrap();
    }
    if let Some(addr) = dial_addr {
        // Listen on random port first
        cmd_sender.send(NetworkCommand::Listen("/ip4/0.0.0.0/tcp/0".parse().unwrap())).await.unwrap();
        cmd_sender.send(NetworkCommand::Dial(addr)).await.unwrap();
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
    // 10 FPS for now
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
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

        // Process network events non-blocking
        while let Ok(event) = event_receiver.try_recv() {
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
