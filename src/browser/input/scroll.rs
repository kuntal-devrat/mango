//! Mouse wheel scrolling and trackpad navigation.

use super::super::BrowserChrome;
use crate::chrome_ui::*;
use crate::form_handler::*;

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

    /// Handles mouse wheel scrolling with cubic ease-out physics and nested container dispatch.
    pub fn handle_wheel_scroll_smooth(&mut self, delta_x: f32, delta_y: f32) {
        let scroll_step_y = -delta_y * 100.0;
        let scroll_step_x = -delta_x * 100.0;

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
            if max > 0.0 {
                let current_target = self
                    .smooth_scroll
                    .as_ref()
                    .filter(|s| s.active)
                    .map(|s| s.target_y)
                    .unwrap_or(self.scroll_y_f64);
                let new_target = (current_target + unconsumed_y as f64).clamp(0.0, max);
                self.scroll_smooth_to_f64(new_target, Some(std::time::Duration::from_millis(180)));
            }
        }
    }
}
