//! Web Locks API coordinator (W3C Web Locks API).
//!
//! Provides cooperative asynchronous resource locking scoped to an origin:
//! - Exclusive (`"exclusive"`) and Shared (`"shared"`) lock modes.
//! - FIFO queueing to prevent resource starvation.
//! - `steal` option to preempt existing lock holders.
//! - `ifAvailable` option to abort without waiting if the lock is held.
//! - `query()` introspection of held and pending locks.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_GRANT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Locking mode requested or held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Exclusive,
    Shared,
}

impl LockMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            LockMode::Exclusive => "exclusive",
            LockMode::Shared => "shared",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "shared" => LockMode::Shared,
            _ => LockMode::Exclusive,
        }
    }
}

/// An active grant of a lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockGrant {
    pub grant_id: u64,
    pub name: String,
    pub mode: LockMode,
    pub client_id: String,
}

/// A queued lock request waiting to be granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRequest {
    pub request_id: u64,
    pub name: String,
    pub mode: LockMode,
    pub client_id: String,
}

/// Result of calling `request_lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockResult {
    /// Granted immediately.
    Granted(LockGrant),
    /// Queued in FIFO order until resource becomes available.
    Queued(LockRequest),
    /// Failed to acquire immediately with `ifAvailable: true`.
    Unavailable,
}

/// Snapshot entry of a lock for `navigator.locks.query()`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LockSnapshotEntry {
    pub name: String,
    pub mode: String,
    #[serde(rename = "clientId")]
    pub client_id: String,
}

/// State snapshot returned by `query()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct LocksSnapshot {
    pub held: Vec<LockSnapshotEntry>,
    pub pending: Vec<LockSnapshotEntry>,
}

/// State of a single lock resource name.
#[derive(Default)]
struct ResourceLockState {
    held: Vec<LockGrant>,
    pending: VecDeque<LockRequest>,
}

/// State of all locks partitioned by origin.
#[derive(Default)]
struct OriginLocksState {
    locks: HashMap<String, ResourceLockState>,
}

/// Central coordinator for Web Locks across all tabs, workers, and documents.
#[derive(Clone, Default)]
pub struct WebLocksEngine {
    state: Arc<Mutex<HashMap<String, OriginLocksState>>>,
}

impl WebLocksEngine {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Requests a lock on `name` within `origin`.
    pub fn request(
        &self,
        origin: &str,
        name: &str,
        mode: LockMode,
        if_available: bool,
        steal: bool,
        client_id: &str,
    ) -> LockResult {
        let mut lock = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let origin_state = lock.entry(origin.to_string()).or_default();
        let res_state = origin_state.locks.entry(name.to_string()).or_default();

        // 1. Steal: Evict all current holders and grant immediately
        if steal {
            res_state.held.clear();
            let grant = LockGrant {
                grant_id: NEXT_GRANT_ID.fetch_add(1, Ordering::Relaxed),
                name: name.to_string(),
                mode,
                client_id: client_id.to_string(),
            };
            res_state.held.push(grant.clone());
            return LockResult::Granted(grant);
        }

        // 2. If no locks are currently held, grant immediately
        if res_state.held.is_empty() {
            let grant = LockGrant {
                grant_id: NEXT_GRANT_ID.fetch_add(1, Ordering::Relaxed),
                name: name.to_string(),
                mode,
                client_id: client_id.to_string(),
            };
            res_state.held.push(grant.clone());
            return LockResult::Granted(grant);
        }

        // 3. If requested mode is Shared, all current holders are Shared,
        // and NO Exclusive requests are waiting in the pending queue
        if mode == LockMode::Shared
            && res_state.held.iter().all(|g| g.mode == LockMode::Shared)
            && !res_state.pending.iter().any(|r| r.mode == LockMode::Exclusive)
        {
            let grant = LockGrant {
                grant_id: NEXT_GRANT_ID.fetch_add(1, Ordering::Relaxed),
                name: name.to_string(),
                mode: LockMode::Shared,
                client_id: client_id.to_string(),
            };
            res_state.held.push(grant.clone());
            return LockResult::Granted(grant);
        }

        // 4. Cannot acquire immediately
        if if_available {
            return LockResult::Unavailable;
        }

        // 5. Enqueue into FIFO pending queue
        let req = LockRequest {
            request_id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
            name: name.to_string(),
            mode,
            client_id: client_id.to_string(),
        };
        res_state.pending.push_back(req.clone());
        LockResult::Queued(req)
    }

    /// Releases a previously granted lock, granting newly unblocked pending requests.
    pub fn release(&self, origin: &str, name: &str, grant_id: u64) -> Vec<LockGrant> {
        let mut lock = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let mut newly_granted = Vec::new();
        let Some(origin_state) = lock.get_mut(origin) else {
            return newly_granted;
        };
        let Some(res_state) = origin_state.locks.get_mut(name) else {
            return newly_granted;
        };

        // Remove the releasing grant
        res_state.held.retain(|g| g.grant_id != grant_id);

        // Try to grant pending requests
        if res_state.held.is_empty() {
            if let Some(first) = res_state.pending.pop_front() {
                let grant = LockGrant {
                    grant_id: NEXT_GRANT_ID.fetch_add(1, Ordering::Relaxed),
                    name: first.name,
                    mode: first.mode,
                    client_id: first.client_id,
                };
                let is_shared = grant.mode == LockMode::Shared;
                res_state.held.push(grant.clone());
                newly_granted.push(grant);

                // If first granted is Shared, grant consecutive shared requests
                if is_shared {
                    while let Some(front) = res_state.pending.front() {
                        if front.mode == LockMode::Shared {
                            let next = res_state.pending.pop_front().unwrap();
                            let next_grant = LockGrant {
                                grant_id: NEXT_GRANT_ID.fetch_add(1, Ordering::Relaxed),
                                name: next.name,
                                mode: LockMode::Shared,
                                client_id: next.client_id,
                            };
                            res_state.held.push(next_grant.clone());
                            newly_granted.push(next_grant);
                        } else {
                            break;
                        }
                    }
                }
            }
        } else if res_state.held.iter().all(|g| g.mode == LockMode::Shared)
            && !res_state.pending.is_empty()
            && res_state.pending.front().unwrap().mode == LockMode::Shared
        {
            // If holders are Shared and front of queue is Shared, drain shared queue
            while let Some(front) = res_state.pending.front() {
                if front.mode == LockMode::Shared {
                    let next = res_state.pending.pop_front().unwrap();
                    let next_grant = LockGrant {
                        grant_id: NEXT_GRANT_ID.fetch_add(1, Ordering::Relaxed),
                        name: next.name,
                        mode: LockMode::Shared,
                        client_id: next.client_id,
                    };
                    res_state.held.push(next_grant.clone());
                    newly_granted.push(next_grant);
                } else {
                    break;
                }
            }
        }

        // Clean up empty map entry
        if res_state.held.is_empty() && res_state.pending.is_empty() {
            origin_state.locks.remove(name);
        }

        newly_granted
    }

    /// Aborts a pending request (e.g. on AbortSignal trigger).
    pub fn abort_pending(&self, origin: &str, name: &str, request_id: u64) -> bool {
        let mut lock = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(origin_state) = lock.get_mut(origin) {
            if let Some(res_state) = origin_state.locks.get_mut(name) {
                let prev_len = res_state.pending.len();
                res_state.pending.retain(|r| r.request_id != request_id);
                return res_state.pending.len() < prev_len;
            }
        }
        false
    }

    /// Queries the current held and pending lock state for an origin.
    pub fn query(&self, origin: &str) -> LocksSnapshot {
        let lock = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let mut snapshot = LocksSnapshot::default();
        if let Some(origin_state) = lock.get(origin) {
            for (name, res) in &origin_state.locks {
                for h in &res.held {
                    snapshot.held.push(LockSnapshotEntry {
                        name: name.clone(),
                        mode: h.mode.as_str().to_string(),
                        client_id: h.client_id.clone(),
                    });
                }
                for p in &res.pending {
                    snapshot.pending.push(LockSnapshotEntry {
                        name: name.clone(),
                        mode: p.mode.as_str().to_string(),
                        client_id: p.client_id.clone(),
                    });
                }
            }
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclusive_and_shared_locking() {
        let engine = WebLocksEngine::new();
        let origin = "https://example.com";

        // Exclusive lock 1
        let res1 = engine.request(origin, "res", LockMode::Exclusive, false, false, "client1");
        let grant1 = match res1 {
            LockResult::Granted(g) => g,
            _ => panic!("Expected grant1 to succeed"),
        };
        assert_eq!(grant1.mode, LockMode::Exclusive);

        // Exclusive lock 2 should queue
        let res2 = engine.request(origin, "res", LockMode::Exclusive, false, false, "client2");
        let req2 = match res2 {
            LockResult::Queued(r) => r,
            _ => panic!("Expected req2 to queue"),
        };
        assert_eq!(req2.client_id, "client2");

        // ifAvailable lock 3 should fail
        let res3 = engine.request(origin, "res", LockMode::Exclusive, true, false, "client3");
        assert_eq!(res3, LockResult::Unavailable);

        // Release lock 1 -> should grant lock 2
        let unblocked = engine.release(origin, "res", grant1.grant_id);
        assert_eq!(unblocked.len(), 1);
        assert_eq!(unblocked[0].client_id, "client2");

        // Release lock 2
        let empty = engine.release(origin, "res", unblocked[0].grant_id);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_shared_concurrency() {
        let engine = WebLocksEngine::new();
        let origin = "https://example.com";

        // Shared lock 1
        let res1 = engine.request(origin, "doc", LockMode::Shared, false, false, "reader1");
        assert!(matches!(res1, LockResult::Granted(_)));

        // Shared lock 2 should also be granted concurrently
        let res2 = engine.request(origin, "doc", LockMode::Shared, false, false, "reader2");
        assert!(matches!(res2, LockResult::Granted(_)));

        // Exclusive writer should queue
        let res3 = engine.request(origin, "doc", LockMode::Exclusive, false, false, "writer");
        assert!(matches!(res3, LockResult::Queued(_)));

        // Another Shared reader must queue behind writer to prevent starvation
        let res4 = engine.request(origin, "doc", LockMode::Shared, false, false, "reader3");
        assert!(matches!(res4, LockResult::Queued(_)));

        let snap = engine.query(origin);
        assert_eq!(snap.held.len(), 2);
        assert_eq!(snap.pending.len(), 2);
    }

    #[test]
    fn test_lock_steal() {
        let engine = WebLocksEngine::new();
        let origin = "https://example.com";

        let res1 = engine.request(origin, "cfg", LockMode::Exclusive, false, false, "c1");
        assert!(matches!(res1, LockResult::Granted(_)));

        // Steal lock
        let res2 = engine.request(origin, "cfg", LockMode::Exclusive, false, true, "stealer");
        assert!(matches!(res2, LockResult::Granted(_)));

        let snap = engine.query(origin);
        assert_eq!(snap.held.len(), 1);
        assert_eq!(snap.held[0].client_id, "stealer");
    }
}
