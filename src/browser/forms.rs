//! HTML Form submission, validation, view-source, PDF export, and accessibility tree.

use super::BrowserChrome;
use crate::navigation::{html_escape, url_encode};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_net::Url;

/// Truncates a string to at most `max` characters with an ASCII ellipsis.
pub fn truncate_label(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(3)).collect();
    format!("{}...", cut)
}

/// Recursively searches for an element with the given `id` attribute.
pub fn find_element_by_id(doc: &Document, target_id: &str) -> Option<NodeId> {
    fn search(doc: &Document, node_id: NodeId, target_id: &str) -> Option<NodeId> {
        if let Some(node) = doc.get(node_id)
            && let NodeData::Element(ref elem) = node.data
            && elem.id() == Some(target_id)
        {
            return Some(node_id);
        }
        for child in doc.children(node_id) {
            if let Some(found) = search(doc, child.id, target_id) {
                return Some(found);
            }
        }
        None
    }
    search(doc, doc.root(), target_id)
}

impl BrowserChrome {
    /// Submits the HTML form enclosing or associated with `control_node_id`.
    pub fn submit_form(&mut self, control_node_id: NodeId) {
        let doc = self.active_document();

        let mut curr = Some(control_node_id);
        let mut form_node = None;
        while let Some(nid) = curr {
            if let Some(node) = doc.get(nid) {
                if let NodeData::Element(el) = &node.data
                    && el.tag_name.eq_ignore_ascii_case("form")
                {
                    form_node = Some(nid);
                    break;
                }
                curr = node.parent;
            } else {
                break;
            }
        }

        let form_id = match form_node {
            Some(f) => f,
            None => {
                if let Some(node) = doc.get(control_node_id)
                    && let NodeData::Element(el) = &node.data
                    && let Some(target_form_id) = el.get_attribute("form")
                    && let Some(fid) = find_element_by_id(&doc, target_form_id)
                {
                    fid
                } else {
                    return;
                }
            }
        };

        let (action, method) = if let Some(node) = doc.get(form_id) {
            if let NodeData::Element(el) = &node.data {
                let a = el.get_attribute("action").unwrap_or("").to_string();
                let m = el
                    .get_attribute("method")
                    .unwrap_or("GET")
                    .to_ascii_uppercase();
                (a, m)
            } else {
                ("".to_string(), "GET".to_string())
            }
        } else {
            ("".to_string(), "GET".to_string())
        };

        let mut form_values = Vec::new();
        let mut stack = vec![form_id];
        while let Some(nid) = stack.pop() {
            if let Some(node) = doc.get(nid) {
                if let NodeData::Element(el) = &node.data {
                    let tag = el.tag_name.to_ascii_lowercase();
                    let is_disabled = el.get_attribute("disabled").is_some();
                    if !is_disabled
                        && (tag == "input" || tag == "textarea" || tag == "select")
                        && let Some(name) = el.get_attribute("name")
                        && !name.is_empty()
                    {
                        if tag == "textarea" {
                            let val = el
                                .get_attribute("value")
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| doc.text_content(nid));
                            form_values.push((name.to_string(), val));
                        } else if tag == "select" {
                            let is_multiple = el.get_attribute("multiple").is_some();
                            let mut options = Vec::new();
                            let mut opt_stack = vec![nid];
                            while let Some(curr_id) = opt_stack.pop() {
                                if let Some(curr_node) = doc.get(curr_id) {
                                    if curr_id != nid
                                        && let NodeData::Element(opt_el) = &curr_node.data
                                        && opt_el.tag_name.eq_ignore_ascii_case("option")
                                    {
                                        let opt_disabled =
                                            opt_el.get_attribute("disabled").is_some();
                                        let opt_selected =
                                            opt_el.get_attribute("selected").is_some();
                                        let opt_val = opt_el
                                            .get_attribute("value")
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| {
                                                doc.text_content(curr_id).trim().to_string()
                                            });
                                        options.push((opt_disabled, opt_selected, opt_val));
                                    }
                                    let mut child = curr_node.first_child;
                                    let mut children = Vec::new();
                                    while let Some(cid) = child {
                                        children.push(cid);
                                        if let Some(cnode) = doc.get(cid) {
                                            child = cnode.next_sibling;
                                        } else {
                                            break;
                                        }
                                    }
                                    for c in children.into_iter().rev() {
                                        opt_stack.push(c);
                                    }
                                }
                            }
                            if is_multiple {
                                for (opt_disabled, opt_selected, opt_val) in options {
                                    if !opt_disabled && opt_selected {
                                        form_values.push((name.to_string(), opt_val));
                                    }
                                }
                            } else {
                                let selected = options
                                    .iter()
                                    .find(|(d, s, _)| !d && *s)
                                    .or_else(|| options.iter().find(|(d, _, _)| !d));
                                if let Some((_, _, opt_val)) = selected {
                                    form_values.push((name.to_string(), opt_val.clone()));
                                } else {
                                    let val = el.get_attribute("value").unwrap_or("").to_string();
                                    form_values.push((name.to_string(), val));
                                }
                            }
                        } else {
                            let input_type = el
                                .get_attribute("type")
                                .unwrap_or("text")
                                .to_ascii_lowercase();
                            if input_type == "button"
                                || input_type == "reset"
                                || input_type == "image"
                            {
                                // Omitted
                            } else if input_type == "submit" {
                                if nid == control_node_id {
                                    let val = el.get_attribute("value").unwrap_or("").to_string();
                                    form_values.push((name.to_string(), val));
                                }
                            } else if input_type == "checkbox" || input_type == "radio" {
                                let is_checked = el.get_attribute("checked").is_some();
                                if is_checked {
                                    let val = el.get_attribute("value").unwrap_or("on").to_string();
                                    form_values.push((name.to_string(), val));
                                }
                            } else {
                                let val = el.get_attribute("value").unwrap_or("").to_string();
                                form_values.push((name.to_string(), val));
                            }
                        }
                    }
                }
                let mut child = node.first_child;
                let mut children = Vec::new();
                while let Some(cid) = child {
                    children.push(cid);
                    if let Some(cnode) = doc.get(cid) {
                        child = cnode.next_sibling;
                    } else {
                        break;
                    }
                }
                for c in children.into_iter().rev() {
                    stack.push(c);
                }
            }
        }

        let mut query_parts = Vec::new();
        for (k, v) in form_values {
            let enc_k = url_encode(&k);
            let enc_v = url_encode(&v);
            query_parts.push(format!("{}={}", enc_k, enc_v));
        }
        let query_str = query_parts.join("&");

        let novalidate = if let Some(node) = doc.get(form_id) {
            if let NodeData::Element(el) = &node.data {
                el.get_attribute("novalidate").is_some()
            } else {
                false
            }
        } else {
            false
        };
        let submitter_novalidate = doc
            .get(control_node_id)
            .map(|node| {
                if let NodeData::Element(el) = &node.data {
                    el.get_attribute("formnovalidate").is_some()
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if !novalidate && !submitter_novalidate {
            let mut invalid_control = None;
            let mut val_stack = vec![form_id];
            while let Some(nid) = val_stack.pop() {
                if let Some(node) = doc.get(nid) {
                    if let NodeData::Element(el) = &node.data {
                        let tag = el.tag_name.to_ascii_lowercase();
                        if (tag == "input" || tag == "textarea" || tag == "select")
                            && !mango_css::selectors::element_is_valid(nid, el)
                        {
                            invalid_control = Some((nid, el.clone()));
                            break;
                        }
                    }
                    let mut child = node.first_child;
                    let mut children = Vec::new();
                    while let Some(cid) = child {
                        children.push(cid);
                        if let Some(cnode) = doc.get(cid) {
                            child = cnode.next_sibling;
                        } else {
                            break;
                        }
                    }
                    for c in children.into_iter().rev() {
                        val_stack.push(c);
                    }
                }
            }

            if let Some((inval_id, inval_el)) = invalid_control {
                self.focused_control = Some(inval_id);
                let msg = if inval_el.get_attribute("required").is_some() {
                    "Please fill out this field."
                } else if inval_el.get_attribute("pattern").is_some() {
                    "Please match the requested format."
                } else {
                    "Please provide a valid value."
                };
                self.status_text = msg.to_string();
                log::info!(
                    "Form submission blocked: control {:?} is invalid ({})",
                    inval_id,
                    msg
                );
                return;
            }
        }

        let resolved_action = if action.is_empty() {
            self.address_text.clone()
        } else if let Some(base) = &self.base_url {
            base.resolve(&action)
                .map(|u| u.to_string())
                .unwrap_or(action)
        } else if action.starts_with('/') {
            format!("https://html.duckduckgo.com{}", action)
        } else {
            action
        };

        if method == "POST" {
            log::info!("Form submitted (POST): sending to {}", resolved_action);
            if let Ok(parsed_url) = Url::parse(&resolved_action) {
                self.status_text = format!("Submitting to {}...", parsed_url);
                self.set_tab_loading(self.active_tab_idx, true);
                self.address_focused = false;
                self.nav_actor.post(
                    self.active_tab_idx,
                    parsed_url,
                    query_str.into_bytes(),
                    "application/x-www-form-urlencoded".to_string(),
                    true,
                );
                return;
            }
        }

        let target_url = if query_str.is_empty() {
            resolved_action
        } else if resolved_action.contains('?') {
            format!("{}&{}", resolved_action, query_str)
        } else {
            format!("{}?{}", resolved_action, query_str)
        };

        log::info!("Form submitted ({}): navigating to {}", method, target_url);
        self.navigate(&target_url);
    }

    /// Opens the page source in a new tab.
    pub fn open_view_source(&mut self) {
        let source_html = format!(
            "<!DOCTYPE html><html><head><title>Source: {}</title><style>body {{ background-color: #1e1e1e; color: #d4d4d4; font-family: monospace; font-size: 13px; padding: 16px; margin: 0; line-height: 1.5; }}</style></head><body><h2>View Source: {}</h2><hr/><p>{}</p></body></html>",
            self.address_text,
            self.address_text,
            html_escape(&self.current_html)
        );
        self.new_tab();
        self.load_html(source_html, format!("view-source:{}", self.address_text));
    }

    /// Exports the current page to a PDF document.
    pub fn print_to_pdf(
        &self,
        path: &std::path::Path,
        options: Option<mango_render::PdfOptions>,
    ) -> std::io::Result<()> {
        let opts = options.unwrap_or_default();
        mango_render::print_to_pdf(&self.page_display_list, path, &opts)
    }

    /// Builds the accessibility tree for the currently loaded document.
    pub fn accessibility_tree(&self) -> mango_layout::a11y::A11yTree {
        let doc = self.document_view();
        mango_layout::a11y::A11yTree::build(&doc, self.root_box.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_label() {
        assert_eq!(truncate_label("hello", 10), "hello");
        assert_eq!(truncate_label("hello", 5), "hello");
        assert_eq!(truncate_label("hello world", 8), "hello...");
        assert_eq!(truncate_label("12345678", 8), "12345678");
        assert_eq!(truncate_label("hello", 2), "...");
        assert_eq!(truncate_label("hello", 3), "...");
        assert_eq!(truncate_label("こんにちは世界", 5), "こん...");
        assert_eq!(truncate_label("こんにちは", 5), "こんにちは");
    }
}
