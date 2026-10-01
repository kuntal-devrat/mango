//! HTML5 Canvas 2D backing store and vector rasterization engine.
//!
//! Provides a full-featured, hardware-accelerated/software-rasterized 2D graphics
//! engine powered by `tiny-skia` and `fontdue`.
//!
//! Implements the WHATWG HTML5 `<canvas>` specification:
//! - Full transformation matrix stack (`scale`, `rotate`, `translate`, `transform`, `setTransform`, `resetTransform`)
//! - Canvas state stack (`save`, `restore`)
//! - Path drawing (`beginPath`, `closePath`, `moveTo`, `lineTo`, `rect`, `arc`, `arcTo`, `bezierCurveTo`, `quadraticCurveTo`)
//! - Shape filling and stroking with anti-aliasing, line widths, caps, joins, and miter limits
//! - Direct rectangle operations (`fillRect`, `strokeRect`, `clearRect`)
//! - Text rendering and metrics with font selection, alignment, baseline, and glyph rasterization (`fillText`, `strokeText`, `measureText`)
//! - Direct pixel manipulation (`getImageData`, `putImageData`)
//! - Snapshot export to base64 data URLs (`toDataURL`)
//! - Thread-safe cross-system canvas registry (`CANVAS_REGISTRY`) for DOM and layout blitting.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{OnceLock, RwLock};

use mango_core::Color;
use tiny_skia::{
    BlendMode, Color as SkiaColor, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect,
    Stroke, Transform,
};

use crate::font::{font_manager, FontFamily, FontStyle, FontWeight};
use crate::image_decode::base64_encode;

/// Drawing state saved and restored on the canvas state stack.
#[derive(Debug, Clone)]
pub struct CanvasState {
    pub transform: Transform,
    pub fill_color: Color,
    pub stroke_color: Color,
    pub line_width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub global_alpha: f32,
    pub font_size: f32,
    pub font_family: FontFamily,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_str: String,
    pub text_align: String,    // "left", "right", "center", "start", "end"
    pub text_baseline: String, // "top", "hanging", "middle", "alphabetic", "ideographic", "bottom"
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            transform: Transform::identity(),
            fill_color: Color::BLACK,
            stroke_color: Color::BLACK,
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            global_alpha: 1.0,
            font_size: 10.0,
            font_family: FontFamily::SansSerif,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            font_str: "10px sans-serif".to_string(),
            text_align: "start".to_string(),
            text_baseline: "alphabetic".to_string(),
        }
    }
}

/// Backing store for an HTML5 `<canvas>` 2D rendering context.
pub struct Canvas2D {
    width: u32,
    height: u32,
    pixmap: Pixmap,
    state: CanvasState,
    state_stack: Vec<CanvasState>,
    current_path: PathBuilder,
    has_subpath: bool,
    current_point: Option<(f32, f32)>,
}

impl Canvas2D {
    /// Creates a new Canvas 2D engine with the given pixel dimensions.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let w = width.max(1);
        let h = height.max(1);
        let mut pixmap = Pixmap::new(w, h)?;
        pixmap.fill(SkiaColor::TRANSPARENT);
        Some(Self {
            width: w,
            height: h,
            pixmap,
            state: CanvasState::default(),
            state_stack: Vec::new(),
            current_path: PathBuilder::new(),
            has_subpath: false,
            current_point: None,
        })
    }

    /// Returns the canvas buffer width in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the canvas buffer height in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Resizes the canvas buffer and resets state (per HTML5 canvas spec).
    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if let Some(mut new_pm) = Pixmap::new(w, h) {
            new_pm.fill(SkiaColor::TRANSPARENT);
            self.pixmap = new_pm;
            self.width = w;
            self.height = h;
            self.state = CanvasState::default();
            self.state_stack.clear();
            self.current_path = PathBuilder::new();
            self.has_subpath = false;
        }
    }

    // --- State Stack ---

    /// Pushes the current canvas state onto the stack.
    pub fn save(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    /// Restores the most recently saved canvas state from the stack.
    pub fn restore(&mut self) {
        if let Some(saved) = self.state_stack.pop() {
            self.state = saved;
        }
    }

    // --- Transformations ---

    /// Scales the current transformation matrix.
    pub fn scale(&mut self, sx: f32, sy: f32) {
        if sx.is_finite() && sy.is_finite() {
            self.state.transform = self.state.transform.pre_scale(sx, sy);
        }
    }

    /// Rotates the current transformation matrix (angle in radians).
    pub fn rotate(&mut self, angle_rad: f32) {
        if angle_rad.is_finite() {
            self.state.transform = self.state.transform.pre_rotate(angle_rad.to_degrees());
        }
    }

    /// Translates the current transformation matrix.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if dx.is_finite() && dy.is_finite() {
            self.state.transform = self.state.transform.pre_translate(dx, dy);
        }
    }

    /// Multiplies the current transformation matrix with an arbitrary affine matrix:
    /// `[a c e]`
    /// `[b d f]`
    /// `[0 0 1]`
    pub fn transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        let other = Transform::from_row(a, b, c, d, e, f);
        self.state.transform = self.state.transform.pre_concat(other);
    }

    /// Resets the current transformation matrix to the specified matrix.
    pub fn set_transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        self.state.transform = Transform::from_row(a, b, c, d, e, f);
    }

    /// Resets the current transformation matrix to identity.
    pub fn reset_transform(&mut self) {
        self.state.transform = Transform::identity();
    }

    // --- Style Properties ---

    pub fn fill_style(&self) -> Color {
        self.state.fill_color
    }

    pub fn set_fill_style(&mut self, color: Color) {
        self.state.fill_color = color;
    }

    pub fn stroke_style(&self) -> Color {
        self.state.stroke_color
    }

    pub fn current_point(&self) -> Option<(f32, f32)> {
        self.current_point
    }

    pub fn set_stroke_style(&mut self, color: Color) {
        self.state.stroke_color = color;
    }

    pub fn line_width(&self) -> f32 {
        self.state.line_width
    }

    pub fn set_line_width(&mut self, width: f32) {
        if width > 0.0 && width.is_finite() {
            self.state.line_width = width;
        }
    }

    pub fn line_cap(&self) -> &str {
        match self.state.line_cap {
            LineCap::Round => "round",
            LineCap::Square => "square",
            LineCap::Butt => "butt",
        }
    }

    pub fn set_line_cap(&mut self, cap: &str) {
        match cap.trim().to_ascii_lowercase().as_str() {
            "round" => self.state.line_cap = LineCap::Round,
            "square" => self.state.line_cap = LineCap::Square,
            _ => self.state.line_cap = LineCap::Butt,
        }
    }

    pub fn line_join(&self) -> &str {
        match self.state.line_join {
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
            LineJoin::Miter | LineJoin::MiterClip => "miter",
        }
    }

    pub fn set_line_join(&mut self, join: &str) {
        match join.trim().to_ascii_lowercase().as_str() {
            "round" => self.state.line_join = LineJoin::Round,
            "bevel" => self.state.line_join = LineJoin::Bevel,
            _ => self.state.line_join = LineJoin::Miter,
        }
    }

    pub fn miter_limit(&self) -> f32 {
        self.state.miter_limit
    }

    pub fn set_miter_limit(&mut self, limit: f32) {
        if limit > 0.0 && limit.is_finite() {
            self.state.miter_limit = limit;
        }
    }

    pub fn global_alpha(&self) -> f32 {
        self.state.global_alpha
    }

    pub fn set_global_alpha(&mut self, alpha: f32) {
        if (0.0..=1.0).contains(&alpha) && alpha.is_finite() {
            self.state.global_alpha = alpha;
        }
    }

    pub fn font(&self) -> &str {
        &self.state.font_str
    }

    pub fn set_font(&mut self, font_str: &str) {
        self.state.font_str = font_str.to_string();
        parse_canvas_font(font_str, &mut self.state);
    }

    pub fn text_align(&self) -> &str {
        &self.state.text_align
    }

    pub fn set_text_align(&mut self, align: &str) {
        self.state.text_align = align.trim().to_ascii_lowercase();
    }

    pub fn text_baseline(&self) -> &str {
        &self.state.text_baseline
    }

    pub fn set_text_baseline(&mut self, baseline: &str) {
        self.state.text_baseline = baseline.trim().to_ascii_lowercase();
    }

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
        if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
            if let Some(r) = Rect::from_xywh(x, y, w, h) {
                self.current_path.push_rect(r);
                self.has_subpath = true;
                self.current_point = Some((x, y));
            }
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
    pub fn bezier_curve_to(
        &mut self,
        cp1x: f32,
        cp1y: f32,
        cp2x: f32,
        cp2y: f32,
        x: f32,
        y: f32,
    ) {
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
                    self.pixmap
                        .fill_path(&path, &paint, FillRule::Winding, self.state.transform, None);
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
                    self.pixmap.fill_rect(rect, &paint, self.state.transform, None);
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
            self.pixmap.fill_rect(rect, &paint, self.state.transform, None);
        }
    }

    // --- Text Rendering ---

    /// Measures the advance width of a text string using the current canvas font.
    pub fn measure_text(&self, text: &str) -> f32 {
        let fm = font_manager();
        let (w, _) = fm.measure_text(
            text,
            self.state.font_size,
            self.state.font_weight,
            self.state.font_family,
        );
        w
    }

    /// Draws filled text at `(x, y)` with optional maximum width constraint.
    pub fn fill_text(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        if text.is_empty() {
            return;
        }
        let fm = font_manager();
        let (orig_w, text_h) = fm.measure_text(
            text,
            self.state.font_size,
            self.state.font_weight,
            self.state.font_family,
        );

        let scale_x = if let Some(mw) = max_width {
            if mw > 0.0 && orig_w > mw {
                mw / orig_w
            } else {
                1.0
            }
        } else {
            1.0
        };

        let total_w = orig_w * scale_x;

        let x_offset = match self.state.text_align.as_str() {
            "center" => -total_w / 2.0,
            "right" | "end" => -total_w,
            _ => 0.0,
        };

        let font = fm.select_font(self.state.font_family, self.state.font_weight);
        let px_size = self.state.font_size.max(1.0);
        let ascent = font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let y_offset = match self.state.text_baseline.as_str() {
            "top" | "hanging" => ascent,
            "middle" => ascent - text_h / 2.0,
            "bottom" | "ideographic" => ascent - text_h,
            _ => 0.0, // "alphabetic": (x, y) is at font baseline
        };

        let base_x = x + x_offset;
        let base_y = y + y_offset;

        let alpha = (self.state.fill_color.a as f32 / 255.0) * self.state.global_alpha;
        if alpha <= 0.0 {
            return;
        }

        let mut cursor_x = base_x;
        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }
            let (metrics, bitmap) = font.rasterize(ch, px_size);
            if metrics.width > 0 && metrics.height > 0 && !bitmap.is_empty() {
                let glyph_x = cursor_x + metrics.xmin as f32 * scale_x;
                let glyph_y = base_y - metrics.height as f32 - metrics.ymin as f32;

                self.blend_glyph_bitmap(
                    &bitmap,
                    metrics.width as u32,
                    metrics.height as u32,
                    glyph_x,
                    glyph_y,
                    scale_x,
                    self.state.fill_color,
                    alpha,
                );
            }
            cursor_x += metrics.advance_width * scale_x;
        }
    }

    /// Draws stroked text at `(x, y)` with optional maximum width constraint.
    pub fn stroke_text(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        if text.is_empty() {
            return;
        }
        // Save current fill color, render text using stroke_color as fill, restore
        let old_fill = self.state.fill_color;
        self.state.fill_color = self.state.stroke_color;
        self.fill_text(text, x, y, max_width);
        self.state.fill_color = old_fill;
    }

    #[allow(clippy::too_many_arguments)]
    fn blend_glyph_bitmap(
        &mut self,
        bitmap: &[u8],
        glyph_w: u32,
        glyph_h: u32,
        gx: f32,
        gy: f32,
        scale_x: f32,
        color: Color,
        base_alpha: f32,
    ) {
        let tf = self.state.transform;
        let pm_w = self.width as i32;
        let pm_h = self.height as i32;
        let data = self.pixmap.data_mut();

        for row in 0..glyph_h {
            for col in 0..glyph_w {
                let cov = bitmap[(row * glyph_w + col) as usize] as f32 / 255.0;
                if cov <= 0.01 {
                    continue;
                }
                let local_x = gx + col as f32 * scale_x;
                let local_y = gy + row as f32;

                // Transform point into pixmap space
                let px = (tf.sx * local_x + tf.kx * local_y + tf.tx).round() as i32;
                let py = (tf.ky * local_x + tf.sy * local_y + tf.ty).round() as i32;

                if px >= 0 && px < pm_w && py >= 0 && py < pm_h {
                    let eff_alpha = base_alpha * cov;
                    let idx = ((py * pm_w + px) * 4) as usize;
                    let dst_r = data[idx] as f32 / 255.0;
                    let dst_g = data[idx + 1] as f32 / 255.0;
                    let dst_b = data[idx + 2] as f32 / 255.0;
                    let dst_a = data[idx + 3] as f32 / 255.0;

                    let src_r = color.r as f32 / 255.0;
                    let src_g = color.g as f32 / 255.0;
                    let src_b = color.b as f32 / 255.0;

                    // Premultiplied alpha composite (Over operator)
                    let out_a = eff_alpha + dst_a * (1.0 - eff_alpha);
                    if out_a > 0.0 {
                        let out_r = (src_r * eff_alpha + dst_r * (1.0 - eff_alpha)).min(out_a);
                        let out_g = (src_g * eff_alpha + dst_g * (1.0 - eff_alpha)).min(out_a);
                        let out_b = (src_b * eff_alpha + dst_b * (1.0 - eff_alpha)).min(out_a);

                        data[idx] = (out_r * 255.0).round() as u8;
                        data[idx + 1] = (out_g * 255.0).round() as u8;
                        data[idx + 2] = (out_b * 255.0).round() as u8;
                        data[idx + 3] = (out_a * 255.0).round() as u8;
                    }
                }
            }
        }
    }

    // --- Pixel Manipulation ---

    /// Extracts unpremultiplied RGBA byte array of the specified rectangle.
    pub fn get_image_data(&self, sx: i32, sy: i32, sw: u32, sh: u32) -> Vec<u8> {
        let mut out = vec![0u8; (sw * sh * 4) as usize];
        let pixmap_data = self.pixmap.data();
        let w = self.width as i32;
        let h = self.height as i32;

        for y in 0..sh as i32 {
            let src_y = sy + y;
            if src_y < 0 || src_y >= h {
                continue;
            }
            for x in 0..sw as i32 {
                let src_x = sx + x;
                if src_x < 0 || src_x >= w {
                    continue;
                }
                let src_idx = ((src_y * w + src_x) * 4) as usize;
                let dst_idx = ((y * sw as i32 + x) * 4) as usize;
                let a = pixmap_data[src_idx + 3];
                if a == 0 {
                    out[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    out[dst_idx..dst_idx + 4].copy_from_slice(&pixmap_data[src_idx..src_idx + 4]);
                } else {
                    let a_f = a as f32 / 255.0;
                    let r = ((pixmap_data[src_idx] as f32 / a_f).min(255.0).round()) as u8;
                    let g = ((pixmap_data[src_idx + 1] as f32 / a_f).min(255.0).round()) as u8;
                    let b = ((pixmap_data[src_idx + 2] as f32 / a_f).min(255.0).round()) as u8;
                    out[dst_idx] = r;
                    out[dst_idx + 1] = g;
                    out[dst_idx + 2] = b;
                    out[dst_idx + 3] = a;
                }
            }
        }
        out
    }

    /// Writes raw RGBA byte data into the canvas buffer at `(dx, dy)`.
    pub fn put_image_data(
        &mut self,
        data: &[u8],
        dx: i32,
        dy: i32,
        dirty_x: i32,
        dirty_y: i32,
        dirty_w: u32,
        dirty_h: u32,
    ) {
        self.put_image_data_with_stride(data, 0, dx, dy, dirty_x, dirty_y, dirty_w, dirty_h);
    }

    /// Writes raw RGBA byte data with explicit source image width into the canvas buffer at `(dx, dy)`.
    pub fn put_image_data_with_stride(
        &mut self,
        data: &[u8],
        src_width: u32,
        dx: i32,
        dy: i32,
        dirty_x: i32,
        dirty_y: i32,
        dirty_w: u32,
        dirty_h: u32,
    ) {
        let pm_data = self.pixmap.data_mut();
        let w = self.width as i32;
        let h = self.height as i32;

        let total_pixels = (data.len() / 4) as i32;
        let src_w = if src_width > 0 {
            src_width as i32
        } else if total_pixels == (dirty_w * dirty_h) as i32 {
            dirty_w as i32
        } else if dirty_w > 0 && total_pixels > (dirty_w * dirty_h) as i32 {
            (total_pixels / (dirty_y + dirty_h as i32).max(1)).max(dirty_w as i32)
        } else {
            dirty_w as i32
        };

        for y in dirty_y..(dirty_y + dirty_h as i32) {
            let target_y = dy + y;
            if target_y < 0 || target_y >= h {
                continue;
            }
            for x in dirty_x..(dirty_x + dirty_w as i32) {
                let target_x = dx + x;
                if target_x < 0 || target_x >= w {
                    continue;
                }
                let src_idx = ((y * src_w + x) * 4) as usize;
                if src_idx + 4 > data.len() {
                    continue;
                }
                let r = data[src_idx];
                let g = data[src_idx + 1];
                let b = data[src_idx + 2];
                let a = data[src_idx + 3];

                let dst_idx = ((target_y * w + target_x) * 4) as usize;
                if a == 0 {
                    pm_data[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    pm_data[dst_idx] = r;
                    pm_data[dst_idx + 1] = g;
                    pm_data[dst_idx + 2] = b;
                    pm_data[dst_idx + 3] = 255;
                } else {
                    let a_f = a as f32 / 255.0;
                    pm_data[dst_idx] = ((r as f32 * a_f).round()) as u8;
                    pm_data[dst_idx + 1] = ((g as f32 * a_f).round()) as u8;
                    pm_data[dst_idx + 2] = ((b as f32 * a_f).round()) as u8;
                    pm_data[dst_idx + 3] = a;
                }
            }
        }
    }

    /// Encodes the current canvas bitmap as a PNG base64 `data:` URI.
    pub fn to_data_url(&self, _mime_type: &str) -> String {
        let raw_rgba = self.get_image_data(0, 0, self.width, self.height);
        if let Some(img) = image::RgbaImage::from_raw(self.width, self.height, raw_rgba) {
            let mut png_bytes = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
            if img.write_with_encoder(encoder).is_ok() {
                let b64 = base64_encode(&png_bytes);
                return format!("data:image/png;base64,{b64}");
            }
        }
        "data:image/png;base64,".to_string()
    }

    /// Converts the current canvas pixmap to row-major `0xAARRGGBB` pixels for framebuffer blitting.
    pub fn get_pixels(&self) -> Vec<u32> {
        let mut pixels = Vec::with_capacity((self.width * self.height) as usize);
        for pixel in self.pixmap.pixels() {
            let a = pixel.alpha();
            if a == 0 {
                pixels.push(0);
            } else if a == 255 {
                pixels.push(
                    0xFF00_0000
                        | ((pixel.red() as u32) << 16)
                        | ((pixel.green() as u32) << 8)
                        | (pixel.blue() as u32),
                );
            } else {
                let a_f = a as f32 / 255.0;
                let r = ((pixel.red() as f32 / a_f).min(255.0).round()) as u32;
                let g = ((pixel.green() as f32 / a_f).min(255.0).round()) as u32;
                let b = ((pixel.blue() as f32 / a_f).min(255.0).round()) as u32;
                pixels.push(((a as u32) << 24) | (r << 16) | (g << 8) | b);
            }
        }
        pixels
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn add_arc_to_path(
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

fn parse_canvas_font(font_str: &str, state: &mut CanvasState) {
    let lower = font_str.to_ascii_lowercase();
    let mut words = lower.split_whitespace().peekable();

    while let Some(word) = words.peek() {
        match *word {
            "italic" | "oblique" => {
                state.font_style = FontStyle::Italic;
                words.next();
            }
            "bold" | "bolder" | "700" | "800" | "900" => {
                state.font_weight = FontWeight::Bold;
                words.next();
            }
            "normal" => {
                state.font_weight = FontWeight::Regular;
                state.font_style = FontStyle::Normal;
                words.next();
            }
            w if w.ends_with("px") => {
                if let Ok(sz) = w.trim_end_matches("px").parse::<f32>() {
                    state.font_size = sz.max(1.0);
                }
                words.next();
                break;
            }
            w if w.parse::<f32>().is_ok() => {
                if let Ok(sz) = w.parse::<f32>() {
                    state.font_size = sz.max(1.0);
                }
                words.next();
                break;
            }
            _ => {
                words.next();
            }
        }
    }

    let remainder: Vec<&str> = words.collect();
    if !remainder.is_empty() {
        let fam_str = remainder.join(" ");
        state.font_family = font_manager().resolve_family_for(&fam_str);
    }
}

// ---------------------------------------------------------------------------
// Global Thread-Safe Canvas Registry
// ---------------------------------------------------------------------------

static CANVAS_REGISTRY: OnceLock<RwLock<HashMap<usize, Canvas2D>>> = OnceLock::new();

fn canvas_registry() -> &'static RwLock<HashMap<usize, Canvas2D>> {
    CANVAS_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Executes a closure with mutable access to a canvas, creating it if it doesn't exist.
pub fn with_canvas_mut<F, R>(node_id: usize, width: u32, height: u32, f: F) -> Option<R>
where
    F: FnOnce(&mut Canvas2D) -> R,
{
    let mut reg = canvas_registry().write().ok()?;
    let canvas = reg.entry(node_id).or_insert_with(|| {
        Canvas2D::new(width, height).expect("Failed to initialize canvas pixmap")
    });
    Some(f(canvas))
}

/// Resizes a canvas in the registry, creating it with the new size if not present.
pub fn resize_canvas(node_id: usize, width: u32, height: u32) {
    if let Ok(mut reg) = canvas_registry().write() {
        if let Some(canvas) = reg.get_mut(&node_id) {
            canvas.resize(width, height);
        } else if let Some(canvas) = Canvas2D::new(width, height) {
            reg.insert(node_id, canvas);
        }
    }
}

/// Retrieves canvas pixel data in `0xAARRGGBB` format and dimensions for display list rendering.
pub fn get_canvas_pixels(node_id: usize) -> Option<(u32, u32, Vec<u32>)> {
    let reg = canvas_registry().read().ok()?;
    let canvas = reg.get(&node_id)?;
    Some((canvas.width, canvas.height, canvas.get_pixels()))
}

/// Retrieves canvas dimensions.
pub fn get_canvas_dimensions(node_id: usize) -> Option<(u32, u32)> {
    let reg = canvas_registry().read().ok()?;
    let canvas = reg.get(&node_id)?;
    Some((canvas.width, canvas.height))
}

/// Clears all canvases from the registry.
pub fn clear_canvas_registry() {
    if let Ok(mut reg) = canvas_registry().write() {
        reg.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_creation_and_fill_rect() {
        let mut canvas = Canvas2D::new(100, 100).expect("Canvas should create");
        canvas.set_fill_style(Color::rgb(255, 0, 0));
        canvas.fill_rect(10.0, 10.0, 20.0, 20.0);

        let pixels = canvas.get_pixels();
        assert_eq!(pixels.len(), 100 * 100);

        // Pixel at (15, 15) should be opaque red
        let idx = 15 * 100 + 15;
        assert_eq!(pixels[idx], 0xFFFF_0000);

        // Pixel outside (0, 0) should be transparent
        assert_eq!(pixels[0], 0);
    }

    #[test]
    fn test_canvas_path_stroke_and_state_stack() {
        let mut canvas = Canvas2D::new(200, 200).expect("Canvas should create");
        canvas.save();
        canvas.set_line_width(4.0);
        canvas.set_stroke_style(Color::rgb(0, 255, 0));
        canvas.translate(50.0, 50.0);

        canvas.begin_path();
        canvas.move_to(0.0, 0.0);
        canvas.line_to(50.0, 0.0);
        canvas.stroke();

        canvas.restore();
        assert_eq!(canvas.line_width(), 1.0);
    }

    #[test]
    fn test_canvas_get_put_image_data_and_data_url() {
        let mut canvas = Canvas2D::new(50, 50).expect("Canvas should create");
        canvas.set_fill_style(Color::rgb(0, 0, 255));
        canvas.fill_rect(0.0, 0.0, 50.0, 50.0);

        let data = canvas.get_image_data(0, 0, 50, 50);
        assert_eq!(data.len(), 50 * 50 * 4);
        assert_eq!(data[0], 0);   // R
        assert_eq!(data[1], 0);   // G
        assert_eq!(data[2], 255); // B
        assert_eq!(data[3], 255); // A

        let url = canvas.to_data_url("image/png");
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(url.len() > 30);
    }

    #[test]
    fn test_canvas_text_measurement() {
        let canvas = Canvas2D::new(100, 100).expect("Canvas should create");
        let w = canvas.measure_text("Hello World");
        assert!(w > 0.0, "Measured text width must be positive");
    }

    #[test]
    fn test_canvas_arc_to() {
        let mut canvas = Canvas2D::new(100, 100).expect("Canvas should create");
        canvas.begin_path();
        canvas.move_to(10.0, 10.0);
        // Draw rounded corner via arc_to
        canvas.arc_to(50.0, 10.0, 50.0, 50.0, 10.0);
        canvas.line_to(50.0, 50.0);
        canvas.set_stroke_style(Color::rgb(255, 0, 0));
        canvas.stroke();

        assert_eq!(canvas.current_point, Some((50.0, 50.0)));
    }

    #[test]
    fn test_canvas_put_image_data_dirty_rect() {
        let mut canvas = Canvas2D::new(20, 20).expect("Canvas should create");
        // Create 10x10 red image data
        let mut img_data = vec![0u8; 10 * 10 * 4];
        for i in 0..100 {
            img_data[i * 4] = 255;     // R
            img_data[i * 4 + 3] = 255; // A
        }
        // Put only a dirty sub-rectangle of 4x4 from offset (2, 2)
        canvas.put_image_data_with_stride(&img_data, 10, 5, 5, 2, 2, 4, 4);

        let out = canvas.get_image_data(0, 0, 20, 20);
        // Canvas (5+2, 5+2) = (7, 7) should be red
        let idx = ((7 * 20 + 7) * 4) as usize;
        assert_eq!(out[idx], 255);
        assert_eq!(out[idx + 3], 255);

        // Canvas (0, 0) should still be transparent
        assert_eq!(out[0], 0);
        assert_eq!(out[3], 0);
    }
}
