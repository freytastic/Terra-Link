use crate::network::NetworkEvent;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::io;

pub struct CachedPoint {
    pub screen_x: u16,
    pub screen_y: u16,
    pub original_u: f64,
    pub map_y: usize,
    pub intensity: f64,
}

#[derive(Default)]
pub struct App {
    pub should_quit: bool,
    pub rotation_y: f64,
    pub last_width: u16,
    pub last_height: u16,
    pub projection_cache: Vec<CachedPoint>,
    pub local_peer_id: Option<libp2p::PeerId>,
    pub peers: Vec<libp2p::PeerId>,
    pub listen_addrs: Vec<libp2p::Multiaddr>,
    // Chat State
    pub chat_messages: Vec<(String, String)>, // (sender, text)
    pub input_mode: bool,
    pub input_buffer: String,
    // Geo State
    pub geo_resolver: crate::geo::GeoResolver,
    pub peer_locations: std::collections::HashMap<libp2p::PeerId, (f64, f64, String)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            rotation_y: 0.0,
            last_width: 0,
            last_height: 0,
            projection_cache: Vec::new(),
            local_peer_id: None,
            peers: Vec::new(),
            listen_addrs: Vec::new(),
            chat_messages: Vec::new(),
            input_mode: false,
            input_buffer: String::new(),
            geo_resolver: crate::geo::GeoResolver::new("GeoLite2-City.mmdb"),
            peer_locations: std::collections::HashMap::new(),
        }
    }

    pub fn tick(&mut self) {
        // Rotate the globe slowly
        self.rotation_y = (self.rotation_y + 0.05) % (std::f64::consts::PI * 2.0);
    }

    pub fn handle_events(
        &mut self,
        cmd_sender: &mut tokio::sync::mpsc::Sender<crate::network::NetworkCommand>,
    ) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                self.handle_key(key, cmd_sender);
            }
        }
        Ok(())
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
        cmd_sender: &mut tokio::sync::mpsc::Sender<crate::network::NetworkCommand>,
    ) {
        if self.input_mode {
            match key.code {
                KeyCode::Enter => {
                    let msg = self.input_buffer.clone();
                    self.input_buffer.clear();
                    if !msg.is_empty() {
                        let _ = cmd_sender
                            .try_send(crate::network::NetworkCommand::PublishMessage(msg.clone()));
                        // Optimistically render our own message
                        let me = self
                            .local_peer_id
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "Me".to_string());
                        self.chat_messages.push((me, msg));
                    }
                    self.input_mode = false;
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Esc => {
                    self.input_mode = false;
                    self.input_buffer.clear();
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Enter => self.input_mode = true,
                _ => {}
            }
        }
    }

    pub fn handle_network_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Listening(addr) => {
                self.listen_addrs.push(addr);
            }
            NetworkEvent::PeerConnected(peer_id, ip) => {
                if !self.peers.contains(&peer_id) {
                    self.peers.push(peer_id);
                    if let Some(loc) = self.geo_resolver.get_fuzzed_location(ip) {
                        self.peer_locations.insert(peer_id, loc);
                    }
                }
            }
            NetworkEvent::PeerDisconnected(peer_id) => {
                self.peers.retain(|p| p != &peer_id);
                self.peer_locations.remove(&peer_id);
            }
            NetworkEvent::MessageReceived { sender, text } => {
                self.chat_messages.push((sender.to_string(), text));
                if self.chat_messages.len() > 100 {
                    self.chat_messages.remove(0); // keep it bounded
                }
            }
        }
    }
}
