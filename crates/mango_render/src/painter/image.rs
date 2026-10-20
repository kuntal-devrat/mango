//! Image sampling and blitting routines.

use mango_core::{Color, Rect};

use crate::painter::affine::effective_alpha;
use crate::painter::blend::blend_pixel_mode;

/// Blits decoded image pixel data into the framebuffer with alpha blending.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blit_image(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    img_width: u32,
    img_height: u32,
    pixels: &[u32],
    clip: Rect,
    blend_mode: mango_css::values::BlendMode,
) {
    let cx1 = clip.x().max(0.0) as i32;
    let cy1 = clip.y().max(0.0) as i32;
    let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
    let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

    let is_exact = pixels.len() == (img_width * img_height) as usize;
    let (src_w, src_h) = if is_exact || img_height == 0 || img_width == 0 {
        (img_width, img_height)
    } else {
        let aspect = img_width as f32 / img_height as f32;
        let h = ((pixels.len() as f32 / aspect).sqrt()).round().max(1.0) as u32;
        let w = (pixels.len() as u32 / h).max(1);
        (w, h)
    };

    for iy in 0..img_height {
        let dest_y = y + iy as i32;
        if dest_y < cy1 || dest_y >= cy2 {
            continue;
        }

        for ix in 0..img_width {
            let dest_x = x + ix as i32;
            if dest_x < cx1 || dest_x >= cx2 {
                continue;
            }

            let pixel = if is_exact {
                let src_idx = (iy * img_width + ix) as usize;
                if src_idx >= pixels.len() {
                    continue;
                }
                pixels[src_idx]
            } else {
                let sx = (ix as f32 + 0.5) * (src_w as f32 / img_width as f32) - 0.5;
                let sy = (iy as f32 + 0.5) * (src_h as f32 / img_height as f32) - 0.5;
                sample_bilinear(pixels, src_w, src_h, sx, sy)
            };

            let alpha = effective_alpha(pixel);
            let dest_idx = (dest_y as u32 * buf_width + dest_x as u32) as usize;
            if dest_idx >= buffer.len() {
                continue;
            }

            if alpha == 0 {
                continue;
            }
            let c = Color::rgba(
                ((pixel >> 16) & 0xFF) as u8,
                ((pixel >> 8) & 0xFF) as u8,
                (pixel & 0xFF) as u8,
                alpha as u8,
            );
            if alpha == 255 && blend_mode == mango_css::values::BlendMode::Normal {
                buffer[dest_idx] = pixel & 0x00FFFFFF;
            } else {
                buffer[dest_idx] = blend_pixel_mode(c, buffer[dest_idx], blend_mode);
            }
        }
    }
}

/// Samples pixel data at floating point coordinate (sx, sy) using bilinear interpolation.
#[inline]
pub(crate) fn sample_bilinear(pixels: &[u32], src_w: u32, src_h: u32, sx: f32, sy: f32) -> u32 {
    let w = src_w as usize;
    let h = src_h as usize;
    if w == 0 || h == 0 || pixels.is_empty() {
        return 0;
    }
    let sx_clamped = sx.clamp(0.0, (w.saturating_sub(1)) as f32);
    let sy_clamped = sy.clamp(0.0, (h.saturating_sub(1)) as f32);
    let x0 = sx_clamped.floor() as usize;
    let y0 = sy_clamped.floor() as usize;
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let fx = sx_clamped - x0 as f32;
    let fy = sy_clamped - y0 as f32;

    let p00 = pixels.get(y0 * w + x0).copied().unwrap_or(0);
    let p10 = pixels.get(y0 * w + x1).copied().unwrap_or(0);
    let p01 = pixels.get(y1 * w + x0).copied().unwrap_or(0);
    let p11 = pixels.get(y1 * w + x1).copied().unwrap_or(0);

    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    let eff_a = |px: u32| -> f32 {
        let a = ((px >> 24) & 0xFF) as u32;
        if a == 0 && (px & 0x00FF_FFFF) != 0 {
            255.0
        } else {
            a as f32
        }
    };
    let a00 = eff_a(p00);
    let a10 = eff_a(p10);
    let a01 = eff_a(p01);
    let a11 = eff_a(p11);

    let a = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
    let out_a = a.clamp(0.0, 255.0).round() as u32;

    let has_alpha = a00 > 0.0 || a10 > 0.0 || a01 > 0.0 || a11 > 0.0;
    let alpha_factor = if has_alpha {
        if out_a == 0 {
            return 0;
        }
        out_a as f32 / 255.0
    } else {
        1.0
    };

    let r00 = ((p00 >> 16) & 0xFF) as f32 * if has_alpha { a00 / 255.0 } else { 1.0 };
    let r10 = ((p10 >> 16) & 0xFF) as f32 * if has_alpha { a10 / 255.0 } else { 1.0 };
    let r01 = ((p01 >> 16) & 0xFF) as f32 * if has_alpha { a01 / 255.0 } else { 1.0 };
    let r11 = ((p11 >> 16) & 0xFF) as f32 * if has_alpha { a11 / 255.0 } else { 1.0 };
    let pr = r00 * w00 + r10 * w10 + r01 * w01 + r11 * w11;

    let g00 = ((p00 >> 8) & 0xFF) as f32 * if has_alpha { a00 / 255.0 } else { 1.0 };
    let g10 = ((p10 >> 8) & 0xFF) as f32 * if has_alpha { a10 / 255.0 } else { 1.0 };
    let g01 = ((p01 >> 8) & 0xFF) as f32 * if has_alpha { a01 / 255.0 } else { 1.0 };
    let g11 = ((p11 >> 8) & 0xFF) as f32 * if has_alpha { a11 / 255.0 } else { 1.0 };
    let pg = g00 * w00 + g10 * w10 + g01 * w01 + g11 * w11;

    let b00 = (p00 & 0xFF) as f32 * if has_alpha { a00 / 255.0 } else { 1.0 };
    let b10 = (p10 & 0xFF) as f32 * if has_alpha { a10 / 255.0 } else { 1.0 };
    let b01 = (p01 & 0xFF) as f32 * if has_alpha { a01 / 255.0 } else { 1.0 };
    let b11 = (p11 & 0xFF) as f32 * if has_alpha { a11 / 255.0 } else { 1.0 };
    let pb = b00 * w00 + b10 * w10 + b01 * w01 + b11 * w11;

    let out_r = ((pr / alpha_factor).clamp(0.0, 255.0).round() as u32).min(255);
    let out_g = ((pg / alpha_factor).clamp(0.0, 255.0).round() as u32).min(255);
    let out_b = ((pb / alpha_factor).clamp(0.0, 255.0).round() as u32).min(255);

    if has_alpha {
        (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b
    } else {
        (out_r << 16) | (out_g << 8) | out_b
    }
}
