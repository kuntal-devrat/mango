//! In-page anchor jumping, element scrollIntoView, and JS effect draining.

use mango_html::dom::{Document, NodeData, NodeId};

use super::super::BrowserChrome;
use super::super::forms::find_element_by_id;

/// Decodes percent-encoded bytes in a URL fragment (e.g. `%20` -> ` `).
pub fn percent_decode(s: &str) -> String {
    let mut bytes = Vec::new();
    let input = s.as_bytes();
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            if let Ok(b) =
                u8::from_str_radix(std::str::from_utf8(&input[i + 1..=i + 2]).unwrap_or(""), 16)
            {
                bytes.push(b);
                i += 3;
                continue;
            }
        }
        bytes.push(input[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Recursively searches for an `<a>` element with matching `name` attribute.
pub fn find_element_by_anchor_name(doc: &Document, target_name: &str) -> Option<NodeId> {
    fn search(doc: &Document, node_id: NodeId, target_name: &str) -> Option<NodeId> {
        if let Some(node) = doc.get(node_id)
            && let NodeData::Element(ref elem) = node.data
            && elem.tag_name.eq_ignore_ascii_case("a")
            && elem.get_attribute("name") == Some(target_name)
        {
            return Some(node_id);
        }
        for child in doc.children(node_id) {
            if let Some(found) = search(doc, child.id, target_name) {
                return Some(found);
            }
        }
        None
    }
    search(doc, doc.root(), target_name)
}

/// Finds the target DOM element for a URL fragment identifier per HTML §7.4.2.
pub fn find_anchor_target(doc: &Document, frag: &str) -> Option<NodeId> {
    if frag.is_empty() || frag.eq_ignore_ascii_case("top") {
        return None;
    }
    let decoded = percent_decode(frag);

    // 1. Match exact id
    if let Some(id) = find_element_by_id(doc, frag) {
        return Some(id);
    }
    // 2. Match decoded id (if different)
    if decoded != frag
        && let Some(id) = find_element_by_id(doc, &decoded)
    {
        return Some(id);
    }

    // 3. Match <a name="frag">
    if let Some(id) = find_element_by_anchor_name(doc, frag) {
        return Some(id);
    }
    // 4. Match <a name="decoded">
    if decoded != frag
        && let Some(id) = find_element_by_anchor_name(doc, &decoded)
    {
        return Some(id);
    }

    None
}

impl BrowserChrome {
    /// Scrolls the viewport to bring `node_id` into view.
    pub fn scroll_to_element(&mut self, node_id: NodeId, smooth: bool) {
        let target_y = if let Some(root) = &self.root_box
            && let Some(lbox) = root.find_box_for_node(node_id)
        {
            lbox.dimensions.border_box().y()
        } else {
            return;
        };
        self.scroll_to_y(target_y, smooth);
    }

    /// Scrolls the active tab to `target_y`, clamping to `[0, max_scroll]`.
    pub fn scroll_to_y(&mut self, target_y: f32, smooth: bool) {
        let max_s = self.max_scroll();
        let clamped_y = target_y.clamp(0.0, max_s);
        if smooth {
            self.scroll_smooth_to(clamped_y, None);
        } else {
            self.smooth_scroll = None;
            self.scroll_y = clamped_y;
            self.scroll_y_f64 = clamped_y as f64;
            if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.scroll_y = self.scroll_y;
                tab.scroll_y_f64 = self.scroll_y_f64;
            }
            if let Some(ref mut rt) = self.js_runtime {
                rt.set_window_scroll(0.0, self.scroll_y);
            }
            self.display_list_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            self.page_dirty = true;
        }
    }

    /// Scrolls the active tab to the indicated anchor fragment.
    pub fn scroll_to_anchor_fragment(&mut self, frag: &str) {
        if frag.is_empty() || frag.eq_ignore_ascii_case("top") {
            self.scroll_to_y(0.0, false);
            return;
        }
        let doc = self.active_document();
        if let Some(node_id) = find_anchor_target(&doc, frag) {
            self.scroll_to_element(node_id, false);
        }
    }

    /// Drains any pending JS effects (redirects, window open, pending scroll, alerts).
    pub(crate) fn drain_js_effects(&mut self) {
        let (redirect, open_req, pending_scroll, alert_msg) =
            if let Some(ref mut rt) = self.js_runtime {
                let redirect = rt.take_pending_navigation();
                let open_req = rt.take_pending_open_window();
                let pending_scroll = rt.take_pending_scroll();
                let alert_msg = rt.take_status_text();
                (redirect, open_req, pending_scroll, alert_msg)
            } else {
                return;
            };

        if let Some(msg) = alert_msg {
            self.status_text = msg;
        }
        if let Some(scroll) = pending_scroll {
            if let Some(nid) = scroll.target_node {
                self.scroll_to_element(nid, scroll.smooth);
            } else {
                self.scroll_to_y(scroll.y, scroll.smooth);
            }
        }
        if let Some(redirect) = redirect {
            let nav_url = if let Some(base) = &self.base_url {
                base.resolve(&redirect)
                    .map(|u| u.to_string())
                    .unwrap_or(redirect)
            } else {
                redirect
            };
            self.navigate(&nav_url);
        }
        if let Some(open_req) = open_req {
            let nav_url = if let Some(base) = &self.base_url {
                base.resolve(&open_req.url)
                    .map(|u| u.to_string())
                    .unwrap_or(open_req.url)
            } else {
                open_req.url
            };
            self.new_tab_and_navigate(&nav_url);
        }
    }

    /// Executes a JavaScript snippet against the active page runtime and drains side effects.
    pub fn execute_script(&mut self, script: &str) -> Result<(), String> {
        if let Some(ref mut rt) = self.js_runtime {
            rt.execute_script(script)?;
        }
        self.drain_js_effects();
        Ok(())
    }
}
