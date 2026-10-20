//! SVG path `d` attribute parser (commands M, m, L, l, H, h, V, v, C, c, S, s, Q, q, T, t, A, a, Z, z).

use std::f32::consts::PI;
use tiny_skia::{Path, PathBuilder};

/// Parses an SVG path data string `d` into a `tiny_skia::Path`.
pub fn parse_svg_path(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mut tokens = PathTokenizer::new(d);

    let mut cur_x = 0.0f32;
    let mut cur_y = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;
    let mut last_cp_x = 0.0f32;
    let mut last_cp_y = 0.0f32;
    let mut last_cmd = ' ';

    while let Some(cmd) = tokens.next_cmd() {
        match cmd {
            'M' | 'm' => {
                let is_rel = cmd == 'm';
                let mut first = true;
                while let (Some(x), Some(y)) = (tokens.next_f32(), tokens.next_f32()) {
                    let nx = if is_rel { cur_x + x } else { x };
                    let ny = if is_rel { cur_y + y } else { y };
                    if first {
                        pb.move_to(nx, ny);
                        start_x = nx;
                        start_y = ny;
                        first = false;
                    } else {
                        pb.line_to(nx, ny);
                    }
                    cur_x = nx;
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'L' | 'l' => {
                let is_rel = cmd == 'l';
                while let (Some(x), Some(y)) = (tokens.next_f32(), tokens.next_f32()) {
                    let nx = if is_rel { cur_x + x } else { x };
                    let ny = if is_rel { cur_y + y } else { y };
                    pb.line_to(nx, ny);
                    cur_x = nx;
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'H' | 'h' => {
                let is_rel = cmd == 'h';
                while let Some(x) = tokens.next_f32() {
                    let nx = if is_rel { cur_x + x } else { x };
                    pb.line_to(nx, cur_y);
                    cur_x = nx;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'V' | 'v' => {
                let is_rel = cmd == 'v';
                while let Some(y) = tokens.next_f32() {
                    let ny = if is_rel { cur_y + y } else { y };
                    pb.line_to(cur_x, ny);
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'C' | 'c' => {
                let is_rel = cmd == 'c';
                while let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let (nx1, ny1) = if is_rel {
                        (cur_x + x1, cur_y + y1)
                    } else {
                        (x1, y1)
                    };
                    let (nx2, ny2) = if is_rel {
                        (cur_x + x2, cur_y + y2)
                    } else {
                        (x2, y2)
                    };
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };

                    pb.cubic_to(nx1, ny1, nx2, ny2, nx, ny);
                    last_cp_x = nx2;
                    last_cp_y = ny2;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'S' | 's' => {
                let is_rel = cmd == 's';
                while let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let (nx2, ny2) = if is_rel {
                        (cur_x + x2, cur_y + y2)
                    } else {
                        (x2, y2)
                    };
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };

                    let (nx1, ny1) = if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                        (2.0 * cur_x - last_cp_x, 2.0 * cur_y - last_cp_y)
                    } else {
                        (cur_x, cur_y)
                    };

                    pb.cubic_to(nx1, ny1, nx2, ny2, nx, ny);
                    last_cp_x = nx2;
                    last_cp_y = ny2;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'Q' | 'q' => {
                let is_rel = cmd == 'q';
                while let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let (nx1, ny1) = if is_rel {
                        (cur_x + x1, cur_y + y1)
                    } else {
                        (x1, y1)
                    };
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };

                    pb.quad_to(nx1, ny1, nx, ny);
                    last_cp_x = nx1;
                    last_cp_y = ny1;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'T' | 't' => {
                let is_rel = cmd == 't';
                while let (Some(x), Some(y)) = (tokens.next_f32(), tokens.next_f32()) {
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };
                    let (nx1, ny1) = if matches!(last_cmd, 'Q' | 'q' | 'T' | 't') {
                        (2.0 * cur_x - last_cp_x, 2.0 * cur_y - last_cp_y)
                    } else {
                        (cur_x, cur_y)
                    };

                    pb.quad_to(nx1, ny1, nx, ny);
                    last_cp_x = nx1;
                    last_cp_y = ny1;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'A' | 'a' => {
                let is_rel = cmd == 'a';
                while let (
                    Some(rx),
                    Some(ry),
                    Some(x_axis_rotation),
                    Some(large_arc_flag),
                    Some(sweep_flag),
                    Some(x),
                    Some(y),
                ) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_flag(),
                    tokens.next_flag(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let nx = if is_rel { cur_x + x } else { x };
                    let ny = if is_rel { cur_y + y } else { y };

                    add_svg_arc_to_path(
                        &mut pb,
                        cur_x,
                        cur_y,
                        rx.abs(),
                        ry.abs(),
                        x_axis_rotation,
                        large_arc_flag != 0,
                        sweep_flag != 0,
                        nx,
                        ny,
                    );

                    cur_x = nx;
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'Z' | 'z' => {
                pb.close();
                cur_x = start_x;
                cur_y = start_y;
                last_cp_x = cur_x;
                last_cp_y = cur_y;
            }
            _ => {}
        }
        last_cmd = cmd;
    }

    pb.finish()
}

/// Convert an SVG arc segment into cubic Bézier curves and add to the builder.
#[allow(clippy::too_many_arguments)]
fn add_svg_arc_to_path(
    pb: &mut PathBuilder,
    x0: f32,
    y0: f32,
    mut rx: f32,
    mut ry: f32,
    x_axis_rotation: f32,
    large_arc_flag: bool,
    sweep_flag: bool,
    x1: f32,
    y1: f32,
) {
    if (x0 - x1).abs() < 1e-6 && (y0 - y1).abs() < 1e-6 {
        return;
    }
    if rx < 1e-6 || ry < 1e-6 {
        pb.line_to(x1, y1);
        return;
    }

    let phi = x_axis_rotation * PI / 180.0;
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    let dx = (x0 - x1) / 2.0;
    let dy = (y0 - y1) / 2.0;

    let x1_p = cos_phi * dx + sin_phi * dy;
    let y1_p = -sin_phi * dx + cos_phi * dy;

    // Radii correction
    let prx = rx * rx;
    let pry = ry * ry;
    let px1 = x1_p * x1_p;
    let py1 = y1_p * y1_p;

    let radii_check = px1 / prx + py1 / pry;
    if radii_check > 1.0 {
        let factor = radii_check.sqrt();
        rx *= factor;
        ry *= factor;
    }

    let sign = if large_arc_flag != sweep_flag {
        1.0
    } else {
        -1.0
    };
    let numer = (rx * rx * ry * ry) - (rx * rx * y1_p * y1_p) - (ry * ry * x1_p * x1_p);
    let denom = (rx * rx * y1_p * y1_p) + (ry * ry * x1_p * x1_p);
    let sq = if denom > 0.0 {
        (numer.max(0.0) / denom).sqrt()
    } else {
        0.0
    };

    let cx_p = sign * sq * (rx * y1_p / ry);
    let cy_p = sign * sq * -(ry * x1_p / rx);

    let cx = cos_phi * cx_p - sin_phi * cy_p + (x0 + x1) / 2.0;
    let cy = sin_phi * cx_p + cos_phi * cy_p + (y0 + y1) / 2.0;

    let theta1 = angle_between(1.0, 0.0, (x1_p - cx_p) / rx, (y1_p - cy_p) / ry);
    let mut dtheta = angle_between(
        (x1_p - cx_p) / rx,
        (y1_p - cy_p) / ry,
        (-x1_p - cx_p) / rx,
        (-y1_p - cy_p) / ry,
    );

    if !sweep_flag && dtheta > 0.0 {
        dtheta -= 2.0 * PI;
    } else if sweep_flag && dtheta < 0.0 {
        dtheta += 2.0 * PI;
    }

    // Split into segments of at most PI/2
    let num_segments = (dtheta.abs() / (PI / 2.0)).ceil() as usize;
    let num_segments = num_segments.max(1);
    let seg_angle = dtheta / num_segments as f32;

    let mut current_theta = theta1;
    for _ in 0..num_segments {
        let next_theta = current_theta + seg_angle;
        let alpha = seg_angle / 2.0;
        let t = (4.0 / 3.0) * (alpha / 2.0).tan();

        let cos_curr = current_theta.cos();
        let sin_curr = current_theta.sin();
        let cos_next = next_theta.cos();
        let sin_next = next_theta.sin();

        // Control points on unit circle
        let p1x = cos_curr - t * sin_curr;
        let p1y = sin_curr + t * cos_curr;
        let p2x = cos_next + t * sin_next;
        let p2y = sin_next - t * cos_next;

        // Transform to ellipse and rotate
        let cp1_x = cos_phi * (rx * p1x) - sin_phi * (ry * p1y) + cx;
        let cp1_y = sin_phi * (rx * p1x) + cos_phi * (ry * p1y) + cy;
        let cp2_x = cos_phi * (rx * p2x) - sin_phi * (ry * p2y) + cx;
        let cp2_y = sin_phi * (rx * p2x) + cos_phi * (ry * p2y) + cy;
        let end_x = cos_phi * (rx * cos_next) - sin_phi * (ry * sin_next) + cx;
        let end_y = sin_phi * (rx * cos_next) + cos_phi * (ry * sin_next) + cy;

        pb.cubic_to(cp1_x, cp1_y, cp2_x, cp2_y, end_x, end_y);
        current_theta = next_theta;
    }
}

fn angle_between(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
    let dot = ux * vx + uy * vy;
    let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
    let cos_val = (dot / len.max(1e-6)).clamp(-1.0, 1.0);
    let sign = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 };
    sign * cos_val.acos()
}

struct PathTokenizer<'a> {
    chars: &'a [u8],
    idx: usize,
}

impl<'a> PathTokenizer<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.as_bytes(),
            idx: 0,
        }
    }

    fn skip_whitespace_and_commas(&mut self) {
        while self.idx < self.chars.len() {
            let c = self.chars[self.idx];
            if c.is_ascii_whitespace() || c == b',' {
                self.idx += 1;
            } else {
                break;
            }
        }
    }

    fn next_cmd(&mut self) -> Option<char> {
        self.skip_whitespace_and_commas();
        if self.idx >= self.chars.len() {
            return None;
        }
        let c = self.chars[self.idx] as char;
        if c.is_ascii_alphabetic() {
            self.idx += 1;
            Some(c)
        } else {
            None
        }
    }

    fn next_f32(&mut self) -> Option<f32> {
        self.skip_whitespace_and_commas();
        if self.idx >= self.chars.len() {
            return None;
        }

        let start = self.idx;
        let mut has_digit = false;
        let mut has_dot = false;

        if self.chars[self.idx] == b'+' || self.chars[self.idx] == b'-' {
            self.idx += 1;
        }

        while self.idx < self.chars.len() {
            let c = self.chars[self.idx];
            if c.is_ascii_digit() {
                has_digit = true;
                self.idx += 1;
            } else if c == b'.' && !has_dot {
                has_dot = true;
                self.idx += 1;
            } else if (c == b'e' || c == b'E') && has_digit {
                self.idx += 1;
                if self.idx < self.chars.len()
                    && (self.chars[self.idx] == b'+' || self.chars[self.idx] == b'-')
                {
                    self.idx += 1;
                }
            } else {
                break;
            }
        }

        if !has_digit {
            self.idx = start;
            return None;
        }

        let slice = std::str::from_utf8(&self.chars[start..self.idx]).ok()?;
        slice.parse::<f32>().ok()
    }

    fn next_flag(&mut self) -> Option<u8> {
        self.skip_whitespace_and_commas();
        if self.idx < self.chars.len() {
            let c = self.chars[self.idx];
            if c == b'0' || c == b'1' {
                self.idx += 1;
                return Some(c - b'0');
            }
        }
        None
    }
}
