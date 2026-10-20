//! Text and decorated text rasterization.

use mango_core::{Color, Rect};

use crate::font::{FontFamily, FontStyle, FontWeight, TextDecoration, font_manager};
use crate::painter::affine::{apply_matrix, blit_image_transformed, is_identity, matrix_scale};
use crate::painter::blend::fill_rect_mode;
use crate::painter::shapes::intersect_rect;

/// Rasterizes a text run into the buffer, handling affine transforms, decorations, and blend modes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_text(
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
    decoration: TextDecoration,
    letter_spacing: f32,
    active_clip: Rect,
    transform: [f32; 6],
    active_blend: mango_css::values::BlendMode,
) {
    if color.a == 0 || font_size <= 0.5 || text.is_empty() {
        return;
    }
    let fm = font_manager();

    // Column lengths give the true per-axis scale (handles
    // non-uniform scale + rotation/shear without sqrt(det) distortion).
    let sx = (transform[0] * transform[0] + transform[1] * transform[1]).sqrt();
    let sy = (transform[2] * transform[2] + transform[3] * transform[3]).sqrt();
    let max_s = sx.max(sy).max(1.0);
    let non_uniform = (sx - sy).abs() > 1e-4 * max_s;
    let is_rotated = transform[1].abs() > 1e-4 || transform[2].abs() > 1e-4 || non_uniform;

    if is_rotated {
        let (text_w, text_h) =
            fm.measure_text_with_spacing(text, font_size, weight, family, letter_spacing);
        let w_u32 = text_w.ceil().max(1.0) as u32;
        let h_u32 = text_h.ceil().max(1.0) as u32;
        let mut text_buf = vec![0u32; (w_u32 * h_u32) as usize];
        let local_clip = Rect::new(0.0, 0.0, text_w, text_h);
        fm.rasterize_text_clipped(
            &mut text_buf,
            w_u32,
            h_u32,
            0,
            0,
            text,
            color,
            font_size,
            weight,
            family,
            style,
            letter_spacing,
            local_clip,
        );
        blit_image_transformed(
            buffer,
            buf_width,
            buf_height,
            x,
            y,
            text_w,
            text_h,
            &text_buf,
            active_clip,
            transform,
            active_blend,
        );
    } else {
        let (draw_x, draw_y, draw_size) = if is_identity(transform) {
            (x.round(), y.round(), font_size)
        } else {
            let (tx, ty) = apply_matrix(transform, x, y);
            let scale = matrix_scale(transform);
            (tx.round(), ty.round(), font_size * scale)
        };
        if draw_size <= 0.5 {
            return;
        }
        fm.rasterize_text_clipped(
            buffer,
            buf_width,
            buf_height,
            draw_x as i32,
            draw_y as i32,
            text,
            color,
            draw_size,
            weight,
            family,
            style,
            letter_spacing * (draw_size / font_size.max(0.001)),
            active_clip,
        );
    }

    if decoration != TextDecoration::None && is_identity(transform) {
        let (text_w, _) =
            fm.measure_text_with_spacing(text, font_size, weight, family, letter_spacing);
        let font = fm.select_font(family, weight);
        let ascent = font
            .horizontal_line_metrics(font_size.max(1.0))
            .map(|lm| lm.ascent)
            .unwrap_or(font_size * 0.8);
        let thickness = (font_size / 14.0).max(1.0);

        let line_rect = match decoration {
            TextDecoration::Underline => {
                let line_y = y + ascent + 2.0;
                Rect::new(x, line_y, text_w.ceil(), thickness)
            }
            TextDecoration::LineThrough => {
                let line_y = y + ascent - (font_size * 0.3);
                Rect::new(x, line_y, text_w.ceil(), thickness)
            }
            TextDecoration::Overline => {
                let line_y = y + 1.0;
                Rect::new(x, line_y, text_w.ceil(), thickness)
            }
            TextDecoration::None => Rect::new(0.0, 0.0, 0.0, 0.0),
        };
        let r = intersect_rect(active_clip, line_rect);
        if r.width() > 0.0 && r.height() > 0.0 {
            fill_rect_mode(
                buffer,
                buf_width,
                buf_height,
                r.x() as i32,
                r.y() as i32,
                r.width() as u32,
                r.height() as u32,
                color,
                active_blend,
            );
        }
    }
}

/// Rasterizes text filled with a gradient shader.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_text_with_gradient(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    text: &str,
    x: f32,
    y: f32,
    gradient: &mango_css::values::Gradient,
    gradient_rect: Rect,
    font_size: f32,
    weight: FontWeight,
    family: FontFamily,
    style: FontStyle,
    letter_spacing: f32,
    active_clip: Rect,
) {
    let fm = font_manager();
    fm.rasterize_text_gradient_clipped(
        buffer,
        buf_width,
        buf_height,
        x.round() as i32,
        y.round() as i32,
        text,
        gradient,
        gradient_rect,
        font_size,
        weight,
        family,
        style,
        letter_spacing,
        active_clip,
    );
}
