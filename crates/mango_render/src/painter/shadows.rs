//! Box shadow and text shadow rasterization.

use mango_core::{Color, Rect};

use crate::font::{FontFamily, FontStyle, FontWeight, font_manager};
use crate::painter::shapes::{fill_rounded_rect_clipped, intersect_rect};

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
            let steps = (blur.min(16.0) / 2.0).ceil().max(1.0) as i32;
            let base_alpha = color.a as f32;
            for step in (0..=steps).rev() {
                let t = (step as f32) / (steps as f32);
                let cur_spread = spread_radius + blur * t;
                let step_alpha =
                    ((base_alpha * (1.0 - t * 0.6)) / (steps as f32 + 1.0)).clamp(0.0, 255.0) as u8;
                if step_alpha == 0 {
                    continue;
                }
                let step_color = Color::rgba(color.r, color.g, color.b, step_alpha);
                let inner_rect = Rect::new(
                    rect.x() + offset_x + cur_spread,
                    rect.y() + offset_y + cur_spread,
                    (rect.width() - cur_spread * 2.0).max(0.0),
                    (rect.height() - cur_spread * 2.0).max(0.0),
                );
                let clip_out = Some((&inner_rect, radii));
                fill_rounded_rect_clipped(
                    buffer,
                    buf_width,
                    buf_height,
                    rect,
                    step_color,
                    radii,
                    clip_out,
                    inset_clip,
                    active_blend,
                );
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
        let steps = (blur.min(16.0) / 2.0).ceil().max(1.0) as i32;
        let base_alpha = color.a as f32;
        for step in (0..=steps).rev() {
            let t = (step as f32) / (steps as f32);
            let cur_spread = spread_radius + blur * t;
            let step_alpha =
                ((base_alpha * (1.0 - t * 0.6)) / (steps as f32 + 1.0)).clamp(0.0, 255.0) as u8;
            if step_alpha == 0 {
                continue;
            }
            let step_color = Color::rgba(color.r, color.g, color.b, step_alpha);
            let shadow_rect = Rect::new(
                rect.x() + offset_x - cur_spread,
                rect.y() + offset_y - cur_spread,
                (rect.width() + cur_spread * 2.0).max(0.0),
                (rect.height() + cur_spread * 2.0).max(0.0),
            );
            let step_radii = [
                (radii[0] + cur_spread).max(0.0),
                (radii[1] + cur_spread).max(0.0),
                (radii[2] + cur_spread).max(0.0),
                (radii[3] + cur_spread).max(0.0),
            ];
            let r = intersect_rect(active_clip, shadow_rect);
            if r.width() > 0.0 && r.height() > 0.0 {
                fill_rounded_rect_clipped(
                    buffer,
                    buf_width,
                    buf_height,
                    &shadow_rect,
                    step_color,
                    step_radii,
                    clip_out,
                    active_clip,
                    active_blend,
                );
            }
        }
    }
}
