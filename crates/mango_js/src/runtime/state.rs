use mango_html::dom::Document;

use crate::console::ConsoleMessage;
use crate::dom_bindings;
use crate::runtime::JsRuntime;
use crate::web_apis;

impl JsRuntime {
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

    /// Publishes layout margins (DOM node id → `[top, right, bottom, left]`) so that
    /// `getComputedStyle()` returns the used margin lengths in px.
    pub fn set_layout_margins(&self, margins: std::collections::HashMap<u32, [f32; 4]>) {
        *self.layout_margins.borrow_mut() = margins;
    }

    /// Sets an on-demand layout provider callback that is invoked automatically
    /// when measurement APIs query geometry while the DOM/style tree is dirty.
    pub fn set_layout_callback<F>(&self, cb: F)
    where
        F: Fn(&Document) -> std::collections::HashMap<u32, [f32; 4]> + 'static,
    {
        *self.layout_callback.borrow_mut() = Some(Box::new(cb));
    }

    /// Sets cross-origin isolation status directly on the runtime.
    pub fn set_cross_origin_isolated(&mut self, isolated: bool) {
        let _ = self.execute_script(&format!("globalThis.__setCrossOriginIsolated({isolated});"));
    }

    /// Configures Cross-Origin-Opener-Policy (COOP) and Cross-Origin-Embedder-Policy (COEP)
    /// headers, updating `crossOriginIsolated` per the HTML specification.
    pub fn set_cross_origin_isolation_headers(&mut self, coop: &str, coep: &str) {
        let _ = self.execute_script(&format!(
            "globalThis.__setCrossOriginIsolationHeaders('{coop}', '{coep}');"
        ));
    }

    /// Returns whether the JavaScript environment is currently cross-origin isolated.
    pub fn is_cross_origin_isolated(&mut self) -> bool {
        let _guard = self.enter_isolate();
        let val = self
            .runtime
            .execute_script("<coi>", "globalThis.crossOriginIsolated");
        match val {
            Ok(global) => {
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                let local = deno_core::v8::Local::new(scope, global);
                local.is_true()
            }
            Err(_) => false,
        }
    }

    /// Returns true if the DOM has been mutated since the last check.
    pub fn is_dom_dirty(&self) -> bool {
        *self.dom_dirty.borrow()
    }

    /// Clears the DOM dirty flag.
    pub fn clear_dom_dirty(&self) {
        *self.dom_dirty.borrow_mut() = false;
    }

    /// Returns console messages captured since the last drain.
    pub fn drain_console_messages(&self) -> Vec<ConsoleMessage> {
        let mut buf = self.console_buffer.borrow_mut();
        let messages = buf.clone();
        buf.clear();
        messages
    }

    /// Drains and returns all captured console messages.
    pub fn take_console_messages(&self) -> Vec<crate::console::ConsoleMessage> {
        std::mem::take(&mut *self.console_buffer.borrow_mut())
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
    ///
    /// Prefer [`document_ref()`] when you only need read access to avoid an O(N) deep clone.
    #[deprecated(
        since = "0.2.0",
        note = "Prefer document_ref() for read-only access or document_ref().clone() if ownership is strictly required"
    )]
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

    /// Takes any pending window open request triggered by `window.open(url, target, features)`.
    pub fn take_pending_open_window(&mut self) -> Option<web_apis::PendingWindowOpen> {
        self.pending_open.borrow_mut().take()
    }

    /// Takes any pending scroll request triggered by `scrollIntoView()` or `window.scrollTo()`.
    pub fn take_pending_scroll(&mut self) -> Option<web_apis::PendingScroll> {
        self.pending_scroll.borrow_mut().take()
    }

    /// Updates the window scroll coordinates in the JavaScript runtime.
    pub fn set_window_scroll(&mut self, x: f32, y: f32) {
        let code = format!(
            "if (typeof _setWinScroll === 'function') _setWinScroll({}, {}, true);",
            x, y
        );
        let _ = self.execute_script(&code);
    }

    /// Sets an attribute on a node by NodeId and marks DOM as dirty.
    pub fn set_node_attribute(
        &mut self,
        node_id: mango_html::dom::NodeId,
        name: &str,
        value: &str,
    ) {
        let mut doc = self.document.borrow_mut();
        if let Some(node) = doc.get_mut(node_id)
            && let mango_html::dom::NodeData::Element(elem) = &mut node.data
        {
            if let Some((_, v)) = elem
                .attributes
                .iter_mut()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
            {
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
            elem.attributes
                .retain(|(k, _)| !k.eq_ignore_ascii_case(name));
            *self.dom_dirty.borrow_mut() = true;
        }
    }

    /// Gets an attribute value from a node by NodeId.
    pub fn get_node_attribute(
        &self,
        node_id: mango_html::dom::NodeId,
        name: &str,
    ) -> Option<String> {
        let doc = self.document.borrow();
        doc.get(node_id).and_then(|node| {
            if let mango_html::dom::NodeData::Element(elem) = &node.data {
                elem.get_attribute(name).map(|s| s.to_string())
            } else {
                None
            }
        })
    }
}
