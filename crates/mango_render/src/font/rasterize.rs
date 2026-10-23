//! Glyph rasterization, ClearType subpixel filtering, and clipping.

use fontdue::Font;
use mango_core::Color;
use std::sync::atomic::{AtomicU64, Ordering};

use super::emoji::{get_or_rasterize_color_emoji, is_emoji_char};
use super::manager::FontManager;
use super::types::{
    ACCESS_COUNTER, CachedGlyph, FontFamily, FontStyle, FontWeight, GlyphCacheEntry, glyph_cache,
};

impl FontManager {
    /// Retrieves a rasterized glyph from the glyph cache, or rasterizes and caches it.
    pub fn get_or_rasterize_glyph(
        &self,
        family: FontFamily,
        weight: FontWeight,
        font: &Font,
        ch: char,
        px_size: f32,
    ) -> CachedGlyph {
        let size_key = (px_size * 10.0).round() as u32;
        let key = (family, weight, size_key, ch);

        if let Ok(cache) = glyph_cache().read()
            && let Some(entry) = cache.get(&key)
        {
            entry.last_access.store(
                ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed),
                Ordering::Relaxed,
            );
            return entry.glyph.clone();
        }

        let (mut metrics, bitmap) = font.rasterize(ch, px_size);
        let final_bitmap: std::sync::Arc<[u8]> = if weight == FontWeight::Bold
            && std::ptr::eq(font, self.select_font(family, FontWeight::Regular).as_ref())
            && metrics.width > 0
            && metrics.height > 0
            && !bitmap.is_empty()
            && !ch.is_whitespace()
        {
            let offset = if px_size >= 48.0 {
                3
            } else if px_size >= 28.0 {
                2
            } else {
                1
            };
            let old_w = metrics.width;
            let h = metrics.height;
            let new_w = old_w + offset;
            let mut smeared = vec![0u8; new_w * h];
            for y in 0..h {
                for x in 0..old_w {
                    let cov = bitmap[y * old_w + x];
                    if cov > 0 {
                        for off in 0..=offset {
                            let dst_idx = y * new_w + x + off;
                            smeared[dst_idx] = smeared[dst_idx].max(cov);
                        }
                    }
                }
            }
            metrics.width = new_w;
            metrics.advance_width += offset as f32;
            smeared.into()
        } else {
            bitmap.into()
        };

        let cached = CachedGlyph {
            metrics,
            bitmap: final_bitmap,
        };

        if let Ok(mut cache) = glyph_cache().write() {
            if cache.len() >= 4096 {
                // Bounded LRU eviction: retain the most recent 2048 entries instead of clearing all
                let mut access_times: Vec<u64> = cache
                    .values()
                    .map(|e| e.last_access.load(Ordering::Relaxed))
                    .collect();
                access_times.sort_unstable();
                let median_threshold = access_times[access_times.len() / 2];
                cache.retain(|_, v| v.last_access.load(Ordering::Relaxed) >= median_threshold);
            }
            let tick = ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed);
            cache.insert(
                key,
                GlyphCacheEntry {
                    glyph: cached.clone(),
                    last_access: AtomicU64::new(tick),
                },
            );
        }

        cached
    }

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

            if is_emoji_char(ch) {
                let emoji = get_or_rasterize_color_emoji(ch, px_size);
                let emoji_x = cursor_x;
                let emoji_y = y as f32 + ascent - emoji.height as f32;

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

                        let eff_a = ((sa * color.a as u32) + 127) / 255;
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

                cursor_x += emoji.width as f32 + letter_spacing;
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, &font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y = y as f32 + ascent - metrics.height as f32 - metrics.ymin as f32;

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
                            let r = ((color.r as u32 * effective_alpha + bg_r * inv_alpha + 127)
                                / 255)
                                .min(255);
                            let g = ((color.g as u32 * effective_alpha + bg_g * inv_alpha + 127)
                                / 255)
                                .min(255);
                            let b = ((color.b as u32 * effective_alpha + bg_b * inv_alpha + 127)
                                / 255)
                                .min(255);
                            buffer[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
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

            if is_emoji_char(ch) {
                let emoji = get_or_rasterize_color_emoji(ch, px_size);
                let emoji_x = cursor_x;
                let emoji_y = y as f32 + ascent - emoji.height as f32;

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

                        let eff_a = ((sa * color.a as u32) + 127) / 255;
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

                cursor_x += emoji.width as f32 + letter_spacing;
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, &font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y = y as f32 + ascent - metrics.height as f32 - metrics.ymin as f32;
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

                        let eff_r = ((cov_r * color.a as u32) + 127) / 255;
                        let eff_g = ((cov_g * color.a as u32) + 127) / 255;
                        let eff_b = ((cov_b * color.a as u32) + 127) / 255;

                        let bg = buffer[idx];
                        let bg_r = (bg >> 16) & 0xFF;
                        let bg_g = (bg >> 8) & 0xFF;
                        let bg_b = bg & 0xFF;

                        let out_r =
                            ((color.r as u32 * eff_r + bg_r * (255 - eff_r) + 127) / 255).min(255);
                        let out_g =
                            ((color.g as u32 * eff_g + bg_g * (255 - eff_g) + 127) / 255).min(255);
                        let out_b =
                            ((color.b as u32 * eff_b + bg_b * (255 - eff_b) + 127) / 255).min(255);

                        buffer[idx] = (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
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
        if grad_pixels.is_empty() {
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
                let glyph_y = y as f32 + ascent - metrics.height as f32 - metrics.ymin as f32;

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
