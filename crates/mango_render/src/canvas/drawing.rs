//! Canvas 2D path construction, shape rasterization, and direct rect operations.

use std::f32::consts::PI;
use tiny_skia::{BlendMode, Color as SkiaColor, FillRule, Paint, PathBuilder, Rect, Stroke};

use super::context::Canvas2D;

impl Canvas2D {
    // --- Path Operations ---

    /// Clears the current path and starts a new one.
    pub fn begin_path(&mut self) {
        self.current_path = PathBuilder::new();
        self.has_subpath = false;
        self.current_point = None;
    }

    /// Closes the current subpath with a straight line back to the start.
    pub fn close_path(&mut self) {
        if self.has_subpath {
            self.current_path.close();
        }
    }

    /// Moves the subpath to the given coordinates.
    pub fn move_to(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.current_path.move_to(x, y);
            self.has_subpath = true;
            self.current_point = Some((x, y));
        }
    }

    /// Adds a straight line to the given coordinates.
    pub fn line_to(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            if !self.has_subpath {
                self.current_path.move_to(x, y);
                self.has_subpath = true;
            } else {
                self.current_path.line_to(x, y);
            }
            self.current_point = Some((x, y));
        }
    }

    /// Adds a rectangle subpath.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w.is_finite()
            && h.is_finite()
            && w > 0.0
            && h > 0.0
            && let Some(r) = Rect::from_xywh(x, y, w, h)
        {
            self.current_path.push_rect(r);
            self.has_subpath = true;
            self.current_point = Some((x, y));
        }
    }

    /// Adds an arc/circular curve to the current path.
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        counterclockwise: bool,
    ) {
        if r <= 0.0 || !cx.is_finite() || !cy.is_finite() || !r.is_finite() {
            return;
        }
        add_arc_to_path(
            &mut self.current_path,
            &mut self.has_subpath,
            cx,
            cy,
            r,
            start_angle,
            end_angle,
            counterclockwise,
        );
        self.current_point = Some((cx + r * end_angle.cos(), cy + r * end_angle.sin()));
    }

    /// Adds an arc with tangent control points.
    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        if radius <= 0.0 || !self.has_subpath {
            return;
        }
        let Some((x0, y0)) = self.current_point else {
            self.move_to(x1, y1);
            return;
        };

        let v1_x = x0 - x1;
        let v1_y = y0 - y1;
        let v2_x = x2 - x1;
        let v2_y = y2 - y1;

        let len1 = (v1_x * v1_x + v1_y * v1_y).sqrt();
        let len2 = (v2_x * v2_x + v2_y * v2_y).sqrt();

        if len1 == 0.0 || len2 == 0.0 {
            self.line_to(x1, y1);
            return;
        }

        let u1_x = v1_x / len1;
        let u1_y = v1_y / len1;
        let u2_x = v2_x / len2;
        let u2_y = v2_y / len2;

        let cos_theta = (u1_x * u2_x + u1_y * u2_y).clamp(-1.0, 1.0);
        if (cos_theta - 1.0).abs() < 1e-6 || (cos_theta + 1.0).abs() < 1e-6 {
            self.line_to(x1, y1);
            return;
        }

        let theta = cos_theta.acos();
        let half_theta = theta / 2.0;
        let tan_half = half_theta.tan();
        if tan_half.abs() < 1e-6 {
            self.line_to(x1, y1);
            return;
        }

        let d = radius / tan_half;
        let t1_x = x1 + d * u1_x;
        let t1_y = y1 + d * u1_y;
        let t2_x = x1 + d * u2_x;
        let t2_y = y1 + d * u2_y;

        self.line_to(t1_x, t1_y);

        let sin_half = half_theta.sin();
        let h = radius / sin_half;
        let mut b_x = u1_x + u2_x;
        let mut b_y = u1_y + u2_y;
        let b_len = (b_x * b_x + b_y * b_y).sqrt();
        if b_len > 0.0 {
            b_x /= b_len;
            b_y /= b_len;
        }
        let cx = x1 + h * b_x;
        let cy = y1 + h * b_y;

        let phi1 = (t1_y - cy).atan2(t1_x - cx);
        let phi2 = (t2_y - cy).atan2(t2_x - cx);

        let cross = u1_x * u2_y - u1_y * u2_x;
        let counterclockwise = cross > 0.0;

        add_arc_to_path(
            &mut self.current_path,
            &mut self.has_subpath,
            cx,
            cy,
            radius,
            phi1,
            phi2,
            counterclockwise,
        );
        self.current_point = Some((t2_x, t2_y));
    }

    /// Adds a cubic Bézier curve to the path.
    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        if !self.has_subpath {
            self.current_path.move_to(cp1x, cp1y);
            self.has_subpath = true;
        }
        self.current_path.cubic_to(cp1x, cp1y, cp2x, cp2y, x, y);
        self.current_point = Some((x, y));
    }

    /// Adds a quadratic Bézier curve to the path.
    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        if !self.has_subpath {
            self.current_path.move_to(cpx, cpy);
            self.has_subpath = true;
        }
        self.current_path.quad_to(cpx, cpy, x, y);
        self.current_point = Some((x, y));
    }

    // --- Fill & Stroke ---

    /// Fills the current path with the current fillStyle and globalAlpha.
    pub fn fill(&mut self) {
        let mut builder = PathBuilder::new();
        std::mem::swap(&mut self.current_path, &mut builder);
        if let Some(path) = builder.clone().finish() {
            let alpha = (self.state.fill_color.a as f32 / 255.0) * self.state.global_alpha;
            if alpha > 0.0 {
                let mut paint = Paint::default();
                if let Some(sc) = SkiaColor::from_rgba(
                    self.state.fill_color.r as f32 / 255.0,
                    self.state.fill_color.g as f32 / 255.0,
                    self.state.fill_color.b as f32 / 255.0,
                    alpha,
                ) {
                    paint.set_color(sc);
                    paint.anti_alias = true;
                    self.pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        self.state.transform,
                        None,
                    );
                }
            }
        }
        self.current_path = builder;
    }

    /// Strokes the current path with the current strokeStyle, lineWidth, and stroke caps/joins.
    pub fn stroke(&mut self) {
        let mut builder = PathBuilder::new();
        std::mem::swap(&mut self.current_path, &mut builder);
        if let Some(path) = builder.clone().finish() {
            let alpha = (self.state.stroke_color.a as f32 / 255.0) * self.state.global_alpha;
            if alpha > 0.0 && self.state.line_width > 0.0 {
                let mut paint = Paint::default();
                if let Some(sc) = SkiaColor::from_rgba(
                    self.state.stroke_color.r as f32 / 255.0,
                    self.state.stroke_color.g as f32 / 255.0,
                    self.state.stroke_color.b as f32 / 255.0,
                    alpha,
                ) {
                    paint.set_color(sc);
                    paint.anti_alias = true;
                    let stroke = Stroke {
                        width: self.state.line_width,
                        line_cap: self.state.line_cap,
                        line_join: self.state.line_join,
                        miter_limit: self.state.miter_limit,
                        ..Default::default()
                    };
                    self.pixmap
                        .stroke_path(&path, &paint, &stroke, self.state.transform, None);
                }
            }
        }
        self.current_path = builder;
    }

    // --- Direct Rect Operations ---

    /// Draws a filled rectangle without modifying the current path.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w <= 0.0 || h <= 0.0 || !w.is_finite() || !h.is_finite() {
            return;
        }
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let alpha = (self.state.fill_color.a as f32 / 255.0) * self.state.global_alpha;
            if alpha > 0.0 {
                let mut paint = Paint::default();
                if let Some(sc) = SkiaColor::from_rgba(
                    self.state.fill_color.r as f32 / 255.0,
                    self.state.fill_color.g as f32 / 255.0,
                    self.state.fill_color.b as f32 / 255.0,
                    alpha,
                ) {
                    paint.set_color(sc);
                    paint.anti_alias = true;
                    self.pixmap
                        .fill_rect(rect, &paint, self.state.transform, None);
                }
            }
        }
    }

    /// Strokes a rectangle without modifying the current path.
    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w <= 0.0 || h <= 0.0 || !w.is_finite() || !h.is_finite() {
            return;
        }
        let mut pb = PathBuilder::new();
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                let alpha = (self.state.stroke_color.a as f32 / 255.0) * self.state.global_alpha;
                if alpha > 0.0 && self.state.line_width > 0.0 {
                    let mut paint = Paint::default();
                    if let Some(sc) = SkiaColor::from_rgba(
                        self.state.stroke_color.r as f32 / 255.0,
                        self.state.stroke_color.g as f32 / 255.0,
                        self.state.stroke_color.b as f32 / 255.0,
                        alpha,
                    ) {
                        paint.set_color(sc);
                        paint.anti_alias = true;
                        let stroke = Stroke {
                            width: self.state.line_width,
                            line_cap: self.state.line_cap,
                            line_join: self.state.line_join,
                            miter_limit: self.state.miter_limit,
                            ..Default::default()
                        };
                        self.pixmap
                            .stroke_path(&path, &paint, &stroke, self.state.transform, None);
                    }
                }
            }
        }
    }

    /// Clears the pixels in a rectangle to transparent black, respecting transformations.
    pub fn clear_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w <= 0.0 || h <= 0.0 || !w.is_finite() || !h.is_finite() {
            return;
        }
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.blend_mode = BlendMode::Clear;
            self.pixmap
                .fill_rect(rect, &paint, self.state.transform, None);
        }
    }
}

pub(crate) fn add_arc_to_path(
    pb: &mut PathBuilder,
    has_subpath: &mut bool,
    cx: f32,
    cy: f32,
    r: f32,
    start_angle: f32,
    end_angle: f32,
    counterclockwise: bool,
) {
    let two_pi = 2.0 * PI;
    let mut s = start_angle % two_pi;
    if s < 0.0 {
        s += two_pi;
    }
    let mut e = end_angle % two_pi;
    if e < 0.0 {
        e += two_pi;
    }

    let sweep = if !counterclockwise {
        if end_angle - start_angle >= two_pi {
            two_pi
        } else {
            let mut diff = e - s;
            if diff < 0.0 {
                diff += two_pi;
            }
            diff
        }
    } else if start_angle - end_angle >= two_pi {
        -two_pi
    } else {
        let mut diff = e - s;
        if diff > 0.0 {
            diff -= two_pi;
        }
        diff
    };

    let start_x = cx + r * s.cos();
    let start_y = cy + r * s.sin();
    if !*has_subpath {
        pb.move_to(start_x, start_y);
        *has_subpath = true;
    } else {
        pb.line_to(start_x, start_y);
    }

    let num_segments = ((sweep.abs() / (PI / 2.0)).ceil() as usize).max(1);
    let step = sweep / num_segments as f32;

    let mut cur_angle = s;
    for _ in 0..num_segments {
        let next_angle = cur_angle + step;
        let half_step = step / 2.0;
        let k = (4.0 / 3.0) * (half_step / 2.0).tan();

        let p0_x = cx + r * cur_angle.cos();
        let p0_y = cy + r * cur_angle.sin();
        let p3_x = cx + r * next_angle.cos();
        let p3_y = cy + r * next_angle.sin();

        let cp1_x = p0_x - k * r * cur_angle.sin();
        let cp1_y = p0_y + k * r * cur_angle.cos();
        let cp2_x = p3_x + k * r * next_angle.sin();
        let cp2_y = p3_y - k * r * next_angle.cos();

        pb.cubic_to(cp1_x, cp1_y, cp2_x, cp2_y, p3_x, p3_y);
        cur_angle = next_angle;
    }
}
