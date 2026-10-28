//! Unicode emoji detection and procedural 32-bit ARGB color emoji rasterization.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Returns true if a character belongs to Unicode emoji and symbol ranges.
pub fn is_emoji_char(ch: char) -> bool {
    let u = ch as u32;
    matches!(
        u,
        0x1F300..=0x1F5FF   // Misc Symbols & Pictographs
        | 0x1F600..=0x1F64F // Emoticons
        | 0x1F680..=0x1F6FF // Transport & Map
        | 0x1F700..=0x1F77F // Alchemical Symbols
        | 0x1F780..=0x1F7FF // Geometric Shapes Extended
        | 0x1F800..=0x1F8FF // Supplemental Arrows-C
        | 0x1F900..=0x1FAFF // Supplemental Symbols & Pictographs / Ext-A
        | 0x2300..=0x23FF   // Misc Technical (e.g. ⌚, ⌛, ⏰, ⏳)
        | 0x2600..=0x26FF   // Misc Symbols (e.g. ☀️, ☁️, ⚡, ☕)
        | 0x2700..=0x27BF   // Dingbats (e.g. ✂️, ✈️, ✉️, ✏️, ✨, ❤)
        | 0x2B00..=0x2BFF   // Misc Symbols and Arrows (e.g. ⭐, ⭕, ⬛, ⬜)
    )
}

/// Returns true if a character is a Unicode variation selector (VS1..VS16).
#[inline]
pub fn is_variation_selector(ch: char) -> bool {
    matches!(ch, '\u{FE00}'..='\u{FE0F}')
}

/// Returns true if a character is a zero-width formatter, joiner, or non-spacing format mark.
#[inline]
pub fn is_zero_width_or_format(ch: char) -> bool {
    matches!(
        ch,
        '\u{FE00}'..='\u{FE0F}' // Variation Selectors (e.g. VS16 emoji presentation)
            | '\u{200B}'..='\u{200D}' // Zero-width space, ZWNJ, ZWJ
            | '\u{FEFF}' // BOM / Zero-width no-break space
    )
}

/// A 32-bit ARGB rasterized color emoji bitmap.
#[derive(Debug, Clone)]
pub struct ColorEmojiBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

static EMOJI_CACHE: OnceLock<RwLock<HashMap<(char, u32), ColorEmojiBitmap>>> = OnceLock::new();

fn emoji_cache() -> &'static RwLock<HashMap<(char, u32), ColorEmojiBitmap>> {
    EMOJI_CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(256)))
}

/// Retrieves a cached color emoji bitmap or rasterizes and caches it.
pub fn get_or_rasterize_color_emoji(ch: char, size: f32) -> ColorEmojiBitmap {
    let px = size.round().clamp(10.0, 128.0) as u32;
    let key = (ch, px);
    if let Ok(c) = emoji_cache().read()
        && let Some(bmp) = c.get(&key)
    {
        return bmp.clone();
    }
    let bmp = rasterize_color_emoji(ch, px);
    if let Ok(mut c) = emoji_cache().write() {
        if c.len() >= 512 {
            let keys_to_remove: Vec<_> = c.keys().take(256).cloned().collect();
            for k in keys_to_remove {
                c.remove(&k);
            }
        }
        c.insert(key, bmp.clone());
    }
    bmp
}

/// Rasterizes an authentic 32-bit ARGB color emoji bitmap for `ch` at dimension `s x s`.
pub fn rasterize_color_emoji(ch: char, s: u32) -> ColorEmojiBitmap {
    let s = s.clamp(10, 128);
    let mut pixels = vec![0u32; (s * s) as usize];
    let wf = s as f32;
    let hf = s as f32;

    for py in 0..s {
        for px in 0..s {
            let nx = (px as f32 + 0.5) / wf;
            let ny = (py as f32 + 0.5) / hf;
            let idx = (py * s + px) as usize;

            let color_opt = match ch {
                '🥭' => {
                    // Mango fruit body: warm golden-amber gradient with green leaf and brown stem
                    let body_dx = nx - (0.48 + 0.08 * (ny - 0.55).powi(2));
                    let body_dy = ny - 0.55;
                    let dist_body = ((body_dx / 0.34).powi(2) + (body_dy / 0.38).powi(2)).sqrt();

                    let stem_dx = (nx - 0.48).abs();
                    let is_stem = stem_dx < 0.04 && (0.12..=0.24).contains(&ny);

                    let leaf_dx = nx - 0.62;
                    let leaf_dy = ny - 0.22;
                    let dist_leaf = ((leaf_dx / 0.18).powi(2) + (leaf_dy / 0.10).powi(2)).sqrt();

                    if is_stem {
                        Some(0xFF6D4C41)
                    } else if dist_leaf <= 1.0 {
                        let edge_a = ((1.0 - dist_leaf) * wf * 0.8).clamp(0.0, 1.0);
                        let alpha = (edge_a * 255.0) as u32;
                        let g = if leaf_dy.abs() < 0.02 { 0x66 } else { 0xA0 };
                        Some((alpha << 24) | (0x43 << 16) | (g << 8) | 0x47)
                    } else if dist_body <= 1.0 {
                        let edge_a = ((1.0 - dist_body) * wf * 0.8).clamp(0.0, 1.0);
                        let alpha = (edge_a * 255.0) as u32;
                        let t = (nx + ny) * 0.5;
                        let r = 255u32;
                        let g = (180.0 - t * 80.0).clamp(70.0, 210.0) as u32;
                        let b = (10.0 + t * 15.0).clamp(5.0, 40.0) as u32;
                        Some((alpha << 24) | (r << 16) | (g << 8) | b)
                    } else {
                        None
                    }
                }
                '🚀' => {
                    // Rocket: silver hull, circular blue window, red nosecone and fins, flame plume
                    let hull_dx = (nx - 0.5).abs();
                    let hull_dy = ny - 0.45;
                    let is_hull = (hull_dx / 0.20).powi(2) + (hull_dy / 0.32).powi(2) <= 1.0;
                    let is_nose = ny < 0.25 && hull_dx < (0.25 - ny) * 0.8;
                    let dist_win = ((nx - 0.5).powi(2) + (ny - 0.42).powi(2)).sqrt();
                    let is_fin = ny > 0.55 && ny < 0.78 && hull_dx > 0.15 && hull_dx < 0.38;
                    let is_flame = ny > 0.74 && ny < 0.95 && hull_dx < (0.95 - ny) * 0.7;

                    if dist_win < 0.08 {
                        if dist_win < 0.04 && nx < 0.5 && ny < 0.42 {
                            Some(0xFFFFFFFF)
                        } else {
                            Some(0xFF0288D1)
                        }
                    } else if is_nose || is_fin {
                        Some(0xFFE53935)
                    } else if is_hull {
                        let shade = ((1.0 - nx) * 50.0) as u32;
                        let c = (210 + shade).min(255);
                        Some(0xFF000000 | (c << 16) | (c << 8) | c)
                    } else if is_flame {
                        if hull_dx < (0.95 - ny) * 0.35 {
                            Some(0xFFFFEB3B)
                        } else {
                            Some(0xFFFF5722)
                        }
                    } else {
                        None
                    }
                }
                '😀' | '😃' | '😊' => {
                    // Smiley face
                    let dist = ((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt();
                    let is_eye_l =
                        ((nx - 0.35).powi(2) + ((ny - 0.38) / 1.3).powi(2)).sqrt() < 0.06;
                    let is_eye_r =
                        ((nx - 0.65).powi(2) + ((ny - 0.38) / 1.3).powi(2)).sqrt() < 0.06;
                    let is_mouth = (0.55..=0.72).contains(&ny)
                        && ((nx - 0.5).abs() < 0.25)
                        && ((nx - 0.5).powi(2) + (ny - 0.55).powi(2)).sqrt() < 0.25;

                    if is_eye_l || is_eye_r {
                        Some(0xFF212121)
                    } else if is_mouth {
                        if ny > 0.65 {
                            Some(0xFFE53935)
                        } else {
                            Some(0xFF212121)
                        }
                    } else if dist <= 0.45 {
                        let edge_a = ((0.45 - dist) * wf * 0.8).clamp(0.0, 1.0);
                        let alpha = (edge_a * 255.0) as u32;
                        Some((alpha << 24) | 0xFFCA28)
                    } else {
                        None
                    }
                }
                '🔥' => {
                    // Fire flame
                    let dx = (nx - 0.5).abs();
                    let dy = 1.0 - ny;
                    let outer = dx < 0.45 * (dy / 0.9).sqrt() && ny > 0.1;
                    let mid = dx < 0.28 * (dy / 0.8).sqrt() && ny > 0.25;
                    let core = dx < 0.14 * (dy / 0.6).sqrt() && ny > 0.45;

                    if core {
                        Some(0xFFFFEE58)
                    } else if mid {
                        Some(0xFFFF9800)
                    } else if outer {
                        Some(0xFFD32F2F)
                    } else {
                        None
                    }
                }
                '🎉' => {
                    let cone = ny > 0.45 && nx < 0.65 && (nx + ny) > 0.85 && (nx - ny).abs() < 0.35;
                    let is_confetti_cyan =
                        ((nx - 0.35).powi(2) + (ny - 0.25).powi(2)).sqrt() < 0.07;
                    let is_confetti_pink =
                        ((nx - 0.65).powi(2) + (ny - 0.35).powi(2)).sqrt() < 0.06;
                    let is_confetti_lime =
                        ((nx - 0.45).powi(2) + (ny - 0.15).powi(2)).sqrt() < 0.06;
                    if cone {
                        Some(0xFFFFC107)
                    } else if is_confetti_cyan {
                        Some(0xFF00E5FF)
                    } else if is_confetti_pink {
                        Some(0xFFFF4081)
                    } else if is_confetti_lime {
                        Some(0xFF76FF03)
                    } else {
                        None
                    }
                }
                '⭐' => {
                    let dx = (nx - 0.5).abs();
                    let dy = (ny - 0.5).abs();
                    let dist = (dx + dy).max(dx * 1.5).max(dy * 1.5);
                    if dist < 0.42 {
                        Some(0xFFFFD700)
                    } else if dist < 0.46 {
                        Some(0xFFFFA000)
                    } else {
                        None
                    }
                }
                '❤' => {
                    let hx = (nx - 0.5) * 2.4;
                    let hy = (0.55 - ny) * 2.4;
                    let val = (hx * hx + hy * hy - 1.0).powi(3) - hx * hx * hy.powi(3);
                    if val <= 0.0 { Some(0xFFE53935) } else { None }
                }
                '👍' => {
                    let is_thumb = nx > 0.25 && nx < 0.55 && ny < 0.55;
                    let is_hand = nx > 0.20 && nx < 0.75 && (0.45..=0.80).contains(&ny);
                    let is_cuff = nx < 0.32 && ny > 0.65;
                    if is_cuff {
                        Some(0xFF1976D2)
                    } else if is_thumb || is_hand {
                        Some(0xFFFFCA28)
                    } else {
                        None
                    }
                }
                _ => {
                    let dist = ((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt();
                    if dist <= 0.44 {
                        let edge_a = ((0.44 - dist) * wf * 0.8).clamp(0.0, 1.0);
                        let alpha = (edge_a * 255.0) as u32;
                        let u = ch as u32;
                        let theme_r = (u * 67) % 180 + 75;
                        let theme_g = (u * 131) % 180 + 75;
                        let theme_b = (u * 197) % 180 + 75;

                        let is_inner = dist < 0.20;
                        if is_inner {
                            Some(0xFFFFFFFF)
                        } else {
                            Some((alpha << 24) | (theme_r << 16) | (theme_g << 8) | theme_b)
                        }
                    } else {
                        None
                    }
                }
            };

            if let Some(c) = color_opt {
                pixels[idx] = c;
            }
        }
    }

    ColorEmojiBitmap {
        width: s,
        height: s,
        pixels,
    }
}
