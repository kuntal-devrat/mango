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

pub(crate) type GlyphCacheKey = (FontFamily, FontWeight, u32, char);

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
    pub(crate) bold: Vec<Arc<Font>>,
    pub(crate) is_system: bool,
    pub(crate) last_accessed: AtomicU64,
}
