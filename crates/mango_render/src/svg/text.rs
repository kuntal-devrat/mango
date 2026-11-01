//! SVG `<text>` and `<tspan>` layout, text-anchor positioning, and font rasterization.

use mango_core::Color;
use tiny_skia::{Mask, Pixmap, Transform};

use super::ast::{SvgPaint, SvgRenderState, SvgTextAnchor, SvgTspan};
use crate::font::{FontFamily, FontWeight, font_manager};

/// Renders an SVG text node and its nested tspans onto the target pixmap.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_svg_text(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    main_text: &str,
    font_size: f32,
    font_family_str: &str,
    font_weight_str: &str,
    text_anchor: SvgTextAnchor,
    tspans: &[SvgTspan],
    state: &SvgRenderState,
    transform: Transform,
    current_color: Color,
    clip_mask: Option<&Mask>,
) {
    let effective_font_size = if font_size > 0.0 {
        font_size
    } else if state.font_size > 0.0 {
        state.font_size
    } else {
        16.0
    };

    let family = parse_font_family(if !font_family_str.is_empty() {
        font_family_str
    } else {
        &state.font_family
    });
    let weight = parse_font_weight(if !font_weight_str.is_empty() {
        font_weight_str
    } else {
        &state.font_weight
    });

    let fm = font_manager();

    // Compute total text span length for text-anchor adjustment
    let mut total_width = 0.0f32;
    if !main_text.is_empty() {
        let (w, _) = fm.measure_text(main_text, effective_font_size, weight, family);
        total_width += w;
    }
    for tspan in tspans {
        let (w, _) = fm.measure_text(&tspan.text, effective_font_size, weight, family);
        total_width += w;
    }

    let anchor_offset_x = match text_anchor {
        SvgTextAnchor::Start => 0.0,
        SvgTextAnchor::Middle => -total_width / 2.0,
        SvgTextAnchor::End => -total_width,
    };

    let mut cursor_x = x + dx + anchor_offset_x;
    let mut baseline_y = y + dy;

    // 1. Render main text if present
    if !main_text.is_empty() {
        cursor_x = render_text_run(
            pixmap,
            main_text,
            cursor_x,
            baseline_y,
            effective_font_size,
            family,
            weight,
            state,
            transform,
            current_color,
            clip_mask,
        );
    }

    // 2. Render nested tspans
    for tspan in tspans {
        if let Some(sx) = tspan.x {
            cursor_x = sx + anchor_offset_x;
        }
        if let Some(sy) = tspan.y {
            baseline_y = sy;
        }
        if let Some(sdx) = tspan.dx {
            cursor_x += sdx;
        }
        if let Some(sdy) = tspan.dy {
            baseline_y += sdy;
        }

        let tspan_state = super::render::compose_state(&tspan.state, state);
        let tspan_size = if tspan_state.font_size > 0.0 {
            tspan_state.font_size
        } else {
            effective_font_size
        };

        cursor_x = render_text_run(
            pixmap,
            &tspan.text,
            cursor_x,
            baseline_y,
            tspan_size,
            family,
            weight,
            &tspan_state,
            transform,
            current_color,
            clip_mask,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_text_run(
    pixmap: &mut Pixmap,
    text: &str,
    mut cursor_x: f32,
    baseline_y: f32,
    font_size: f32,
    family: FontFamily,
    weight: FontWeight,
    state: &SvgRenderState,
    transform: Transform,
    current_color: Color,
    clip_mask: Option<&Mask>,
) -> f32 {
    let fill_color = match state.fill {
        Some(SvgPaint::Color(c)) => Some(c),
        Some(SvgPaint::CurrentColor) => Some(current_color),
        Some(SvgPaint::None) => None,
        Some(SvgPaint::Url(_)) => Some(Color::BLACK),
        None => Some(Color::BLACK), // Default SVG text fill is black
    };

    let Some(color) = fill_color else {
        let fm = font_manager();
        let (w, _) = fm.measure_text(text, font_size, weight, family);
        return cursor_x + w;
    };

    let base_alpha = (color.a as f32 / 255.0) * state.fill_opacity * state.opacity;
    if base_alpha <= 0.0 {
        let fm = font_manager();
        let (w, _) = fm.measure_text(text, font_size, weight, family);
        return cursor_x + w;
    }

    let fm = font_manager();
    let font = fm.select_font(family, weight);

    let pm_w = pixmap.width() as i32;
    let pm_h = pixmap.height() as i32;

    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            continue;
        }
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        if metrics.width > 0 && metrics.height > 0 && !bitmap.is_empty() {
            let glyph_x = cursor_x + metrics.xmin as f32;
            let glyph_y = baseline_y - metrics.height as f32 - metrics.ymin as f32;
            let gw = metrics.width as u32;
            let gh = metrics.height as u32;

            blend_glyph(
                pixmap, &bitmap, gw, gh, glyph_x, glyph_y, color, base_alpha, transform, pm_w,
                pm_h, clip_mask,
            );
        }
        cursor_x += metrics.advance_width;
    }

    cursor_x
}

#[allow(clippy::too_many_arguments)]
fn blend_glyph(
    pixmap: &mut Pixmap,
    bitmap: &[u8],
    glyph_w: u32,
    glyph_h: u32,
    gx: f32,
    gy: f32,
    color: Color,
    base_alpha: f32,
    tf: Transform,
    pm_w: i32,
    pm_h: i32,
    clip_mask: Option<&Mask>,
) {
    let data = pixmap.data_mut();

    for row in 0..glyph_h {
        for col in 0..glyph_w {
            let cov = bitmap[(row * glyph_w + col) as usize] as f32 / 255.0;
            if cov <= 0.01 {
                continue;
            }
            let local_x = gx + col as f32;
            let local_y = gy + row as f32;

            let px = (tf.sx * local_x + tf.kx * local_y + tf.tx).round() as i32;
            let py = (tf.ky * local_x + tf.sy * local_y + tf.ty).round() as i32;

            if px >= 0 && px < pm_w && py >= 0 && py < pm_h {
                let mask_factor = if let Some(mask) = clip_mask {
                    let midx = (py * pm_w + px) as usize;
                    mask.data()[midx] as f32 / 255.0
                } else {
                    1.0
                };

                if mask_factor <= 0.01 {
                    continue;
                }

                let eff_alpha = base_alpha * cov * mask_factor;
                let idx = ((py * pm_w + px) * 4) as usize;
                let dst_r = data[idx] as f32 / 255.0;
                let dst_g = data[idx + 1] as f32 / 255.0;
                let dst_b = data[idx + 2] as f32 / 255.0;
                let dst_a = data[idx + 3] as f32 / 255.0;

                let src_r = color.r as f32 / 255.0;
                let src_g = color.g as f32 / 255.0;
                let src_b = color.b as f32 / 255.0;

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

pub(crate) fn parse_font_family(s: &str) -> FontFamily {
    FontFamily::from_css_name(s)
}

pub(crate) fn parse_font_weight(s: &str) -> FontWeight {
    let lower = s.trim().to_ascii_lowercase();
    if lower == "bold" || lower == "bolder" || lower == "700" || lower == "800" || lower == "900" {
        FontWeight::Bold
    } else {
        FontWeight::Regular
    }
}
