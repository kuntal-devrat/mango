//! Caching, global symbol registration, and high-level SVG rendering entrypoints.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use mango_core::Color;
use tiny_skia::{Color as SkiaColor, Pixmap, Transform};

use super::ast::{SvgDocument, SvgNode, SvgSymbol};
use crate::image_decode::DecodedImage;

/// Bounded memory cache for rendered SVG pixmaps.
struct SvgCacheEntry {
    image: DecodedImage,
    last_accessed: std::sync::atomic::AtomicU64,
}

static SVG_ACCESS_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) struct BoundedSvgRenderStore<K: std::hash::Hash + Eq + Clone> {
    entries: HashMap<K, SvgCacheEntry>,
    total_bytes: usize,
    max_bytes: usize,
}

impl<K: std::hash::Hash + Eq + Clone> BoundedSvgRenderStore<K> {
    fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(256),
            total_bytes: 0,
            max_bytes,
        }
    }

    fn insert(&mut self, key: K, img: DecodedImage) {
        let entry_bytes = img.pixels.len() * 4;
        if entry_bytes > self.max_bytes {
            return;
        }

        if let Some(old) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(old.image.pixels.len() * 4);
        }

        while (self.total_bytes + entry_bytes > self.max_bytes || self.entries.len() >= 512)
            && !self.entries.is_empty()
        {
            let oldest_key = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| {
                    entry
                        .last_accessed
                        .load(std::sync::atomic::Ordering::Relaxed)
                })
                .map(|(k, _)| k.clone());

            let Some(k) = oldest_key else {
                break;
            };
            if let Some(removed) = self.entries.remove(&k) {
                self.total_bytes = self
                    .total_bytes
                    .saturating_sub(removed.image.pixels.len() * 4);
            }
        }

        self.total_bytes += entry_bytes;
        let tick = SVG_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.entries.insert(
            key,
            SvgCacheEntry {
                image: img,
                last_accessed: std::sync::atomic::AtomicU64::new(tick),
            },
        );
    }

    fn get(&self, key: &K) -> Option<&DecodedImage> {
        let entry = self.entries.get(key)?;
        entry.last_accessed.store(
            SVG_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        Some(&entry.image)
    }

    #[allow(dead_code)]
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }
}

pub(crate) struct SvgDocStore {
    entries: HashMap<String, std::sync::Arc<SvgDocument>>,
    total_source_bytes: usize,
}

impl SvgDocStore {
    const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;

    fn new() -> Self {
        Self {
            entries: HashMap::with_capacity(128),
            total_source_bytes: 0,
        }
    }

    fn get(&self, key: &str) -> Option<std::sync::Arc<SvgDocument>> {
        self.entries.get(key).cloned()
    }

    #[cfg(test)]
    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    fn insert(&mut self, key: String, value: std::sync::Arc<SvgDocument>) {
        let bytes = key.len();
        if bytes > Self::MAX_SOURCE_BYTES {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            // The key owns the source string; recover its size from the old
            // entry by conservatively rebuilding the total below if needed.
            let _ = old;
            self.total_source_bytes = self.entries.keys().map(String::len).sum();
        }
        while self.total_source_bytes + bytes > Self::MAX_SOURCE_BYTES && !self.entries.is_empty() {
            if let Some(oldest) = self.entries.keys().next().cloned() {
                self.entries.remove(&oldest);
                self.total_source_bytes = self.entries.keys().map(String::len).sum();
            }
        }
        self.total_source_bytes += bytes;
        self.entries.insert(key, value);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.total_source_bytes = 0;
    }
}

static SVG_DOC_CACHE: OnceLock<RwLock<SvgDocStore>> = OnceLock::new();
static SVG_RENDER_CACHE: OnceLock<RwLock<BoundedSvgRenderStore<(String, u32, u32, u32)>>> =
    OnceLock::new();

pub(crate) fn svg_doc_cache() -> &'static RwLock<SvgDocStore> {
    SVG_DOC_CACHE.get_or_init(|| RwLock::new(SvgDocStore::new()))
}

pub(crate) fn svg_render_cache() -> &'static RwLock<BoundedSvgRenderStore<(String, u32, u32, u32)>>
{
    SVG_RENDER_CACHE.get_or_init(|| RwLock::new(BoundedSvgRenderStore::new(16 * 1024 * 1024)))
}

/// Evicts all cached SVG DOMs and rendered SVG pixmaps to reclaim memory.
pub fn prune_svg_caches() {
    if let Ok(mut cache) = svg_doc_cache().write() {
        cache.clear();
    }
    if let Ok(mut cache) = svg_render_cache().write() {
        cache.entries.clear();
        cache.total_bytes = 0;
    }
}

/// Render an SVG XML string into a [`DecodedImage`] of `(target_w, target_h)` pixels.
/// Caches both parsed SVG DOMs and rendered pixmaps to avoid re-parsing on every paint.
///
/// When `currentColor` is used in SVG attributes (`fill="currentColor"` or `stroke="currentColor"`),
/// it resolves to the provided `current_color`.
pub fn render_svg(
    svg_xml: &str,
    target_w: u32,
    target_h: u32,
    current_color: Color,
) -> Option<DecodedImage> {
    if target_w == 0 || target_h == 0 {
        return None;
    }

    let color_key = current_color.to_argb_u32();
    let cache_key = (svg_xml.to_string(), target_w, target_h, color_key);

    // 1. Check rendered pixmap cache
    if let Ok(cache) = svg_render_cache().read()
        && let Some(img) = cache.get(&cache_key)
    {
        return Some(img.clone());
    }

    // 2. Check parsed SVG DOM cache
    let doc: std::sync::Arc<SvgDocument> = {
        let parsed_opt = if let Ok(cache) = svg_doc_cache().read() {
            cache.get(svg_xml)
        } else {
            None
        };

        if let Some(d) = parsed_opt {
            d
        } else {
            let parsed = SvgDocument::parse(svg_xml)?;
            let arc_doc = std::sync::Arc::new(parsed);
            if let Ok(mut cache) = svg_doc_cache().write() {
                cache.insert(svg_xml.to_string(), arc_doc.clone());
            }
            arc_doc
        }
    };

    let mut pixmap = Pixmap::new(target_w, target_h)?;
    pixmap.fill(SkiaColor::TRANSPARENT);

    // Determine coordinate transformation
    let (vb_x, vb_y, vb_w, vb_h) = if let Some(vb) = doc.view_box {
        vb
    } else {
        let w = doc.width.unwrap_or(target_w as f32);
        let h = doc.height.unwrap_or(target_h as f32);
        (0.0, 0.0, w, h)
    };

    if vb_w <= 0.0 || vb_h <= 0.0 {
        return None;
    }

    // Uniform fit (meet) preserving aspect ratio, centered in viewport
    let scale_x = target_w as f32 / vb_w;
    let scale_y = target_h as f32 / vb_h;
    let scale = scale_x.min(scale_y);

    let offset_x = (target_w as f32 - vb_w * scale) / 2.0;
    let offset_y = (target_h as f32 - vb_h * scale) / 2.0;

    let transform = Transform::from_translate(offset_x, offset_y)
        .pre_scale(scale, scale)
        .pre_translate(-vb_x, -vb_y);

    let initial_state = doc.root_state.clone();

    doc.render(&mut pixmap, transform, &initial_state, current_color);

    // Convert pixmap to DecodedImage
    let mut pixels = Vec::with_capacity((target_w * target_h) as usize);
    for pixel in pixmap.pixels() {
        let a = pixel.alpha();
        let r = pixel.red();
        let g = pixel.green();
        let b = pixel.blue();
        pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
    }

    let decoded = DecodedImage {
        width: target_w,
        height: target_h,
        pixels,
    };

    // Store in render cache
    if let Ok(mut cache) = svg_render_cache().write() {
        cache.insert(cache_key, decoded.clone());
    }

    Some(decoded)
}

static GLOBAL_SVG_SYMBOLS: OnceLock<RwLock<HashMap<String, SvgSymbol>>> = OnceLock::new();

fn global_svg_symbols() -> &'static RwLock<HashMap<String, SvgSymbol>> {
    GLOBAL_SVG_SYMBOLS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Registers an SVG symbol in the global cross-SVG symbol registry.
pub fn register_global_svg_symbol(id: String, symbol: SvgSymbol) {
    if !id.is_empty()
        && let Ok(mut lock) = global_svg_symbols().write()
    {
        lock.insert(id, symbol);
    }
}

/// Looks up an SVG symbol in the global cross-SVG symbol registry by ID.
pub fn get_global_svg_symbol(id: &str) -> Option<SvgSymbol> {
    let clean = id.trim_start_matches('#');
    let lock = global_svg_symbols().read().ok()?;
    lock.get(clean).cloned()
}

/// Clears all globally registered SVG symbols.
pub fn clear_global_svg_symbols() {
    if let Ok(mut lock) = global_svg_symbols().write() {
        lock.clear();
    }
}

/// Extracts intrinsic dimensions (width, height) from an SVG document if specified.
pub fn get_svg_intrinsic_dimensions(svg_xml: &str) -> (Option<f32>, Option<f32>) {
    let doc: std::sync::Arc<SvgDocument> = {
        let cached = if let Ok(cache) = svg_doc_cache().read() {
            cache.get(svg_xml)
        } else {
            None
        };
        if let Some(d) = cached {
            d
        } else if let Some(parsed) = SvgDocument::parse(svg_xml) {
            let arc_doc = std::sync::Arc::new(parsed);
            if let Ok(mut cache) = svg_doc_cache().write() {
                cache.insert(svg_xml.to_string(), arc_doc.clone());
            }
            arc_doc
        } else {
            return (None, None);
        }
    };

    match (doc.width, doc.height, doc.view_box) {
        (Some(w), Some(h), _) => return (Some(w), Some(h)),
        (Some(w), None, Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
            return (Some(w), Some(w / (vb_w / vb_h)));
        }
        (None, Some(h), Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
            return (Some(h * (vb_w / vb_h)), Some(h));
        }
        (Some(w), None, _) => return (Some(w), None),
        (None, Some(h), _) => return (None, Some(h)),
        (None, None, Some((_, _, vb_w, vb_h))) => return (Some(vb_w), Some(vb_h)),
        (None, None, None) => {}
    }
    for child in &doc.children {
        if let SvgNode::Use {
            href,
            width,
            height,
            ..
        } = child
        {
            let sym_id = href.split('#').next_back().unwrap_or(href).trim();
            let sym = doc
                .symbols
                .get(sym_id)
                .cloned()
                .or_else(|| get_global_svg_symbol(sym_id));
            let sym_vb = sym.and_then(|s| s.view_box);
            match (*width, *height, sym_vb) {
                (Some(w), Some(h), _) => return (Some(w), Some(h)),
                (Some(w), None, Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
                    return (Some(w), Some(w / (vb_w / vb_h)));
                }
                (None, Some(h), Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
                    return (Some(h * (vb_w / vb_h)), Some(h));
                }
                (Some(w), None, _) => return (Some(w), None),
                (None, Some(h), _) => return (None, Some(h)),
                (None, None, Some((_, _, vb_w, vb_h))) => return (Some(vb_w), Some(vb_h)),
                _ => {}
            }
        }
    }
    (None, None)
}
