//! Mouse movement tracking, hover hit-testing, and cursor styling.

use super::super::BrowserChrome;
use crate::chrome_ui::*;
use crate::form_handler::*;

impl BrowserChrome {
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
}
