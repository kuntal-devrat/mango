//! Central JavaScript runtime for the Mango browser.
//!
//! Wraps a Boa `Context` with registered DOM bindings, console, and Web APIs.
//! The browser creates one `JsRuntime` per page load and uses it to execute
//! inline `<script>` tags and handle timer callbacks.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use boa_engine::{Context, Source};
use mango_html::dom::Document;

use crate::console::{self, ConsoleBuffer, ConsoleMessage};
use crate::dom_bindings::{self, DomDirtyFlag, SharedDocument};
use crate::event_loop::EventLoop;
use crate::web_apis::{self, SharedEventLoop, SharedLocalStorage, SharedStatusText};

/// Keyboard / mouse event modifier flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl EventModifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };
}

/// The Mango JavaScript runtime — one per page.
pub struct JsRuntime {
    context: Context,
    event_loop: SharedEventLoop,
    console_buffer: ConsoleBuffer,
    document: SharedDocument,
    dom_dirty: DomDirtyFlag,
    status_text: SharedStatusText,
    pending_nav: web_apis::SharedPendingNav,
    /// Shared localStorage backing store — the browser may persist this to disk.
    pub local_storage: SharedLocalStorage,
    /// Shared sessionStorage backing store — tab-scoped, in-memory.
    pub session_storage: web_apis::SharedSessionStorage,
    /// Layout geometry published by the browser for measurement APIs.
    layout_bounds: dom_bindings::LayoutBounds,
    /// Shared cookie jar backing `document.cookie` — owned by the browser so
    /// it can be persisted to the profile (GAP-016).
    cookie_jar: dom_bindings::CookieStore,
    /// Set of DOM nodes currently in the `:hover` chain (target and its ancestors).
    hovered_nodes: std::collections::HashSet<mango_html::dom::NodeId>,
    /// Set of DOM nodes currently in the `:active` chain (target and its ancestors).
    active_nodes: std::collections::HashSet<mango_html::dom::NodeId>,
}

impl JsRuntime {
    /// Creates a new JS runtime with a fresh Boa context, registers all
    /// DOM bindings, console, and Web API globals.
    pub fn new(document: Document, viewport_width: f32, viewport_height: f32) -> Self {
        Self::new_with_url(document, viewport_width, viewport_height, "about:blank")
    }

    /// Creates a new JS runtime initialized with the target page URL.
    pub fn new_with_url(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
    ) -> Self {
        Self::new_with_url_and_storage(
            document,
            viewport_width,
            viewport_height,
            page_url,
            Arc::new(Mutex::new(HashMap::new())),
        )
    }

    /// Creates a new JS runtime with a shared localStorage backing store.
    ///
    /// Pass a pre-populated map to restore localStorage across page loads
    /// (GAP-017 — localStorage persistence).
    pub fn new_with_url_and_storage(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
    ) -> Self {
        Self::new_with_url_storage_and_cookies(
            document,
            viewport_width,
            viewport_height,
            page_url,
            local_storage,
            Arc::new(Mutex::new(mango_net::CookieJar::new())),
        )
    }

    /// Creates a new JS runtime with shared localStorage **and** a shared cookie jar.
    ///
    /// The browser owns the cookie jar so it can be flushed to the profile file;
    /// sharing it means `document.cookie` writes are picked up by the very next
    /// `HttpClient` request without an explicit copy step.
    pub fn new_with_url_storage_and_cookies(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
        cookie_jar: dom_bindings::CookieStore,
    ) -> Self {
        Self::new_with_url_storage_session_and_cookies(
            document,
            viewport_width,
            viewport_height,
            page_url,
            local_storage,
            Arc::new(Mutex::new(HashMap::new())),
            cookie_jar,
        )
    }

    /// Creates a new JS runtime with shared localStorage, tab-scoped sessionStorage, and cookie jar.
    pub fn new_with_url_storage_session_and_cookies(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
        session_storage: web_apis::SharedSessionStorage,
        cookie_jar: dom_bindings::CookieStore,
    ) -> Self {
        let mut context = Context::default();

        let shared_doc: SharedDocument = Rc::new(RefCell::new(document));
        let dom_dirty: DomDirtyFlag = Rc::new(RefCell::new(false));
        let event_loop: SharedEventLoop = Rc::new(RefCell::new(EventLoop::new()));
        let console_buffer = console::new_console_buffer();
        let status_text: SharedStatusText = Rc::new(RefCell::new(String::new()));
        let pending_nav: web_apis::SharedPendingNav = Rc::new(RefCell::new(None));
        let vw = Rc::new(RefCell::new(viewport_width));
        let vh = Rc::new(RefCell::new(viewport_height));
        let layout_bounds: dom_bindings::LayoutBounds =
            Rc::new(RefCell::new(std::collections::HashMap::new()));

        // Register all APIs
        console::register_console(&mut context, console_buffer.clone());
        dom_bindings::register_document_api(
            &mut context,
            shared_doc.clone(),
            dom_dirty.clone(),
            layout_bounds.clone(),
            cookie_jar.clone(),
            page_url.to_string(),
        );
        web_apis::register_web_apis(
            &mut context,
            event_loop.clone(),
            status_text.clone(),
            vw,
            vh,
            pending_nav.clone(),
            page_url,
            local_storage.clone(),
            session_storage.clone(),
            cookie_jar.clone(),
        );

        Self {
            context,
            event_loop,
            console_buffer,
            document: shared_doc,
            dom_dirty,
            status_text,
            pending_nav,
            local_storage,
            session_storage,
            layout_bounds,
            cookie_jar,
            hovered_nodes: std::collections::HashSet::new(),
            active_nodes: std::collections::HashSet::new(),
        }
    }

    /// Persists the runtime's localStorage to the specified file path.
    pub fn persist_local_storage(&self, path: &std::path::Path) -> std::io::Result<()> {
        web_apis::save_local_storage_to_file(path, &self.local_storage)
    }

    /// Restores localStorage from the specified file path into the runtime's store.
    pub fn load_local_storage(&self, path: &std::path::Path) -> std::io::Result<usize> {
        web_apis::load_local_storage_from_file(path, &self.local_storage)
    }

    /// Returns the shared cookie jar handle owned by the browser.
    pub fn cookie_jar(&self) -> dom_bindings::CookieStore {
        self.cookie_jar.clone()
    }

    /// Publishes layout geometry (DOM node id → `[x, y, width, height]`) so that
    /// `getBoundingClientRect()`, `offsetWidth`, and `elementFromPoint()` return
    /// values from the real layout tree instead of placeholders.
    pub fn set_layout_bounds(&self, bounds: std::collections::HashMap<u32, [f32; 4]>) {
        *self.layout_bounds.borrow_mut() = bounds;
    }

    /// Executes a JavaScript source string (e.g., inline `<script>` content).
    pub fn execute_script(&mut self, source: &str) -> Result<(), String> {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.context.eval(Source::from_bytes(source))
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(_)) => {
                log::info!("Script executed successfully ({} bytes)", source.len());
                let _ = self.context.run_jobs();
                Ok(())
            }
            Ok(Err(e)) => {
                let err_msg = format!("JS Error: {}", e);
                log::warn!("{}", err_msg);
                Err(err_msg)
            }
            Err(panic_err) => {
                let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown JS panic".to_string()
                };
                let err_msg = format!("JS Panic caught safely: {}", msg);
                log::warn!("{}", err_msg);
                Err(err_msg)
            }
        }
    }

    /// Evaluates a JavaScript expression and returns its string result.
    pub fn eval(&mut self, source: &str) -> Result<String, String> {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.context.eval(Source::from_bytes(source))
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(val)) => {
                let _ = self.context.run_jobs();
                let s = val
                    .to_string(&mut self.context)
                    .map(|js_str| js_str.to_std_string_escaped())
                    .unwrap_or_else(|_| format!("{:?}", val));
                Ok(s)
            }
            Ok(Err(e)) => {
                let err_msg = format!("JS Error: {}", e);
                log::warn!("{}", err_msg);
                Err(err_msg)
            }
            Err(panic_err) => {
                let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown JS panic".to_string()
                };
                let err_msg = format!("JS Panic caught safely: {}", msg);
                log::warn!("{}", err_msg);
                Err(err_msg)
            }
        }
    }

    /// Processes any expired timers and drains promise microtasks. Returns `true` if the DOM was mutated.
    pub fn tick(&mut self) -> bool {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let timer_fired = self.event_loop.borrow_mut().tick(&mut self.context);
            let _ = self.context.run_jobs();
            timer_fired
        }));
        std::panic::set_hook(prev_hook);
        let fired = res.unwrap_or(false);
        let dirty = *self.dom_dirty.borrow();
        if dirty {
            *self.dom_dirty.borrow_mut() = false;
        }
        fired || dirty
    }

    /// Drains any queued Promise microtasks, MutationObserver callbacks, or deferred jobs.
    pub fn run_jobs(&mut self) {
        let _ = self.context.run_jobs();
    }

    /// Returns true if the DOM has been mutated since the last check.
    pub fn is_dom_dirty(&self) -> bool {
        *self.dom_dirty.borrow()
    }

    /// Clears the DOM dirty flag.
    pub fn clear_dom_dirty(&self) {
        *self.dom_dirty.borrow_mut() = false;
    }

    /// Returns true if there are pending timers.
    pub fn has_pending_timers(&self) -> bool {
        self.event_loop.borrow().has_pending_timers()
    }

    /// Returns console messages captured since the last drain.
    pub fn drain_console_messages(&self) -> Vec<ConsoleMessage> {
        let mut buf = self.console_buffer.borrow_mut();
        let messages = buf.clone();
        buf.clear();
        messages
    }

    /// Returns any status text set by `alert()` or similar.
    pub fn take_status_text(&self) -> Option<String> {
        let mut st = self.status_text.borrow_mut();
        if st.is_empty() {
            None
        } else {
            let text = st.clone();
            st.clear();
            Some(text)
        }
    }

    /// Returns a clone of the current DOM document.
    /// Used by the browser to re-layout after JS modifications.
    ///
    /// **BUG-007 note**: This clones the entire Document (O(n) nodes).
    /// Prefer [`document_ref()`] when you only need read access.
    pub fn document_snapshot(&self) -> Document {
        self.document.borrow().clone()
    }

    /// Returns a borrowed reference to the current DOM document.
    /// This avoids the O(n) clone of `document_snapshot()` and should be
    /// preferred for read-only access (e.g., querying attributes, text content).
    pub fn document_ref(&self) -> std::cell::Ref<'_, Document> {
        self.document.borrow()
    }

    /// Sets or clears the active target fragment identifier for `:target` CSS selector matching.
    pub fn set_target_id(&mut self, target_id: Option<String>) {
        self.document.borrow_mut().set_target_id(target_id);
        *self.dom_dirty.borrow_mut() = true;
    }

    /// Takes any pending navigation URL requested by `location.href = ...`.
    pub fn take_pending_navigation(&mut self) -> Option<String> {
        self.pending_nav.borrow_mut().take()
    }

    /// Clears all timers (called on page navigation).
    pub fn clear_timers(&mut self) {
        self.event_loop.borrow_mut().clear_all();
    }

    /// Drains and returns all captured console messages.
    pub fn take_console_messages(&self) -> Vec<crate::console::ConsoleMessage> {
        std::mem::take(&mut *self.console_buffer.borrow_mut())
    }

    /// Sets an attribute on a node by NodeId and marks DOM as dirty.
    pub fn set_node_attribute(&mut self, node_id: mango_html::dom::NodeId, name: &str, value: &str) {
        let mut doc = self.document.borrow_mut();
        if let Some(node) = doc.get_mut(node_id)
            && let mango_html::dom::NodeData::Element(elem) = &mut node.data
        {
            if let Some((_, v)) = elem.attributes.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
                *v = value.to_string();
            } else {
                elem.attributes.push((name.to_string(), value.to_string()));
            }
            *self.dom_dirty.borrow_mut() = true;
        }
    }

    /// Removes an attribute from a node by NodeId and marks DOM as dirty.
    pub fn remove_node_attribute(&mut self, node_id: mango_html::dom::NodeId, name: &str) {
        let mut doc = self.document.borrow_mut();
        if let Some(node) = doc.get_mut(node_id)
            && let mango_html::dom::NodeData::Element(elem) = &mut node.data
        {
            elem.attributes.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
            *self.dom_dirty.borrow_mut() = true;
        }
    }

    /// Gets an attribute value from a node by NodeId.
    pub fn get_node_attribute(&self, node_id: mango_html::dom::NodeId, name: &str) -> Option<String> {
        let doc = self.document.borrow();
        doc.get(node_id).and_then(|node| {
            if let mango_html::dom::NodeData::Element(elem) = &node.data {
                elem.get_attribute(name).map(|s| s.to_string())
            } else {
                None
            }
        })
    }

    /// Dispatches a DOM event through the unified 3-phase capture/target/bubble event pipeline.
    /// `target_node_id`: Some(NodeId) for an element, None or Some(0) for document, or -1 for window.
    /// Returns `true` if `defaultPrevented` was NOT set (i.e. default action may proceed),
    /// or `false` if `preventDefault()` was called.
    pub fn dispatch_event(
        &mut self,
        target_node_id: Option<mango_html::dom::NodeId>,
        event_type: &str,
        event_dict_json: &str,
    ) -> Result<bool, String> {
        let node_id_js = match target_node_id {
            Some(id) => id.raw().to_string(),
            None => "null".to_string(),
        };
        let script = format!(
            "(function() {{ if (typeof globalThis._dispatchInternalEvent === 'function') {{ return globalThis._dispatchInternalEvent({}, {:?}, {}); }} return true; }})()",
            node_id_js, event_type, if event_dict_json.trim().is_empty() { "{}" } else { event_dict_json }
        );
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let val = self.context.eval(Source::from_bytes(&script));
            let _ = self.context.run_jobs();
            val
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(val)) => Ok(val.to_boolean()),
            Ok(Err(e)) => Err(format!("Event dispatch error: {}", e)),
            Err(_) => Err("Event dispatch panic".to_string()),
        }
    }

    /// Dispatches a standard mouse `click` event to the target element or document.
    pub fn dispatch_click(
        &mut self,
        node_id: Option<mango_html::dom::NodeId>,
        client_x: f32,
        client_y: f32,
        button: i16,
        buttons: u16,
        modifiers: EventModifiers,
    ) -> Result<bool, String> {
        let dict = format!(
            r#"{{"eventType":"mouse","bubbles":true,"cancelable":true,"clientX":{},"clientY":{},"button":{},"buttons":{},"ctrlKey":{},"shiftKey":{},"altKey":{},"metaKey":{}}}"#,
            client_x, client_y, button, buttons, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.meta
        );
        self.dispatch_event(node_id, "click", &dict)
    }

    /// Dispatches a mouse `mousemove` event to the target element or document.
    pub fn dispatch_mouse_move(
        &mut self,
        node_id: Option<mango_html::dom::NodeId>,
        client_x: f32,
        client_y: f32,
        buttons: u16,
        modifiers: EventModifiers,
    ) -> Result<bool, String> {
        let dict = format!(
            r#"{{"eventType":"mouse","bubbles":true,"cancelable":true,"clientX":{},"clientY":{},"button":0,"buttons":{},"ctrlKey":{},"shiftKey":{},"altKey":{},"metaKey":{}}}"#,
            client_x, client_y, buttons, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.meta
        );
        self.dispatch_event(node_id, "mousemove", &dict)
    }

    /// Dispatches a mouse `mousedown` event to the target element or document.
    pub fn dispatch_mouse_down(
        &mut self,
        node_id: Option<mango_html::dom::NodeId>,
        client_x: f32,
        client_y: f32,
        button: i16,
        buttons: u16,
        modifiers: EventModifiers,
    ) -> Result<bool, String> {
        let dict = format!(
            r#"{{"eventType":"mouse","bubbles":true,"cancelable":true,"clientX":{},"clientY":{},"button":{},"buttons":{},"ctrlKey":{},"shiftKey":{},"altKey":{},"metaKey":{}}}"#,
            client_x, client_y, button, buttons, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.meta
        );
        self.dispatch_event(node_id, "mousedown", &dict)
    }

    /// Dispatches a mouse `mouseup` event to the target element or document.
    pub fn dispatch_mouse_up(
        &mut self,
        node_id: Option<mango_html::dom::NodeId>,
        client_x: f32,
        client_y: f32,
        button: i16,
        buttons: u16,
        modifiers: EventModifiers,
    ) -> Result<bool, String> {
        let dict = format!(
            r#"{{"eventType":"mouse","bubbles":true,"cancelable":true,"clientX":{},"clientY":{},"button":{},"buttons":{},"ctrlKey":{},"shiftKey":{},"altKey":{},"metaKey":{}}}"#,
            client_x, client_y, button, buttons, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.meta
        );
        self.dispatch_event(node_id, "mouseup", &dict)
    }

    /// Dispatches a `keydown` event to the focused element (or document if None).
    pub fn dispatch_key_down(
        &mut self,
        key: &str,
        code: &str,
        key_code: u32,
        modifiers: EventModifiers,
    ) -> Result<bool, String> {
        let dict = format!(
            r#"{{"eventType":"keyboard","bubbles":true,"cancelable":true,"key":{:?},"code":{:?},"keyCode":{},"which":{},"ctrlKey":{},"shiftKey":{},"altKey":{},"metaKey":{}}}"#,
            key, code, key_code, key_code, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.meta
        );
        self.dispatch_event(None, "keydown", &dict)
    }

    /// Dispatches a `keyup` event to the focused element (or document if None).
    pub fn dispatch_key_up(
        &mut self,
        key: &str,
        code: &str,
        key_code: u32,
        modifiers: EventModifiers,
    ) -> Result<bool, String> {
        let dict = format!(
            r#"{{"eventType":"keyboard","bubbles":true,"cancelable":true,"key":{:?},"code":{:?},"keyCode":{},"which":{},"ctrlKey":{},"shiftKey":{},"altKey":{},"metaKey":{}}}"#,
            key, code, key_code, key_code, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.meta
        );
        self.dispatch_event(None, "keyup", &dict)
    }

    /// Dispatches an `input` event to an input/textarea element.
    pub fn dispatch_input(
        &mut self,
        node_id: mango_html::dom::NodeId,
        data: Option<&str>,
    ) -> Result<bool, String> {
        let data_val = match data {
            Some(d) => format!("{:?}", d),
            None => "null".to_string(),
        };
        let dict = format!(
            r#"{{"eventType":"input","bubbles":true,"cancelable":false,"data":{},"inputType":"insertText"}}"#,
            data_val
        );
        self.dispatch_event(Some(node_id), "input", &dict)
    }

    /// Dispatches a `change` event to an input, select, or textarea element.
    pub fn dispatch_change(&mut self, node_id: mango_html::dom::NodeId) -> Result<bool, String> {
        let dict = r#"{"bubbles":true,"cancelable":false}"#;
        self.dispatch_event(Some(node_id), "change", dict)
    }

    /// Dispatches a `submit` event to a form element.
    pub fn dispatch_submit(&mut self, node_id: mango_html::dom::NodeId) -> Result<bool, String> {
        let dict = r#"{"eventType":"submit","bubbles":true,"cancelable":true}"#;
        self.dispatch_event(Some(node_id), "submit", dict)
    }

    /// Dispatches a `reset` event to a form element.
    pub fn dispatch_reset(&mut self, node_id: mango_html::dom::NodeId) -> Result<bool, String> {
        let dict = r#"{"bubbles":true,"cancelable":true}"#;
        self.dispatch_event(Some(node_id), "reset", dict)
    }

    /// Dispatches the `DOMContentLoaded` event on `document`.
    pub fn dispatch_dom_content_loaded(&mut self) -> Result<bool, String> {
        let dict = r#"{"bubbles":true,"cancelable":false}"#;
        self.dispatch_event(None, "DOMContentLoaded", dict)
    }

    /// Dispatches the `load` event on `window`.
    pub fn dispatch_load(&mut self) -> Result<bool, String> {
        let script = r#"(function() {
            if (typeof globalThis._dispatchInternalEvent === 'function') {
                return globalThis._dispatchInternalEvent(-1, 'load', { bubbles: false, cancelable: false });
            }
            return true;
        })()"#;
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let val = self.context.eval(Source::from_bytes(script));
            let _ = self.context.run_jobs();
            val
        }));
        std::panic::set_hook(prev_hook);
        match res {
            Ok(Ok(val)) => Ok(val.to_boolean()),
            Ok(Err(e)) => Err(format!("Load event error: {}", e)),
            Err(_) => Err("Load event panic".to_string()),
        }
    }

    /// Dispatches a `resize` event on `window` and updates inner dimensions.
    pub fn dispatch_window_resize(&mut self, width: f32, height: f32) -> Result<bool, String> {
        let script = format!(
            r#"(function() {{
                try {{ globalThis.innerWidth = {}; }} catch(e) {{}}
                try {{ globalThis.innerHeight = {}; }} catch(e) {{}}
                if (typeof globalThis !== 'undefined' && globalThis.window) {{
                    try {{ globalThis.window.innerWidth = {}; }} catch(e) {{}}
                    try {{ globalThis.window.innerHeight = {}; }} catch(e) {{}}
                }}
                if (typeof globalThis._dispatchInternalEvent === 'function') {{
                    return globalThis._dispatchInternalEvent(-1, 'resize', {{ bubbles: false, cancelable: false, eventType: 'ui' }});
                }}
                return true;
            }})()"#,
            width, height, width, height
        );
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let val = self.context.eval(Source::from_bytes(&script));
            let _ = self.context.run_jobs();
            val
        }));
        std::panic::set_hook(prev_hook);
        match res {
            Ok(Ok(val)) => Ok(val.to_boolean()),
            Ok(Err(e)) => Err(format!("Resize event error: {}", e)),
            Err(_) => Err("Resize event panic".to_string()),
        }
    }

    /// Dispatches a `scroll` event on `window` and updates scroll offsets.
    pub fn dispatch_window_scroll(&mut self, scroll_x: f32, scroll_y: f32) -> Result<bool, String> {
        let script = format!(
            r#"(function() {{
                if (typeof globalThis !== 'undefined' && globalThis.window) {{
                    globalThis.window.scrollX = {};
                    globalThis.window.scrollY = {};
                    globalThis.window.pageXOffset = {};
                    globalThis.window.pageYOffset = {};
                }}
                if (typeof globalThis._dispatchInternalEvent === 'function') {{
                    return globalThis._dispatchInternalEvent(-1, 'scroll', {{ bubbles: false, cancelable: false, eventType: 'ui' }});
                }}
                return true;
            }})()"#,
            scroll_x, scroll_y, scroll_x, scroll_y
        );
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let val = self.context.eval(Source::from_bytes(&script));
            let _ = self.context.run_jobs();
            val
        }));
        std::panic::set_hook(prev_hook);
        match res {
            Ok(Ok(val)) => Ok(val.to_boolean()),
            Ok(Err(e)) => Err(format!("Scroll event error: {}", e)),
            Err(_) => Err("Scroll event panic".to_string()),
        }
    }

    /// Sets the currently hovered DOM element by NodeId (or None if no element is hovered).
    /// Updates `data-mango-hover="true"` on the element and all of its ancestor elements,
    /// and removes `data-mango-hover` from elements that are no longer hovered.
    /// Returns `true` if the hover state changed (and thus CSS `:hover` rules will re-evaluate).
    pub fn set_hover_state(&mut self, target_node_id: Option<mango_html::dom::NodeId>) -> bool {
        let mut new_chain = std::collections::HashSet::new();
        if let Some(target) = target_node_id {
            let doc = self.document.borrow();
            let mut cur = Some(target);
            let mut guard = 0;
            while let Some(nid) = cur {
                if guard > 512 { break; }
                guard += 1;
                new_chain.insert(nid);
                cur = doc.get(nid).and_then(|n| n.parent);
            }
        }

        if new_chain == self.hovered_nodes {
            return false;
        }

        let mut doc = self.document.borrow_mut();
        // Remove from unhovered
        for nid in &self.hovered_nodes {
            if !new_chain.contains(nid) {
                if let Some(node) = doc.get_mut(*nid) {
                    if let mango_html::dom::NodeData::Element(el) = &mut node.data {
                        el.attributes.retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-hover"));
                    }
                }
            }
        }
        // Add to newly hovered
        for nid in &new_chain {
            if !self.hovered_nodes.contains(nid) {
                if let Some(node) = doc.get_mut(*nid) {
                    if let mango_html::dom::NodeData::Element(el) = &mut node.data {
                        if let Some((_, v)) = el.attributes.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("data-mango-hover")) {
                            *v = "true".to_string();
                        } else {
                            el.attributes.push(("data-mango-hover".to_string(), "true".to_string()));
                        }
                    }
                }
            }
        }
        self.hovered_nodes = new_chain;
        *self.dom_dirty.borrow_mut() = true;
        true
    }

    /// Sets the currently active (mouse pressed) DOM element by NodeId (or None if released).
    /// Updates `data-mango-active="true"` on the element and all of its ancestor elements,
    /// and removes `data-mango-active` from elements that are no longer active.
    /// Returns `true` if the active state changed (and thus CSS `:active` rules will re-evaluate).
    pub fn set_active_state(&mut self, target_node_id: Option<mango_html::dom::NodeId>) -> bool {
        let mut new_chain = std::collections::HashSet::new();
        if let Some(target) = target_node_id {
            let doc = self.document.borrow();
            let mut cur = Some(target);
            let mut guard = 0;
            while let Some(nid) = cur {
                if guard > 512 { break; }
                guard += 1;
                new_chain.insert(nid);
                cur = doc.get(nid).and_then(|n| n.parent);
            }
        }

        if new_chain == self.active_nodes {
            return false;
        }

        let mut doc = self.document.borrow_mut();
        // Remove from inactive
        for nid in &self.active_nodes {
            if !new_chain.contains(nid) {
                if let Some(node) = doc.get_mut(*nid) {
                    if let mango_html::dom::NodeData::Element(el) = &mut node.data {
                        el.attributes.retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-active"));
                    }
                }
            }
        }
        // Add to newly active
        for nid in &new_chain {
            if !self.active_nodes.contains(nid) {
                if let Some(node) = doc.get_mut(*nid) {
                    if let mango_html::dom::NodeData::Element(el) = &mut node.data {
                        if let Some((_, v)) = el.attributes.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("data-mango-active")) {
                            *v = "true".to_string();
                        } else {
                            el.attributes.push(("data-mango-active".to_string(), "true".to_string()));
                        }
                    }
                }
            }
        }
        self.active_nodes = new_chain;
        *self.dom_dirty.borrow_mut() = true;
        true
    }

    /// Returns the currently hovered node IDs.
    pub fn get_hovered_nodes(&self) -> &std::collections::HashSet<mango_html::dom::NodeId> {
        &self.hovered_nodes
    }

    /// Returns the currently active node IDs.
    pub fn get_active_nodes(&self) -> &std::collections::HashSet<mango_html::dom::NodeId> {
        &self.active_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_html::parse_html;

    #[test]
    fn test_js_eval_and_console() {
        let doc = parse_html("<html><body></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let result = rt.execute_script("console.log('Hello from Boa JS!', 42);");
        assert!(result.is_ok());

        let messages = rt.drain_console_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].level, console::ConsoleLevel::Log);
        assert_eq!(messages[0].text, "Hello from Boa JS! 42");
    }

    #[test]
    fn test_dom_get_element_and_modify_text() {
        let html = r#"<html><body><h1 id="title">Old Title</h1></body></html>"#;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            var el = document.getElementById("title");
            console.log("Found title:", el.textContent);
            el.textContent = "New Dynamic Title!";
        "#;
        assert!(rt.execute_script(script).is_ok());
        assert!(rt.is_dom_dirty());

        let updated_doc = rt.document_snapshot();
        let root = updated_doc.root();
        let h1_node = updated_doc.find_element_by_tag(root, "h1").unwrap();
        assert_eq!(updated_doc.text_content(h1_node), "New Dynamic Title!");
    }

    #[test]
    fn test_dom_create_element_and_append() {
        let html = r#"<html><body><div id="container"></div></body></html>"#;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            var container = document.getElementById("container");
            var p = document.createElement("p");
            p.textContent = "Inserted Paragraph";
            p.setAttribute("class", "lead");
            container.appendChild(p);
        "#;
        assert!(rt.execute_script(script).is_ok());
        assert!(rt.is_dom_dirty());

        let updated_doc = rt.document_snapshot();
        let root = updated_doc.root();
        let p_node = updated_doc.find_element_by_tag(root, "p").unwrap();
        assert_eq!(updated_doc.text_content(p_node), "Inserted Paragraph");
        if let mango_html::dom::NodeData::Element(ref elem) = updated_doc.get(p_node).unwrap().data {
            assert_eq!(elem.get_attribute("class"), Some("lead"));
        } else {
            panic!("Expected element node");
        }
    }

    #[test]
    fn test_timers_set_timeout() {
        let html = r#"<html><body><div id="counter">0</div></body></html>"#;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            setTimeout(function() {
                var el = document.getElementById("counter");
                el.textContent = "1";
            }, 10);
        "#;
        assert!(rt.execute_script(script).is_ok());
        assert!(rt.has_pending_timers());

        // Wait a bit to let the timer expire
        std::thread::sleep(std::time::Duration::from_millis(20));
        let changed = rt.tick();
        assert!(changed);

        let updated_doc = rt.document_snapshot();
        let root = updated_doc.root();
        let div = updated_doc.find_element_by_tag(root, "div").unwrap();
        assert_eq!(updated_doc.text_content(div), "1");
    }

    #[test]
    fn test_web_platform_prototypes_and_node_constants() {
        let doc = parse_html("<html><body><div id='test'></div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // Node constants
            if (Node.ELEMENT_NODE !== 1) throw new Error("Node.ELEMENT_NODE mismatch");
            if (Node.TEXT_NODE !== 3) throw new Error("Node.TEXT_NODE mismatch");
            if (Node.DOCUMENT_FRAGMENT_NODE !== 11) throw new Error("Node.DOCUMENT_FRAGMENT_NODE mismatch");

            // Prototypes
            if (!(new HTMLTemplateElement() instanceof HTMLElement)) throw new Error("Template not HTMLElement");
            if (!(new HTMLElement() instanceof Element)) throw new Error("HTMLElement not Element");
            if (!(new Element() instanceof Node)) throw new Error("Element not Node");
            if (!(new Node() instanceof EventTarget)) throw new Error("Node not EventTarget");
            if (!(new ShadowRoot() instanceof DocumentFragment)) throw new Error("ShadowRoot not DocumentFragment");
            if (!(new CDATASection() instanceof Text)) throw new Error("CDATASection not Text");
            if (!(new CustomEvent("test") instanceof Event)) throw new Error("CustomEvent not Event");
            if (!(new MouseEvent("click") instanceof Event)) throw new Error("MouseEvent not Event");
            if (!customElements || !(customElements instanceof CustomElementRegistry)) throw new Error("CustomElements registry missing");
        "#;
        assert!(rt.execute_script(script).is_ok());
    }

    #[test]
    fn test_document_advanced_apis() {
        let doc = parse_html("<html><body><div id='box'></div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // document.implementation
            var inert = document.implementation.createHTMLDocument("inert");
            if (!inert || inert.title !== "inert") throw new Error("createHTMLDocument failed");

            // document factories
            var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
            if (!svg || svg.tagName !== "SVG") throw new Error("createElementNS failed");

            var b = document.body;
            var tw = document.createTreeWalker(b, NodeFilter.SHOW_ALL, null);
            if (!tw || tw.root !== b) throw new Error("createTreeWalker failed");

            var evt = document.createEvent("Event");
            evt.initEvent("custom", true, true);
            if (evt.type !== "custom" || !evt.bubbles) throw new Error("createEvent failed");

            if (document.readyState !== "complete") throw new Error("readyState not complete");
        "#;
        if let Err(e) = rt.execute_script(script) {
            panic!("Document test failed: {}", e);
        }
    }

    #[test]
    fn test_canvas_2d_and_web_animations() {
        let doc = parse_html("<html><body><canvas id='c'></canvas><div id='box'></div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            var c = document.getElementById("c");
            if (!c || c.width !== 300 || c.height !== 150) throw new Error("Canvas initial dims invalid");
            if (typeof HTMLCanvasElement !== 'undefined' && !(c instanceof HTMLCanvasElement)) {
                throw new Error("c not instanceof HTMLCanvasElement");
            }

            var ctx = c.getContext("2d");
            if (!ctx || ctx.canvas !== c) throw new Error("Canvas context or back-reference missing");
            if (typeof CanvasRenderingContext2D !== 'undefined' && !(ctx instanceof CanvasRenderingContext2D)) {
                throw new Error("ctx not instanceof CanvasRenderingContext2D");
            }

            ctx.fillStyle = "#ff0000";
            ctx.fillRect(0, 0, 100, 100);

            // Verify real rasterized red pixel at (50, 50)
            var imgData = ctx.getImageData(50, 50, 1, 1);
            if (!imgData || imgData.data[0] !== 255 || imgData.data[1] !== 0 || imgData.data[2] !== 0 || imgData.data[3] !== 255) {
                throw new Error("Canvas pixel verification failed: got [" + (imgData ? Array.from(imgData.data) : null) + "]");
            }

            // Verify pixel outside filled rect is transparent
            var outside = ctx.getImageData(150, 50, 1, 1);
            if (outside.data[3] !== 0) throw new Error("Outside pixel should be transparent");

            ctx.save();
            ctx.restore();

            // Real fontdue font metrics
            var metrics = ctx.measureText("Hello");
            if (!metrics || !(metrics.width > 0)) throw new Error("Canvas measureText failed: " + JSON.stringify(metrics));

            // PNG data URL snapshot
            var url = c.toDataURL();
            if (!url || url.indexOf("data:image/png;base64,") !== 0) throw new Error("Canvas toDataURL failed");

            var box = document.getElementById("box");
            var anim = box.animate([{ opacity: 0 }, { opacity: 1 }], 1000);
            if (!anim || typeof anim.play !== "function" || typeof anim.cancel !== "function") {
                throw new Error("Web Animations API failed");
            }
        "##;
        if let Err(e) = rt.execute_script(script) {
            panic!("Canvas test failed: {}", e);
        }
    }

    #[test]
    fn test_dom_selectors_matches_closest_and_compat_mode() {
        let html = r##"<!DOCTYPE html>
<html>
<body>
    <div id="wrapper" class="container main-content">
        <ul id="list">
            <li class="item active"><a href="#1" class="link">Link 1</a></li>
            <li class="item"><a href="#2" class="link">Link 2</a></li>
        </ul>
    </div>
</body>
</html>"##;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            if (document.compatMode !== "CSS1Compat") {
                throw new Error("compatMode should be CSS1Compat, got " + document.compatMode);
            }

            // querySelector with complex selector
            var activeLink = document.querySelector("#list > li.active a.link");
            if (!activeLink) throw new Error("querySelector activeLink not found");
            if (activeLink.textContent !== "Link 1") throw new Error("wrong textContent: " + activeLink.textContent);

            // matches
            if (!activeLink.matches("a.link")) throw new Error("activeLink should match a.link");
            if (activeLink.matches("div")) throw new Error("activeLink should not match div");

            // closest
            var parentLi = activeLink.closest("li.item");
            if (!parentLi) throw new Error("closest li.item not found");
            if (!parentLi.classList.contains("active")) {
                throw new Error("closest returned wrong li");
            }
            var container = activeLink.closest("#wrapper");
            if (!container || container.id !== "wrapper") throw new Error("closest #wrapper failed");

            // querySelectorAll
            var allItems = document.querySelectorAll("ul > li.item");
            if (allItems.length !== 2) throw new Error("querySelectorAll returned length " + allItems.length);

            // Tree traversal
            var list = document.getElementById("list");
            if (!list) throw new Error("list not found");
            var firstLi = list.firstElementChild;
            if (!firstLi || !firstLi.classList.contains("active")) throw new Error("firstElementChild failed");
            var nextLi = firstLi.nextElementSibling;
            if (!nextLi || nextLi.classList.contains("active")) throw new Error("nextElementSibling failed");
            var prevLi = nextLi.previousElementSibling;
            if (!prevLi || prevLi._nodeId !== firstLi._nodeId) throw new Error("previousElementSibling failed");
            if (firstLi.parentElement._nodeId !== list._nodeId) throw new Error("parentElement failed");
        "##;
        if let Err(e) = rt.execute_script(script) {
            panic!("Selector & navigation test failed: {}", e);
        }

        // Quirks mode document (missing DOCTYPE)
        let quirks_doc = parse_html("<html><body><p>Hello</p></body></html>");
        let mut quirks_rt = JsRuntime::new(quirks_doc, 800.0, 600.0);
        let quirks_script = r#"
            if (document.compatMode !== "BackCompat") {
                throw new Error("compatMode in quirks mode should be BackCompat, got " + document.compatMode);
            }
        "#;
        if let Err(e) = quirks_rt.execute_script(quirks_script) {
            panic!("Quirks compatMode test failed: {}", e);
        }
    }

    #[test]
    fn test_document_character_set_and_content_type() {
        let mut doc = parse_html("<!DOCTYPE html><html><head><meta charset=\"windows-1252\"></head><body></body></html>");
        assert_eq!(doc.character_set, "windows-1252");
        doc.content_type = "text/html".to_string();

        let mut rt = JsRuntime::new(doc, 800.0, 600.0);
        let script = r#"
            if (document.characterSet !== "windows-1252") {
                throw new Error("expected windows-1252, got " + document.characterSet);
            }
            if (document.charset !== "windows-1252") {
                throw new Error("expected charset windows-1252, got " + document.charset);
            }
            if (document.inputEncoding !== "windows-1252") {
                throw new Error("expected inputEncoding windows-1252, got " + document.inputEncoding);
            }
            if (document.contentType !== "text/html") {
                throw new Error("expected contentType text/html, got " + document.contentType);
            }
        "#;
        if let Err(e) = rt.execute_script(script) {
            panic!("document characterSet test failed: {}", e);
        }
    }

    #[test]
    fn test_html_iframe_element_bindings() {
        let doc = parse_html("<!DOCTYPE html><html><body><div id='container'></div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // Prototype hierarchy
            if (!(new HTMLIFrameElement() instanceof HTMLElement)) throw new Error("HTMLIFrameElement not HTMLElement");
            if (!(new HTMLIFrameElement() instanceof Element)) throw new Error("HTMLIFrameElement not Element");

            // Creation via document.createElement
            var frame = document.createElement("iframe");
            if (!(frame instanceof HTMLIFrameElement)) throw new Error("Created iframe not instanceof HTMLIFrameElement");

            // Attributes
            frame.src = "https://mango-browser.internal/frame";
            if (frame.src !== "https://mango-browser.internal/frame") throw new Error("iframe.src getter/setter failed");

            frame.srcdoc = "<h1>Nested</h1>";
            if (frame.srcdoc !== "<h1>Nested</h1>") throw new Error("iframe.srcdoc getter/setter failed");

            frame.width = "600";
            frame.height = "400";
            if (frame.width !== "600" || frame.height !== "400") throw new Error("iframe dimensions failed");

            // Sandbox DOMTokenList
            frame.sandbox.add("allow-scripts");
            frame.sandbox.add("allow-same-origin");
            if (!frame.sandbox.contains("allow-scripts")) throw new Error("sandbox.contains('allow-scripts') failed");
            if (!frame.sandbox.contains("allow-same-origin")) throw new Error("sandbox.contains('allow-same-origin') failed");
            if (frame.sandbox.contains("allow-popups")) throw new Error("sandbox should not contain allow-popups");

            // contentDocument and contentWindow
            var cDoc = frame.contentDocument;
            if (!cDoc) throw new Error("iframe.contentDocument missing");
            if (cDoc.readyState !== "complete") throw new Error("contentDocument readyState invalid");

            var cWin = frame.contentWindow;
            if (!cWin) throw new Error("iframe.contentWindow missing");
            if (cWin.frameElement !== frame) throw new Error("contentWindow.frameElement mismatch");

            // Append to DOM
            var container = document.getElementById("container");
            container.appendChild(frame);
            if (container.children.length !== 1) throw new Error("appendChild failed");
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("HTMLIFrameElement test failed: {}", e);
        }
    }

    #[test]
    fn test_html_video_and_audio_element_bindings() {
        let html = r#"<!DOCTYPE html>
<html>
<body>
    <div id="player-container">
        <video id="vid1" src="https://example.com/movie.mp4" poster="movie.jpg" controls autoplay muted></video>
        <audio id="aud1" src="https://example.com/song.mp3" controls></audio>
    </div>
</body>
</html>"#;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // 1. Prototype & Constants Verification
            if (HTMLMediaElement.HAVE_ENOUGH_DATA !== 4) throw new Error("HTMLMediaElement.HAVE_ENOUGH_DATA mismatch");
            if (HTMLMediaElement.NETWORK_IDLE !== 1) throw new Error("HTMLMediaElement.NETWORK_IDLE mismatch");

            var vDirect = new HTMLVideoElement();
            if (!(vDirect instanceof HTMLMediaElement)) throw new Error("HTMLVideoElement not instanceof HTMLMediaElement");
            if (!(vDirect instanceof HTMLElement)) throw new Error("HTMLVideoElement not instanceof HTMLElement");

            var aDirect = new HTMLAudioElement();
            if (!(aDirect instanceof HTMLMediaElement)) throw new Error("HTMLAudioElement not instanceof HTMLMediaElement");

            // 2. createElement('video') & Property Getters/Setters
            var vNew = document.createElement("video");
            if (!(vNew instanceof HTMLVideoElement)) throw new Error("createElement('video') not HTMLVideoElement");
            if (!(vNew instanceof HTMLMediaElement)) throw new Error("createElement('video') not HTMLMediaElement");

            vNew.src = "promo.webm";
            if (vNew.src !== "promo.webm" || vNew.currentSrc !== "promo.webm") throw new Error("vNew.src mismatch");

            vNew.poster = "thumb.png";
            if (vNew.poster !== "thumb.png") throw new Error("vNew.poster mismatch");

            vNew.controls = true;
            if (!vNew.controls) throw new Error("vNew.controls should be true");
            vNew.controls = false;
            if (vNew.controls) throw new Error("vNew.controls should be false");

            vNew.muted = true;
            if (!vNew.muted) throw new Error("vNew.muted should be true");

            vNew.currentTime = 42.5;
            if (vNew.currentTime !== 42.5) throw new Error("vNew.currentTime should be 42.5, got " + vNew.currentTime);

            vNew.volume = 0.8;
            if (vNew.volume !== 0.8) throw new Error("vNew.volume should be 0.8, got " + vNew.volume);

            if (vNew.canPlayType("video/mp4") !== "probably") throw new Error("canPlayType('video/mp4') failed");
            if (vNew.canPlayType("audio/mp3") !== "probably") throw new Error("canPlayType('audio/mp3') failed");

            // 3. play() and pause() methods
            vNew.play();
            if (vNew.paused) throw new Error("vNew should not be paused after play()");
            vNew.pause();
            if (!vNew.paused) throw new Error("vNew should be paused after pause()");

            // 4. new Audio(src) Constructor
            var snd = new Audio("alert.wav");
            if (!(snd instanceof HTMLAudioElement)) throw new Error("new Audio not instanceof HTMLAudioElement");
            if (!(snd instanceof HTMLMediaElement)) throw new Error("new Audio not instanceof HTMLMediaElement");
            if (snd.src !== "alert.wav") throw new Error("new Audio src mismatch: " + snd.src);
            snd.play();
            if (snd.paused) throw new Error("snd should not be paused after play()");

            // 5. Existing DOM Video & Audio Elements
            var vid1 = document.getElementById("vid1");
            if (!vid1) throw new Error("vid1 not found in DOM");
            if (!(vid1 instanceof HTMLVideoElement)) throw new Error("vid1 not HTMLVideoElement");
            if (!vid1.controls) throw new Error("vid1 should have controls");
            if (!vid1.autoplay) throw new Error("vid1 should have autoplay");
            if (!vid1.muted) throw new Error("vid1 should be muted");
            if (vid1.poster !== "movie.jpg") throw new Error("vid1 poster mismatch: " + vid1.poster);

            var aud1 = document.getElementById("aud1");
            if (!aud1) throw new Error("aud1 not found in DOM");
            if (!(aud1 instanceof HTMLAudioElement)) throw new Error("aud1 not HTMLAudioElement");
            if (!aud1.controls) throw new Error("aud1 should have controls");
            if (aud1.src !== "https://example.com/song.mp3") throw new Error("aud1.src mismatch: " + aud1.src);
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_html_video_and_audio_element_bindings failed: {}", e);
        }
    }

    #[test]
    fn test_canvas_2d_comprehensive_api() {
        let doc = parse_html("<html><body><div id='container'></div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // 1. Dynamic creation via document.createElement('canvas')
            var canvas = document.createElement('canvas');
            canvas.width = 200;
            canvas.height = 100;
            if (canvas.width !== 200 || canvas.height !== 100) throw new Error("Canvas width/height setting failed");

            var ctx = canvas.getContext('2d');
            if (!ctx) throw new Error("getContext('2d') returned null");

            // 2. Clear rect & fill rect
            ctx.fillStyle = '#00ff00';
            ctx.fillRect(10, 10, 50, 50);

            var greenPixel = ctx.getImageData(20, 20, 1, 1);
            if (greenPixel.data[0] !== 0 || greenPixel.data[1] !== 255 || greenPixel.data[2] !== 0 || greenPixel.data[3] !== 255) {
                throw new Error("Green pixel verification failed: got " + Array.from(greenPixel.data));
            }

            // clearRect should clear the area to transparent
            ctx.clearRect(15, 15, 10, 10);
            var clearedPixel = ctx.getImageData(16, 16, 1, 1);
            if (clearedPixel.data[3] !== 0) throw new Error("Cleared pixel should be transparent");

            // 3. PutImageData
            var newImg = ctx.createImageData(2, 2);
            // set to blue
            newImg.data[0] = 0; newImg.data[1] = 0; newImg.data[2] = 255; newImg.data[3] = 255;
            newImg.data[4] = 0; newImg.data[5] = 0; newImg.data[6] = 255; newImg.data[7] = 255;
            newImg.data[8] = 0; newImg.data[9] = 0; newImg.data[10] = 255; newImg.data[11] = 255;
            newImg.data[12] = 0; newImg.data[13] = 0; newImg.data[14] = 255; newImg.data[15] = 255;
            ctx.putImageData(newImg, 100, 50);

            var bluePixel = ctx.getImageData(100, 50, 1, 1);
            if (bluePixel.data[0] !== 0 || bluePixel.data[1] !== 0 || bluePixel.data[2] !== 255 || bluePixel.data[3] !== 255) {
                throw new Error("putImageData blue pixel verification failed: got " + Array.from(bluePixel.data));
            }

            // 4. Path operations (arc, lineTo, fill)
            ctx.beginPath();
            ctx.arc(80, 80, 10, 0, Math.PI * 2);
            ctx.fillStyle = '#ffff00';
            ctx.fill();

            var yellowPixel = ctx.getImageData(80, 80, 1, 1);
            if (yellowPixel.data[0] !== 255 || yellowPixel.data[1] !== 255 || yellowPixel.data[2] !== 0 || yellowPixel.data[3] !== 255) {
                throw new Error("yellow arc pixel failed: got " + Array.from(yellowPixel.data));
            }

            // 5. Gradients and patterns stub compatibility
            var grad = ctx.createLinearGradient(0, 0, 100, 100);
            grad.addColorStop(0, '#fff');
            grad.addColorStop(1, '#000');
            ctx.fillStyle = grad;
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_canvas_2d_comprehensive_api failed: {}", e);
        }
    }

    #[test]
    fn test_web_components_and_template_slot() {
        let doc = parse_html("<html><body><div id='host'><slot>default</slot></div><template id='tmpl'><p>inside template</p></template></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // 1. Template element .content DocumentFragment
            var tmpl = document.getElementById("tmpl");
            if (!tmpl || !(tmpl instanceof HTMLTemplateElement)) throw new Error("tmpl not HTMLTemplateElement");
            var content = tmpl.content;
            if (!content || !(content instanceof DocumentFragment)) throw new Error("tmpl.content not DocumentFragment");
            var cloned = content.cloneNode(true);
            if (!cloned) throw new Error("content.cloneNode failed");

            // 2. Element.attachShadow & ShadowRoot
            var host = document.getElementById("host");
            if (!host) throw new Error("host element not found");
            var shadow = host.attachShadow({ mode: "open" });
            if (!shadow) throw new Error("attachShadow returned falsy");
            if (host.shadowRoot !== shadow) throw new Error("host.shadowRoot mismatch");
            if (shadow.host !== host) throw new Error("shadow.host mismatch");
            if (shadow.mode !== "open") throw new Error("shadow.mode mismatch");

            // Shadow DOM content
            var p = document.createElement("p");
            p.textContent = "Shadow text";
            shadow.appendChild(p);
            if (shadow.childNodes.length !== 1) throw new Error("shadow childNodes length mismatch");

            // 3. Slot element
            var slot = document.createElement("slot");
            if (!(slot instanceof HTMLSlotElement)) throw new Error("slot not HTMLSlotElement");
            shadow.appendChild(slot);
            var assigned = slot.assignedNodes();
            if (!Array.isArray(assigned)) throw new Error("assignedNodes not array");

            // 4. CustomElementRegistry
            var connectedCalls = 0;
            function MyElement() {
                HTMLElement.call(this);
            }
            MyElement.prototype = Object.create(HTMLElement.prototype);
            MyElement.prototype.connectedCallback = function() {
                connectedCalls++;
            };

            customElements.define("my-element", MyElement);
            if (customElements.get("my-element") !== MyElement) throw new Error("customElements.get failed");
            var myEl = document.createElement("my-element");
            if (!(myEl instanceof MyElement)) throw new Error("myEl not instance of MyElement");
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_web_components_and_template_slot failed: {}", e);
        }
    }

    #[test]
    fn test_worker_and_service_worker_apis() {
        let doc = parse_html("<html><body></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // 1. Worker
            if (typeof Worker !== "function") throw new Error("Worker constructor missing");
            var w = new Worker("worker.js");
            if (!w || typeof w.postMessage !== "function" || typeof w.terminate !== "function") {
                throw new Error("Worker instance interface incomplete");
            }
            w.postMessage({ hello: "world" });
            w.terminate();

            // 2. ServiceWorkerContainer
            if (!navigator.serviceWorker) throw new Error("navigator.serviceWorker missing");
            if (typeof navigator.serviceWorker.register !== "function") throw new Error("navigator.serviceWorker.register missing");
            if (!navigator.serviceWorker.ready) throw new Error("navigator.serviceWorker.ready missing");

            var regPromise = navigator.serviceWorker.register("/sw.js");
            if (!(regPromise instanceof Promise)) throw new Error("register must return a Promise");
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_worker_and_service_worker_apis failed: {}", e);
        }
    }

    #[test]
    fn test_missing_html_element_bindings() {
        let html = r##"<!DOCTYPE html>
<html>
<body>
    <dialog id="dlg">Dialog content</dialog>
    <progress id="prog" value="50" max="100"></progress>
    <progress id="indet_prog"></progress>
    <meter id="met" value="0.7" min="0" max="1" low="0.25" high="0.75" optimum="0.8"></meter>
    <output id="out" for="a b">Initial</output>
    <input id="inp" list="dl">
    <datalist id="dl">
        <option value="Apple">
        <option value="Banana">
    </datalist>
    <map id="m" name="map1">
        <area shape="rect" coords="0,0,10,10" href="#rect" alt="RectArea">
    </map>
    <object id="obj" data="test.svg" type="image/svg+xml"></object>
    <embed id="emb" src="player.swf" type="application/x-shockwave-flash" width="300" height="200">
</body>
</html>"##;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r#"
            // 1. HTMLDialogElement
            var dlg = document.getElementById("dlg");
            if (!dlg || !(dlg instanceof HTMLDialogElement)) throw new Error("dlg not HTMLDialogElement");
            if (dlg.open !== false) throw new Error("dlg should initially be closed");
            dlg.show();
            if (dlg.open !== true) throw new Error("dlg.show() should open dialog");
            if (!dlg.hasAttribute("open")) throw new Error("dlg open attribute missing");

            var closeEventFired = false;
            dlg.addEventListener("close", function(e) {
                closeEventFired = true;
            });
            dlg.close("resultVal");
            if (dlg.open !== false) throw new Error("dlg should be closed after close()");
            if (dlg.returnValue !== "resultVal") throw new Error("dlg.returnValue mismatch: " + dlg.returnValue);
            if (!closeEventFired) throw new Error("close event not fired");

            dlg.showModal();
            if (dlg.open !== true) throw new Error("dlg should be open after showModal()");
            // Calling showModal() when already open should throw InvalidStateError
            var threw = false;
            try {
                dlg.showModal();
            } catch (e) {
                threw = true;
            }
            if (!threw) throw new Error("showModal() should throw when already open");
            dlg.close();

            // 2. HTMLProgressElement
            var prog = document.getElementById("prog");
            if (!prog || !(prog instanceof HTMLProgressElement)) throw new Error("prog not HTMLProgressElement");
            if (prog.value !== 50) throw new Error("prog.value should be 50, got " + prog.value);
            if (prog.max !== 100) throw new Error("prog.max should be 100, got " + prog.max);
            if (Math.abs(prog.position - 0.5) > 0.001) throw new Error("prog.position should be 0.5, got " + prog.position);

            var indet = document.getElementById("indet_prog");
            if (indet.position !== -1) throw new Error("indeterminate progress position should be -1, got " + indet.position);
            indet.value = 25;
            if (indet.value !== 25) throw new Error("indet value setter failed");

            // 3. HTMLMeterElement
            var met = document.getElementById("met");
            if (!met || !(met instanceof HTMLMeterElement)) throw new Error("met not HTMLMeterElement");
            if (Math.abs(met.value - 0.7) > 0.001) throw new Error("met.value should be 0.7, got " + met.value);
            if (met.min !== 0) throw new Error("met.min should be 0, got " + met.min);
            if (met.max !== 1) throw new Error("met.max should be 1, got " + met.max);
            if (Math.abs(met.low - 0.25) > 0.001) throw new Error("met.low should be 0.25, got " + met.low);
            if (Math.abs(met.high - 0.75) > 0.001) throw new Error("met.high should be 0.75, got " + met.high);
            if (Math.abs(met.optimum - 0.8) > 0.001) throw new Error("met.optimum should be 0.8, got " + met.optimum);
            met.value = 0.9;
            if (Math.abs(met.value - 0.9) > 0.001) throw new Error("met.value setter failed");

            // 4. HTMLOutputElement
            var out = document.getElementById("out");
            if (!out || !(out instanceof HTMLOutputElement)) throw new Error("out not HTMLOutputElement");
            if (out.type !== "output") throw new Error("out.type should be 'output'");
            if (out.value !== "Initial") throw new Error("out.value should be 'Initial'");
            if (out.defaultValue !== "Initial") throw new Error("out.defaultValue should be 'Initial'");
            out.value = "Updated";
            if (out.value !== "Updated") throw new Error("out.value setter failed");
            if (out.htmlFor !== "a b") throw new Error("out.htmlFor mismatch: " + out.htmlFor);

            // 5. HTMLDataListElement and input.list
            var dl = document.getElementById("dl");
            if (!dl || !(dl instanceof HTMLDataListElement)) throw new Error("dl not HTMLDataListElement");
            if (!dl.options || dl.options.length !== 2) throw new Error("dl.options length should be 2, got " + (dl.options ? dl.options.length : null));
            var inp = document.getElementById("inp");
            if (!inp.list || inp.list !== dl) throw new Error("inp.list should point to dl");

            // 6. HTMLMapElement & HTMLAreaElement
            var m = document.getElementById("m");
            if (!m || !(m instanceof HTMLMapElement)) throw new Error("m not HTMLMapElement");
            if (!m.areas || m.areas.length !== 1) throw new Error("m.areas length should be 1, got " + (m.areas ? m.areas.length : null));
            var area = m.areas[0];
            if (!(area instanceof HTMLAreaElement)) throw new Error("area not HTMLAreaElement");
            if (area.shape !== "rect") throw new Error("area.shape mismatch: " + area.shape);
            if (area.coords !== "0,0,10,10") throw new Error("area.coords mismatch: " + area.coords);
            if (area.alt !== "RectArea") throw new Error("area.alt mismatch: " + area.alt);

            // 7. HTMLObjectElement & HTMLEmbedElement
            var obj = document.getElementById("obj");
            if (!obj || !(obj instanceof HTMLObjectElement)) throw new Error("obj not HTMLObjectElement");
            if (obj.data !== "test.svg") throw new Error("obj.data mismatch: " + obj.data);
            if (obj.type !== "image/svg+xml") throw new Error("obj.type mismatch: " + obj.type);

            var emb = document.getElementById("emb");
            if (!emb || !(emb instanceof HTMLEmbedElement)) throw new Error("emb not HTMLEmbedElement");
            if (emb.src !== "player.swf") throw new Error("emb.src mismatch: " + emb.src);
            if (emb.width !== "300" || emb.height !== "200") throw new Error("emb dims mismatch");
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_missing_html_element_bindings failed: {}", e);
        }
    }

    #[test]
    fn test_form_elements_and_validation() {
        let html = r##"<!DOCTYPE html>
<html>
<body>
    <form id="f1" action="/test">
        <fieldset id="fs1">
            <legend id="leg1">Personal Info</legend>
            <input id="num" type="number" min="10" max="100" step="5" value="20" required>
            <input id="email" type="email" value="bad-email">
            <input id="url" type="url" value="not-a-url">
            <input id="pat" type="text" pattern="[A-Z]{3}" value="abc">
            <input id="dt" type="date" min="2026-01-01" max="2026-12-31" value="2026-06-15">
            <input id="tm" type="time" value="14:30">
            <input id="clr" type="color" value="#ff5500">
            <input id="rng" type="range" min="0" max="10" step="1" value="5">
            <input id="hid" type="hidden" value="secret">
        </fieldset>
        <select id="sel_single">
            <option id="opt_a" value="apple">Apple</option>
            <option id="opt_b" value="banana" selected>Banana</option>
        </select>
        <select id="sel_multi" multiple size="4">
            <optgroup id="og1" label="Citrus">
                <option id="opt_orange" value="orange" selected>Orange</option>
                <option id="opt_lemon" value="lemon">Lemon</option>
            </optgroup>
            <option id="opt_grape" value="grape" selected>Grape</option>
        </select>
    </form>
</body>
</html>"##;

        let dom = mango_html::parse_html(html);
        let mut rt = JsRuntime::new(dom, 1024.0, 768.0);

        let script = r#"
            // 1. Prototype and subclass validation
            var f = document.getElementById("f1");
            if (!f || !(f instanceof HTMLFormElement)) throw new Error("f not HTMLFormElement");
            var fs = document.getElementById("fs1");
            if (!fs || !(fs instanceof HTMLFieldSetElement)) throw new Error("fs not HTMLFieldSetElement");
            var leg = document.getElementById("leg1");
            if (!leg || !(leg instanceof HTMLLegendElement)) throw new Error("leg not HTMLLegendElement");
            var selS = document.getElementById("sel_single");
            if (!selS || !(selS instanceof HTMLSelectElement)) throw new Error("selS not HTMLSelectElement");
            var selM = document.getElementById("sel_multi");
            if (!selM || !(selM instanceof HTMLSelectElement)) throw new Error("selM not HTMLSelectElement");
            var og = document.getElementById("og1");
            if (!og || !(og instanceof HTMLOptGroupElement)) throw new Error("og not HTMLOptGroupElement");
            var optB = document.getElementById("opt_b");
            if (!optB || !(optB instanceof HTMLOptionElement)) throw new Error("optB not HTMLOptionElement");

            // 2. Hierarchy and references (.form, .elements, .length)
            if (leg.form !== f) throw new Error("leg.form should point to enclosing form");
            if (fs.form !== f) throw new Error("fs.form should point to enclosing form");
            var num = document.getElementById("num");
            if (num.form !== f) throw new Error("num.form should point to form");
            if (!f.elements || f.elements.length < 10) throw new Error("f.elements length mismatch: " + (f.elements ? f.elements.length : null));
            if (f.elements['num'] !== num && f.elements.num !== num) throw new Error("f.elements named item access failed");
            if (f.length !== f.elements.length) throw new Error("f.length mismatch");

            // 3. Number input & stepUp / stepDown
            if (num.value !== "20") throw new Error("num initial value mismatch: " + num.value);
            if (!num.checkValidity()) throw new Error("num=20 should be valid");
            num.stepUp(2); // 20 + 2*5 = 30
            if (num.value !== "30") throw new Error("num after stepUp(2) should be 30, got: " + num.value);
            num.stepDown(1); // 30 - 5 = 25
            if (num.value !== "25") throw new Error("num after stepDown(1) should be 25, got: " + num.value);

            // 4. Form validation API: rangeUnderflow, rangeOverflow, stepMismatch
            num.value = "5"; // Below min=10
            if (num.checkValidity()) throw new Error("num=5 should be invalid (min=10)");
            if (!num.validity.rangeUnderflow || num.validity.valid) throw new Error("num.validity.rangeUnderflow expected");

            num.value = "105"; // Above max=100
            if (num.checkValidity()) throw new Error("num=105 should be invalid (max=100)");
            if (!num.validity.rangeOverflow || num.validity.valid) throw new Error("num.validity.rangeOverflow expected");

            num.value = "22"; // Not multiple of step=5 from min=10
            if (num.checkValidity()) throw new Error("num=22 should be invalid (step=5)");
            if (!num.validity.stepMismatch || num.validity.valid) throw new Error("num.validity.stepMismatch expected");

            num.value = "25"; // Valid again
            if (!num.checkValidity() || !num.validity.valid) throw new Error("num=25 should be valid");

            // 5. Email & URL & Pattern validation
            var email = document.getElementById("email");
            if (email.checkValidity() || !email.validity.typeMismatch) throw new Error("bad-email should trigger typeMismatch");
            email.value = "hello@rust-lang.org";
            if (!email.checkValidity() || email.validity.typeMismatch) throw new Error("valid email rejected");

            var url = document.getElementById("url");
            if (url.checkValidity() || !url.validity.typeMismatch) throw new Error("not-a-url should trigger typeMismatch");
            url.value = "https://www.rust-lang.org/";
            if (!url.checkValidity() || url.validity.typeMismatch) throw new Error("valid url rejected");

            var pat = document.getElementById("pat");
            if (pat.checkValidity() || !pat.validity.patternMismatch) throw new Error("pat='abc' should fail [A-Z]{3}");
            pat.value = "XYZ";
            if (!pat.checkValidity() || pat.validity.patternMismatch) throw new Error("pat='XYZ' should match [A-Z]{3}");

            // Custom validity
            pat.setCustomValidity("Three uppercase letters required!");
            if (pat.checkValidity() || !pat.validity.customError) throw new Error("customError flag not set");
            if (pat.validationMessage !== "Three uppercase letters required!") throw new Error("validationMessage mismatch: " + pat.validationMessage);
            pat.setCustomValidity("");
            if (!pat.checkValidity() || pat.validity.customError) throw new Error("customError should be cleared");

            // Hidden element does not participate in constraint validation
            var hid = document.getElementById("hid");
            if (hid.willValidate) throw new Error("hidden input willValidate should be false");

            // 6. Select (single) & Option
            if (selS.multiple !== false) throw new Error("selS.multiple should be false");
            if (selS.type !== "select-one") throw new Error("selS.type should be select-one");
            if (selS.value !== "banana") throw new Error("selS.value should be banana, got: " + selS.value);
            if (selS.selectedIndex !== 1) throw new Error("selS.selectedIndex should be 1, got: " + selS.selectedIndex);
            selS.value = "apple";
            if (selS.value !== "apple") throw new Error("selS.value should be apple");
            if (selS.selectedIndex !== 0) throw new Error("selS.selectedIndex should be 0");

            // 7. Select (multiple) & OptGroup
            if (selM.multiple !== true) throw new Error("selM.multiple should be true");
            if (selM.type !== "select-multiple") throw new Error("selM.type should be select-multiple");
            if (og.label !== "Citrus") throw new Error("og.label mismatch: " + og.label);
            if (!selM.options || selM.options.length !== 3) throw new Error("selM.options length should be 3, got: " + (selM.options ? selM.options.length : null));
            if (!selM.selectedOptions || selM.selectedOptions.length !== 2) throw new Error("selM.selectedOptions length should be 2, got: " + (selM.selectedOptions ? selM.selectedOptions.length : null));

            // Select add & remove
            var newOpt = document.createElement("option");
            newOpt.value = "lime";
            newOpt.text = "Lime";
            selM.add(newOpt);
            if (selM.options.length !== 4) throw new Error("selM.options.length should be 4 after add()");
            selM.remove(3);
            if (selM.options.length !== 3) throw new Error("selM.options.length should be 3 after remove(3)");

            // 8. Form checkValidity & reset
            if (!f.checkValidity()) throw new Error("form should be valid when all controls valid");
            num.value = "5"; // make invalid
            if (f.checkValidity()) throw new Error("form should be invalid when num=5");
            f.reset();
            if (num.value !== "20") throw new Error("f.reset() should restore num to defaultValue 20, got: " + num.value);
        "#;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_form_elements_and_validation failed: {}", e);
        }
    }

    #[test]
    fn test_dom_api_completeness() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Section 5.5 Test</title>
</head>
<body>
    <form id="f1" name="formOne"><input name="user" value="alice"></form>
    <img id="img1" src="pic.png" alt="Pic">
    <a id="l1" href="https://example.com">Link 1</a>
    <a id="l2">Anchor Without Href</a>
    <map name="map1"><area id="ar1" href="https://example.com/area" shape="rect" coords="0,0,10,10"></map>
    <div id="container" data-initial-state="ready" data-count-num="42"><p id="p1">First</p><!-- A comment -->Middle Text<p id="p2">Second</p></div>
</body>
</html>"#;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            // 1. document.documentElement, document.head, document.body
            if (!document.documentElement || document.documentElement.tagName.toLowerCase() !== 'html') {
                throw new Error("document.documentElement failed: " + (document.documentElement ? document.documentElement.tagName : null));
            }
            if (!document.head || document.head.tagName.toLowerCase() !== 'head') {
                throw new Error("document.head failed");
            }
            if (!document.body || document.body.tagName.toLowerCase() !== 'body') {
                throw new Error("document.body failed");
            }

            // 2. document.createTextNode()
            var tn = document.createTextNode("Hello World");
            if (!tn || tn.nodeType !== 3) throw new Error("createTextNode nodeType should be 3");
            if (tn.nodeValue !== "Hello World") throw new Error("createTextNode nodeValue mismatch: " + tn.nodeValue);
            if (tn.textContent !== "Hello World") throw new Error("createTextNode textContent mismatch: " + tn.textContent);

            // 3. document.createDocumentFragment()
            var frag = document.createDocumentFragment();
            if (!frag || frag.nodeType !== 11) throw new Error("createDocumentFragment nodeType should be 11");
            var fChild1 = document.createElement("span");
            fChild1.id = "fc1";
            fChild1.textContent = "Frag 1";
            var fChild2 = document.createElement("span");
            fChild2.id = "fc2";
            fChild2.textContent = "Frag 2";
            frag.appendChild(fChild1);
            frag.appendChild(fChild2);
            if (frag.childNodes.length !== 2) throw new Error("frag childNodes length should be 2");

            // 4. element.children vs element.childNodes
            var container = document.getElementById("container");
            if (!container) throw new Error("container not found");
            // children should be element-only HTMLCollection (p1, p2)
            if (container.children.length !== 2) throw new Error("container.children.length should be 2, got: " + container.children.length);
            if (container.children[0].id !== "p1" || container.children[1].id !== "p2") throw new Error("container.children elements mismatch");
            if (container.children.item(0).id !== "p1") throw new Error("container.children.item(0) mismatch");
            // childNodes contains elements, comments, text nodes
            if (container.childNodes.length !== 4) throw new Error("container.childNodes.length should be 4, got: " + container.childNodes.length);
            if (container.firstChild.id !== "p1") throw new Error("container.firstChild should be p1");
            if (container.lastChild.id !== "p2") throw new Error("container.lastChild should be p2");

            // 5. Node navigation (nextSibling, previousSibling, firstElementChild, etc.)
            var p1 = document.getElementById("p1");
            if (!p1.nextSibling || p1.nextSibling.nodeType !== 8) throw new Error("p1.nextSibling should be Comment (8)");
            if (p1.nextElementSibling.id !== "p2") throw new Error("p1.nextElementSibling should be p2");
            var p2 = document.getElementById("p2");
            if (!p2.previousSibling || p2.previousSibling.nodeType !== 3) throw new Error("p2.previousSibling should be Text (3)");
            if (p2.previousElementSibling.id !== "p1") throw new Error("p2.previousElementSibling should be p1");

            // 6. element.insertBefore() with regular node and DocumentFragment
            var ins1 = document.createElement("b");
            ins1.id = "ins1";
            container.insertBefore(ins1, p2);
            if (ins1.nextSibling !== p2 && ins1.nextElementSibling !== p2) throw new Error("ins1 not inserted before p2");

            // Insert fragment into container before p1
            container.insertBefore(frag, p1);
            if (document.getElementById("fc1") === null || document.getElementById("fc2") === null) {
                throw new Error("fragment children fc1/fc2 not in document");
            }
            if (container.firstElementChild.id !== "fc1") throw new Error("first child after frag insertion should be fc1");
            // Fragment should be emptied per spec
            if (frag.childNodes.length !== 0) throw new Error("frag childNodes should be 0 after insertion, got: " + frag.childNodes.length);

            // 7. node.replaceChild()
            var repNew = document.createElement("i");
            repNew.id = "repNew";
            var oldChild = container.replaceChild(repNew, ins1);
            if (oldChild.id !== "ins1") throw new Error("replaceChild return value should be oldChild");
            if (document.getElementById("ins1") !== null) throw new Error("oldChild should no longer be in document");
            if (document.getElementById("repNew") === null) throw new Error("repNew should be in document");

            // replaceChild with DocumentFragment
            var repFrag = document.createDocumentFragment();
            var rf1 = document.createElement("u"); rf1.id = "rf1";
            var rf2 = document.createElement("u"); rf2.id = "rf2";
            repFrag.appendChild(rf1);
            repFrag.appendChild(rf2);
            container.replaceChild(repFrag, repNew);
            if (document.getElementById("repNew") !== null) throw new Error("repNew should be replaced");
            if (!document.getElementById("rf1") || !document.getElementById("rf2")) throw new Error("repFrag children missing");

            // 8. element.insertAdjacentElement()
            var target = document.getElementById("rf1");
            var bb = document.createElement("span"); bb.id = "adj_bb";
            var ab = document.createElement("span"); ab.id = "adj_ab";
            var be = document.createElement("span"); be.id = "adj_be";
            var ae = document.createElement("span"); ae.id = "adj_ae";

            target.insertAdjacentElement("beforebegin", bb);
            target.insertAdjacentElement("afterbegin", ab);
            target.insertAdjacentElement("beforeend", be);
            target.insertAdjacentElement("afterend", ae);

            if (target.previousElementSibling.id !== "adj_bb") throw new Error("insertAdjacentElement beforebegin failed");
            if (target.firstElementChild.id !== "adj_ab") throw new Error("insertAdjacentElement afterbegin failed");
            if (target.lastElementChild.id !== "adj_be") throw new Error("insertAdjacentElement beforeend failed");
            if (target.nextElementSibling.id !== "adj_ae") throw new Error("insertAdjacentElement afterend failed");

            // 9. element.insertAdjacentHTML()
            target.insertAdjacentHTML("beforebegin", "<em id='html_bb'>HBB</em>");
            target.insertAdjacentHTML("afterbegin", "<em id='html_ab'>HAB</em>");
            target.insertAdjacentHTML("beforeend", "<em id='html_be'>HBE</em>");
            target.insertAdjacentHTML("afterend", "<em id='html_ae'>HAE</em>");

            if (!document.getElementById("html_bb") || !document.getElementById("html_ab") ||
                !document.getElementById("html_be") || !document.getElementById("html_ae")) {
                throw new Error("insertAdjacentHTML elements missing");
            }

            // 10. element.innerHTML (get)
            var p1Html = p1.innerHTML;
            if (p1Html !== "First") throw new Error("p1.innerHTML get mismatch: " + p1Html);

            // 11. element.outerHTML (get & set)
            var p1Outer = p1.outerHTML;
            if (p1Outer.indexOf('<p id="p1">First</p>') === -1 && p1Outer.indexOf("<p id='p1'>First</p>") === -1) {
                throw new Error("p1.outerHTML get mismatch: " + p1Outer);
            }
            // Set outerHTML and verify in-place replacement
            p1.outerHTML = "<section id='sec1'>Section 1</section>";
            if (document.getElementById("p1") !== null) throw new Error("p1 should be removed by outerHTML setter");
            var sec1 = document.getElementById("sec1");
            if (!sec1 || sec1.textContent !== "Section 1") throw new Error("sec1 not found or text mismatch after outerHTML setter");

            // 12. element.dataset
            if (container.dataset.initialState !== "ready") throw new Error("dataset get initial-state failed: " + container.dataset.initialState);
            if (container.dataset.countNum !== "42") throw new Error("dataset get count-num failed: " + container.dataset.countNum);
            container.dataset.newProp = "testValue";
            if (container.getAttribute("data-new-prop") !== "testValue") throw new Error("dataset set new-prop failed: " + container.getAttribute("data-new-prop"));
            if (!('newProp' in container.dataset)) throw new Error("'newProp' in dataset should be true");
            delete container.dataset.newProp;
            if (container.hasAttribute("data-new-prop")) throw new Error("delete dataset property should remove attribute");

            // 13. NodeList / HTMLCollection live collections
            var pTags = document.getElementsByTagName("p");
            var initialPLength = pTags.length;
            var newP = document.createElement("p");
            document.body.appendChild(newP);
            if (pTags.length !== initialPLength + 1) throw new Error("getElementsByTagName should be live: " + pTags.length + " vs " + (initialPLength + 1));
            // Iteration
            var countIter = 0;
            for (var pItem of pTags) {
                if (pItem && pItem.tagName.toLowerCase() === 'p') countIter++;
            }
            if (countIter !== pTags.length) throw new Error("HTMLCollection iteration count mismatch");

            // 14. document.forms, document.images, document.links
            if (!document.forms || document.forms.length !== 1 || document.forms[0].id !== "f1") {
                throw new Error("document.forms mismatch: " + (document.forms ? document.forms.length : null));
            }
            if (document.forms.namedItem("formOne") !== document.forms[0]) {
                throw new Error("document.forms.namedItem mismatch");
            }
            if (!document.images || document.images.length !== 1 || document.images[0].id !== "img1") {
                throw new Error("document.images mismatch: " + (document.images ? document.images.length : null));
            }
            // document.links: only <a> and <area> WITH href (l1 and ar1, NOT l2)
            if (!document.links || document.links.length !== 2) {
                throw new Error("document.links length should be 2 (l1 and ar1), got: " + (document.links ? document.links.length : null));
            }
            var linkIds = [document.links[0].id, document.links[1].id];
            if (linkIds.indexOf("l1") === -1 || linkIds.indexOf("ar1") === -1 || linkIds.indexOf("l2") !== -1) {
                throw new Error("document.links items incorrect: " + JSON.stringify(linkIds));
            }

            // 15. node.cloneNode(deep)
            var cloneShallow = container.cloneNode(false);
            if (!cloneShallow || cloneShallow.id !== "container") throw new Error("cloneNode(false) id mismatch");
            if (cloneShallow.childNodes.length !== 0) throw new Error("cloneNode(false) should have no children, got: " + cloneShallow.childNodes.length);

            var cloneDeep = container.cloneNode(true);
            if (!cloneDeep || cloneDeep.childNodes.length === 0) throw new Error("cloneNode(true) should have children");
            if (cloneDeep.querySelector("#sec1") === null) throw new Error("cloneNode(true) subtree missing #sec1");

            // Clone <template> with content
            var tmpl = document.createElement("template");
            var spanInside = document.createElement("span");
            spanInside.id = "insideTemplate";
            tmpl.content.appendChild(spanInside);
            var tmplCloned = tmpl.cloneNode(true);
            if (!tmplCloned.content || tmplCloned.content.childNodes.length !== 1) {
                throw new Error("tmpl.cloneNode(true) content not cloned properly");
            }
            var qRes = tmplCloned.content.querySelector("#insideTemplate");
            if (!qRes || qRes.id !== "insideTemplate") {
                throw new Error("tmpl.cloneNode(true) cloned content missing querySelector #insideTemplate");
            }
        "##;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_dom_api_completeness failed: {}", e);
        }
    }

    #[test]
    fn test_selectors_level_4_dom_bindings() {
        let html = r##"<!DOCTYPE html>
<html>
<head><title>Selectors 4 Test</title></head>
<body>
  <div id="target-elem" data-mango-target="true">Target Section</div>
  <div id="card1" class="card">
    <h2 class="title">Heading 1</h2>
    <p class="desc">First description</p>
    <span class="badge">New</span>
  </div>
  <div id="card2" class="card">
    <h2 class="title">Heading 2</h2>
    <!-- Card with empty box -->
    <div id="empty-box">   <!-- comment --> </div>
  </div>

  <ul id="items">
    <li class="item featured" id="it1">Item 1</li>
    <li class="item" id="it2">Item 2</li>
    <li class="item featured" id="it3">Item 3</li>
    <li class="item" id="it4">Item 4</li>
    <li class="item featured" id="it5">Item 5</li>
  </ul>

  <div id="solo-box">
    <span id="only-span">Only child span</span>
  </div>

  <form id="test-form">
    <input type="text" id="uname" required placeholder="Your name" value="">
    <input type="email" id="uemail" required value="invalid-email">
    <input type="text" id="filled" placeholder="Nickname" value="mango_user">
    <input type="checkbox" id="chk" checked>
    <input type="text" id="dis" disabled value="locked">
    <input type="text" id="ro" readonly value="read only text">
  </form>
</body>
</html>"##;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            // 1. :is() and :where()
            var h = document.querySelector(":is(.card, .box) .title");
            if (!h || h.textContent !== "Heading 1") throw new Error(":is() querySelector failed");

            var w = document.querySelector(":where(#card1, #card2) .desc");
            if (!w || w.textContent !== "First description") throw new Error(":where() querySelector failed");

            // 2. :not() with complex/compound selector
            var nonFeatured = document.querySelectorAll("ul#items > li:not(.featured)");
            if (nonFeatured.length !== 2) throw new Error(":not(.featured) length: " + nonFeatured.length);
            if (nonFeatured[0].id !== "it2" || nonFeatured[1].id !== "it4") {
                throw new Error(":not(.featured) elements incorrect");
            }

            // 3. :has() relative child, sibling, and descendant
            var cardWithBadge = document.querySelector(".card:has(> span.badge)");
            if (!cardWithBadge || cardWithBadge.id !== "card1") throw new Error(":has(> span.badge) failed");

            var headingWithNextP = document.querySelector("h2:has(+ p.desc)");
            if (!headingWithNextP || headingWithNextP.textContent !== "Heading 1") {
                throw new Error("h2:has(+ p.desc) failed");
            }

            var cardWithoutP = document.querySelector(".card:has(#empty-box)");
            if (!cardWithoutP || cardWithoutP.id !== "card2") throw new Error(".card:has(#empty-box) failed");

            // 4. :nth-child(An+B of S)
            var secondFeatured = document.querySelector("li:nth-child(2 of .featured)");
            if (!secondFeatured || secondFeatured.id !== "it3") throw new Error("nth-child(2 of .featured) failed: " + (secondFeatured ? secondFeatured.id : "null"));

            var lastFeatured = document.querySelector("li:nth-last-child(1 of .featured)");
            if (!lastFeatured || lastFeatured.id !== "it5") throw new Error("nth-last-child(1 of .featured) failed: " + (lastFeatured ? lastFeatured.id : "null"));

            // 5. :only-child, :first-of-type, :last-of-type
            var onlySpan = document.querySelector("#solo-box > span:only-child");
            if (!onlySpan || onlySpan.id !== "only-span") throw new Error(":only-child failed");

            var firstLi = document.querySelector("#items > li:first-of-type");
            if (!firstLi || firstLi.id !== "it1") throw new Error(":first-of-type failed");

            // 6. :empty
            var emptyBox = document.querySelector("#empty-box:empty");
            if (!emptyBox) throw new Error(":empty failed for whitespace/comment-only node");

            // 7. :target
            var targetElem = document.querySelector("#target-elem:target");
            if (!targetElem) throw new Error(":target failed");

            // 8. :placeholder-shown
            var emptyInput = document.querySelector("input:placeholder-shown");
            if (!emptyInput || emptyInput.id !== "uname") throw new Error(":placeholder-shown failed: " + (emptyInput ? emptyInput.id : "null"));

            // 9. :checked, :disabled, :enabled
            var chk = document.querySelector("input:checked");
            if (!chk || chk.id !== "chk") throw new Error(":checked failed");

            var dis = document.querySelector("input:disabled");
            if (!dis || dis.id !== "dis") throw new Error(":disabled failed");

            var en = document.querySelectorAll("form input:enabled");
            if (en.length !== 5) throw new Error(":enabled count: " + en.length);

            // 10. :required, :optional, :invalid
            var req = document.querySelectorAll("input:required");
            if (req.length !== 2) throw new Error(":required count: " + req.length);

            var opt = document.querySelectorAll("form input:optional");
            if (opt.length !== 4) throw new Error(":optional count: " + opt.length);

            var inv = document.querySelectorAll("input:invalid");
            if (inv.length !== 2) throw new Error(":invalid count: " + inv.length);
            if (inv[0].id !== "uname" || inv[1].id !== "uemail") throw new Error(":invalid elements incorrect");

            // 11. :read-only, :read-write
            var ro = document.querySelectorAll("form input:read-only");
            if (ro.length !== 3) throw new Error("input:read-only count: " + ro.length);
            if (ro[0].id !== "chk" || ro[1].id !== "dis" || ro[2].id !== "ro") {
                throw new Error("input:read-only elements incorrect: " + ro[0].id + ", " + ro[1].id + ", " + ro[2].id);
            }

            var rw = document.querySelectorAll("form input:read-write");
            if (rw.length !== 3) throw new Error("input:read-write count: " + rw.length);
            if (rw[0].id !== "uname" || rw[1].id !== "uemail" || rw[2].id !== "filled") {
                throw new Error("input:read-write elements incorrect");
            }

            // 12. element.matches()
            if (!cardWithBadge.matches(".card:has(.badge)")) throw new Error("element.matches(:has) failed");
            if (!secondFeatured.matches(":nth-child(2 of .featured)")) throw new Error("element.matches(nth-child of S) failed");
            if (cardWithBadge.matches(":empty")) throw new Error("cardWithBadge should not match :empty");

            // 13. element.closest()
            var desc = document.querySelector(".desc");
            var foundCard = desc.closest(".card:has(.badge)");
            if (!foundCard || foundCard.id !== "card1") throw new Error("element.closest(:has) failed");
        "##;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_selectors_level_4_dom_bindings failed: {}", e);
        }
    }

    #[test]
    fn test_core_js_engine() {
        let doc = parse_html("<!DOCTYPE html><html><body><div id='out'>initial</div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            // 1. Microtask draining & Promise jobs
            var promiseRan = false;
            Promise.resolve().then(function() {
                promiseRan = true;
                var el = document.getElementById("out");
                if (el) el.textContent = "promise-resolved";
            });

            // 2. queueMicrotask execution
            var microtaskRan = false;
            queueMicrotask(function() {
                microtaskRan = true;
            });

            // 3. Intl.NumberFormat - Standard & Compact Notation (YouTube view counts)
            var nf = new Intl.NumberFormat("en-US");
            if (nf.format(1000) !== "1,000") throw new Error("NumberFormat 1000: " + nf.format(1000));

            var compactNf = new Intl.NumberFormat("en-US", { notation: "compact", compactDisplay: "short" });
            var millionFormatted = compactNf.format(1200000);
            if (millionFormatted.indexOf("M") === -1 && millionFormatted.indexOf("1.2") === -1) {
                throw new Error("Compact notation failed: " + millionFormatted);
            }
            var thousandFormatted = compactNf.format(45000);
            if (thousandFormatted.indexOf("K") === -1 && thousandFormatted.indexOf("45") === -1) {
                throw new Error("Compact notation 45K failed: " + thousandFormatted);
            }

            // Currency formatting
            var currNf = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });
            var currFormatted = currNf.format(100);
            if (currFormatted.indexOf("$") === -1 && currFormatted.indexOf("100") === -1) {
                throw new Error("Currency formatting failed: " + currFormatted);
            }

            // Percent formatting
            var pctNf = new Intl.NumberFormat("en-US", { style: "percent" });
            if (pctNf.format(0.75).indexOf("%") === -1) {
                throw new Error("Percent formatting failed: " + pctNf.format(0.75));
            }

            // 4. Intl.DateTimeFormat
            var dtf = new Intl.DateTimeFormat("en-US");
            var dateStr = dtf.format(new Date(1700000000000));
            if (!dateStr || dateStr.length === 0) throw new Error("DateTimeFormat format failed");
            var parts = dtf.formatToParts(new Date(1700000000000));
            if (!Array.isArray(parts) || parts.length === 0) throw new Error("formatToParts failed");
            var rangeStr = dtf.formatRange(new Date(1700000000000), new Date(1700100000000));
            if (!rangeStr || rangeStr.length === 0) throw new Error("formatRange failed");

            // 5. Intl.Collator
            var collator = new Intl.Collator("en");
            if (collator.compare("a", "b") >= 0) throw new Error("Collator a vs b failed");
            if (collator.compare("b", "a") <= 0) throw new Error("Collator b vs a failed");
            if (collator.compare("a", "a") !== 0) throw new Error("Collator a vs a failed");

            // 6. Intl.PluralRules
            var pr = new Intl.PluralRules("en-US");
            if (pr.select(1) !== "one") throw new Error("PluralRules 1: " + pr.select(1));
            if (pr.select(5) !== "other") throw new Error("PluralRules 5: " + pr.select(5));

            // 7. Intl.RelativeTimeFormat
            var rtf = new Intl.RelativeTimeFormat("en");
            var rtfYesterday = rtf.format(-1, "day");
            if (rtfYesterday.indexOf("day") === -1 && rtfYesterday.indexOf("ago") === -1 && rtfYesterday.indexOf("yesterday") === -1) {
                throw new Error("RelativeTimeFormat failed: " + rtfYesterday);
            }

            // 8. Intl.DisplayNames
            var dn = new Intl.DisplayNames(["en"], { type: "language" });
            if (dn.of("en") !== "English") throw new Error("DisplayNames en: " + dn.of("en"));

            // 9. Intl.ListFormat
            var lf = new Intl.ListFormat("en", { style: "long", type: "conjunction" });
            var listFormatted = lf.format(["Motorcycle", "Bus", "Car"]);
            if (listFormatted.indexOf("Motorcycle") === -1 || listFormatted.indexOf("Car") === -1) {
                throw new Error("ListFormat failed: " + listFormatted);
            }

            // 10. Intl.Segmenter
            var seg = new Intl.Segmenter("en", { granularity: "word" });
            var segments = seg.segment("Hello World");
            if (typeof segments[Symbol.iterator] !== "function") throw new Error("Segmenter iterator missing");
        "##;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_core_js_engine failed: {}", e);
        }

        // Verify that Boa microtasks drained synchronously via execute_script's context.run_jobs()
        let check_script = r#"
            if (!promiseRan) throw new Error("Promise microtask did not drain");
            if (!microtaskRan) throw new Error("queueMicrotask did not drain");
            var el = document.getElementById("out");
            if (!el || el.textContent !== "promise-resolved") throw new Error("DOM mutation from microtask missing");
        "#;
        if let Err(e) = rt.execute_script(check_script) {
            panic!("test_core_js_engine verification failed: {}", e);
        }
    }

    #[test]
    fn test_dom_events() {
        let html = r#"<!DOCTYPE html>
        <html>
        <body>
            <div id="parent">
                <button id="btn">Click me</button>
            </div>
            <form id="myform">
                <input id="myinput" type="text" value="hello" />
            </form>
        </body>
        </html>"#;

        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let setup_script = r##"
            window.log = [];

            // Window capture & bubble
            window.addEventListener("click", function(e) {
                window.log.push("window-capture:phase=" + e.eventPhase);
            }, true);
            window.addEventListener("click", function(e) {
                window.log.push("window-bubble:phase=" + e.eventPhase);
            }, false);

            // Document capture & bubble
            document.addEventListener("click", function(e) {
                window.log.push("document-capture:phase=" + e.eventPhase);
            }, { capture: true });
            document.addEventListener("click", function(e) {
                window.log.push("document-bubble:phase=" + e.eventPhase);
            }, { capture: false });

            // Parent element capture & bubble
            var parent = document.getElementById("parent");
            parent.addEventListener("click", function(e) {
                window.log.push("parent-capture:phase=" + e.eventPhase);
            }, true);
            parent.addEventListener("click", function(e) {
                window.log.push("parent-bubble:phase=" + e.eventPhase);
            }, false);

            // Button target listener & composedPath
            var btn = document.getElementById("btn");
            btn.addEventListener("click", function(e) {
                window.log.push("btn-target:phase=" + e.eventPhase);
                var path = e.composedPath();
                if (!Array.isArray(path) || path.length < 3) {
                    throw new Error("composedPath invalid: " + path.length);
                }
                if (path[0] !== btn) throw new Error("composedPath[0] is not btn");
            });

            // once: true test
            window.onceCount = 0;
            btn.addEventListener("click", function() {
                window.onceCount++;
            }, { once: true });

            // passive: true test
            window.passiveTriedPrevent = false;
            btn.addEventListener("click", function(e) {
                e.preventDefault();
                window.passiveDefaultPrevented = e.defaultPrevented;
            }, { passive: true });

            // AbortSignal test
            var ac = new AbortController();
            window.abortedFired = false;
            btn.addEventListener("click", function() {
                window.abortedFired = true;
            }, { signal: ac.signal });
            ac.abort(); // Immediately remove listener before click
        "##;

        if let Err(e) = rt.execute_script(setup_script) {
            panic!("setup_script failed: {}", e);
        }

        // Find button node id
        let btn_id = {
            let doc = rt.document_ref();
            let root = doc.root();
            doc.find_element_by_tag(root, "button").expect("button node")
        };

        // Dispatch click from host
        let default_allowed = rt.dispatch_click(
            Some(btn_id),
            120.0,
            240.0,
            0,
            1,
            EventModifiers::NONE,
        ).expect("dispatch_click");
        assert!(default_allowed);

        // Verify propagation sequence and options
        let verify_script = r##"
            // Expected 3-phase propagation order:
            // 1. window-capture (phase 1)
            // 2. document-capture (phase 1)
            // 3. parent-capture (phase 1)
            // 4. btn-target (phase 2)
            // 5. parent-bubble (phase 3)
            // 6. document-bubble (phase 3)
            // 7. window-bubble (phase 3)
            var expected = [
                "window-capture:phase=1",
                "document-capture:phase=1",
                "parent-capture:phase=1",
                "btn-target:phase=2",
                "parent-bubble:phase=3",
                "document-bubble:phase=3",
                "window-bubble:phase=3"
            ];
            if (window.log.length !== expected.length) {
                throw new Error("Log length: " + window.log.length + " vs expected " + expected.length + ": " + window.log.join(", "));
            }
            for (var i = 0; i < expected.length; i++) {
                if (window.log[i] !== expected[i]) {
                    throw new Error("Step " + i + ": got " + window.log[i] + " expected " + expected[i]);
                }
            }

            // once: true check (fired once)
            if (window.onceCount !== 1) throw new Error("onceCount: " + window.onceCount);

            // passive: true check (defaultPrevented remained false)
            if (window.passiveDefaultPrevented !== false) {
                throw new Error("passive listener failed to ignore preventDefault");
            }

            // AbortSignal check (did not fire)
            if (window.abortedFired !== false) throw new Error("Aborted listener fired!");
        "##;

        if let Err(e) = rt.execute_script(verify_script) {
            panic!("verify_script failed: {}", e);
        }

        // Test second click: once listener should NOT fire again
        rt.dispatch_click(Some(btn_id), 120.0, 240.0, 0, 1, EventModifiers::NONE).unwrap();
        let check_once2 = "if (window.onceCount !== 1) throw new Error('once listener fired twice');";
        rt.execute_script(check_once2).unwrap();

        // Test stopPropagation
        let stop_script = r##"
            window.stopLog = [];
            var p = document.getElementById("parent");
            p.addEventListener("custom-test", function(e) {
                window.stopLog.push("parent-capture");
                e.stopPropagation();
            }, true);
            var b = document.getElementById("btn");
            b.addEventListener("custom-test", function(e) {
                window.stopLog.push("btn-target");
            });
            var ev = new CustomEvent("custom-test", { bubbles: true, cancelable: true });
            b.dispatchEvent(ev);
            if (window.stopLog.length !== 1 || window.stopLog[0] !== "parent-capture") {
                throw new Error("stopPropagation failed: " + window.stopLog.join(", "));
            }
        "##;
        rt.execute_script(stop_script).unwrap();

        // Test stopImmediatePropagation
        let imm_script = r##"
            window.immLog = [];
            var b = document.getElementById("btn");
            b.addEventListener("imm-test", function(e) {
                window.immLog.push("l1");
                e.stopImmediatePropagation();
            });
            b.addEventListener("imm-test", function(e) {
                window.immLog.push("l2");
            });
            b.dispatchEvent(new Event("imm-test"));
            if (window.immLog.length !== 1 || window.immLog[0] !== "l1") {
                throw new Error("stopImmediatePropagation failed: " + window.immLog.join(", "));
            }
        "##;
        rt.execute_script(imm_script).unwrap();

        // Test host event dispatch methods for input, change, submit, resize, scroll, domcontentloaded, load
        let input_id = {
            let doc = rt.document_ref();
            let root = doc.root();
            doc.find_element_by_tag(root, "input").expect("input node")
        };
        let form_id = {
            let doc = rt.document_ref();
            let root = doc.root();
            doc.find_element_by_tag(root, "form").expect("form node")
        };

        let host_listener_script = r##"
            window.eventsReceived = {};
            document.getElementById("myinput").addEventListener("input", function(e) {
                window.eventsReceived["input"] = e.data;
            });
            document.getElementById("myinput").addEventListener("change", function(e) {
                window.eventsReceived["change"] = true;
            });
            document.getElementById("myform").addEventListener("submit", function(e) {
                window.eventsReceived["submit"] = true;
                e.preventDefault(); // cancel form submit
            });
            document.addEventListener("DOMContentLoaded", function(e) {
                window.eventsReceived["DOMContentLoaded"] = true;
            });
            window.addEventListener("load", function(e) {
                window.eventsReceived["load"] = true;
            });
            window.addEventListener("resize", function(e) {
                window.eventsReceived["resize"] = window.innerWidth + "x" + window.innerHeight;
            });
            window.addEventListener("scroll", function(e) {
                window.eventsReceived["scroll"] = window.scrollX + "," + window.scrollY;
            });
        "##;
        rt.execute_script(host_listener_script).unwrap();

        rt.dispatch_input(input_id, Some("world")).unwrap();
        rt.dispatch_change(input_id).unwrap();
        let submit_allowed = rt.dispatch_submit(form_id).unwrap();
        assert!(!submit_allowed, "submit was preventDefault'd");
        rt.dispatch_dom_content_loaded().unwrap();
        rt.dispatch_load().unwrap();
        rt.dispatch_window_resize(1280.0, 720.0).unwrap();
        rt.dispatch_window_scroll(0.0, 300.0).unwrap();

        let check_host_events = r##"
            if (window.eventsReceived["input"] !== "world") throw new Error("input event missing");
            if (!window.eventsReceived["change"]) throw new Error("change event missing");
            if (!window.eventsReceived["submit"]) throw new Error("submit event missing");
            if (!window.eventsReceived["DOMContentLoaded"]) throw new Error("DOMContentLoaded missing");
            if (!window.eventsReceived["load"]) throw new Error("load missing");
            if (window.eventsReceived["resize"] !== "1280x720") throw new Error("resize missing: " + window.eventsReceived["resize"]);
            if (window.eventsReceived["scroll"] !== "0,300") throw new Error("scroll missing: " + window.eventsReceived["scroll"]);
        "##;
        rt.execute_script(check_host_events).unwrap();

        // Test hover and active state management
        assert!(rt.set_hover_state(Some(btn_id)));
        assert!(rt.is_dom_dirty());
        rt.clear_dom_dirty();

        // Verify button and parent both have data-mango-hover="true"
        assert_eq!(rt.get_node_attribute(btn_id, "data-mango-hover"), Some("true".to_string()));
        let parent_id = {
            let doc = rt.document_ref();
            let root = doc.root();
            doc.find_element_by_tag(root, "div").expect("parent div node")
        };
        assert_eq!(rt.get_node_attribute(parent_id, "data-mango-hover"), Some("true".to_string()));

        // Unhover
        assert!(rt.set_hover_state(None));
        assert_eq!(rt.get_node_attribute(btn_id, "data-mango-hover"), None);
        assert_eq!(rt.get_node_attribute(parent_id, "data-mango-hover"), None);

        // Test active state
        assert!(rt.set_active_state(Some(btn_id)));
        assert_eq!(rt.get_node_attribute(btn_id, "data-mango-active"), Some("true".to_string()));
        assert_eq!(rt.get_node_attribute(parent_id, "data-mango-active"), Some("true".to_string()));
        assert!(rt.set_active_state(None));
        assert_eq!(rt.get_node_attribute(btn_id, "data-mango-active"), None);
        assert_eq!(rt.get_node_attribute(parent_id, "data-mango-active"), None);
    }

    #[test]
    fn test_web_apis() {
        let doc = parse_html("<html><body><div id='root'></div></body></html>");
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            // 1. URL and URLSearchParams
            var params = new URLSearchParams("a=1&b=2&a=3");
            if (params.get("a") !== "1") throw new Error("params.get failed: " + params.get("a"));
            var allA = params.getAll("a");
            if (allA.length !== 2 || allA[0] !== "1" || allA[1] !== "3") throw new Error("params.getAll failed");
            if (!params.has("b") || params.has("c")) throw new Error("params.has failed");
            params.set("b", "4");
            if (params.get("b") !== "4") throw new Error("params.set failed");
            params.append("c", "5");
            if (params.get("c") !== "5") throw new Error("params.append failed");
            params.delete("a");
            if (params.has("a")) throw new Error("params.delete failed");
            params.set("z", "9");
            params.set("a", "1");
            params.sort();
            var sortedKeys = Array.from(params.keys());
            if (sortedKeys[0] !== "a") throw new Error("params.sort failed");

            // URL constructor and properties
            var url = new URL("https://example.com:8080/path/test?q=search&lang=en#heading");
            if (url.protocol !== "https:") throw new Error("url.protocol mismatch: " + url.protocol);
            if (url.hostname !== "example.com") throw new Error("url.hostname mismatch: " + url.hostname);
            if (url.port !== "8080") throw new Error("url.port mismatch: " + url.port);
            if (url.host !== "example.com:8080") throw new Error("url.host mismatch: " + url.host);
            if (url.pathname !== "/path/test") throw new Error("url.pathname mismatch: " + url.pathname);
            if (url.search !== "?q=search&lang=en") throw new Error("url.search mismatch: " + url.search);
            if (url.hash !== "#heading") throw new Error("url.hash mismatch: " + url.hash);
            if (url.origin !== "https://example.com:8080") throw new Error("url.origin mismatch: " + url.origin);
            if (url.searchParams.get("q") !== "search") throw new Error("url.searchParams mismatch");

            // URL relative resolution
            var relUrl = new URL("../sub/page", "https://example.com/path/test");
            if (relUrl.pathname !== "/sub/page") throw new Error("URL relative resolution failed: " + relUrl.pathname);

            // URL static methods
            if (!URL.canParse("https://example.com")) throw new Error("URL.canParse should be true for valid URL");
            if (URL.canParse("not a url")) throw new Error("URL.canParse should be false for invalid URL");
            var blobObj = new Blob(["test"], { type: "text/plain" });
            var blobUrl = URL.createObjectURL(blobObj);
            if (blobUrl.indexOf("blob:") !== 0) throw new Error("createObjectURL failed: " + blobUrl);
            URL.revokeObjectURL(blobUrl);

            // 2. FormData
            var fd = new FormData();
            fd.append("name", "mango");
            fd.append("item", "1");
            fd.append("item", "2");
            if (fd.get("name") !== "mango") throw new Error("fd.get failed");
            var items = fd.getAll("item");
            if (items.length !== 2 || items[0] !== "1" || items[1] !== "2") throw new Error("fd.getAll failed");
            if (!fd.has("name") || fd.has("missing")) throw new Error("fd.has failed");
            fd.set("item", "single");
            if (fd.getAll("item").length !== 1 || fd.get("item") !== "single") throw new Error("fd.set failed");
            fd.delete("name");
            if (fd.has("name")) throw new Error("fd.delete failed");

            // 3. Blob, File, FileReader
            var blob = new Blob(["Hello", " ", "World"], { type: "text/plain" });
            if (blob.size !== 11) throw new Error("blob.size mismatch: " + blob.size);
            if (blob.type !== "text/plain") throw new Error("blob.type mismatch: " + blob.type);
            var sliced = blob.slice(0, 5);
            if (sliced.size !== 5) throw new Error("blob.slice size mismatch: " + sliced.size);

            var file = new File(["test file content"], "sample.txt", { type: "text/plain", lastModified: 1700000000000 });
            if (file.name !== "sample.txt") throw new Error("file.name mismatch: " + file.name);
            if (file.size !== 17) throw new Error("file.size mismatch: " + file.size);
            if (file.lastModified !== 1700000000000) throw new Error("file.lastModified mismatch: " + file.lastModified);

            window.readerResult = null;
            var reader = new FileReader();
            reader.onload = function(e) {
                window.readerResult = e.target.result;
            };
            reader.readAsText(blob);

            // 4. AbortController & AbortSignal
            var ac = new AbortController();
            if (ac.signal.aborted) throw new Error("signal should initially not be aborted");
            window.abortEventFired = false;
            ac.signal.addEventListener("abort", function() {
                window.abortEventFired = true;
            });
            ac.abort("CustomCancel");
            if (!ac.signal.aborted) throw new Error("signal should be aborted after ac.abort()");
            if (ac.signal.reason !== "CustomCancel") throw new Error("signal.reason mismatch: " + ac.signal.reason);
            if (!window.abortEventFired) throw new Error("abort event did not fire");

            var instantAbort = AbortSignal.abort("instant");
            if (!instantAbort.aborted || instantAbort.reason !== "instant") throw new Error("AbortSignal.abort failed");

            // 5. TextEncoder & TextDecoder
            var encoder = new TextEncoder();
            if (encoder.encoding !== "utf-8") throw new Error("encoder.encoding should be utf-8");
            var encoded = encoder.encode("Hello \u2764");
            if (!(encoded instanceof Uint8Array)) throw new Error("encoded should be Uint8Array");

            var dest = new Uint8Array(10);
            var res = encoder.encodeInto("Hi", dest);
            if (res.read !== 2 || res.written !== 2 || dest[0] !== 72 || dest[1] !== 105) {
                throw new Error("encodeInto failed: " + JSON.stringify(res));
            }

            var decoder = new TextDecoder();
            if (decoder.encoding !== "utf-8") throw new Error("decoder.encoding should be utf-8");
            var decoded = decoder.decode(encoded);
            if (decoded !== "Hello \u2764") throw new Error("decoder.decode failed: " + decoded);

            // 6. crypto.getRandomValues, crypto.randomUUID, crypto.subtle
            if (!crypto || !crypto.subtle) throw new Error("crypto or crypto.subtle missing");
            var randArr = new Uint8Array(16);
            crypto.getRandomValues(randArr);
            var anyNonZero = false;
            for (var i = 0; i < randArr.length; i++) {
                if (randArr[i] !== 0) anyNonZero = true;
            }
            if (!anyNonZero) throw new Error("getRandomValues returned all zeros");

            var uuid = crypto.randomUUID();
            if (typeof uuid !== "string" || uuid.length !== 36 || uuid[14] !== '4') {
                throw new Error("randomUUID format invalid: " + uuid);
            }

            // 7. btoa & atob
            var b64 = btoa("Hello World!");
            if (b64 !== "SGVsbG8gV29ybGQh") throw new Error("btoa failed: " + b64);
            var origStr = atob(b64);
            if (origStr !== "Hello World!") throw new Error("atob failed: " + origStr);

            // 8. structuredClone
            var original = {
                num: 42,
                str: "mango",
                flag: true,
                date: new Date(1700000000000),
                regex: /test/gi,
                arr: [1, 2, { nested: "yes" }],
                u8: new Uint8Array([10, 20, 30])
            };
            // Add cycle
            original.self = original;
            var cloned = structuredClone(original);
            if (cloned.num !== 42 || cloned.str !== "mango" || cloned.flag !== true) throw new Error("cloned primitives mismatch");
            if (cloned === original) throw new Error("cloned object should not be identical reference");
            if (cloned.date.getTime() !== 1700000000000) throw new Error("cloned Date mismatch");
            if (cloned.regex.source !== "test" || !cloned.regex.global || !cloned.regex.ignoreCase) throw new Error("cloned RegExp mismatch");
            if (cloned.arr[2].nested !== "yes") throw new Error("cloned nested array mismatch");
            if (cloned.self !== cloned) throw new Error("cloned circular reference failed");
            if (cloned.u8[1] !== 20 || cloned.u8 === original.u8) throw new Error("cloned TypedArray mismatch");

            // 9. fetch, Headers, Request, Response
            var headers = new Headers({ "Content-Type": "text/html", "X-Custom": "val" });
            if (headers.get("content-type") !== "text/html") throw new Error("headers.get failed");
            headers.append("Accept", "text/plain");
            if (!headers.has("accept")) throw new Error("headers.has failed");
            var headerKeys = Array.from(headers.keys());
            if (headerKeys.indexOf("x-custom") === -1) throw new Error("headers.keys failed");

            var req = new Request("https://example.com/api", {
                method: "POST",
                headers: { "X-Api-Key": "secret" },
                body: "payload"
            });
            if (req.method !== "POST") throw new Error("req.method mismatch: " + req.method);
            if (req.url !== "https://example.com/api") throw new Error("req.url mismatch: " + req.url);
            if (req.headers.get("x-api-key") !== "secret") throw new Error("req.headers mismatch");
            if (req.mode !== "cors") throw new Error("req.mode default mismatch: " + req.mode);
            if (req.credentials !== "same-origin") throw new Error("req.credentials default mismatch: " + req.credentials);

            // SOP test via _mangoFetch: cross-origin in same-origin mode must be blocked
            var sopRes = JSON.parse(_mangoFetch("https://api.other.com/data", "GET", "{}", "", "https://example.com", "same-origin", false));
            if (!sopRes.error || sopRes.error.indexOf("Same-Origin Policy") === -1) {
                throw new Error("SOP check failed to block cross-origin request: " + JSON.stringify(sopRes));
            }

            var res = new Response("Test body", {
                status: 200,
                statusText: "OK",
                headers: { "X-Powered-By": "Mango" }
            });
            if (res.status !== 200 || !res.ok) throw new Error("res.status mismatch");
            if (res.statusText !== "OK") throw new Error("res.statusText mismatch");
            if (res.headers.get("x-powered-by") !== "Mango") throw new Error("res.headers mismatch");

            var jsonRes = Response.json({ success: true });
            if (jsonRes.status !== 200) throw new Error("Response.json status mismatch");
            if (jsonRes.headers.get("content-type") !== "application/json") throw new Error("Response.json content-type mismatch");

            // 10. XMLHttpRequest & XMLHttpRequestUpload
            var xhr = new XMLHttpRequest();
            if (!(xhr instanceof XMLHttpRequest)) throw new Error("xhr not XMLHttpRequest");
            if (!xhr.upload) throw new Error("xhr.upload missing");
            if (typeof xhr.upload.addEventListener !== "function") throw new Error("xhr.upload.addEventListener missing");
            xhr.open("GET", "/test-endpoint");
            xhr.setRequestHeader("X-Requested-With", "XMLHttpRequest");
            xhr.send();

            // 11. WebSocket
            var ws = new WebSocket("wss://echo.example.com");
            if (!(ws instanceof WebSocket)) throw new Error("ws not WebSocket");
            if (ws.readyState !== 0 /* CONNECTING */ && ws.readyState !== 3 /* CLOSED */) throw new Error("ws.readyState invalid");
            if (typeof ws.addEventListener !== "function") throw new Error("ws.addEventListener missing");
            if (typeof ws.send !== "function") throw new Error("ws.send missing");
            if (typeof ws.close !== "function") throw new Error("ws.close missing");

            // 12. Cache API & Resource Timing
            if (typeof caches === "undefined" || typeof CacheStorage === "undefined") throw new Error("caches undefined");
            caches.open("v1").then(function(c) {
                var cacheReq = new Request("https://example.com/cached-page");
                var cacheRes = new Response("Cached content", { status: 200 });
                return c.put(cacheReq, cacheRes).then(function() {
                    return c.match("https://example.com/cached-page");
                }).then(function(matched) {
                    if (!matched) throw new Error("cache.match failed to find entry");
                    return matched.text();
                }).then(function(txt) {
                    if (txt !== "Cached content") throw new Error("cache text mismatch: " + txt);
                    window.__cacheApiTested = true;
                });
            });

            performance.mark("task-start");
            performance.mark("task-end");
            var measure = performance.measure("task-duration", "task-start", "task-end");
            if (measure.entryType !== "measure") throw new Error("measure entryType mismatch");
            if (performance.getEntriesByType("mark").length < 2) throw new Error("performance.getEntriesByType marks missing");
        "##;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_web_apis failed: {}", e);
        }

        let verify_reader = r#"
            if (window.readerResult !== "Hello World") throw new Error("FileReader.readAsText failed: " + window.readerResult);
            if (!window.__cacheApiTested) throw new Error("Cache API async operations failed or did not resolve");
        "#;
        if let Err(e) = rt.execute_script(verify_reader) {
            panic!("test_web_apis verify_reader failed: {}", e);
        }
    }

    #[test]
    fn test_dom_measurement_and_layout() {
        let doc = parse_html(r#"
            <html>
            <head></head>
            <body>
              <div id="parent" style="position: relative; width: 400px; height: 300px; border-width: 5px;">
                <div id="child" style="color: red; font-size: 16px;">Hello</div>
              </div>
              <div id="target" style="width: 200px; height: 50px;">Target</div>
            </body>
            </html>
        "#);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let parent_id = {
            let d = rt.document_ref();
            let root = d.root();
            d.find_element_by_id(root, "parent").expect("parent")
        };
        let child_id = {
            let d = rt.document_ref();
            let root = d.root();
            d.find_element_by_id(root, "child").expect("child")
        };
        let target_id = {
            let d = rt.document_ref();
            let root = d.root();
            d.find_element_by_id(root, "target").expect("target")
        };

        let mut bounds = HashMap::new();
        bounds.insert(parent_id.raw(), [20.0, 30.0, 400.0, 300.0]);
        bounds.insert(child_id.raw(), [70.0, 90.0, 100.0, 80.0]);
        bounds.insert(target_id.raw(), [10.0, 400.0, 200.0, 50.0]);
        rt.set_layout_bounds(bounds);

        let script = r##"
            var parent = document.getElementById("parent");
            var child = document.getElementById("child");
            var target = document.getElementById("target");

            // 1. element.getBoundingClientRect()
            var childRect = child.getBoundingClientRect();
            if (!(childRect instanceof DOMRect)) throw new Error("childRect should be instanceof DOMRect");
            if (!(childRect instanceof DOMRectReadOnly)) throw new Error("childRect should be instanceof DOMRectReadOnly");
            if (childRect.x !== 70 || childRect.y !== 90 || childRect.width !== 100 || childRect.height !== 80) {
                throw new Error("getBoundingClientRect coordinates incorrect: " + JSON.stringify(childRect));
            }
            if (childRect.top !== 90 || childRect.left !== 70 || childRect.right !== 170 || childRect.bottom !== 170) {
                throw new Error("getBoundingClientRect bounds incorrect: " + JSON.stringify(childRect));
            }
            var jsonRect = childRect.toJSON();
            if (jsonRect.width !== 100 || jsonRect.height !== 80 || jsonRect.top !== 90 || jsonRect.left !== 70) {
                throw new Error("DOMRect.toJSON failed: " + JSON.stringify(jsonRect));
            }
            var fromRect = DOMRect.fromRect({ x: 5, y: 15, width: 25, height: 35 });
            if (fromRect.x !== 5 || fromRect.y !== 15 || fromRect.width !== 25 || fromRect.height !== 35) {
                throw new Error("DOMRect.fromRect failed");
            }
            var fromRectRO = DOMRectReadOnly.fromRect({ x: 1, y: 2, width: 3, height: 4 });
            if (fromRectRO.x !== 1 || fromRectRO.y !== 2 || fromRectRO.width !== 3 || fromRectRO.height !== 4) {
                throw new Error("DOMRectReadOnly.fromRect failed");
            }

            // 2. element.getClientRects()
            var clientRects = child.getClientRects();
            if (!(clientRects instanceof DOMRectList)) throw new Error("clientRects should be instanceof DOMRectList");
            if (clientRects.length !== 1) throw new Error("clientRects.length should be 1");
            if (clientRects.item(0).width !== 100) throw new Error("clientRects.item(0) width mismatch");
            if (clientRects[0].height !== 80) throw new Error("clientRects[0] height mismatch");
            var rectArr = Array.from(clientRects);
            if (rectArr.length !== 1 || rectArr[0].width !== 100) throw new Error("clientRects iterable mismatch");

            // 3. element.offsetWidth / offsetHeight / offsetTop / offsetLeft / offsetParent
            if (child.offsetWidth !== 100) throw new Error("child.offsetWidth mismatch: " + child.offsetWidth);
            if (child.offsetHeight !== 80) throw new Error("child.offsetHeight mismatch: " + child.offsetHeight);
            if (child.offsetParent !== parent) throw new Error("child.offsetParent should be parent");
            if (child.offsetLeft !== 50) throw new Error("child.offsetLeft should be 50, got: " + child.offsetLeft);
            if (child.offsetTop !== 60) throw new Error("child.offsetTop should be 60, got: " + child.offsetTop);
            if (parent.offsetWidth !== 400 || parent.offsetHeight !== 300) {
                throw new Error("parent offset dimensions mismatch");
            }
            var detached = document.createElement("div");
            if (detached.offsetParent !== null) throw new Error("detached element offsetParent should be null");

            // 4. element.clientWidth / clientHeight / clientTop / clientLeft
            if (parent.clientLeft !== 5) throw new Error("parent.clientLeft mismatch: " + parent.clientLeft);
            if (parent.clientTop !== 5) throw new Error("parent.clientTop mismatch: " + parent.clientTop);
            if (parent.clientWidth !== 390) throw new Error("parent.clientWidth mismatch: " + parent.clientWidth);
            if (parent.clientHeight !== 290) throw new Error("parent.clientHeight mismatch: " + parent.clientHeight);

            if (child.clientLeft !== 0 || child.clientTop !== 0) throw new Error("child clientTop/clientLeft mismatch");
            if (child.clientWidth !== 100 || child.clientHeight !== 80) throw new Error("child client dimensions mismatch");

            // 5. element.scrollWidth / scrollHeight / scrollTop / scrollLeft (get/set)
            if (child.scrollWidth !== 100 || child.scrollHeight !== 80) {
                throw new Error("child scrollWidth/Height mismatch: " + child.scrollWidth + "x" + child.scrollHeight);
            }
            child.scrollTop = 25;
            if (child.scrollTop !== 25) throw new Error("child.scrollTop setter failed: " + child.scrollTop);
            child.scrollLeft = 35;
            if (child.scrollLeft !== 35) throw new Error("child.scrollLeft setter failed: " + child.scrollLeft);

            child.scrollTo(10, 20);
            if (child.scrollLeft !== 10 || child.scrollTop !== 20) throw new Error("child.scrollTo failed");
            child.scrollBy(5, 10);
            if (child.scrollLeft !== 15 || child.scrollTop !== 30) throw new Error("child.scrollBy failed");
            child.scroll({ left: 0, top: 0 });
            if (child.scrollLeft !== 0 || child.scrollTop !== 0) throw new Error("child.scroll failed");

            // 6. element.scrollIntoView()
            target.scrollIntoView();
            if (window.scrollY !== 400) throw new Error("scrollIntoView should update window.scrollY to 400, got: " + window.scrollY);
            target.scrollIntoView({ behavior: 'smooth', block: 'start' });

            // 7. window.getComputedStyle()
            var style = window.getComputedStyle(child);
            if (!(style instanceof CSSStyleDeclaration)) throw new Error("style should be instanceof CSSStyleDeclaration");
            if (style.color !== "red") throw new Error("style.color should be red, got: " + style.color);
            if (style.fontSize !== "16px") throw new Error("style.fontSize should be 16px, got: " + style.fontSize);
            if (style.getPropertyValue("font-size") !== "16px") throw new Error("style.getPropertyValue failed");
            if (style.getPropertyValue("color") !== "red") throw new Error("style.getPropertyValue color failed");
            if (style.width !== "100px") throw new Error("computed width should be 100px from layout, got: " + style.width);
            if (style.height !== "80px") throw new Error("computed height should be 80px from layout, got: " + style.height);
            if (style.length === 0) throw new Error("style.length should be > 0");
            if (!style.item(0)) throw new Error("style.item(0) should be non-empty");
            var styleProps = Array.from(style);
            if (styleProps.indexOf("color") === -1) throw new Error("CSSStyleDeclaration iterable should include 'color'");

            // 8. window.scrollTo(), window.scrollBy(), window.scroll() + scroll event
            window.scrollEventCount = 0;
            window.addEventListener("scroll", function() {
                window.scrollEventCount++;
            });
            window.scrollTo(50, 100);
            if (window.scrollX !== 50 || window.scrollY !== 100) throw new Error("window.scrollTo failed");
            window.scrollBy(10, 20);
            if (window.scrollX !== 60 || window.scrollY !== 120) throw new Error("window.scrollBy failed");
            window.scroll({ left: 30, top: 70 });
            if (window.scrollX !== 30 || window.scrollY !== 70) throw new Error("window.scroll failed");
            if (window.scrollEventCount < 3) throw new Error("scroll events should have fired: " + window.scrollEventCount);

            // 9. window.pageXOffset / window.pageYOffset / window.scrollX / window.scrollY
            if (window.pageXOffset !== window.scrollX || window.pageXOffset !== 30) throw new Error("window.pageXOffset mismatch");
            if (window.pageYOffset !== window.scrollY || window.pageYOffset !== 70) throw new Error("window.pageYOffset mismatch");

            // 10. document.elementFromPoint() & document.elementsFromPoint()
            var topEl = document.elementFromPoint(75, 95);
            if (topEl !== child) throw new Error("elementFromPoint should return child, got: " + (topEl ? topEl.tagName : topEl));
            var allEls = document.elementsFromPoint(75, 95);
            if (allEls.length < 2) throw new Error("elementsFromPoint should return at least 2 elements");
            if (allEls[0] !== child) throw new Error("first elementsFromPoint element should be child");
            if (allEls[1] !== parent) throw new Error("second elementsFromPoint element should be parent");

            // 11. ResizeObserver
            window.roFired = false;
            window.roTarget = null;
            window.roWidth = 0;
            var ro = new ResizeObserver(function(entries) {
                window.roFired = true;
                if (entries.length > 0) {
                    window.roTarget = entries[0].target;
                    window.roWidth = entries[0].contentRect.width;
                }
            });
            ro.observe(child);

            // 12. IntersectionObserver
            window.ioFired = false;
            window.ioTarget = null;
            window.ioIntersecting = false;
            var io = new IntersectionObserver(function(entries) {
                window.ioFired = true;
                if (entries.length > 0) {
                    window.ioTarget = entries[0].target;
                    window.ioIntersecting = entries[0].isIntersecting;
                }
            }, { threshold: [0, 1] });
            io.observe(child);

            // 13. MutationObserver
            window.moRecords = [];
            var mo = new MutationObserver(function(records) {
                window.moRecords.push.apply(window.moRecords, records);
            });
            mo.observe(parent, { childList: true, attributes: true, subtree: true, characterData: true });

            var newSpan = document.createElement("span");
            parent.appendChild(newSpan);
            parent.setAttribute("data-test", "val");
            newSpan.textContent = "Hi";

            // Test takeRecords() synchronously
            var syncRecords = mo.takeRecords();
            if (syncRecords.length === 0) throw new Error("MutationObserver.takeRecords() should have records");
            var hasChildList = syncRecords.some(function(r) { return r.type === "childList"; });
            var hasAttributes = syncRecords.some(function(r) { return r.type === "attributes" && r.attributeName === "data-test"; });
            var hasCharData = syncRecords.some(function(r) { return r.type === "characterData"; });
            if (!hasChildList) throw new Error("Missing childList mutation record");
            if (!hasAttributes) throw new Error("Missing attributes mutation record");
            if (!hasCharData) throw new Error("Missing characterData mutation record");

            // Make one more mutation to test microtask delivery
            parent.setAttribute("data-async", "yes");

            // 14. Lazy loading with IntersectionObserver
            window.lazyImg = document.createElement("img");
            window.lazyImg.setAttribute("data-src", "https://example.com/photo.jpg");
            window.lazyImg.loading = "lazy";
            parent.appendChild(window.lazyImg);
            if (window.lazyImg.loading !== "lazy") throw new Error("lazyImg.loading should be lazy");
        "##;

        if let Err(e) = rt.execute_script(script) {
            panic!("test_dom_measurement_and_layout script failed: {}", e);
        }

        // Verify async callbacks scheduled via microtasks (run_jobs was called at the end of execute_script)
        let verify_observers = r#"
            if (!window.roFired) throw new Error("ResizeObserver callback was not called");
            if (window.roTarget !== child) throw new Error("ResizeObserver target mismatch");
            if (window.roWidth !== 100) throw new Error("ResizeObserver width mismatch: " + window.roWidth);

            if (!window.ioFired) throw new Error("IntersectionObserver callback was not called");
            if (window.ioTarget !== child) throw new Error("IntersectionObserver target mismatch");
            if (!window.ioIntersecting) throw new Error("IntersectionObserver isIntersecting should be true");

            if (window.moRecords.length === 0) throw new Error("MutationObserver async callback was not called");
            var asyncAttr = window.moRecords.some(function(r) { return r.attributeName === "data-async"; });
            if (!asyncAttr) throw new Error("MutationObserver did not deliver data-async record");

            if (!window.lazyImg._lazyLoaded) throw new Error("lazyImg._lazyLoaded should be true");
            if (window.lazyImg.getAttribute("src") !== "https://example.com/photo.jpg") throw new Error("lazyImg src should be populated from data-src");
        "#;

        if let Err(e) = rt.execute_script(verify_observers) {
            panic!("test_dom_measurement_and_layout verify_observers failed: {}", e);
        }
    }

    #[test]
    fn test_csprng_and_storage_partitioning() {
        let doc_a = Document::new();
        let doc_b = Document::new();
        let shared_storage = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let mut rt_a = JsRuntime::new_with_url_and_storage(
            doc_a,
            800.0,
            600.0,
            "https://origin-a.example/page",
            shared_storage.clone(),
        );
        let mut rt_b = JsRuntime::new_with_url_and_storage(
            doc_b,
            800.0,
            600.0,
            "https://origin-b.example/page",
            shared_storage.clone(),
        );

        let script_crypto = r#"
            var buf = new Uint8Array(32);
            crypto.getRandomValues(buf);
            var nonZero = false;
            for (var i = 0; i < buf.length; i++) {
                if (buf[i] !== 0) nonZero = true;
            }
            if (!nonZero) throw new Error("CSPRNG returned all zeroes");

            var quotaExceeded = false;
            try {
                var bigBuf = new Uint8Array(65537);
                crypto.getRandomValues(bigBuf);
            } catch(e) {
                if (e.name === "QuotaExceededError") quotaExceeded = true;
            }
            if (!quotaExceeded) throw new Error("Expected QuotaExceededError for length > 65536");

            var uuid = crypto.randomUUID();
            if (typeof uuid !== "string" || uuid.length !== 36) {
                throw new Error("Invalid UUID format: " + uuid);
            }
        "#;
        rt_a.execute_script(script_crypto).expect("Crypto test failed");

        // Test localStorage origin partitioning
        rt_a.execute_script("localStorage.setItem('secret', 'originA_data');")
            .expect("A setItem failed");

        rt_b.execute_script(r#"
            if (localStorage.getItem('secret') !== null) {
                throw new Error("Cross-origin key leakage: origin B read origin A's key");
            }
            localStorage.removeItem('secret');
        "#).expect("B read/remove failed");

        rt_a.execute_script(r#"
            if (localStorage.getItem('secret') !== 'originA_data') {
                throw new Error("Cross-origin deletion leakage: origin B deleted origin A's key");
            }
        "#).expect("A verify key remains failed");

        rt_b.execute_script("localStorage.clear();").expect("B clear failed");

        rt_a.execute_script(r#"
            if (localStorage.getItem('secret') !== 'originA_data') {
                throw new Error("Cross-origin clear leakage: origin B clear() wiped origin A's key");
            }
        "#).expect("A verify key after clear failed");
    }

    #[test]
    fn test_dom_manipulation() {
        let html = r##"
            <html>
            <head></head>
            <body>
              <div id="container">
                <p id="target">Middle</p>
              </div>
              <div id="replace-container">
                <span id="old1">One</span>
                <span id="old2">Two</span>
              </div>
              <ul id="list">
                <li id="item2">Two</li>
              </ul>
              <div id="removable">To be removed</div>
              <select id="my-select">
                <option value="1">First</option>
                <option value="2">Second</option>
                <option value="3">Third</option>
              </select>
              <div id="range-test">
                <span id="r1">Hello</span> <span id="r2">World</span>
              </div>
              <input type="text" id="my-input" />
              <button id="my-btn">Click Me</button>
              <input type="checkbox" id="my-check" />
              <button id="disabled-btn" disabled>Disabled</button>
            </body>
            </html>
        "##;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        let script = r##"
            // ==========================================
            // 1. insertAdjacentHTML
            // ==========================================
            var target = document.getElementById("target");
            target.insertAdjacentHTML("beforebegin", "<span id='before-b'>BB</span>");
            target.insertAdjacentHTML("afterbegin", "<span id='after-b'>AB</span>");
            target.insertAdjacentHTML("beforeend", "<span id='before-e'>BE</span>");
            target.insertAdjacentHTML("afterend", "<span id='after-e'>AE</span>");

            var container = document.getElementById("container");
            var bb = document.getElementById("before-b");
            var ab = document.getElementById("after-b");
            var be = document.getElementById("before-e");
            var ae = document.getElementById("after-e");

            if (!bb || !ab || !be || !ae) throw new Error("insertAdjacentHTML: nodes not found");
            if (target.previousElementSibling !== bb) throw new Error("beforebegin failed");
            if (target.firstElementChild !== ab) throw new Error("afterbegin failed");
            if (target.lastElementChild !== be) throw new Error("beforeend failed");
            if (target.nextElementSibling !== ae) throw new Error("afterend failed");

            var syntaxErrorCaught = false;
            try {
                target.insertAdjacentHTML("invalid_pos", "<span>fail</span>");
            } catch (e) {
                if (e.name === "SyntaxError") syntaxErrorCaught = true;
            }
            if (!syntaxErrorCaught) throw new Error("insertAdjacentHTML did not throw SyntaxError on invalid position");

            // ==========================================
            // 2. replaceChildren
            // ==========================================
            var repCont = document.getElementById("replace-container");
            if (repCont.children.length !== 2) throw new Error("replaceChildren setup invalid");

            // replace with mix of elements and text
            var newChild1 = document.createElement("b");
            newChild1.textContent = "Bold";
            repCont.replaceChildren(newChild1, "TextString");
            if (repCont.childNodes.length !== 2) throw new Error("replaceChildren length mismatch");
            if (repCont.childNodes[0].tagName.toLowerCase() !== "b") throw new Error("replaceChildren first child mismatch");
            if (repCont.childNodes[1].textContent !== "TextString") throw new Error("replaceChildren string node mismatch");

            // replace with empty args clears all children
            repCont.replaceChildren();
            if (repCont.childNodes.length !== 0) throw new Error("replaceChildren() should clear children");

            // ==========================================
            // 3. append, prepend, before, after, replaceWith
            // ==========================================
            var list = document.getElementById("list");
            var item2 = document.getElementById("item2");

            // prepend
            var item1 = document.createElement("li");
            item1.id = "item1";
            item1.textContent = "One";
            list.prepend(item1);
            if (list.firstElementChild.id !== "item1") throw new Error("prepend failed");

            // append
            var item4 = document.createElement("li");
            item4.id = "item4";
            item4.textContent = "Four";
            list.append(item4, "end-text");
            if (list.lastChild.textContent !== "end-text") throw new Error("append string failed");
            if (item4.previousElementSibling.id !== "item2") throw new Error("append element failed");

            // before & after on item2
            var itemBefore = document.createElement("li");
            itemBefore.id = "item-before";
            item2.before(itemBefore);
            if (item2.previousElementSibling.id !== "item-before") throw new Error("before() failed");

            var itemAfter = document.createElement("li");
            itemAfter.id = "item-after";
            item2.after(itemAfter);
            if (item2.nextElementSibling.id !== "item-after") throw new Error("after() failed");

            // replaceWith
            var itemReplaced = document.createElement("li");
            itemReplaced.id = "item-replaced";
            itemBefore.replaceWith(itemReplaced);
            if (document.getElementById("item-before") !== null) throw new Error("replaceWith old node still exists");
            if (item2.previousElementSibling.id !== "item-replaced") throw new Error("replaceWith new node not in place");

            // ==========================================
            // 4. remove()
            // ==========================================
            var removable = document.getElementById("removable");
            removable.remove();
            if (document.getElementById("removable") !== null) throw new Error("remove() element failed");

            // HTMLSelectElement.remove(index)
            var select = document.getElementById("my-select");
            if (select.children.length !== 3) throw new Error("select setup invalid");
            select.remove(1); // removes option 2
            if (select.children.length !== 2) throw new Error("select.remove(1) length mismatch");
            if (select.children[1].value !== "3") throw new Error("select.remove(1) wrong option removed");

            // ==========================================
            // 5. Range API & document.createRange()
            // ==========================================
            var range = document.createRange();
            if (!range) throw new Error("document.createRange returned falsy");
            if (Range.START_TO_START !== 0 || Range.START_TO_END !== 1 || Range.END_TO_END !== 2 || Range.END_TO_START !== 3) {
                throw new Error("Range constants invalid");
            }
            if (range.START_TO_START !== 0) throw new Error("Range.prototype constants invalid");

            var rTest = document.getElementById("range-test");
            var r1 = document.getElementById("r1");
            var r2 = document.getElementById("r2");

            range.selectNodeContents(rTest);
            if (range.startContainer !== rTest || range.endContainer !== rTest) {
                throw new Error("selectNodeContents container mismatch");
            }
            if (range.collapsed) throw new Error("selectNodeContents range should not be collapsed");

            range.collapse(true);
            if (!range.collapsed) throw new Error("range.collapse(true) failed");

            range.setStart(rTest, 0);
            range.setEnd(rTest, 1);
            if (range.commonAncestorContainer !== rTest) throw new Error("commonAncestorContainer mismatch");

            var frag = range.cloneContents();
            if (!frag || frag.nodeType !== 11) throw new Error("cloneContents should return DocumentFragment");

            var ctxFrag = range.createContextualFragment("<span id='ctx-elem'>Context</span>");
            if (!ctxFrag || ctxFrag.nodeType !== 11) throw new Error("createContextualFragment failed");
            if (!ctxFrag.querySelector("#ctx-elem")) throw new Error("createContextualFragment child missing");

            var clonedRange = range.cloneRange();
            if (clonedRange.startContainer !== range.startContainer || clonedRange.endOffset !== range.endOffset) {
                throw new Error("cloneRange failed");
            }

            var comp = range.compareBoundaryPoints(Range.START_TO_START, clonedRange);
            if (comp !== 0) throw new Error("compareBoundaryPoints should be 0 for identical ranges");

            // Range navigation: setStartBefore/After, setEndBefore/After
            range.setStartBefore(r2);
            range.setEndAfter(r2);
            if (!range.intersectsNode(r2)) throw new Error("range should intersect r2");
            if (!range.isPointInRange(range.startContainer, range.startOffset)) throw new Error("range isPointInRange failed");
            if (range.comparePoint(range.startContainer, range.startOffset) !== 0) throw new Error("range comparePoint failed");

            // Range getBoundingClientRect
            var rRect = range.getBoundingClientRect();
            if (typeof rRect.width !== "number" || typeof rRect.height !== "number") {
                throw new Error("range.getBoundingClientRect() invalid");
            }

            // insertNode
            var insertedSpan = document.createElement("span");
            insertedSpan.id = "inserted-into-range";
            range.collapse(true);
            range.insertNode(insertedSpan);
            if (document.getElementById("inserted-into-range") === null) {
                throw new Error("insertNode failed to insert element into DOM");
            }

            // extractContents
            var extRange = document.createRange();
            extRange.selectNode(insertedSpan);
            var extractedFrag = extRange.extractContents();
            if (document.getElementById("inserted-into-range") !== null) {
                throw new Error("extractContents failed to remove element from DOM");
            }
            if (!extractedFrag || extractedFrag.childNodes.length === 0) {
                throw new Error("extractContents failed to return fragment with nodes");
            }

            // surroundContents
            var surroundWrapper = document.createElement("div");
            surroundWrapper.id = "surround-wrapper";
            var surRange = document.createRange();
            surRange.selectNode(r1);
            surRange.surroundContents(surroundWrapper);
            if (r1.parentElement.id !== "surround-wrapper") {
                throw new Error("surroundContents failed to wrap target node");
            }

            // deleteContents
            var delRange = document.createRange();
            delRange.selectNode(surroundWrapper);
            delRange.deleteContents();
            if (document.getElementById("surround-wrapper") !== null) {
                throw new Error("deleteContents failed to delete nodes from DOM");
            }

            // ==========================================
            // 6. Selection API & window.getSelection()
            // ==========================================
            var sel = window.getSelection();
            if (!sel) throw new Error("window.getSelection returned falsy");
            if (document.getSelection() !== sel) throw new Error("document.getSelection !== window.getSelection");

            sel.removeAllRanges();
            if (sel.rangeCount !== 0) throw new Error("removeAllRanges failed");

            sel.addRange(range);
            if (sel.rangeCount !== 1) throw new Error("addRange failed, rangeCount !== 1");
            if (sel.getRangeAt(0) !== range) throw new Error("getRangeAt(0) mismatch");

            sel.selectAllChildren(rTest);
            if (sel.anchorNode !== rTest) throw new Error("selectAllChildren anchorNode mismatch");
            if (!sel.containsNode(r2, true)) throw new Error("containsNode should be true for child r2");

            sel.setBaseAndExtent(rTest, 0, rTest, 1);
            if (sel.anchorOffset !== 0 || sel.focusOffset !== 1) {
                throw new Error("setBaseAndExtent offsets mismatch");
            }

            sel.collapse(r2, 0);
            if (!sel.isCollapsed) throw new Error("Selection should be collapsed after collapse()");

            sel.extend(rTest, 1);
            if (sel.focusNode !== rTest || sel.focusOffset !== 1) {
                throw new Error("extend failed");
            }

            sel.empty();
            if (sel.rangeCount !== 0) throw new Error("empty() failed to clear ranges");

            // DocumentFragment & Document manipulation
            var testFrag = document.createDocumentFragment();
            testFrag.append("FragItem1", "FragItem2");
            if (testFrag.childNodes.length !== 2) throw new Error("Fragment.append failed");
            testFrag.replaceChildren("NewFragItem");
            if (testFrag.childNodes.length !== 1 || testFrag.childNodes[0].textContent !== "NewFragItem") {
                throw new Error("Fragment.replaceChildren failed");
            }

            // ==========================================
            // 7. element.focus() and element.blur()
            // ==========================================
            var input = document.getElementById("my-input");
            var focusEvents = [];
            input.addEventListener("focus", function(e) { focusEvents.push("focus"); });
            input.addEventListener("focusin", function(e) { focusEvents.push("focusin"); });
            input.addEventListener("blur", function(e) { focusEvents.push("blur"); });
            input.addEventListener("focusout", function(e) { focusEvents.push("focusout"); });

            if (document.activeElement !== document.body) {
                throw new Error("Initial activeElement should be document.body");
            }

            input.focus();
            if (document.activeElement !== input) {
                throw new Error("activeElement should be input after focus()");
            }
            if (focusEvents.indexOf("focus") === -1 || focusEvents.indexOf("focusin") === -1) {
                throw new Error("focus/focusin events not dispatched on focus()");
            }

            input.blur();
            if (document.activeElement !== document.body) {
                throw new Error("activeElement should revert to body after blur()");
            }
            if (focusEvents.indexOf("blur") === -1 || focusEvents.indexOf("focusout") === -1) {
                throw new Error("blur/focusout events not dispatched on blur()");
            }

            // ==========================================
            // 8. element.click() (synthetic click)
            // ==========================================
            var btn = document.getElementById("my-btn");
            var btnClicked = false;
            var clickDetail = 0;
            var clickBubbles = false;
            var clickCancelable = false;
            btn.addEventListener("click", function(e) {
                btnClicked = true;
                clickDetail = e.detail;
                clickBubbles = e.bubbles;
                clickCancelable = e.cancelable;
            });
            btn.click();
            if (!btnClicked) throw new Error("btn.click() did not trigger click listener");
            if (!clickBubbles) throw new Error("synthetic click event should bubble");
            if (!clickCancelable) throw new Error("synthetic click event should be cancelable");

            // Checkbox click toggle & change event
            var check = document.getElementById("my-check");
            var checkChanged = false;
            check.addEventListener("change", function(e) {
                checkChanged = true;
            });
            if (check.checked) throw new Error("checkbox should start unchecked");
            check.click();
            if (!check.checked) throw new Error("checkbox should be checked after click()");
            if (!checkChanged) throw new Error("checkbox change event should fire on click()");

            // Disabled button click should be ignored
            var disabledBtn = document.getElementById("disabled-btn");
            var disabledClicked = false;
            disabledBtn.addEventListener("click", function() {
                disabledClicked = true;
            });
            disabledBtn.click();
            if (disabledClicked) throw new Error("Disabled button should not fire click");
        "##;

        rt.execute_script(script).expect("Section 7.5 DOM manipulation test failed");
    }

    #[test]
    fn test_storage_and_history() {
        let temp_dir = std::env::temp_dir();
        let storage_file = temp_dir.join(format!(
            "mango_test_storage_{}.tsv",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let shared_ls = Arc::new(Mutex::new(HashMap::new()));
        let tab1_ss = Arc::new(Mutex::new(HashMap::new()));
        let cookies = Arc::new(Mutex::new(mango_net::CookieJar::new()));

        let html = "<html><head></head><body><h1>Storage and History Test</h1></body></html>";

        let mut rt_loc = JsRuntime::new_with_url_storage_session_and_cookies(
            parse_html(html),
            800.0,
            600.0,
            "https://example.com:8080/path/index.html?search=rust#section1",
            shared_ls.clone(),
            tab1_ss.clone(),
            cookies.clone(),
        );

        // 1. Test Location Object
        let loc_script = r##"
            // Location properties
            if (location.href !== "https://example.com:8080/path/index.html?search=rust#section1") {
                throw new Error("location.href mismatch: " + location.href);
            }
            if (location.protocol !== "https:") throw new Error("location.protocol mismatch: " + location.protocol);
            if (location.host !== "example.com:8080") throw new Error("location.host mismatch: " + location.host);
            if (location.hostname !== "example.com") throw new Error("location.hostname mismatch: " + location.hostname);
            if (location.port !== "8080") throw new Error("location.port mismatch: " + location.port);
            if (location.pathname !== "/path/index.html") throw new Error("location.pathname mismatch: " + location.pathname);
            if (location.search !== "?search=rust") throw new Error("location.search mismatch: " + location.search);
            if (location.hash !== "#section1") throw new Error("location.hash mismatch: " + location.hash);
            if (location.origin !== "https://example.com:8080") throw new Error("location.origin mismatch: " + location.origin);
            if (location.toString() !== location.href) throw new Error("location.toString() mismatch");

            // Location hash change event dispatch
            var hashChanged = false;
            var oldHashUrl = "";
            var newHashUrl = "";
            window.addEventListener("hashchange", function(e) {
                hashChanged = true;
                oldHashUrl = e.oldURL;
                newHashUrl = e.newURL;
            });
            location.hash = "#section2";
            if (location.hash !== "#section2") throw new Error("location.hash setter failed");
            if (!location.href.endsWith("#section2")) throw new Error("location.href not updated after hash change");
            if (!hashChanged) throw new Error("hashchange event not fired on location.hash change");

            // Location search & pathname setter
            location.search = "?page=2";
            if (location.search !== "?page=2") throw new Error("location.search setter failed");
            location.pathname = "/newpath";
            if (location.pathname !== "/newpath") throw new Error("location.pathname setter failed");

            // Location assign & replace & reload
            location.assign("/assigned");
            if (location.pathname !== "/assigned") throw new Error("location.assign failed");
            location.replace("/replaced");
            if (location.pathname !== "/replaced") throw new Error("location.replace failed");
            location.reload(); // Should execute safely
        "##;
        rt_loc.execute_script(loc_script).expect("Location test failed");

        // 2. Test History Object
        let mut rt_hist = JsRuntime::new_with_url_storage_session_and_cookies(
            parse_html(html),
            800.0,
            600.0,
            "https://example.com:8080/test",
            shared_ls.clone(),
            tab1_ss.clone(),
            cookies.clone(),
        );

        let hist_script = r##"
            if (history.state !== null) throw new Error("Initial history.state should be null");
            if (history.length !== 1) throw new Error("Initial history.length should be 1, got: " + history.length);
            if (history.scrollRestoration !== "auto") throw new Error("Initial scrollRestoration should be auto");
            history.scrollRestoration = "manual";
            if (history.scrollRestoration !== "manual") throw new Error("scrollRestoration setter failed");

            // PushState
            history.pushState({ count: 1, name: "step1" }, "Step 1", "/step1?q=1#h1");
            if (history.length !== 2) throw new Error("history.length after pushState should be 2, got: " + history.length);
            if (!history.state || history.state.count !== 1 || history.state.name !== "step1") {
                throw new Error("history.state mismatch after pushState: " + JSON.stringify(history.state));
            }
            if (location.pathname !== "/step1") throw new Error("location.pathname after pushState mismatch: " + location.pathname);
            if (location.search !== "?q=1") throw new Error("location.search after pushState mismatch: " + location.search);
            if (location.hash !== "#h1") throw new Error("location.hash after pushState mismatch: " + location.hash);

            // ReplaceState
            history.replaceState({ count: 2, name: "step2" }, "Step 2", "/step2?q=2#h2");
            if (history.length !== 2) throw new Error("history.length after replaceState should remain 2, got: " + history.length);
            if (!history.state || history.state.count !== 2 || history.state.name !== "step2") {
                throw new Error("history.state mismatch after replaceState: " + JSON.stringify(history.state));
            }
            if (location.pathname !== "/step2") throw new Error("location.pathname after replaceState mismatch: " + location.pathname);

            // Cross-origin SecurityError test
            var securityCaught = false;
            try {
                history.pushState({ evil: true }, "Evil", "https://attacker.com/evil");
            } catch (e) {
                if (e.name === "SecurityError") {
                    securityCaught = true;
                }
            }
            if (!securityCaught) throw new Error("Cross-origin pushState should throw SecurityError");

            // Popstate event via back/forward/go
            history.pushState({ count: 3, name: "step3" }, "Step 3", "/step3");
            if (history.length !== 3) throw new Error("history.length should be 3");

            var popstateFired = false;
            var popstateState = null;
            window.addEventListener("popstate", function(e) {
                popstateFired = true;
                popstateState = e.state;
            });

            history.back();
            if (!popstateFired) throw new Error("popstate event did not fire on history.back()");
            if (!popstateState || popstateState.count !== 2) {
                throw new Error("popstate event.state mismatch: " + JSON.stringify(popstateState));
            }
            if (location.pathname !== "/step2") throw new Error("location not updated on back(): " + location.pathname);

            popstateFired = false;
            history.forward();
            if (!popstateFired) throw new Error("popstate event did not fire on history.forward()");
            if (!popstateState || popstateState.count !== 3) {
                throw new Error("popstate event.state mismatch on forward(): " + JSON.stringify(popstateState));
            }
            if (location.pathname !== "/step3") throw new Error("location not updated on forward(): " + location.pathname);

            popstateFired = false;
            history.go(-1);
            if (!popstateFired || !popstateState || popstateState.count !== 2) {
                throw new Error("history.go(-1) failed to fire popstate or wrong state");
            }
        "##;
        rt_hist.execute_script(hist_script).expect("History test failed");

        // 3. Test localStorage (Item methods, property proxy access, file persistence)
        let mut rt_storage = JsRuntime::new_with_url_storage_session_and_cookies(
            parse_html(html),
            800.0,
            600.0,
            "https://example.com:8080/storage_test",
            shared_ls.clone(),
            tab1_ss.clone(),
            cookies.clone(),
        );

        let ls_script = r##"
            localStorage.clear();
            if (localStorage.length !== 0) throw new Error("localStorage should be empty after clear()");
            if (localStorage.getItem("missing") !== null) throw new Error("getItem on missing should return null");

            localStorage.setItem("user", "Alice");
            localStorage.setItem("theme", "dark");
            if (localStorage.length !== 2) throw new Error("localStorage.length should be 2, got: " + localStorage.length);
            if (localStorage.getItem("user") !== "Alice") throw new Error("getItem('user') mismatch");
            if (localStorage.getItem("theme") !== "dark") throw new Error("getItem('theme') mismatch");

            // Proxy property access
            localStorage.customProp = "customValue";
            if (localStorage.getItem("customProp") !== "customValue") throw new Error("Proxy property set failed to write to storage");
            if (localStorage.customProp !== "customValue") throw new Error("Proxy property get failed");
            if (!("customProp" in localStorage)) throw new Error("'customProp' in localStorage failed");

            // key(index)
            var key0 = localStorage.key(0);
            if (key0 === null) throw new Error("localStorage.key(0) returned null");

            // removeItem & delete proxy
            localStorage.removeItem("theme");
            if (localStorage.getItem("theme") !== null) throw new Error("removeItem failed");
            delete localStorage.customProp;
            if (localStorage.getItem("customProp") !== null) throw new Error("delete localStorage.prop failed");
            if (localStorage.length !== 1) throw new Error("localStorage.length after removals should be 1, got: " + localStorage.length);
        "##;
        rt_storage.execute_script(ls_script).expect("localStorage test failed");

        // Persist localStorage to file
        rt_storage
            .persist_local_storage(&storage_file)
            .expect("Failed to persist localStorage to file");
        assert!(storage_file.exists(), "Storage file was not written to disk");

        // Create a new fresh runtime simulating browser restart, and load from file
        let new_shared_ls = Arc::new(Mutex::new(HashMap::new()));
        let mut rt_restored = JsRuntime::new_with_url_storage_session_and_cookies(
            parse_html(html),
            800.0,
            600.0,
            "https://example.com:8080/restored",
            new_shared_ls.clone(),
            Arc::new(Mutex::new(HashMap::new())),
            cookies.clone(),
        );
        let restored_count = rt_restored
            .load_local_storage(&storage_file)
            .expect("Failed to load localStorage from file");
        assert!(restored_count > 0, "No entries restored from file");

        let verify_ls_restored = r##"
            if (localStorage.getItem("user") !== "Alice") {
                throw new Error("Restored localStorage missing 'user' key: " + localStorage.getItem("user"));
            }
        "##;
        rt_restored
            .execute_script(verify_ls_restored)
            .expect("Restored localStorage verification failed");
        let _ = std::fs::remove_file(&storage_file);

        // 4. Test sessionStorage (Tab-scoped, in-memory, isolated between tabs)
        let ss_tab1_script = r##"
            sessionStorage.clear();
            if (sessionStorage.length !== 0) throw new Error("sessionStorage should be empty");
            sessionStorage.setItem("tab_token", "tab1_secret_token_123");
            sessionStorage.proxyKey = "proxyVal";
            if (sessionStorage.getItem("tab_token") !== "tab1_secret_token_123") throw new Error("sessionStorage getItem failed");
            if (sessionStorage.proxyKey !== "proxyVal") throw new Error("sessionStorage proxy get failed");
            if (sessionStorage.length !== 2) throw new Error("sessionStorage.length should be 2, got: " + sessionStorage.length);
        "##;
        rt_storage
            .execute_script(ss_tab1_script)
            .expect("sessionStorage tab 1 script failed");

        // Simulate page navigation in Tab 1: new runtime instance with SAME tab1_ss
        let mut rt_tab1_navigated = JsRuntime::new_with_url_storage_session_and_cookies(
            parse_html(html),
            800.0,
            600.0,
            "https://example.com:8080/navigated_in_tab1",
            shared_ls.clone(),
            tab1_ss.clone(), // Same tab-scoped session storage
            cookies.clone(),
        );
        let ss_navigated_script = r##"
            if (sessionStorage.getItem("tab_token") !== "tab1_secret_token_123") {
                throw new Error("sessionStorage did not persist across navigation in same tab: " + sessionStorage.getItem("tab_token"));
            }
            if (sessionStorage.proxyKey !== "proxyVal") {
                throw new Error("sessionStorage proxy property did not persist in same tab");
            }
        "##;
        rt_tab1_navigated
            .execute_script(ss_navigated_script)
            .expect("sessionStorage navigation in same tab failed");

        // Simulate Tab 2: new runtime instance with SEPARATE new session storage
        let tab2_ss = Arc::new(Mutex::new(HashMap::new()));
        let mut rt_tab2 = JsRuntime::new_with_url_storage_session_and_cookies(
            parse_html(html),
            800.0,
            600.0,
            "https://example.com:8080/tab2_page",
            shared_ls.clone(),
            tab2_ss.clone(), // Independent tab session storage
            cookies.clone(),
        );
        let ss_tab2_script = r##"
            if (sessionStorage.getItem("tab_token") !== null) {
                throw new Error("sessionStorage leaked from Tab 1 to Tab 2!");
            }
            if (sessionStorage.length !== 0) {
                throw new Error("Tab 2 sessionStorage should be empty, got length: " + sessionStorage.length);
            }
        "##;
        rt_tab2
            .execute_script(ss_tab2_script)
            .expect("sessionStorage Tab 2 isolation check failed");
    }
}





