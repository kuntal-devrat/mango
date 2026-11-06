//! Font types, styles, metrics entries, and cache keys.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use fontdue::Font;

static WEB_FONT_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Returns the current web font generation counter.
/// Increments every time a new web font face is loaded and registered.
pub fn web_font_epoch() -> u64 {
    WEB_FONT_EPOCH.load(Ordering::Relaxed)
}

/// Signals that new web fonts were registered or mutated, requiring layout/display-list invalidation.
pub fn bump_web_font_epoch() {
    WEB_FONT_EPOCH.fetch_add(1, Ordering::SeqCst);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Regular,
    Bold,
}

/// A cached rasterized glyph with its metrics and 8-bit coverage bitmap.
#[derive(Clone)]
pub struct CachedGlyph {
    pub metrics: fontdue::Metrics,
    pub bitmap: std::sync::Arc<[u8]>,
}

/// Discriminant key for cached rasterized glyphs (either Unicode character or OpenType glyph index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphKey {
    Char(char),
    Index { glyph_id: u16, font_ptr: usize },
}

pub(crate) type GlyphCacheKey = (FontFamily, FontWeight, u32, GlyphKey);

pub(crate) struct GlyphCacheEntry {
    pub(crate) glyph: CachedGlyph,
    pub(crate) last_access: AtomicU64,
}

pub(crate) static ACCESS_COUNTER: AtomicU64 = AtomicU64::new(1);
pub(crate) static GLYPH_CACHE_BYTES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
pub(crate) const MAX_GLYPH_CACHE_BYTES: usize = 16 * 1024 * 1024;
static GLYPH_CACHE: OnceLock<RwLock<HashMap<GlyphCacheKey, GlyphCacheEntry>>> = OnceLock::new();

pub(crate) fn glyph_cache() -> &'static RwLock<HashMap<GlyphCacheKey, GlyphCacheEntry>> {
    GLYPH_CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(1024)))
}

pub(crate) fn store_in_glyph_cache(key: GlyphCacheKey, cached: CachedGlyph) {
    if let Ok(mut cache) = glyph_cache().write() {
        let new_bytes = cached.bitmap.len();
        while new_bytes <= MAX_GLYPH_CACHE_BYTES
            && GLYPH_CACHE_BYTES.load(Ordering::Relaxed) + new_bytes > MAX_GLYPH_CACHE_BYTES
            && !cache.is_empty()
        {
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_access.load(Ordering::Relaxed))
                .map(|(key, _)| *key);
            if let Some(k) = oldest_key {
                if let Some(removed) = cache.remove(&k) {
                    GLYPH_CACHE_BYTES.fetch_sub(
                        removed.glyph.bitmap.len(),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
            }
        }
        if let Some(old) = cache.remove(&key) {
            GLYPH_CACHE_BYTES
                .fetch_sub(old.glyph.bitmap.len(), std::sync::atomic::Ordering::Relaxed);
        }
        if new_bytes <= MAX_GLYPH_CACHE_BYTES {
            let tick = ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed);
            cache.insert(
                key,
                GlyphCacheEntry {
                    glyph: cached,
                    last_access: AtomicU64::new(tick),
                },
            );
            GLYPH_CACHE_BYTES.fetch_add(new_bytes, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl super::manager::FontManager {
    /// Retrieves a rasterized glyph from the glyph cache, or rasterizes and caches it.
    pub fn get_or_rasterize_glyph(
        &self,
        family: FontFamily,
        weight: FontWeight,
        font: &Font,
        ch: char,
        px_size: f32,
    ) -> CachedGlyph {
        let size_key = (px_size * 10.0).round() as u32;
        let key = (family, weight, size_key, GlyphKey::Char(ch));

        if let Ok(cache) = glyph_cache().read()
            && let Some(entry) = cache.get(&key)
        {
            entry.last_access.store(
                ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed),
                Ordering::Relaxed,
            );
            return entry.glyph.clone();
        }

        let is_ahem = match family {
            FontFamily::Custom(id) => {
                if let Ok(web_fonts) = self.web_fonts.read() {
                    web_fonts
                        .get(&id)
                        .map(|e| e.family_name.eq_ignore_ascii_case("Ahem"))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            _ => false,
        };

        let (mut metrics, bitmap) = font.rasterize(ch, px_size);
        if is_ahem && metrics.width > 0 && metrics.height > 0 && !bitmap.is_empty() {
            let target_w = metrics.bounds.width.round().max(1.0) as usize;
            let target_h = metrics.bounds.height.round().max(1.0) as usize;
            let solid = vec![255u8; target_w * target_h];
            metrics.width = target_w;
            metrics.height = target_h;
            metrics.xmin = metrics.bounds.xmin.round() as i32;
            metrics.ymin = metrics.bounds.ymin.round() as i32;
            let cached = CachedGlyph {
                metrics,
                bitmap: solid.into(),
            };
            store_in_glyph_cache(key, cached.clone());
            return cached;
        }
        let final_bitmap: std::sync::Arc<[u8]> = if weight == FontWeight::Bold
            && std::ptr::eq(font, self.select_font(family, FontWeight::Regular).as_ref())
            && metrics.width > 0
            && metrics.height > 0
            && !bitmap.is_empty()
            && !ch.is_whitespace()
        {
            let offset = if px_size >= 48.0 {
                3
            } else if px_size >= 28.0 {
                2
            } else {
                1
            };
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
        store_in_glyph_cache(key, cached.clone());
        cached
    }

    /// Retrieves an indexed rasterized glyph from the glyph cache, or rasterizes and caches it.
    pub fn get_or_rasterize_glyph_indexed(
        &self,
        family: FontFamily,
        weight: FontWeight,
        font: &Font,
        glyph_id: u16,
        px_size: f32,
    ) -> CachedGlyph {
        let size_key = (px_size * 10.0).round() as u32;
        let key = (
            family,
            weight,
            size_key,
            GlyphKey::Index {
                glyph_id,
                font_ptr: font as *const Font as usize,
            },
        );

        if let Ok(cache) = glyph_cache().read()
            && let Some(entry) = cache.get(&key)
        {
            entry.last_access.store(
                ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed),
                Ordering::Relaxed,
            );
            return entry.glyph.clone();
        }

        let (metrics, bitmap) = font.rasterize_indexed(glyph_id, px_size);
        let cached = CachedGlyph {
            metrics,
            bitmap: bitmap.into(),
        };
        store_in_glyph_cache(key, cached.clone());
        cached
    }
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
        super::manager::FontManager::resolve_family(name)
    }
}

/// Text antialiasing mode (ClearType-style subpixel LCD vs. grayscale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAntialias {
    #[default]
    Grayscale,
    SubpixelLcd,
}

pub(crate) const MAX_WEB_FONT_FAMILIES: usize = 64;
pub(crate) static FONT_ACCESS_TICK: AtomicU64 = AtomicU64::new(1);

pub(crate) struct WebFontEntry {
    #[allow(dead_code)]
    pub(crate) family_name: String,
    pub(crate) regular: Vec<Arc<Font>>,
    pub(crate) regular_data: Vec<Arc<[u8]>>,
    pub(crate) bold: Vec<Arc<Font>>,
    pub(crate) bold_data: Vec<Arc<[u8]>>,
    pub(crate) is_system: bool,
    pub(crate) last_accessed: AtomicU64,
}
