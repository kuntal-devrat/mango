//! Form picker overlay lifecycle: open, click, date/time calculation, and dismissal.

use mango_html::dom::NodeId;

use super::super::BrowserChrome;
use crate::chrome_ui::*;
use crate::form_handler::*;

impl BrowserChrome {
    /// Opens a picker overlay anchored below the clicked control (above it when
    /// there is no room below), clamped into the window's content area.
    pub(crate) fn open_picker(&mut self, node_id: NodeId, kind: PickerKind) {
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
    pub(crate) fn handle_picker_click(&mut self) {
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

    /// Determines the initial `(year, month)` for the date picker from the
    /// control's value, falling back to today's date.
    pub(crate) fn picker_start_date(&self, node_id: NodeId) -> (i32, u32) {
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
    pub(crate) fn picker_start_time(&self, node_id: NodeId) -> (u32, u32, u32) {
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
}
