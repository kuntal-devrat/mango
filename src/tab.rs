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
    /// High-precision 64-bit scroll position.
    pub scroll_y_f64: f64,
    /// BFCache frozen engine layout snapshot for zero-latency instant back/forward navigation.
    pub bfcache_snapshot: Option<Box<mango_layout::LayoutSnapshot>>,
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
    pub display_list_cache: mango_layout::display_list::DisplayListCache,
    pub image_rx: Option<std::sync::mpsc::Receiver<String>>,
    pub resource_scheduler: crate::browser::resource_scheduler::DocumentResourceScheduler,
    pub has_active_animations: bool,
    pub canvas_bg: mango_core::Color,
    pub engine: Option<mango_engine::Engine>,
}

/// A single browser tab with navigation history and isolated web contents.
pub struct Tab {
    /// The current URL.
    pub url: String,
    /// The page title.
    pub title: String,
    /// Scroll position (vertical).
    pub scroll_y: f32,
    /// High-precision 64-bit vertical scroll position.
    pub scroll_y_f64: f64,
    /// History stack.
    history: Vec<HistoryEntry>,
    /// Current position in history.
    history_idx: usize,
    /// Per-tab web contents state (DOM tree, JS runtime, layout, styles).
    pub contents: Option<TabWebContents>,
    /// Whether this tab has an in-flight navigation. Loading state must be
    /// per-tab so navigating one tab does not show a spinner on another.
    pub loading: bool,
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
            scroll_y_f64: 0.0,
            bfcache_snapshot: None,
        };
        Self {
            url: u,
            title: t,
            scroll_y: 0.0,
            scroll_y_f64: 0.0,
            history: vec![initial_entry],
            history_idx: 0,
            contents: None,
            loading: false,
            session_storage: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Stores a frozen BFCache layout snapshot on the current history entry.
    pub fn store_bfcache_snapshot(&mut self, snapshot: Box<mango_layout::LayoutSnapshot>) {
        // A layout snapshot duplicates a large part of a page's retained
        // geometry. Keep a bounded back/forward pair; two entries are needed
        // for instant traversal while preventing unbounded retained growth.
        const MAX_BFCACHE_ENTRIES: usize = 2;
        let current_idx = self.history_idx;
        if let Some(entry) = self.history.get_mut(current_idx) {
            entry.bfcache_snapshot = Some(snapshot);
        }
        let mut snapshot_indices: Vec<usize> = self
            .history
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if e.bfcache_snapshot.is_some() {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        if snapshot_indices.len() > MAX_BFCACHE_ENTRIES {
            snapshot_indices.sort_by_key(|&idx| std::cmp::Reverse(idx.abs_diff(current_idx)));
            for &idx in &snapshot_indices[MAX_BFCACHE_ENTRIES..] {
                if let Some(entry) = self.history.get_mut(idx) {
                    entry.bfcache_snapshot = None;
                }
            }
        }
    }

    /// Returns the frozen BFCache snapshot of the current history entry if available.
    pub fn current_bfcache_snapshot(&self) -> Option<&mango_layout::LayoutSnapshot> {
        self.history
            .get(self.history_idx)
            .and_then(|e| e.bfcache_snapshot.as_deref())
    }

    /// Pushes a new URL into navigation history, truncating any forward history.
    pub fn push_history(&mut self, url: impl Into<String>, title: impl Into<String>) {
        let u = url.into();
        let t = title.into();

        // Save current scroll position on current history entry if exists
        if let Some(entry) = self.history.get_mut(self.history_idx) {
            entry.scroll_y = self.scroll_y;
            entry.scroll_y_f64 = self.scroll_y_f64;
        }

        // Truncate any forward history beyond current index
        if !self.history.is_empty() {
            self.history.truncate(self.history_idx + 1);
        }

        self.url = u.clone();
        self.title = t.clone();
        self.scroll_y = 0.0;
        self.scroll_y_f64 = 0.0;

        self.history.push(HistoryEntry {
            url: u,
            title: t,
            scroll_y: 0.0,
            scroll_y_f64: 0.0,
            bfcache_snapshot: None,
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
        self.go_back_f64().map(|(u, s)| (u, s as f32))
    }

    /// Navigates back in history with 64-bit precision scroll offset.
    pub fn go_back_f64(&mut self) -> Option<(String, f64)> {
        if self.can_go_back() {
            // Save current scroll pos
            if let Some(entry) = self.history.get_mut(self.history_idx) {
                entry.scroll_y = self.scroll_y;
                entry.scroll_y_f64 = self.scroll_y_f64;
            }

            self.history_idx -= 1;
            let entry = &self.history[self.history_idx];
            self.url = entry.url.clone();
            self.title = entry.title.clone();
            self.scroll_y = entry.scroll_y;
            self.scroll_y_f64 = entry.scroll_y_f64;
            Some((self.url.clone(), self.scroll_y_f64))
        } else {
            None
        }
    }

    /// Navigates forward in history. Returns the target URL and scroll_y if available.
    pub fn go_forward(&mut self) -> Option<(String, f32)> {
        self.go_forward_f64().map(|(u, s)| (u, s as f32))
    }

    /// Navigates forward in history with 64-bit precision scroll offset.
    pub fn go_forward_f64(&mut self) -> Option<(String, f64)> {
        if self.can_go_forward() {
            if let Some(entry) = self.history.get_mut(self.history_idx) {
                entry.scroll_y = self.scroll_y;
                entry.scroll_y_f64 = self.scroll_y_f64;
            }

            self.history_idx += 1;
            let entry = &self.history[self.history_idx];
            self.url = entry.url.clone();
            self.title = entry.title.clone();
            self.scroll_y = entry.scroll_y;
            self.scroll_y_f64 = entry.scroll_y_f64;
            Some((self.url.clone(), self.scroll_y_f64))
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

    /// Clears all history entries and associated bfcache snapshots.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_idx = 0;
    }

    /// Estimates the approximate memory footprint of this tab in bytes.
    pub fn approx_memory_usage(&mut self) -> usize {
        let mut bytes = std::mem::size_of::<Self>() + self.url.len() + self.title.len();
        bytes += self.history.len() * std::mem::size_of::<HistoryEntry>();
        for h in &self.history {
            bytes += h.url.len() + h.title.len();
        }
        if let Some(contents) = &mut self.contents {
            bytes += contents.current_html.len();
            bytes += contents.page_display_list.len() * 64;
            bytes += contents.style_snapshot.len() * 128;
            bytes += contents.cached_stylesheets.len() * 4096;
            if let Some(doc) = &contents.cached_document {
                bytes += doc.estimated_memory_usage();
            }
            if let Some(rt) = &mut contents.js_runtime {
                bytes += rt.heap_size_bytes();
            }
        }
        // Base overhead for tab runtime structures (at least 256KB)
        bytes.max(256 * 1024)
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::new()
    }
}
