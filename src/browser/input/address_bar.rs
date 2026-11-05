//! Address bar and Omnibox keyboard input handling.

use super::super::BrowserChrome;
use super::text_editing;
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
                MangoKey::ArrowLeft => {
                    self.is_all_selected = false;
                    self.cursor_pos =
                        text_editing::prev_word_boundary(&self.address_text, self.cursor_pos);
                    return;
                }
                MangoKey::ArrowRight => {
                    self.is_all_selected = false;
                    self.cursor_pos =
                        text_editing::next_word_boundary(&self.address_text, self.cursor_pos);
                    return;
                }
                MangoKey::Backspace => {
                    if self.is_all_selected {
                        self.address_text.clear();
                        self.cursor_pos = 0;
                        self.is_all_selected = false;
                    } else {
                        self.cursor_pos =
                            text_editing::backspace_word(&mut self.address_text, self.cursor_pos);
                    }
                    return;
                }
                MangoKey::Delete => {
                    if self.is_all_selected {
                        self.address_text.clear();
                        self.cursor_pos = 0;
                        self.is_all_selected = false;
                    } else {
                        text_editing::delete_word(&mut self.address_text, self.cursor_pos);
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
                    self.cursor_pos =
                        text_editing::backspace_grapheme(&mut self.address_text, self.cursor_pos);
                }
            }
            MangoKey::Delete => {
                if self.is_all_selected {
                    self.address_text.clear();
                    self.cursor_pos = 0;
                    self.is_all_selected = false;
                } else if self.cursor_pos < self.address_text.len() {
                    text_editing::delete_grapheme(&mut self.address_text, self.cursor_pos);
                }
            }
            MangoKey::ArrowLeft => {
                self.is_all_selected = false;
                self.cursor_pos =
                    text_editing::prev_grapheme_offset(&self.address_text, self.cursor_pos);
            }
            MangoKey::ArrowRight => {
                self.is_all_selected = false;
                self.cursor_pos =
                    text_editing::next_grapheme_offset(&self.address_text, self.cursor_pos);
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
            let mut current_val = self
                .js_runtime
                .as_ref()
                .and_then(|rt| {
                    rt.get_node_attribute(node_id, "value")
                        .or_else(|| rt.get_node_attribute(node_id, "data-mango-value"))
                })
                .or_else(|| {
                    self.cached_document.as_ref().and_then(|doc| {
                        doc.get(node_id)
                            .and_then(|n| n.as_element())
                            .and_then(|el| {
                                el.get_attribute("value")
                                    .or_else(|| el.get_attribute("data-mango-value"))
                            })
                            .map(|s| s.to_string())
                    })
                })
                .unwrap_or_default();
            let safe_pos = self.control_cursor_pos.min(current_val.len());
            current_val.insert_str(safe_pos, text);
            self.control_cursor_pos = safe_pos + text.len();
            self.set_control_value(node_id, &current_val);
            self.sync_cursor_pos(node_id);
            self.relayout();
        }
    }

    /// Handles IME active preedit composition updates.
    pub fn handle_ime_preedit(&mut self, text: &str, _cursor: Option<(usize, usize)>) {
        if text.is_empty() {
            self.status_text = "Ready".to_string();
        } else {
            self.status_text = format!("IME composing: {text}");
        }
        self.relayout();
    }
}
