//! Tab management, lifecycle, and WebContents state preservation.

use super::BrowserChrome;
use crate::tab::Tab;
use super::welcome::welcome_page_html;

impl BrowserChrome {
    #[allow(dead_code)]
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab_idx)
    }

    #[allow(dead_code)]
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab_idx)
    }

    #[allow(dead_code)]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[allow(dead_code)]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Saves the active tab's WebContents (DOM tree, JS runtime, scroll, form state) into `self.tabs`.
    pub fn save_active_tab_state(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.url = self.address_text.clone();
            tab.title = self.page_title.clone();
            tab.scroll_y = self.scroll_y;
            tab.scroll_y_f64 = self.scroll_y_f64;
            tab.contents = Some(crate::tab::TabWebContents {
                current_html: std::mem::take(&mut self.current_html),
                cached_document: self.cached_document.take(),
                base_url: self.base_url.take(),
                cached_stylesheets: std::mem::take(&mut self.cached_stylesheets),
                root_box: self.root_box.take(),
                js_runtime: self.js_runtime.take(),
                style_snapshot: std::mem::take(&mut self.style_snapshot),
                focused_control: self.focused_control.take(),
                page_display_list: std::mem::take(&mut self.page_display_list),
                display_list_cache: std::mem::replace(
                    &mut *self.display_list_cache.lock().unwrap(),
                    mango_layout::display_list::DisplayListCache::new(),
                ),
                image_rx: self.image_rx.take(),
                has_active_animations: self.anim_state.is_animating(),
                canvas_bg: self.canvas_bg,
            });
        }
    }

    /// Restores the active tab's WebContents from `self.tabs[self.active_tab_idx]`.
    pub fn restore_active_tab_state(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            self.address_text = tab.url.clone();
            self.cursor_pos = self.address_text.len();
            self.page_title = tab.title.clone();
            self.scroll_y = tab.scroll_y;
            self.scroll_y_f64 = tab.scroll_y_f64;
            if let Some(contents) = tab.contents.take() {
                self.current_html = contents.current_html;
                self.cached_document = contents.cached_document;
                self.base_url = contents.base_url;
                self.cached_stylesheets = contents.cached_stylesheets;
                self.root_box = contents.root_box;
                self.js_runtime = contents.js_runtime;
                self.style_snapshot = contents.style_snapshot;
                self.focused_control = contents.focused_control;
                self.page_display_list = contents.page_display_list;
                self.display_list_cache = std::sync::Mutex::new(contents.display_list_cache);
                self.image_rx = contents.image_rx;
                self.canvas_bg = contents.canvas_bg;
                self.cached_scrollable_height =
                    crate::scroll::compute_scrollable_height(self.root_box.as_ref());
                self.page_dirty = true;
                self.status_text = format!("Tab: {}", self.page_title);
                return;
            }
        }
        self.navigate_to_address(false);
    }

    /// Reloads the current page.
    pub fn reload(&mut self) {
        self.navigate_to_address(false);
    }

    /// Creates a new tab navigating to about:welcome.
    pub fn new_tab(&mut self) {
        self.save_active_tab_state();
        let new_tab = Tab::with_url_and_title("about:welcome", "New Tab");
        self.tabs.push(new_tab);
        self.active_tab_idx = self.tabs.len() - 1;
        self.address_text = "about:welcome".to_string();
        self.cursor_pos = self.address_text.len();
        self.scroll_y = 0.0;
        self.scroll_y_f64 = 0.0;
        self.load_html_internal(welcome_page_html(), "about:welcome".to_string(), false);
    }

    /// Closes the specified tab.
    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            // If only one tab remains, reset it to welcome page
            self.navigate("about:welcome");
            return;
        }

        let closing_active = self.active_tab_idx == idx;
        self.tabs.remove(idx);
        if self.active_tab_idx >= self.tabs.len() || closing_active {
            self.active_tab_idx = self.active_tab_idx.min(self.tabs.len() - 1);
            self.restore_active_tab_state();
        } else if self.active_tab_idx > idx {
            self.active_tab_idx -= 1;
        }
    }

    /// Closes the currently active tab.
    pub fn close_active_tab(&mut self) {
        self.close_tab(self.active_tab_idx);
    }

    /// Switches active tab to the given index.
    pub fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() && idx != self.active_tab_idx {
            self.save_active_tab_state();
            self.active_tab_idx = idx;
            self.restore_active_tab_state();
        }
    }

    /// Switches to the next tab (Ctrl+Tab).
    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            let next_idx = (self.active_tab_idx + 1) % self.tabs.len();
            self.switch_tab(next_idx);
        }
    }
}
