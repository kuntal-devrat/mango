//! DNS resolution and thread-safe resolution caching.

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Default time-to-live for DNS cache entries (5 minutes).
const DEFAULT_DNS_TTL: Duration = Duration::from_secs(300);

/// Maximum number of DNS cache entries.
const MAX_DNS_CACHE_ENTRIES: usize = 512;

/// Default timeout for a single DNS lookup (prevents UI hangs on dead DNS).
pub const DNS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

struct InflightSignal {
    sync_notify: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    async_notify: std::sync::Arc<tokio::sync::Notify>,
}

impl InflightSignal {
    fn new() -> Self {
        Self {
            sync_notify: std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new())),
            async_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
        }
    }

    fn notify_all(&self) {
        if let Ok(mut done) = self.sync_notify.0.lock() {
            *done = true;
            self.sync_notify.1.notify_all();
        }
        self.async_notify.notify_waiters();
    }
}

/// A thread-safe in-memory cache for DNS lookups.
pub struct DnsCache {
    entries: RwLock<HashMap<String, (Vec<SocketAddr>, Instant)>>,
    ttl: Duration,
    /// Hosts with a lookup currently in flight (singleflight dedup with event notifications).
    inflight: std::sync::Mutex<HashMap<String, InflightSignal>>,
}

impl Default for DnsCache {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsCache {
    /// Creates a new DNS cache with default 5-minute TTL.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl: DEFAULT_DNS_TTL,
            inflight: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Checks the cache for fresh resolution results.
    fn get_cached(&self, cache_key: &str) -> Option<Vec<SocketAddr>> {
        if let Ok(guard) = self.entries.read()
            && let Some((addrs, timestamp)) = guard.get(cache_key)
            && timestamp.elapsed() < self.ttl
        {
            Some(addrs.clone())
        } else {
            None
        }
    }

    /// Stores fresh resolution results in the cache with bounded eviction.
    fn store_cached(&self, cache_key: String, addrs: Vec<SocketAddr>) {
        if let Ok(mut guard) = self.entries.write() {
            if guard.len() >= MAX_DNS_CACHE_ENTRIES {
                let ttl = self.ttl;
                guard.retain(|_, (_, ts)| ts.elapsed() < ttl);
            }
            if guard.len() >= MAX_DNS_CACHE_ENTRIES {
                // Batch-evict the oldest 10% of entries to avoid O(n) scan on every insert (M-06)
                let to_evict = (MAX_DNS_CACHE_ENTRIES / 10).max(1);
                let mut entries_by_age: Vec<(String, Instant)> = guard
                    .iter()
                    .map(|(k, (_, ts))| (k.clone(), *ts))
                    .collect();
                entries_by_age.sort_by_key(|(_, ts)| *ts);
                for (k, _) in entries_by_age.into_iter().take(to_evict) {
                    guard.remove(&k);
                }
            }
            guard.insert(cache_key, (addrs, Instant::now()));
        }
    }

    /// Resolves a hostname and port into socket addresses synchronously, utilizing cached entries if fresh.
    pub fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
        let cache_key = format!("{host}:{port}");

        if let Some(addrs) = self.get_cached(&cache_key) {
            return Ok(addrs);
        }

        // Perform system DNS resolution
        let host_port = format!("{host}:{port}");
        let addrs: Vec<SocketAddr> = host_port
            .to_socket_addrs()
            .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?
            .collect();

        if addrs.is_empty() {
            return Err(format!("No IP addresses found for host '{host}'"));
        }

        self.store_cached(cache_key, addrs.clone());
        Ok(addrs)
    }

    /// Asynchronously resolves a hostname and port without blocking the executor thread (OPT-3.3.2).
    pub async fn resolve_async(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
        let cache_key = format!("{host}:{port}");

        if let Some(addrs) = self.get_cached(&cache_key) {
            return Ok(addrs);
        }

        let host_port = cache_key.clone();
        let host_name = host.to_string();
        let addrs: Vec<SocketAddr> = tokio::task::spawn_blocking(move || {
            host_port
                .to_socket_addrs()
                .map(|iter| iter.collect::<Vec<_>>())
                .map_err(|e| format!("DNS resolution failed for '{host_name}': {e}"))
        })
        .await
        .map_err(|e| format!("DNS resolution thread task panicked: {e}"))??;

        if addrs.is_empty() {
            return Err(format!("No IP addresses found for host '{host}'"));
        }

        self.store_cached(cache_key, addrs.clone());
        Ok(addrs)
    }

    /// Resolves a hostname with a timeout limit to prevent indefinite hangs.
    pub fn resolve_timeout(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<SocketAddr>, String> {
        let cache_key = format!("{host}:{port}");

        if let Some(addrs) = self.get_cached(&cache_key) {
            return Ok(addrs);
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let host_port = cache_key.clone();
        let host_name = host.to_string();

        std::thread::Builder::new()
            .name("mango-dns-lookup".to_string())
            .spawn(move || {
                let res = host_port
                    .to_socket_addrs()
                    .map(|iter| iter.collect::<Vec<_>>())
                    .map_err(|e| format!("DNS resolution failed for '{host_name}': {e}"));
                let _ = tx.send(res);
            })
            .map_err(|e| format!("Failed to spawn DNS thread: {e}"))?;

        let addrs = match rx.recv_timeout(timeout) {
            Ok(Ok(addrs)) => addrs,
            Ok(Err(e)) => return Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return Err(format!("DNS resolution timed out after {:?}", timeout));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("DNS resolution worker disconnected".to_string());
            }
        };

        if addrs.is_empty() {
            return Err(format!("No IP addresses found for host '{host}'"));
        }

        self.store_cached(cache_key, addrs.clone());
        Ok(addrs)
    }

    /// Clears all entries from the DNS cache.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.entries.write() {
            guard.clear();
        }
    }

    /// Synchronous singleflight resolve: concurrent callers for the same
    /// `host:port` wait for the in-flight lookup instead of issuing duplicate
    /// system DNS queries. Bounded by `timeout`.
    pub fn resolve_singleflight(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<SocketAddr>, String> {
        let cache_key = format!("{host}:{port}");
        if let Some(addrs) = self.get_cached(&cache_key) {
            return Ok(addrs);
        }

        let signal_opt = {
            let mut inflight = self.inflight.lock().map_err(|e| e.to_string())?;
            if let Some(existing) = inflight.get(&cache_key) {
                Some(existing.sync_notify.clone())
            } else {
                inflight.insert(cache_key.clone(), InflightSignal::new());
                None
            }
        };

        if let Some(sync_notify) = signal_opt {
            let (lock, cvar) = &*sync_notify;
            let mut done = lock.lock().map_err(|e| e.to_string())?;
            let deadline = Instant::now() + timeout;
            while !*done {
                let now = Instant::now();
                if now >= deadline {
                    return Err(format!(
                        "DNS singleflight wait timed out after {:?} for '{host}'",
                        timeout
                    ));
                }
                let remaining = deadline - now;
                let (new_guard, timeout_result) =
                    cvar.wait_timeout(done, remaining).map_err(|e| e.to_string())?;
                done = new_guard;
                if timeout_result.timed_out() && !*done {
                    return Err(format!(
                        "DNS singleflight wait timed out after {:?} for '{host}'",
                        timeout
                    ));
                }
            }
            return self.get_cached(&cache_key).ok_or_else(|| {
                format!("DNS resolution failed for '{host}' in primary resolver")
            });
        }

        let res = self.resolve_timeout(host, port, timeout);
        let signal = {
            let inflight = self.inflight.lock().ok();
            inflight.and_then(|mut s| s.remove(&cache_key))
        };
        if let Some(sig) = signal {
            sig.notify_all();
        }
        res
    }

    /// Async singleflight resolve with timeout. Never blocks the executor:
    /// the blocking lookup runs via [`Self::resolve_async`] and concurrent
    /// callers for the same host coalesce onto the cached result.
    pub async fn resolve_async_timeout(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<SocketAddr>, String> {
        let cache_key = format!("{host}:{port}");
        if let Some(addrs) = self.get_cached(&cache_key) {
            return Ok(addrs);
        }

        let signal_opt = {
            let mut inflight = self.inflight.lock().map_err(|e| e.to_string())?;
            if let Some(existing) = inflight.get(&cache_key) {
                Some(existing.async_notify.clone())
            } else {
                inflight.insert(cache_key.clone(), InflightSignal::new());
                None
            }
        };

        if let Some(async_notify) = signal_opt {
            let wait_fut = async_notify.notified();
            tokio::time::timeout(timeout, wait_fut)
                .await
                .map_err(|_| {
                    format!(
                        "DNS singleflight wait timed out after {:?} for '{host}'",
                        timeout
                    )
                })?;
            return self.get_cached(&cache_key).ok_or_else(|| {
                format!("DNS resolution failed for '{host}' in primary resolver")
            });
        }

        let host_owned = host.to_string();
        let fut = self.resolve_async(&host_owned, port);
        let res = tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| format!("DNS resolution timed out after {:?} for '{host}'", timeout))?
            .map_err(|e| e.to_string());

        let signal = {
            let inflight = self.inflight.lock().ok();
            inflight.and_then(|mut s| s.remove(&cache_key))
        };
        if let Some(sig) = signal {
            sig.notify_all();
        }
        res
    }

    /// Async resolve with the default lookup timeout plus singleflight.
    pub async fn resolve_async_singleflight(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, String> {
        self.resolve_async_timeout(host, port, DNS_LOOKUP_TIMEOUT)
            .await
    }
}

/// Global DNS cache singleton.
static GLOBAL_DNS_CACHE: std::sync::OnceLock<DnsCache> = std::sync::OnceLock::new();

/// Returns a reference to the global DNS cache.
pub fn global_dns_cache() -> &'static DnsCache {
    GLOBAL_DNS_CACHE.get_or_init(DnsCache::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_localhost() {
        let cache = DnsCache::new();
        let addrs = cache.resolve("localhost", 80).unwrap();
        assert!(
            !addrs.is_empty(),
            "localhost should resolve to at least one address"
        );
        // Verify cache hit
        let cached = cache.resolve("localhost", 80).unwrap();
        assert_eq!(addrs, cached);
    }

    #[tokio::test]
    async fn test_resolve_async_localhost() {
        let cache = DnsCache::new();
        let addrs = cache.resolve_async("localhost", 80).await.unwrap();
        assert!(
            !addrs.is_empty(),
            "async localhost should resolve to at least one address"
        );
        let cached = cache.resolve_async("localhost", 80).await.unwrap();
        assert_eq!(addrs, cached);
    }

    #[test]
    fn test_resolve_timeout() {
        let cache = DnsCache::new();
        let addrs = cache
            .resolve_timeout("localhost", 80, Duration::from_secs(2))
            .unwrap();
        assert!(!addrs.is_empty());
    }
}
