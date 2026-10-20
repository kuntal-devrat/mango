//! Border rasterization: CSS border images, 9-slicing, subpixel snapping, and triangles.

use mango_core::{Color, Rect};

use crate::display_list::BorderWidths;
use crate::painter::affine::{
    apply_matrix, effective_alpha, fill_rect_transformed, invert_matrix, is_identity,
    is_pure_translation, transformed_bounds,
};
use crate::painter::blend::{blend_pixel, fill_rect_mode};
use crate::painter::shapes::{fill_rounded_rect_clipped, intersect_rect};

/// Paints borders for a box, handling CSS triangles, rounded corners, and device pixel snapping.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_border(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    color: Color,
    widths: &BorderWidths,
    radii: &[f32; 4],
    active_clip: Rect,
    transform: [f32; 6],
    active_blend: mango_css::values::BlendMode,
) {
    let eff_rect = if is_identity(transform) {
        *rect
    } else if let Some((tx, ty)) = is_pure_translation(transform) {
        Rect::new(rect.x() + tx, rect.y() + ty, rect.width(), rect.height())
    } else {
        let x = rect.x();
        let y = rect.y();
        let w = rect.width();
        let h = rect.height();
        let side_y = y + widths.top;
        let side_h = (h - widths.top - widths.bottom).max(0.0);
        let edges = [
            Rect::new(x, y, w, widths.top),
            Rect::new(x, y + h - widths.bottom, w, widths.bottom),
            Rect::new(x, side_y, widths.left, side_h),
            Rect::new(x + w - widths.right, side_y, widths.right, side_h),
        ];
        let present = [
            widths.top > 0.0,
            widths.bottom > 0.0,
            widths.left > 0.0,
            widths.right > 0.0,
        ];
        for (edge, keep) in edges.iter().zip(present.iter()) {
            if *keep {
                fill_rect_transformed(
                    buffer,
                    buf_width,
                    buf_height,
                    edge,
                    color,
                    None,
                    active_clip,
                    transform,
                    active_blend,
                );
            }
        }
        return;
    };

    if radii.iter().any(|&r| r > 0.0) {
        let inner_x = eff_rect.x() + widths.left;
        let inner_y = eff_rect.y() + widths.top;
        let inner_w = (eff_rect.width() - widths.left - widths.right).max(0.0);
        let inner_h = (eff_rect.height() - widths.top - widths.bottom).max(0.0);
        let inner_rect = Rect::new(inner_x, inner_y, inner_w, inner_h);
        let inner_radii = [
            (radii[0] - widths.top.max(widths.left)).max(0.0),
            (radii[1] - widths.top.max(widths.right)).max(0.0),
            (radii[2] - widths.bottom.max(widths.right)).max(0.0),
            (radii[3] - widths.bottom.max(widths.left)).max(0.0),
        ];
        fill_rounded_rect_clipped(
            buffer,
            buf_width,
            buf_height,
            &eff_rect,
            color,
            *radii,
            Some((&inner_rect, inner_radii)),
            active_clip,
            active_blend,
        );
        return;
    }
    let x = eff_rect.x();
    let y = eff_rect.y();
    let w = eff_rect.width();
    let h = eff_rect.height();

    // CSS Border Triangle detection:
    // When an element has zero (or near-zero) content dimensions, borders meet at the center forming triangles.
    let is_zero_content =
        w <= widths.left + widths.right + 0.5 && h <= widths.top + widths.bottom + 0.5;
    if is_zero_content
        && (widths.top > 0.0 || widths.bottom > 0.0 || widths.left > 0.0 || widths.right > 0.0)
    {
        if widths.top > 0.0 && widths.bottom == 0.0 {
            // Downward-pointing triangle
            let start_y = y.round() as i32;
            let end_y = (y + widths.top).round() as i32;
            for cur_y in start_y..end_y {
                let t = if widths.top > 0.0 {
                    (cur_y as f32 - y) / widths.top
                } else {
                    0.0
                };
                let row_x1 = x + widths.left * t;
                let row_x2 = x + w - widths.right * t;
                let clip_x1 = (row_x1.max(active_clip.x())).round() as i32;
                let clip_x2 = (row_x2.min(active_clip.right())).round() as i32;
                if clip_x2 > clip_x1
                    && cur_y >= active_clip.y() as i32
                    && cur_y < active_clip.bottom() as i32
                {
                    fill_rect_mode(
                        buffer,
                        buf_width,
                        buf_height,
                        clip_x1,
                        cur_y,
                        (clip_x2 - clip_x1) as u32,
                        1,
                        color,
                        active_blend,
                    );
                }
            }
            return;
        } else if widths.bottom > 0.0 && widths.top == 0.0 {
            // Upward-pointing triangle
            let start_y = y.round() as i32;
            let end_y = (y + widths.bottom).round() as i32;
            for cur_y in start_y..end_y {
                let t = if widths.bottom > 0.0 {
                    (y + widths.bottom - cur_y as f32) / widths.bottom
                } else {
                    0.0
                };
                let row_x1 = x + widths.left * t;
                let row_x2 = x + w - widths.right * t;
                let clip_x1 = (row_x1.max(active_clip.x())).round() as i32;
                let clip_x2 = (row_x2.min(active_clip.right())).round() as i32;
                if clip_x2 > clip_x1
                    && cur_y >= active_clip.y() as i32
                    && cur_y < active_clip.bottom() as i32
                {
                    fill_rect_mode(
                        buffer,
                        buf_width,
                        buf_height,
                        clip_x1,
                        cur_y,
                        (clip_x2 - clip_x1) as u32,
                        1,
                        color,
                        active_blend,
                    );
                }
            }
            return;
        } else if widths.left > 0.0 && widths.right == 0.0 {
            // Rightward-pointing triangle
            let start_x = x.round() as i32;
            let end_x = (x + widths.left).round() as i32;
            for cur_x in start_x..end_x {
                let t = if widths.left > 0.0 {
                    (cur_x as f32 - x) / widths.left
                } else {
                    0.0
                };
                let col_y1 = y + widths.top * t;
                let col_y2 = y + h - widths.bottom * t;
                let clip_y1 = (col_y1.max(active_clip.y())).round() as i32;
                let clip_y2 = (col_y2.min(active_clip.bottom())).round() as i32;
                if clip_y2 > clip_y1
                    && cur_x >= active_clip.x() as i32
                    && cur_x < active_clip.right() as i32
                {
                    fill_rect_mode(
                        buffer,
                        buf_width,
                        buf_height,
                        cur_x,
                        clip_y1,
                        1,
                        (clip_y2 - clip_y1) as u32,
                        color,
                        active_blend,
                    );
                }
            }
            return;
        } else if widths.right > 0.0 && widths.left == 0.0 {
            // Leftward-pointing triangle
            let start_x = x.round() as i32;
            let end_x = (x + widths.right).round() as i32;
            for cur_x in start_x..end_x {
                let t = if widths.right > 0.0 {
                    (x + widths.right - cur_x as f32) / widths.right
                } else {
                    0.0
                };
                let col_y1 = y + widths.top * t;
                let col_y2 = y + h - widths.bottom * t;
                let clip_y1 = (col_y1.max(active_clip.y())).round() as i32;
                let clip_y2 = (col_y2.min(active_clip.bottom())).round() as i32;
                if clip_y2 > clip_y1
                    && cur_x >= active_clip.x() as i32
                    && cur_x < active_clip.right() as i32
                {
                    fill_rect_mode(
                        buffer,
                        buf_width,
                        buf_height,
                        cur_x,
                        clip_y1,
                        1,
                        (clip_y2 - clip_y1) as u32,
                        color,
                        active_blend,
                    );
                }
            }
            return;
        }
    }

    // Physical subpixel border snapping:
    // Snap outer coordinates and widths to integer device pixels so 1px borders
    // always rasterize cleanly without fractional blur or gaps.
    let snap_x = x.round();
    let snap_y = y.round();
    let snap_r = (x + w).round();
    let snap_b = (y + h).round();
    let snap_w = (snap_r - snap_x).max(0.0);
    let snap_h = (snap_b - snap_y).max(0.0);

    let snap_top = if widths.top > 0.0 {
        widths.top.round().max(1.0)
    } else {
        0.0
    };
    let snap_bottom = if widths.bottom > 0.0 {
        widths.bottom.round().max(1.0)
    } else {
        0.0
    };
    let snap_left = if widths.left > 0.0 {
        widths.left.round().max(1.0)
    } else {
        0.0
    };
    let snap_right = if widths.right > 0.0 {
        widths.right.round().max(1.0)
    } else {
        0.0
    };

    // Top border
    if snap_top > 0.0 {
        let top_rect = intersect_rect(active_clip, Rect::new(snap_x, snap_y, snap_w, snap_top));
        if top_rect.width() > 0.0 && top_rect.height() > 0.0 {
            fill_rect_mode(
                buffer,
                buf_width,
                buf_height,
                top_rect.x().round() as i32,
                top_rect.y().round() as i32,
                top_rect.width().round() as u32,
                top_rect.height().round() as u32,
                color,
                active_blend,
            );
        }
    }
    // Bottom border
    if snap_bottom > 0.0 {
        let bottom_rect = intersect_rect(
            active_clip,
            Rect::new(snap_x, snap_b - snap_bottom, snap_w, snap_bottom),
        );
        if bottom_rect.width() > 0.0 && bottom_rect.height() > 0.0 {
            fill_rect_mode(
                buffer,
                buf_width,
                buf_height,
                bottom_rect.x().round() as i32,
                bottom_rect.y().round() as i32,
                bottom_rect.width().round() as u32,
                bottom_rect.height().round() as u32,
                color,
                active_blend,
            );
        }
    }
    // Left border
    if snap_left > 0.0 {
        let side_y = snap_y + snap_top;
        let side_h = (snap_h - snap_top - snap_bottom).max(0.0);
        let left_rect = intersect_rect(active_clip, Rect::new(snap_x, side_y, snap_left, side_h));
        if left_rect.width() > 0.0 && left_rect.height() > 0.0 {
            fill_rect_mode(
                buffer,
                buf_width,
                buf_height,
                left_rect.x().round() as i32,
                left_rect.y().round() as i32,
                left_rect.width().round() as u32,
                left_rect.height().round() as u32,
                color,
                active_blend,
            );
        }
    }
    // Right border
    if snap_right > 0.0 {
        let side_y = snap_y + snap_top;
        let side_h = (snap_h - snap_top - snap_bottom).max(0.0);
        let right_rect = intersect_rect(
            active_clip,
            Rect::new(snap_r - snap_right, side_y, snap_right, side_h),
        );
        if right_rect.width() > 0.0 && right_rect.height() > 0.0 {
            fill_rect_mode(
                buffer,
                buf_width,
                buf_height,
                right_rect.x().round() as i32,
                right_rect.y().round() as i32,
                right_rect.width().round() as u32,
                right_rect.height().round() as u32,
                color,
                active_blend,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_border_image(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    pixels: &[u32],
    img_w: u32,
    img_h: u32,
    slice: [f32; 4],
    widths: BorderWidths,
    repeat_h: mango_css::values::BorderImageRepeat,
    repeat_v: mango_css::values::BorderImageRepeat,
    fill: bool,
    clip: Rect,
    transform: [f32; 6],
) {
    if img_w == 0 || img_h == 0 || pixels.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0
    {
        return;
    }
    let s_top = slice[0].clamp(0.0, img_h as f32);
    let s_right = slice[1].clamp(0.0, img_w as f32);
    let s_bottom = slice[2].clamp(0.0, img_h as f32);
    let s_left = slice[3].clamp(0.0, img_w as f32);
    let s_mid_w = (img_w as f32 - s_left - s_right).max(0.0);
    let s_mid_h = (img_h as f32 - s_top - s_bottom).max(0.0);

    let d_x = rect.x();
    let d_y = rect.y();
    let d_w = rect.width();
    let d_h = rect.height();
    let d_top = widths.top.min(d_h * 0.5);
    let d_right = widths.right.min(d_w * 0.5);
    let d_bottom = widths.bottom.min(d_h * 0.5);
    let d_left = widths.left.min(d_w * 0.5);
    let d_mid_w = (d_w - d_left - d_right).max(0.0);
    let d_mid_h = (d_h - d_top - d_bottom).max(0.0);

    let slices = [
        // 1. Top-left corner
        (
            Rect::new(d_x, d_y, d_left, d_top),
            Rect::new(0.0, 0.0, s_left, s_top),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 2. Top-right corner
        (
            Rect::new(d_x + d_w - d_right, d_y, d_right, d_top),
            Rect::new(img_w as f32 - s_right, 0.0, s_right, s_top),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 3. Bottom-right corner
        (
            Rect::new(d_x + d_w - d_right, d_y + d_h - d_bottom, d_right, d_bottom),
            Rect::new(
                img_w as f32 - s_right,
                img_h as f32 - s_bottom,
                s_right,
                s_bottom,
            ),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 4. Bottom-left corner
        (
            Rect::new(d_x, d_y + d_h - d_bottom, d_left, d_bottom),
            Rect::new(0.0, img_h as f32 - s_bottom, s_left, s_bottom),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 5. Top edge
        (
            Rect::new(d_x + d_left, d_y, d_mid_w, d_top),
            Rect::new(s_left, 0.0, s_mid_w, s_top),
            repeat_h,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 6. Bottom edge
        (
            Rect::new(d_x + d_left, d_y + d_h - d_bottom, d_mid_w, d_bottom),
            Rect::new(s_left, img_h as f32 - s_bottom, s_mid_w, s_bottom),
            repeat_h,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 7. Left edge
        (
            Rect::new(d_x, d_y + d_top, d_left, d_mid_h),
            Rect::new(0.0, s_top, s_left, s_mid_h),
            mango_css::values::BorderImageRepeat::Stretch,
            repeat_v,
        ),
        // 8. Right edge
        (
            Rect::new(d_x + d_w - d_right, d_y + d_top, d_right, d_mid_h),
            Rect::new(img_w as f32 - s_right, s_top, s_right, s_mid_h),
            mango_css::values::BorderImageRepeat::Stretch,
            repeat_v,
        ),
    ];

    for (d_rect, s_rect, rep_h, rep_v) in slices {
        if d_rect.width() > 0.0
            && d_rect.height() > 0.0
            && s_rect.width() > 0.0
            && s_rect.height() > 0.0
        {
            blit_border_slice(
                buffer, buf_width, buf_height, &d_rect, pixels, img_w, img_h, &s_rect, rep_h,
                rep_v, clip, transform,
            );
        }
    }

    if fill && d_mid_w > 0.0 && d_mid_h > 0.0 && s_mid_w > 0.0 && s_mid_h > 0.0 {
        let center_d = Rect::new(d_x + d_left, d_y + d_top, d_mid_w, d_mid_h);
        let center_s = Rect::new(s_left, s_top, s_mid_w, s_mid_h);
        blit_border_slice(
            buffer, buf_width, buf_height, &center_d, pixels, img_w, img_h, &center_s, repeat_h,
            repeat_v, clip, transform,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_border_slice(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    dest: &Rect,
    pixels: &[u32],
    img_w: u32,
    img_h: u32,
    src: &Rect,
    rep_h: mango_css::values::BorderImageRepeat,
    rep_v: mango_css::values::BorderImageRepeat,
    clip: Rect,
    transform: [f32; 6],
) {
    let bounds = intersect_rect(clip, transformed_bounds(dest, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };

    let x_start = bounds.x().max(0.0) as u32;
    let y_start = bounds.y().max(0.0) as u32;
    let x_end = ((bounds.x() + bounds.width()) as u32).min(buf_width);
    let y_end = ((bounds.y() + bounds.height()) as u32).min(buf_height);

    for py in y_start..y_end {
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let (dx, dy) = apply_matrix(inv, fx, fy);
            if dx < dest.x()
                || dx >= dest.x() + dest.width()
                || dy < dest.y()
                || dy >= dest.y() + dest.height()
            {
                continue;
            }
            let norm_x = (dx - dest.x()) / dest.width().max(1.0);
            let norm_y = (dy - dest.y()) / dest.height().max(1.0);

            let sx = match rep_h {
                mango_css::values::BorderImageRepeat::Stretch => src.x() + norm_x * src.width(),
                _ => src.x() + ((dx - dest.x()) % src.width().max(1.0)),
            };
            let sy = match rep_v {
                mango_css::values::BorderImageRepeat::Stretch => src.y() + norm_y * src.height(),
                _ => src.y() + ((dy - dest.y()) % src.height().max(1.0)),
            };

            let ix = (sx as u32).min(img_w.saturating_sub(1));
            let iy = (sy as u32).min(img_h.saturating_sub(1));
            let pix = pixels[(iy * img_w + ix) as usize];
            let a = effective_alpha(pix) as u8;
            if a == 0 {
                continue;
            }
            let idx = row + px as usize;
            if idx < buffer.len() {
                let r = ((pix >> 16) & 0xFF) as u8;
                let g = ((pix >> 8) & 0xFF) as u8;
                let b = (pix & 0xFF) as u8;
                buffer[idx] = blend_pixel(Color::rgba(r, g, b, a), buffer[idx]);
            }
        }
    }
}
