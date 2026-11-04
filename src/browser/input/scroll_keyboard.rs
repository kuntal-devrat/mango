//! Mouse wheel scrolling, trackpad navigation, and keyboard event dispatching.

use super::super::BrowserChrome;
use crate::chrome_ui::*;
use mango_html::dom::NodeData;
use mango_platform::input::{KeyEvent, MangoKey};

impl BrowserChrome {
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

                let (key_str, code_str, key_code) = w3c_key_props(&event.key);
                let allowed = self.fire_keyboard_event(
                    node_id,
                    "keydown",
                    &key_str,
                    &code_str,
                    key_code,
                    event.modifiers.ctrl,
                    event.modifiers.alt,
                    event.modifiers.shift,
                    false,
                );
                if !allowed {
                    return;
                }

                match &event.key {
                    MangoKey::Char(c) => {
                        let _ = self.fire_keyboard_event(
                            node_id,
                            "keypress",
                            &key_str,
                            &code_str,
                            key_code,
                            event.modifiers.ctrl,
                            event.modifiers.alt,
                            event.modifiers.shift,
                            false,
                        );
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
                        let _ = self.fire_keyboard_event(
                            node_id,
                            "keypress",
                            " ",
                            "Space",
                            32,
                            event.modifiers.ctrl,
                            event.modifiers.alt,
                            event.modifiers.shift,
                            false,
                        );
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
                        let doc = self.active_document();
                        let is_multiline_textarea = doc.get(node_id).is_some_and(|n| {
                            if let NodeData::Element(ref el) = n.data {
                                el.tag_name.eq_ignore_ascii_case("textarea")
                                    && el
                                        .get_attribute("rows")
                                        .and_then(|r| r.parse::<u32>().ok())
                                        .unwrap_or(2)
                                        > 1
                            } else {
                                false
                            }
                        });

                        if is_multiline_textarea && !event.modifiers.ctrl {
                            let mut chars: Vec<char> = current_val.chars().collect();
                            let pos = self.control_cursor_pos.min(chars.len());
                            chars.insert(pos, '\n');
                            self.control_cursor_pos = pos + 1;
                            let new_val: String = chars.into_iter().collect();
                            self.set_control_value(node_id, &new_val);
                            self.relayout();
                            return;
                        }

                        let _ = self.fire_keyboard_event(
                            node_id,
                            "keypress",
                            "Enter",
                            "Enter",
                            13,
                            event.modifiers.ctrl,
                            event.modifiers.alt,
                            event.modifiers.shift,
                            false,
                        );
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
        self.handle_address_bar_key(event);
    }

    /// Handles key release events to fire `keyup` to the focused form control.
    pub fn handle_key_release(&mut self, event: &KeyEvent) {
        if !self.address_focused
            && let Some(node_id) = self.focused_control
        {
            let (key_str, code_str, key_code) = w3c_key_props(&event.key);
            self.fire_keyboard_event(
                node_id,
                "keyup",
                &key_str,
                &code_str,
                key_code,
                event.modifiers.ctrl,
                event.modifiers.alt,
                event.modifiers.shift,
                false,
            );
        }
    }
}

fn w3c_key_props(key: &MangoKey) -> (String, String, u32) {
    match key {
        MangoKey::Char(c) => {
            let code = if c.is_ascii_alphabetic() {
                format!("Key{}", c.to_ascii_uppercase())
            } else if c.is_ascii_digit() {
                format!("Digit{}", c)
            } else {
                match c {
                    ' ' => "Space".to_string(),
                    '-' | '_' => "Minus".to_string(),
                    '=' | '+' => "Equal".to_string(),
                    '[' | '{' => "BracketLeft".to_string(),
                    ']' | '}' => "BracketRight".to_string(),
                    ';' | ':' => "Semicolon".to_string(),
                    '\'' | '"' => "Quote".to_string(),
                    ',' | '<' => "Comma".to_string(),
                    '.' | '>' => "Period".to_string(),
                    '/' | '?' => "Slash".to_string(),
                    '\\' | '|' => "Backslash".to_string(),
                    '`' | '~' => "Backquote".to_string(),
                    _ => "Unidentified".to_string(),
                }
            };
            (c.to_string(), code, c.to_ascii_uppercase() as u32)
        }
        MangoKey::Enter => ("Enter".to_string(), "Enter".to_string(), 13),
        MangoKey::Backspace => ("Backspace".to_string(), "Backspace".to_string(), 8),
        MangoKey::Delete => ("Delete".to_string(), "Delete".to_string(), 46),
        MangoKey::Space => (" ".to_string(), "Space".to_string(), 32),
        MangoKey::Tab => ("Tab".to_string(), "Tab".to_string(), 9),
        MangoKey::Escape => ("Escape".to_string(), "Escape".to_string(), 27),
        MangoKey::ArrowLeft => ("ArrowLeft".to_string(), "ArrowLeft".to_string(), 37),
        MangoKey::ArrowUp => ("ArrowUp".to_string(), "ArrowUp".to_string(), 38),
        MangoKey::ArrowRight => ("ArrowRight".to_string(), "ArrowRight".to_string(), 39),
        MangoKey::ArrowDown => ("ArrowDown".to_string(), "ArrowDown".to_string(), 40),
        MangoKey::Home => ("Home".to_string(), "Home".to_string(), 36),
        MangoKey::End => ("End".to_string(), "End".to_string(), 35),
        MangoKey::PageUp => ("PageUp".to_string(), "PageUp".to_string(), 33),
        MangoKey::PageDown => ("PageDown".to_string(), "PageDown".to_string(), 34),
        MangoKey::F1 => ("F1".to_string(), "F1".to_string(), 112),
        MangoKey::F3 => ("F3".to_string(), "F3".to_string(), 114),
        MangoKey::F5 => ("F5".to_string(), "F5".to_string(), 116),
        MangoKey::F11 => ("F11".to_string(), "F11".to_string(), 122),
        MangoKey::F12 => ("F12".to_string(), "F12".to_string(), 123),
        MangoKey::Unknown => ("Unidentified".to_string(), "Unidentified".to_string(), 0),
    }
}
