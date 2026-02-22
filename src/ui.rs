use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};

use crate::app::App;

pub struct GlobeWidget<'a> {
    pub app: &'a mut App,
    pub sun_vector: (f64, f64, f64),
}

impl<'a> Widget for GlobeWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 1,
        });
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // If terminal resized, precompute all spatial projection math
        if self.app.last_width != inner.width || self.app.last_height != inner.height {
            self.app.last_width = inner.width;
            self.app.last_height = inner.height;
            self.app.projection_cache.clear();

            let width = inner.width as f64;
            let height = inner.height as f64;
            let cx = width / 2.0;
            let cy = height / 2.0;
            let r = (height / 2.0) - 1.0;

            if r > 0.0 {
                for y in 0..inner.height {
                    let dy = (y as f64 - cy) / r;
                    let dy_sq = dy * dy;

                    // Reusable unmodified Y for latitude math (points down)
                    let y0 = dy;
                    let normal_y = -dy;

                    for x in 0..inner.width {
                        let dx = (x as f64 - cx) / r * 0.45;
                        let d2 = dx * dx + dy_sq;

                        if d2 <= 1.0 {
                            let dz = f64::sqrt(1.0 - d2);
                            let normal_x = dx;
                            let normal_z = dz;

                            // Intensity is completely static for a given pixel if sun doesnt move
                            let intensity = normal_x * self.sun_vector.0
                                + normal_y * self.sun_vector.1
                                + normal_z * self.sun_vector.2;

                            // Precalculate the Unrotated mapping coordinates
                            let original_lon = dz.atan2(dx);
                            let original_u = (original_lon + std::f64::consts::PI)
                                / (2.0 * std::f64::consts::PI);

                            let lat = (-y0).asin();
                            let v = 1.0 - (lat + std::f64::consts::PI / 2.0) / std::f64::consts::PI;
                            let map_y = ((v * crate::globe::EARTH_MAP_HEIGHT as f64) as usize)
                                .clamp(0, crate::globe::EARTH_MAP_HEIGHT - 1);

                            self.app.projection_cache.push(crate::app::CachedPoint {
                                screen_x: inner.x + x,
                                screen_y: inner.y + y,
                                original_u,
                                map_y,
                                intensity,
                            });
                        }
                    }
                }
            }
        }

        let rot_u = self.app.rotation_y / (2.0 * std::f64::consts::PI);
        let map_width = crate::globe::EARTH_MAP_WIDTH as f64;

        for p in &self.app.projection_cache {
            if p.screen_x < inner.right() && p.screen_y < inner.bottom() {
                //  mapped rotating longitude
                let u = (p.original_u - rot_u).rem_euclid(1.0);
                let map_x = ((u * map_width) as usize).clamp(0, crate::globe::EARTH_MAP_WIDTH - 1);

                let is_land = crate::globe::EARTH_MAP[p.map_y].as_bytes()[map_x] == b'#';

                let (character, color) = crate::globe::get_appearance(is_land, p.intensity);

                buf.cell_mut((p.screen_x, p.screen_y))
                    .unwrap()
                    .set_char(character)
                    .set_fg(color);
            }
        }
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let block = Block::default()
        .title("Terra-Link 🌍 - The Live Decentralized Terminal Globe")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(block, f.area());

    // A simple sun vector pointing right and slightly down
    let sun_vector = (1.0, 0.2, 0.0);

    let globe = GlobeWidget {
        app: &mut *app,
        sun_vector,
    };

    f.render_widget(globe, f.area());

    // Overlay Network Info
    let mut net_info = vec![];
    if let Some(peer_id) = app.local_peer_id {
        net_info.push(ratatui::text::Line::from(format!("PeerID: {}", peer_id)));
    }
    net_info.push(ratatui::text::Line::from(format!(
        "Listening on: {:?}",
        app.listen_addrs
    )));
    net_info.push(ratatui::text::Line::from(format!(
        "Peers ({}): {:?}",
        app.peers.len(),
        app.peers
    )));

    let info_widget = ratatui::widgets::Paragraph::new(net_info)
        .block(Block::default().borders(Borders::ALL).title("Network Info"))
        .style(Style::default().fg(Color::Cyan));

    let info_area = ratatui::layout::Rect {
        x: f.area().x,
        y: f.area().bottom().saturating_sub(6),
        width: 100.min(f.area().width),
        height: 6,
    };
    f.render_widget(info_widget, info_area);

    // Overlay Chat Feed
    let mut chat_lines = vec![];
    for (sender, text) in app.chat_messages.iter().rev().take(10).rev() {
        // libp2p PeerIds all start with `12D3KooW...` for Ed25519 keys, so it should show the END of the string to distinguish them.
        let short_id = if sender.len() > 8 {
            &sender[sender.len() - 8..]
        } else {
            sender
        };
        chat_lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!("{short_id}: "),
                Style::default().fg(Color::Yellow),
            ),
            ratatui::text::Span::raw(text),
        ]));
    }

    let chat_widget = ratatui::widgets::Paragraph::new(chat_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Global Chat (/world)"),
        )
        .style(Style::default().fg(Color::White))
        .wrap(ratatui::widgets::Wrap { trim: true });

    let chat_area = ratatui::layout::Rect {
        x: f.area().right().saturating_sub(40).max(0),
        y: f.area().bottom().saturating_sub(12),
        width: 40.min(f.area().width),
        height: 12,
    };
    f.render_widget(chat_widget, chat_area);

    // Overlay Input Box
    if app.input_mode {
        let input_widget = ratatui::widgets::Paragraph::new(app.input_buffer.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Message")
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().fg(Color::White));

        let input_area = ratatui::layout::Rect {
            x: chat_area.x,
            y: chat_area.bottom().saturating_sub(3),
            width: chat_area.width,
            height: 3,
        };

        // Clear background for input box
        f.render_widget(ratatui::widgets::Clear, input_area);
        f.render_widget(input_widget, input_area);

        // Render Cursor
        f.set_cursor_position(ratatui::layout::Position::new(
            input_area.x + app.input_buffer.len() as u16 + 1,
            input_area.y + 1,
        ));
    }
}
