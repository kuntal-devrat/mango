//! Canvas 2D text layout, measurement, and glyph rasterization blending.

use mango_core::Color;

use super::context::Canvas2D;
use crate::font::font_manager;

impl Canvas2D {
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
}
