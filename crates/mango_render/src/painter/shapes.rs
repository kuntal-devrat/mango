//! Vector shape rasterization: rounded rectangles, SDF coverage, lines, gradients.

use mango_core::Color;

use crate::painter::affine::{apply_matrix, invert_matrix, transformed_bounds};
use crate::painter::blend::{blend_pixel, blend_pixel_mode, fill_rect_mode};

/// Intersects two rectangles, returning a rectangle with zero width/height if they do not overlap.
#[inline]
pub(crate) fn intersect_rect(a: mango_core::Rect, b: mango_core::Rect) -> mango_core::Rect {
    let x1 = a.x().max(b.x());
    let y1 = a.y().max(b.y());
    let x2 = (a.x() + a.width()).min(b.x() + b.width());
    let y2 = (a.y() + a.height()).min(b.y() + b.height());
    let w = (x2 - x1).max(0.0);
    let h = (y2 - y1).max(0.0);
    mango_core::Rect::new(x1, y1, w, h)
}

/// Normalizes corner radii per CSS Backgrounds 3 §5.2: when the sum of any
/// two adjacent radii exceeds the corresponding side length, all four radii
/// are scaled proportionally by `min(1, available / sum)` so the shape stays valid.
pub(crate) fn normalize_radii(w: f32, h: f32, radii: [f32; 4]) -> [f32; 4] {
    let r = [
        radii[0].max(0.0),
        radii[1].max(0.0),
        radii[2].max(0.0),
        radii[3].max(0.0),
    ];
    if w <= 0.0 || h <= 0.0 {
        return [0.0; 4];
    }
    let mut scale: f32 = 1.0;
    let top = r[0] + r[1];
    if top > w && top > 0.0 {
        scale = scale.min(w / top);
    }
    let bottom = r[2] + r[3];
    if bottom > w && bottom > 0.0 {
        scale = scale.min(w / bottom);
    }
    let left = r[3] + r[0];
    if left > h && left > 0.0 {
        scale = scale.min(h / left);
    }
    let right = r[1] + r[2];
    if right > h && right > 0.0 {
        scale = scale.min(h / right);
    }
    [r[0] * scale, r[1] * scale, r[2] * scale, r[3] * scale]
}

/// Evaluates the signed distance field (SDF) of a point (fx, fy) relative to a rounded rectangle.
/// Negative distance is inside, zero is boundary, positive is outside.
pub(crate) fn sdf_rounded_rect(fx: f32, fy: f32, rect: &mango_core::Rect, radii: [f32; 4]) -> f32 {
    let rx = rect.x();
    let ry = rect.y();
    let rw = rect.width();
    let rh = rect.height();
    if rw <= 0.0 || rh <= 0.0 {
        return 1e6;
    }
    let n = normalize_radii(rw, rh, radii);
    let r_tl = n[0];
    let r_tr = n[1];
    let r_br = n[2];
    let r_bl = n[3];

    let mid_x = rx + rw * 0.5;
    let mid_y = ry + rh * 0.5;

    if fx < mid_x {
        if fy < mid_y {
            // Top-left quadrant
            let dx = (rx + r_tl) - fx;
            let dy = (ry + r_tl) - fy;
            if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt() - r_tl
            } else {
                dx.max(dy) - r_tl
            }
        } else {
            // Bottom-left quadrant
            let dx = (rx + r_bl) - fx;
            let dy = fy - (ry + rh - r_bl);
            if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt() - r_bl
            } else {
                dx.max(dy) - r_bl
            }
        }
    } else if fy < mid_y {
        // Top-right quadrant
        let dx = fx - (rx + rw - r_tr);
        let dy = (ry + r_tr) - fy;
        if dx > 0.0 && dy > 0.0 {
            (dx * dx + dy * dy).sqrt() - r_tr
        } else {
            dx.max(dy) - r_tr
        }
    } else {
        // Bottom-right quadrant
        let dx = fx - (rx + rw - r_br);
        let dy = fy - (ry + rh - r_br);
        if dx > 0.0 && dy > 0.0 {
            (dx * dx + dy * dy).sqrt() - r_br
        } else {
            dx.max(dy) - r_br
        }
    }
}

pub fn is_inside_rounded_rect(fx: f32, fy: f32, rect: &mango_core::Rect, radii: [f32; 4]) -> bool {
    sdf_rounded_rect(fx, fy, rect, radii) <= 0.0
}

/// Fill a rounded rectangle in the pixel buffer, optionally clipping out an inner region.
/// Uses analytical SDF subpixel coverage anti-aliasing so thin 1px/2px borders and curves
/// render continuous, unbroken lines.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_rounded_rect_clipped(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    color: Color,
    radii: [f32; 4], // [top-left, top-right, bottom-right, bottom-left]
    clip_out: Option<(&mango_core::Rect, [f32; 4])>,
    clip: mango_core::Rect,
    blend_mode: mango_css::values::BlendMode,
) {
    if color.a == 0 {
        return;
    }
    let rx = rect.x();
    let ry = rect.y();
    let rw = rect.width();
    let rh = rect.height();

    if rw <= 0.0 || rh <= 0.0 {
        return;
    }

    let solid_pixel = color.to_rgb_u32();
    let needs_blend = color.a < 255 || blend_mode != mango_css::values::BlendMode::Normal;

    let cx1 = clip.x().max(0.0) as i32;
    let cy1 = clip.y().max(0.0) as i32;
    let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
    let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

    let x_start = ((rx - 1.0) as i32).max(cx1).max(0) as u32;
    let y_start = ((ry - 1.0) as i32).max(cy1).max(0) as u32;
    let x_end = ((((rx + rw + 1.0).ceil()) as i32).min(cx2).max(0) as u32).min(buf_width);
    let y_end = ((((ry + rh + 1.0).ceil()) as i32).min(cy2).max(0) as u32).min(buf_height);

    let has_clip_out = clip_out.is_some();
    let (clip_rect, clip_radii) = clip_out.unwrap_or((rect, [0.0; 4]));

    for py in y_start..y_end {
        let fy = py as f32 + 0.5;
        let row_offset = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;

            let d_out = sdf_rounded_rect(fx, fy, rect, radii);
            let cov_out = (0.5 - d_out).clamp(0.0, 1.0);
            if cov_out <= 0.0 {
                continue;
            }

            let cov = if has_clip_out {
                let d_in = sdf_rounded_rect(fx, fy, clip_rect, clip_radii);
                let cov_in = (0.5 - d_in).clamp(0.0, 1.0);
                cov_out * (1.0 - cov_in)
            } else {
                cov_out
            };

            if cov <= 0.001 {
                continue;
            }

            let idx = row_offset + px as usize;
            if idx < buffer.len() {
                if cov >= 0.999 && !needs_blend {
                    buffer[idx] = solid_pixel;
                } else {
                    let eff_a = ((color.a as f32) * cov).round() as u8;
                    if eff_a > 0 {
                        let blended_color = Color::rgba(color.r, color.g, color.b, eff_a);
                        buffer[idx] = blend_pixel_mode(blended_color, buffer[idx], blend_mode);
                    }
                }
            }
        }
    }
}

/// Fill a rounded rectangle in the pixel buffer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_rounded_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    color: Color,
    radii: [f32; 4], // [top-left, top-right, bottom-right, bottom-left]
    clip: mango_core::Rect,
    blend_mode: mango_css::values::BlendMode,
) {
    let x0 = rect.x().round();
    let y0 = rect.y().round();
    let x1 = (rect.x() + rect.width()).round();
    let y1 = (rect.y() + rect.height()).round();
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let snapped = mango_core::Rect::new(x0, y0, w, h);

    if radii[0] == 0.0 && radii[1] == 0.0 && radii[2] == 0.0 && radii[3] == 0.0 {
        let r = intersect_rect(clip, snapped);
        if r.width() > 0.0 && r.height() > 0.0 {
            let rx0 = r.x().round() as i32;
            let ry0 = r.y().round() as i32;
            let rx1 = (r.x() + r.width()).round() as i32;
            let ry1 = (r.y() + r.height()).round() as i32;
            let rw = (rx1 - rx0).max(0) as u32;
            let rh = (ry1 - ry0).max(0) as u32;
            if rw > 0 && rh > 0 {
                fill_rect_mode(
                    buffer, buf_width, buf_height, rx0, ry0, rw, rh, color, blend_mode,
                );
            }
        }
        return;
    }
    fill_rounded_rect_clipped(
        buffer, buf_width, buf_height, &snapped, color, radii, None, clip, blend_mode,
    );
}

/// Rasterizes a thick line segment between two points with round-cap distance-field
/// coverage and the active blend mode.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_line_segment(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    thickness: f32,
    clip: mango_core::Rect,
    blend_mode: mango_css::values::BlendMode,
) {
    if color.a == 0 || thickness <= 0.0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let half = thickness * 0.5;
    let min_x = (x1.min(x2) - half - 1.0).max(clip.x()).max(0.0).floor() as i32;
    let min_y = (y1.min(y2) - half - 1.0).max(clip.y()).max(0.0).floor() as i32;
    let max_x = ((x1.max(x2) + half + 1.0)
        .min(clip.right())
        .min(buf_width as f32)
        .ceil() as i32)
        .clamp(0, buf_width as i32);
    let max_y = ((y1.max(y2) + half + 1.0)
        .min(clip.bottom())
        .min(buf_height as f32)
        .ceil() as i32)
        .clamp(0, buf_height as i32);
    if min_x >= max_x || min_y >= max_y {
        return;
    }
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len2 = dx * dx + dy * dy;
    let is_dot = len2 < 1e-8;
    for py in (min_y as u32)..(max_y as u32) {
        let fy = py as f32 + 0.5;
        let row = (py * buf_width) as usize;
        for px in (min_x as u32)..(max_x as u32) {
            let fx = px as f32 + 0.5;
            let dist = if is_dot {
                ((fx - x1) * (fx - x1) + (fy - y1) * (fy - y1)).sqrt()
            } else {
                let t = (((fx - x1) * dx + (fy - y1) * dy) / len2).clamp(0.0, 1.0);
                let cx = x1 + dx * t;
                let cy = y1 + dy * t;
                ((fx - cx) * (fx - cx) + (fy - cy) * (fy - cy)).sqrt()
            };
            let cov = (half + 0.5 - dist).clamp(0.0, 1.0);
            if cov <= 0.001 {
                continue;
            }
            let idx = row + px as usize;
            if idx >= buffer.len() {
                continue;
            }
            if cov >= 0.999 && color.a == 255 && blend_mode == mango_css::values::BlendMode::Normal
            {
                buffer[idx] = color.to_rgb_u32();
            } else {
                let eff_a = ((color.a as f32) * cov).round().clamp(0.0, 255.0) as u8;
                if eff_a == 0 {
                    continue;
                }
                let c = Color::rgba(color.r, color.g, color.b, eff_a);
                buffer[idx] = blend_pixel_mode(c, buffer[idx], blend_mode);
            }
        }
    }
}

/// Rasterizes a CSS gradient into the given rectangle, honoring rounded corners
/// and the active transform.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_gradient(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    gradient: &mango_css::values::Gradient,
    radii: [f32; 4],
    opacity: f32,
    clip: mango_core::Rect,
    transform: [f32; 6],
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || opacity <= 0.0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };
    let bounds = intersect_rect(clip, transformed_bounds(rect, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }

    let grad_w = rect.width().max(1.0) as u32;
    let grad_h = rect.height().max(1.0) as u32;
    let pixels = gradient.rasterize(grad_w, grad_h);
    if pixels.is_empty() {
        return;
    }
    let has_radii = radii.iter().any(|r| *r > 0.0);
    let alpha_scale = opacity.clamp(0.0, 1.0);

    let x_start = bounds.x().round().max(0.0) as u32;
    let y_start = bounds.y().round().max(0.0) as u32;
    let x_end = ((bounds.x() + bounds.width()).round() as u32).min(buf_width);
    let y_end = ((bounds.y() + bounds.height()).round() as u32).min(buf_height);

    for py in y_start..y_end {
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let (sx, sy) = apply_matrix(inv, fx, fy);
            if sx < rect.x() || sx >= rect.x() + rect.width() {
                continue;
            }
            if sy < rect.y() || sy >= rect.y() + rect.height() {
                continue;
            }
            if has_radii && !is_inside_rounded_rect(sx, sy, rect, radii) {
                continue;
            }
            let gx = ((sx - rect.x()) / rect.width() * grad_w as f32) as u32;
            let gy = ((sy - rect.y()) / rect.height() * grad_h as f32) as u32;
            let gx = gx.min(grad_w.saturating_sub(1));
            let gy = gy.min(grad_h.saturating_sub(1));
            let src = pixels[(gy * grad_w + gx) as usize];
            let mut color = Color::rgba(
                ((src >> 16) & 0xFF) as u8,
                ((src >> 8) & 0xFF) as u8,
                (src & 0xFF) as u8,
                ((src >> 24) & 0xFF) as u8,
            );
            if alpha_scale < 1.0 {
                color.a = ((color.a as f32) * alpha_scale).round() as u8;
            }
            if color.a == 0 {
                continue;
            }
            let idx = row + px as usize;
            if idx < buffer.len() {
                buffer[idx] = blend_pixel(color, buffer[idx]);
            }
        }
    }
}
