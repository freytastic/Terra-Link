use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
    Frame,
};

use crate::app::App;

const NEON_CYAN: Color = Color::Rgb(0, 255, 255);
const NEON_PINK: Color = Color::Rgb(255, 45, 149);
const NEON_YELLOW: Color = Color::Rgb(255, 215, 0);
const NEON_VIOLET: Color = Color::Rgb(138, 43, 226);
const HUD_DIM: Color = Color::Rgb(80, 80, 100);
const HUD_TEXT: Color = Color::Rgb(180, 200, 220);
const HUD_BG: Color = Color::Rgb(8, 8, 18);

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

        // Scanline phase shifts every few ticks for subtle CRT movement
        let scanline_offset = (self.app.tick_count / 3) as u16;

        for p in &self.app.projection_cache {
            if p.screen_x < inner.right() && p.screen_y < inner.bottom() {
                //  mapped rotating longitude
                let u = (p.original_u - rot_u).rem_euclid(1.0);
                let map_x = ((u * map_width) as usize).clamp(0, crate::globe::EARTH_MAP_WIDTH - 1);

                let is_land = crate::globe::EARTH_MAP[p.map_y].as_bytes()[map_x] == b'#';

                let (character, mut color) = crate::globe::get_appearance(is_land, p.intensity);

                // Scanline dimming every 3rd row gets slightly darker
                if (p.screen_y + scanline_offset) % 3 == 0 {
                    color = dim_color(color, 0.75);
                }

                buf.cell_mut((p.screen_x, p.screen_y))
                    .unwrap()
                    .set_char(character)
                    .set_fg(color);
            }
        }

        // Render Peer Markers
        let r = (inner.height as f64 / 2.0) - 1.0;
        let cx = inner.width as f64 / 2.0;
        let cy = inner.height as f64 / 2.0;

        // Breathing pulse — alternate marker glyph on tick
        let marker_char = if self.app.tick_count % 6 < 3 {
            '◈'
        } else {
            '◇'
        };

        for (_, (lat, lon, _)) in &self.app.peer_locations {
            let lat_rad = lat.to_radians();
            let lon_rad = lon.to_radians();

            // The texture is mapped such that longitude 0 is the center.
            //  revolve the sphere by subtracting rotation_y.
            let current_lon = lon_rad - self.app.rotation_y;

            let x = lat_rad.cos() * current_lon.sin();
            let y = -lat_rad.sin(); // Maps to screen Y downwards
            let z = lat_rad.cos() * current_lon.cos();

            // If the point is on the hemisphere facing the camera
            if z > 0.0 {
                let screen_x = (cx + (x * r / 0.45)).round() as u16;
                let screen_y = (cy + y * r).round() as u16;

                if screen_x < inner.width && screen_y < inner.height {
                    if let Some(cell) = buf.cell_mut((inner.x + screen_x, inner.y + screen_y)) {
                        cell.set_char(marker_char).set_fg(NEON_YELLOW);
                    }
                }
            }
        }
    }
}

// Dim an RGB color by a given factor (0.0 = black, 1.0 = unchanged).
fn dim_color(color: Color, factor: f64) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor) as u8,
            (g as f64 * factor) as u8,
            (b as f64 * factor) as u8,
        ),
        other => other,
    }
}

// Returns a connection status indicator dot and color based on peer count.
fn connection_indicator(peer_count: usize, tick: u64) -> (char, Color) {
    if peer_count == 0 {
        // Blink red when disconnected
        let ch = if tick % 6 < 3 { '●' } else { '○' };
        (ch, Color::Rgb(255, 50, 50))
    } else if peer_count < 3 {
        ('●', NEON_YELLOW)
    } else {
        ('●', Color::Rgb(0, 255, 100))
    }
}

// Returns an oscillating network pulse bar.
fn network_pulse(tick: u64) -> &'static str {
    const FRAMES: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    // Bounce: 0-7 then 7-0
    let pos = (tick % 14) as usize;
    if pos < 8 {
        FRAMES[pos]
    } else {
        FRAMES[14 - pos - 1]
    }
}

// Format a signal strength bar for a peer.
fn signal_bar(is_relayed: bool) -> (&'static str, Color) {
    if is_relayed {
        ("▰▰▱▱▱", NEON_YELLOW)
    } else {
        ("▰▰▰▰▱", Color::Rgb(0, 255, 100))
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    if !app.boot_complete {
        render_boot_splash(f, app);
        return;
    }

    let (conn_dot, conn_color) = connection_indicator(app.peers.len(), app.tick_count);
    let pulse = network_pulse(app.tick_count);

    let title = Line::from(vec![
        Span::styled("╡ ", Style::default().fg(HUD_DIM)),
        Span::styled(
            "TERRA-LINK",
            Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" v0.1.0 ", Style::default().fg(HUD_DIM)),
        Span::styled("╞", Style::default().fg(HUD_DIM)),
    ]);

    let status_bar = Line::from(vec![
        Span::styled("╡ ", Style::default().fg(HUD_DIM)),
        Span::styled(format!("{conn_dot}"), Style::default().fg(conn_color)),
        Span::styled(
            format!(" NODES: {} ", app.peers.len()),
            Style::default().fg(HUD_TEXT),
        ),
        Span::styled("│ ", Style::default().fg(HUD_DIM)),
        Span::styled(format!("{pulse}"), Style::default().fg(NEON_CYAN)),
        Span::styled(" MESH ", Style::default().fg(HUD_TEXT)),
        Span::styled("╞", Style::default().fg(HUD_DIM)),
    ]);

    let block = Block::default()
        .title_top(title)
        .title_bottom(status_bar)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(NEON_CYAN))
        .style(Style::default().bg(HUD_BG));

    f.render_widget(block, area);

    let sun_vector = (1.0, 0.2, 0.0);
    let globe = GlobeWidget {
        app: &mut *app,
        sun_vector,
    };
    f.render_widget(globe, area);

    render_network_info(f, app);

    render_chat(f, app);

    render_keybind_footer(f, app);
}

fn render_network_info(f: &mut Frame, app: &App) {
    let area = f.area();
    let mut lines = vec![];

    if let Some(peer_id) = app.local_peer_id {
        let full = peer_id.to_string();
        let short = if full.len() > 12 {
            format!("{}…{}", &full[..4], &full[full.len() - 8..])
        } else {
            full
        };
        lines.push(Line::from(vec![
            Span::styled("⌘ ", Style::default().fg(NEON_PINK)),
            Span::styled(short, Style::default().fg(HUD_TEXT)),
        ]));
    }

    lines.push(Line::from(vec![Span::styled(
        format!("  Peers: {}", app.peers.len()),
        Style::default().fg(HUD_DIM),
    )]));

    for peer in &app.peers {
        let full_id = peer.to_string();
        let short_id = if full_id.len() > 8 {
            &full_id[full_id.len() - 8..]
        } else {
            &full_id
        };

        let (bar, bar_color) = signal_bar(false);

        if let Some((_, _, loc)) = app.peer_locations.get(peer) {
            lines.push(Line::from(vec![
                Span::styled("  ⌘ ", Style::default().fg(NEON_YELLOW)),
                Span::styled(format!("{loc}"), Style::default().fg(HUD_TEXT)),
                Span::styled(format!("  {bar}"), Style::default().fg(bar_color)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  ◇ ", Style::default().fg(HUD_DIM)),
                Span::styled(short_id.to_string(), Style::default().fg(HUD_DIM)),
            ]));
        }
    }

    let info_height = (3 + app.peers.len() as u16).max(5);
    let info_area = Rect {
        x: area.x + 1,
        y: area.bottom().saturating_sub(info_height + 2), // +2 for keybind footer
        width: 50.min(area.width.saturating_sub(2)),
        height: info_height,
    };

    let info_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::styled("┤ ", Style::default().fg(HUD_DIM)),
                    Span::styled("NETWORK", Style::default().fg(NEON_CYAN)),
                    Span::styled(" ├", Style::default().fg(HUD_DIM)),
                ]))
                .border_style(Style::default().fg(NEON_CYAN.into())),
        )
        .style(Style::default().fg(HUD_TEXT).bg(HUD_BG));

    f.render_widget(Clear, info_area);
    f.render_widget(info_widget, info_area);
}

fn render_chat(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let mut chat_lines = vec![];
    for (sender, text) in app.chat_messages.iter().rev().take(10).rev() {
        let short_id = if sender.len() > 8 {
            &sender[sender.len() - 8..]
        } else {
            sender
        };
        chat_lines.push(Line::from(vec![
            Span::styled(format!("{short_id}: "), Style::default().fg(NEON_PINK)),
            Span::styled(text, Style::default().fg(HUD_TEXT)),
        ]));
    }

    let chat_widget = Paragraph::new(chat_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::styled("┤ ", Style::default().fg(HUD_DIM)),
                    Span::styled("GLOBAL FEED", Style::default().fg(NEON_CYAN)),
                    Span::styled(" :: ", Style::default().fg(HUD_DIM)),
                    Span::styled("/world", Style::default().fg(NEON_VIOLET)),
                    Span::styled(" ├", Style::default().fg(HUD_DIM)),
                ]))
                .border_style(Style::default().fg(NEON_CYAN)),
        )
        .style(Style::default().fg(HUD_TEXT).bg(HUD_BG))
        .wrap(Wrap { trim: true });

    let chat_area = Rect {
        x: area.right().saturating_sub(42).max(0),
        y: area.bottom().saturating_sub(14), // 12 + 2 for keybind footer
        width: 42.min(area.width),
        height: 12,
    };

    f.render_widget(Clear, chat_area);
    f.render_widget(chat_widget, chat_area);

    if app.input_mode {
        let input_text = format!(">_ {}", app.input_buffer);
        let input_widget = Paragraph::new(input_text.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Line::from(vec![
                        Span::styled("┤ ", Style::default().fg(HUD_DIM)),
                        Span::styled("TRANSMIT", Style::default().fg(NEON_YELLOW)),
                        Span::styled(" ├", Style::default().fg(HUD_DIM)),
                    ]))
                    .border_style(Style::default().fg(NEON_YELLOW)),
            )
            .style(Style::default().fg(Color::White).bg(HUD_BG));

        let input_area = Rect {
            x: chat_area.x,
            y: chat_area.bottom().saturating_sub(3),
            width: chat_area.width,
            height: 3,
        };

        f.render_widget(Clear, input_area);
        f.render_widget(input_widget, input_area);

        // Cursor after ">_ " prefix (3 chars) + buffer length
        f.set_cursor_position(ratatui::layout::Position::new(
            input_area.x + app.input_buffer.len() as u16 + 4,
            input_area.y + 1,
        ));
    }
}

fn render_keybind_footer(f: &mut Frame, app: &App) {
    let area = f.area();

    let (conn_dot, conn_color) = connection_indicator(app.peers.len(), app.tick_count);

    let legend = Line::from(vec![
        Span::styled(" [", Style::default().fg(HUD_DIM)),
        Span::styled("Q", Style::default().fg(NEON_YELLOW)),
        Span::styled("]uit  [", Style::default().fg(HUD_DIM)),
        Span::styled("Enter", Style::default().fg(NEON_YELLOW)),
        Span::styled("]Chat  │  ", Style::default().fg(HUD_DIM)),
        Span::styled(format!("{conn_dot}"), Style::default().fg(conn_color)),
        Span::styled(
            format!(" {} nodes online", app.peers.len()),
            Style::default().fg(HUD_TEXT),
        ),
    ]);

    let footer_area = Rect {
        x: area.x + 1,
        y: area.bottom().saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };

    let footer = Paragraph::new(legend).style(Style::default().bg(HUD_BG));
    f.render_widget(footer, footer_area);
}

fn render_boot_splash(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let bg = Block::default().style(Style::default().bg(HUD_BG));
    f.render_widget(bg, area);

    // Blinking cursor effect
    let cursor_char = if app.tick_count % 6 < 3 { "█" } else { " " };
    let input_display = format!(">_ {}{}", app.nickname_buffer, cursor_char);

    // Remaining characters indicator
    let remaining = 8 - app.nickname_buffer.len();
    let char_hint = if app.nickname_buffer.is_empty() {
        "    (max 8 characters)".to_string()
    } else {
        format!("    ({remaining} remaining)")
    };

    let splash_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄",
            Style::default().fg(NEON_CYAN),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "       T E R R A ",
                Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
            ),
            Span::styled("- ", Style::default().fg(HUD_DIM)),
            Span::styled(
                "L I N K",
                Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "    DECENTRALIZED SPATIAL MESH",
            Style::default().fg(HUD_DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀",
            Style::default().fg(NEON_CYAN),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    ENTER YOUR NICKNAME (OPTIONAL)",
            Style::default().fg(NEON_YELLOW),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(&input_display, Style::default().fg(Color::White)),
        ]),
        Line::from(Span::styled(&char_hint, Style::default().fg(HUD_DIM))),
        Line::from(""),
        Line::from(Span::styled(
            "    Press [ENTER] to continue",
            Style::default().fg(HUD_DIM),
        )),
        Line::from(""),
    ];

    let splash_height = splash_lines.len() as u16;
    let splash_width = 44;
    let splash_area = Rect {
        x: area.x + (area.width.saturating_sub(splash_width)) / 2,
        y: area.y + (area.height.saturating_sub(splash_height)) / 2,
        width: splash_width.min(area.width),
        height: splash_height.min(area.height),
    };

    let splash = Paragraph::new(splash_lines).style(Style::default().bg(HUD_BG));
    f.render_widget(splash, splash_area);
}
