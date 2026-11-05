//! Glyph rasterization, ClearType subpixel filtering, and clipping.

use mango_core::Color;

use super::emoji::{get_or_rasterize_color_emoji, is_emoji_char};
use super::manager::FontManager;
use super::types::{FontFamily, FontStyle, FontWeight};

fn blit_emoji_px(
    buffer: &mut [u32],
    buf_width: u32,
    emoji: &super::emoji::ColorEmojiBitmap,
    emoji_x: f32,
    emoji_y: f32,
    color_a: u8,
    clip_bounds: (i32, i32, i32, i32),
) {
    let (cx1, cy1, cx2, cy2) = clip_bounds;
    for ey in 0..emoji.height {
        for ex in 0..emoji.width {
            let src_px = emoji.pixels[(ey * emoji.width + ex) as usize];
            let sa = (src_px >> 24) & 0xFF;
            if sa == 0 {
                continue;
            }

            let px = (emoji_x + ex as f32) as i32;
            let py = (emoji_y + ey as f32) as i32;

            if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                continue;
            }

            let idx = (py as u32 * buf_width + px as u32) as usize;
            if idx >= buffer.len() {
                continue;
            }

            let eff_a = ((sa * color_a as u32) + 127) / 255;
            if eff_a == 255 {
                buffer[idx] = src_px & 0x00FFFFFF;
            } else if eff_a > 0 {
                let sr = (src_px >> 16) & 0xFF;
                let sg = (src_px >> 8) & 0xFF;
                let sb = src_px & 0xFF;

                let bg = buffer[idx];
                let bg_r = (bg >> 16) & 0xFF;
                let bg_g = (bg >> 8) & 0xFF;
                let bg_b = bg & 0xFF;

                let inv_a = 255 - eff_a;
                let r = ((sr * eff_a + bg_r * inv_a + 127) / 255).min(255);
                let g = ((sg * eff_a + bg_g * inv_a + 127) / 255).min(255);
                let b = ((sb * eff_a + bg_b * inv_a + 127) / 255).min(255);
                buffer[idx] = (r << 16) | (g << 8) | b;
            }
        }
    }
}

fn blit_glyph_bitmap(
    buffer: &mut [u32],
    buf_width: u32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
    glyph_x: f32,
    glyph_y: f32,
    is_italic: bool,
    color: Color,
    clip_bounds: (i32, i32, i32, i32),
) {
    let (cx1, cy1, cx2, cy2) = clip_bounds;
    for gy in 0..metrics.height {
        let slant = if is_italic {
            (metrics.height as f32 - gy as f32) * 0.22
        } else {
            0.0
        };

        for gx in 0..metrics.width {
            let coverage = bitmap[gy * metrics.width + gx];
            if coverage == 0 {
                continue;
            }

            let px = (glyph_x + gx as f32 + slant) as i32;
            let py = (glyph_y + gy as f32) as i32;

            if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                continue;
            }

            let idx = (py as u32 * buf_width + px as u32) as usize;
            if idx >= buffer.len() {
                continue;
            }

            let effective_alpha = ((coverage as u32 * color.a as u32) + 127) / 255;
            if effective_alpha == 0 {
                continue;
            }

            if effective_alpha == 255 {
                buffer[idx] = color.to_rgb_u32();
            } else {
                let inv_alpha = 255 - effective_alpha;
                let bg = buffer[idx];
                let bg_r = (bg >> 16) & 0xFF;
                let bg_g = (bg >> 8) & 0xFF;
                let bg_b = bg & 0xFF;
                let r =
                    ((color.r as u32 * effective_alpha + bg_r * inv_alpha + 127) / 255).min(255);
                let g =
                    ((color.g as u32 * effective_alpha + bg_g * inv_alpha + 127) / 255).min(255);
                let b =
                    ((color.b as u32 * effective_alpha + bg_b * inv_alpha + 127) / 255).min(255);
                buffer[idx] = (r << 16) | (g << 8) | b;
            }
        }
    }
}

fn blit_glyph_bitmap_subpixel(
    buffer: &mut [u32],
    buf_width: u32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
    glyph_x: f32,
    glyph_y: f32,
    is_italic: bool,
    color: Color,
    clip_bounds: (i32, i32, i32, i32),
) {
    let (cx1, cy1, cx2, cy2) = clip_bounds;
    let w = metrics.width;
    for gy in 0..metrics.height {
        let slant = if is_italic {
            (metrics.height as f32 - gy as f32) * 0.22
        } else {
            0.0
        };

        let row_start = gy * w;
        for gx in 0..w {
            let center = bitmap[row_start + gx] as u32;
            let left = if gx > 0 {
                bitmap[row_start + gx - 1] as u32
            } else {
                0
            };
            let right = if gx + 1 < w {
                bitmap[row_start + gx + 1] as u32
            } else {
                0
            };

            let cov_r = ((left + 2 * center + 1) / 3).min(255);
            let cov_g = center.min(255);
            let cov_b = ((2 * center + right + 1) / 3).min(255);

            if cov_r == 0 && cov_g == 0 && cov_b == 0 {
                continue;
            }

            let px = (glyph_x + gx as f32 + slant) as i32;
            let py = (glyph_y + gy as f32) as i32;

            if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                continue;
            }

            let idx = (py as u32 * buf_width + px as u32) as usize;
            if idx >= buffer.len() {
                continue;
            }

            let cr = color.r as u32;
            let cg = color.g as u32;
            let cb = color.b as u32;

            let eff_ar = ((cov_r * color.a as u32) + 127) / 255;
            let eff_ag = ((cov_g * color.a as u32) + 127) / 255;
            let eff_ab = ((cov_b * color.a as u32) + 127) / 255;

            let bg = buffer[idx];
            let b_r = (bg >> 16) & 0xFF;
            let b_g = (bg >> 8) & 0xFF;
            let b_b = bg & 0xFF;

            let out_r = ((cr * eff_ar + b_r * (255 - eff_ar) + 127) / 255).min(255);
            let out_g = ((cg * eff_ag + b_g * (255 - eff_ag) + 127) / 255).min(255);
            let out_b = ((cb * eff_ab + b_b * (255 - eff_ab) + 127) / 255).min(255);

            buffer[idx] = (out_r << 16) | (out_g << 8) | out_b;
        }
    }
}

impl FontManager {
    /// Rasterizes a text string into a pixel buffer with alpha-blended glyphs and letter spacing.
    ///
    /// The buffer is `&mut [u32]` in `0xRRGGBB` format, with the given width and height.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
    ) {
        self.rasterize_text_clipped(
            buffer,
            buf_width,
            buf_height,
            x,
            y,
            text,
            color,
            font_size,
            weight,
            family,
            style,
            letter_spacing,
            mango_core::Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_clipped(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
        clip: mango_core::Rect,
    ) {
        if font_size <= 0.0 || text.is_empty() {
            return;
        }
        let primary_font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);

        let ascent = primary_font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let is_italic = matches!(style, FontStyle::Italic | FontStyle::Oblique);
        let mut cursor_x = x as f32;

        let cx1 = clip.x().max(0.0) as i32;
        let cy1 = clip.y().max(0.0) as i32;
        let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
        let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

        if text.chars().any(super::shaping::is_complex_char) {
            let script_ch = text
                .chars()
                .find(|c| super::shaping::is_complex_char(*c))
                .unwrap_or(' ');
            let font = self.select_font_for_char(family, weight, script_ch);
            let font_data = self.select_font_data_for_char(family, weight, script_ch);
            if let Some(shaped) = super::shaping::shape_text_with_font_data(
                &font_data,
                0,
                text,
                px_size,
                letter_spacing,
            ) {
                if !shaped.glyphs.is_empty() && shaped.glyphs.iter().any(|g| g.glyph_id != 0) {
                    for glyph in &shaped.glyphs {
                        if glyph.glyph_id == 0 {
                            cursor_x += glyph.x_advance;
                            continue;
                        }
                        let cached = self.get_or_rasterize_glyph_indexed(
                            family,
                            weight,
                            &font,
                            glyph.glyph_id,
                            px_size,
                        );
                        let metrics = &cached.metrics;
                        let bitmap = &cached.bitmap;
                        if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                            let glyph_x = cursor_x + glyph.x_offset + metrics.xmin as f32;
                            let glyph_y = y as f32
                                + (ascent - metrics.height as f32 - metrics.ymin as f32
                                    + glyph.y_offset)
                                    .round();
                            blit_glyph_bitmap(
                                buffer,
                                buf_width,
                                metrics,
                                bitmap,
                                glyph_x,
                                glyph_y,
                                is_italic,
                                color,
                                (cx1, cy1, cx2, cy2),
                            );
                        }
                        cursor_x += glyph.x_advance;
                    }
                    return;
                }
            }
        }

        let shaped_text;
        let text_to_draw = if text.chars().any(super::shaping::is_complex_char) {
            shaped_text = super::shaping::shape_complex_script(text);
            &shaped_text
        } else {
            text
        };

        for ch in text_to_draw.chars() {
            if ch == '\n' || super::emoji::is_zero_width_or_format(ch) {
                continue;
            }

            if is_emoji_char(ch) {
                let emoji = get_or_rasterize_color_emoji(ch, px_size);
                let emoji_x = cursor_x;
                let emoji_y = y as f32 + ascent - emoji.height as f32;
                blit_emoji_px(
                    buffer,
                    buf_width,
                    &emoji,
                    emoji_x,
                    emoji_y,
                    color.a,
                    (cx1, cy1, cx2, cy2),
                );
                cursor_x += emoji.width as f32 + letter_spacing;
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, &font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y =
                    y as f32 + (ascent - metrics.height as f32 - metrics.ymin as f32).round();
                blit_glyph_bitmap(
                    buffer,
                    buf_width,
                    metrics,
                    bitmap,
                    glyph_x,
                    glyph_y,
                    is_italic,
                    color,
                    (cx1, cy1, cx2, cy2),
                );
            }

            cursor_x += metrics.advance_width + letter_spacing;
        }
    }

    /// Rasterizes text with ClearType-style subpixel LCD anti-aliasing without clipping.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_subpixel(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
    ) {
        self.rasterize_text_subpixel_clipped(
            buffer,
            buf_width,
            buf_height,
            x,
            y,
            text,
            color,
            font_size,
            weight,
            family,
            style,
            letter_spacing,
            mango_core::Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32),
        );
    }

    /// Rasterizes text with ClearType-style subpixel LCD anti-aliasing.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_subpixel_clipped(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
        clip: mango_core::Rect,
    ) {
        if font_size <= 0.0 || text.is_empty() {
            return;
        }
        let primary_font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);

        let ascent = primary_font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let is_italic = matches!(style, FontStyle::Italic | FontStyle::Oblique);
        let mut cursor_x = x as f32;

        let cx1 = clip.x().max(0.0) as i32;
        let cy1 = clip.y().max(0.0) as i32;
        let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
        let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

        if text.chars().any(super::shaping::is_complex_char) {
            let script_ch = text
                .chars()
                .find(|c| super::shaping::is_complex_char(*c))
                .unwrap_or(' ');
            let font = self.select_font_for_char(family, weight, script_ch);
            let font_data = self.select_font_data_for_char(family, weight, script_ch);
            if let Some(shaped) = super::shaping::shape_text_with_font_data(
                &font_data,
                0,
                text,
                px_size,
                letter_spacing,
            ) {
                if !shaped.glyphs.is_empty() && shaped.glyphs.iter().any(|g| g.glyph_id != 0) {
                    for glyph in &shaped.glyphs {
                        if glyph.glyph_id == 0 {
                            cursor_x += glyph.x_advance;
                            continue;
                        }
                        let cached = self.get_or_rasterize_glyph_indexed(
                            family,
                            weight,
                            &font,
                            glyph.glyph_id,
                            px_size,
                        );
                        let metrics = &cached.metrics;
                        let bitmap = &cached.bitmap;
                        if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                            let glyph_x = cursor_x + glyph.x_offset + metrics.xmin as f32;
                            let glyph_y = y as f32
                                + (ascent - metrics.height as f32 - metrics.ymin as f32
                                    + glyph.y_offset)
                                    .round();
                            blit_glyph_bitmap_subpixel(
                                buffer,
                                buf_width,
                                metrics,
                                bitmap,
                                glyph_x,
                                glyph_y,
                                is_italic,
                                color,
                                (cx1, cy1, cx2, cy2),
                            );
                        }
                        cursor_x += glyph.x_advance;
                    }
                    return;
                }
            }
        }

        let shaped_text;
        let text_to_draw = if text.chars().any(super::shaping::is_complex_char) {
            shaped_text = super::shaping::shape_complex_script(text);
            &shaped_text
        } else {
            text
        };

        for ch in text_to_draw.chars() {
            if ch == '\n' || super::emoji::is_zero_width_or_format(ch) {
                continue;
            }

            if is_emoji_char(ch) {
                let emoji = get_or_rasterize_color_emoji(ch, px_size);
                let emoji_x = cursor_x;
                let emoji_y = y as f32 + ascent - emoji.height as f32;
                blit_emoji_px(
                    buffer,
                    buf_width,
                    &emoji,
                    emoji_x,
                    emoji_y,
                    color.a,
                    (cx1, cy1, cx2, cy2),
                );
                cursor_x += emoji.width as f32 + letter_spacing;
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, &font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y =
                    y as f32 + (ascent - metrics.height as f32 - metrics.ymin as f32).round();
                blit_glyph_bitmap_subpixel(
                    buffer,
                    buf_width,
                    metrics,
                    bitmap,
                    glyph_x,
                    glyph_y,
                    is_italic,
                    color,
                    (cx1, cy1, cx2, cy2),
                );
            }

            cursor_x += metrics.advance_width + letter_spacing;
        }
    }

    /// Rasterizes text clipped to a gradient fill (used for `background-clip: text`).
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_gradient_clipped(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        gradient: &mango_css::values::Gradient,
        gradient_rect: mango_core::Rect,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
        clip: mango_core::Rect,
    ) {
        let gw = gradient_rect.width().max(1.0) as u32;
        let gh = gradient_rect.height().max(1.0) as u32;
        let grad_pixels = gradient.rasterize(gw, gh);
        if grad_pixels.is_empty() || font_size <= 0.0 || text.is_empty() {
            return;
        }

        let primary_font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);

        let ascent = primary_font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let is_italic = matches!(style, FontStyle::Italic | FontStyle::Oblique);
        let mut cursor_x = x as f32;

        let cx1 = clip.x().max(0.0) as i32;
        let cy1 = clip.y().max(0.0) as i32;
        let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
        let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, &font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y =
                    y as f32 + (ascent - metrics.height as f32 - metrics.ymin as f32).round();

                for gy in 0..metrics.height {
                    let slant = if is_italic {
                        (metrics.height as f32 - gy as f32) * 0.22
                    } else {
                        0.0
                    };

                    for gx in 0..metrics.width {
                        let coverage = bitmap[gy * metrics.width + gx];
                        if coverage == 0 {
                            continue;
                        }

                        let px = (glyph_x + gx as f32 + slant) as i32;
                        let py = (glyph_y + gy as f32) as i32;

                        if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                            continue;
                        }

                        let idx = (py as u32 * buf_width + px as u32) as usize;
                        if idx >= buffer.len() {
                            continue;
                        }

                        let grad_x =
                            (((px as f32 - gradient_rect.x()) / gradient_rect.width().max(1.0))
                                * gw as f32)
                                .clamp(0.0, (gw - 1) as f32) as u32;
                        let grad_y =
                            (((py as f32 - gradient_rect.y()) / gradient_rect.height().max(1.0))
                                * gh as f32)
                                .clamp(0.0, (gh - 1) as f32) as u32;
                        let src = grad_pixels[(grad_y * gw + grad_x) as usize];
                        let src_a = (src >> 24) & 0xFF;
                        let src_r = (src >> 16) & 0xFF;
                        let src_g = (src >> 8) & 0xFF;
                        let src_b = src & 0xFF;

                        let effective_alpha = ((coverage as u32 * src_a) + 127) / 255;
                        if effective_alpha == 0 {
                            continue;
                        }

                        if effective_alpha == 255 {
                            buffer[idx] = (src_r << 16) | (src_g << 8) | src_b;
                        } else {
                            let inv_alpha = 255 - effective_alpha;
                            let bg = buffer[idx];
                            let bg_r = (bg >> 16) & 0xFF;
                            let bg_g = (bg >> 8) & 0xFF;
                            let bg_b = bg & 0xFF;
                            let r =
                                ((src_r * effective_alpha + bg_r * inv_alpha + 127) / 255).min(255);
                            let g =
                                ((src_g * effective_alpha + bg_g * inv_alpha + 127) / 255).min(255);
                            let b =
                                ((src_b * effective_alpha + bg_b * inv_alpha + 127) / 255).min(255);
                            buffer[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
            }

            cursor_x += metrics.advance_width + letter_spacing;
        }
    }
}
