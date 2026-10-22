//! Cross-context Broadcast Channel API (WHATWG HTML § 9.4).
//!
//! Provides asynchronous 1-to-many communication between browsing contexts
//! (windows, tabs, iframes, workers) sharing the same origin. Senders never receive
//! their own messages.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

static NEXT_CHANNEL_ID: AtomicU64 = AtomicU64::new(1);

/// A subscriber endpoint connected to a broadcast channel.
#[derive(Clone)]
pub struct BroadcastSubscriber {
    pub id: u64,
    /// Optional thread-safe event loop task queue for injecting JS callbacks.
    pub task_queue: Option<Arc<Mutex<Vec<String>>>>,
    /// Optional direct mpsc sender for rust-native or worker listeners.
    pub direct_tx: Option<Sender<String>>,
}

/// Channel registry key identified strictly by origin and channel name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelKey {
    pub origin: String,
    pub name: String,
}

impl ChannelKey {
    pub fn new(origin: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            name: name.into(),
        }
    }
}

/// Inner state of all active broadcast channels.
#[derive(Default)]
struct HubState {
    channels: HashMap<ChannelKey, HashMap<u64, BroadcastSubscriber>>,
}

/// Thread-safe central broker for all BroadcastChannel instances.
#[derive(Clone, Default)]
pub struct BroadcastChannelHub {
    state: Arc<Mutex<HubState>>,
}

impl BroadcastChannelHub {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HubState::default())),
        }
    }

    /// Allocates a unique ID for a new BroadcastChannel instance.
    pub fn next_id(&self) -> u64 {
        NEXT_CHANNEL_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Subscribes a channel endpoint with the given origin and name.
    pub fn subscribe(
        &self,
        origin: &str,
        name: &str,
        channel_id: u64,
        task_queue: Option<Arc<Mutex<Vec<String>>>>,
        direct_tx: Option<Sender<String>>,
    ) {
        let key = ChannelKey::new(origin, name);
        let sub = BroadcastSubscriber {
            id: channel_id,
            task_queue,
            direct_tx,
        };

        if let Ok(mut lock) = self.state.lock() {
            lock.channels
                .entry(key)
                .or_default()
                .insert(channel_id, sub);
        }
    }

    /// Unsubscribes and closes a channel endpoint.
    pub fn unsubscribe(&self, origin: &str, name: &str, channel_id: u64) {
        let key = ChannelKey::new(origin, name);
        if let Ok(mut lock) = self.state.lock() {
            if let Some(map) = lock.channels.get_mut(&key) {
                map.remove(&channel_id);
                if map.is_empty() {
                    lock.channels.remove(&key);
                }
            }
        }
    }

    /// Broadcasts a JSON message payload to all subscribers on the same (origin, name),
    /// strictly excluding the sender. Returns the number of recipients delivered to.
    pub fn broadcast(&self, origin: &str, name: &str, sender_id: u64, message_json: &str) -> usize {
        let key = ChannelKey::new(origin, name);
        let subscribers: Vec<BroadcastSubscriber> = if let Ok(lock) = self.state.lock() {
            if let Some(map) = lock.channels.get(&key) {
                map.values()
                    .filter(|s| s.id != sender_id)
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let count = subscribers.len();
        for sub in subscribers {
            // 1. Direct channel
            if let Some(tx) = &sub.direct_tx {
                let _ = tx.send(message_json.to_string());
            }

            // 2. Main thread / Worker event loop task queue
            if let Some(tasks) = &sub.task_queue {
                let escaped = serde_json::to_string(message_json)
                    .unwrap_or_else(|_| format!("\"{}\"", message_json));
                let js_eval = format!(
                    "if (typeof _mangoDispatchBroadcastMessage === 'function') {{ \
                        _mangoDispatchBroadcastMessage({}, {}); \
                    }}",
                    sub.id, escaped
                );
                if let Ok(mut q) = tasks.lock() {
                    q.push(js_eval);
                }
                mango_core::wake_event_loop();
            }
        }

        count
    }

    /// Returns the number of active subscribers for a channel on a given origin.
    pub fn subscriber_count(&self, origin: &str, name: &str) -> usize {
        let key = ChannelKey::new(origin, name);
        if let Ok(lock) = self.state.lock() {
            lock.channels.get(&key).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_broadcast_channel_same_origin_same_name() {
        let hub = BroadcastChannelHub::new();
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();

        let id1 = hub.next_id();
        let id2 = hub.next_id();

        hub.subscribe("https://example.com", "sync", id1, None, Some(tx1));
        hub.subscribe("https://example.com", "sync", id2, None, Some(tx2));

        // Sender 1 sends a message
        let recipients = hub.broadcast("https://example.com", "sync", id1, r#"{"type":"reload"}"#);
        assert_eq!(recipients, 1);

        // Sender 1 must NOT receive its own message
        assert!(rx1.try_recv().is_err());

        // Receiver 2 MUST receive the message
        let msg = rx2.try_recv().expect("should receive message");
        assert_eq!(msg, r#"{"type":"reload"}"#);
    }

    #[test]
    fn test_broadcast_channel_cross_origin_isolation() {
        let hub = BroadcastChannelHub::new();
        let (tx1, _rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();

        let id1 = hub.next_id();
        let id2 = hub.next_id();

        // Different origins
        hub.subscribe("https://site-a.com", "news", id1, None, Some(tx1));
        hub.subscribe("https://site-b.com", "news", id2, None, Some(tx2));

        let delivered = hub.broadcast("https://site-a.com", "news", id1, "hello");
        assert_eq!(delivered, 0);
        assert!(rx2.try_recv().is_err());
    }

    #[test]
    fn test_broadcast_channel_unsubscribe_on_close() {
        let hub = BroadcastChannelHub::new();
        let (tx, rx) = mpsc::channel();
        let id1 = hub.next_id();
        let id2 = hub.next_id();

        hub.subscribe("https://example.com", "ch", id1, None, Some(tx));
        assert_eq!(hub.subscriber_count("https://example.com", "ch"), 1);

        hub.unsubscribe("https://example.com", "ch", id1);
        assert_eq!(hub.subscriber_count("https://example.com", "ch"), 0);

        let delivered = hub.broadcast("https://example.com", "ch", id2, "test");
        assert_eq!(delivered, 0);
        assert!(rx.try_recv().is_err());
    }
}
