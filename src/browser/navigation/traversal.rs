//! Back/forward history traversal and BFCache restoration.

use crate::browser::BrowserChrome;
use crate::chrome_ui::HEADER_HEIGHT;

impl BrowserChrome {
    /// Navigates back in history if possible, restoring from BFCache when available.
    pub fn go_back(&mut self) {
        let nav_info = if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if let Some(ref root) = self.root_box {
                let (w, h) = (
                    self.width as f32,
                    (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                );
                let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                    root,
                    mango_core::Size::new(w, h),
                );
                tab.store_bfcache_snapshot(Box::new(snapshot));
            }

            if let Some((url, scroll_pos)) = tab.go_back_f64() {
                let title = tab.title.clone();
                let snapshot = tab.current_bfcache_snapshot().cloned();
                Some((url, title, scroll_pos, snapshot))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((url, title, scroll_pos, snapshot)) = nav_info {
            self.address_text = url;
            self.page_title = title;
            self.cursor_pos = self.address_text.len();
            self.set_scroll_y_f64(scroll_pos);

            if let Some(snapshot) = snapshot {
                let dl = if self.scroll_y == 0.0 {
                    snapshot.display_list().clone()
                } else {
                    snapshot.display_list_with_scroll(self.scroll_y)
                };
                self.page_display_list = dl;
                self.root_box = Some(snapshot.root_box().clone());
                self.cached_scrollable_height =
                    crate::scroll::compute_scrollable_height(self.root_box.as_ref());
                self.page_dirty = true;
                self.status_text = format!("Restored from BFCache: {}", self.page_title);
                return;
            }

            self.navigate_to_address(false);
        }
    }

    /// Navigates forward in history if possible, restoring from BFCache when available.
    pub fn go_forward(&mut self) {
        let nav_info = if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if let Some(ref root) = self.root_box {
                let (w, h) = (
                    self.width as f32,
                    (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                );
                let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                    root,
                    mango_core::Size::new(w, h),
                );
                tab.store_bfcache_snapshot(Box::new(snapshot));
            }

            if let Some((url, scroll_pos)) = tab.go_forward_f64() {
                let title = tab.title.clone();
                let snapshot = tab.current_bfcache_snapshot().cloned();
                Some((url, title, scroll_pos, snapshot))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((url, title, scroll_pos, snapshot)) = nav_info {
            self.address_text = url;
            self.page_title = title;
            self.cursor_pos = self.address_text.len();
            self.set_scroll_y_f64(scroll_pos);

            if let Some(snapshot) = snapshot {
                let dl = if self.scroll_y == 0.0 {
                    snapshot.display_list().clone()
                } else {
                    snapshot.display_list_with_scroll(self.scroll_y)
                };
                self.page_display_list = dl;
                self.root_box = Some(snapshot.root_box().clone());
                self.cached_scrollable_height =
                    crate::scroll::compute_scrollable_height(self.root_box.as_ref());
                self.page_dirty = true;
                self.status_text = format!("Restored from BFCache: {}", self.page_title);
                return;
            }

            self.navigate_to_address(false);
        }
    }
}
