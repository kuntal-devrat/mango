//! Pixel blending and fill utilities for the painter.

use mango_core::Color;

/// Blends a source color with a destination pixel using integer math.
#[inline]
pub fn blend_pixel(src: Color, dst_rgb: u32) -> u32 {
    blend_pixel_mode(src, dst_rgb, mango_css::values::BlendMode::Normal)
}

/// Blends a source color with a destination pixel using CSS blend modes.
pub fn blend_pixel_mode(src: Color, dst_rgb: u32, mode: mango_css::values::BlendMode) -> u32 {
    use mango_css::values::BlendMode;
    if mode == BlendMode::Normal {
        let alpha = src.a as u32;
        if alpha == 255 {
            return src.to_rgb_u32();
        }
        if alpha == 0 {
            return dst_rgb;
        }
        let dst_r = (dst_rgb >> 16) & 0xFF;
        let dst_g = (dst_rgb >> 8) & 0xFF;
        let dst_b = dst_rgb & 0xFF;
        let src_r = src.r as u32;
        let src_g = src.g as u32;
        let src_b = src.b as u32;
        let inv_a = 255 - alpha;
        let out_r = (src_r * alpha + dst_r * inv_a + 127) / 255;
        let out_g = (src_g * alpha + dst_g * inv_a + 127) / 255;
        let out_b = (src_b * alpha + dst_b * inv_a + 127) / 255;
        return (out_r << 16) | (out_g << 8) | out_b;
    }

    let alpha = src.a as f32 / 255.0;
    if alpha <= 0.0 {
        return dst_rgb;
    }

    let cb_r = ((dst_rgb >> 16) & 0xFF) as f32 / 255.0;
    let cb_g = ((dst_rgb >> 8) & 0xFF) as f32 / 255.0;
    let cb_b = (dst_rgb & 0xFF) as f32 / 255.0;

    let cs_r = src.r as f32 / 255.0;
    let cs_g = src.g as f32 / 255.0;
    let cs_b = src.b as f32 / 255.0;

    let blend_ch = |cb: f32, cs: f32| -> f32 {
        match mode {
            BlendMode::Multiply => cb * cs,
            BlendMode::Screen => cb + cs - cb * cs,
            BlendMode::Overlay => {
                if cb <= 0.5 {
                    2.0 * cb * cs
                } else {
                    1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
                }
            }
            BlendMode::Darken => cb.min(cs),
            BlendMode::Lighten => cb.max(cs),
            BlendMode::ColorDodge => {
                if cb <= 0.0 {
                    0.0
                } else if cs >= 1.0 {
                    1.0
                } else {
                    (cb / (1.0 - cs)).min(1.0)
                }
            }
            BlendMode::ColorBurn => {
                if cb >= 1.0 {
                    1.0
                } else if cs <= 0.0 {
                    0.0
                } else {
                    (1.0 - (1.0 - cb) / cs).max(0.0)
                }
            }
            BlendMode::HardLight => {
                if cs <= 0.5 {
                    2.0 * cb * cs
                } else {
                    1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
                }
            }
            BlendMode::SoftLight => {
                if cs <= 0.5 {
                    cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
                } else {
                    let d = if cb <= 0.25 {
                        ((16.0 * cb - 12.0) * cb + 4.0) * cb
                    } else {
                        cb.sqrt()
                    };
                    cb + (2.0 * cs - 1.0) * (d - cb)
                }
            }
            BlendMode::Difference => (cb - cs).abs(),
            BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
            _ => cs,
        }
    };

    let cm_r = blend_ch(cb_r, cs_r);
    let cm_g = blend_ch(cb_g, cs_g);
    let cm_b = blend_ch(cb_b, cs_b);

    let cr_r = ((1.0 - alpha) * cb_r + alpha * cm_r).clamp(0.0, 1.0);
    let cr_g = ((1.0 - alpha) * cb_g + alpha * cm_g).clamp(0.0, 1.0);
    let cr_b = ((1.0 - alpha) * cb_b + alpha * cm_b).clamp(0.0, 1.0);

    let out_r = (cr_r * 255.0).round() as u32;
    let out_g = (cr_g * 255.0).round() as u32;
    let out_b = (cr_b * 255.0).round() as u32;

    (out_r << 16) | (out_g << 8) | out_b
}

/// Fill a rectangle in the pixel buffer with default blend mode.
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn fill_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    fill_rect_mode(
        buffer,
        buf_width,
        buf_height,
        x,
        y,
        w,
        h,
        color,
        mango_css::values::BlendMode::Normal,
    );
}

/// Fill a rectangle in the pixel buffer with a specified CSS blend mode.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_rect_mode(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
    blend_mode: mango_css::values::BlendMode,
) {
    if color.a == 0 || w == 0 || h == 0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let solid_pixel = color.to_rgb_u32();
    let has_alpha = color.a < 255 || blend_mode != mango_css::values::BlendMode::Normal;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x.saturating_add(w as i32)).clamp(0, buf_width as i32);
    let y1 = (y.saturating_add(h as i32)).clamp(0, buf_height as i32);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;

    for py in y_start..y_end {
        let row_offset = (py * buf_width) as usize;
        for px in x_start..x_end {
            let idx = row_offset + px as usize;
            if idx < buffer.len() {
                if has_alpha {
                    buffer[idx] = blend_pixel_mode(color, buffer[idx], blend_mode);
                } else {
                    buffer[idx] = solid_pixel;
                }
            }
        }
    }
}
