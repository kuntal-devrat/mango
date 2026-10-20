//! Form control focus, tab navigation, media player controls, and value manipulation.

use mango_html::dom::NodeId;

use super::super::BrowserChrome;
use crate::form_handler::*;

impl BrowserChrome {
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
    pub(crate) fn handle_media_control_click(&mut self, hit: mango_layout::MediaControlHit) {
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
    pub(crate) fn fire_dom_event(&mut self, node_id: NodeId, event_type: &str) {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_node_attribute(node_id, "data-mango-fire", event_type);
            let script = format!(
                r#"if (typeof _mangoWrap === 'function') {{
                    var __el = _mangoWrap({});
                    if (__el) {{
                        var __evt;
                        if (typeof PointerEvent === 'function' && '{evt}'.startsWith('pointer')) {{
                            __evt = new PointerEvent('{evt}', {{ bubbles: true, cancelable: true, view: window }});
                        }} else if (typeof MouseEvent === 'function' && ('{evt}' === 'click' || '{evt}' === 'mousedown' || '{evt}' === 'mouseup')) {{
                            __evt = new MouseEvent('{evt}', {{ bubbles: true, cancelable: true, view: window }});
                        }} else if (typeof Event === 'function') {{
                            __evt = new Event('{evt}', {{ bubbles: true, cancelable: true }});
                        }} else {{
                            __evt = {{ type: '{evt}', bubbles: true, cancelable: true }};
                        }}
                        __el.dispatchEvent(__evt);
                    }}
                }} else if (typeof document !== 'undefined') {{
                    var __el = document.querySelector('[data-mango-fire]');
                    if (__el) {{
                        var __evt = (typeof Event === 'function') ? new Event('{evt}', {{ bubbles: true, cancelable: true }}) : {{ type: '{evt}', bubbles: true }};
                        __el.dispatchEvent(__evt);
                        __el.removeAttribute('data-mango-fire');
                    }}
                }}"#,
                node_id.raw(),
                evt = event_type
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
    pub(crate) fn numeric_params(&self, node_id: NodeId) -> (Option<f32>, Option<f32>, f32) {
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
    pub(crate) fn range_value_for_fraction(&self, node_id: NodeId, frac: f32) -> f32 {
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
    pub(crate) fn step_numeric_control(&mut self, node_id: NodeId, dir: f32) {
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
    pub(crate) fn focus_text_control(
        &mut self,
        node_id: NodeId,
        hit: &mango_layout::FormControlHit,
    ) {
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
}
