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
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            rotation_y: 0.0,
            last_width: 0,
            last_height: 0,
            projection_cache: Vec::new(),
        }
    }

    pub fn tick(&mut self) {
        // Rotate the globe slowly
        self.rotation_y = (self.rotation_y + 0.05) % (std::f64::consts::PI * 2.0);
    }

    pub fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }
}
