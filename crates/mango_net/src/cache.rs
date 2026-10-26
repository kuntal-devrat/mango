//! In-memory resource cache for HTTP responses (HTML, CSS, images).
//!
//! Provides a 20MB byte-limited LRU memory cache with conditional ETag /
//! If-Modified-Since revalidation support.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Default time-to-live for cached web resources (10 minutes).
const DEFAULT_RESOURCE_TTL: Duration = Duration::from_secs(600);
/// Maximum number of cached items before pruning old entries.
const MAX_CACHE_ENTRIES: usize = 1000;
/// Default maximum cache size in bytes (20 MB).
pub const DEFAULT_MAX_CACHE_BYTES: usize = 20 * 1024 * 1024;

static CACHE_ACCESS_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A cached HTTP resource.
#[derive(Debug, Clone)]
pub struct CachedResource {
    pub url: String,
    pub content_type: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub inserted_at: Instant,
    pub last_accessed: Instant,
    pub ttl: Duration,
    pub access_seq: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl CachedResource {
    /// Returns `true` if this cache entry has expired.
    pub fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }

    /// Returns `true` if this cache entry requires origin server revalidation before use
    /// (e.g. `Cache-Control: no-cache` or `must-revalidate` when expired) per RFC 9111.
    pub fn requires_revalidation(&self) -> bool {
        if let Some(cc) = self.headers.get("cache-control") {
            let cc_lower = cc.to_ascii_lowercase();
            if cc_lower.contains("no-cache") {
                return true;
            }
            if cc_lower.contains("must-revalidate") && self.is_expired() {
                return true;
            }
        }
        self.is_expired()
    }

    /// Approximate memory consumption in bytes for this cache entry.
    pub fn byte_size(&self) -> usize {
        self.body.len() + self.url.len() + self.content_type.len() + 256
    }

    /// Returns the ETag header value if present.
    pub fn etag(&self) -> Option<&str> {
        self.headers.get("etag").map(|s| s.as_str())
    }

    /// Returns the Last-Modified header value if present.
    pub fn last_modified(&self) -> Option<&str> {
        self.headers.get("last-modified").map(|s| s.as_str())
    }
}

/// Normalizes cache lookup keys by removing any URL fragment.
#[inline]
pub fn normalize_cache_key(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

struct CacheInner {
    entries: HashMap<String, CachedResource>,
    total_bytes: usize,
}

/// A thread-safe in-memory cache for network resources with byte-bounded LRU eviction.
pub struct ResourceCache {
    inner: RwLock<CacheInner>,
    default_ttl: Duration,
    max_entries: usize,
    max_bytes: usize,
}

impl Default for ResourceCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceCache {
    /// Creates a new resource cache with standard 20MB and 1000-entry defaults.
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_CACHE_BYTES, MAX_CACHE_ENTRIES)
    }

    /// Creates a new resource cache with explicit byte and entry capacity limits.
    pub fn with_limits(max_bytes: usize, max_entries: usize) -> Self {
        Self {
            inner: RwLock::new(CacheInner {
                entries: HashMap::new(),
                total_bytes: 0,
            }),
            default_ttl: DEFAULT_RESOURCE_TTL,
            max_entries,
            max_bytes,
        }
    }

    /// Looks up a resource in the cache by URL (excluding URL fragments).
    ///
    /// Returns `Some(resource)` if present, not expired, and not requiring revalidation.
    /// Fast read-lock path avoids write contention on cache misses and expired checks.
    /// Updates the atomic LRU access sequence on hits under the shared read lock.
    pub fn get(&self, url: &str) -> Option<CachedResource> {
        let key = normalize_cache_key(url);
        let inner = self.inner.read().ok()?;
        let res = inner.entries.get(key)?;
        if res.is_expired() || res.requires_revalidation() {
            return None;
        }
        res.access_seq.store(
            CACHE_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        Some(res.clone())
    }

    /// Returns HTTP conditional headers (`If-None-Match` / `If-Modified-Since`)
    /// if a cached copy (even if expired) exists for `url`.
    pub fn get_revalidation_headers(&self, url: &str) -> Option<Vec<(String, String)>> {
        let key = normalize_cache_key(url);
        let inner = self.inner.read().ok()?;
        let res = inner.entries.get(key)?;
        let mut headers = Vec::new();
        if let Some(etag) = res.etag() {
            headers.push(("If-None-Match".to_string(), etag.to_string()));
        }
        if let Some(lm) = res.last_modified() {
            headers.push(("If-Modified-Since".to_string(), lm.to_string()));
        }
        if headers.is_empty() {
            None
        } else {
            Some(headers)
        }
    }

    /// Refreshes a cached resource upon receiving an HTTP 304 Not Modified response.
    pub fn revalidate_304(
        &self,
        url: &str,
        new_headers: &HashMap<String, String>,
    ) -> Option<CachedResource> {
        let key = normalize_cache_key(url);
        let mut inner = self.inner.write().ok()?;
        let res = inner.entries.get_mut(key)?;

        for (k, v) in new_headers {
            res.headers.insert(k.clone(), v.clone());
        }
        res.inserted_at = Instant::now();
        res.last_accessed = Instant::now();
        res.access_seq.store(
            CACHE_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );

        if let Some(cc) = new_headers.get("cache-control") {
            let cc_lower = cc.to_ascii_lowercase();
            if let Some(pos) = cc_lower.find("max-age=") {
                let val_str = cc_lower[pos + 8..].split(',').next().unwrap_or("").trim();
                if let Ok(secs) = val_str.parse::<u64>() {
                    res.ttl = Duration::from_secs(secs);
                }
            }
        }

        Some(res.clone())
    }

    /// Inserts or updates a cached resource, honoring `Cache-Control` directives and memory limits.
    pub fn insert(
        &self,
        url: &str,
        content_type: &str,
        status: u16,
        headers: HashMap<String, String>,
        body: Vec<u8>,
        custom_ttl: Option<Duration>,
    ) {
        // Honor Cache-Control: no-store
        if let Some(cc) = headers.get("cache-control") {
            let cc_lower = cc.to_ascii_lowercase();
            if cc_lower.contains("no-store") {
                return;
            }
        }

        let has_explicit_max_age = headers
            .get("cache-control")
            .map_or(false, |cc| cc.to_ascii_lowercase().contains("max-age="));

        // RFC 9111: Do not cache 4xx/5xx responses unless explicit Cache-Control max-age is present
        if status >= 400 && !has_explicit_max_age && custom_ttl.is_none() {
            return;
        }

        let ttl = if let Some(custom) = custom_ttl {
            custom
        } else if let Some(cc) = headers.get("cache-control") {
            let cc_lower = cc.to_ascii_lowercase();
            if let Some(pos) = cc_lower.find("max-age=") {
                let val_str = cc_lower[pos + 8..].split(',').next().unwrap_or("").trim();
                if let Ok(secs) = val_str.parse::<u64>() {
                    Duration::from_secs(secs)
                } else {
                    self.default_ttl
                }
            } else {
                self.default_ttl
            }
        } else {
            self.default_ttl
        };

        let key = normalize_cache_key(url).to_string();
        let now = Instant::now();
        let resource = CachedResource {
            url: key.clone(),
            content_type: content_type.to_string(),
            status,
            headers,
            body,
            inserted_at: now,
            last_accessed: now,
            ttl,
            access_seq: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
                CACHE_ACCESS_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            )),
        };

        let new_bytes = resource.byte_size();
        if new_bytes > self.max_bytes {
            // Oversized single item, do not cache to protect budget
            return;
        }

        if let Ok(mut inner) = self.inner.write() {
            // Remove existing version if present
            if let Some(old) = inner.entries.remove(&key) {
                inner.total_bytes = inner.total_bytes.saturating_sub(old.byte_size());
            }

            // 1. Evict expired entries first
            if inner.total_bytes + new_bytes > self.max_bytes
                || inner.entries.len() >= self.max_entries
            {
                let mut expired_keys = Vec::new();
                for (k, v) in inner.entries.iter() {
                    if v.is_expired() {
                        expired_keys.push((k.clone(), v.byte_size()));
                    }
                }
                for (k, sz) in expired_keys {
                    inner.entries.remove(&k);
                    inner.total_bytes = inner.total_bytes.saturating_sub(sz);
                }
            }

            // 2. If still exceeding budget, evict least recently used entries
            while (inner.total_bytes + new_bytes > self.max_bytes
                || inner.entries.len() >= self.max_entries)
                && !inner.entries.is_empty()
            {
                let oldest_key = inner
                    .entries
                    .iter()
                    .min_by_key(|(_, v)| v.access_seq.load(std::sync::atomic::Ordering::Relaxed))
                    .map(|(k, _)| k.clone());
                if let Some(k) = oldest_key {
                    if let Some(removed) = inner.entries.remove(&k) {
                        inner.total_bytes = inner.total_bytes.saturating_sub(removed.byte_size());
                    }
                } else {
                    break;
                }
            }

            inner.total_bytes += new_bytes;
            inner.entries.insert(key, resource);
        }
    }

    /// Returns the number of items in the cache.
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.entries.len()).unwrap_or(0)
    }

    /// Returns the total bytes consumed by all cached items.
    pub fn total_bytes(&self) -> usize {
        self.inner.read().map(|g| g.total_bytes).unwrap_or(0)
    }

    /// Returns the maximum allowed bytes for this cache.
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all entries from the cache.
    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.entries.clear();
            inner.total_bytes = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_insert_and_get() {
        let cache = ResourceCache::new();
        let url = "https://example.com/style.css";
        let body = b"body { color: red; }".to_vec();

        cache.insert(url, "text/css", 200, HashMap::new(), body.clone(), None);

        let cached = cache.get(url).expect("Should find cached item");
        assert_eq!(cached.status, 200);
        assert_eq!(cached.content_type, "text/css");
        assert_eq!(cached.body, body);
        assert!(cache.total_bytes() > body.len());
    }

    #[test]
    fn test_cache_expiration() {
        let cache = ResourceCache::new();
        let url = "https://example.com/fast-expire";

        // Insert with 1 millisecond TTL
        cache.insert(
            url,
            "text/plain",
            200,
            HashMap::new(),
            b"hello".to_vec(),
            Some(Duration::from_millis(1)),
        );

        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get(url).is_none(), "Expired item should return None");
    }

    #[test]
    fn test_byte_bounded_lru_eviction() {
        // Create cache capped at 1500 bytes (each 400-byte body entry is ~680 bytes with metadata)
        let cache = ResourceCache::with_limits(1500, 10);
        let body1 = vec![b'A'; 400];
        let body2 = vec![b'B'; 400];
        let body3 = vec![b'C'; 400];

        cache.insert(
            "http://a.com",
            "text/plain",
            200,
            HashMap::new(),
            body1,
            None,
        );
        cache.insert(
            "http://b.com",
            "text/plain",
            200,
            HashMap::new(),
            body2,
            None,
        );

        // Access a to make b least recently used
        assert!(cache.get("http://a.com").is_some());

        // Inserting c should evict b because a was accessed more recently
        cache.insert(
            "http://c.com",
            "text/plain",
            200,
            HashMap::new(),
            body3,
            None,
        );

        assert!(cache.get("http://c.com").is_some());
        assert!(
            cache.get("http://b.com").is_none(),
            "b should have been LRU evicted"
        );
    }

    #[test]
    fn test_revalidation_headers_and_304() {
        let cache = ResourceCache::new();
        let url = "https://example.com/script.js";
        let mut headers = HashMap::new();
        headers.insert("etag".to_string(), "\"abc123\"".to_string());
        headers.insert(
            "last-modified".to_string(),
            "Wed, 21 Oct 2025 07:28:00 GMT".to_string(),
        );

        cache.insert(
            url,
            "application/javascript",
            200,
            headers,
            b"console.log('hi');".to_vec(),
            None,
        );

        let reval = cache
            .get_revalidation_headers(url)
            .expect("Should have revalidation headers");
        assert!(
            reval
                .iter()
                .any(|(k, v)| k == "If-None-Match" && v == "\"abc123\"")
        );
        assert!(
            reval
                .iter()
                .any(|(k, v)| k == "If-Modified-Since" && v == "Wed, 21 Oct 2025 07:28:00 GMT")
        );

        let mut new_headers = HashMap::new();
        new_headers.insert("etag".to_string(), "\"abc123\"".to_string());
        new_headers.insert("cache-control".to_string(), "max-age=3600".to_string());

        let refreshed = cache
            .revalidate_304(url, &new_headers)
            .expect("Should revalidate 304");
        assert_eq!(refreshed.ttl, Duration::from_secs(3600));
    }

    #[test]
    fn test_no_cache_4xx_5xx_without_explicit_max_age() {
        let cache = ResourceCache::new();
        let url_404 = "https://example.com/notfound.html";
        cache.insert(
            url_404,
            "text/html",
            404,
            HashMap::new(),
            b"404 Not Found".to_vec(),
            None,
        );
        assert!(
            cache.get(url_404).is_none(),
            "404 without max-age must not be cached"
        );

        let url_500 = "https://example.com/error.html";
        cache.insert(
            url_500,
            "text/html",
            500,
            HashMap::new(),
            b"500 Internal Error".to_vec(),
            None,
        );
        assert!(
            cache.get(url_500).is_none(),
            "500 without max-age must not be cached"
        );

        // With explicit max-age, error can be cached per RFC 9111
        let mut cc_headers = HashMap::new();
        cc_headers.insert("cache-control".to_string(), "max-age=60".to_string());
        cache.insert(
            url_404,
            "text/html",
            404,
            cc_headers,
            b"404 Not Found".to_vec(),
            None,
        );
        assert!(
            cache.get(url_404).is_some(),
            "404 with explicit max-age may be cached"
        );
    }

    #[test]
    fn test_cache_control_no_cache_requires_revalidation() {
        let cache = ResourceCache::new();
        let url = "https://example.com/dynamic.json";
        let mut headers = HashMap::new();
        headers.insert("cache-control".to_string(), "no-cache".to_string());
        headers.insert("etag".to_string(), "\"v1\"".to_string());
        cache.insert(
            url,
            "application/json",
            200,
            headers,
            b"{\"fresh\": true}".to_vec(),
            None,
        );

        // Even though stored, get() returns None because it requires revalidation
        assert!(cache.get(url).is_none());

        // Revalidation headers are present for the network request
        let reval = cache.get_revalidation_headers(url).expect("etag present");
        assert_eq!(
            reval,
            vec![("If-None-Match".to_string(), "\"v1\"".to_string())]
        );

        // When server replies 304, revalidate_304 refreshes and returns resource
        let revalidated = cache.revalidate_304(url, &HashMap::new());
        assert!(revalidated.is_some());
    }
}
