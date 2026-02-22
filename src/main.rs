mod app;
mod globe;
mod tui;
mod ui;

use app::App;
use std::io;
use std::time::{Duration, Instant};

fn main() -> io::Result<()> {
    let mut terminal = tui::init()?;
    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    tui::restore()?;
    res
}

fn run_app(terminal: &mut tui::Tui, app: &mut App) -> io::Result<()> {
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

        if ratatui::crossterm::event::poll(Duration::from_millis(10))? {
            app.handle_events()?;
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
