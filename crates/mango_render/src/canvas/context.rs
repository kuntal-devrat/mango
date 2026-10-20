//! Canvas 2D context backing store, state stack, and transformations.

use mango_core::Color;
use tiny_skia::{Color as SkiaColor, LineCap, LineJoin, PathBuilder, Pixmap, Transform};

use super::state::{CanvasState, parse_canvas_font};

/// Backing store for an HTML5 `<canvas>` 2D rendering context.
pub struct Canvas2D {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixmap: Pixmap,
    pub(crate) state: CanvasState,
    pub(crate) state_stack: Vec<CanvasState>,
    pub(crate) current_path: PathBuilder,
    pub(crate) has_subpath: bool,
    pub(crate) current_point: Option<(f32, f32)>,
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
}
