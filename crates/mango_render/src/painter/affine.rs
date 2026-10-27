//! 2D affine transform matrix math and transformed shape/image rasterization.

use mango_core::Color;

use crate::painter::blend::blend_pixel_mode;
use crate::painter::image::sample_bilinear;
use crate::painter::shapes::{intersect_rect, is_inside_rounded_rect};

/// The identity 2D affine matrix `matrix(1, 0, 0, 1, 0, 0)`.
pub(crate) const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Returns true when `m` is (numerically) the identity transform.
#[inline]
pub(crate) fn is_identity(m: [f32; 6]) -> bool {
    (m[0] - 1.0).abs() < 1e-4
        && m[1].abs() < 1e-4
        && m[2].abs() < 1e-4
        && (m[3] - 1.0).abs() < 1e-4
        && m[4].abs() < 1e-4
        && m[5].abs() < 1e-4
}

/// Returns Some((tx, ty)) when `m` is a pure 2D translation without rotation, scale, or shear.
#[inline]
pub(crate) fn is_pure_translation(m: [f32; 6]) -> Option<(f32, f32)> {
    if (m[0] - 1.0).abs() < 1e-4
        && m[1].abs() < 1e-4
        && m[2].abs() < 1e-4
        && (m[3] - 1.0).abs() < 1e-4
    {
        Some((m[4], m[5]))
    } else {
        None
    }
}

/// Composes two affine matrices: `outer * inner` (apply `inner` first).
#[inline]
pub(crate) fn concat_matrix(outer: [f32; 6], inner: [f32; 6]) -> [f32; 6] {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

/// Maps a point through an affine matrix.
#[inline]
pub(crate) fn apply_matrix(m: [f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// Computes the inverse of an affine matrix, or `None` when it is singular.
pub fn invert_matrix(m: [f32; 6]) -> Option<[f32; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-6 {
        return None;
    }
    let inv_det = 1.0 / det;
    let a = m[3] * inv_det;
    let b = -m[1] * inv_det;
    let c = -m[2] * inv_det;
    let d = m[0] * inv_det;
    let e = -(a * m[4] + c * m[5]);
    let f = -(b * m[4] + d * m[5]);
    Some([a, b, c, d, e, f])
}

/// Approximates the uniform scale factor of a matrix (geometric mean of axes).
pub(crate) fn matrix_scale(m: [f32; 6]) -> f32 {
    let det = (m[0] * m[3] - m[1] * m[2]).abs();
    det.sqrt().max(0.0001)
}

/// Axis-aligned bounding box of a transformed rectangle.
pub(crate) fn transformed_bounds(rect: &mango_core::Rect, m: [f32; 6]) -> mango_core::Rect {
    let corners = [
        (rect.x(), rect.y()),
        (rect.x() + rect.width(), rect.y()),
        (rect.x(), rect.y() + rect.height()),
        (rect.x() + rect.width(), rect.y() + rect.height()),
    ];
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (cx, cy) in corners {
        let (tx, ty) = apply_matrix(m, cx, cy);
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
        max_x = max_x.max(tx);
        max_y = max_y.max(ty);
    }
    mango_core::Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

/// Fills a rectangle through an affine transform, optionally clipped to rounded corners.
///
/// Uses inverse mapping: every destination pixel inside the transformed quad tests
/// its source coordinate against the original rectangle (and corner radii).
/// Respects the active CSS `blend_mode` for all writes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_rect_transformed(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    color: Color,
    radii: Option<[f32; 4]>,
    clip: mango_core::Rect,
    transform: [f32; 6],
    blend_mode: mango_css::values::BlendMode,
) {
    if color.a == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };
    let bounds = intersect_rect(clip, transformed_bounds(rect, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let x0 = bounds.x().max(0.0).floor() as i32;
    let y0 = bounds.y().max(0.0).floor() as i32;
    let x1 = ((bounds.x() + bounds.width()).ceil() as i32).clamp(0, buf_width as i32);
    let y1 = ((bounds.y() + bounds.height()).ceil() as i32).clamp(0, buf_height as i32);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;
    let e = 0.5;
    for py in y_start..y_end {
        let fy = py as f32 + 0.5;
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let (sx, sy) = apply_matrix(inv, fx, fy);
            // Sample with a half-pixel tolerance so transformed edges stay solid.
            if sx < rect.x() - e || sx > rect.x() + rect.width() + e {
                continue;
            }
            if sy < rect.y() - e || sy > rect.y() + rect.height() + e {
                continue;
            }
            if let Some(r) = radii
                && !is_inside_rounded_rect(sx, sy, rect, r)
            {
                continue;
            }
            let idx = row + px as usize;
            if idx < buffer.len() {
                if color.a == 255 && blend_mode == mango_css::values::BlendMode::Normal {
                    buffer[idx] = color.to_rgb_u32();
                } else {
                    buffer[idx] = blend_pixel_mode(color, buffer[idx], blend_mode);
                }
            }
        }
    }
}

/// Returns the effective alpha of a `0xAARRGGBB` pixel, treating legacy
/// `0x00RRGGBB` pixels (alpha byte 0 with non-zero RGB) as fully opaque.
///
/// Framebuffers historically stored `0x00RRGGBB`; image/canvases store
/// canonical `0xAARRGGBB`. This helper unifies both so legacy opaque
/// pixels are never mistaken for transparent.
#[inline]
pub(crate) fn effective_alpha(pixel: u32) -> u32 {
    let a = (pixel >> 24) & 0xFF;
    if a == 0 && (pixel & 0x00FF_FFFF) != 0 {
        255
    } else {
        a
    }
}

/// Blits decoded image pixels through an affine transform using nearest-neighbour mapping.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blit_image_transformed(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: f32,
    y: f32,
    img_width: f32,
    img_height: f32,
    pixels: &[u32],
    clip: mango_core::Rect,
    transform: [f32; 6],
    blend_mode: mango_css::values::BlendMode,
) {
    if img_width <= 0.0 || img_height <= 0.0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };
    let quad = mango_core::Rect::new(x, y, img_width, img_height);
    let bounds = intersect_rect(clip, transformed_bounds(&quad, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let src_stride = img_width.max(1.0) as usize;

    let x0 = bounds.x().max(0.0).floor() as i32;
    let y0 = bounds.y().max(0.0).floor() as i32;
    let x1 = ((bounds.x() + bounds.width()).ceil() as i32).clamp(0, buf_width as i32);
    let y1 = ((bounds.y() + bounds.height()).ceil() as i32).clamp(0, buf_height as i32);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;

    for py in y_start..y_end {
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let (sx, sy) = apply_matrix(inv, fx, fy);
            if sx < x || sx >= x + img_width || sy < y || sy >= y + img_height {
                continue;
            }
            let u = ((sx - x) / img_width).clamp(0.0, 0.9999);
            let v = ((sy - y) / img_height).clamp(0.0, 0.9999);
            let samp_x = u * (src_stride as f32);
            let samp_y = v * img_height;
            let pixel = sample_bilinear(
                pixels,
                src_stride as u32,
                img_height.max(1.0) as u32,
                samp_x,
                samp_y,
            );
            let alpha = effective_alpha(pixel);
            if alpha == 0 {
                continue;
            }
            let idx = row + px as usize;
            if idx >= buffer.len() {
                continue;
            }
            let c = Color::rgba(
                ((pixel >> 16) & 0xFF) as u8,
                ((pixel >> 8) & 0xFF) as u8,
                (pixel & 0xFF) as u8,
                alpha as u8,
            );
            if alpha == 255 && blend_mode == mango_css::values::BlendMode::Normal {
                buffer[idx] = pixel & 0x00FFFFFF;
            } else {
                buffer[idx] = blend_pixel_mode(c, buffer[idx], blend_mode);
            }
        }
    }
}
