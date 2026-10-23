//! Mouse wheel scrolling, trackpad navigation, and keyboard event dispatching.

use super::super::BrowserChrome;
use crate::chrome_ui::*;
use crate::form_handler::*;
use mango_platform::input::{KeyEvent, MangoKey};

impl BrowserChrome {
    /// Handles mouse wheel scrolling with clamping.
    pub fn handle_scroll(&mut self, delta_y: f32) {
        if self.devtools_scroll(delta_y) {
            return;
        }
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
            let delta = if delta_y < 0.0 {
                2
            } else if delta_y > 0.0 {
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
}
