//! Thread-safe bounded memory caches and asynchronous decode workers.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use super::decoder::{create_placeholder, decode_animated_gif, decode_image_bytes};
use super::image::{AnimatedImage, DecodedImage};

/// Intrusive index-linked node for constant-time LRU doubly-linked list ordering.
struct ImageCacheNode<K> {
    key: K,
    image: DecodedImage,
    bytes: usize,
    prev: Option<usize>,
    next: Option<usize>,
}

/// Thread-safe bounded memory cache for decoded image buffers with O(1) LRU eviction.
pub(crate) struct BoundedImageStore<K: std::hash::Hash + Eq + Clone> {
    map: HashMap<K, usize>,
    nodes: Vec<Option<ImageCacheNode<K>>>,
    free_slots: Vec<usize>,
    head: Option<usize>, // least-recently-used (oldest)
    tail: Option<usize>, // most-recently-used (newest)
    pub(crate) total_bytes: usize,
    pub(crate) max_bytes: usize,
}

impl<K: std::hash::Hash + Eq + Clone> BoundedImageStore<K> {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            map: HashMap::new(),
            nodes: Vec::new(),
            free_slots: Vec::new(),
            head: None,
            tail: None,
            total_bytes: 0,
            max_bytes,
        }
    }

    /// Moves the node at `idx` to the tail (most-recently-used position) in O(1) time.
    fn touch(&mut self, idx: usize) {
        if self.tail == Some(idx) {
            return;
        }

        let (prev, next) = match self.nodes.get(idx).and_then(|n| n.as_ref()) {
            Some(node) => (node.prev, node.next),
            None => return,
        };

        // Unlink from current position
        if let Some(p) = prev {
            if let Some(Some(prev_node)) = self.nodes.get_mut(p) {
                prev_node.next = next;
            }
        } else {
            self.head = next;
        }

        if let Some(n) = next {
            if let Some(Some(next_node)) = self.nodes.get_mut(n) {
                next_node.prev = prev;
            }
        } else {
            self.tail = prev;
        }

        // Link at tail
        if let Some(old_tail) = self.tail {
            if let Some(Some(tail_node)) = self.nodes.get_mut(old_tail) {
                tail_node.next = Some(idx);
            }
        } else {
            self.head = Some(idx);
        }

        if let Some(Some(node)) = self.nodes.get_mut(idx) {
            node.prev = self.tail;
            node.next = None;
        }
        self.tail = Some(idx);
    }

    /// Evicts the least-recently-used entry from the head in O(1) time.
    fn pop_lru(&mut self) -> Option<(K, DecodedImage)> {
        let head_idx = self.head?;
        let node = self.nodes[head_idx].take()?;
        self.map.remove(&node.key);
        self.total_bytes = self.total_bytes.saturating_sub(node.bytes);
        self.head = node.next;
        if let Some(new_head) = self.head {
            if let Some(Some(head_node)) = self.nodes.get_mut(new_head) {
                head_node.prev = None;
            }
        } else {
            self.tail = None;
        }
        self.free_slots.push(head_idx);
        Some((node.key, node.image))
    }

    pub(crate) fn insert(&mut self, key: K, img: DecodedImage) {
        let entry_bytes = img.pixels.len() * 4;
        if entry_bytes > self.max_bytes {
            return;
        }

        // If key already exists, update image and touch to MRU
        if let Some(&idx) = self.map.get(&key) {
            if let Some(Some(node)) = self.nodes.get_mut(idx) {
                self.total_bytes = self.total_bytes.saturating_sub(node.bytes);
                node.image = img;
                node.bytes = entry_bytes;
                self.total_bytes += entry_bytes;
            }
            self.touch(idx);
            return;
        }

        // Evict least-recently-used entries in O(1) per eviction until under budget
        while (self.total_bytes + entry_bytes > self.max_bytes || self.map.len() >= 1024)
            && !self.map.is_empty()
        {
            if self.pop_lru().is_none() {
                break;
            }
        }

        let idx = if let Some(slot) = self.free_slots.pop() {
            slot
        } else {
            let slot = self.nodes.len();
            self.nodes.push(None);
            slot
        };

        // Link at tail
        if let Some(old_tail) = self.tail {
            if let Some(Some(tail_node)) = self.nodes.get_mut(old_tail) {
                tail_node.next = Some(idx);
            }
        } else {
            self.head = Some(idx);
        }

        let prev = self.tail;
        self.tail = Some(idx);
        self.nodes[idx] = Some(ImageCacheNode {
            key: key.clone(),
            image: img,
            bytes: entry_bytes,
            prev,
            next: None,
        });
        self.map.insert(key, idx);
        self.total_bytes += entry_bytes;
    }

    pub(crate) fn get<Q>(&mut self, key: &Q) -> Option<&DecodedImage>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq + ?Sized,
    {
        let &idx = self.map.get(key)?;
        self.touch(idx);
        self.nodes[idx].as_ref().map(|n| &n.image)
    }

    #[allow(dead_code)]
    pub(crate) fn peek<Q>(&self, key: &Q) -> Option<&DecodedImage>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq + ?Sized,
    {
        let &idx = self.map.get(key)?;
        self.nodes[idx].as_ref().map(|n| &n.image)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &DecodedImage)> {
        self.nodes
            .iter()
            .filter_map(|opt| opt.as_ref().map(|n| (&n.key, &n.image)))
    }

    #[allow(dead_code)]
    pub(crate) fn remove<Q>(&mut self, key: &Q) -> Option<DecodedImage>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq + ?Sized,
    {
        let idx = self.map.remove(key)?;
        let node = self.nodes[idx].take()?;
        self.total_bytes = self.total_bytes.saturating_sub(node.bytes);

        // Unlink from current position
        if let Some(p) = node.prev {
            if let Some(Some(prev_node)) = self.nodes.get_mut(p) {
                prev_node.next = node.next;
            }
        } else {
            self.head = node.next;
        }

        if let Some(n) = node.next {
            if let Some(Some(next_node)) = self.nodes.get_mut(n) {
                next_node.prev = node.prev;
            }
        } else {
            self.tail = node.prev;
        }

        self.free_slots.push(idx);
        Some(node.image)
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.nodes.clear();
        self.free_slots.clear();
        self.head = None;
        self.tail = None;
        self.total_bytes = 0;
    }

    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// 64MB budget for decoded images
static DECODED_IMAGE_CACHE: std::sync::OnceLock<Mutex<BoundedImageStore<String>>> =
    std::sync::OnceLock::new();
static DECODE_IN_FLIGHT: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();

// 32MB budget for scaled image variants
static SCALED_IMAGE_CACHE: std::sync::OnceLock<Mutex<BoundedImageStore<(String, u32, u32)>>> =
    std::sync::OnceLock::new();

fn global_image_cache() -> &'static Mutex<BoundedImageStore<String>> {
    DECODED_IMAGE_CACHE.get_or_init(|| Mutex::new(BoundedImageStore::new(64 * 1024 * 1024)))
}

fn global_scaled_cache() -> &'static Mutex<BoundedImageStore<(String, u32, u32)>> {
    SCALED_IMAGE_CACHE.get_or_init(|| Mutex::new(BoundedImageStore::new(32 * 1024 * 1024)))
}

fn in_flight_set() -> &'static Mutex<HashSet<String>> {
    DECODE_IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Stores a decoded image in the global image cache.
pub fn cache_image(key: &str, img: DecodedImage) {
    if let Ok(mut cache) = global_image_cache().lock() {
        cache.insert(key.to_string(), img);
    }
}

/// Retrieves a scaled image from the scaled image cache by (url, target_width, target_height).
pub fn get_scaled_image(key: &str, target_w: u32, target_h: u32) -> Option<DecodedImage> {
    let mut cache = global_scaled_cache().lock().ok()?;
    cache.get(&(key.to_string(), target_w, target_h)).cloned()
}

/// Stores a scaled image in the scaled image cache.
pub fn cache_scaled_image(key: &str, target_w: u32, target_h: u32, img: DecodedImage) {
    if let Ok(mut cache) = global_scaled_cache().lock() {
        cache.insert((key.to_string(), target_w, target_h), img);
    }
}

/// Prunes intermediate scaled image variants, decoded images, animated images, and SVG caches to conserve memory across navigations.
pub fn prune_image_caches() {
    if let Ok(mut cache) = global_scaled_cache().lock() {
        cache.clear();
    }
    if let Ok(mut cache) = global_image_cache().lock() {
        cache.clear();
    }
    if let Ok(mut cache) = global_animated_cache().lock() {
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
    if let Ok(mut in_flight) = in_flight_set().lock()
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
            if let Ok(mut in_flight) = in_flight_set().lock() {
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
    let mut cache = global_image_cache().lock().ok()?;
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

static ANIMATED_IMAGE_CACHE: std::sync::OnceLock<Mutex<HashMap<String, AnimatedImage>>> =
    std::sync::OnceLock::new();

fn global_animated_cache() -> &'static Mutex<HashMap<String, AnimatedImage>> {
    ANIMATED_IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn cache_animated_image(key: &str, anim: AnimatedImage) {
    if let Ok(mut cache) = global_animated_cache().lock() {
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
    let cache = global_animated_cache().lock().ok()?;
    cache.get(key).cloned()
}

/// Retrieves the active frame of an animated image for elapsed time `elapsed_ms`.
pub fn get_animated_frame(key: &str, elapsed_ms: u64) -> Option<DecodedImage> {
    let anim = get_cached_animated_image(key)?;
    Some(anim.frame_at_time(elapsed_ms).clone())
}
