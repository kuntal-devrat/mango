//! Find-in-page search and DOM node hover/active state tracking.

use super::super::BrowserChrome;

impl BrowserChrome {
    /// Toggles the Find-in-Page search bar overlay (Ctrl+F).
    pub fn toggle_find(&mut self) {
        self.find_in_page.active = !self.find_in_page.active;
        if self.find_in_page.active {
            self.address_focused = false;
            self.clear_form_focus();
            if !self.find_in_page.query.is_empty() {
                let q = self.find_in_page.query.clone();
                self.find_in_page_search(&q);
            }
        } else {
            self.find_in_page.matches.clear();
        }
        self.page_dirty = true;
    }

    /// Searches the page text for `query` and updates matching bounding boxes.
    pub fn find_in_page_search(&mut self, query: &str) {
        self.find_in_page.query = query.to_string();
        self.find_in_page.matches.clear();
        self.find_in_page.current_match = 0;

        let trimmed = query.trim();
        if trimmed.is_empty() {
            self.page_dirty = true;
            return;
        }

        let query_lower = trimmed.to_lowercase();
        let mut matches = Vec::new();

        if let Some(ref root) = self.root_box {
            fn collect_matches(
                b: &mango_layout::box_tree::LayoutBox,
                q: &str,
                out: &mut Vec<mango_core::Rect>,
            ) {
                if let mango_layout::box_model::BoxType::TextNode(ref text) = b.box_type {
                    let text_lower = text.to_lowercase();
                    let mut start = 0;
                    let total_chars = text.chars().count().max(1) as f32;
                    let bbox = b.dimensions.border_box();
                    while let Some(pos) = text_lower[start..].find(q) {
                        let absolute_byte_idx = start + pos;
                        let char_start = text[..absolute_byte_idx].chars().count();
                        let char_len = q.chars().count();
                        let start_frac = (char_start as f32 / total_chars).clamp(0.0, 1.0);
                        let len_frac = (char_len as f32 / total_chars).clamp(0.01, 1.0);
                        let mx = bbox.x() + bbox.width() * start_frac;
                        let mw = (bbox.width() * len_frac).max(6.0);
                        out.push(mango_core::Rect::new(
                            mx,
                            bbox.y(),
                            mw,
                            bbox.height().max(12.0),
                        ));
                        start = absolute_byte_idx + q.len();
                        if start >= text_lower.len() {
                            break;
                        }
                    }
                }
                for child in &b.children {
                    collect_matches(child, q, out);
                }
            }
            collect_matches(root, &query_lower, &mut matches);
        }

        matches.sort_by(|a, b| {
            if (a.y() - b.y()).abs() > 4.0 {
                a.y()
                    .partial_cmp(&b.y())
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.x()
                    .partial_cmp(&b.x())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        if !matches.is_empty() {
            let target_y = (matches[0].y() - 100.0).max(0.0).min(self.max_scroll());
            self.scroll_y = target_y;
            if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.scroll_y = self.scroll_y;
            }
        }

        self.find_in_page.matches = matches;
        self.page_dirty = true;
    }

    /// Navigates to the next search match.
    pub fn find_next(&mut self) {
        if self.find_in_page.matches.is_empty() {
            return;
        }
        self.find_in_page.current_match =
            (self.find_in_page.current_match + 1) % self.find_in_page.matches.len();
        let target_match = self.find_in_page.matches[self.find_in_page.current_match];
        let target_y = (target_match.y() - 100.0).max(0.0).min(self.max_scroll());
        self.scroll_y = target_y;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.scroll_y = self.scroll_y;
        }
        self.page_dirty = true;
    }

    /// Navigates to the previous search match.
    pub fn find_previous(&mut self) {
        if self.find_in_page.matches.is_empty() {
            return;
        }
        self.find_in_page.current_match =
            (self.find_in_page.current_match + self.find_in_page.matches.len() - 1)
                % self.find_in_page.matches.len();
        let target_match = self.find_in_page.matches[self.find_in_page.current_match];
        let target_y = (target_match.y() - 100.0).max(0.0).min(self.max_scroll());
        self.scroll_y = target_y;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.scroll_y = self.scroll_y;
        }
        self.page_dirty = true;
    }

    /// Sets the currently hovered DOM element, updating `data-mango-hover` in the active document.
    /// Returns `true` if the hovered element chain changed.
    pub fn set_hover_node(&mut self, node_id: Option<mango_html::dom::NodeId>) -> bool {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_hover_state(node_id)
        } else if let Some(ref mut doc) = self.cached_document {
            let mut new_chain = std::collections::HashSet::new();
            if let Some(target) = node_id {
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
            if new_chain == self.hovered_dom_nodes {
                return false;
            }
            for nid in &self.hovered_dom_nodes {
                if !new_chain.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-hover"));
                }
            }
            for nid in &new_chain {
                if !self.hovered_dom_nodes.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .push(("data-mango-hover".to_string(), "true".to_string()));
                }
            }
            self.hovered_dom_nodes = new_chain;
            true
        } else {
            false
        }
    }

    /// Sets the currently active (mouse pressed) DOM element, updating `data-mango-active` in the active document.
    /// Returns `true` if the active element chain changed.
    pub fn set_active_node(&mut self, node_id: Option<mango_html::dom::NodeId>) -> bool {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_active_state(node_id)
        } else if let Some(ref mut doc) = self.cached_document {
            let mut new_chain = std::collections::HashSet::new();
            if let Some(target) = node_id {
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
            if new_chain == self.active_dom_nodes {
                return false;
            }
            for nid in &self.active_dom_nodes {
                if !new_chain.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-active"));
                }
            }
            for nid in &new_chain {
                if !self.active_dom_nodes.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .push(("data-mango-active".to_string(), "true".to_string()));
                }
            }
            self.active_dom_nodes = new_chain;
            true
        } else {
            false
        }
    }
}
