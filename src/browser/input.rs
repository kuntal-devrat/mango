//! Keyboard, mouse, scroll, focus, and picker interaction handling.

use super::BrowserChrome;
use super::forms::find_element_by_id;
use crate::chrome_ui::*;
use crate::context_menu::*;
use crate::form_handler::*;
use mango_html::dom::{NodeData, NodeId};
use mango_platform::input::{KeyEvent, KeyState, MangoKey, MouseButton};

impl BrowserChrome {
    /// Handles mouse wheel scrolling with clamping.
    pub fn handle_scroll(&mut self, delta_y: f32) {
        // While the file picker overlay is open under the cursor, scroll its
        // directory listing instead of the page.
        let (mx, my) = (self.mouse_x, self.mouse_y);
        if let Some(picker) = &mut self.picker
            && mx >= picker.x
            && mx <= picker.x + picker.width
            && my >= picker.y
            && my <= picker.y + picker.height
            && let PickerKind::File {
                dir,
                entries,
                offset,
            } = &mut picker.kind
        {
            let has_parent = dir.parent().is_some();
            let total = entries.len() + usize::from(has_parent);
            let max_off = total.saturating_sub(PICKER_FILE_ROWS);
            let delta = if delta_y > 0.0 {
                2
            } else if delta_y < 0.0 {
                -2
            } else {
                0
            };
            *offset = ((*offset as i64) + delta).clamp(0, max_off as i64) as usize;
            return;
        }

        self.handle_scroll_2d(0.0, delta_y);
    }

    /// Handles 2D mouse wheel/trackpad scrolling with nested container dispatch and page clamping.
    pub fn handle_scroll_2d(&mut self, delta_x: f32, delta_y: f32) {
        let scroll_step_y = -delta_y * 45.0;
        let scroll_step_x = -delta_x * 45.0;

        let content_y = HEADER_HEIGHT + 1.0;
        let mouse_doc_x = self.mouse_x;
        let mouse_doc_y = (self.mouse_y - content_y).max(0.0) + self.scroll_y;

        // Try nested scrolling on scrollable containers under the cursor
        let (_unconsumed_x, unconsumed_y) = if let Some(root) = &mut self.root_box {
            let rem =
                root.dispatch_nested_scroll(mouse_doc_x, mouse_doc_y, scroll_step_x, scroll_step_y);
            if (rem.0 - scroll_step_x).abs() > 0.001 || (rem.1 - scroll_step_y).abs() > 0.001 {
                self.display_list_cache.lock().unwrap().clear();
            }
            rem
        } else {
            (scroll_step_x, scroll_step_y)
        };

        if unconsumed_y.abs() > 0.001 {
            let max = self.max_scroll() as f64;
            self.scroll_y_f64 = (self.scroll_y_f64 + unconsumed_y as f64).clamp(0.0, max);
            self.scroll_y = self.scroll_y_f64 as f32;
            if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.scroll_y_f64 = self.scroll_y_f64;
                tab.scroll_y = self.scroll_y;
            }
        }
    }

    /// Handles keyboard events.
    pub fn handle_key_event(&mut self, event: &KeyEvent) {
        // F12 or Ctrl+Shift+I toggles DevTools
        if event.key == MangoKey::F12
            || (event.modifiers.ctrl
                && event.modifiers.shift
                && matches!(&event.key, MangoKey::Char('i') | MangoKey::Char('I')))
        {
            self.devtools.toggle();
            self.relayout();
            return;
        }

        // Global Shortcuts:
        // F11: Fullscreen toggle
        if event.key == MangoKey::F11 {
            self.toggle_fullscreen();
            return;
        }

        // Ctrl+T: New tab
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('t') | MangoKey::Char('T')) {
            self.new_tab();
            return;
        }

        // Ctrl+W: Close active tab
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('w') | MangoKey::Char('W')) {
            self.close_active_tab();
            return;
        }

        // Ctrl+L: Focus Omnibox / Address bar
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('l') | MangoKey::Char('L')) {
            self.clear_form_focus();
            if self.find_in_page.active {
                self.find_in_page.active = false;
            }
            self.address_focused = true;
            self.is_all_selected = true;
            self.cursor_pos = self.address_text.len();
            return;
        }

        // Ctrl+H: History
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('h') | MangoKey::Char('H')) {
            self.navigate("about:history");
            return;
        }

        // Ctrl+F: Find in page
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('f') | MangoKey::Char('F')) {
            self.toggle_find();
            return;
        }

        // Ctrl++ or Ctrl+=: Zoom in
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('+') | MangoKey::Char('=')) {
            self.zoom_in();
            return;
        }

        // Ctrl+-: Zoom out
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('-') | MangoKey::Char('_')) {
            self.zoom_out();
            return;
        }

        // Ctrl+0: Reset zoom
        if event.modifiers.ctrl && matches!(&event.key, MangoKey::Char('0')) {
            self.zoom_reset();
            return;
        }

        // If Find-in-Page is active, route typing, Enter, Shift+Enter, and Escape
        if self.find_in_page.active {
            if event.key == MangoKey::Escape {
                self.find_in_page.active = false;
                self.find_in_page.matches.clear();
                self.page_dirty = true;
                return;
            }
            if event.key == MangoKey::Enter || event.key == MangoKey::F3 {
                if event.modifiers.shift {
                    self.find_previous();
                } else {
                    self.find_next();
                }
                return;
            }
            if event.key == MangoKey::Backspace {
                self.find_in_page.query.pop();
                let q = self.find_in_page.query.clone();
                self.find_in_page_search(&q);
                return;
            }
            if let MangoKey::Char(c) = event.key {
                if !event.modifiers.ctrl && !event.modifiers.alt {
                    self.find_in_page.query.push(c);
                    let q = self.find_in_page.query.clone();
                    self.find_in_page_search(&q);
                    return;
                }
            }
        }

        if !self.address_focused {
            // Dismiss popups on Escape
            if event.key == MangoKey::Escape {
                if self.picker.is_some() {
                    self.picker = None;
                    return;
                }
                if self.select_dropdown.is_some() {
                    self.select_dropdown = None;
                    self.relayout();
                    return;
                }
                if self.context_menu.is_some() {
                    self.context_menu = None;
                    return;
                }
            }

            // Tab / Shift+Tab keyboard focus navigation
            if event.key == MangoKey::Tab && !event.modifiers.ctrl {
                self.focus_next_element(event.modifiers.shift);
                return;
            }

            // Keyboard navigation in open select dropdown
            if let Some(ref mut dropdown) = self.select_dropdown {
                match &event.key {
                    MangoKey::ArrowDown => {
                        let cur = dropdown.hovered_idx.unwrap_or(0);
                        if cur + 1 < dropdown.options.len() {
                            dropdown.hovered_idx = Some(cur + 1);
                        }
                        return;
                    }
                    MangoKey::ArrowUp => {
                        let cur = dropdown.hovered_idx.unwrap_or(0);
                        if cur > 0 {
                            dropdown.hovered_idx = Some(cur - 1);
                        }
                        return;
                    }
                    MangoKey::Enter => {
                        let idx = dropdown.hovered_idx.unwrap_or(0);
                        if let Some(opt) = dropdown.options.get(idx) {
                            let selected_val = opt.value.clone();
                            let selected_node_id = opt.node_id;
                            let select_node_id = dropdown.select_node_id;
                            let selected_text = opt.text.clone();
                            if let Some(ref mut rt) = self.js_runtime {
                                for o in &dropdown.options {
                                    if o.node_id == selected_node_id {
                                        rt.set_node_attribute(o.node_id, "selected", "true");
                                    } else {
                                        rt.remove_node_attribute(o.node_id, "selected");
                                    }
                                }
                            }
                            self.set_control_value(select_node_id, &selected_val);
                            self.fire_dom_event(select_node_id, "change");
                            if let Some(ref mut rt) = self.js_runtime
                                && let Some(onchange) =
                                    rt.get_node_attribute(select_node_id, "onchange")
                            {
                                let _ = rt.execute_script(&onchange);
                            }
                            self.status_text = format!("Selected: {}", selected_text);
                        }
                        self.select_dropdown = None;
                        self.relayout();
                        return;
                    }
                    _ => {}
                }
            }

            // Global focus shortcut: Ctrl+L
            if event.modifiers.ctrl
                && matches!(&event.key, MangoKey::Char('l') | MangoKey::Char('L'))
            {
                self.clear_form_focus();
                self.address_focused = true;
                self.is_all_selected = true;
                self.cursor_pos = self.address_text.len();
                return;
            }

            // Tab / Shift+Tab keyboard focus navigation
            if event.key == MangoKey::Tab && !event.modifiers.ctrl {
                self.focus_next_element(event.modifiers.shift);
                return;
            }

            // Route key events to focused form input
            if let Some(node_id) = self.focused_control {
                let current_val = self
                    .js_runtime
                    .as_ref()
                    .and_then(|rt| rt.get_node_attribute(node_id, "value"))
                    .unwrap_or_default();

                // ArrowUp/ArrowDown step numeric controls (range slider / number spinner).
                if matches!(event.key, MangoKey::ArrowUp | MangoKey::ArrowDown)
                    && !event.modifiers.ctrl
                {
                    let input_type = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(node_id, "type"))
                        .map(|t| t.to_ascii_lowercase())
                        .unwrap_or_default();
                    if input_type == "range" || input_type == "number" {
                        let dir = if event.key == MangoKey::ArrowUp {
                            1.0
                        } else {
                            -1.0
                        };
                        self.step_numeric_control(node_id, dir);
                        return;
                    }
                }

                if event.modifiers.ctrl {
                    match &event.key {
                        MangoKey::Char('a') | MangoKey::Char('A') => {
                            self.control_cursor_pos = current_val.chars().count();
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    node_id,
                                    "_mango_cursor_pos",
                                    &self.control_cursor_pos.to_string(),
                                );
                            }
                            self.relayout();
                            return;
                        }
                        MangoKey::Char('c') | MangoKey::Char('C') => {
                            if !current_val.is_empty()
                                && let Ok(mut clipboard) = arboard::Clipboard::new()
                            {
                                let _ = clipboard.set_text(current_val);
                                self.status_text = "Copied text to clipboard".to_string();
                            }
                            return;
                        }
                        MangoKey::Char('v') | MangoKey::Char('V') => {
                            if let Ok(mut clipboard) = arboard::Clipboard::new()
                                && let Ok(paste_text) = clipboard.get_text()
                            {
                                let chars: Vec<char> = current_val.chars().collect();
                                let idx = self.control_cursor_pos.min(chars.len());
                                let mut new_chars =
                                    Vec::with_capacity(chars.len() + paste_text.len());
                                new_chars.extend_from_slice(&chars[..idx]);
                                new_chars.extend(paste_text.chars());
                                new_chars.extend_from_slice(&chars[idx..]);
                                self.control_cursor_pos = idx + paste_text.chars().count();
                                let new_val: String = new_chars.into_iter().collect();
                                self.set_control_value(node_id, &new_val);
                                if let Some(ref mut rt) = self.js_runtime {
                                    rt.set_node_attribute(
                                        node_id,
                                        "_mango_cursor_pos",
                                        &self.control_cursor_pos.to_string(),
                                    );
                                }
                                self.relayout();
                            }
                            return;
                        }
                        MangoKey::Char('x') | MangoKey::Char('X') => {
                            if !current_val.is_empty()
                                && let Ok(mut clipboard) = arboard::Clipboard::new()
                            {
                                let _ = clipboard.set_text(current_val);
                                self.control_cursor_pos = 0;
                                self.set_control_value(node_id, "");
                                if let Some(ref mut rt) = self.js_runtime {
                                    rt.set_node_attribute(node_id, "_mango_cursor_pos", "0");
                                }
                                self.relayout();
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                match &event.key {
                    MangoKey::Char(c) => {
                        let chars: Vec<char> = current_val.chars().collect();
                        let idx = self.control_cursor_pos.min(chars.len());
                        let mut new_chars = Vec::with_capacity(chars.len() + 1);
                        new_chars.extend_from_slice(&chars[..idx]);
                        new_chars.push(*c);
                        new_chars.extend_from_slice(&chars[idx..]);
                        self.control_cursor_pos = idx + 1;
                        let new_val: String = new_chars.into_iter().collect();
                        self.set_control_value(node_id, &new_val);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Space => {
                        let chars: Vec<char> = current_val.chars().collect();
                        let idx = self.control_cursor_pos.min(chars.len());
                        let mut new_chars = Vec::with_capacity(chars.len() + 1);
                        new_chars.extend_from_slice(&chars[..idx]);
                        new_chars.push(' ');
                        new_chars.extend_from_slice(&chars[idx..]);
                        self.control_cursor_pos = idx + 1;
                        let new_val: String = new_chars.into_iter().collect();
                        self.set_control_value(node_id, &new_val);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Backspace => {
                        let mut chars: Vec<char> = current_val.chars().collect();
                        if self.control_cursor_pos > 0 && !chars.is_empty() {
                            let idx = self.control_cursor_pos.min(chars.len()) - 1;
                            chars.remove(idx);
                            self.control_cursor_pos = idx;
                            let new_val: String = chars.into_iter().collect();
                            self.set_control_value(node_id, &new_val);
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    node_id,
                                    "_mango_cursor_pos",
                                    &self.control_cursor_pos.to_string(),
                                );
                            }
                            self.relayout();
                        }
                        return;
                    }
                    MangoKey::Delete => {
                        let mut chars: Vec<char> = current_val.chars().collect();
                        if self.control_cursor_pos < chars.len() {
                            chars.remove(self.control_cursor_pos);
                            let new_val: String = chars.into_iter().collect();
                            self.set_control_value(node_id, &new_val);
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    node_id,
                                    "_mango_cursor_pos",
                                    &self.control_cursor_pos.to_string(),
                                );
                            }
                            self.relayout();
                        }
                        return;
                    }
                    MangoKey::ArrowLeft => {
                        self.control_cursor_pos = self.control_cursor_pos.saturating_sub(1);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::ArrowRight => {
                        let count = current_val.chars().count();
                        self.control_cursor_pos = (self.control_cursor_pos + 1).min(count);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Home => {
                        self.control_cursor_pos = 0;
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::End => {
                        self.control_cursor_pos = current_val.chars().count();
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Escape => {
                        self.clear_form_focus();
                        return;
                    }
                    MangoKey::Enter => {
                        self.status_text = "Submitting form...".to_string();
                        if let Some(ref mut rt) = self.js_runtime
                            && let Some(handler) = rt.get_node_attribute(node_id, "onchange")
                        {
                            let _ = rt.execute_script(&handler);
                        }
                        self.submit_form(node_id);
                        return;
                    }
                    _ => {}
                }
            }

            // Scroll content when address bar is not focused
            let content_h =
                (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
            let page_step = (content_h * 0.85) / 45.0;
            match &event.key {
                MangoKey::ArrowDown => {
                    self.handle_scroll(-1.0);
                }
                MangoKey::ArrowUp => {
                    self.handle_scroll(1.0);
                }
                MangoKey::PageDown => {
                    self.handle_scroll(-page_step);
                }
                MangoKey::PageUp => {
                    self.handle_scroll(page_step);
                }
                MangoKey::Space => {
                    if event.modifiers.shift {
                        self.handle_scroll(page_step);
                    } else {
                        self.handle_scroll(-page_step);
                    }
                }
                MangoKey::Home => {
                    self.scroll_y = 0.0;
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = 0.0;
                    }
                }
                MangoKey::End => {
                    self.scroll_y = self.max_scroll();
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = self.scroll_y;
                    }
                }
                _ => {}
            }
            return;
        }

        // ── Address Bar Focused ──
        if event.modifiers.ctrl {
            match &event.key {
                MangoKey::Char('a') | MangoKey::Char('A') => {
                    self.is_all_selected = true;
                    self.cursor_pos = self.address_text.len();
                    return;
                }
                MangoKey::Char('c') | MangoKey::Char('C') => {
                    let text_to_copy = &self.address_text;
                    if !text_to_copy.is_empty()
                        && let Ok(mut clipboard) = arboard::Clipboard::new()
                    {
                        let _ = clipboard.set_text(text_to_copy.to_string());
                        self.status_text = "Copied address to clipboard".to_string();
                    }
                    return;
                }
                MangoKey::Char('x') | MangoKey::Char('X') => {
                    if !self.address_text.is_empty()
                        && let Ok(mut clipboard) = arboard::Clipboard::new()
                    {
                        let _ = clipboard.set_text(self.address_text.clone());
                        self.address_text.clear();
                        self.cursor_pos = 0;
                        self.is_all_selected = false;
                        self.status_text = "Cut address to clipboard".to_string();
                    }
                    return;
                }
                MangoKey::Char('v') | MangoKey::Char('V') => {
                    if let Ok(mut clipboard) = arboard::Clipboard::new()
                        && let Ok(paste_text) = clipboard.get_text()
                    {
                        let clean_paste: String = paste_text
                            .chars()
                            .filter(|c| !c.is_control() || *c == ' ')
                            .collect();
                        if self.is_all_selected {
                            self.address_text = clean_paste;
                            self.cursor_pos = self.address_text.len();
                            self.is_all_selected = false;
                        } else {
                            let safe_pos = self.cursor_pos.min(self.address_text.len());
                            self.address_text.insert_str(safe_pos, &clean_paste);
                            self.cursor_pos = safe_pos + clean_paste.len();
                        }
                        self.status_text = "Pasted from clipboard".to_string();
                    }
                    return;
                }
                _ => {}
            }
        }

        match &event.key {
            MangoKey::Char(ch) => {
                if self.is_all_selected {
                    self.address_text.clear();
                    self.address_text.push(*ch);
                    self.cursor_pos = self.address_text.len();
                    self.is_all_selected = false;
                } else {
                    let safe_pos = self.cursor_pos.min(self.address_text.len());
                    self.address_text.insert(safe_pos, *ch);
                    self.cursor_pos = safe_pos + ch.len_utf8();
                }
            }
            MangoKey::Space => {
                if self.is_all_selected {
                    self.address_text.clear();
                    self.address_text.push(' ');
                    self.cursor_pos = 1;
                    self.is_all_selected = false;
                } else {
                    let safe_pos = self.cursor_pos.min(self.address_text.len());
                    self.address_text.insert(safe_pos, ' ');
                    self.cursor_pos = safe_pos + 1;
                }
            }
            MangoKey::Backspace => {
                if self.is_all_selected {
                    self.address_text.clear();
                    self.cursor_pos = 0;
                    self.is_all_selected = false;
                } else if self.cursor_pos > 0 {
                    let safe_pos = self.cursor_pos.min(self.address_text.len());
                    let mut prev = safe_pos - 1;
                    while prev > 0 && !self.address_text.is_char_boundary(prev) {
                        prev -= 1;
                    }
                    self.address_text.drain(prev..safe_pos);
                    self.cursor_pos = prev;
                }
            }
            MangoKey::Delete => {
                if self.is_all_selected {
                    self.address_text.clear();
                    self.cursor_pos = 0;
                    self.is_all_selected = false;
                } else if self.cursor_pos < self.address_text.len() {
                    let safe_pos = self.cursor_pos.min(self.address_text.len());
                    let mut next = safe_pos + 1;
                    while next < self.address_text.len()
                        && !self.address_text.is_char_boundary(next)
                    {
                        next += 1;
                    }
                    self.address_text.drain(safe_pos..next);
                }
            }
            MangoKey::ArrowLeft => {
                self.is_all_selected = false;
                if self.cursor_pos > 0 {
                    let mut prev = self.cursor_pos.min(self.address_text.len()) - 1;
                    while prev > 0 && !self.address_text.is_char_boundary(prev) {
                        prev -= 1;
                    }
                    self.cursor_pos = prev;
                }
            }
            MangoKey::ArrowRight => {
                self.is_all_selected = false;
                if self.cursor_pos < self.address_text.len() {
                    let mut next = self.cursor_pos + 1;
                    while next < self.address_text.len()
                        && !self.address_text.is_char_boundary(next)
                    {
                        next += 1;
                    }
                    self.cursor_pos = next;
                }
            }
            MangoKey::Home => {
                self.is_all_selected = false;
                self.cursor_pos = 0;
            }
            MangoKey::End => {
                self.is_all_selected = false;
                self.cursor_pos = self.address_text.len();
            }
            MangoKey::Enter => {
                self.is_all_selected = false;
                self.navigate_to_address(true);
            }
            MangoKey::Escape => {
                self.address_focused = false;
                self.is_all_selected = false;
                self.status_text = "Ready".to_string();
            }
            _ => {}
        }
    }

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

    /// Handles mouse movement and updates hovered target. Returns `true` if hover state changed.
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) -> bool {
        let old_hovered = self.hovered_target;
        let old_status = self.status_text.clone();
        let old_menu_hover = self.context_menu.as_ref().map(|m| m.hovered_idx);

        self.mouse_x = x;
        self.mouse_y = y;

        // While a <input type="range"> thumb is being dragged, follow the mouse.
        if let Some(node_id) = self.range_dragging {
            self.update_range_drag(node_id);
            return true;
        }

        // If user is actively dragging the scrollbar thumb:
        if self.scrollbar_dragging {
            let content_h =
                (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
            let doc_h = self.scrollable_height();
            let max_s = self.max_scroll();
            if max_s > 0.0 && doc_h > 0.0 {
                let thumb_h =
                    ((content_h / doc_h) * content_h).clamp(32.0f32.min(content_h), content_h);
                let max_thumb_travel = (content_h - thumb_h).max(1.0);
                let delta_mouse_y = y - self.drag_start_mouse_y;
                let delta_scroll = (delta_mouse_y / max_thumb_travel) * max_s;
                self.scroll_y = (self.drag_start_scroll_y + delta_scroll).clamp(0.0, max_s);
                if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                    tab.scroll_y = self.scroll_y;
                }
            }
            return true;
        }

        // Context menu hover hit testing
        if let Some(ref mut menu) = self.context_menu {
            if x >= menu.x && x <= menu.x + menu.width && y >= menu.y && y <= menu.y + menu.height {
                let idx = ((y - menu.y - 4.0) / 26.0) as usize;
                if idx < menu.items.len() {
                    menu.hovered_idx = Some(idx);
                } else {
                    menu.hovered_idx = None;
                }
                return old_hovered != self.hovered_target
                    || old_status != self.status_text
                    || old_menu_hover != Some(menu.hovered_idx);
            } else {
                menu.hovered_idx = None;
            }
        }

        // Select dropdown hover hit testing
        if let Some(ref mut dropdown) = self.select_dropdown {
            let total_h = (dropdown.options.len() as f32 * dropdown.item_height + 8.0).min(300.0);
            if x >= dropdown.x
                && x <= dropdown.x + dropdown.width
                && y >= dropdown.y
                && y <= dropdown.y + total_h
            {
                let idx = ((y - dropdown.y - 4.0) / dropdown.item_height) as usize;
                if idx < dropdown.options.len() {
                    dropdown.hovered_idx = Some(idx);
                } else {
                    dropdown.hovered_idx = None;
                }
                return true;
            } else {
                dropdown.hovered_idx = None;
            }
        }

        // Picker overlay (color/date/time/file) hover hit testing
        if let Some(ref mut picker) = self.picker {
            let new_hover = picker_hit_test(picker, x, y);
            let changed = picker.hover != new_hover;
            picker.hover = new_hover;
            return changed;
        }

        let w = self.width as f32;
        let mut hover_changed = false;

        // If mouse is outside web content (in header/tabs/toolbar or status bar), clear DOM content hover
        if (y <= HEADER_HEIGHT || y >= self.height as f32 - STATUS_BAR_HEIGHT)
            && self.set_hover_node(None)
        {
            hover_changed = true;
            self.relayout();
        }

        // 1. Tab bar hit testing
        if y < TAB_BAR_HEIGHT {
            let tab_count = self.tabs.len().max(1);
            let max_strip_w = (w - 42.0).max(100.0);
            let tab_w = (max_strip_w / tab_count as f32).clamp(80.0, 210.0);

            let new_tab_x = (tab_count as f32 * tab_w + 4.0).min(w - 30.0);
            if x >= new_tab_x && x <= new_tab_x + 24.0 && (6.0..=30.0).contains(&y) {
                self.hovered_target = Some(HoverTarget::NewTab);
                return hover_changed
                    || old_hovered != self.hovered_target
                    || old_status != self.status_text
                    || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
            }

            for i in 0..self.tabs.len() {
                let tab_x = i as f32 * tab_w;
                if x >= tab_x && x < tab_x + tab_w {
                    let close_x = tab_x + tab_w - 22.0;
                    if x >= close_x && x <= close_x + 18.0 && (8.0..=26.0).contains(&y) {
                        self.hovered_target = Some(HoverTarget::TabClose(i));
                    } else {
                        self.hovered_target = Some(HoverTarget::Tab(i));
                    }
                    return hover_changed
                        || old_hovered != self.hovered_target
                        || old_status != self.status_text
                        || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
                }
            }
            self.hovered_target = None;
            return hover_changed
                || old_hovered != self.hovered_target
                || old_status != self.status_text
                || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
        }

        // 2. Toolbar hit testing
        if (TAB_BAR_HEIGHT..HEADER_HEIGHT).contains(&y) {
            let btn_y = TAB_BAR_HEIGHT + 6.0;
            if y >= btn_y - 2.0 && y <= btn_y + 26.0 {
                if (6.0..=32.0).contains(&x) {
                    self.hovered_target = Some(HoverTarget::Back);
                    return hover_changed
                        || old_hovered != self.hovered_target
                        || old_status != self.status_text
                        || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
                }
                if (36.0..=62.0).contains(&x) {
                    self.hovered_target = Some(HoverTarget::Forward);
                    return hover_changed
                        || old_hovered != self.hovered_target
                        || old_status != self.status_text
                        || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
                }
                if (66.0..=92.0).contains(&x) {
                    self.hovered_target = Some(HoverTarget::Reload);
                    return hover_changed
                        || old_hovered != self.hovered_target
                        || old_status != self.status_text
                        || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
                }
                if (96.0..=122.0).contains(&x) {
                    self.hovered_target = Some(HoverTarget::Home);
                    return hover_changed
                        || old_hovered != self.hovered_target
                        || old_status != self.status_text
                        || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
                }
                let menu_x = w - 34.0;
                if x >= menu_x - 3.0 && x <= menu_x + 24.0 {
                    self.hovered_target = Some(HoverTarget::Menu);
                    return hover_changed
                        || old_hovered != self.hovered_target
                        || old_status != self.status_text
                        || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
                }
            }

            let addr_x = 132.0;
            let addr_w = (w - 176.0).max(120.0);
            let addr_y = TAB_BAR_HEIGHT + 5.0;
            if x >= addr_x && x <= addr_x + addr_w && y >= addr_y && y <= addr_y + 30.0 {
                self.hovered_target = Some(HoverTarget::AddressBar);
                return hover_changed
                    || old_hovered != self.hovered_target
                    || old_status != self.status_text
                    || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
            }

            self.hovered_target = None;
            return hover_changed
                || old_hovered != self.hovered_target
                || old_status != self.status_text
                || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
        }

        self.hovered_target = None;

        // 3. Vertical Scrollbar Hit Testing (x >= w - 14.0, within content area)
        let content_y_top = HEADER_HEIGHT + 1.0;
        let content_h = (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let max_s = self.max_scroll();
        if max_s > 0.0 && x >= w - 14.0 && y >= content_y_top && y <= content_y_top + content_h {
            if self.set_hover_node(None) {
                hover_changed = true;
                self.relayout();
            }
            let doc_h = self.scrollable_height();
            let thumb_h =
                ((content_h / doc_h) * content_h).clamp(32.0f32.min(content_h), content_h);
            let max_thumb_travel = (content_h - thumb_h).max(1.0);
            let thumb_y = content_y_top + (self.scroll_y / max_s) * max_thumb_travel;
            if y >= thumb_y && y <= thumb_y + thumb_h {
                self.hovered_target = Some(HoverTarget::ScrollbarThumb);
            } else {
                self.hovered_target = Some(HoverTarget::ScrollbarTrack);
            }
            return hover_changed
                || old_hovered != self.hovered_target
                || old_status != self.status_text
                || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx);
        }

        // 4. Content area hit testing (hovered links or form controls)
        if y > HEADER_HEIGHT && y < self.height as f32 - STATUS_BAR_HEIGHT {
            let content_x = x;
            let content_y = y - HEADER_HEIGHT + self.scroll_y;
            let pt = mango_core::Point::new(content_x, content_y);

            // Update DOM hover state (:hover pseudo-class)
            let hovered_node = self.root_box.as_ref().and_then(|root| root.hit_test(pt));
            if self.set_hover_node(hovered_node) {
                hover_changed = true;
            }

            if let Some(target_href) = self
                .root_box
                .as_ref()
                .and_then(|root| root.hit_test_link(pt))
            {
                if let Some(base) = &self.base_url
                    && let Ok(resolved) = base.resolve(target_href)
                {
                    self.status_text = format!("Link: {}", resolved);
                } else {
                    self.status_text = format!("Link: {}", target_href);
                }
            } else if let Some(hit) = self
                .root_box
                .as_ref()
                .and_then(|root| root.hit_test_form_control(pt))
            {
                self.status_text = format!(
                    "Form control: <{} type=\"{}\">",
                    hit.tag_name, hit.form_type
                );
            } else if let Some(media) = self
                .root_box
                .as_ref()
                .and_then(|root| root.hit_test_media_control(pt))
            {
                let tag = if media.is_video { "video" } else { "audio" };
                let play_state = if media.is_playing {
                    "Playing"
                } else {
                    "Paused"
                };
                let cur_m = (media.current_time as u32) / 60;
                let cur_s = (media.current_time as u32) % 60;
                let dur_m = (media.duration as u32) / 60;
                let dur_s = (media.duration as u32) % 60;
                self.status_text = format!(
                    "Media <{}>: {} ({:02}:{:02} / {:02}:{:02}){}",
                    tag,
                    play_state,
                    cur_m,
                    cur_s,
                    dur_m,
                    dur_s,
                    if media.is_muted { " [Muted]" } else { "" }
                );
            } else if self.status_text.starts_with("Link: ")
                || self.status_text.starts_with("Form control: ")
                || self.status_text.starts_with("Media <")
            {
                self.status_text = "Ready".to_string();
            }
        } else {
            if self.set_hover_node(None) {
                hover_changed = true;
            }
            if self.status_text.starts_with("Link: ")
                || self.status_text.starts_with("Form control: ")
                || self.status_text.starts_with("Media <")
            {
                self.status_text = "Ready".to_string();
            }
        }

        if hover_changed {
            self.relayout();
        }

        hover_changed
            || old_hovered != self.hovered_target
            || old_status != self.status_text
            || old_menu_hover != self.context_menu.as_ref().map(|m| m.hovered_idx)
    }

    /// Handles mouse clicks on UI elements, context menu, form inputs, or links in content.
    pub fn handle_mouse_click(&mut self, button: MouseButton, state: KeyState) {
        // Middle click (auxiliary click) on links opens in a background tab
        if state == KeyState::Released && button == MouseButton::Middle {
            if self.mouse_y > HEADER_HEIGHT && self.mouse_y < self.height as f32 - STATUS_BAR_HEIGHT {
                let content_x = self.mouse_x;
                let content_y = self.mouse_y - HEADER_HEIGHT + self.scroll_y;
                let hit_link = self
                    .root_box
                    .as_ref()
                    .and_then(|root| {
                        root.hit_test_link(mango_core::Point::new(content_x, content_y))
                    })
                    .map(|s| s.to_string());
                if let Some(target_href) = hit_link {
                    let nav_url = if let Some(base) = &self.base_url {
                        base.resolve(&target_href)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| target_href.clone())
                    } else {
                        target_href
                    };
                    log::info!("Middle-click link: opening in background tab: {}", nav_url);
                    self.new_background_tab(&nav_url);
                }
            }
            return;
        }

        if state == KeyState::Released && button == MouseButton::Left {
            self.scrollbar_dragging = false;

            let mut link_to_navigate: Option<(String, Option<String>)> = None;

            if self.mouse_y > HEADER_HEIGHT && self.mouse_y < self.height as f32 - STATUS_BAR_HEIGHT
            {
                let content_x = self.mouse_x;
                let content_y = self.mouse_y - HEADER_HEIGHT + self.scroll_y;
                let hit_link = self
                    .root_box
                    .as_ref()
                    .and_then(|root| {
                        root.hit_test_link_full(mango_core::Point::new(content_x, content_y))
                    })
                    .map(|(s, f)| (s.to_string(), f.map(|frame| frame.to_string())));
                let hit_node = self
                    .root_box
                    .as_ref()
                    .and_then(|root| root.hit_test(mango_core::Point::new(content_x, content_y)));

                if let Some((target_href, target_frame)) = hit_link {
                    if let Some(node_id) = hit_node {
                        self.fire_dom_event(node_id, "click");
                    }
                    let nav_url = if let Some(base) = &self.base_url {
                        base.resolve(&target_href)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| target_href.clone())
                    } else {
                        target_href
                    };
                    log::info!(
                        "Navigating to clicked link: {} (target={:?})",
                        nav_url,
                        target_frame
                    );
                    link_to_navigate = Some((nav_url, target_frame));
                } else if let Some(node_id) = hit_node {
                    self.fire_dom_event(node_id, "click");
                }
            }

            let active_changed = self.set_active_node(None);
            if active_changed {
                self.relayout();
            }
            // Releasing a dragged range slider commits its `change` event.
            if let Some(node_id) = self.range_dragging.take() {
                let start = self.range_drag_start_value.take();
                let current = self
                    .js_runtime
                    .as_ref()
                    .and_then(|rt| rt.get_node_attribute(node_id, "value"));
                if current != start {
                    self.fire_dom_event(node_id, "change");
                }
            }
            if let Some((nav_url, target_frame)) = link_to_navigate {
                if target_frame.as_deref() == Some("_blank") {
                    self.new_tab_and_navigate(&nav_url);
                } else {
                    self.navigate(&nav_url);
                }
            }
            return;
        }

        if state != KeyState::Pressed {
            return;
        }

        // Find-in-Page bar hit testing
        if button == MouseButton::Left && self.find_in_page.active {
            let content_y = HEADER_HEIGHT + 1.0;
            let bar_w = 320.0f32;
            let bar_h = 38.0f32;
            let bar_x = (self.width as f32 - bar_w - 24.0).max(10.0);
            let bar_y = content_y + 10.0;

            if self.mouse_x >= bar_x
                && self.mouse_x <= bar_x + bar_w
                && self.mouse_y >= bar_y
                && self.mouse_y <= bar_y + bar_h
            {
                // Close button (bar_x + bar_w - 32.0 .. bar_x + bar_w)
                if self.mouse_x >= bar_x + bar_w - 32.0 {
                    self.find_in_page.active = false;
                    self.find_in_page.matches.clear();
                    self.page_dirty = true;
                    return;
                }
                // Next button (bar_x + bar_w - 60.0 .. bar_x + bar_w - 32.0)
                if self.mouse_x >= bar_x + bar_w - 60.0 {
                    self.find_next();
                    return;
                }
                // Prev button (bar_x + bar_w - 90.0 .. bar_x + bar_w - 60.0)
                if self.mouse_x >= bar_x + bar_w - 90.0 {
                    self.find_previous();
                    return;
                }
                // Click in query input area
                self.address_focused = false;
                self.clear_form_focus();
                return;
            }
        }

        if button == MouseButton::Left
            && self.mouse_y > HEADER_HEIGHT
            && self.mouse_y < self.height as f32 - STATUS_BAR_HEIGHT
        {
            let content_x = self.mouse_x;
            let content_y = self.mouse_y - HEADER_HEIGHT + self.scroll_y;
            let pt = mango_core::Point::new(content_x, content_y);
            let active_node = self.root_box.as_ref().and_then(|root| root.hit_test(pt));
            let active_changed = self.set_active_node(active_node);
            if active_changed {
                self.relayout();
            }
        }

        // 1. Right Click -> Open Context Menu
        if button == MouseButton::Right {
            let content_x = self.mouse_x;
            let content_y = self.mouse_y - HEADER_HEIGHT + self.scroll_y;
            let hit_link = self
                .root_box
                .as_ref()
                .and_then(|r| r.hit_test_link(mango_core::Point::new(content_x, content_y)));
            let hit_media = self.root_box.as_ref().and_then(|r| {
                r.hit_test_media_control(mango_core::Point::new(content_x, content_y))
            });

            let can_back = self.active_tab().is_some_and(|t| t.can_go_back());
            let can_fwd = self.active_tab().is_some_and(|t| t.can_go_forward());

            let mut items = Vec::new();

            if let Some(media) = hit_media
                && let Some(nid) = media.node_id
            {
                let play_label = if media.is_playing { "Pause" } else { "Play" };
                items.push(ContextMenuItem {
                    label: play_label.to_string(),
                    action: ContextMenuAction::ToggleMediaPlay(nid, !media.is_playing),
                    enabled: true,
                });
                let mute_label = if media.is_muted { "Unmute" } else { "Mute" };
                items.push(ContextMenuItem {
                    label: mute_label.to_string(),
                    action: ContextMenuAction::ToggleMediaMute(nid, !media.is_muted),
                    enabled: true,
                });
            }
            if let Some(link) = hit_link {
                let resolved = if let Some(base) = &self.base_url {
                    base.resolve(link)
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| link.to_string())
                } else {
                    link.to_string()
                };
                items.push(ContextMenuItem {
                    label: "Open Link in New Tab".to_string(),
                    action: ContextMenuAction::OpenLinkInNewTab(resolved.clone()),
                    enabled: true,
                });
                items.push(ContextMenuItem {
                    label: "Copy Link Address".to_string(),
                    action: ContextMenuAction::CopyLink(resolved),
                    enabled: true,
                });
            }
            items.push(ContextMenuItem {
                label: "Back".to_string(),
                action: ContextMenuAction::Back,
                enabled: can_back,
            });
            items.push(ContextMenuItem {
                label: "Forward".to_string(),
                action: ContextMenuAction::Forward,
                enabled: can_fwd,
            });
            items.push(ContextMenuItem {
                label: "Reload".to_string(),
                action: ContextMenuAction::Reload,
                enabled: true,
            });
            items.push(ContextMenuItem {
                label: "View Page Source".to_string(),
                action: ContextMenuAction::ViewSource,
                enabled: true,
            });
            items.push(ContextMenuItem {
                label: "Inspect Element".to_string(),
                action: ContextMenuAction::Inspect,
                enabled: true,
            });

            let menu_w = 175.0;
            let menu_h = items.len() as f32 * 26.0 + 8.0;
            let menu_x = self
                .mouse_x
                .min((self.width as f32 - menu_w - 4.0).max(0.0));
            let menu_y = self
                .mouse_y
                .min((self.height as f32 - menu_h - 4.0).max(0.0));

            self.context_menu = Some(ContextMenu {
                x: menu_x,
                y: menu_y,
                width: menu_w,
                height: menu_h,
                items,
                hovered_idx: None,
            });
            return;
        }

        // 2. Left Click
        if button == MouseButton::Left {
            // An open picker overlay (color/date/time/file) consumes clicks:
            // inside clicks act on the picker, outside clicks dismiss it.
            if self.picker.is_some() {
                self.handle_picker_click();
                return;
            }

            // Dismiss or select option in select dropdown if open
            if let Some(dropdown) = self.select_dropdown.take() {
                let total_h =
                    (dropdown.options.len() as f32 * dropdown.item_height + 8.0).min(300.0);
                if self.mouse_x >= dropdown.x
                    && self.mouse_x <= dropdown.x + dropdown.width
                    && self.mouse_y >= dropdown.y
                    && self.mouse_y <= dropdown.y + total_h
                {
                    let idx = ((self.mouse_y - dropdown.y - 4.0) / dropdown.item_height) as usize;
                    if let Some(opt) = dropdown.options.get(idx) {
                        let selected_val = opt.value.clone();
                        let selected_node_id = opt.node_id;
                        let select_node_id = dropdown.select_node_id;
                        let selected_text = opt.text.clone();

                        if let Some(ref mut rt) = self.js_runtime {
                            for o in &dropdown.options {
                                if o.node_id == selected_node_id {
                                    rt.set_node_attribute(o.node_id, "selected", "true");
                                } else {
                                    rt.remove_node_attribute(o.node_id, "selected");
                                }
                            }
                        }
                        self.set_control_value(select_node_id, &selected_val);
                        self.fire_dom_event(select_node_id, "change");
                        if let Some(ref mut rt) = self.js_runtime
                            && let Some(onchange) =
                                rt.get_node_attribute(select_node_id, "onchange")
                        {
                            let _ = rt.execute_script(&onchange);
                        }
                        self.status_text = format!("Selected: {}", selected_text);
                        self.relayout();
                        return;
                    }
                }
                self.relayout();
            }

            // Dismiss or activate context menu if open
            if let Some(menu) = self.context_menu.take()
                && self.mouse_x >= menu.x
                && self.mouse_x <= menu.x + menu.width
                && self.mouse_y >= menu.y
                && self.mouse_y <= menu.y + menu.height
            {
                let idx = ((self.mouse_y - menu.y - 4.0) / 26.0) as usize;
                if let Some(item) = menu.items.get(idx)
                    && item.enabled
                {
                    match &item.action {
                        ContextMenuAction::Back => self.go_back(),
                        ContextMenuAction::Forward => self.go_forward(),
                        ContextMenuAction::Reload => self.reload(),
                        ContextMenuAction::OpenLinkInNewTab(url) => {
                            self.new_tab_and_navigate(url);
                        }
                        ContextMenuAction::CopyLink(url) => {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(url.clone());
                            }
                            self.status_text = format!("Copied: {}", url);
                        }
                        ContextMenuAction::ViewSource => self.open_view_source(),
                        ContextMenuAction::Inspect => {
                            self.devtools
                                .open_tab(crate::devtools::DevToolsTab::Elements);
                            self.status_text = "Mango DevTools: Inspect Element opened".to_string();
                            self.relayout();
                        }
                        ContextMenuAction::ToggleMediaPlay(node_id, play) => {
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    *node_id,
                                    "data-mango-playing",
                                    if *play { "true" } else { "false" },
                                );
                                let evt_type = if *play { "play" } else { "pause" };
                                let script = format!(
                                    "if (typeof document !== 'undefined') {{ var el = document.querySelector('[data-mango-playing]'); if (el) el.dispatchEvent({{ type: '{}' }}); }}",
                                    evt_type
                                );
                                let _ = rt.execute_script(&script);
                            }
                            self.status_text = if *play {
                                "Media playing".to_string()
                            } else {
                                "Media paused".to_string()
                            };
                            self.relayout();
                        }
                        ContextMenuAction::ToggleMediaMute(node_id, mute) => {
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    *node_id,
                                    "data-mango-muted",
                                    if *mute { "true" } else { "false" },
                                );
                                let script = "if (typeof document !== 'undefined') { var el = document.querySelector('[data-mango-muted]'); if (el) el.dispatchEvent({ type: 'volumechange' }); }";
                                let _ = rt.execute_script(script);
                            }
                            self.status_text = if *mute {
                                "Media muted".to_string()
                            } else {
                                "Media unmuted".to_string()
                            };
                            self.relayout();
                        }
                    }
                    return;
                }
            }

            match self.hovered_target {
                Some(HoverTarget::Back) => {
                    self.go_back();
                    return;
                }
                Some(HoverTarget::Forward) => {
                    self.go_forward();
                    return;
                }
                Some(HoverTarget::Reload) => {
                    self.reload();
                    return;
                }
                Some(HoverTarget::Home) => {
                    self.navigate("about:welcome");
                    return;
                }
                Some(HoverTarget::NewTab) => {
                    self.new_tab();
                    return;
                }
                Some(HoverTarget::Tab(i)) => {
                    self.switch_tab(i);
                    return;
                }
                Some(HoverTarget::TabClose(i)) => {
                    self.close_tab(i);
                    return;
                }
                Some(HoverTarget::AddressBar) => {
                    self.clear_form_focus();
                    self.address_focused = true;
                    self.is_all_selected = true;
                    self.cursor_pos = self.address_text.len();
                    return;
                }
                Some(HoverTarget::Menu) => {
                    self.status_text = "Mango 2026 — Pure Rust Lightweight Browser".to_string();
                    return;
                }
                Some(HoverTarget::ScrollbarThumb) => {
                    self.scrollbar_dragging = true;
                    self.drag_start_mouse_y = self.mouse_y;
                    self.drag_start_scroll_y = self.scroll_y;
                    return;
                }
                Some(HoverTarget::ScrollbarTrack) => {
                    let content_y_top = HEADER_HEIGHT + 1.0;
                    let content_h =
                        (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
                    let max_s = self.max_scroll();
                    if max_s > 0.0 {
                        let click_ratio =
                            ((self.mouse_y - content_y_top) / content_h).clamp(0.0, 1.0);
                        self.scroll_y = (click_ratio * max_s).clamp(0.0, max_s);
                        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                            tab.scroll_y = self.scroll_y;
                        }
                        self.scrollbar_dragging = true;
                        self.drag_start_mouse_y = self.mouse_y;
                        self.drag_start_scroll_y = self.scroll_y;
                    }
                    return;
                }
                None => {}
            }

            // Click outside toolbar / tabs unfocuses address bar
            if self.mouse_y > HEADER_HEIGHT && self.mouse_y < self.height as f32 - STATUS_BAR_HEIGHT
            {
                self.address_focused = false;
                self.is_all_selected = false;

                // Direct hit test on scrollbar track or thumb on left-click
                let scrollbar_w = 14.0f32;
                let max_s = self.max_scroll();
                if max_s > 0.0 && self.mouse_x >= self.width as f32 - scrollbar_w {
                    let content_y_top = HEADER_HEIGHT + 1.0;
                    let content_h =
                        (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
                    if self.mouse_y >= content_y_top && self.mouse_y <= content_y_top + content_h {
                        let doc_h = self.scrollable_height();
                        let thumb_h = ((content_h / doc_h) * content_h)
                            .clamp(32.0f32.min(content_h), content_h);
                        let max_thumb_travel = (content_h - thumb_h).max(1.0);
                        let thumb_y = content_y_top + (self.scroll_y / max_s) * max_thumb_travel;

                        if self.mouse_y >= thumb_y && self.mouse_y <= thumb_y + thumb_h {
                            self.scrollbar_dragging = true;
                            self.drag_start_mouse_y = self.mouse_y;
                            self.drag_start_scroll_y = self.scroll_y;
                        } else {
                            let click_ratio =
                                ((self.mouse_y - content_y_top) / content_h).clamp(0.0, 1.0);
                            self.scroll_y = (click_ratio * max_s).clamp(0.0, max_s);
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                                tab.scroll_y = self.scroll_y;
                            }
                            self.scrollbar_dragging = true;
                            self.drag_start_mouse_y = self.mouse_y;
                            self.drag_start_scroll_y = self.scroll_y;
                        }
                        return;
                    }
                }

                let content_x = self.mouse_x;
                let content_y = self.mouse_y - HEADER_HEIGHT + self.scroll_y;

                // Hit-test media controls (<video>, <audio>)
                if let Some(hit) = self.root_box.as_ref().and_then(|root| {
                    root.hit_test_media_control(mango_core::Point::new(content_x, content_y))
                }) {
                    self.handle_media_control_click(hit);
                    return;
                }

                // Hit-test form controls (<input>, <button>, <select>, <textarea>)
                if let Some(hit) = self.root_box.as_ref().and_then(|root| {
                    root.hit_test_form_control(mango_core::Point::new(content_x, content_y))
                }) {
                    self.handle_form_control_click(hit);
                    return;
                }

                // If clicked normal content, clear form focus
                self.clear_form_focus();
            }
        }
    }

    /// Clears any active focus on form controls in the document, firing a
    /// trailing `change` event if the value was edited while focused.
    pub fn clear_form_focus(&mut self) {
        if let Some(old_id) = self.focused_control.take() {
            let initial = self.focused_initial_value.take();
            let current = self
                .js_runtime
                .as_ref()
                .and_then(|rt| rt.get_node_attribute(old_id, "value"));
            if let Some(ref mut rt) = self.js_runtime {
                rt.remove_node_attribute(old_id, "data-mango-focused");
                rt.remove_node_attribute(old_id, "data-mango-focus-visible");
                rt.remove_node_attribute(old_id, "_mango_cursor_pos");
            }
            self.control_cursor_pos = 0;
            self.fire_dom_event(old_id, "blur");
            if let (Some(initial), Some(current)) = (initial, current)
                && initial != current
            {
                self.fire_dom_event(old_id, "change");
            }
            self.relayout();
        }
    }

    /// Sets keyboard focus onto a specific DOM form control element.
    pub fn focus_control(&mut self, node_id: mango_html::dom::NodeId) {
        if self.focused_control != Some(node_id) {
            self.clear_form_focus();
            self.focused_control = Some(node_id);
            if let Some(ref mut rt) = self.js_runtime {
                rt.set_node_attribute(node_id, "data-mango-focused", "true");
                rt.set_node_attribute(node_id, "data-mango-focus-visible", "true");
            }
            self.fire_dom_event(node_id, "focus");
            self.relayout();
        }
    }

    /// Collects all interactive elements in DOM tree order that can receive keyboard focus.
    pub fn collect_focusable_elements(&self) -> Vec<mango_html::dom::NodeId> {
        let doc = self.document_view();
        let mut focusable = Vec::new();
        let mut stack = vec![doc.root()];

        while let Some(nid) = stack.pop() {
            if let Some(node) = doc.get(nid) {
                if let mango_html::dom::NodeData::Element(el) = &node.data {
                    let tag = el.tag_name.to_ascii_lowercase();
                    let disabled = el.get_attribute("disabled").is_some();
                    let tabindex_opt = el
                        .get_attribute("tabindex")
                        .and_then(|s| s.trim().parse::<i32>().ok());
                    let is_hidden_input = tag == "input"
                        && el
                            .get_attribute("type")
                            .map(|t| t.eq_ignore_ascii_case("hidden"))
                            .unwrap_or(false);

                    let can_focus = if disabled || is_hidden_input {
                        false
                    } else if let Some(ti) = tabindex_opt {
                        ti >= 0
                    } else {
                        matches!(tag.as_str(), "input" | "button" | "select" | "textarea")
                            || (tag == "a" && el.get_attribute("href").is_some())
                            || el
                                .get_attribute("contenteditable")
                                .map(|v| v.eq_ignore_ascii_case("true") || v.is_empty())
                                .unwrap_or(false)
                    };

                    if can_focus {
                        focusable.push(nid);
                    }
                }

                // Push children in reverse order so pop() visits in document order
                let children: Vec<_> = doc.children(nid).collect();
                for child in children.into_iter().rev() {
                    stack.push(child.id);
                }
            }
        }

        focusable
    }

    /// Advances or reverses focus among interactive elements on Tab / Shift+Tab.
    pub fn focus_next_element(&mut self, reverse: bool) {
        let focusable = self.collect_focusable_elements();
        if focusable.is_empty() {
            return;
        }

        let next_idx = match self.focused_control {
            Some(cur_id) => {
                if let Some(pos) = focusable.iter().position(|&id| id == cur_id) {
                    if reverse {
                        if pos == 0 {
                            focusable.len() - 1
                        } else {
                            pos - 1
                        }
                    } else {
                        (pos + 1) % focusable.len()
                    }
                } else if reverse {
                    focusable.len() - 1
                } else {
                    0
                }
            }
            None => {
                if reverse {
                    focusable.len() - 1
                } else {
                    0
                }
            }
        };

        let target_node = focusable[next_idx];
        self.focus_control(target_node);
    }

    /// Handles clicks on HTML5 <video> and <audio> media controls.
    fn handle_media_control_click(&mut self, hit: mango_layout::MediaControlHit) {
        let Some(node_id) = hit.node_id else {
            return;
        };
        match hit.action {
            mango_layout::MediaClickAction::TogglePlayPause => {
                let next_playing = !hit.is_playing;
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_node_attribute(
                        node_id,
                        "data-mango-playing",
                        if next_playing { "true" } else { "false" },
                    );
                    let evt_type = if next_playing { "play" } else { "pause" };
                    let script = format!(
                        "if (typeof document !== 'undefined') {{ var el = document.querySelector('[data-mango-playing]'); if (el) el.dispatchEvent({{ type: '{}' }}); }}",
                        evt_type
                    );
                    let _ = rt.execute_script(&script);
                }
                self.status_text = if next_playing {
                    if hit.is_video {
                        "Video playing".to_string()
                    } else {
                        "Audio playing".to_string()
                    }
                } else {
                    if hit.is_video {
                        "Video paused".to_string()
                    } else {
                        "Audio paused".to_string()
                    }
                };
                self.relayout();
            }
            mango_layout::MediaClickAction::ToggleMute => {
                let next_muted = !hit.is_muted;
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_node_attribute(
                        node_id,
                        "data-mango-muted",
                        if next_muted { "true" } else { "false" },
                    );
                    let script = "if (typeof document !== 'undefined') { var el = document.querySelector('[data-mango-muted]'); if (el) el.dispatchEvent({ type: 'volumechange' }); }";
                    let _ = rt.execute_script(script);
                }
                self.status_text = if next_muted {
                    "Media muted".to_string()
                } else {
                    "Media unmuted".to_string()
                };
                self.relayout();
            }
            mango_layout::MediaClickAction::Seek(target_time) => {
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_node_attribute(
                        node_id,
                        "data-mango-time",
                        &format!("{:.1}", target_time),
                    );
                    let script = "if (typeof document !== 'undefined') { var el = document.querySelector('[data-mango-time]'); if (el) el.dispatchEvent({ type: 'timeupdate' }); }";
                    let _ = rt.execute_script(script);
                }
                self.status_text = format!(
                    "Seek to {:02}:{:02}",
                    (target_time as u32) / 60,
                    (target_time as u32) % 60
                );
                self.relayout();
            }
            mango_layout::MediaClickAction::ToggleFullscreen => {
                self.status_text = "Fullscreen toggled".to_string();
                self.relayout();
            }
        }
    }

    /// Returns the content rect `(x, y, w, h)` of a laid-out control, in document coordinates.
    pub(crate) fn control_content_rect(&self, node_id: NodeId) -> Option<(f32, f32, f32, f32)> {
        let root = self.root_box.as_ref()?;
        let lbox = root.find_box_for_node(node_id)?;
        let c = lbox.dimensions.content;
        Some((c.x(), c.y(), c.width(), c.height()))
    }

    /// Writes a form control's value to both the `value` and `data-mango-value`
    /// attributes — keeping the JS `el.value` surface and `:valid`/`:placeholder-shown`
    /// selector matching in sync — and fires `input` when the value changed.
    pub fn set_control_value(&mut self, node_id: NodeId, new_val: &str) {
        let old = self
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, "value"))
            .unwrap_or_default();
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_node_attribute(node_id, "value", new_val);
            rt.set_node_attribute(node_id, "data-mango-value", new_val);
        }
        if old != new_val {
            self.fire_dom_event(node_id, "input");
        }
    }

    /// Dispatches a DOM event (`input`/`change`/`click`) to a page element by
    /// resolving its cached wrapper through `_mangoWrap` (falling back to a
    /// marker attribute + `querySelector` for older shims).
    fn fire_dom_event(&mut self, node_id: NodeId, event_type: &str) {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_node_attribute(node_id, "data-mango-fire", event_type);
            let script = format!(
                "if (typeof _mangoWrap === 'function') {{ var __el = _mangoWrap({}); if (__el) __el.dispatchEvent({{ type: '{}' }}); }} else if (typeof document !== 'undefined') {{ var __el = document.querySelector('[data-mango-fire]'); if (__el) {{ __el.dispatchEvent({{ type: '{}' }}); __el.removeAttribute('data-mango-fire'); }} }}",
                node_id.raw(),
                event_type,
                event_type
            );
            let _ = rt.execute_script(&script);
            rt.remove_node_attribute(node_id, "data-mango-fire");
        }
        // Committing a `change` while the control is focused re-bases the focus
        // snapshot so a later blur does not fire a duplicate `change`.
        if event_type == "change" && self.focused_control == Some(node_id) {
            self.focused_initial_value = self
                .js_runtime
                .as_ref()
                .and_then(|rt| rt.get_node_attribute(node_id, "value"));
        }
    }

    /// Parses `(min, max, step)` for a numeric input per HTML defaults
    /// (`step` defaults to 1 and must be positive).
    fn numeric_params(&self, node_id: NodeId) -> (Option<f32>, Option<f32>, f32) {
        let (min, max, step) = self
            .js_runtime
            .as_ref()
            .map(|rt| {
                (
                    rt.get_node_attribute(node_id, "min"),
                    rt.get_node_attribute(node_id, "max"),
                    rt.get_node_attribute(node_id, "step"),
                )
            })
            .unwrap_or((None, None, None));
        let min_v = min.and_then(|v| v.parse::<f32>().ok());
        let max_v = max.and_then(|v| v.parse::<f32>().ok());
        let step_v = step
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(1.0);
        (min_v, max_v, step_v)
    }

    /// Computes the value of a `range` slider for a click fraction (0..=1) across it,
    /// snapped to `step` and clamped to `min`/`max`.
    fn range_value_for_fraction(&self, node_id: NodeId, frac: f32) -> f32 {
        let (min_v, max_v, step) = self.numeric_params(node_id);
        let min_v = min_v.unwrap_or(0.0);
        let max_v = max_v.unwrap_or(100.0);
        let lo = min_v.min(max_v);
        let hi = min_v.max(max_v);
        let span = hi - lo;
        let mut val = lo + frac.clamp(0.0, 1.0) * span;
        if span > f32::EPSILON && step > 0.0 {
            val = lo + ((val - lo) / step).round() * step;
        }
        val.clamp(lo, hi)
    }

    /// Applies a ±step adjustment to a focused `range`/`number` control, clamped
    /// to `min`/`max`, firing `input` + `change` (used by ArrowUp/ArrowDown and
    /// by the number spinner arrows).
    fn step_numeric_control(&mut self, node_id: NodeId, dir: f32) {
        let (min_v, max_v, step) = self.numeric_params(node_id);
        let base = min_v.unwrap_or(0.0);
        let cur = self
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, "value"))
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(base);
        let mut next = base + ((cur - base) / step + dir).round() * step;
        if let Some(mn) = min_v {
            next = next.max(mn);
        }
        if let Some(mx) = max_v {
            next = next.min(mx);
        }
        let formatted = fmt_number(next);
        self.set_control_value(node_id, &formatted);
        self.fire_dom_event(node_id, "change");
        self.relayout();
        self.status_text = format!("Value: {}", formatted);
    }

    /// Gives keyboard focus to a text-like form control and places the caret at
    /// the clicked character position. Snapshots the initial value so a trailing
    /// `change` event can be fired when focus moves away.
    fn focus_text_control(&mut self, node_id: NodeId, hit: &mango_layout::FormControlHit) {
        if self.focused_control != Some(node_id) {
            self.clear_form_focus();
            self.focused_control = Some(node_id);
            self.focused_initial_value = self
                .js_runtime
                .as_ref()
                .and_then(|rt| rt.get_node_attribute(node_id, "value"));
        }
        let current_val = if let Some(ref rt) = self.js_runtime {
            rt.get_node_attribute(node_id, "value").unwrap_or_default()
        } else {
            hit.value.clone()
        };

        // Calculate clicked character position from hit.click_offset_x
        let chars: Vec<char> = current_val.chars().collect();
        if chars.is_empty() || hit.click_offset_x <= 0.0 {
            self.control_cursor_pos = 0;
        } else {
            let mut best_idx = chars.len();
            for i in 0..=chars.len() {
                let sub: String = chars[..i].iter().collect();
                let w = mango_layout::measure_text_width(&sub, 14.0);
                if w >= hit.click_offset_x {
                    if i > 0 {
                        let prev_sub: String = chars[..i - 1].iter().collect();
                        let prev_w = mango_layout::measure_text_width(&prev_sub, 14.0);
                        if (hit.click_offset_x - prev_w).abs() < (w - hit.click_offset_x).abs() {
                            best_idx = i - 1;
                        } else {
                            best_idx = i;
                        }
                    } else {
                        best_idx = 0;
                    }
                    break;
                }
            }
            self.control_cursor_pos = best_idx.min(chars.len());
        }

        if let Some(ref mut rt) = self.js_runtime {
            rt.set_node_attribute(node_id, "data-mango-focused", "true");
            rt.set_node_attribute(
                node_id,
                "_mango_cursor_pos",
                &self.control_cursor_pos.to_string(),
            );
        }
        self.relayout();
        self.status_text = format!("Focused {} field", hit.form_type);
    }

    /// Opens a picker overlay anchored below the clicked control (above it when
    /// there is no room below), clamped into the window's content area.
    fn open_picker(&mut self, node_id: NodeId, kind: PickerKind) {
        let (pw, ph) = picker_size(&kind);
        let Some((cx, cy, _cw, ch)) = self.control_content_rect(node_id) else {
            return;
        };
        let screen_top = HEADER_HEIGHT + cy - self.scroll_y;
        let px = cx.min((self.width as f32 - pw - 4.0).max(0.0));
        let mut py = screen_top + ch + 2.0;
        if py + ph > self.height as f32 - STATUS_BAR_HEIGHT {
            let above = screen_top - ph - 2.0;
            py = if above >= HEADER_HEIGHT {
                above
            } else {
                (self.height as f32 - STATUS_BAR_HEIGHT - ph).max(HEADER_HEIGHT)
            };
        }
        self.picker = Some(PickerOverlay {
            node_id,
            kind,
            x: px,
            y: py,
            width: pw,
            height: ph,
            hover: None,
        });
    }

    /// Handles a click while a picker overlay is open: inside clicks act on the
    /// picker, inert inside clicks are ignored, and outside clicks dismiss it.
    fn handle_picker_click(&mut self) {
        let Some(picker) = self.picker.clone() else {
            return;
        };
        let inside = self.mouse_x >= picker.x
            && self.mouse_x <= picker.x + picker.width
            && self.mouse_y >= picker.y
            && self.mouse_y <= picker.y + picker.height;
        let hit = if inside {
            picker_hit_test(&picker, self.mouse_x, self.mouse_y)
        } else {
            None
        };
        let node_id = picker.node_id;

        match hit {
            None if inside => {}
            None => {
                self.picker = None;
            }
            Some(PickerHit::ColorSwatch(i)) => {
                let color = mango_layout::COLOR_SWATCHES
                    .get(i as usize)
                    .copied()
                    .unwrap_or("#000000");
                self.set_control_value(node_id, color);
                self.fire_dom_event(node_id, "change");
                self.picker = None;
                self.relayout();
                self.status_text = format!("Color: {}", color.to_ascii_uppercase());
            }
            Some(PickerHit::PrevMonth) => {
                if let Some(p) = self.picker.as_mut()
                    && let PickerKind::Date { year, month } = &mut p.kind
                {
                    if *month <= 1 {
                        *month = 12;
                        *year -= 1;
                    } else {
                        *month -= 1;
                    }
                    p.hover = None;
                }
            }
            Some(PickerHit::NextMonth) => {
                if let Some(p) = self.picker.as_mut()
                    && let PickerKind::Date { year, month } = &mut p.kind
                {
                    if *month >= 12 {
                        *month = 1;
                        *year += 1;
                    } else {
                        *month += 1;
                    }
                    p.hover = None;
                }
            }
            Some(PickerHit::Day(day)) => {
                if let PickerKind::Date { year, month } = picker.kind {
                    let value = format!("{:04}-{:02}-{:02}", year, month, day);
                    self.set_control_value(node_id, &value);
                    self.fire_dom_event(node_id, "change");
                    self.picker = None;
                    self.relayout();
                    self.status_text = format!("Date: {}", value);
                }
            }
            Some(PickerHit::HourUp) => {
                if let Some(p) = self.picker.as_mut()
                    && let PickerKind::Time { hour, .. } = &mut p.kind
                {
                    *hour = (*hour + 1) % 24;
                    p.hover = None;
                }
            }
            Some(PickerHit::HourDown) => {
                if let Some(p) = self.picker.as_mut()
                    && let PickerKind::Time { hour, .. } = &mut p.kind
                {
                    *hour = (*hour + 23) % 24;
                    p.hover = None;
                }
            }
            Some(PickerHit::MinUp) => {
                if let Some(p) = self.picker.as_mut()
                    && let PickerKind::Time {
                        minute,
                        minute_step,
                        ..
                    } = &mut p.kind
                {
                    *minute = (*minute + *minute_step) % 60;
                    p.hover = None;
                }
            }
            Some(PickerHit::MinDown) => {
                if let Some(p) = self.picker.as_mut()
                    && let PickerKind::Time {
                        minute,
                        minute_step,
                        ..
                    } = &mut p.kind
                {
                    *minute = (*minute + 60 - (*minute_step % 60)) % 60;
                    p.hover = None;
                }
            }
            Some(PickerHit::SetTime) => {
                if let PickerKind::Time { hour, minute, .. } = picker.kind {
                    let value = format!("{:02}:{:02}", hour, minute);
                    self.set_control_value(node_id, &value);
                    self.fire_dom_event(node_id, "change");
                    self.picker = None;
                    self.relayout();
                    self.status_text = format!("Time: {}", value);
                }
            }
            Some(PickerHit::FileParent) => {
                if let PickerKind::File { dir, .. } = picker.kind
                    && let Some(parent) = dir.parent().map(|p| p.to_path_buf())
                {
                    let entries = list_directory(&parent);
                    if let Some(p) = self.picker.as_mut() {
                        p.kind = PickerKind::File {
                            dir: parent,
                            entries,
                            offset: 0,
                        };
                        p.hover = None;
                    }
                }
            }
            Some(PickerHit::FileRow(idx)) => {
                if let PickerKind::File { dir, entries, .. } = picker.kind {
                    let idx = idx as usize;
                    if let Some(entry) = entries.get(idx) {
                        if entry.is_dir {
                            let new_dir = dir.join(&entry.name);
                            let entries = list_directory(&new_dir);
                            if let Some(p) = self.picker.as_mut() {
                                p.kind = PickerKind::File {
                                    dir: new_dir,
                                    entries,
                                    offset: 0,
                                };
                                p.hover = None;
                            }
                        } else {
                            let full = dir.join(&entry.name);
                            let name = entry.name.clone();
                            self.set_control_value(node_id, &name);
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(node_id, "data-mango-filename", &name);
                                rt.set_node_attribute(
                                    node_id,
                                    "data-mango-filepath",
                                    &full.to_string_lossy(),
                                );
                            }
                            self.fire_dom_event(node_id, "change");
                            self.picker = None;
                            self.relayout();
                            self.status_text = format!("Selected file: {}", name);
                        }
                    }
                }
            }
        }
    }

    /// Recomputes a dragged slider's value from the current mouse position.
    fn update_range_drag(&mut self, node_id: NodeId) {
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

    /// Determines the initial `(year, month)` for the date picker from the
    /// control's value, falling back to today's date.
    fn picker_start_date(&self, node_id: NodeId) -> (i32, u32) {
        let val = self
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, "value"))
            .unwrap_or_default();
        if let Some((y, m, _)) = parse_date_value(&val) {
            return (y, m);
        }
        let (y, m, _) = civil_from_days(unix_days_now());
        (y, m)
    }

    /// Determines the initial `(hour, minute, minute_step)` for the time picker
    /// from the control's `value`/`step` attributes, falling back to the current
    /// time and a one-minute step (the HTML default for `<input type="time">`).
    fn picker_start_time(&self, node_id: NodeId) -> (u32, u32, u32) {
        let (value, step) = self
            .js_runtime
            .as_ref()
            .map(|rt| {
                (
                    rt.get_node_attribute(node_id, "value"),
                    rt.get_node_attribute(node_id, "step"),
                )
            })
            .unwrap_or((None, None));
        let (hour, minute) = value
            .as_deref()
            .and_then(parse_time_value)
            .unwrap_or_else(|| {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (((secs % 86_400) / 3600) as u32, ((secs % 3600) / 60) as u32)
            });
        let minute_step = step
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| ((s / 60.0).round() as i64).clamp(1, 30) as u32)
            .unwrap_or(1);
        (hour, minute, minute_step)
    }

    /// Handles clicks on interactive form controls.
    fn handle_form_control_click(&mut self, hit: mango_layout::FormControlHit) {
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
                // BUG-004 fix: Uncheck all sibling radio buttons with the same name
                self.fire_dom_event(node_id, "click");
                if let Some(ref mut rt) = self.js_runtime {
                    let radio_name = rt.get_node_attribute(node_id, "name").unwrap_or_default();
                    if !radio_name.is_empty() {
                        let radios_to_uncheck = {
                            let doc = rt.document_ref();
                            let form_root = {
                                let mut curr = Some(node_id);
                                let mut found_form = None;
                                while let Some(nid) = curr {
                                    if let Some(node) = doc.get(nid) {
                                        if let NodeData::Element(el) = &node.data
                                            && el.tag_name.eq_ignore_ascii_case("form")
                                        {
                                            found_form = Some(nid);
                                            break;
                                        }
                                        curr = node.parent;
                                    } else {
                                        break;
                                    }
                                }
                                found_form.unwrap_or(doc.root())
                            };
                            let mut radios = Vec::new();
                            let mut stack = vec![form_root];
                            while let Some(nid) = stack.pop() {
                                if let Some(node) = doc.get(nid) {
                                    if let NodeData::Element(el) = &node.data
                                        && el.tag_name.eq_ignore_ascii_case("input")
                                        && el
                                            .get_attribute("type")
                                            .map(|t| t.eq_ignore_ascii_case("radio"))
                                            .unwrap_or(false)
                                        && el
                                            .get_attribute("name")
                                            .map(|n| n == radio_name)
                                            .unwrap_or(false)
                                        && nid != node_id
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
                        };
                        // Uncheck all siblings
                        for rid in radios_to_uncheck {
                            rt.remove_node_attribute(rid, "checked");
                        }
                    }
                    // Check the clicked radio
                    rt.set_node_attribute(node_id, "checked", "true");
                }
                if let Some(ref mut doc) = self.cached_document {
                    let radio_name = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(node_id, "name"))
                        .unwrap_or_default();
                    if !radio_name.is_empty() {
                        let mut stack = vec![doc.root()];
                        while let Some(nid) = stack.pop() {
                            if let Some(node) = doc.get_mut(nid) {
                                if let NodeData::Element(ref mut el) = node.data
                                    && el.tag_name.eq_ignore_ascii_case("input")
                                    && el
                                        .get_attribute("type")
                                        .map(|t| t.eq_ignore_ascii_case("radio"))
                                        .unwrap_or(false)
                                    && el
                                        .get_attribute("name")
                                        .map(|n| n == radio_name)
                                        .unwrap_or(false)
                                {
                                    if nid == node_id {
                                        el.set_attribute("checked", "true");
                                    } else {
                                        el.remove_attribute("checked");
                                    }
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
                if let Some(node) = doc.get(node_id) {
                    let mut child = node.first_child;
                    while let Some(cid) = child {
                        if let Some(cnode) = doc.get(cid) {
                            if let NodeData::Element(el) = &cnode.data
                                && el.tag_name.eq_ignore_ascii_case("option")
                            {
                                // BUG-008 fix: Use recursive text_content to handle nested inline elements
                                let opt_text = doc.text_content(cid);
                                let text = if opt_text.trim().is_empty() {
                                    el.get_attribute("value").unwrap_or("").to_string()
                                } else {
                                    opt_text.trim().to_string()
                                };
                                let value = el.get_attribute("value").unwrap_or(&text).to_string();
                                let selected =
                                    el.get_attribute("selected").is_some() || hit.value == value;
                                options.push(SelectDropdownOption {
                                    node_id: cid,
                                    text,
                                    value,
                                    selected,
                                });
                            }
                            child = cnode.next_sibling;
                        } else {
                            break;
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
                let mut curr = doc.get(node_id).and_then(|n| n.parent);
                let mut details_node = None;
                while let Some(nid) = curr {
                    if let Some(node) = doc.get(nid) {
                        if let NodeData::Element(el) = &node.data
                            && el.tag_name.eq_ignore_ascii_case("details")
                        {
                            details_node = Some(nid);
                            break;
                        }
                        curr = node.parent;
                    } else {
                        break;
                    }
                }

                if let Some(det_id) = details_node {
                    let is_open = if let Some(det_node) = doc.get(det_id)
                        && let NodeData::Element(el) = &det_node.data
                    {
                        el.get_attribute("open").is_some()
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
