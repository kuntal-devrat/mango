use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::event_loop::EventLoop;

/// Shared mutable event loop accessible from both JS callbacks and the runtime.
pub type SharedEventLoop = Rc<RefCell<EventLoop>>;

/// Shared status text for `alert()` display.
pub type SharedStatusText = Rc<RefCell<String>>;

/// Shared pending navigation URL triggered by `location.href` assignment.
pub type SharedPendingNav = Rc<RefCell<Option<String>>>;

/// Represents a window opening request triggered by `window.open(url, target, features)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingWindowOpen {
    pub url: String,
    pub target: String,
    pub features: String,
}

/// Shared pending window open request.
pub type SharedPendingOpenWindow = Rc<RefCell<Option<PendingWindowOpen>>>;

/// Represents a pending scroll request triggered by `scrollIntoView()` or `window.scrollTo()`.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingScroll {
    pub target_node: Option<mango_html::dom::NodeId>,
    pub x: f32,
    pub y: f32,
    pub smooth: bool,
}

/// Shared pending scroll request.
pub type SharedPendingScroll = Rc<RefCell<Option<PendingScroll>>>;

/// Thread-safe localStorage store. Keyed by string; values are UTF-8 strings.
/// `Arc<Mutex<_>>` so the browser can persist it to disk after each page unload.
pub type SharedLocalStorage = Arc<Mutex<HashMap<String, String>>>;

/// Thread-safe sessionStorage store (tab-scoped, in-memory). Keyed by string (`{origin}\x1f{key}`).
pub type SharedSessionStorage = Arc<Mutex<HashMap<String, String>>>;

/// Thread-safe cookie jar handle.
pub type SharedCookieStore = Arc<Mutex<mango_net::CookieJar>>;
