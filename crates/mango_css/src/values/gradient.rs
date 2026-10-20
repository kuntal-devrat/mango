use super::value::Value;
use mango_core::Color;
use std::collections::HashMap;

/// A CSS color stop for gradients.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorStop {
    pub color: Color,
    /// Position as a fraction [0.0, 1.0]. `None` means evenly spaced.
    pub position: Option<f32>,
    /// An optional explicit second position (for `color 10% 20%` double stops).
    pub end_position: Option<f32>,
    /// True when the color stop specified `currentColor`.
    pub is_current_color: bool,
    /// Custom property variable name (e.g. `--brand-color-canvas-default`) to resolve at computed style time.
    pub var_name: Option<String>,
}

impl ColorStop {
    pub fn new(color: Color, position: Option<f32>, end_position: Option<f32>) -> Self {
        Self {
            color,
            position,
            end_position,
            is_current_color: false,
            var_name: None,
        }
    }
}

/// CSS gradient type.
#[derive(Debug, Clone, PartialEq)]
pub enum Gradient {
    /// `linear-gradient(angle_deg, stops)`.
    /// Angle: 0° = to top, 90° = to right, 180° = to bottom.
    Linear {
        angle_deg: f32,
        stops: Vec<ColorStop>,
        /// `repeating-linear-gradient()` when true.
        repeating: bool,
    },
    /// `radial-gradient([size] [at <position>], stops)`.
    Radial {
        stops: Vec<ColorStop>,
        /// `repeating-radial-gradient()` when true.
        repeating: bool,
        /// Horizontal center fraction [0.0..1.0]. Default: 0.5.
        center_x: f32,
        /// Vertical center fraction [0.0..1.0]. Default: 0.5.
        center_y: f32,
        /// Optional explicit horizontal radius fraction.
        radius_x: Option<f32>,
        /// Optional explicit vertical radius fraction.
        radius_y: Option<f32>,
    },
    /// `conic-gradient(from <angle> at <position>, stops)`.
    Conic {
        angle_deg: f32,
        stops: Vec<ColorStop>,
        /// `repeating-conic-gradient()` when true.
        repeating: bool,
    },
}

impl Gradient {
    /// Returns the color stops of this gradient.
    pub fn stops(&self) -> &[ColorStop] {
        match self {
            Gradient::Linear { stops, .. }
            | Gradient::Radial { stops, .. }
            | Gradient::Conic { stops, .. } => stops,
        }
    }

    /// Resolves any `currentColor` stops in this gradient to the specified color.
    pub fn resolve_current_color(&mut self, current_color: Color) {
        let stops = match self {
            Gradient::Linear { stops, .. }
            | Gradient::Radial { stops, .. }
            | Gradient::Conic { stops, .. } => stops,
        };
        for stop in stops {
            if stop.is_current_color {
                stop.color = current_color;
            }
        }
    }

    /// Resolves any `var(--*)` stops in this gradient using custom properties.
    pub fn resolve_var_colors(&mut self, custom_props: &HashMap<String, Value>) {
        let stops = match self {
            Gradient::Linear { stops, .. }
            | Gradient::Radial { stops, .. }
            | Gradient::Conic { stops, .. } => stops,
        };
        for stop in stops {
            if let Some(ref var_name) = stop.var_name {
                let val_opt = custom_props.get(var_name).or_else(|| {
                    custom_props
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(var_name))
                        .map(|(_, v)| v)
                });
                if let Some(val) = val_opt {
                    match val {
                        Value::Color(c) => stop.color = *c,
                        Value::Keyword(s) | Value::String(s) => {
                            if let Some(c) = Value::parse_color(s) {
                                stop.color = c;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Rasterizes the gradient into a row-major `0xAARRGGBB` pixel buffer.
    pub fn rasterize(&self, width: u32, height: u32) -> Vec<u32> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let total = (width * height) as usize;
        let mut pixels = vec![0u32; total];
        let w = width as f32;
        let h = height as f32;
        match self {
            Gradient::Linear {
                angle_deg,
                stops,
                repeating,
            } => {
                if stops.is_empty() {
                    return pixels;
                }
                // Resolve positions
                let resolved = resolve_stops(stops);
                let (a_pos, _) = resolved[0];
                let (z_pos, _) = resolved[resolved.len() - 1];
                let span = (z_pos - a_pos).max(0.0001);
                let angle_rad = angle_deg.to_radians();
                let (sin_a, cos_a) = angle_rad.sin_cos();
                // Gradient line length: project corners onto gradient direction
                let grad_len = (w * sin_a.abs() + h * cos_a.abs()).max(1.0);
                for y in 0..height {
                    for x in 0..width {
                        // Normalised position along gradient line (0..1)
                        let cx = x as f32 + 0.5 - w * 0.5;
                        let cy = y as f32 + 0.5 - h * 0.5;
                        let proj = cx * sin_a + cy * (-cos_a);
                        let mut t = proj / grad_len + 0.5;
                        if *repeating {
                            t = a_pos + (t - a_pos).rem_euclid(span);
                        }
                        let color = sample_stops(&resolved, t);
                        pixels[(y * width + x) as usize] = color;
                    }
                }
            }
            Gradient::Radial {
                stops,
                repeating,
                center_x,
                center_y,
                radius_x,
                radius_y,
            } => {
                if stops.is_empty() {
                    return pixels;
                }
                let resolved = resolve_stops(stops);
                let (a_pos, _) = resolved[0];
                let (z_pos, _) = resolved[resolved.len() - 1];
                let span = (z_pos - a_pos).max(0.0001);
                let cx = w * center_x;
                let cy = h * center_y;
                let (rx, ry) = match (radius_x, radius_y) {
                    (Some(rx), Some(ry)) => ((w * rx).max(1.0), (h * ry).max(1.0)),
                    (Some(r), None) | (None, Some(r)) => {
                        let r_px = (w.max(h) * r).max(1.0);
                        (r_px, r_px)
                    }
                    (None, None) => {
                        let mx = cx.max(w - cx);
                        let my = cy.max(h - cy);
                        let r = (mx * mx + my * my).sqrt().max(1.0);
                        (r, r)
                    }
                };
                for y in 0..height {
                    for x in 0..width {
                        let dx = (x as f32 + 0.5 - cx) / rx;
                        let dy = (y as f32 + 0.5 - cy) / ry;
                        let mut t = (dx * dx + dy * dy).sqrt();
                        if *repeating {
                            t = a_pos + (t - a_pos).rem_euclid(span);
                        }
                        let color = sample_stops(&resolved, t);
                        pixels[(y * width + x) as usize] = color;
                    }
                }
            }
            Gradient::Conic {
                angle_deg,
                stops,
                repeating,
            } => {
                if stops.is_empty() {
                    return pixels;
                }
                let resolved = resolve_stops(stops);
                let (a_pos, _) = resolved[0];
                let (z_pos, _) = resolved[resolved.len() - 1];
                let span = (z_pos - a_pos).max(0.0001);
                let cx = w * 0.5;
                let cy = h * 0.5;
                // CSS conic gradients sweep clockwise starting from the 12 o'clock position,
                // offset by `from <angle>`.
                let from = angle_deg.to_radians();
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 + 0.5 - cx;
                        let dy = y as f32 + 0.5 - cy;
                        // atan2 measures from +x axis; rotate so 0 is straight up (12 o'clock)
                        let mut theta = dy.atan2(dx) + std::f32::consts::FRAC_PI_2 - from;
                        theta = theta.rem_euclid(std::f32::consts::TAU);
                        let mut t = theta / std::f32::consts::TAU;
                        if *repeating {
                            t = a_pos + (t - a_pos).rem_euclid(span);
                        }
                        let color = sample_stops(&resolved, t.clamp(0.0, 1.0));
                        pixels[(y * width + x) as usize] = color;
                    }
                }
            }
        }
        pixels
    }
}

fn resolve_stops(stops: &[ColorStop]) -> Vec<(f32, Color)> {
    let n = stops.len();
    let mut out: Vec<(f32, Color)> = Vec::with_capacity(n);
    for (i, stop) in stops.iter().enumerate() {
        let default_pos = if n == 1 {
            0.5
        } else {
            i as f32 / (n - 1) as f32
        };
        let start = stop.position.unwrap_or(default_pos);
        out.push((start, stop.color));
        if let Some(end) = stop.end_position {
            out.push((end, stop.color));
        }
    }
    // Enforce CSS monotonicity: each stop is at least its predecessor's position.
    let mut prev = f32::NEG_INFINITY;
    for entry in out.iter_mut() {
        if entry.0 < prev {
            entry.0 = prev;
        }
        prev = entry.0;
    }
    out
}

fn sample_stops(stops: &[(f32, Color)], t: f32) -> u32 {
    if stops.is_empty() {
        return 0;
    }
    if stops.len() == 1 {
        let c = stops[0].1;
        return ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    }
    // Find the bracketing stops
    let (a_pos, a_col) = stops[0];
    let (z_pos, z_col) = stops[stops.len() - 1];
    if t <= a_pos {
        let c = a_col;
        return ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    }
    if t >= z_pos {
        let c = z_col;
        return ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    }
    for i in 1..stops.len() {
        let (p1, c1) = stops[i - 1];
        let (p2, c2) = stops[i];
        if t >= p1 && t <= p2 {
            let span = (p2 - p1).max(0.0001);
            let f = ((t - p1) / span).clamp(0.0, 1.0);
            let r = lerp_u8(c1.r, c2.r, f);
            let g = lerp_u8(c1.g, c2.g, f);
            let b = lerp_u8(c1.b, c2.b, f);
            let a = lerp_u8(c1.a, c2.a, f);
            return ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
    }
    0
}

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}
