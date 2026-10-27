//! Thread-safe bounded memory caches and asynchronous decode workers.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use super::decoder::{create_placeholder, decode_animated_gif, decode_image_bytes};
use super::image::{AnimatedImage, DecodedImage};

/// Thread-safe bounded memory cache for decoded image buffers.
struct ImageCacheEntry {
    image: DecodedImage,
    last_accessed: std::sync::atomic::AtomicU64,
}

static IMAGE_ACCESS_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Thread-safe bounded memory cache for decoded image buffers with LRU eviction.
pub(crate) struct BoundedImageStore<K: std::hash::Hash + Eq + Clone> {
    entries: HashMap<K, ImageCacheEntry>,
    pub(crate) total_bytes: usize,
    pub(crate) max_bytes: usize,
}

impl<K: std::hash::Hash + Eq + Clone> BoundedImageStore<K> {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            total_bytes: 0,
            max_bytes,
        }
    }

    pub(crate) fn insert(&mut self, key: K, img: DecodedImage) {
        let entry_bytes = img.pixels.len() * 4;
        if entry_bytes > self.max_bytes {
            return;
        }

        if let Some(old) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(old.image.pixels.len() * 4);
        }

        while (self.total_bytes + entry_bytes > self.max_bytes || self.entries.len() >= 1024)
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
        let tick = IMAGE_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.entries.insert(
            key,
            ImageCacheEntry {
                image: img,
                last_accessed: std::sync::atomic::AtomicU64::new(tick),
            },
        );
    }

    pub(crate) fn get<Q>(&self, key: &Q) -> Option<&DecodedImage>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq + ?Sized,
    {
        let entry = self.entries.get(key)?;
        entry.last_accessed.store(
            IMAGE_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        Some(&entry.image)
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &DecodedImage)> {
        self.entries.iter().map(|(k, entry)| (k, &entry.image))
    }

    #[allow(dead_code)]
    fn remove(&mut self, key: &K) -> Option<DecodedImage> {
        if let Some(removed) = self.entries.remove(key) {
            self.total_bytes = self
                .total_bytes
                .saturating_sub(removed.image.pixels.len() * 4);
            Some(removed.image)
        } else {
            None
        }
    }
}

// 64MB budget for decoded images
static DECODED_IMAGE_CACHE: std::sync::OnceLock<RwLock<BoundedImageStore<String>>> =
    std::sync::OnceLock::new();
static DECODE_IN_FLIGHT: std::sync::OnceLock<RwLock<HashSet<String>>> = std::sync::OnceLock::new();

// 32MB budget for scaled image variants
static SCALED_IMAGE_CACHE: std::sync::OnceLock<RwLock<BoundedImageStore<(String, u32, u32)>>> =
    std::sync::OnceLock::new();

fn global_image_cache() -> &'static RwLock<BoundedImageStore<String>> {
    DECODED_IMAGE_CACHE.get_or_init(|| RwLock::new(BoundedImageStore::new(64 * 1024 * 1024)))
}

fn global_scaled_cache() -> &'static RwLock<BoundedImageStore<(String, u32, u32)>> {
    SCALED_IMAGE_CACHE.get_or_init(|| RwLock::new(BoundedImageStore::new(32 * 1024 * 1024)))
}

fn in_flight_set() -> &'static RwLock<HashSet<String>> {
    DECODE_IN_FLIGHT.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Stores a decoded image in the global image cache.
pub fn cache_image(key: &str, img: DecodedImage) {
    if let Ok(mut cache) = global_image_cache().write() {
        cache.insert(key.to_string(), img);
    }
}

/// Retrieves a scaled image from the scaled image cache by (url, target_width, target_height).
pub fn get_scaled_image(key: &str, target_w: u32, target_h: u32) -> Option<DecodedImage> {
    let cache = global_scaled_cache().read().ok()?;
    cache.get(&(key.to_string(), target_w, target_h)).cloned()
}

/// Stores a scaled image in the scaled image cache.
pub fn cache_scaled_image(key: &str, target_w: u32, target_h: u32, img: DecodedImage) {
    if let Ok(mut cache) = global_scaled_cache().write() {
        cache.insert((key.to_string(), target_w, target_h), img);
    }
}

/// Prunes intermediate scaled image variants, decoded images, animated images, and SVG caches to conserve memory across navigations.
pub fn prune_image_caches() {
    if let Ok(mut cache) = global_scaled_cache().write() {
        cache.entries.clear();
        cache.total_bytes = 0;
    }
    if let Ok(mut cache) = global_image_cache().write() {
        cache.entries.clear();
        cache.total_bytes = 0;
    }
    if let Ok(mut cache) = global_animated_cache().write() {
        cache.clear();
    }
    crate::svg::prune_svg_caches();
}

/// Gets an existing scaled image or scales the provided image and caches the result.
pub fn get_or_resize_cached(
    key: &str,
    img: &DecodedImage,
    target_w: u32,
    target_h: u32,
) -> DecodedImage {
    if let Some(scaled) = get_scaled_image(key, target_w, target_h) {
        return scaled;
    }
    let scaled = img.resize(target_w, target_h);
    cache_scaled_image(key, target_w, target_h, scaled.clone());
    scaled
}

/// Triggers asynchronous decoding of image bytes in a background worker thread.
/// Once decoded, the image is automatically inserted into the global image cache.
pub fn decode_image_async(key: String, bytes: Vec<u8>) {
    if get_cached_image(&key).is_some() {
        return;
    }
    if let Ok(mut in_flight) = in_flight_set().write()
        && !in_flight.insert(key.clone())
    {
        return; // Decode already in progress
    }

    std::thread::Builder::new()
        .name("mango-img-decode".to_string())
        .spawn(move || {
            if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
                if let Some(anim) = decode_animated_gif(&bytes) {
                    if anim.frames.len() > 1 {
                        cache_animated_image(&key, anim.clone());
                    }
                    if let Some(first) = anim.frames.first() {
                        cache_image(&key, first.image.clone());
                    }
                } else if let Some(decoded) = decode_image_bytes(&bytes) {
                    cache_image(&key, decoded);
                }
            } else if let Some(decoded) = decode_image_bytes(&bytes) {
                cache_image(&key, decoded);
            }
            if let Ok(mut in_flight) = in_flight_set().write() {
                in_flight.remove(&key);
            }
        })
        .ok();
}

/// Retrieves a decoded image from cache, or initiates background decoding
/// while returning a placeholder image immediately to avoid blocking the main event loop.
pub fn get_or_decode_async(
    key: &str,
    bytes: &[u8],
    placeholder_w: u32,
    placeholder_h: u32,
) -> DecodedImage {
    if let Some(img) = get_cached_image(key) {
        return img;
    }
    decode_image_async(key.to_string(), bytes.to_vec());
    create_placeholder(placeholder_w, placeholder_h)
}

/// Retrieves a decoded image from the global image cache by key/URL.
/// Supports exact match, protocol-relative (`//`), query/fragment stripping, and path suffixes.
pub fn get_cached_image(key: &str) -> Option<DecodedImage> {
    let cache = global_image_cache().read().ok()?;
    if let Some(img) = cache.get(key) {
        return Some(img.clone());
    }
    if key.starts_with("//") {
        let https_key = format!("https:{key}");
        if let Some(img) = cache.get(&https_key) {
            return Some(img.clone());
        }
    }
    let clean_key = key
        .split('?')
        .next()
        .unwrap_or(key)
        .split('#')
        .next()
        .unwrap_or(key);
    if let Some(img) = cache.get(clean_key) {
        return Some(img.clone());
    }
    if key.starts_with('/') {
        for (k, img) in cache.iter() {
            if k.ends_with(key) {
                return Some(img.clone());
            }
        }
    }
    None
}

static ANIMATED_IMAGE_CACHE: std::sync::OnceLock<RwLock<HashMap<String, AnimatedImage>>> =
    std::sync::OnceLock::new();

fn global_animated_cache() -> &'static RwLock<HashMap<String, AnimatedImage>> {
    ANIMATED_IMAGE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn cache_animated_image(key: &str, anim: AnimatedImage) {
    if let Ok(mut cache) = global_animated_cache().write() {
        if cache.len() >= 8 {
            let keys_to_remove: Vec<_> = cache.keys().take(4).cloned().collect();
            for k in keys_to_remove {
                cache.remove(&k);
            }
        }
        cache.insert(key.to_string(), anim);
    }
}

/// Retrieves an animated image from the cache.
pub fn get_cached_animated_image(key: &str) -> Option<AnimatedImage> {
    let cache = global_animated_cache().read().ok()?;
    cache.get(key).cloned()
}

/// Retrieves the active frame of an animated image for elapsed time `elapsed_ms`.
pub fn get_animated_frame(key: &str, elapsed_ms: u64) -> Option<DecodedImage> {
    let anim = get_cached_animated_image(key)?;
    Some(anim.frame_at_time(elapsed_ms).clone())
}
