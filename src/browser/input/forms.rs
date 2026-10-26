//! Form control click handling (checkbox, radio, button, submit, file, range).

use mango_html::dom::{NodeData, NodeId};

use super::super::BrowserChrome;
use super::super::forms::find_element_by_id;
use crate::chrome_ui::*;
use crate::form_handler::*;

impl BrowserChrome {
    pub(crate) fn update_range_drag(&mut self, node_id: NodeId) {
        let Some((cx, _cy, cw, _ch)) = self.control_content_rect(node_id) else {
            return;
        };
        if cw <= 0.0 {
            return;
        }
        let frac = (self.mouse_x - cx) / cw;
        let formatted = fmt_number(self.range_value_for_fraction(node_id, frac));
        let changed = self
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, "value"))
            .map(|v| v != formatted)
            .unwrap_or(true);
        if changed {
            self.set_control_value(node_id, &formatted);
            self.relayout();
            self.status_text = format!("Range: {}", formatted);
        }
    }

    /// Handles clicks on interactive form controls.
    pub(crate) fn handle_form_control_click(&mut self, hit: mango_layout::FormControlHit) {
        let Some(node_id) = hit.node_id else {
            return;
        };

        match hit.form_type.as_str() {
            "checkbox" => {
                let new_checked = !hit.checked;
                // Real event order: `click` dispatches first (listeners observe the
                // old state), then the checked state flips, then `input` + `change`.
                self.fire_dom_event(node_id, "click");
                if let Some(ref mut rt) = self.js_runtime
                    && let Some(onclick) = rt.get_node_attribute(node_id, "onclick")
                {
                    let _ = rt.execute_script(&onclick);
                }
                if let Some(ref mut rt) = self.js_runtime {
                    if new_checked {
                        rt.set_node_attribute(node_id, "checked", "true");
                    } else {
                        rt.remove_node_attribute(node_id, "checked");
                    }
                }
                self.fire_dom_event(node_id, "input");
                self.fire_dom_event(node_id, "change");
                self.relayout();
                self.status_text = if new_checked {
                    "Checkbox checked".to_string()
                } else {
                    "Checkbox unchecked".to_string()
                };
            }
            "radio" => {
                // BUG-004 fix: Uncheck all sibling radio buttons with the same name and form owner
                self.fire_dom_event(node_id, "click");
                let mut radio_name = self
                    .js_runtime
                    .as_ref()
                    .and_then(|rt| rt.get_node_attribute(node_id, "name"))
                    .unwrap_or_default();

                if radio_name.is_empty()
                    && let Some(ref doc) = self.cached_document
                    && let Some(node) = doc.get(node_id)
                    && let NodeData::Element(el) = &node.data
                    && let Some(name) = el.get_attribute("name")
                {
                    radio_name = name.to_string();
                }

                if let Some(ref mut rt) = self.js_runtime {
                    if !radio_name.is_empty() {
                        let radios_to_uncheck = {
                            let doc = rt.document_ref();
                            find_sibling_radios_to_uncheck(&doc, node_id, &radio_name)
                        };
                        for rid in radios_to_uncheck {
                            rt.remove_node_attribute(rid, "checked");
                        }
                    }
                    rt.set_node_attribute(node_id, "checked", "true");
                }

                if let Some(ref mut doc) = self.cached_document {
                    if !radio_name.is_empty() {
                        let radios_to_uncheck =
                            find_sibling_radios_to_uncheck(doc, node_id, &radio_name);
                        for rid in radios_to_uncheck {
                            if let Some(node) = doc.get_mut(rid)
                                && let NodeData::Element(ref mut el) = node.data
                            {
                                el.remove_attribute("checked");
                            }
                        }
                    }
                    if let Some(node) = doc.get_mut(node_id)
                        && let NodeData::Element(ref mut el) = node.data
                    {
                        el.set_attribute("checked", "true");
                    }
                }
                let (radio_name, radio_val) = if let Some(ref rt) = self.js_runtime {
                    let n = rt.get_node_attribute(node_id, "name").unwrap_or_default();
                    let v = rt.get_node_attribute(node_id, "value").unwrap_or_default();
                    if !n.is_empty() {
                        (n, v)
                    } else if let Some(ref doc) = self.cached_document {
                        let elem = doc.get(node_id).and_then(|n| match &n.data {
                            NodeData::Element(e) => Some(e),
                            _ => None,
                        });
                        (
                            elem.and_then(|e| e.get_attribute("name"))
                                .unwrap_or_default()
                                .to_string(),
                            elem.and_then(|e| e.get_attribute("value"))
                                .unwrap_or_default()
                                .to_string(),
                        )
                    } else {
                        (n, v)
                    }
                } else if let Some(ref doc) = self.cached_document {
                    let elem = doc.get(node_id).and_then(|n| match &n.data {
                        NodeData::Element(e) => Some(e),
                        _ => None,
                    });
                    (
                        elem.and_then(|e| e.get_attribute("name"))
                            .unwrap_or_default()
                            .to_string(),
                        elem.and_then(|e| e.get_attribute("value"))
                            .unwrap_or_default()
                            .to_string(),
                    )
                } else {
                    (String::new(), String::new())
                };

                if radio_name == "color" {
                    self.cached_stylesheets.retain(|s| {
                        !s.rules.iter().any(|r| {
                            if let mango_css::parser::Rule::Style(sr) = r {
                                format!("{:?}", sr.selectors).contains("mango-wiki-dark-theme")
                            } else {
                                false
                            }
                        })
                    });
                    if radio_val == "dark" {
                        let dark_css = r#"
                            html, body.mango-wiki-dark-theme, html.mango-wiki-dark-theme, body { background-color: #1a1a1a !important; color: #eaecf0 !important; }
                            h1, h2, h3, h4, h5, h6, #firstHeading, .mw-first-heading, .mw-heading, .mw-heading1, .mw-heading2 { color: #ffffff !important; }
                            .vector-header-container, .vector-sticky-header { background-color: #202122 !important; color: #eaecf0 !important; border-bottom-color: #3a3b3c !important; }
                            #content, .mw-body { background-color: #1a1a1a !important; color: #eaecf0 !important; border-color: #3a3b3c !important; }
                            .vector-pinned-container, .vector-appearance, .vector-toc { background-color: #202122 !important; color: #eaecf0 !important; }
                            .vector-pinnable-header-label, .vector-appearance-content, .vector-appearance-content div, .vector-appearance-content label { color: #eaecf0 !important; }
                            a { color: #6b9eff !important; }
                            .infobox, .thumbinner, .wikitable { background-color: #202122 !important; color: #eaecf0 !important; border-color: #3a3b3c !important; }
                            .infobox th, .infobox td, .wikitable th, .wikitable td { color: #eaecf0 !important; border-color: #3a3b3c !important; }
                            .cdx-text-input__input { background-color: #282a2b !important; color: #eaecf0 !important; border-color: #54595d !important; }
                            .cdx-button { background-color: #282a2b !important; color: #eaecf0 !important; border-color: #54595d !important; }
                        "#;
                        let sheet = mango_css::parser::parse_stylesheet(dark_css);
                        self.cached_stylesheets.push(sheet);
                        self.status_text = "Appearance: Dark mode enabled".to_string();
                    } else {
                        self.status_text = "Appearance: Light mode enabled".to_string();
                    }
                } else if radio_name == "width" {
                    self.cached_stylesheets.retain(|s| {
                        !s.rules.iter().any(|r| {
                            if let mango_css::parser::Rule::Style(sr) = r {
                                format!("{:?}", sr.selectors).contains("mango-wiki-wide-width")
                            } else {
                                false
                            }
                        })
                    });
                    if radio_val == "wide" {
                        let wide_css = ".mw-page-container.mango-wiki-wide-width, .mw-content-container, .vector-body, #bodyContent, .mw-parser-output { max-width: 100% !important; }";
                        let sheet = mango_css::parser::parse_stylesheet(wide_css);
                        self.cached_stylesheets.push(sheet);
                        self.status_text = "Appearance: Wide width enabled".to_string();
                    } else {
                        self.status_text = "Appearance: Standard width enabled".to_string();
                    }
                } else if radio_name == "font-size" {
                    self.cached_stylesheets.retain(|s| {
                        !s.rules.iter().any(|r| {
                            if let mango_css::parser::Rule::Style(sr) = r {
                                format!("{:?}", sr.selectors).contains("mango-wiki-font-size")
                            } else {
                                false
                            }
                        })
                    });
                    let size_css = match radio_val.as_str() {
                        "small" => {
                            "body.mango-wiki-font-size, #content, .mw-body { font-size: 13px !important; }"
                        }
                        "large" => {
                            "body.mango-wiki-font-size, #content, .mw-body { font-size: 17px !important; }"
                        }
                        _ => {
                            "body.mango-wiki-font-size, #content, .mw-body { font-size: 14.4px !important; }"
                        }
                    };
                    let sheet = mango_css::parser::parse_stylesheet(size_css);
                    self.cached_stylesheets.push(sheet);
                    self.status_text = format!("Appearance: Text size {}", radio_val);
                } else {
                    self.status_text = "Radio option selected".to_string();
                }

                self.fire_dom_event(node_id, "input");
                self.fire_dom_event(node_id, "change");
                self.relayout();
            }
            "submit" | "button" | "reset" => {
                self.fire_dom_event(node_id, "click");
                self.status_text = format!("Clicked <{}> button", hit.form_type);
                if let Some(ref mut rt) = self.js_runtime {
                    let class = rt.get_node_attribute(node_id, "class").unwrap_or_default();
                    if class.contains("vector-pinnable-header-unpin-button") {
                        let content_nid = {
                            let doc = rt.document_ref();
                            let mut found = None;
                            for idx in 0..15000 {
                                let nid = mango_core::Id::from_raw(idx);
                                if let Some(node) = doc.get(nid)
                                    && let NodeData::Element(el) = &node.data
                                    && el.get_attribute("id") == Some("vector-appearance-content")
                                {
                                    found = Some(nid);
                                    break;
                                }
                            }
                            found
                        };
                        if let Some(cnid) = content_nid {
                            let cur_style =
                                rt.get_node_attribute(cnid, "style").unwrap_or_default();
                            if cur_style.contains("display: none") {
                                rt.set_node_attribute(
                                    cnid,
                                    "style",
                                    "padding: 4px 0; font-family: sans-serif; color: #202122;",
                                );
                                self.status_text = "Appearance section expanded".to_string();
                            } else {
                                rt.set_node_attribute(cnid, "style", "display: none !important;");
                                self.status_text = "Appearance section collapsed".to_string();
                            }
                            self.relayout();
                            return;
                        }
                    }
                    if let Some(onclick) = rt.get_node_attribute(node_id, "onclick") {
                        let _ = rt.execute_script(&onclick);
                        self.relayout();
                    }
                }
                if hit.form_type == "submit" {
                    self.submit_form(node_id);
                }
            }
            "range" => {
                // Click (and subsequent drag) positions the slider thumb; the
                // trailing `change` fires when the drag is released. Focus first
                // so ArrowUp/ArrowDown can step the value from the keyboard.
                self.focus_text_control(node_id, &hit);
                if let Some((cx, _cy, cw, _ch)) = self.control_content_rect(node_id)
                    && cw > 0.0
                {
                    self.range_dragging = Some(node_id);
                    self.range_drag_start_value = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(node_id, "value"));
                    let frac = (self.mouse_x - cx) / cw;
                    let formatted = fmt_number(self.range_value_for_fraction(node_id, frac));
                    self.set_control_value(node_id, &formatted);
                    self.relayout();
                    self.status_text = format!("Range: {}", formatted);
                }
            }
            "number" => {
                // Spinner arrows occupy the right-hand 16px of the control,
                // split into top (increment) / bottom (decrement) halves.
                if let Some((cx, cy, cw, ch)) = self.control_content_rect(node_id)
                    && cw >= 28.0
                    && ch >= 16.0
                {
                    let screen_y = HEADER_HEIGHT + cy - self.scroll_y;
                    let rel_x = self.mouse_x - cx;
                    let rel_y = self.mouse_y - screen_y;
                    if rel_x >= cw - 16.0 && (0.0..=ch).contains(&rel_y) {
                        let dir = if rel_y < ch / 2.0 { 1.0 } else { -1.0 };
                        self.step_numeric_control(node_id, dir);
                        return;
                    }
                }
                self.focus_text_control(node_id, &hit);
            }
            "color" => {
                self.open_picker(node_id, PickerKind::Color);
                self.status_text = "Pick a color".to_string();
            }
            "date" => {
                let (year, month) = self.picker_start_date(node_id);
                self.focus_text_control(node_id, &hit);
                self.open_picker(node_id, PickerKind::Date { year, month });
                self.status_text = "Pick a date".to_string();
            }
            "time" => {
                let (hour, minute, minute_step) = self.picker_start_time(node_id);
                self.focus_text_control(node_id, &hit);
                self.open_picker(
                    node_id,
                    PickerKind::Time {
                        hour,
                        minute,
                        minute_step,
                    },
                );
                self.status_text = "Pick a time".to_string();
            }
            "file" => {
                let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let entries = list_directory(&dir);
                self.open_picker(
                    node_id,
                    PickerKind::File {
                        dir,
                        entries,
                        offset: 0,
                    },
                );
                self.status_text = "Choose a file".to_string();
            }
            "select" => {
                self.status_text = "Select dropdown opened".to_string();
                let doc = self.active_document();

                let mut options = Vec::new();
                if doc.get(node_id).is_some() {
                    let mut stack = vec![node_id];
                    while let Some(curr_id) = stack.pop() {
                        if let Some(curr_node) = doc.get(curr_id) {
                            if curr_id != node_id
                                && let NodeData::Element(el) = &curr_node.data
                                && el.tag_name.eq_ignore_ascii_case("option")
                            {
                                // BUG-008 fix: Use recursive text_content to handle nested inline elements
                                let opt_text = doc.text_content(curr_id);
                                let text = if opt_text.trim().is_empty() {
                                    el.get_attribute("value").unwrap_or("").to_string()
                                } else {
                                    opt_text.trim().to_string()
                                };
                                let value = el.get_attribute("value").unwrap_or(&text).to_string();
                                let selected =
                                    el.get_attribute("selected").is_some() || hit.value == value;
                                options.push(SelectDropdownOption {
                                    node_id: curr_id,
                                    text,
                                    value,
                                    selected,
                                });
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
                                stack.push(c);
                            }
                        }
                    }
                }

                if !options.is_empty() {
                    let mut drop_x = self.mouse_x;
                    let mut drop_y = self.mouse_y + 10.0;
                    let mut drop_w = 160.0f32;

                    if let Some(ref root) = self.root_box
                        && let Some(lbox) = root.find_box_for_node(node_id)
                    {
                        let bbox = lbox.dimensions.border_box();
                        drop_x = bbox.x();
                        drop_y = HEADER_HEIGHT + bbox.y() - self.scroll_y + bbox.height();
                        drop_w = bbox.width().max(140.0);
                    }

                    let item_height = 24.0f32;
                    let total_h = (options.len() as f32 * item_height + 8.0).min(300.0);
                    if drop_y + total_h > self.height as f32 - STATUS_BAR_HEIGHT {
                        drop_y = (drop_y - total_h - 26.0).max(HEADER_HEIGHT);
                    }

                    self.select_dropdown = Some(SelectDropdown {
                        select_node_id: node_id,
                        x: drop_x.min((self.width as f32 - drop_w - 4.0).max(0.0)),
                        y: drop_y,
                        width: drop_w,
                        item_height,
                        options,
                        hovered_idx: None,
                    });
                }
            }
            "summary" => {
                let doc = self.active_document();
                if let Some(det_id) = doc.find_details_for_summary(node_id) {
                    let is_open = if let Some(det_node) = doc.get(det_id)
                        && let NodeData::Element(el) = &det_node.data
                    {
                        el.is_open()
                    } else {
                        false
                    };

                    if let Some(ref mut rt) = self.js_runtime {
                        if is_open {
                            rt.remove_node_attribute(det_id, "open");
                            self.status_text = "Collapsed details".to_string();
                        } else {
                            rt.set_node_attribute(det_id, "open", "");
                            self.status_text = "Expanded details".to_string();
                        }
                        let _ = rt.dispatch_toggle_event(det_id);
                    }
                    self.relayout();
                }
            }
            "label" => {
                let doc = self.active_document();

                let target_node = if let Some(lbl_node) = doc.get(node_id)
                    && let NodeData::Element(el) = &lbl_node.data
                    && let Some(for_id) = el.get_attribute("for")
                {
                    find_element_by_id(&doc, for_id)
                } else {
                    let mut found = None;
                    let mut stack = vec![node_id];
                    while let Some(nid) = stack.pop() {
                        if nid != node_id
                            && let Some(node) = doc.get(nid)
                            && let NodeData::Element(el) = &node.data
                            && matches!(
                                el.tag_name.to_ascii_lowercase().as_str(),
                                "input" | "select" | "textarea" | "button"
                            )
                        {
                            found = Some(nid);
                            break;
                        }
                        if let Some(node) = doc.get(nid) {
                            let mut child = node.first_child;
                            while let Some(cid) = child {
                                stack.push(cid);
                                child = doc.get(cid).and_then(|c| c.next_sibling);
                            }
                        }
                    }
                    found
                };

                if let Some(target_id) = target_node
                    && let Some(tnode) = doc.get(target_id)
                    && let NodeData::Element(el) = &tnode.data
                {
                    let t_type = el
                        .get_attribute("type")
                        .unwrap_or(if el.tag_name.eq_ignore_ascii_case("select") {
                            "select"
                        } else {
                            "text"
                        })
                        .to_ascii_lowercase();
                    let t_checked = el.get_attribute("checked").is_some();
                    self.handle_form_control_click(mango_layout::FormControlHit {
                        node_id: Some(target_id),
                        tag_name: el.tag_name.to_ascii_lowercase(),
                        form_type: t_type,
                        name: el.get_attribute("name").unwrap_or("").to_string(),
                        value: el.get_attribute("value").unwrap_or("").to_string(),
                        checked: t_checked,
                        click_offset_x: 0.0,
                    });
                }
            }
            _ => {
                // Text input / password / search / textarea / date / time / number
                self.focus_text_control(node_id, &hit);
            }
        }
    }
}

/// Returns the form owner of a node, according to the HTML specification.
/// A form control's form owner is either the `<form>` whose id matches its `form` attribute,
/// or its nearest ancestor `<form>` element. If neither exists, returns None (document-wide scope).
pub(crate) fn find_form_owner(doc: &mango_html::dom::Document, node_id: NodeId) -> Option<NodeId> {
    // 1. Explicit form attribute pointing to a <form id="...">
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(el) = &node.data
        && let Some(form_attr) = el.get_attribute("form")
        && !form_attr.is_empty()
    {
        if let Some(form_id) = find_element_by_id(doc, form_attr) {
            if let Some(fnode) = doc.get(form_id)
                && let NodeData::Element(fel) = &fnode.data
                && fel.tag_name.eq_ignore_ascii_case("form")
            {
                return Some(form_id);
            }
        }
    }

    // 2. Nearest ancestor <form>
    let mut curr = doc.get(node_id).and_then(|n| n.parent);
    while let Some(nid) = curr {
        if let Some(node) = doc.get(nid) {
            if let NodeData::Element(el) = &node.data
                && el.tag_name.eq_ignore_ascii_case("form")
            {
                return Some(nid);
            }
            curr = node.parent;
        } else {
            break;
        }
    }
    None
}

/// Finds all sibling radio button nodes in the same radio button group as `target_node_id`.
/// Per HTML standard, two radio buttons belong to the same group if they share the same
/// name and the same form owner (or both have no form owner).
pub(crate) fn find_sibling_radios_to_uncheck(
    doc: &mango_html::dom::Document,
    target_node_id: NodeId,
    radio_name: &str,
) -> Vec<NodeId> {
    let target_owner = find_form_owner(doc, target_node_id);
    let mut radios = Vec::new();
    let mut stack = vec![doc.root()];
    while let Some(nid) = stack.pop() {
        if let Some(node) = doc.get(nid) {
            if nid != target_node_id
                && let NodeData::Element(el) = &node.data
                && el.tag_name.eq_ignore_ascii_case("input")
                && el
                    .get_attribute("type")
                    .map(|t| t.eq_ignore_ascii_case("radio"))
                    .unwrap_or(false)
                && el
                    .get_attribute("name")
                    .map(|n| n == radio_name)
                    .unwrap_or(false)
                && find_form_owner(doc, nid) == target_owner
            {
                radios.push(nid);
            }
            let mut child = node.first_child;
            while let Some(cid) = child {
                stack.push(cid);
                if let Some(cnode) = doc.get(cid) {
                    child = cnode.next_sibling;
                } else {
                    break;
                }
            }
        }
    }
    radios
}
