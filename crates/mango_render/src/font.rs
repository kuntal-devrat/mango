//! Font subsystem: TrueType font loading, text measurement, and glyph rasterization.
//!
//! Replaces the Phase 0 bitmap font renderer with `fontdue` for proper anti-aliased
//! TrueType/OpenType glyph rendering with proportional metrics.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};

use fontdue::{Font, FontSettings};
use mango_core::Color;

/// Embedded font data — compiled directly into the binary.
const FONT_SANS: &[u8] = include_bytes!("fonts/DejaVuSans.ttf");
const FONT_SANS_BOLD: &[u8] = include_bytes!("fonts/DejaVuSans-Bold.ttf");
const FONT_SERIF: &[u8] = include_bytes!("fonts/DejaVuSerif.ttf");
const FONT_SERIF_BOLD: &[u8] = include_bytes!("fonts/DejaVuSerif-Bold.ttf");
const FONT_MONO: &[u8] = include_bytes!("fonts/DejaVuSansMono.ttf");

/// Global singleton for the font manager.
static FONT_MANAGER: OnceLock<FontManager> = OnceLock::new();

/// Returns a reference to the global [`FontManager`] singleton.
///
/// Initializes on first call by parsing the embedded TTF font data.
pub fn font_manager() -> &'static FontManager {
    FONT_MANAGER.get_or_init(FontManager::new)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Regular,
    Bold,
}

/// A cached rasterized glyph with its metrics and 8-bit coverage bitmap (OPT-007).
#[derive(Clone)]
pub struct CachedGlyph {
    pub metrics: fontdue::Metrics,
    pub bitmap: std::sync::Arc<[u8]>,
}

type GlyphCacheKey = (FontFamily, FontWeight, u32, char);

struct GlyphCacheEntry {
    glyph: CachedGlyph,
    last_access: AtomicU64,
}

static ACCESS_COUNTER: AtomicU64 = AtomicU64::new(1);
static GLYPH_CACHE: OnceLock<RwLock<HashMap<GlyphCacheKey, GlyphCacheEntry>>> = OnceLock::new();

fn glyph_cache() -> &'static RwLock<HashMap<GlyphCacheKey, GlyphCacheEntry>> {
    GLYPH_CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(1024)))
}

/// Font style selection (normal vs. italic/oblique).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// CSS text decoration line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecoration {
    #[default]
    None,
    Underline,
    LineThrough,
    Overline,
}

/// Font family selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Monospace,
    Custom(u32),
}

impl FontFamily {
    /// Maps a CSS `font-family` property string to a `FontFamily`.
    pub fn from_css_name(name: &str) -> Self {
        FontManager::resolve_family(name)
    }
}

/// Text antialiasing mode (ClearType-style subpixel LCD vs. grayscale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAntialias {
    #[default]
    Grayscale,
    SubpixelLcd,
}

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
        | 0xFE00..=0xFE0F   // Variation Selectors
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

/// An OpenType variation axis extracted from the `fvar` table.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableFontAxis {
    /// 4-byte OpenType tag (e.g., `*b"wght"`).
    pub tag: [u8; 4],
    /// Human-readable axis name or tag string (e.g., "wght").
    pub tag_str: String,
    /// Minimum allowed coordinate value.
    pub min_value: f32,
    /// Default coordinate value.
    pub default_value: f32,
    /// Maximum allowed coordinate value.
    pub max_value: f32,
    /// Axis flags.
    pub flags: u16,
}

/// Metadata describing a variable font's variation space.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableFontMetadata {
    pub axes: Vec<VariableFontAxis>,
    pub instance_count: u16,
}

/// Parses the `fvar` (Font Variations) table from TrueType/OpenType font bytes.
pub fn parse_fvar_table(bytes: &[u8]) -> Option<VariableFontMetadata> {
    if bytes.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut fvar_offset = None;
    let mut fvar_length = 0usize;

    for i in 0..num_tables {
        let rec_start = 12 + i * 16;
        if rec_start + 16 > bytes.len() {
            break;
        }
        let tag = &bytes[rec_start..rec_start + 4];
        if tag == b"fvar" {
            let offset = u32::from_be_bytes([
                bytes[rec_start + 8],
                bytes[rec_start + 9],
                bytes[rec_start + 10],
                bytes[rec_start + 11],
            ]) as usize;
            let length = u32::from_be_bytes([
                bytes[rec_start + 12],
                bytes[rec_start + 13],
                bytes[rec_start + 14],
                bytes[rec_start + 15],
            ]) as usize;
            fvar_offset = Some(offset);
            fvar_length = length;
            break;
        }
    }

    let offset = fvar_offset?;
    if offset + 16 > bytes.len() || fvar_length < 16 {
        return None;
    }

    let axes_offset = u16::from_be_bytes([bytes[offset + 4], bytes[offset + 5]]) as usize;
    let axis_count = u16::from_be_bytes([bytes[offset + 8], bytes[offset + 9]]) as usize;
    let axis_size = u16::from_be_bytes([bytes[offset + 10], bytes[offset + 11]]) as usize;
    let instance_count = u16::from_be_bytes([bytes[offset + 12], bytes[offset + 13]]);

    if axis_size < 20 {
        return None;
    }

    let mut axes = Vec::with_capacity(axis_count);
    let mut cur = offset + axes_offset;

    for _ in 0..axis_count {
        if cur + 20 > bytes.len() {
            break;
        }
        let tag: [u8; 4] = [bytes[cur], bytes[cur + 1], bytes[cur + 2], bytes[cur + 3]];
        let min_raw = i32::from_be_bytes([
            bytes[cur + 4],
            bytes[cur + 5],
            bytes[cur + 6],
            bytes[cur + 7],
        ]);
        let def_raw = i32::from_be_bytes([
            bytes[cur + 8],
            bytes[cur + 9],
            bytes[cur + 10],
            bytes[cur + 11],
        ]);
        let max_raw = i32::from_be_bytes([
            bytes[cur + 12],
            bytes[cur + 13],
            bytes[cur + 14],
            bytes[cur + 15],
        ]);
        let flags = u16::from_be_bytes([bytes[cur + 16], bytes[cur + 17]]);

        let min_value = min_raw as f32 / 65536.0;
        let default_value = def_raw as f32 / 65536.0;
        let max_value = max_raw as f32 / 65536.0;
        let tag_str = String::from_utf8_lossy(&tag).to_string();

        axes.push(VariableFontAxis {
            tag,
            tag_str,
            min_value,
            default_value,
            max_value,
            flags,
        });

        cur += axis_size;
    }

    Some(VariableFontMetadata {
        axes,
        instance_count,
    })
}

/// Normalizes a user axis coordinate into OpenType normalized design space `[-1.0, 1.0]`.
pub fn interpolate_axis(axis: &VariableFontAxis, user_value: f32) -> f32 {
    let clamped = user_value.clamp(axis.min_value, axis.max_value);
    if (clamped - axis.default_value).abs() < 0.0001 {
        0.0
    } else if clamped < axis.default_value {
        let range = axis.default_value - axis.min_value;
        if range > 0.0001 {
            (clamped - axis.default_value) / range
        } else {
            0.0
        }
    } else {
        let range = axis.max_value - axis.default_value;
        if range > 0.0001 {
            (clamped - axis.default_value) / range
        } else {
            0.0
        }
    }
}

/// Decompresses and parses font bytes, supporting WOFF2, WOFF1, TrueType, and OpenType.
pub fn decode_font_bytes(bytes: &[u8]) -> Result<Font, String> {
    if bytes.len() < 4 {
        return Err("Font data is too short".to_string());
    }

    let trimmed = if bytes.starts_with(b" ") || bytes.starts_with(b"\n") || bytes.starts_with(b"\r") || bytes.starts_with(b"\t") {
        bytes.iter().copied().skip_while(|&b| b.is_ascii_whitespace()).collect::<Vec<u8>>()
    } else {
        Vec::new()
    };
    let slice = if trimmed.is_empty() { bytes } else { &trimmed };
    if slice.starts_with(b"<!DOC")
        || slice.starts_with(b"<!doc")
        || slice.starts_with(b"<html")
        || slice.starts_with(b"<HTML")
        || slice.starts_with(b"<?xml")
    {
        return Err("Expected binary font data, received HTML text".to_string());
    }

    let decompressed: Cow<[u8]> = if bytes.starts_with(b"wOF2") {
        let dec = wuff::decompress_woff2(bytes)
            .map_err(|e| format!("Failed to decompress WOFF2 font: {:?}", e))?;
        Cow::Owned(dec)
    } else if bytes.starts_with(b"wOFF") {
        let dec = wuff::decompress_woff1(bytes)
            .map_err(|e| format!("Failed to decompress WOFF1 font: {:?}", e))?;
        Cow::Owned(dec)
    } else {
        Cow::Borrowed(bytes)
    };

    Font::from_bytes(decompressed.as_ref(), FontSettings::default())
        .map_err(|e| format!("Failed to parse TrueType/OpenType font tables: {}", e))
}

struct WebFontEntry {
    #[allow(dead_code)]
    family_name: String,
    regular: Option<&'static Font>,
    bold: Option<&'static Font>,
}

fn load_system_font(filename: &str) -> Option<Font> {
    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let path = format!("{}\\Fonts\\{}", windir, filename);
        if let Ok(bytes) = std::fs::read(&path) {
            return Font::from_bytes(bytes, FontSettings::default()).ok();
        }
    }
    #[cfg(target_os = "linux")]
    {
        let paths = [
            format!("/usr/share/fonts/truetype/{}", filename),
            format!("/usr/share/fonts/opentype/{}", filename),
            format!("/usr/share/fonts/TTF/{}", filename),
            format!("/usr/share/fonts/noto/{}", filename),
            format!("/usr/share/fonts/noto-cjk/{}", filename),
            format!("/usr/share/fonts/truetype/noto/{}", filename),
            format!("/usr/share/fonts/google-noto/{}", filename),
            format!("/usr/share/fonts/truetype/wqy/{}", filename),
        ];
        for p in paths {
            if let Ok(bytes) = std::fs::read(&p)
                && let Ok(f) = Font::from_bytes(bytes, FontSettings::default())
            {
                return Some(f);
            }
        }
    }
    let _ = filename;
    None
}

/// Manages loaded font faces and provides text measurement and glyph rasterization.
pub struct FontManager {
    sans_regular: Font,
    sans_bold: Font,
    serif_regular: Font,
    serif_bold: Font,
    mono: Font,
    fallback_fonts: RwLock<Vec<&'static Font>>,
    unloaded_fallbacks: RwLock<Vec<&'static str>>,
    missing_glyphs: RwLock<HashSet<char>>,
    preload_started: AtomicBool,
    web_fonts: RwLock<HashMap<u32, WebFontEntry>>,
    family_to_id: RwLock<HashMap<String, u32>>,
    next_font_id: AtomicU32,
}

impl FontManager {
    /// Creates a new `FontManager` prioritizing native platform system fonts,
    /// with graceful fallback to embedded DejaVu fonts.
    ///
    /// The generic families must match what Chromium resolves them to on the same
    /// platform, otherwise every page that omits `font-family` (or asks for
    /// `Arial, sans-serif`) lays out with different glyph widths and line heights.
    /// Chromium on Windows uses Arial for `sans-serif`, Times New Roman for `serif`
    /// and Consolas for `monospace`; `system-ui` is Segoe UI and is resolved
    /// separately, by name, in `resolve_family_for`.
    fn new() -> Self {
        let settings = FontSettings::default();

        let sans_regular = load_system_font("arial.ttf")
            .or_else(|| load_system_font("segoeui.ttf"))
            .unwrap_or_else(|| {
                Font::from_bytes(FONT_SANS, settings)
                    .expect("Failed to parse embedded DejaVuSans.ttf")
            });
        let sans_bold = load_system_font("arialbd.ttf")
            .or_else(|| load_system_font("segoeuib.ttf"))
            .unwrap_or_else(|| {
                Font::from_bytes(FONT_SANS_BOLD, settings)
                    .expect("Failed to parse embedded DejaVuSans-Bold.ttf")
            });
        let serif_regular = load_system_font("times.ttf")
            .or_else(|| load_system_font("georgia.ttf"))
            .unwrap_or_else(|| {
                Font::from_bytes(FONT_SERIF, settings)
                    .expect("Failed to parse embedded DejaVuSerif.ttf")
            });
        let serif_bold = load_system_font("timesbd.ttf")
            .or_else(|| load_system_font("georgiab.ttf"))
            .unwrap_or_else(|| {
                Font::from_bytes(FONT_SERIF_BOLD, settings)
                    .expect("Failed to parse embedded DejaVuSerif-Bold.ttf")
            });
        let mono = load_system_font("consola.ttf")
            .or_else(|| load_system_font("cour.ttf"))
            .unwrap_or_else(|| {
                Font::from_bytes(FONT_MONO, settings)
                    .expect("Failed to parse embedded DejaVuSansMono.ttf")
            });

        // Fallback font candidates for Indic scripts (Hindi, Bengali, Telugu, Tamil, etc.), CJK, Arabic, Thai, Hebrew, and symbols.
        // NOTE: These are loaded LAZILY on demand in `select_font_for_char` to avoid
        // parsing ~150-200MB of massive TTC/TTF fonts into RAM during browser startup.
        let fallback_names: Vec<&'static str> = vec![
            // Indic scripts (Devanagari, Bengali, Telugu, Tamil, Gujarati, Kannada, Malayalam, Gurmukhi, Oriya, Sinhala)
            "Nirmala.ttc",
            "nirmala.ttf",
            "mangal.ttf",
            "vrinda.ttf",
            "gautami.ttf",
            "latha.ttf",
            // CJK: Chinese (Simplified & Traditional)
            "msyh.ttc",
            "msyhl.ttc",
            "simsun.ttc",
            "simsunb.ttf",
            "SimsunExtG.ttf",
            "msjh.ttc",
            "mingliu.ttc",
            "deng.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSansSC-Regular.otf",
            "NotoSansTC-Regular.otf",
            "wqy-zenhei.ttc",
            "wqy-microhei.ttc",
            // CJK: Japanese
            "msgothic.ttc",
            "meiryo.ttc",
            "YuGothR.ttc",
            "NotoSansJP-Regular.otf",
            // CJK: Korean
            "malgun.ttf",
            "malgunbd.ttf",
            "gulim.ttc",
            "NotoSansKR-Regular.otf",
            // Arabic, Persian, Urdu
            "NotoSansArabic-Regular.ttf",
            "NotoKufiArabic-Regular.ttf",
            "NotoNaskhArabic-Regular.ttf",
            "tahoma.ttf",
            "tahomabd.ttf",
            // Thai
            "LeelawUI.ttf",
            "leelawad.ttf",
            "NotoSansThai-Regular.ttf",
            // Hebrew
            "NotoSansHebrew-Regular.ttf",
            "DavidLibre-Regular.ttf",
            "DavidCLM-Medium.otf",
            "david.ttf",
            // Armenian, Georgian, Lao
            "NotoSansArmenian-Regular.ttf",
            "NotoSansGeorgian-Regular.ttf",
            "NotoSansLao-Regular.ttf",
            // Emoji and Symbols
            "seguiemj.ttf",
            "seguisym.ttf",
            "NotoColorEmoji.ttf",
            "Symbola.ttf",
        ];

        Self {
            sans_regular,
            sans_bold,
            serif_regular,
            serif_bold,
            mono,
            fallback_fonts: RwLock::new(Vec::new()),
            unloaded_fallbacks: RwLock::new(fallback_names),
            missing_glyphs: RwLock::new(HashSet::new()),
            preload_started: AtomicBool::new(false),
            web_fonts: RwLock::new(HashMap::new()),
            family_to_id: RwLock::new(HashMap::new()),
            next_font_id: AtomicU32::new(1),
        }
    }

    /// Selects the appropriate font face based on family and weight.
    pub fn select_font(&self, family: FontFamily, weight: FontWeight) -> &Font {
        match family {
            FontFamily::Monospace => &self.mono,
            FontFamily::Serif => match weight {
                FontWeight::Bold => &self.serif_bold,
                FontWeight::Regular => &self.serif_regular,
            },
            FontFamily::SansSerif => match weight {
                FontWeight::Bold => &self.sans_bold,
                FontWeight::Regular => &self.sans_regular,
            },
            FontFamily::Custom(id) => {
                if let Ok(web_fonts) = self.web_fonts.read()
                    && let Some(entry) = web_fonts.get(&id)
                {
                    let font_opt = match weight {
                        FontWeight::Bold => entry.bold.or(entry.regular),
                        FontWeight::Regular => entry.regular.or(entry.bold),
                    };
                    if let Some(font) = font_opt {
                        return font;
                    }
                }
                match weight {
                    FontWeight::Bold => &self.sans_bold,
                    FontWeight::Regular => &self.sans_regular,
                }
            }
        }
    }
}

/// Returns targeted system font filenames prioritized for the Unicode script of `ch` (OPT-3.2.4).
fn script_candidates_for_char(ch: char) -> &'static [&'static str] {
    let u = ch as u32;
    match u {
        // Devanagari & Indic scripts (Hindi, Bengali, Telugu, Tamil, Gujarati, Kannada, Malayalam, Gurmukhi, Oriya, Sinhala)
        0x0900..=0x0DFF => &[
            "Nirmala.ttc",
            "nirmala.ttf",
            "mangal.ttf",
            "vrinda.ttf",
            "gautami.ttf",
            "latha.ttf",
        ],
        // Arabic, Persian, Urdu
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            &[
                "NotoSansArabic-Regular.ttf",
                "tahoma.ttf",
                "tahomabd.ttf",
                "NotoKufiArabic-Regular.ttf",
                "NotoNaskhArabic-Regular.ttf",
            ]
        }
        // Hebrew
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => &[
            "NotoSansHebrew-Regular.ttf",
            "DavidLibre-Regular.ttf",
            "david.ttf",
            "tahoma.ttf",
        ],
        // Thai
        0x0E00..=0x0E7F => &[
            "LeelawUI.ttf",
            "leelawad.ttf",
            "NotoSansThai-Regular.ttf",
            "tahoma.ttf",
        ],
        // CJK Japanese (Hiragana, Katakana)
        0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF65..=0xFF9F => &[
            "msgothic.ttc",
            "meiryo.ttc",
            "YuGothR.ttc",
            "NotoSansJP-Regular.otf",
            "msyh.ttc",
            "NotoSansCJK-Regular.ttc",
        ],
        // CJK Korean (Hangul Syllables & Jamo)
        0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => &[
            "malgun.ttf",
            "malgunbd.ttf",
            "gulim.ttc",
            "NotoSansKR-Regular.otf",
            "msyh.ttc",
            "NotoSansCJK-Regular.ttc",
        ],
        // CJK Unified Ideographs (Chinese / Hanzi / Kanji / Hanja)
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF => &[
            "msyh.ttc",
            "msyhl.ttc",
            "simsun.ttc",
            "simsunb.ttf",
            "SimsunExtG.ttf",
            "msjh.ttc",
            "mingliu.ttc",
            "deng.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSansSC-Regular.otf",
            "NotoSansTC-Regular.otf",
            "wqy-zenhei.ttc",
            "wqy-microhei.ttc",
            "msgothic.ttc",
            "malgun.ttf",
        ],
        // Armenian, Georgian, Lao
        0x0530..=0x058F | 0x10A0..=0x10FF | 0x0E80..=0x0EFF => &[
            "NotoSansArmenian-Regular.ttf",
            "NotoSansGeorgian-Regular.ttf",
            "NotoSansLao-Regular.ttf",
        ],
        // Emoji & Symbols
        0x1F000..=0x1FAFF | 0x2600..=0x27BF => &[
            "seguiemj.ttf",
            "seguisym.ttf",
            "NotoColorEmoji.ttf",
            "Symbola.ttf",
        ],
        _ => &[],
    }
}

impl FontManager {
    /// Selects a font face that contains a glyph for `ch`, falling back to platform Indic/CJK/symbol fonts.
    pub fn select_font_for_char(&self, family: FontFamily, weight: FontWeight, ch: char) -> &Font {
        let primary = self.select_font(family, weight);
        if ch.is_ascii_whitespace() || primary.lookup_glyph_index(ch) != 0 {
            return primary;
        }

        // Fast-path: if this glyph is known to be missing across all system fallbacks, return immediately
        if let Ok(missing) = self.missing_glyphs.read()
            && missing.contains(&ch)
        {
            return primary;
        }

        // 1. Check already-loaded fallback fonts
        if let Ok(loaded) = self.fallback_fonts.read() {
            for fb in loaded.iter() {
                if fb.lookup_glyph_index(ch) != 0 {
                    return fb;
                }
            }
        }

        // Kick off background fallback preloading to avoid future main-thread freezes (OPT-3.2.4)
        if !self.preload_started.swap(true, Ordering::Relaxed) {
            std::thread::Builder::new()
                .name("mango-font-preload".to_string())
                .spawn(|| {
                    let fm = font_manager();
                    loop {
                        let next_name = {
                            let mut pending = match fm.unloaded_fallbacks.write() {
                                Ok(p) => p,
                                Err(_) => break,
                            };
                            if pending.is_empty() {
                                break;
                            }
                            pending.remove(0)
                        };
                        if let Some(font) = load_system_font(next_name) {
                            let leaked: &'static Font = Box::leak(Box::new(font));
                            if let Ok(mut loaded) = fm.fallback_fonts.write() {
                                loaded.push(leaked);
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                })
                .ok();
        }

        // 2. Targeted script-based fallback search (OPT-3.2.4)
        let script_candidates = script_candidates_for_char(ch);
        if let Ok(mut pending) = self.unloaded_fallbacks.write() {
            let mut found_font = None;

            if !script_candidates.is_empty() {
                for &target_name in script_candidates {
                    if let Some(pos) = pending
                        .iter()
                        .position(|&name| name.eq_ignore_ascii_case(target_name))
                    {
                        let name = pending.remove(pos);
                        if let Some(font) = load_system_font(name) {
                            let leaked: &'static Font = Box::leak(Box::new(font));
                            if let Ok(mut loaded) = self.fallback_fonts.write() {
                                loaded.push(leaked);
                            }
                            if leaked.lookup_glyph_index(ch) != 0 {
                                found_font = Some(leaked);
                                break;
                            }
                        }
                    }
                }
            }

            // Fallback to remaining unloaded pool if script-specific candidates didn't match
            if found_font.is_none() {
                let mut remaining = Vec::with_capacity(pending.len());
                for name in pending.drain(..) {
                    if found_font.is_none() {
                        if let Some(font) = load_system_font(name) {
                            let leaked: &'static Font = Box::leak(Box::new(font));
                            if let Ok(mut loaded) = self.fallback_fonts.write() {
                                loaded.push(leaked);
                            }
                            if leaked.lookup_glyph_index(ch) != 0 {
                                found_font = Some(leaked);
                                continue;
                            }
                        }
                    } else {
                        remaining.push(name);
                    }
                }
                *pending = remaining;
            }

            if let Some(fb) = found_font {
                return fb;
            }
        }

        // Record glyph in missing set to avoid re-scanning on subsequent frames
        if let Ok(mut missing) = self.missing_glyphs.write() {
            missing.insert(ch);
        }

        primary
    }

    /// Returns the font's (ascent, descent, line_gap) in pixels at the specified font size.
    pub fn font_metrics(
        &self,
        family: FontFamily,
        weight: FontWeight,
        font_size: f32,
    ) -> (f32, f32, f32) {
        let font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);
        if let Some(lm) = font.horizontal_line_metrics(px_size) {
            (lm.ascent, lm.descent.abs(), lm.line_gap)
        } else {
            (px_size * 0.8, px_size * 0.2, 0.0)
        }
    }

    /// Registers a downloadable web font (WOFF2, WOFF1, TrueType, or OpenType) into the font manager.
    ///
    /// Accepts binary font data or `data:` URIs (base64 encoded).
    /// Returns the unique font family ID on success.
    pub fn register_web_font(
        &self,
        family_name: &str,
        weight: FontWeight,
        font_data: &[u8],
    ) -> Result<u32, String> {
        let raw_bytes: Cow<[u8]> = if font_data.starts_with(b"data:") {
            if let Ok(data_str) = std::str::from_utf8(font_data) {
                if let Some(comma_idx) = data_str.find(',') {
                    let metadata = &data_str[..comma_idx];
                    let payload = &data_str[comma_idx + 1..];
                    if metadata.contains(";base64") {
                        if let Some(dec) = crate::image_decode::base64_decode(payload) {
                            Cow::Owned(dec)
                        } else {
                            return Err("Invalid base64 encoding in font data URI".to_string());
                        }
                    } else {
                        Cow::Borrowed(payload.as_bytes())
                    }
                } else {
                    return Err("Invalid font data URI: missing comma".to_string());
                }
            } else {
                return Err("Invalid UTF-8 in font data URI".to_string());
            }
        } else {
            Cow::Borrowed(font_data)
        };

        let font = decode_font_bytes(&raw_bytes)?;
        let leaked: &'static Font = Box::leak(Box::new(font));

        let key = family_name
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();

        let mut family_map = self.family_to_id.write().map_err(|e| e.to_string())?;
        let mut web_fonts = self.web_fonts.write().map_err(|e| e.to_string())?;

        let id = if let Some(&existing_id) = family_map.get(&key) {
            if let Some(entry) = web_fonts.get_mut(&existing_id) {
                match weight {
                    FontWeight::Bold => entry.bold = Some(leaked),
                    FontWeight::Regular => entry.regular = Some(leaked),
                }
            }
            existing_id
        } else {
            let new_id = self.next_font_id.fetch_add(1, Ordering::Relaxed);
            let mut entry = WebFontEntry {
                family_name: family_name.to_string(),
                regular: None,
                bold: None,
            };
            match weight {
                FontWeight::Bold => entry.bold = Some(leaked),
                FontWeight::Regular => entry.regular = Some(leaked),
            }
            family_map.insert(key, new_id);
            web_fonts.insert(new_id, entry);
            new_id
        };

        Ok(id)
    }

    /// Checks if a custom web font family has been registered.
    pub fn has_web_font(&self, family_name: &str) -> bool {
        let key = family_name
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();
        self.family_to_id
            .read()
            .map(|m| m.contains_key(&key))
            .unwrap_or(false)
    }

    /// Checks if a specific weight for a custom web font family has been registered.
    pub fn has_web_font_weight(&self, family_name: &str, weight: FontWeight) -> bool {
        let key = family_name
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();
        if let Ok(family_map) = self.family_to_id.read()
            && let Some(&id) = family_map.get(&key)
            && let Ok(web_fonts) = self.web_fonts.read()
            && let Some(entry) = web_fonts.get(&id)
        {
            return match weight {
                FontWeight::Bold => entry.bold.is_some(),
                FontWeight::Regular => entry.regular.is_some(),
            };
        }
        false
    }

    /// Returns human-readable description of a font family including loaded weights.
    pub fn debug_family_name(&self, family: FontFamily) -> String {
        match family {
            FontFamily::SansSerif => "SansSerif".to_string(),
            FontFamily::Serif => "Serif".to_string(),
            FontFamily::Monospace => "Monospace".to_string(),
            FontFamily::Custom(id) => {
                if let Ok(web_fonts) = self.web_fonts.read()
                    && let Some(entry) = web_fonts.get(&id)
                {
                    return format!(
                        "{}(has_reg={}, has_bold={})",
                        entry.family_name,
                        entry.regular.is_some(),
                        entry.bold.is_some()
                    );
                }
                format!("Custom({})", id)
            }
        }
    }

    /// Maps a CSS `font-family` string to a `FontFamily` enum via the global FontManager.
    pub fn resolve_family(family_str: &str) -> FontFamily {
        font_manager().resolve_family_for(family_str)
    }

    /// Maps a CSS `font-family` string to a `FontFamily` enum, prioritizing registered web fonts
    /// and named platform system fonts (`Verdana`, `Segoe UI`, `Arial`, etc.).
    pub fn resolve_family_for(&self, family_str: &str) -> FontFamily {
        for item in family_str.split(',') {
            let mut clean = item
                .trim()
                .trim_matches('\'')
                .trim_matches('"')
                .to_ascii_lowercase();
            if clean.is_empty() {
                continue;
            }

            // `system-ui` is the platform UI font in Chromium (Segoe UI on Windows),
            // not the generic `sans-serif` family, so normalize it before checking cache.
            if matches!(
                clean.as_str(),
                "system-ui" | "-apple-system" | "blinkmacsystemfont"
            ) {
                clean = "segoe ui".to_string();
            }

            if let Ok(family_map) = self.family_to_id.read()
                && let Some(&id) = family_map.get(&clean)
            {
                return FontFamily::Custom(id);
            }

            // Check generic families early to skip disk checks
            if clean == "sans-serif" {
                return FontFamily::SansSerif;
            } else if clean == "serif" {
                return FontFamily::Serif;
            } else if clean == "monospace" {
                return FontFamily::Monospace;
            } else if clean == "cursive" || clean == "fantasy" {
                return FontFamily::SansSerif;
            }

            // Check if this is a named system font that can be loaded on demand
            let system_files = match clean.as_str() {
                "verdana" => Some(("verdana.ttf", "verdanab.ttf")),
                "segoe ui" | "segoe" => Some(("segoeui.ttf", "segoeuib.ttf")),
                "arial" | "helvetica" => Some(("arial.ttf", "arialbd.ttf")),
                "times new roman" | "times" => Some(("times.ttf", "timesbd.ttf")),
                "consolas" => Some(("consola.ttf", "consolab.ttf")),
                "georgia" => Some(("georgia.ttf", "georgiab.ttf")),
                "tahoma" => Some(("tahoma.ttf", "tahomabd.ttf")),
                "trebuchet ms" | "trebuchet" => Some(("trebuc.ttf", "trebucbd.ttf")),
                "calibri" => Some(("calibri.ttf", "calibrib.ttf")),
                "courier new" | "courier" => Some(("cour.ttf", "courbd.ttf")),
                _ => None,
            };

            if let Some((reg_file, bold_file)) = system_files {
                // A family that is exactly the platform's generic face maps straight to
                // the bundled generic: no second copy of Arial/Times/Courier is loaded.
                if (clean == "arial" || clean == "helvetica") && cfg!(target_os = "windows") {
                    return FontFamily::SansSerif;
                }
                if (clean == "times new roman" || clean == "times") && cfg!(target_os = "windows") {
                    return FontFamily::Serif;
                }
                if (clean == "courier new" || clean == "courier") && cfg!(target_os = "windows") {
                    return FontFamily::Monospace;
                }

                // Check again with write lock to avoid duplicate loading from multiple threads/passes
                let mut family_map = match self.family_to_id.write() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if let Some(&id) = family_map.get(&clean) {
                    return FontFamily::Custom(id);
                }

                if let Some(reg_font) = load_system_font(reg_file) {
                    let bold_font = load_system_font(bold_file);
                    let reg_leaked: &'static Font = Box::leak(Box::new(reg_font));
                    let bold_leaked: Option<&'static Font> = bold_font.map(|f| {
                        let b: &'static Font = Box::leak(Box::new(f));
                        b
                    });

                    let new_id = self.next_font_id.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut web_fonts) = self.web_fonts.write() {
                        family_map.insert(clean.clone(), new_id);
                        if clean == "segoe ui" {
                            family_map.insert("segoe".to_string(), new_id);
                            family_map.insert("system-ui".to_string(), new_id);
                            family_map.insert("-apple-system".to_string(), new_id);
                            family_map.insert("blinkmacsystemfont".to_string(), new_id);
                        }
                        web_fonts.insert(
                            new_id,
                            WebFontEntry {
                                family_name: clean,
                                regular: Some(reg_leaked),
                                bold: bold_leaked,
                            },
                        );
                        return FontFamily::Custom(new_id);
                    }
                }
            }
        }

        let lower = family_str.to_lowercase();
        if lower.contains("mono") || lower.contains("courier") || lower.contains("consolas") {
            FontFamily::Monospace
        } else if (lower.contains("serif") && !lower.contains("sans"))
            || lower.contains("times")
            || lower.contains("georgia")
            || lower.contains("palatino")
            || lower.contains("cambria")
        {
            FontFamily::Serif
        } else {
            FontFamily::SansSerif
        }
    }

    /// Measures the total width and height of a text string at the given font size.
    ///
    /// Returns `(width, height)` in pixels using proportional advance widths.
    pub fn measure_text(
        &self,
        text: &str,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
    ) -> (f32, f32) {
        self.measure_text_with_spacing(text, font_size, weight, family, 0.0)
    }

    /// Measures the total width and height of a text string at the given font size,
    /// adding `letter_spacing` after each character.
    pub fn measure_text_with_spacing(
        &self,
        text: &str,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        letter_spacing: f32,
    ) -> (f32, f32) {
        let primary_font = self.select_font(family, weight);
        let reg_font = self.select_font(family, FontWeight::Regular);
        let is_synth_bold = weight == FontWeight::Bold;
        let px_size = font_size.max(1.0);

        let mut total_width = 0.0f32;
        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }
            if is_emoji_char(ch) {
                total_width += px_size + letter_spacing;
                continue;
            }
            let font = self.select_font_for_char(family, weight, ch);
            let metrics = font.metrics(ch, px_size);
            let synth_extra =
                if is_synth_bold && std::ptr::eq(font, reg_font) && !ch.is_whitespace() {
                    if px_size >= 28.0 { 2.0 } else { 1.0 }
                } else {
                    0.0
                };
            total_width += metrics.advance_width + synth_extra + letter_spacing;
        }

        // Use the font's line height metrics for the vertical extent
        let line_metrics = primary_font.horizontal_line_metrics(px_size);
        let height = line_metrics
            .map(|lm| lm.new_line_size)
            .unwrap_or(px_size * 1.2);

        (total_width, height)
    }

    /// Measures text using default SansSerif family (for UI elements).
    pub fn measure_text_sans(&self, text: &str, font_size: f32, weight: FontWeight) -> (f32, f32) {
        self.measure_text(text, font_size, weight, FontFamily::SansSerif)
    }

    /// Measures the advance width of a single character.
    pub fn measure_char_width(
        &self,
        ch: char,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
    ) -> f32 {
        if is_emoji_char(ch) {
            return font_size.max(1.0);
        }
        let font = self.select_font_for_char(family, weight, ch);
        let metrics = font.metrics(ch, font_size.max(1.0));
        let synth_extra = if weight == FontWeight::Bold
            && std::ptr::eq(font, self.select_font(family, FontWeight::Regular))
            && !ch.is_whitespace()
        {
            if font_size >= 28.0 { 2.0 } else { 1.0 }
        } else {
            0.0
        };
        metrics.advance_width + synth_extra
    }

    /// Rasterizes a text string into a pixel buffer with alpha-blended glyphs and letter spacing.
    ///
    /// The buffer is `&mut [u32]` in `0xRRGGBB` format, with the given width and height.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
    ) {
        self.rasterize_text_clipped(
            buffer,
            buf_width,
            buf_height,
            x,
            y,
            text,
            color,
            font_size,
            weight,
            family,
            style,
            letter_spacing,
            mango_core::Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32),
        );
    }

    /// Retrieves a rasterized glyph from the glyph cache, or rasterizes and caches it (OPT-007).
    pub fn get_or_rasterize_glyph(
        &self,
        family: FontFamily,
        weight: FontWeight,
        font: &Font,
        ch: char,
        px_size: f32,
    ) -> CachedGlyph {
        let size_key = (px_size * 10.0).round() as u32;
        let key = (family, weight, size_key, ch);

        if let Ok(cache) = glyph_cache().read()
            && let Some(entry) = cache.get(&key)
        {
            entry.last_access.store(
                ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed),
                Ordering::Relaxed,
            );
            return entry.glyph.clone();
        }

        let (mut metrics, bitmap) = font.rasterize(ch, px_size);
        let final_bitmap: std::sync::Arc<[u8]> = if weight == FontWeight::Bold
            && std::ptr::eq(font, self.select_font(family, FontWeight::Regular))
            && metrics.width > 0
            && metrics.height > 0
            && !bitmap.is_empty()
            && !ch.is_whitespace()
        {
            let offset = if px_size >= 28.0 { 2 } else { 1 };
            let old_w = metrics.width;
            let h = metrics.height;
            let new_w = old_w + offset;
            let mut smeared = vec![0u8; new_w * h];
            for y in 0..h {
                for x in 0..old_w {
                    let cov = bitmap[y * old_w + x];
                    if cov > 0 {
                        for off in 0..=offset {
                            let dst_idx = y * new_w + x + off;
                            smeared[dst_idx] = smeared[dst_idx].max(cov);
                        }
                    }
                }
            }
            metrics.width = new_w;
            metrics.advance_width += offset as f32;
            smeared.into()
        } else {
            bitmap.into()
        };

        let cached = CachedGlyph {
            metrics,
            bitmap: final_bitmap,
        };

        if let Ok(mut cache) = glyph_cache().write() {
            if cache.len() >= 4096 {
                // Bounded LRU eviction: retain the most recent 2048 entries instead of clearing all (OPT-3.2.3)
                let mut access_times: Vec<u64> = cache
                    .values()
                    .map(|e| e.last_access.load(Ordering::Relaxed))
                    .collect();
                access_times.sort_unstable();
                let median_threshold = access_times[access_times.len() / 2];
                cache.retain(|_, v| v.last_access.load(Ordering::Relaxed) >= median_threshold);
            }
            let tick = ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed);
            cache.insert(
                key,
                GlyphCacheEntry {
                    glyph: cached.clone(),
                    last_access: AtomicU64::new(tick),
                },
            );
        }

        cached
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_clipped(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
        clip: mango_core::Rect,
    ) {
        let primary_font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);

        // Compute baseline offset: top of the em square to the baseline
        let ascent = primary_font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let is_italic = matches!(style, FontStyle::Italic | FontStyle::Oblique);
        let mut cursor_x = x as f32;

        let cx1 = clip.x().max(0.0) as i32;
        let cy1 = clip.y().max(0.0) as i32;
        let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
        let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }

            if is_emoji_char(ch) {
                let emoji = get_or_rasterize_color_emoji(ch, px_size);
                let emoji_x = cursor_x;
                let emoji_y = y as f32 + ascent - emoji.height as f32;

                for ey in 0..emoji.height {
                    for ex in 0..emoji.width {
                        let src_px = emoji.pixels[(ey * emoji.width + ex) as usize];
                        let sa = (src_px >> 24) & 0xFF;
                        if sa == 0 {
                            continue;
                        }

                        let px = (emoji_x + ex as f32) as i32;
                        let py = (emoji_y + ey as f32) as i32;

                        if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                            continue;
                        }

                        let idx = (py as u32 * buf_width + px as u32) as usize;
                        if idx >= buffer.len() {
                            continue;
                        }

                        let eff_a = ((sa * color.a as u32) + 127) / 255;
                        if eff_a == 255 {
                            buffer[idx] = src_px & 0x00FFFFFF;
                        } else if eff_a > 0 {
                            let sr = (src_px >> 16) & 0xFF;
                            let sg = (src_px >> 8) & 0xFF;
                            let sb = src_px & 0xFF;

                            let bg = buffer[idx];
                            let bg_r = (bg >> 16) & 0xFF;
                            let bg_g = (bg >> 8) & 0xFF;
                            let bg_b = bg & 0xFF;

                            let inv_a = 255 - eff_a;
                            let r = ((sr * eff_a + bg_r * inv_a + 127) / 255).min(255);
                            let g = ((sg * eff_a + bg_g * inv_a + 127) / 255).min(255);
                            let b = ((sb * eff_a + bg_b * inv_a + 127) / 255).min(255);
                            buffer[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }

                cursor_x += emoji.width as f32 + letter_spacing;
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                // Calculate glyph position relative to the baseline
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y = y as f32 + ascent - metrics.height as f32 - metrics.ymin as f32;

                // Blit the coverage bitmap with alpha blending and optional italic slant
                for gy in 0..metrics.height {
                    let slant = if is_italic {
                        (metrics.height as f32 - gy as f32) * 0.22
                    } else {
                        0.0
                    };

                    for gx in 0..metrics.width {
                        let coverage = bitmap[gy * metrics.width + gx];
                        if coverage == 0 {
                            continue;
                        }

                        let px = (glyph_x + gx as f32 + slant) as i32;
                        let py = (glyph_y + gy as f32) as i32;

                        if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                            continue;
                        }

                        let idx = (py as u32 * buf_width + px as u32) as usize;
                        if idx >= buffer.len() {
                            continue;
                        }

                        let effective_alpha = ((coverage as u32 * color.a as u32) + 127) / 255;
                        if effective_alpha == 0 {
                            continue;
                        }

                        if effective_alpha == 255 {
                            // Fully opaque — direct write
                            buffer[idx] = color.to_rgb_u32();
                        } else {
                            // Alpha blend Over
                            let inv_alpha = 255 - effective_alpha;
                            let bg = buffer[idx];
                            let bg_r = (bg >> 16) & 0xFF;
                            let bg_g = (bg >> 8) & 0xFF;
                            let bg_b = bg & 0xFF;
                            let r = ((color.r as u32 * effective_alpha + bg_r * inv_alpha + 127)
                                / 255)
                                .min(255);
                            let g = ((color.g as u32 * effective_alpha + bg_g * inv_alpha + 127)
                                / 255)
                                .min(255);
                            let b = ((color.b as u32 * effective_alpha + bg_b * inv_alpha + 127)
                                / 255)
                                .min(255);
                            buffer[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
            }

            cursor_x += metrics.advance_width + letter_spacing;
        }
    }

    /// Rasterizes text with ClearType-style subpixel LCD anti-aliasing.
    ///
    /// Applies a 3-tap FIR filter across adjacent horizontal subpixel samples
    /// [0.25, 0.5, 0.25], blending individual LCD red, green, and blue physical
    /// subpixels against the destination framebuffer to triple horizontal resolution.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_subpixel_clipped(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
        clip: mango_core::Rect,
    ) {
        let primary_font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);

        let ascent = primary_font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let is_italic = matches!(style, FontStyle::Italic | FontStyle::Oblique);
        let mut cursor_x = x as f32;

        let cx1 = clip.x().max(0.0) as i32;
        let cy1 = clip.y().max(0.0) as i32;
        let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
        let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }

            if is_emoji_char(ch) {
                let emoji = get_or_rasterize_color_emoji(ch, px_size);
                let emoji_x = cursor_x;
                let emoji_y = y as f32 + ascent - emoji.height as f32;

                for ey in 0..emoji.height {
                    for ex in 0..emoji.width {
                        let src_px = emoji.pixels[(ey * emoji.width + ex) as usize];
                        let sa = (src_px >> 24) & 0xFF;
                        if sa == 0 {
                            continue;
                        }

                        let px = (emoji_x + ex as f32) as i32;
                        let py = (emoji_y + ey as f32) as i32;

                        if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                            continue;
                        }

                        let idx = (py as u32 * buf_width + px as u32) as usize;
                        if idx >= buffer.len() {
                            continue;
                        }

                        let eff_a = ((sa * color.a as u32) + 127) / 255;
                        if eff_a == 255 {
                            buffer[idx] = src_px & 0x00FFFFFF;
                        } else if eff_a > 0 {
                            let sr = (src_px >> 16) & 0xFF;
                            let sg = (src_px >> 8) & 0xFF;
                            let sb = src_px & 0xFF;

                            let bg = buffer[idx];
                            let bg_r = (bg >> 16) & 0xFF;
                            let bg_g = (bg >> 8) & 0xFF;
                            let bg_b = bg & 0xFF;

                            let inv_a = 255 - eff_a;
                            let r = ((sr * eff_a + bg_r * inv_a + 127) / 255).min(255);
                            let g = ((sg * eff_a + bg_g * inv_a + 127) / 255).min(255);
                            let b = ((sb * eff_a + bg_b * inv_a + 127) / 255).min(255);
                            buffer[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }

                cursor_x += emoji.width as f32 + letter_spacing;
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y = y as f32 + ascent - metrics.height as f32 - metrics.ymin as f32;
                let w = metrics.width;

                for gy in 0..metrics.height {
                    let slant = if is_italic {
                        (metrics.height as f32 - gy as f32) * 0.22
                    } else {
                        0.0
                    };

                    let row_start = gy * w;
                    for gx in 0..w {
                        let center = bitmap[row_start + gx] as u32;
                        let left = if gx > 0 {
                            bitmap[row_start + gx - 1] as u32
                        } else {
                            0
                        };
                        let right = if gx + 1 < w {
                            bitmap[row_start + gx + 1] as u32
                        } else {
                            0
                        };

                        let cov_r = ((left + 2 * center + 1) / 3).min(255);
                        let cov_g = center.min(255);
                        let cov_b = ((2 * center + right + 1) / 3).min(255);

                        if cov_r == 0 && cov_g == 0 && cov_b == 0 {
                            continue;
                        }

                        let px = (glyph_x + gx as f32 + slant) as i32;
                        let py = (glyph_y + gy as f32) as i32;

                        if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                            continue;
                        }

                        let idx = (py as u32 * buf_width + px as u32) as usize;
                        if idx >= buffer.len() {
                            continue;
                        }

                        let eff_r = ((cov_r * color.a as u32) + 127) / 255;
                        let eff_g = ((cov_g * color.a as u32) + 127) / 255;
                        let eff_b = ((cov_b * color.a as u32) + 127) / 255;

                        let bg = buffer[idx];
                        let bg_r = (bg >> 16) & 0xFF;
                        let bg_g = (bg >> 8) & 0xFF;
                        let bg_b = bg & 0xFF;

                        let out_r =
                            ((color.r as u32 * eff_r + bg_r * (255 - eff_r) + 127) / 255).min(255);
                        let out_g =
                            ((color.g as u32 * eff_g + bg_g * (255 - eff_g) + 127) / 255).min(255);
                        let out_b =
                            ((color.b as u32 * eff_b + bg_b * (255 - eff_b) + 127) / 255).min(255);

                        buffer[idx] = (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }

            cursor_x += metrics.advance_width + letter_spacing;
        }
    }

    /// Rasterizes text with ClearType-style subpixel LCD anti-aliasing without clipping.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_subpixel(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
    ) {
        self.rasterize_text_subpixel_clipped(
            buffer,
            buf_width,
            buf_height,
            x,
            y,
            text,
            color,
            font_size,
            weight,
            family,
            style,
            letter_spacing,
            mango_core::Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32),
        );
    }

    /// Rasterizes text clipped to a gradient fill (used for `background-clip: text`).
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_text_gradient_clipped(
        &self,
        buffer: &mut [u32],
        buf_width: u32,
        buf_height: u32,
        x: i32,
        y: i32,
        text: &str,
        gradient: &mango_css::values::Gradient,
        gradient_rect: mango_core::Rect,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        letter_spacing: f32,
        clip: mango_core::Rect,
    ) {
        let gw = gradient_rect.width().max(1.0) as u32;
        let gh = gradient_rect.height().max(1.0) as u32;
        let grad_pixels = gradient.rasterize(gw, gh);
        if grad_pixels.is_empty() {
            return;
        }

        let primary_font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);

        let ascent = primary_font
            .horizontal_line_metrics(px_size)
            .map(|lm| lm.ascent)
            .unwrap_or(px_size * 0.8);

        let is_italic = matches!(style, FontStyle::Italic | FontStyle::Oblique);
        let mut cursor_x = x as f32;

        let cx1 = clip.x().max(0.0) as i32;
        let cy1 = clip.y().max(0.0) as i32;
        let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
        let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }

            let font = self.select_font_for_char(family, weight, ch);
            let cached = self.get_or_rasterize_glyph(family, weight, font, ch, px_size);
            let metrics = &cached.metrics;
            let bitmap = &cached.bitmap;

            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                let glyph_x = cursor_x + metrics.xmin as f32;
                let glyph_y = y as f32 + ascent - metrics.height as f32 - metrics.ymin as f32;

                for gy in 0..metrics.height {
                    let slant = if is_italic {
                        (metrics.height as f32 - gy as f32) * 0.22
                    } else {
                        0.0
                    };

                    for gx in 0..metrics.width {
                        let coverage = bitmap[gy * metrics.width + gx];
                        if coverage == 0 {
                            continue;
                        }

                        let px = (glyph_x + gx as f32 + slant) as i32;
                        let py = (glyph_y + gy as f32) as i32;

                        if px < cx1 || py < cy1 || px >= cx2 || py >= cy2 {
                            continue;
                        }

                        let idx = (py as u32 * buf_width + px as u32) as usize;
                        if idx >= buffer.len() {
                            continue;
                        }

                        let grad_x =
                            (((px as f32 - gradient_rect.x()) / gradient_rect.width().max(1.0))
                                * gw as f32)
                                .clamp(0.0, (gw - 1) as f32) as u32;
                        let grad_y =
                            (((py as f32 - gradient_rect.y()) / gradient_rect.height().max(1.0))
                                * gh as f32)
                                .clamp(0.0, (gh - 1) as f32) as u32;
                        let src = grad_pixels[(grad_y * gw + grad_x) as usize];
                        let src_a = (src >> 24) & 0xFF;
                        let src_r = (src >> 16) & 0xFF;
                        let src_g = (src >> 8) & 0xFF;
                        let src_b = src & 0xFF;

                        let effective_alpha = ((coverage as u32 * src_a) + 127) / 255;
                        if effective_alpha == 0 {
                            continue;
                        }

                        if effective_alpha == 255 {
                            buffer[idx] = (src_r << 16) | (src_g << 8) | src_b;
                        } else {
                            let inv_alpha = 255 - effective_alpha;
                            let bg = buffer[idx];
                            let bg_r = (bg >> 16) & 0xFF;
                            let bg_g = (bg >> 8) & 0xFF;
                            let bg_b = bg & 0xFF;
                            let r =
                                ((src_r * effective_alpha + bg_r * inv_alpha + 127) / 255).min(255);
                            let g =
                                ((src_g * effective_alpha + bg_g * inv_alpha + 127) / 255).min(255);
                            let b =
                                ((src_b * effective_alpha + bg_b * inv_alpha + 127) / 255).min(255);
                            buffer[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
            }

            cursor_x += metrics.advance_width + letter_spacing;
        }
    }
}

// FontManager is safe to share across threads: fontdue::Font is Send + Sync,
// and we never mutate the fonts after construction.
unsafe impl Send for FontManager {}
unsafe impl Sync for FontManager {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_initializes() {
        let fm = font_manager();
        // Should not panic — fonts parsed successfully
        let (w, h) = fm.measure_text("Hello", 16.0, FontWeight::Regular, FontFamily::SansSerif);
        assert!(w > 0.0, "text width should be positive");
        assert!(h > 0.0, "text height should be positive");
    }

    #[test]
    fn test_proportional_measurement() {
        let fm = font_manager();
        let (w_wide, _) =
            fm.measure_text("WWWWW", 16.0, FontWeight::Regular, FontFamily::SansSerif);
        let (w_narrow, _) =
            fm.measure_text("iiiii", 16.0, FontWeight::Regular, FontFamily::SansSerif);
        // 'W' should be wider than 'i' in a proportional font
        assert!(
            w_wide > w_narrow,
            "W ({w_wide}) should be wider than i ({w_narrow})"
        );
    }

    #[test]
    fn test_bold_measurement() {
        let fm = font_manager();
        let (w_regular, _) =
            fm.measure_text("Hello", 16.0, FontWeight::Regular, FontFamily::SansSerif);
        let (w_bold, _) = fm.measure_text("Hello", 16.0, FontWeight::Bold, FontFamily::SansSerif);
        println!("w_regular={}, w_bold={}", w_regular, w_bold);
        assert!(w_regular > 0.0);
        assert!(w_bold > 0.0);
    }

    #[test]
    fn test_serif_measurement() {
        let fm = font_manager();
        let (w_serif, h_serif) =
            fm.measure_text("Moby-Dick", 16.0, FontWeight::Regular, FontFamily::Serif);
        let (w_serif_bold, _) =
            fm.measure_text("Moby-Dick", 16.0, FontWeight::Bold, FontFamily::Serif);
        assert!(w_serif > 0.0, "serif width should be positive");
        assert!(h_serif > 0.0, "serif height should be positive");
        assert!(w_serif_bold > 0.0, "serif bold width should be positive");
    }

    #[test]
    fn test_mono_measurement() {
        let fm = font_manager();
        // In a monospace font, each character should have the same width
        let w_a = fm
            .select_font(FontFamily::Monospace, FontWeight::Regular)
            .metrics('a', 16.0)
            .advance_width;
        let w_m = fm
            .select_font(FontFamily::Monospace, FontWeight::Regular)
            .metrics('M', 16.0)
            .advance_width;
        assert!(
            (w_a - w_m).abs() < 0.01,
            "monospace chars should have equal width: a={w_a}, M={w_m}"
        );
    }

    #[test]
    fn test_rasterize_text_no_panic() {
        let fm = font_manager();
        let mut buffer = vec![0xFFFFFFu32; 200 * 50];
        fm.rasterize_text(
            &mut buffer,
            200,
            50,
            5,
            5,
            "Hello World!",
            Color::BLACK,
            16.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
            FontStyle::Normal,
            0.0,
        );
        // At least some pixels should have been modified from the white background
        let modified = buffer.iter().filter(|&&px| px != 0xFFFFFF).count();
        assert!(
            modified > 0,
            "rasterize_text should have painted some pixels"
        );
    }

    #[test]
    fn test_resolve_family() {
        assert_eq!(
            FontManager::resolve_family("monospace"),
            FontFamily::Monospace
        );
        assert_eq!(
            FontManager::resolve_family("Courier New"),
            FontFamily::Monospace
        );
        assert_eq!(FontManager::resolve_family("Arial"), FontFamily::SansSerif);
        assert_eq!(
            FontManager::resolve_family("sans-serif"),
            FontFamily::SansSerif
        );
        assert_eq!(FontManager::resolve_family("serif"), FontFamily::Serif);
        assert_eq!(
            FontManager::resolve_family("Times New Roman"),
            FontFamily::Serif
        );
        let georgia = FontManager::resolve_family("Georgia");
        assert!(
            georgia == FontFamily::Serif || matches!(georgia, FontFamily::Custom(_)),
            "Georgia should resolve to Custom(id) if installed on system, or fallback to Serif"
        );
    }

    #[test]
    fn test_web_font_registration_and_resolution() {
        let fm = font_manager();
        assert!(!fm.has_web_font("CustomWikipediaSans"));

        // Register embedded TTF as a new custom web font
        let font_id = fm
            .register_web_font("CustomWikipediaSans", FontWeight::Regular, FONT_SANS)
            .expect("should register TTF bytes as web font");

        assert!(fm.has_web_font("CustomWikipediaSans"));
        assert!(fm.has_web_font("customwikipediasans")); // case-insensitive check

        // Resolving family with fallback list should pick the registered custom font
        let resolved = fm.resolve_family_for("'CustomWikipediaSans', sans-serif");
        assert_eq!(resolved, FontFamily::Custom(font_id));

        // Measuring text with custom font should succeed and yield valid dimensions
        let (w, h) = fm.measure_text("Wikipedia Parity", 18.0, FontWeight::Regular, resolved);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_web_font_registration_via_data_uri() {
        let fm = font_manager();
        // A minimal dummy data URI with valid TTF prefix but too short payload
        let result = fm.register_web_font(
            "BadFont",
            FontWeight::Regular,
            b"data:font/woff2;base64,AAAA",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_glyph_caching() {
        let fm = font_manager();
        let font = fm.select_font(FontFamily::SansSerif, FontWeight::Regular);
        let g1 =
            fm.get_or_rasterize_glyph(FontFamily::SansSerif, FontWeight::Regular, font, 'M', 16.0);
        let g2 =
            fm.get_or_rasterize_glyph(FontFamily::SansSerif, FontWeight::Regular, font, 'M', 16.0);
        assert_eq!(g1.metrics.width, g2.metrics.width);
        assert_eq!(g1.metrics.height, g2.metrics.height);
        assert_eq!(g1.bitmap.len(), g2.bitmap.len());
        // Verify bitmap Arc pointer equality to confirm it was retrieved from cache without re-rasterizing
        assert!(std::sync::Arc::ptr_eq(&g1.bitmap, &g2.bitmap));
    }

    #[test]
    fn test_indic_font_fallback() {
        let fm = font_manager();
        // Google offered in: Hindi, Bengali, Telugu
        let hindi_text = "हिन्दी";
        let (w, h) = fm.measure_text(hindi_text, 14.0, FontWeight::Regular, FontFamily::SansSerif);
        assert!(w > 0.0);
        assert!(h > 0.0);

        let mut buf = vec![0u32; 200 * 50];
        fm.rasterize_text(
            &mut buf,
            200,
            50,
            0,
            0,
            hindi_text,
            Color::WHITE,
            14.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
            FontStyle::Normal,
            0.0,
        );
        // Verify non-zero pixels were written
        assert!(buf.iter().any(|&p| p != 0));
    }

    #[test]
    fn test_multilingual_font_fallback() {
        let fm = font_manager();
        let samples = [
            ("Chinese", "芒果浏览器"),
            ("Japanese", "こんにちは世界"),
            ("Korean", "안녕하세요"),
            ("Arabic", "العربية"),
            ("Thai", "ภาษาไทย"),
            ("Hebrew", "שלום עולם"),
            ("Emoji", "🥭🚀"),
        ];

        for (lang, text) in samples {
            let (w, h) = fm.measure_text(text, 16.0, FontWeight::Regular, FontFamily::SansSerif);
            assert!(w > 0.0, "Language {lang} width must be > 0, got {w}");
            assert!(h > 0.0, "Language {lang} height must be > 0, got {h}");

            let mut buf = vec![0u32; 300 * 60];
            fm.rasterize_text(
                &mut buf,
                300,
                60,
                0,
                0,
                text,
                Color::WHITE,
                16.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
                FontStyle::Normal,
                0.0,
            );
            assert!(
                buf.iter().any(|&p| p != 0),
                "Language {lang} must rasterize non-zero pixels"
            );
        }
    }

    #[test]
    fn test_subpixel_text_rendering() {
        let fm = font_manager();
        let mut buffer = vec![0xFFFFFFu32; 200 * 50];
        fm.rasterize_text_subpixel(
            &mut buffer,
            200,
            50,
            10,
            10,
            "ClearType Subpixel",
            Color::BLACK,
            16.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
            FontStyle::Normal,
            0.0,
        );

        // Check that pixels were drawn
        let modified = buffer.iter().filter(|&&px| px != 0xFFFFFF).count();
        assert!(modified > 0, "Subpixel rendering must modify buffer pixels");

        // Verify that subpixel channels differ on anti-aliased edge pixels (ClearType FIR characteristic)
        let has_colored_subpixel_edges = buffer.iter().any(|&px| {
            let r = (px >> 16) & 0xFF;
            let g = (px >> 8) & 0xFF;
            let b = px & 0xFF;
            r != g || g != b
        });
        assert!(
            has_colored_subpixel_edges,
            "Subpixel rendering must yield distinct R, G, B channels on edge pixels"
        );
    }

    #[test]
    fn test_color_emoji_rendering() {
        assert!(is_emoji_char('🥭'));
        assert!(is_emoji_char('🚀'));
        assert!(is_emoji_char('😀'));
        assert!(is_emoji_char('🔥'));
        assert!(is_emoji_char('🎉'));
        assert!(is_emoji_char('⭐'));
        assert!(is_emoji_char('\u{2764}'));
        assert!(!is_emoji_char('A'));
        assert!(!is_emoji_char('7'));

        let mango_emoji = rasterize_color_emoji('🥭', 24);
        assert_eq!(mango_emoji.width, 24);
        assert_eq!(mango_emoji.height, 24);
        assert!(!mango_emoji.pixels.is_empty());

        // Verify multi-colored non-monochrome pixels (e.g. green leaf vs orange body)
        let has_warm_orange = mango_emoji.pixels.iter().any(|&p| {
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            r > 200 && g > 80 && g < 220 && b < 80
        });
        let has_green_leaf = mango_emoji.pixels.iter().any(|&p| {
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            g > r && g > b && g > 80
        });
        assert!(
            has_warm_orange,
            "Mango emoji must contain golden/orange body pixels"
        );
        assert!(has_green_leaf, "Mango emoji must contain green leaf pixels");

        // Test drawing emoji in text run
        let fm = font_manager();
        let mut buf = vec![0xFFFFFFu32; 100 * 40];
        fm.rasterize_text(
            &mut buf,
            100,
            40,
            5,
            5,
            "🥭 Rocket 🚀",
            Color::BLACK,
            20.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
            FontStyle::Normal,
            0.0,
        );
        let non_white = buf.iter().filter(|&&p| p != 0xFFFFFF).count();
        assert!(non_white > 0, "Emoji text rasterization must draw pixels");
    }

    #[test]
    fn test_variable_font_axis_interpolation() {
        // Construct a synthetic TrueType header with an fvar table
        let mut ttf = Vec::new();
        // sfnt header: version 0x00010000, num_tables = 1
        ttf.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        ttf.extend_from_slice(&1u16.to_be_bytes()); // num_tables
        ttf.extend_from_slice(&[0x00, 0x10, 0x00, 0x00, 0x00, 0x00]); // search_range, entry_selector, range_shift

        let fvar_offset = 12 + 16;
        // Table record: tag 'fvar', checksum, offset, length
        ttf.extend_from_slice(b"fvar");
        ttf.extend_from_slice(&0u32.to_be_bytes());
        ttf.extend_from_slice(&(fvar_offset as u32).to_be_bytes());
        let fvar_len = 16 + 20; // header (16) + 1 axis (20)
        ttf.extend_from_slice(&(fvar_len as u32).to_be_bytes());

        // fvar header:
        ttf.extend_from_slice(&1u16.to_be_bytes()); // major_version = 1
        ttf.extend_from_slice(&0u16.to_be_bytes()); // minor_version = 0
        ttf.extend_from_slice(&16u16.to_be_bytes()); // axes_array_offset = 16
        ttf.extend_from_slice(&2u16.to_be_bytes()); // reserved = 2
        ttf.extend_from_slice(&1u16.to_be_bytes()); // axis_count = 1
        ttf.extend_from_slice(&20u16.to_be_bytes()); // axis_size = 20
        ttf.extend_from_slice(&0u16.to_be_bytes()); // instance_count = 0
        ttf.extend_from_slice(&0u16.to_be_bytes()); // instance_size = 0

        // Axis record for 'wght': min=100.0 (100 * 65536), default=400.0 (400 * 65536), max=900.0 (900 * 65536)
        ttf.extend_from_slice(b"wght");
        ttf.extend_from_slice(&(100i32 * 65536).to_be_bytes());
        ttf.extend_from_slice(&(400i32 * 65536).to_be_bytes());
        ttf.extend_from_slice(&(900i32 * 65536).to_be_bytes());
        ttf.extend_from_slice(&0u16.to_be_bytes()); // flags
        ttf.extend_from_slice(&256u16.to_be_bytes()); // name_id

        let meta = parse_fvar_table(&ttf).expect("fvar table should parse successfully");
        assert_eq!(meta.axes.len(), 1);
        let wght = &meta.axes[0];
        assert_eq!(&wght.tag, b"wght");
        assert_eq!(wght.tag_str, "wght");
        assert!((wght.min_value - 100.0).abs() < 0.01);
        assert!((wght.default_value - 400.0).abs() < 0.01);
        assert!((wght.max_value - 900.0).abs() < 0.01);

        // Test axis coordinate interpolation:
        // Default weight 400 -> 0.0
        assert_eq!(interpolate_axis(wght, 400.0), 0.0);
        // Min weight 100 -> -1.0
        assert!((interpolate_axis(wght, 100.0) - (-1.0)).abs() < 0.001);
        // Max weight 900 -> 1.0
        assert!((interpolate_axis(wght, 900.0) - 1.0).abs() < 0.001);
        // Intermediate weight 650 -> (650 - 400) / (900 - 400) = 250 / 500 = 0.5
        assert!((interpolate_axis(wght, 650.0) - 0.5).abs() < 0.001);
    }
}
