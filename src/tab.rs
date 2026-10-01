//! Tab state management.
//!
//! Each tab owns its own URL, title, scroll position, and navigation history.

/// A single entry in the tab's navigation history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// URL of the visited page.
    pub url: String,
    /// Title of the page.
    pub title: String,
    /// Scroll position when navigated away.
    pub scroll_y: f32,
}

/// Per-tab web contents state (DOM tree, JS runtime, stylesheets, layout, scroll, and form state).
pub struct TabWebContents {
    pub current_html: String,
    pub cached_document: Option<mango_html::dom::Document>,
    pub base_url: Option<mango_net::Url>,
    pub cached_stylesheets: Vec<mango_css::Stylesheet>,
    pub root_box: Option<mango_layout::box_tree::LayoutBox>,
    pub js_runtime: Option<mango_js::JsRuntime>,
    pub style_snapshot: std::collections::HashMap<u32, mango_css::ComputedStyle>,
    pub focused_control: Option<mango_html::dom::NodeId>,
    pub page_display_list: mango_render::display_list::DisplayList,
    pub canvas_bg: mango_core::Color,
}

/// A single browser tab with navigation history and isolated web contents.
pub struct Tab {
    /// The current URL.
    pub url: String,
    /// The page title.
    pub title: String,
    /// Scroll position (vertical).
    pub scroll_y: f32,
    /// History stack.
    history: Vec<HistoryEntry>,
    /// Current position in history.
    history_idx: usize,
    /// Per-tab web contents state (DOM tree, JS runtime, layout, styles).
    pub contents: Option<TabWebContents>,
    /// Tab-scoped in-memory session storage (survives navigations in this tab, PRD 7.6).
    pub session_storage: mango_js::web_apis::SharedSessionStorage,
}

impl Tab {
    pub fn new() -> Self {
        Self::with_url_and_title("about:blank", "New Tab")
    }

    pub fn with_url_and_title(url: impl Into<String>, title: impl Into<String>) -> Self {
        let u = url.into();
        let t = title.into();
        let initial_entry = HistoryEntry {
            url: u.clone(),
            title: t.clone(),
            scroll_y: 0.0,
        };
        Self {
            url: u,
            title: t,
            scroll_y: 0.0,
            history: vec![initial_entry],
            history_idx: 0,
            contents: None,
            session_storage: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Pushes a new URL into navigation history, truncating any forward history.
    pub fn push_history(&mut self, url: impl Into<String>, title: impl Into<String>) {
        let u = url.into();
        let t = title.into();

        // Save current scroll position on current history entry if exists
        if let Some(entry) = self.history.get_mut(self.history_idx) {
            entry.scroll_y = self.scroll_y;
        }

        // Truncate any forward history beyond current index
        if !self.history.is_empty() {
            self.history.truncate(self.history_idx + 1);
        }

        self.url = u.clone();
        self.title = t.clone();
        self.scroll_y = 0.0;

        self.history.push(HistoryEntry {
            url: u,
            title: t,
            scroll_y: 0.0,
        });
        self.history_idx = self.history.len().saturating_sub(1);
    }

    /// Returns true if the user can navigate back.
    pub fn can_go_back(&self) -> bool {
        self.history_idx > 0
    }

    /// Returns true if the user can navigate forward.
    pub fn can_go_forward(&self) -> bool {
        self.history_idx + 1 < self.history.len()
    }

    /// Navigates back in history. Returns the target URL and scroll_y if available.
    pub fn go_back(&mut self) -> Option<(String, f32)> {
        if self.can_go_back() {
            // Save current scroll pos
            if let Some(entry) = self.history.get_mut(self.history_idx) {
                entry.scroll_y = self.scroll_y;
            }

            self.history_idx -= 1;
            let entry = &self.history[self.history_idx];
            self.url = entry.url.clone();
            self.title = entry.title.clone();
            self.scroll_y = entry.scroll_y;
            Some((self.url.clone(), self.scroll_y))
        } else {
            None
        }
    }

    /// Navigates forward in history. Returns the target URL and scroll_y if available.
    pub fn go_forward(&mut self) -> Option<(String, f32)> {
        if self.can_go_forward() {
            if let Some(entry) = self.history.get_mut(self.history_idx) {
                entry.scroll_y = self.scroll_y;
            }

            self.history_idx += 1;
            let entry = &self.history[self.history_idx];
            self.url = entry.url.clone();
            self.title = entry.title.clone();
            self.scroll_y = entry.scroll_y;
            Some((self.url.clone(), self.scroll_y))
        } else {
            None
        }
    }

    /// Updates current page title (e.g. after DOM parsing `<title>`).
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        if let Some(entry) = self.history.get_mut(self.history_idx) {
            entry.title = self.title.clone();
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::new()
    }
}
