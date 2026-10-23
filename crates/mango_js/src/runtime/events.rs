use crate::runtime::{EventModifiers, JsRuntime};

impl JsRuntime {
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
            Some(id) => mango_html::dom::node_id_to_js(id).to_string(),
            None => "null".to_string(),
        };
        let script = format!(
            "(function() {{ if (typeof globalThis._dispatchInternalEvent === 'function') {{ return globalThis._dispatchInternalEvent({}, {:?}, {}); }} return true; }})()",
            node_id_js,
            event_type,
            if event_dict_json.trim().is_empty() {
                "{}"
            } else {
                event_dict_json
            }
        );
        self.eval_bool_script(&script)
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
            client_x,
            client_y,
            button,
            buttons,
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
            modifiers.meta
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
            client_x,
            client_y,
            buttons,
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
            modifiers.meta
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
            client_x,
            client_y,
            button,
            buttons,
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
            modifiers.meta
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
            client_x,
            client_y,
            button,
            buttons,
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
            modifiers.meta
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
            key,
            code,
            key_code,
            key_code,
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
            modifiers.meta
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
            key,
            code,
            key_code,
            key_code,
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
            modifiers.meta
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
        let _ = self.execute_script(
            r#"(function() {
                if (typeof document !== 'undefined') {
                    document.readyState = "interactive";
                    if (typeof document.dispatchEvent === 'function') {
                        try { document.dispatchEvent(new Event("readystatechange")); } catch(e) {}
                    }
                }
            })()"#,
        );
        let dict = r#"{"bubbles":true,"cancelable":false}"#;
        self.dispatch_event(None, "DOMContentLoaded", dict)
    }

    /// Dispatches the `load` event on `window`.
    pub fn dispatch_load(&mut self) -> Result<bool, String> {
        let _ = self.execute_script(
            r#"(function() {
                if (typeof document !== 'undefined') {
                    document.readyState = "complete";
                    if (typeof document.dispatchEvent === 'function') {
                        try { document.dispatchEvent(new Event("readystatechange")); } catch(e) {}
                    }
                    if (typeof globalThis._invokeConnectedCallback === 'function' && document.body) {
                        try { globalThis._invokeConnectedCallback(document.body); } catch(e) {}
                    }
                }
            })()"#,
        );
        let script = r#"(function() {
            if (typeof globalThis._dispatchInternalEvent === 'function') {
                return globalThis._dispatchInternalEvent(-1, 'load', { bubbles: false, cancelable: false });
            }
            return true;
        })()"#;
        self.eval_bool_script(script)
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
        self.eval_bool_script(&script)
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
        self.eval_bool_script(&script)
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
                if guard > 512 {
                    break;
                }
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
            if !new_chain.contains(nid)
                && let Some(node) = doc.get_mut(*nid)
                && let mango_html::dom::NodeData::Element(el) = &mut node.data
            {
                el.is_hovered = false;
                el.attributes
                    .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-hover"));
            }
        }
        // Add to newly hovered
        for nid in &new_chain {
            if !self.hovered_nodes.contains(nid)
                && let Some(node) = doc.get_mut(*nid)
                && let mango_html::dom::NodeData::Element(el) = &mut node.data
            {
                el.is_hovered = true;
                el.attributes
                    .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-hover"));
                el.attributes
                    .push(("data-mango-hover".to_string(), "true".to_string()));
            }
        }
        self.hovered_nodes = new_chain;
        *self.dom_dirty.borrow_mut() = true;
        true
    }

    /// Sets the currently active (mouse pressed) DOM element by NodeId (or None if released).
    /// Updates `is_active = true` on the element and all of its ancestor elements,
    /// and clears `is_active` from elements that are no longer active.
    /// Returns `true` if the active state changed (and thus CSS `:active` rules will re-evaluate).
    pub fn set_active_state(&mut self, target_node_id: Option<mango_html::dom::NodeId>) -> bool {
        let mut new_chain = std::collections::HashSet::new();
        if let Some(target) = target_node_id {
            let doc = self.document.borrow();
            let mut cur = Some(target);
            let mut guard = 0;
            while let Some(nid) = cur {
                if guard > 512 {
                    break;
                }
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
            if !new_chain.contains(nid)
                && let Some(node) = doc.get_mut(*nid)
                && let mango_html::dom::NodeData::Element(el) = &mut node.data
            {
                el.is_active = false;
                el.attributes
                    .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-active"));
            }
        }
        // Add to newly active
        for nid in &new_chain {
            if !self.active_nodes.contains(nid)
                && let Some(node) = doc.get_mut(*nid)
                && let mango_html::dom::NodeData::Element(el) = &mut node.data
            {
                el.is_active = true;
                el.attributes
                    .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-active"));
                el.attributes
                    .push(("data-mango-active".to_string(), "true".to_string()));
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
