use ratatui::style::Color;
use std::f64::consts::PI;

pub struct GlobePoint {
    pub x: f64,
    pub y: f64,
    pub color: Color,
}

/// Computes the 3D points of the globe rotated and projected onto 2D.
/// Uses orthographic projection, returning points that are visible (z >= 0).
pub fn project_globe(rotation_y: f64, sun_vector: (f64, f64, f64)) -> Vec<GlobePoint> {
    let mut points = Vec::new();
    let num_lat = 40;
    let num_lon = 80;

    for lat_idx in 0..num_lat {
        let lat = -PI / 2.0 + (PI * lat_idx as f64) / (num_lat as f64 - 1.0);
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();

        for lon_idx in 0..num_lon {
            let lon = -PI + (2.0 * PI * lon_idx as f64) / (num_lon as f64);

            // Base sphere coordinates (radius = 1)
            let x0 = cos_lat * lon.cos();
            let y0 = sin_lat;
            let z0 = cos_lat * lon.sin();

            // Rotate around Y-axis
            let x = x0 * rotation_y.cos() - z0 * rotation_y.sin();
            let y = y0;
            let z = x0 * rotation_y.sin() + z0 * rotation_y.cos();

            // Only forward-facing points (Orthographic projection)
            if z >= 0.0 {
                // Calculate dot product with sun vector for day/night shading
                // sun_vector is assumed normalized.
                let dot = x * sun_vector.0 + y * sun_vector.1 + z * sun_vector.2;

                let color = if dot > 0.0 {
                    Color::Green // Day side
                } else {
                    Color::DarkGray // Night side
                };

                points.push(GlobePoint { x, y, color });
            }
        }
    }

    points
}
