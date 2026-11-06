//! Box shadow and text shadow rasterization.

use mango_core::{Color, Rect};

use crate::font::{FontFamily, FontStyle, FontWeight, font_manager};
use crate::painter::blend::blend_pixel_mode;
use crate::painter::shapes::{fill_rounded_rect_clipped, intersect_rect, sdf_rounded_rect};

/// Paints a blurred or crisp text shadow using multi-sample proxy blurring.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_text_shadow(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    text: &str,
    x: f32,
    y: f32,
    color: Color,
    font_size: f32,
    weight: FontWeight,
    family: FontFamily,
    style: FontStyle,
    blur_radius: f32,
    letter_spacing: f32,
    active_clip: Rect,
) {
    if color.a == 0 || font_size <= 0.5 || text.is_empty() {
        return;
    }
    let fm = font_manager();
    let blur = blur_radius.max(0.0);
    if blur <= 0.5 {
        fm.rasterize_text_clipped(
            buffer,
            buf_width,
            buf_height,
            x.round() as i32,
            y.round() as i32,
            text,
            color,
            font_size,
            weight,
            family,
            style,
            letter_spacing,
            active_clip,
        );
    } else {
        let radius = (blur * 0.5).clamp(1.0, 6.0);
        let steps = radius.ceil() as i32;
        let samples = ((steps * 2 + 1) * (steps * 2 + 1)) as f32;
        let base_alpha = color.a as f32;
        let step_alpha = (base_alpha * 1.6 / samples).clamp(4.0, 255.0) as u8;
        let faded = Color::rgba(color.r, color.g, color.b, step_alpha);
        for dy in -steps..=steps {
            for dx in -steps..=steps {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist > radius + 0.5 {
                    continue;
                }
                fm.rasterize_text_clipped(
                    buffer,
                    buf_width,
                    buf_height,
                    x as i32 + dx,
                    y as i32 + dy,
                    text,
                    faded,
                    font_size,
                    weight,
                    family,
                    style,
                    letter_spacing,
                    active_clip,
                );
            }
        }
    }
}

/// Paints an inset or outset box shadow with rounded corners and multi-step blur.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_box_shadow(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    color: Color,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    spread_radius: f32,
    radii: [f32; 4],
    inset: bool,
    active_clip: Rect,
    active_blend: mango_css::values::BlendMode,
) {
    if color.a == 0 {
        return;
    }
    if inset {
        let inset_clip = intersect_rect(active_clip, *rect);
        if inset_clip.width() <= 0.0 || inset_clip.height() <= 0.0 {
            return;
        }
        let blur = blur_radius.max(0.0);
        if blur <= 0.5 {
            let inner_rect = Rect::new(
                rect.x() + offset_x + spread_radius,
                rect.y() + offset_y + spread_radius,
                (rect.width() - spread_radius * 2.0).max(0.0),
                (rect.height() - spread_radius * 2.0).max(0.0),
            );
            let clip_out = Some((&inner_rect, radii));
            fill_rounded_rect_clipped(
                buffer,
                buf_width,
                buf_height,
                rect,
                color,
                radii,
                clip_out,
                inset_clip,
                active_blend,
            );
        } else {
            let inner_rect = Rect::new(
                rect.x() + offset_x + spread_radius,
                rect.y() + offset_y + spread_radius,
                (rect.width() - spread_radius * 2.0).max(0.0),
                (rect.height() - spread_radius * 2.0).max(0.0),
            );
            let sigma = (blur * 0.55).max(0.5);
            let inv_2_sigma_sq = 1.0 / (2.0 * sigma * sigma);
            let base_a = color.a as f32;

            let min_x = (inset_clip.x().max(0.0) as u32).min(buf_width);
            let min_y = (inset_clip.y().max(0.0) as u32).min(buf_height);
            let max_x = ((inset_clip.x() + inset_clip.width()).max(0.0) as u32).min(buf_width);
            let max_y = ((inset_clip.y() + inset_clip.height()).max(0.0) as u32).min(buf_height);

            for y in min_y..max_y {
                let fy = y as f32 + 0.5;
                let row_offset = (y * buf_width) as usize;
                for x in min_x..max_x {
                    let fx = x as f32 + 0.5;
                    let d_elem = sdf_rounded_rect(fx, fy, rect, radii);
                    if d_elem >= 0.5 {
                        continue;
                    }
                    let cov_elem = (0.5 - d_elem).clamp(0.0, 1.0);
                    let d_inner = sdf_rounded_rect(fx, fy, &inner_rect, radii);
                    let shadow_cov = if d_inner >= 0.0 {
                        (d_inner + 0.5).clamp(0.0, 1.0)
                    } else {
                        let dist = -d_inner;
                        (-dist * dist * inv_2_sigma_sq).exp()
                    };
                    let total_cov = cov_elem * shadow_cov;
                    if total_cov <= 0.002 {
                        continue;
                    }
                    let eff_a = (base_a * total_cov).round() as u8;
                    if eff_a > 0 {
                        let blended_color = Color::rgba(color.r, color.g, color.b, eff_a);
                        let idx = row_offset + x as usize;
                        if idx < buffer.len() {
                            buffer[idx] =
                                blend_pixel_mode(blended_color, buffer[idx], active_blend);
                        }
                    }
                }
            }
        }
        return;
    }

    let clip_out = Some((rect, radii));
    let blur = blur_radius.max(0.0);
    if blur <= 0.5 {
        let shadow_rect = Rect::new(
            rect.x() + offset_x - spread_radius,
            rect.y() + offset_y - spread_radius,
            (rect.width() + spread_radius * 2.0).max(0.0),
            (rect.height() + spread_radius * 2.0).max(0.0),
        );
        let r = intersect_rect(active_clip, shadow_rect);
        if r.width() > 0.0 && r.height() > 0.0 {
            fill_rounded_rect_clipped(
                buffer,
                buf_width,
                buf_height,
                &shadow_rect,
                color,
                radii,
                clip_out,
                active_clip,
                active_blend,
            );
        }
    } else {
        let extent = spread_radius + blur * 2.5;
        let min_x = ((rect.x() + offset_x - extent).min(rect.x()) as i32)
            .max(active_clip.x() as i32)
            .max(0) as u32;
        let min_y = ((rect.y() + offset_y - extent).min(rect.y()) as i32)
            .max(active_clip.y() as i32)
            .max(0) as u32;
        let max_x = (((rect.right() + offset_x + extent).max(rect.right()).ceil()) as i32)
            .min((active_clip.x() + active_clip.width()) as i32)
            .min(buf_width as i32)
            .max(0) as u32;
        let max_y = (((rect.bottom() + offset_y + extent)
            .max(rect.bottom())
            .ceil()) as i32)
            .min((active_clip.y() + active_clip.height()) as i32)
            .min(buf_height as i32)
            .max(0) as u32;

        let shadow_base = Rect::new(
            rect.x() + offset_x - spread_radius,
            rect.y() + offset_y - spread_radius,
            (rect.width() + spread_radius * 2.0).max(0.0),
            (rect.height() + spread_radius * 2.0).max(0.0),
        );
        let shadow_radii = [
            (radii[0] + spread_radius).max(0.0),
            (radii[1] + spread_radius).max(0.0),
            (radii[2] + spread_radius).max(0.0),
            (radii[3] + spread_radius).max(0.0),
        ];
        let sigma = (blur * 0.55).max(0.5);
        let inv_2_sigma_sq = 1.0 / (2.0 * sigma * sigma);
        let base_a = color.a as f32;

        for y in min_y..max_y {
            let fy = y as f32 + 0.5;
            let row_offset = (y * buf_width) as usize;
            for x in min_x..max_x {
                let fx = x as f32 + 0.5;
                let d_in = sdf_rounded_rect(fx, fy, rect, radii);
                if d_in <= -0.5 {
                    continue;
                }
                let cov_out = (d_in + 0.5).clamp(0.0, 1.0);
                let d_shadow = sdf_rounded_rect(fx, fy, &shadow_base, shadow_radii);
                let shadow_cov = if d_shadow <= 0.0 {
                    (0.5 - d_shadow).clamp(0.0, 1.0)
                } else {
                    (-d_shadow * d_shadow * inv_2_sigma_sq).exp()
                };
                let total_cov = cov_out * shadow_cov;
                if total_cov <= 0.002 {
                    continue;
                }
                let eff_a = (base_a * total_cov).round() as u8;
                if eff_a > 0 {
                    let blended_color = Color::rgba(color.r, color.g, color.b, eff_a);
                    let idx = row_offset + x as usize;
                    if idx < buffer.len() {
                        buffer[idx] = blend_pixel_mode(blended_color, buffer[idx], active_blend);
                    }
                }
            }
        }
    }
}
