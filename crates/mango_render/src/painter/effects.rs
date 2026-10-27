//! Filter effects, backdrop filters, box blur, and CSS clip paths.

use mango_core::{Color, Rect};

use crate::painter::affine::effective_alpha;
use crate::painter::blend::blend_pixel;
use crate::painter::shapes::{intersect_rect, is_inside_rounded_rect};

pub(crate) fn snapshot_rect(
    buffer: &[u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
) -> Vec<u32> {
    let min_x = (rect.x().max(0.0) as u32).min(buf_width);
    let max_x = ((rect.x() + rect.width()).max(0.0) as u32).min(buf_width);
    let min_y = (rect.y().max(0.0) as u32).min(buf_height);
    let max_y = ((rect.y() + rect.height()).max(0.0) as u32).min(buf_height);
    let mut pixels = Vec::with_capacity(((max_x - min_x) * (max_y - min_y)) as usize);
    for y in min_y..max_y {
        for x in min_x..max_x {
            pixels.push(buffer[(y * buf_width + x) as usize]);
        }
    }
    pixels
}

pub fn is_point_inside_clip_path(
    clip_path: &mango_css::values::ClipPath,
    bounds: &Rect,
    px: f32,
    py: f32,
) -> bool {
    use mango_css::values::ClipPath;
    match clip_path {
        ClipPath::None => true,
        ClipPath::Circle {
            radius,
            center_x,
            center_y,
        } => {
            let r = match radius {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => {
                    (bounds.width().min(bounds.height())) * pct / 100.0
                }
                _ => 50.0,
            };
            let cx = bounds.x()
                + match center_x {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                    _ => bounds.width() / 2.0,
                };
            let cy = bounds.y()
                + match center_y {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                    _ => bounds.height() / 2.0,
                };
            let dx = px - cx;
            let dy = py - cy;
            dx * dx + dy * dy <= r * r
        }
        ClipPath::Ellipse {
            radius_x,
            radius_y,
            center_x,
            center_y,
        } => {
            let rx = match radius_x {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                _ => bounds.width() / 2.0,
            }
            .max(0.1);
            let ry = match radius_y {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                _ => bounds.height() / 2.0,
            }
            .max(0.1);
            let cx = bounds.x()
                + match center_x {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                    _ => bounds.width() / 2.0,
                };
            let cy = bounds.y()
                + match center_y {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                    _ => bounds.height() / 2.0,
                };
            let dx = (px - cx) / rx;
            let dy = (py - cy) / ry;
            dx * dx + dy * dy <= 1.0
        }
        ClipPath::Inset {
            top,
            right,
            bottom,
            left,
            round,
        } => {
            let t = match top {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                _ => 0.0,
            };
            let r = match right {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                _ => 0.0,
            };
            let b = match bottom {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                _ => 0.0,
            };
            let l = match left {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                _ => 0.0,
            };
            let min_x = bounds.x() + l;
            let max_x = bounds.x() + bounds.width() - r;
            let min_y = bounds.y() + t;
            let max_y = bounds.y() + bounds.height() - b;
            let rw = (max_x - min_x).max(0.0);
            let rh = (max_y - min_y).max(0.0);
            let inset_rect = Rect::new(min_x, min_y, rw, rh);
            if let Some(radii_lens) = round {
                let r0 = match radii_lens[0] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                let r1 = match radii_lens[1] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                let r2 = match radii_lens[2] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                let r3 = match radii_lens[3] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                is_inside_rounded_rect(px, py, &inset_rect, [r0, r1, r2, r3])
            } else {
                px >= min_x && px <= max_x && py >= min_y && py <= max_y
            }
        }
        ClipPath::Polygon(pts) => {
            if pts.len() < 3 {
                return true;
            }
            let resolved_pts: Vec<(f32, f32)> = pts
                .iter()
                .map(|(x_len, y_len)| {
                    let x = bounds.x()
                        + match x_len {
                            mango_css::values::Length::Px(p) => *p,
                            mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                            _ => 0.0,
                        };
                    let y = bounds.y()
                        + match y_len {
                            mango_css::values::Length::Px(p) => *p,
                            mango_css::values::Length::Percent(pct) => {
                                bounds.height() * pct / 100.0
                            }
                            _ => 0.0,
                        };
                    (x, y)
                })
                .collect();

            // Ray-casting even-odd point-in-polygon
            let mut inside = false;
            let n = resolved_pts.len();
            for i in 0..n {
                let (x1, y1) = resolved_pts[i];
                let (x2, y2) = resolved_pts[(i + 1) % n];
                if ((y1 > py) != (y2 > py)) && (px < (x2 - x1) * (py - y1) / (y2 - y1 + 1e-6) + x1)
                {
                    inside = !inside;
                }
            }
            inside
        }
        ClipPath::Url(_) => true,
    }
}

pub(crate) fn restore_outside_clip_path(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    clip: Rect,
    clip_path: &mango_css::values::ClipPath,
    snapshot: &[u32],
) {
    let orig_min_x = (rect.x().max(0.0) as u32).min(buf_width);
    let orig_max_x = ((rect.x() + rect.width()).max(0.0) as u32).min(buf_width);
    let orig_min_y = (rect.y().max(0.0) as u32).min(buf_height);
    let orig_max_y = ((rect.y() + rect.height()).max(0.0) as u32).min(buf_height);
    let snap_w = orig_max_x.saturating_sub(orig_min_x);
    if snap_w == 0 || snapshot.is_empty() {
        return;
    }

    let r = intersect_rect(clip, *rect);
    let min_x = (r.x().max(0.0) as u32).min(buf_width);
    let max_x = ((r.x() + r.width()).max(0.0) as u32).min(buf_width);
    let min_y = (r.y().max(0.0) as u32).min(buf_height);
    let max_y = ((r.y() + r.height()).max(0.0) as u32).min(buf_height);

    for y in min_y..max_y {
        for x in min_x..max_x {
            if x >= orig_min_x && x < orig_max_x && y >= orig_min_y && y < orig_max_y {
                let snap_idx = ((y - orig_min_y) * snap_w + (x - orig_min_x)) as usize;
                if snap_idx < snapshot.len()
                    && !is_point_inside_clip_path(clip_path, rect, x as f32, y as f32)
                {
                    buffer[(y * buf_width + x) as usize] = snapshot[snap_idx];
                }
            }
        }
    }
}

/// Applies CSS filters to a framebuffer region.
pub(crate) fn apply_filter_to_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    filters: &[mango_css::values::FilterFunction],
) {
    if filters.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let min_x = (rect.x().max(0.0) as u32).min(buf_width);
    let max_x = ((rect.x() + rect.width()).max(0.0) as u32).min(buf_width);
    let min_y = (rect.y().max(0.0) as u32).min(buf_height);
    let max_y = ((rect.y() + rect.height()).max(0.0) as u32).min(buf_height);
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    for filter in filters {
        match filter {
            mango_css::values::FilterFunction::Blur(radius) => {
                let r = (*radius as i32).max(0);
                if r > 0 {
                    let pad = (*radius * 1.5).ceil();
                    let b_min_x = ((rect.x() - pad).max(0.0) as u32).min(buf_width);
                    let b_max_x = ((rect.x() + rect.width() + pad).max(0.0) as u32).min(buf_width);
                    let b_min_y = ((rect.y() - pad).max(0.0) as u32).min(buf_height);
                    let b_max_y =
                        ((rect.y() + rect.height() + pad).max(0.0) as u32).min(buf_height);
                    if b_min_x < b_max_x && b_min_y < b_max_y {
                        let b_w = (b_max_x - b_min_x) as usize;
                        let b_h = (b_max_y - b_min_y) as usize;
                        box_blur_rect(buffer, buf_width, b_min_x, b_min_y, b_w, b_h, r);
                    }
                }
            }
            mango_css::values::FilterFunction::Brightness(factor) => {
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = (((p >> 16) & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
                        let g = (((p >> 8) & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
                        let b = ((p & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (r << 16) | (g << 8) | b;
                    }
                }
            }
            mango_css::values::FilterFunction::Contrast(factor) => {
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((((p >> 16) & 0xff) as f32 - 128.0) * factor + 128.0)
                            .clamp(0.0, 255.0) as u32;
                        let g = ((((p >> 8) & 0xff) as f32 - 128.0) * factor + 128.0)
                            .clamp(0.0, 255.0) as u32;
                        let b =
                            (((p & 0xff) as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (r << 16) | (g << 8) | b;
                    }
                }
            }
            mango_css::values::FilterFunction::Grayscale(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                        let out_r = (r + (gray - r) * f).round() as u32;
                        let out_g = (g + (gray - g) * f).round() as u32;
                        let out_b = (b + (gray - b) * f).round() as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::Invert(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let out_r = (r + ((255.0 - r) - r) * f).round() as u32;
                        let out_g = (g + ((255.0 - g) - g) * f).round() as u32;
                        let out_b = (b + ((255.0 - b) - b) * f).round() as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::Opacity(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let a = (((p >> 24) & 0xff) as f32 * f).round() as u32;
                        buffer[idx] = (a << 24) | (p & 0x00ffffff);
                    }
                }
            }
            mango_css::values::FilterFunction::Saturate(factor) => {
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                        let out_r = (gray + (r - gray) * factor).clamp(0.0, 255.0) as u32;
                        let out_g = (gray + (g - gray) * factor).clamp(0.0, 255.0) as u32;
                        let out_b = (gray + (b - gray) * factor).clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::Sepia(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let sr = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0);
                        let sg = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0);
                        let sb = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0);
                        let out_r = (r + (sr - r) * f).round() as u32;
                        let out_g = (g + (sg - g) * f).round() as u32;
                        let out_b = (b + (sb - b) * f).round() as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::HueRotate(deg) => {
                let rad = deg.to_radians();
                let cos_a = rad.cos();
                let sin_a = rad.sin();
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let out_r = ((0.213 + cos_a * 0.787 - sin_a * 0.213) * r
                            + (0.715 - cos_a * 0.715 - sin_a * 0.715) * g
                            + (0.072 - cos_a * 0.072 + sin_a * 0.928) * b)
                            .clamp(0.0, 255.0) as u32;
                        let out_g = ((0.213 - cos_a * 0.213 + sin_a * 0.143) * r
                            + (0.715 + cos_a * 0.285 + sin_a * 0.140) * g
                            + (0.072 - cos_a * 0.072 - sin_a * 0.283) * b)
                            .clamp(0.0, 255.0) as u32;
                        let out_b = ((0.213 - cos_a * 0.213 - sin_a * 0.787) * r
                            + (0.715 - cos_a * 0.715 + sin_a * 0.715) * g
                            + (0.072 + cos_a * 0.928 + sin_a * 0.072) * b)
                            .clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::DropShadow {
                offset_x,
                offset_y,
                blur,
                color,
            } => {
                let w = (max_x - min_x) as usize;
                let h = (max_y - min_y) as usize;
                if w == 0 || h == 0 || color.a == 0 {
                    continue;
                }
                let mut shadow_alphas = vec![0u8; w * h];
                let ox = offset_x.round() as i32;
                let oy = offset_y.round() as i32;
                for y in 0..h {
                    for x in 0..w {
                        let src_x = x as i32 - ox;
                        let src_y = y as i32 - oy;
                        if src_x >= 0 && src_x < w as i32 && src_y >= 0 && src_y < h as i32 {
                            let src_idx = ((min_y as i32 + src_y) as u32 * buf_width
                                + (min_x as i32 + src_x) as u32)
                                as usize;
                            let alpha = effective_alpha(buffer[src_idx]);
                            shadow_alphas[y * w + x] =
                                (alpha as f32 * (color.a as f32 / 255.0)).round() as u8;
                        }
                    }
                }
                let r = blur.round() as i32;
                if r > 0 {
                    let mut temp = vec![0u8; w * h];
                    let radius = r.min(16).max(1) as usize;
                    for y in 0..h {
                        let mut sum = 0u32;
                        let mut count = 0u32;
                        let init_right = radius.min(w.saturating_sub(1));
                        for kx in 0..=init_right {
                            sum += shadow_alphas[y * w + kx] as u32;
                            count += 1;
                        }
                        for x in 0..w {
                            if x > 0 {
                                if x + radius < w {
                                    sum += shadow_alphas[y * w + (x + radius)] as u32;
                                    count += 1;
                                }
                                if x > radius {
                                    sum -= shadow_alphas[y * w + (x - radius - 1)] as u32;
                                    count -= 1;
                                }
                            }
                            temp[y * w + x] = sum.checked_div(count).unwrap_or(0) as u8;
                        }
                    }
                    for x in 0..w {
                        let mut sum = 0u32;
                        let mut count = 0u32;
                        let init_bottom = radius.min(h.saturating_sub(1));
                        for ky in 0..=init_bottom {
                            sum += temp[ky * w + x] as u32;
                            count += 1;
                        }
                        for y in 0..h {
                            if y > 0 {
                                if y + radius < h {
                                    sum += temp[(y + radius) * w + x] as u32;
                                    count += 1;
                                }
                                if y > radius {
                                    sum -= temp[(y - radius - 1) * w + x] as u32;
                                    count -= 1;
                                }
                            }
                            shadow_alphas[y * w + x] = sum.checked_div(count).unwrap_or(0) as u8;
                        }
                    }
                }
                for y in 0..h {
                    for x in 0..w {
                        let idx = ((min_y + y as u32) * buf_width + (min_x + x as u32)) as usize;
                        let orig_pixel = buffer[idx];
                        let s_a = shadow_alphas[y * w + x];
                        if s_a > 0 {
                            let shadow_color = Color::rgba(color.r, color.g, color.b, s_a);
                            let shadow_u32 = shadow_color.to_rgb_u32();
                            let orig_alpha = effective_alpha(orig_pixel);
                            if orig_alpha == 0 {
                                buffer[idx] = shadow_u32;
                            } else if orig_alpha < 255 {
                                let orig_color = Color::rgba(
                                    ((orig_pixel >> 16) & 0xff) as u8,
                                    ((orig_pixel >> 8) & 0xff) as u8,
                                    (orig_pixel & 0xff) as u8,
                                    orig_alpha as u8,
                                );
                                buffer[idx] = blend_pixel(orig_color, shadow_u32);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn box_blur_rect(
    buffer: &mut [u32],
    buf_width: u32,
    min_x: u32,
    min_y: u32,
    w: usize,
    h: usize,
    radius: i32,
) {
    if w == 0 || h == 0 {
        return;
    }
    let r = radius.min(w.max(h) as i32 / 2).max(1) as usize;
    let mut temp = vec![0u32; w * h];

    for row in 0..h {
        let y = min_y + row as u32;
        let mut sum_a = 0u32;
        let mut sum_r = 0u32;
        let mut sum_g = 0u32;
        let mut sum_b = 0u32;
        let mut count = 0u32;

        let right_init = r.min(w.saturating_sub(1));
        for cx in 0..=right_init {
            let px = min_x + cx as u32;
            let p = buffer[(y * buf_width + px) as usize];
            sum_a += (p >> 24) & 0xff;
            sum_r += (p >> 16) & 0xff;
            sum_g += (p >> 8) & 0xff;
            sum_b += p & 0xff;
            count += 1;
        }

        for col in 0..w {
            if col > 0 {
                if col + r < w {
                    let px = min_x + (col + r) as u32;
                    let p = buffer[(y * buf_width + px) as usize];
                    sum_a += (p >> 24) & 0xff;
                    sum_r += (p >> 16) & 0xff;
                    sum_g += (p >> 8) & 0xff;
                    sum_b += p & 0xff;
                    count += 1;
                }
                if col > r {
                    let px = min_x + (col - r - 1) as u32;
                    let p = buffer[(y * buf_width + px) as usize];
                    sum_a -= (p >> 24) & 0xff;
                    sum_r -= (p >> 16) & 0xff;
                    sum_g -= (p >> 8) & 0xff;
                    sum_b -= p & 0xff;
                    count -= 1;
                }
            }

            if let (Some(a), Some(r), Some(g), Some(b)) = (
                sum_a.checked_div(count),
                sum_r.checked_div(count),
                sum_g.checked_div(count),
                sum_b.checked_div(count),
            ) {
                temp[row * w + col] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }
    }

    for col in 0..w {
        let x = min_x + col as u32;
        let mut sum_a = 0u32;
        let mut sum_r = 0u32;
        let mut sum_g = 0u32;
        let mut sum_b = 0u32;
        let mut count = 0u32;

        let bottom_init = r.min(h.saturating_sub(1));
        for cy in 0..=bottom_init {
            let p = temp[cy * w + col];
            sum_a += (p >> 24) & 0xff;
            sum_r += (p >> 16) & 0xff;
            sum_g += (p >> 8) & 0xff;
            sum_b += p & 0xff;
            count += 1;
        }

        for row in 0..h {
            if row > 0 {
                if row + r < h {
                    let p = temp[(row + r) * w + col];
                    sum_a += (p >> 24) & 0xff;
                    sum_r += (p >> 16) & 0xff;
                    sum_g += (p >> 8) & 0xff;
                    sum_b += p & 0xff;
                    count += 1;
                }
                if row > r {
                    let p = temp[(row - r - 1) * w + col];
                    sum_a -= (p >> 24) & 0xff;
                    sum_r -= (p >> 16) & 0xff;
                    sum_g -= (p >> 8) & 0xff;
                    sum_b -= p & 0xff;
                    count -= 1;
                }
            }

            if let (Some(a), Some(r), Some(g), Some(b)) = (
                sum_a.checked_div(count),
                sum_r.checked_div(count),
                sum_g.checked_div(count),
                sum_b.checked_div(count),
            ) {
                let y = min_y + row as u32;
                buffer[(y * buf_width + x) as usize] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }
    }
}

/// Color stop on a canvas gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasColorStop {
    pub offset: f32,
    pub color: Color,
}

/// Two-point radial gradient matching W3C Canvas 2D specification.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    pub x0: f32,
    pub y0: f32,
    pub r0: f32,
    pub x1: f32,
    pub y1: f32,
    pub r1: f32,
    pub stops: Vec<CanvasColorStop>,
}

impl RadialGradient {
    /// Creates a new two-point radial gradient from start circle (x0, y0, r0) to end circle (x1, y1, r1).
    pub fn new(x0: f32, y0: f32, r0: f32, x1: f32, y1: f32, r1: f32) -> Self {
        Self {
            x0,
            y0,
            r0: r0.max(0.0),
            x1,
            y1,
            r1: r1.max(0.0),
            stops: Vec::new(),
        }
    }

    /// Adds a color stop at the given normalized offset in `[0.0, 1.0]`.
    pub fn add_color_stop(&mut self, offset: f32, color: Color) {
        let offset = offset.clamp(0.0, 1.0);
        self.stops.push(CanvasColorStop { offset, color });
        self.stops.sort_by(|a, b| {
            a.offset
                .partial_cmp(&b.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Evaluates the gradient color at user-space coordinates (x, y).
    pub fn eval_at(&self, x: f32, y: f32) -> Option<Color> {
        evaluate_radial_gradient(
            self.x0,
            self.y0,
            self.r0,
            self.x1,
            self.y1,
            self.r1,
            &self.stops,
            x,
            y,
        )
    }
}

/// Interpolates two-point radial gradient color according to the W3C Canvas 2D specification.
pub(crate) fn evaluate_radial_gradient(
    x0: f32,
    y0: f32,
    r0: f32,
    x1: f32,
    y1: f32,
    r1: f32,
    stops: &[CanvasColorStop],
    x: f32,
    y: f32,
) -> Option<Color> {
    if stops.is_empty() {
        return None;
    }
    let dx = x1 - x0;
    let dy = y1 - y0;
    let dr = r1 - r0;
    let px = x - x0;
    let py = y - y0;

    let a = dx * dx + dy * dy - dr * dr;
    let b = -(px * dx + py * dy + r0 * dr);
    let c = px * px + py * py - r0 * r0;

    let t = if a.abs() < 1e-6 {
        if b.abs() < 1e-6 {
            return None;
        }
        let t = -c / (2.0 * b);
        if (r0 + t * dr) < 0.0 {
            return None;
        }
        t
    } else {
        let disc = b * b - a * c;
        if disc < 0.0 {
            return None;
        }
        let sqrt_disc = disc.sqrt();
        let t1 = (-b - sqrt_disc) / a;
        let t2 = (-b + sqrt_disc) / a;

        let r1_valid = (r0 + t1 * dr) >= 0.0;
        let r2_valid = (r0 + t2 * dr) >= 0.0;

        match (r1_valid, r2_valid) {
            (true, true) => t1.max(t2),
            (true, false) => t1,
            (false, true) => t2,
            (false, false) => return None,
        }
    };

    Some(sample_color_stops(stops, t))
}

/// Samples a list of sorted color stops at normalized position parameter `t`.
pub(crate) fn sample_color_stops(stops: &[CanvasColorStop], t: f32) -> Color {
    if stops.is_empty() {
        return Color::TRANSPARENT;
    }
    if stops.len() == 1 {
        return stops[0].color;
    }
    let t = t.clamp(0.0, 1.0);
    if t <= stops[0].offset {
        return stops[0].color;
    }
    if t >= stops[stops.len() - 1].offset {
        return stops[stops.len() - 1].color;
    }
    for window in stops.windows(2) {
        let s0 = &window[0];
        let s1 = &window[1];
        if t >= s0.offset && t <= s1.offset {
            let span = s1.offset - s0.offset;
            if span <= 1e-6 {
                return s1.color;
            }
            let factor = (t - s0.offset) / span;
            return s0.color.lerp(s1.color, factor);
        }
    }
    stops[stops.len() - 1].color
}
