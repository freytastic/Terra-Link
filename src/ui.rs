use ratatui::{
    Frame,
    style::Color,
    widgets::{
        Block, Borders,
        canvas::{Canvas, Points},
    },
};

use crate::app::App;
use crate::globe::project_globe;

pub fn render(f: &mut Frame, app: &App) {
    let sun_vector = (1.0, 0.0, 0.0); // Assume the sun is to the right
    let globe_points = project_globe(app.rotation_y, sun_vector);

    // Filter points by color to draw them in batches
    let day_points: Vec<(f64, f64)> = globe_points
        .iter()
        .filter(|p| p.color == Color::Green)
        .map(|p| (p.x * 2.1, p.y)) // Aspect ratio correction
        .collect();

    let night_points: Vec<(f64, f64)> = globe_points
        .iter()
        .filter(|p| p.color == Color::DarkGray)
        .map(|p| (p.x * 2.1, p.y))
        .collect();

    let canvas = Canvas::default()
        .block(
            Block::default()
                .title("Terra-Link 🌍 - The Live Decentralized Terminal Globe")
                .borders(Borders::ALL),
        )
        .x_bounds([-2.5, 2.5])
        .y_bounds([-1.2, 1.2])
        .marker(ratatui::symbols::Marker::Braille)
        .paint(move |ctx| {
            ctx.draw(&Points {
                coords: &night_points,
                color: Color::DarkGray,
            });
            ctx.draw(&Points {
                coords: &day_points,
                color: Color::Green,
            });
        });

    f.render_widget(canvas, f.area());
}
