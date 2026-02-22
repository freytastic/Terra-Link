mod app;
mod globe;
mod tui;
mod ui;

use app::App;
use std::io;

fn main() -> io::Result<()> {
    let mut terminal = tui::init()?;
    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    tui::restore()?;
    res
}

fn run_app(terminal: &mut tui::Tui, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui::render(f, app))?;

        app.handle_events()?;
        app.tick();
    }
    Ok(())
}
