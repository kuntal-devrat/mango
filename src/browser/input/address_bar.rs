//! Address bar and Omnibox keyboard input handling.

use super::super::BrowserChrome;
use mango_platform::input::{KeyEvent, MangoKey};

impl BrowserChrome {
    /// Handles keyboard input when the address bar is focused.
    pub fn handle_address_bar_key(&mut self, event: &KeyEvent) {
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

    /// Handles IME text commit into either the address bar or focused form control.
    pub fn handle_ime_commit(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.address_focused {
            if self.is_all_selected {
                self.address_text = text.to_string();
                self.cursor_pos = self.address_text.len();
                self.is_all_selected = false;
            } else {
                let safe_pos = self.cursor_pos.min(self.address_text.len());
                self.address_text.insert_str(safe_pos, text);
                self.cursor_pos = safe_pos + text.len();
            }
            return;
        }

        if let Some(node_id) = self.focused_control {
            let current_val = self
                .js_runtime
                .as_ref()
                .and_then(|rt| rt.get_node_attribute(node_id, "value"))
                .unwrap_or_default();
            let chars: Vec<char> = current_val.chars().collect();
            let idx = self.control_cursor_pos.min(chars.len());
            let mut new_chars = Vec::with_capacity(chars.len() + text.chars().count());
            new_chars.extend_from_slice(&chars[..idx]);
            new_chars.extend(text.chars());
            new_chars.extend_from_slice(&chars[idx..]);
            self.control_cursor_pos = idx + text.chars().count();
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
    }
}
