//! DOM API bindings that bridge JavaScript to Mango's arena-based DOM.

use std::cell::RefCell;
use std::rc::Rc;

use mango_html::dom::Document;

/// Shared document reference accessible from JS callbacks.
pub type SharedDocument = Rc<RefCell<Document>>;

/// Flag indicating the DOM was mutated and needs re-layout.
pub type DomDirtyFlag = Rc<RefCell<bool>>;

/// Published layout geometry: DOM node id -> `[x, y, width, height]` in viewport pixels.
///
/// The browser refreshes this after every layout so that JS measurement APIs
/// (`getBoundingClientRect`, `offsetWidth`, `elementFromPoint`) return real values
/// without `mango_js` depending on `mango_layout`.
pub type LayoutBounds = Rc<RefCell<std::collections::HashMap<u32, [f32; 4]>>>;

/// Shared cookie jar backing `document.cookie`.
///
/// The browser owns the jar (it loads and saves it from the profile) and shares
/// that exact handle with the HTTP loader and this runtime, so a script write is
/// visible to the very next request with no copy step.
pub type CookieStore = std::sync::Arc<std::sync::Mutex<mango_net::CookieJar>>;

pub(crate) fn jar_lock(store: &CookieStore) -> std::sync::MutexGuard<'_, mango_net::CookieJar> {
    store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
