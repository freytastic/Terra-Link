use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::io;
use std::time::Duration;

#[derive(Default)]
pub struct App {
    pub should_quit: bool,
    pub rotation_y: f64,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self) {
        // Rotate the globe slowly
        self.rotation_y = (self.rotation_y + 0.05) % (std::f64::consts::PI * 2.0);
    }

    pub fn handle_events(&mut self) -> io::Result<()> {
        let timeout = Duration::from_millis(16); // ~60 FPS
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key(key);
                }
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
