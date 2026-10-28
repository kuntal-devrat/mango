//! Mouse button click handling, context menus, link navigation, and UI control actions.

use mango_platform::input::{KeyState, MouseButton};

use super::super::BrowserChrome;
use crate::chrome_ui::*;
use crate::context_menu::*;

impl BrowserChrome {
    /// Handles mouse clicks on UI elements, context menu, form inputs, or links in content.
    pub fn handle_mouse_click(&mut self, button: MouseButton, state: KeyState) {
        // Middle click (auxiliary click) on links opens in a background tab
        if state == KeyState::Released && button == MouseButton::Middle {
            if self.mouse_y > HEADER_HEIGHT && self.mouse_y < self.height as f32 - STATUS_BAR_HEIGHT
            {
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

        if button == MouseButton::Left && self.devtools_hit_test(state) {
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
                        self.fire_dom_event(node_id, "pointerup");
                        self.fire_dom_event(node_id, "mouseup");
                        self.fire_dom_event(node_id, "click");
                    }
                    let nav_url = if let Some(base) = &self.base_url {
                        base.resolve(&target_href)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| target_href.clone())
                    } else {
                        target_href
                    };
                    let nav_url =
                        crate::browser::navigation::pipeline::unwrap_tracking_redirect(&nav_url)
                            .unwrap_or(nav_url);
                    log::info!(
                        "Navigating to clicked link: {} (target={:?})",
                        nav_url,
                        target_frame
                    );
                    link_to_navigate = Some((nav_url, target_frame));
                } else if let Some(node_id) = hit_node {
                    self.fire_dom_event(node_id, "pointerup");
                    self.fire_dom_event(node_id, "mouseup");
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
            if let Some(node_id) = active_node {
                self.fire_dom_event(node_id, "pointerdown");
                self.fire_dom_event(node_id, "mousedown");
            }
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
}
