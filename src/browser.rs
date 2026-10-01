//! Browser chrome: Chrome-like tab bar, toolbar, address bar, content area, and status bar.
//!
//! This module owns the browser's UI layout and coordinates with `mango_layout`
//! to render HTML documents dynamically in the content area, as well as integrating
//! the `mango_js` Boa-powered JavaScript engine for interactive DOM manipulation and timers.

use std::collections::{HashMap, HashSet};

use mango_core::{Color, Rect, Size};
use mango_css::computed::ComputedStyle;
use mango_css::{Stylesheet, parse_stylesheet};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;
use mango_js::JsRuntime;
use mango_layout::layout_document;
use mango_net::{FetchedDocument, ResourceLoader, Url};
use mango_platform::input::{KeyEvent, KeyState, MangoKey, MouseButton};
use mango_render::display_list::{BorderWidths, DisplayCommand, DisplayList};
use mango_render::{
    FontFamily, FontWeight, cache_image, decode_data_uri, decode_image_bytes, font_manager,
    get_cached_image,
};

use crate::config::Config;
use crate::tab::Tab;

/// A borrowed view of the active DOM.
///
/// `BrowserChrome::active_document()` returns an owned `Document`, which deep-clones
/// the whole tree (or re-parses the page source). That is acceptable for
/// one-shot commands such as form submission, but it must never happen on the
/// layout/paint path. `document_view()` hands out a borrow of whichever DOM is
/// live — the JS runtime's tree, the cached parse, or a fresh parse as a last
/// resort — so a relayout costs no DOM copying at all.
enum DocumentView<'a> {
    Runtime(std::cell::Ref<'a, Document>),
    Cached(&'a Document),
    Parsed(Box<Document>),
}

impl std::ops::Deref for DocumentView<'_> {
    type Target = Document;

    fn deref(&self) -> &Document {
        match self {
            DocumentView::Runtime(doc) => doc,
            DocumentView::Cached(doc) => doc,
            DocumentView::Parsed(doc) => doc,
        }
    }
}

/// Picks the canvas background for a laid-out page.
///
/// The root element's own background wins; otherwise the first child with a
/// non-transparent background propagates its colour to the canvas, matching the
/// CSS background propagation rules for the root element.
fn canvas_background_color(root: &mango_layout::box_tree::LayoutBox) -> Color {
    if let Some(style) = &root.style {
        if style.background_color != Color::TRANSPARENT {
            return style.background_color;
        }
        for child in &root.children {
            if let Some(child_style) = &child.style
                && child_style.background_color != Color::TRANSPARENT
            {
                return child_style.background_color;
            }
        }
    }
    CONTENT_BG
}

// Subsystem module re-exports (OPT-001)
pub use crate::chrome_ui::*;
pub use crate::context_menu::*;
pub use crate::form_handler::*;
pub use crate::navigation::*;
pub use crate::scroll::*;

struct PendingNavigation {
    target_url: Url,
    rx: std::sync::mpsc::Receiver<Result<FetchedDocument, mango_net::NetworkError>>,
    push_history: bool,
}

/// The browser chrome — manages tab bar, navigation toolbar, address bar, content area, and status bar.
pub struct BrowserChrome {
    /// Current window dimensions.
    width: u32,
    height: u32,
    /// Text in the address bar.
    address_text: String,
    /// Whether the address bar is focused (accepting input).
    address_focused: bool,
    /// Whether all text in the address bar is currently selected.
    is_all_selected: bool,
    /// Text cursor position in the address bar.
    cursor_pos: usize,
    /// Current mouse cursor X coordinate.
    mouse_x: f32,
    /// Current mouse cursor Y coordinate.
    mouse_y: f32,
    /// Currently hovered UI element.
    hovered_target: Option<HoverTarget>,
    /// Status bar text (e.g., "Ready", hovered link URL).
    status_text: String,
    /// Current page title.
    page_title: String,
    /// Open tabs.
    tabs: Vec<Tab>,
    /// Index of the active tab.
    active_tab_idx: usize,
    /// Raw HTML content currently loaded in the active tab.
    current_html: String,
    /// OPT-003: Cached parsed HTML Document of `current_html` to prevent redundant re-parsing.
    cached_document: Option<Document>,
    /// Base URL of current page, if loaded over the network.
    base_url: Option<Url>,
    /// External author stylesheets downloaded via <link rel="stylesheet">.
    cached_stylesheets: Vec<Stylesheet>,
    /// HTTP and resource loader.
    loader: ResourceLoader,
    /// Vertical scroll offset in the content area.
    scroll_y: f32,
    /// Browser configuration.
    #[allow(dead_code)]
    config: Config,
    /// Root layout box of the currently displayed document for hit-testing.
    root_box: Option<mango_layout::box_tree::LayoutBox>,
    /// Live JavaScript runtime for the active page.
    js_runtime: Option<JsRuntime>,
    /// Whether a network load is currently in progress (shows progress bar).
    is_loading: bool,
    /// Active right-click context menu, if open.
    context_menu: Option<ContextMenu>,
    /// Active custom select dropdown overlay, if open.
    select_dropdown: Option<SelectDropdown>,
    /// Active popup picker overlay (`color`/`date`/`time`/`file`), if open.
    picker: Option<PickerOverlay>,
    /// Node id of a `<input type="range">` slider currently being dragged.
    range_dragging: Option<mango_html::dom::NodeId>,
    /// Slider value captured when the current drag started (for the trailing `change` event).
    range_drag_start_value: Option<String>,
    /// Value of the focused text control when focus began (for the trailing `change` event).
    focused_initial_value: Option<String>,
    /// Focused form control node ID in the content area, if any.
    focused_control: Option<mango_html::dom::NodeId>,
    /// Text cursor position within the focused form control.
    control_cursor_pos: usize,
    /// Cached layout display list of the current page.
    page_display_list: DisplayList,
    /// OPT-005: Display list cache with dirty-tracking and scroll-invalidation.
    display_list_cache: std::cell::RefCell<mango_layout::display_list::DisplayListCache>,
    /// Background color of the content canvas.
    canvas_bg: Color,
    /// Whether the user is dragging the scrollbar thumb.
    scrollbar_dragging: bool,
    /// Mouse Y coordinate when scrollbar drag started.
    drag_start_mouse_y: f32,
    /// Scroll offset when scrollbar drag started.
    drag_start_scroll_y: f32,
    /// CSS transition + `@keyframes` animation runtime (GAP-004/GAP-006).
    anim_state: mango_css::AnimationState,
    /// Base (pre-animation) computed styles per DOM node, captured at layout time.
    style_snapshot: HashMap<u32, ComputedStyle>,
    /// Shared cookie jar. One handle is held here, by the HTTP loader, and by
    /// the JS runtime (`document.cookie`), so every writer sees every write.
    /// Flushed to the profile file after each network load (GAP-016).
    cookie_jar: std::sync::Arc<std::sync::Mutex<mango_net::CookieJar>>,
    /// Profile file the cookie jar is persisted to.
    cookie_store_path: std::path::PathBuf,
    /// Shared localStorage store — survives navigations and restarts (GAP-017).
    local_storage: mango_js::web_apis::SharedLocalStorage,
    /// Profile file localStorage is persisted to.
    local_storage_path: std::path::PathBuf,
    /// Image URLs queued for background prefetch (OPT-009: image-heavy pages
    /// no longer hit an arbitrary cap — anything not prefetched inline is
    /// fetched a few per frame instead of being silently dropped).
    pending_image_fetches: Vec<String>,
    /// Base URL that `pending_image_fetches` entries resolve against.
    image_fetch_base: Option<Url>,
    /// Built-in Developer Tools & Element Inspector (GAP-022).
    pub devtools: crate::devtools::DevTools,
    /// Browser extension & add-on manager (GAP-023).
    pub extension_manager: crate::extensions::ExtensionManager,
    /// DOM nodes currently marked as hovered (`:hover`).
    hovered_dom_nodes: HashSet<mango_html::dom::NodeId>,
    /// DOM nodes currently marked as active (`:active`).
    active_dom_nodes: HashSet<mango_html::dom::NodeId>,
    /// Active smooth scrolling animation controller.
    pub smooth_scroll: Option<crate::scroll::SmoothScrollAnimation>,
    /// Active asynchronous navigation task, if any.
    pending_navigation: Option<PendingNavigation>,
    /// Whether network navigations should proceed asynchronously (true in desktop UI, false in tests/headless).
    pub async_navigation: bool,
    /// Channel receiver for images decoded by background workers.
    image_rx: Option<std::sync::mpsc::Receiver<String>>,
}

/// Pre-populates Wikipedia Vector 2022 appearance controls (Text size, Width, Color)
/// if the server HTML contains an empty `#vector-appearance` container.
fn preprocess_wikipedia_appearance_html(html: &mut String, url: &str) {
    if url.contains("wikipedia.org") || html.contains("client-nojs") {
        *html = html.replacen("client-nojs", "client-js", 1);
    }
    if url.contains("wikipedia.org") || html.contains("vector-appearance") {
        let hide_style = "<style>.vector-pinned-container, #vector-appearance-pinned-container, .vector-column-end { display: none !important; }</style>";
        if let Some(pos) = html.find("</head>") {
            html.insert_str(pos, hide_style);
        } else {
            html.push_str(hide_style);
        }
    }
}

impl BrowserChrome {
    pub fn new(width: u32, height: u32) -> Self {
        let initial_html = welcome_page_html();
        let default_tab = Tab::with_url_and_title("about:welcome", "Welcome to Mango");
        let initial_title = default_tab.title.clone();

        let cookie_store_path = mango_net::CookieJar::default_profile_path();
        let cookie_jar = std::sync::Arc::new(std::sync::Mutex::new(mango_net::CookieJar::new()));
        if let Ok(mut jar) = cookie_jar.lock() {
            match jar.load_from_file(&cookie_store_path) {
                Ok(0) => {}
                Ok(n) => log::info!("Restored {n} cookies from {}", cookie_store_path.display()),
                Err(e) => log::warn!("Could not load cookie profile: {e}"),
            }
        }
        let local_storage_path = cookie_store_path
            .parent()
            .map(|p| p.join("localStorage.txt"))
            .unwrap_or_else(|| std::path::PathBuf::from("localStorage.txt"));
        let local_storage: mango_js::web_apis::SharedLocalStorage =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        load_local_storage(&local_storage_path, &local_storage);

        let mut chrome = Self {
            width,
            height,
            address_text: "about:welcome".to_string(),
            address_focused: false,
            is_all_selected: false,
            cursor_pos: 13,
            mouse_x: 0.0,
            mouse_y: 0.0,
            hovered_target: None,
            status_text: "Ready — Phase 7 Modern Web Engine Active".to_string(),
            page_title: initial_title,
            tabs: vec![default_tab],
            active_tab_idx: 0,
            current_html: initial_html.clone(),
            cached_document: None,
            base_url: None,
            cached_stylesheets: Vec::new(),
            scroll_y: 0.0,
            config: Config::new(),
            root_box: None,
            js_runtime: None,
            is_loading: false,
            context_menu: None,
            select_dropdown: None,
            picker: None,
            range_dragging: None,
            range_drag_start_value: None,
            focused_initial_value: None,
            focused_control: None,
            control_cursor_pos: 0,
            page_display_list: DisplayList::new(),
            display_list_cache: std::cell::RefCell::new(
                mango_layout::display_list::DisplayListCache::new(),
            ),
            canvas_bg: CONTENT_BG,
            scrollbar_dragging: false,
            drag_start_mouse_y: 0.0,
            drag_start_scroll_y: 0.0,
            anim_state: mango_css::AnimationState::new(),
            style_snapshot: HashMap::new(),
            loader: ResourceLoader::with_cookie_jar(cookie_jar.clone()),
            cookie_jar,
            cookie_store_path,
            local_storage,
            local_storage_path,
            pending_image_fetches: Vec::new(),
            image_fetch_base: None,
            devtools: crate::devtools::DevTools::new(),
            extension_manager: crate::extensions::ExtensionManager::new(),
            hovered_dom_nodes: HashSet::new(),
            active_dom_nodes: HashSet::new(),
            smooth_scroll: None,
            pending_navigation: None,
            async_navigation: false,
            image_rx: None,
        };

        chrome.load_html_internal(initial_html, "about:welcome".to_string(), false);
        chrome
    }

    #[allow(dead_code)]
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab_idx)
    }

    #[allow(dead_code)]
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab_idx)
    }

    #[allow(dead_code)]
    pub fn page_title(&self) -> &str {
        &self.page_title
    }

    #[allow(dead_code)]
    pub fn current_html(&self) -> &str {
        &self.current_html
    }

    /// Returns the currently active HTML document (from JS runtime snapshot or parsed cache) (OPT-003).
    ///
    /// This deep-clones the DOM. Prefer [`BrowserChrome::document_view`] on hot paths.
    #[allow(dead_code)]
    pub fn active_document(&self) -> Document {
        self.document_view().clone()
    }

    /// Borrows the currently active DOM without cloning it.
    fn document_view(&self) -> DocumentView<'_> {
        if let Some(ref rt) = self.js_runtime {
            DocumentView::Runtime(rt.document_ref())
        } else if let Some(ref cached) = self.cached_document {
            DocumentView::Cached(cached)
        } else {
            DocumentView::Parsed(Box::new(parse_html(&self.current_html)))
        }
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[allow(dead_code)]
    pub fn cached_stylesheets(&self) -> &[Stylesheet] {
        &self.cached_stylesheets
    }

    #[allow(dead_code)]
    pub fn loader(&self) -> &ResourceLoader {
        &self.loader
    }

    /// Recomputes document layout using the active DOM (either from JS runtime or parsed HTML).
    ///
    /// Borrows the live DOM instead of cloning it (see [`DocumentView`]): layout runs
    /// on every resize, JS mutation and animation frame, so a deep copy here would
    /// dominate the frame cost on large documents.
    pub fn relayout(&mut self) {
        let viewport = Size::new(self.width as f32, self.content_height());

        let (root_box, page_dl, canvas_color) = {
            let doc = self.document_view();
            let author_sheets: Vec<&Stylesheet> = self.cached_stylesheets.iter().collect();
            let (root_box, page_dl) = layout_document(&doc, &author_sheets, viewport);
            let canvas_color = canvas_background_color(&root_box);
            (root_box, page_dl, canvas_color)
        };

        self.canvas_bg = canvas_color;
        self.page_display_list = page_dl;
        self.root_box = Some(root_box);
        self.display_list_cache.borrow_mut().invalidate(None);

        self.refresh_animation_targets();
        self.publish_layout_bounds();
    }

    /// Publishes DOM node geometry to the JS runtime so measurement APIs
    /// (`getBoundingClientRect`, `offsetWidth`, `elementFromPoint`) return real values.
    fn publish_layout_bounds(&self) {
        let Some(rt) = &self.js_runtime else {
            return;
        };
        let Some(root) = &self.root_box else {
            return;
        };
        let mut rects = HashMap::new();
        mango_layout::collect_box_rects(root, &mut rects);
        rt.set_layout_bounds(rects);
    }

    /// Re-syncs the animation/transition engines against the freshly laid-out tree.
    ///
    /// Called after every (re)layout: starts `animation-*` declarations that are not
    /// already running, and retargets `transition-*` declarations whose properties
    /// changed since the previous layout pass.
    fn refresh_animation_targets(&mut self) {
        let Some(root) = self.root_box.as_ref() else {
            return;
        };

        let mut fresh: HashMap<u32, ComputedStyle> = HashMap::new();
        mango_layout::collect_box_styles(root, &mut fresh);
        let previous = std::mem::take(&mut self.style_snapshot);

        let mut starts: Vec<(mango_html::dom::NodeId, ComputedStyle)> = Vec::new();
        mango_layout::apply_box_style_overrides(
            self.root_box.as_mut().unwrap(),
            &mut |node, style| {
                if let Some(node) = node {
                    starts.push((node, style.clone()));
                }
            },
        );

        for (node, style) in starts {
            self.anim_state.sync_animations(node, &style);
            if mango_css::animation::has_transitions(&style)
                && let Some(old) = previous.get(&node.raw())
                && old != &style
            {
                self.anim_state.transitions.retarget(node, old, &style);
            }
        }

        self.style_snapshot = fresh;
    }

    /// Advances CSS transitions/animations by `dt_ms` and re-lays-out the page when
    /// anything is animating. Returns `true` when a redraw is required.
    pub fn tick_animations(&mut self, dt_ms: f32) -> bool {
        let mut needs_redraw = false;

        if let Some(mut anim) = self.smooth_scroll.take() {
            let (new_y, finished) = anim.step(std::time::Instant::now());
            if (self.scroll_y - new_y).abs() > 0.01 {
                self.scroll_y = new_y;
                if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                    tab.scroll_y = new_y;
                }
                self.display_list_cache.borrow_mut().clear();
                needs_redraw = true;
            }
            if !finished {
                self.smooth_scroll = Some(anim);
            }
        }

        if self.root_box.is_none() || !self.anim_state.is_animating() {
            return needs_redraw;
        }
        self.anim_state.advance(dt_ms);

        let snapshot = self.style_snapshot.clone();
        let anim_state = &self.anim_state;
        let mut root = match self.root_box.take() {
            Some(r) => r,
            None => return needs_redraw,
        };

        mango_layout::apply_box_style_overrides(&mut root, &mut |node, style| {
            let Some(node) = node else { return };
            let Some(base) = snapshot.get(&node.raw()) else {
                return;
            };
            let mut blended = anim_state.transitions.current_style(node, base);
            anim_state.animations.apply(node, &mut blended);
            *style = blended;
        });

        let viewport = Size::new(self.content_width(), self.content_height());
        self.page_display_list = mango_layout::relayout_box_tree(&mut root, viewport);
        self.root_box = Some(root);
        true
    }

    /// Width of the page content area in CSS pixels.
    fn content_width(&self) -> f32 {
        self.width as f32
    }

    /// Height of the page content area in CSS pixels.
    fn content_height(&self) -> f32 {
        ((self.height as f32) - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.relayout();
        let cw = self.content_width();
        let ch = self.content_height();
        if let Some(ref mut js_rt) = self.js_runtime {
            let _ = js_rt.dispatch_window_resize(cw, ch);
        }
    }

    /// Returns the active document's layout root box, if layout has completed.
    pub fn root_box(&self) -> Option<&mango_layout::box_tree::LayoutBox> {
        self.root_box.as_ref()
    }

    /// Returns the active vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// Sets the vertical scroll offset.
    pub fn set_scroll_y(&mut self, y: f32) {
        self.scroll_y = y.max(0.0);
    }

    /// Adjusts the vertical scroll offset by delta.
    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll_y = (self.scroll_y + dy).max(0.0);
    }

    /// Returns true while CSS transitions or animations still need frames.
    pub fn is_animating(&self) -> bool {
        self.anim_state.is_animating()
    }

    /// Exposes the animation runtime for tests and diagnostics.
    pub fn animation_state(&self) -> &mango_css::AnimationState {
        &self.anim_state
    }

    /// Navigates the browser to the specified URL or input string.
    pub fn navigate(&mut self, url: &str) {
        self.address_text = url.to_string();
        self.cursor_pos = self.address_text.len();
        self.navigate_to_address(true);
    }

    /// Navigates back in history if possible.
    pub fn go_back(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx)
            && let Some((url, scroll_pos)) = tab.go_back()
        {
            self.address_text = url.clone();
            self.cursor_pos = self.address_text.len();
            self.scroll_y = scroll_pos;
            self.navigate_to_address(false);
        }
    }

    /// Navigates forward in history if possible.
    pub fn go_forward(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx)
            && let Some((url, scroll_pos)) = tab.go_forward()
        {
            self.address_text = url.clone();
            self.cursor_pos = self.address_text.len();
            self.scroll_y = scroll_pos;
            self.navigate_to_address(false);
        }
    }

    /// Saves the active tab's WebContents (DOM tree, JS runtime, scroll, form state) into `self.tabs`.
    pub fn save_active_tab_state(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.url = self.address_text.clone();
            tab.title = self.page_title.clone();
            tab.scroll_y = self.scroll_y;
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
                self.canvas_bg = contents.canvas_bg;
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

    /// Public method to load HTML into the active tab.
    #[allow(dead_code)]
    pub fn load_html(&mut self, html: String, url: String) {
        self.load_html_internal(html, url, true);
    }

    /// Internal method to load HTML, execute scripts, initialize JS runtime, and re-layout.
    fn load_html_internal(&mut self, html: String, url: String, push_history: bool) {
        self.load_html_with_metadata(html, url, push_history, None, None);
    }

    /// Internal method to load HTML with known encoding and content-type metadata.
    fn load_html_with_metadata(
        &mut self,
        mut html: String,
        url: String,
        push_history: bool,
        encoding: Option<&str>,
        content_type: Option<&str>,
    ) {
        preprocess_wikipedia_appearance_html(&mut html, &url);
        let mut doc = parse_html(&html);
        if let Some(enc) = encoding {
            doc.character_set = enc.to_string();
        }
        if let Some(ct) = content_type {
            doc.content_type = ct.to_string();
        }
        self.load_document_internal(doc, html, url, push_history, false);
    }

    /// Loads an already parsed Document into the browser without re-parsing HTML.
    fn load_document_internal(
        &mut self,
        doc: Document,
        html: String,
        url: String,
        push_history: bool,
        skip_embedded_sheets_and_fonts: bool,
    ) {
        self.current_html = html;
        self.address_text = url.clone();
        self.cursor_pos = self.address_text.len();
        self.context_menu = None;
        self.select_dropdown = None;
        self.picker = None;
        self.range_dragging = None;
        self.range_drag_start_value = None;
        self.focused_control = None;
        self.focused_initial_value = None;
        self.control_cursor_pos = 0;

        self.cached_document = Some(doc.clone());
        if let Some(title) = extract_title(&doc) {
            self.page_title = title;
        } else {
            self.page_title = url.clone();
        }

        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if push_history {
                tab.push_history(url.clone(), self.page_title.clone());
            } else {
                tab.url = url.clone();
                tab.set_title(self.page_title.clone());
            }
        }

        // ── Phase 5: Initialize Boa JavaScript Runtime ──
        let w = self.width as f32;
        let content_h = (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let session_storage = self
            .tabs
            .get(self.active_tab_idx)
            .map(|t| t.session_storage.clone())
            .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())));
        let mut js_rt = JsRuntime::new_with_url_storage_session_and_cookies(
            doc,
            w,
            content_h,
            &url,
            self.local_storage.clone(),
            session_storage,
            self.cookie_jar.clone(),
        );

        // Load web fonts and imported stylesheets from embedded and cached stylesheets
        let embedded_sheets =
            mango_layout::extract_style_elements(self.cached_document.as_ref().unwrap());
        self.base_url = Url::parse(&url).ok();
        if !skip_embedded_sheets_and_fonts {
            if self.cached_stylesheets.is_empty()
                && let Some(ref base) = self.base_url.clone()
            {
                let mut visited = std::collections::HashSet::new();
                for sheet in &embedded_sheets {
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import_path) = rule {
                            load_stylesheet_recursive(
                                &self.loader,
                                base,
                                import_path,
                                &mut visited,
                                &mut self.cached_stylesheets,
                            );
                        }
                    }
                }
            }
            load_web_fonts(
                &self.loader,
                self.base_url.as_ref(),
                &self.cached_stylesheets,
            );
            load_web_fonts(&self.loader, self.base_url.as_ref(), &embedded_sheets);
        }

        // Inject extension stylesheets (GAP-023)
        let (ext_css, ext_js) = self
            .extension_manager
            .get_content_scripts_for_url(&url, crate::extensions::RunAt::DocumentEnd);
        for css in ext_css {
            let sheet = mango_css::parser::parse_stylesheet(&css);
            self.cached_stylesheets.push(sheet);
        }

        // Reset the animation runtime and load the page's @keyframes definitions
        // (GAP-004/GAP-006). Author sheets win over embedded ones, matching the
        // cascade order used for style resolution.
        self.anim_state = mango_css::AnimationState::new();
        self.style_snapshot.clear();
        self.hovered_dom_nodes.clear();
        self.active_dom_nodes.clear();
        self.anim_state
            .animations
            .set_keyframes(mango_css::collect_keyframes(
                embedded_sheets.iter().chain(self.cached_stylesheets.iter()),
            ));

        // Pre-layout so the page renders immediately even if scripts take time
        self.js_runtime = None;
        self.relayout();

        // Eagerly prefetch any CSS background images that matched active elements on the page (e.g. logos, icons)
        if let Some(ref root) = self.root_box {
            let mut bg_images = Vec::new();
            collect_box_background_images(root, &mut bg_images);
            if let Some(ref base) = self.base_url {
                let mut any_fetched = false;
                for bg in bg_images {
                    if mango_render::get_cached_image(&bg).is_none() {
                        self.prefetch_image(base, &bg);
                        any_fetched = true;
                    }
                }
                if any_fetched {
                    self.relayout();
                }
            }
        }

        // Extract and execute scripts in document order
        let mut scripts = Vec::new();
        let doc_ref = self.active_document();
        collect_scripts(&doc_ref, doc_ref.root(), &mut scripts);

        let mut external_count = 0;
        for script in scripts {
            match script {
                ScriptToRun::Inline(code) => {
                    let _ = js_rt.execute_script(&code);
                }
                ScriptToRun::External(src) => {
                    if external_count < 8
                        && let Some(ref base) = self.base_url
                        && let Ok(script_code) = self.loader.fetch_script(base, &src)
                    {
                        external_count += 1;
                        let _ = js_rt.execute_script(&script_code);
                    }
                }
            }
        }

        // 3. Run extension content scripts (GAP-023)
        for js_code in ext_js {
            let _ = js_rt.execute_script(&js_code);
        }

        if let Some(alert_msg) = js_rt.take_status_text() {
            self.status_text = alert_msg;
        }

        let redirect_url = js_rt.take_pending_navigation();

        self.js_runtime = Some(js_rt);
        self.relayout();

        // ── Phase 8: Autofocus on interactive form control if specified ──
        if !self.address_focused {
            let doc_snap = self.active_document();
            fn find_autofocus(doc: &Document, nid: NodeId) -> Option<NodeId> {
                if let Some(node) = doc.get(nid) {
                    if let NodeData::Element(elem) = &node.data {
                        let tag = elem.tag_name.to_ascii_lowercase();
                        if (tag == "input"
                            || tag == "textarea"
                            || tag == "button"
                            || tag == "select")
                            && elem.get_attribute("autofocus").is_some()
                        {
                            return Some(nid);
                        }
                    }
                    for child in doc.children(nid) {
                        if let Some(found) = find_autofocus(doc, child.id) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            if let Some(autofocus_nid) = find_autofocus(&doc_snap, doc_snap.root()) {
                self.focused_control = Some(autofocus_nid);
                let current_val = if let Some(ref rt) = self.js_runtime {
                    rt.get_node_attribute(autofocus_nid, "value")
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                self.control_cursor_pos = current_val.chars().count();
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_node_attribute(autofocus_nid, "data-mango-focused", "true");
                    rt.set_node_attribute(
                        autofocus_nid,
                        "_mango_cursor_pos",
                        &self.control_cursor_pos.to_string(),
                    );
                }
                self.relayout();
            }
        }

        // If a script requested navigation (e.g. window.location.href = ...), follow it
        if let Some(redirect) = redirect_url
            && redirect != url
        {
            self.navigate(&redirect);
        }
    }

    /// Ticks the JavaScript event loop (executing setTimeout/setInterval).
    /// Returns `true` if the DOM was mutated or timers fired, necessitating a redraw.
    pub fn tick_js(&mut self) -> bool {
        if let Some(ref mut rt) = self.js_runtime {
            let mutated = rt.tick();
            if let Some(alert_msg) = rt.take_status_text() {
                self.status_text = alert_msg;
            }
            for msg in rt.take_console_messages() {
                let lvl = match msg.level {
                    mango_js::console::ConsoleLevel::Log => crate::devtools::ConsoleLevel::Log,
                    mango_js::console::ConsoleLevel::Info => crate::devtools::ConsoleLevel::Info,
                    mango_js::console::ConsoleLevel::Warn => crate::devtools::ConsoleLevel::Warn,
                    mango_js::console::ConsoleLevel::Error => crate::devtools::ConsoleLevel::Error,
                };
                self.devtools.log(lvl, msg.text);
            }
            if let Some(redirect) = rt.take_pending_navigation() {
                self.navigate(&redirect);
                return true;
            }
            if mutated {
                self.relayout();
                return true;
            }
        }
        false
    }

    /// Returns true if the active page has pending animations, smooth scrolling, or JavaScript timers.
    pub fn has_pending_timers(&self) -> bool {
        self.smooth_scroll.is_some()
            || self.anim_state.is_animating()
            || self
                .js_runtime
                .as_ref()
                .is_some_and(|rt| rt.has_pending_timers())
    }

    /// Triggers smooth scrolling to `target_y` over the specified duration.
    pub fn scroll_smooth_to(&mut self, target_y: f32, duration: Option<std::time::Duration>) {
        let max_s = self.max_scroll();
        let dur = duration.unwrap_or(std::time::Duration::from_millis(300));
        if let Some(existing) = &mut self.smooth_scroll {
            existing.update_target(target_y, dur);
        } else {
            self.smooth_scroll = Some(crate::scroll::SmoothScrollAnimation::new(
                self.scroll_y,
                target_y,
                max_s,
                dur,
            ));
        }
    }
}

impl BrowserChrome {
    /// Returns the true scrollable document height scanning all layout boxes.
    pub fn scrollable_height(&self) -> f32 {
        crate::scroll::compute_scrollable_height(self.root_box.as_ref())
    }

    /// Computes maximum allowed scroll offset based on rendered content height.
    pub fn max_scroll(&self) -> f32 {
        let content_h = (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let doc_h = self.scrollable_height();
        (doc_h - content_h).max(0.0)
    }

    /// Handles mouse wheel scrolling with clamping.
    pub fn handle_scroll(&mut self, delta_y: f32) {
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
            let delta = if delta_y > 0.0 {
                2
            } else if delta_y < 0.0 {
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
                self.display_list_cache.borrow_mut().clear();
            }
            rem
        } else {
            (scroll_step_x, scroll_step_y)
        };

        if unconsumed_y.abs() > 0.001 {
            let max = self.max_scroll();
            self.scroll_y = (self.scroll_y + unconsumed_y).clamp(0.0, max);
            if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.scroll_y = self.scroll_y;
            }
        }
    }

    /// Builds the complete display list for the browser window.
    pub fn build_display_list(&self, (width, height): (u32, u32)) -> DisplayList {
        let mut dl = DisplayList::new();
        let w = width as f32;
        let h = height as f32;

        let content_y = HEADER_HEIGHT + 1.0;
        let content_h = (h - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);

        // Content area background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, content_y, w, content_h),
            color: self.canvas_bg,
        });

        // Translate & clip page commands (OPT-005 display list cache)
        let page_dl = if let Some(root) = &self.root_box {
            self.display_list_cache
                .borrow_mut()
                .get_or_build(root, self.scroll_y)
        } else {
            self.page_display_list.clone()
        };

        // The page may only paint inside the content viewport. A single clip around
        // the translated page commands keeps page overflow (wide tables, transformed
        // layers, absolutely positioned art) out of the browser chrome and lets
        // partially scrolled lines and images draw clipped, as Chromium does, instead
        // of popping in and out one baseline at a time.
        dl.push(DisplayCommand::PushClip {
            rect: Rect::new(0.0, content_y, w, content_h),
        });

        for cmd in page_dl.into_iter() {
            match cmd {
                DisplayCommand::FillRect { rect, color } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    if clamped_bottom > clamped_top {
                        dl.push(DisplayCommand::FillRect {
                            rect: Rect::new(
                                rect.x(),
                                clamped_top,
                                rect.width(),
                                clamped_bottom - clamped_top,
                            ),
                            color,
                        });
                    }
                }
                DisplayCommand::FillRoundedRect { rect, color, radii } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    if clamped_bottom > clamped_top {
                        if translated_y >= content_y
                            && translated_y + rect.height() <= content_y + content_h
                        {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect: Rect::new(
                                    rect.x(),
                                    translated_y,
                                    rect.width(),
                                    rect.height(),
                                ),
                                color,
                                radii,
                            });
                        } else {
                            dl.push(DisplayCommand::FillRect {
                                rect: Rect::new(
                                    rect.x(),
                                    clamped_top,
                                    rect.width(),
                                    clamped_bottom - clamped_top,
                                ),
                                color,
                            });
                        }
                    }
                }
                DisplayCommand::DrawBorder {
                    rect,
                    color,
                    widths,
                    radii,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    if clamped_bottom > clamped_top {
                        dl.push(DisplayCommand::DrawBorder {
                            rect: Rect::new(
                                rect.x(),
                                clamped_top,
                                rect.width(),
                                clamped_bottom - clamped_top,
                            ),
                            color,
                            widths,
                            radii,
                        });
                    }
                }
                DisplayCommand::DrawBoxShadow {
                    rect,
                    color,
                    offset_x,
                    offset_y,
                    blur_radius,
                    spread_radius,
                    radii,
                    inset,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let translated_rect =
                        Rect::new(rect.x(), translated_y, rect.width(), rect.height());
                    let max_blur = blur_radius + spread_radius;
                    if translated_rect.bottom() + max_blur >= content_y
                        && translated_rect.y() - max_blur <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawBoxShadow {
                            rect: translated_rect,
                            color,
                            offset_x,
                            offset_y,
                            blur_radius,
                            spread_radius,
                            radii,
                            inset,
                        });
                    }
                }
                DisplayCommand::DrawText {
                    text,
                    x,
                    y,
                    color,
                    font_size,
                    weight,
                    family,
                    style,
                    decoration,
                    letter_spacing,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    // Cull only runs whose glyph box (ascent above the baseline, descent
                    // below it) cannot touch the content band; the rest is clipped.
                    if translated_y + font_size * 0.3 >= content_y
                        && translated_y - font_size * 1.2 <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawText {
                            text,
                            x,
                            y: translated_y,
                            color,
                            font_size,
                            weight,
                            family,
                            style,
                            decoration,
                            letter_spacing,
                        });
                    }
                }
                DisplayCommand::DrawLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    thickness,
                } => {
                    let ty1 = y1 + content_y - self.scroll_y;
                    let ty2 = y2 + content_y - self.scroll_y;
                    let (lo, hi) = (ty1.min(ty2) - thickness, ty1.max(ty2) + thickness);
                    if hi >= content_y && lo <= content_y + content_h {
                        dl.push(DisplayCommand::DrawLine {
                            x1,
                            y1: ty1,
                            x2,
                            y2: ty2,
                            color,
                            thickness,
                        });
                    }
                }
                DisplayCommand::DrawImage {
                    x,
                    y,
                    width,
                    height,
                    pixels,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    // The content clip crops the visible rows, so the image is pushed
                    // once (no per-row re-slicing) and the painter clips it.
                    if translated_y + height >= content_y && translated_y <= content_y + content_h {
                        dl.push(DisplayCommand::DrawImage {
                            x,
                            y: translated_y,
                            width,
                            height,
                            pixels,
                        });
                    }
                }
                DisplayCommand::FillGradient {
                    rect,
                    gradient,
                    radii,
                    opacity,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    if clamped_bottom > clamped_top {
                        dl.push(DisplayCommand::FillGradient {
                            rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                            gradient,
                            radii,
                            opacity,
                        });
                    }
                }
                DisplayCommand::DrawTextShadow {
                    text,
                    x,
                    y,
                    color,
                    font_size,
                    weight,
                    family,
                    style,
                    blur_radius,
                    letter_spacing,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    if translated_y + font_size * 0.3 + blur_radius >= content_y
                        && translated_y - font_size * 1.2 - blur_radius <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawTextShadow {
                            text,
                            x,
                            y: translated_y,
                            color,
                            font_size,
                            weight,
                            family,
                            style,
                            blur_radius,
                            letter_spacing,
                        });
                    }
                }
                DisplayCommand::PushTransform { matrix } => {
                    let sy = content_y - self.scroll_y;
                    let m_screen = [
                        matrix[0],
                        matrix[1],
                        matrix[2],
                        matrix[3],
                        matrix[4],
                        matrix[5] + (1.0 - matrix[3]) * sy,
                    ];
                    dl.push(DisplayCommand::PushTransform { matrix: m_screen });
                }
                DisplayCommand::PopTransform => {
                    dl.push(DisplayCommand::PopTransform);
                }
                DisplayCommand::PushClip { rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    let clamped_h = (clamped_bottom - clamped_top).max(0.0);
                    dl.push(DisplayCommand::PushClip {
                        rect: Rect::new(rect.x(), clamped_top, rect.width(), clamped_h),
                    });
                }
                DisplayCommand::PopClip => {
                    dl.push(DisplayCommand::PopClip);
                }
                DisplayCommand::PushFilter { filters, rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    dl.push(DisplayCommand::PushFilter {
                        filters,
                        rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                    });
                }
                DisplayCommand::PopFilter => {
                    dl.push(DisplayCommand::PopFilter);
                }
                DisplayCommand::PushBackdropFilter { filters, rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    dl.push(DisplayCommand::PushBackdropFilter {
                        filters,
                        rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                    });
                }
                DisplayCommand::PopBackdropFilter => {
                    dl.push(DisplayCommand::PopBackdropFilter);
                }
                DisplayCommand::PushBlendMode { mode } => {
                    dl.push(DisplayCommand::PushBlendMode { mode });
                }
                DisplayCommand::PopBlendMode => {
                    dl.push(DisplayCommand::PopBlendMode);
                }
                DisplayCommand::PushClipPath { clip_path, rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    dl.push(DisplayCommand::PushClipPath {
                        clip_path,
                        rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                    });
                }
                DisplayCommand::PopClipPath => {
                    dl.push(DisplayCommand::PopClipPath);
                }
                DisplayCommand::DrawBorderImage {
                    rect,
                    pixels,
                    img_width,
                    img_height,
                    slice,
                    widths,
                    repeat_h,
                    repeat_v,
                    fill,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    if translated_y + rect.height() >= content_y
                        && translated_y <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawBorderImage {
                            rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                            pixels,
                            img_width,
                            img_height,
                            slice,
                            widths,
                            repeat_h,
                            repeat_v,
                            fill,
                        });
                    }
                }
                DisplayCommand::DrawTextWithGradient {
                    text,
                    x,
                    y,
                    gradient,
                    font_size,
                    weight,
                    family,
                    style,
                    decoration,
                    letter_spacing,
                    gradient_rect,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    let translated_rect_y = gradient_rect.y() + content_y - self.scroll_y;
                    if translated_y + font_size * 0.3 >= content_y
                        && translated_y - font_size * 1.2 <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawTextWithGradient {
                            text,
                            x,
                            y: translated_y,
                            gradient,
                            font_size,
                            weight,
                            family,
                            style,
                            decoration,
                            letter_spacing,
                            gradient_rect: Rect::new(
                                gradient_rect.x(),
                                translated_rect_y,
                                gradient_rect.width(),
                                gradient_rect.height(),
                            ),
                        });
                    }
                }
            }
        }

        dl.push(DisplayCommand::PopClip);

        // ── SCROLLBAR (y = content_y..content_y + content_h) ──
        let max_s = self.max_scroll();
        if max_s > 0.0 {
            let doc_h = self.scrollable_height();
            let (scrollbar_w, custom_track, custom_thumb) = if let Some(root) = &self.root_box {
                if let Some(s) = &root.style {
                    let w = match s.scrollbar_width {
                        mango_css::values::ScrollbarWidth::None => 0.0,
                        mango_css::values::ScrollbarWidth::Thin => 8.0,
                        mango_css::values::ScrollbarWidth::Auto => 12.0,
                    };
                    let (track, thumb) = if let Some((c_thumb, c_track)) = s.scrollbar_color {
                        (Some(c_track), Some(c_thumb))
                    } else {
                        (None, None)
                    };
                    (w, track, thumb)
                } else {
                    (12.0, None, None)
                }
            } else {
                (12.0, None, None)
            };

            if scrollbar_w > 0.0 {
                let scrollbar_x = w - scrollbar_w;
                let track_rect = Rect::new(scrollbar_x, content_y, scrollbar_w, content_h);

                // Track background (subtle semi-transparent or custom)
                dl.push(DisplayCommand::FillRect {
                    rect: track_rect,
                    color: custom_track.unwrap_or_else(|| Color::rgba(240, 240, 240, 140)),
                });

                // Thumb dimensions and position
                let thumb_h =
                    ((content_h / doc_h) * content_h).clamp(32.0f32.min(content_h), content_h);
                let max_thumb_travel = (content_h - thumb_h).max(1.0);
                let thumb_y = content_y + (self.scroll_y / max_s) * max_thumb_travel;

                let thumb_padding = 2.0f32;
                let thumb_rect = Rect::new(
                    scrollbar_x + thumb_padding,
                    thumb_y,
                    (scrollbar_w - thumb_padding * 2.0).max(1.0),
                    thumb_h,
                );

                let thumb_color = if let Some(ct) = custom_thumb {
                    ct
                } else if self.scrollbar_dragging {
                    Color::rgb(100, 100, 100) // Active dark dragging
                } else if self.hovered_target == Some(HoverTarget::ScrollbarThumb) {
                    Color::rgb(140, 140, 140) // Medium hover
                } else {
                    Color::rgb(190, 190, 190) // Normal sleek grey
                };

                dl.push(DisplayCommand::FillRoundedRect {
                    rect: thumb_rect,
                    color: thumb_color,
                    radii: [4.0, 4.0, 4.0, 4.0],
                });
            }
        }

        // ══════════════════════════════════════════════════════════════════════
        // ── CHROME REDESIGN: PART 1 — TAB STRIP (y = 0..TAB_BAR_HEIGHT) ──
        // ══════════════════════════════════════════════════════════════════════

        // Tab strip background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, w, TAB_BAR_HEIGHT),
            color: TAB_BAR_BG,
        });

        let tab_count = self.tabs.len().max(1);
        let max_strip_w = (w - 42.0).max(100.0);
        let tab_w = (max_strip_w / tab_count as f32).clamp(80.0, 210.0);

        for (i, tab) in self.tabs.iter().enumerate() {
            let tab_x = i as f32 * tab_w;
            let is_active = i == self.active_tab_idx;
            let is_hovered = self.hovered_target == Some(HoverTarget::Tab(i));

            let tab_bg = if is_active {
                TAB_ACTIVE_BG
            } else if is_hovered {
                TAB_HOVER_BG
            } else {
                TAB_INACTIVE_BG
            };

            // Tab body (rounded top feel: offset y = 5.0 to TAB_BAR_HEIGHT)
            let tab_h = TAB_BAR_HEIGHT - 5.0;
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(tab_x + 2.0, 5.0, tab_w - 4.0, tab_h),
                color: tab_bg,
            });

            // If active, connect tab seamlessly to toolbar below
            if is_active {
                dl.push(DisplayCommand::FillRect {
                    rect: Rect::new(tab_x + 2.0, TAB_BAR_HEIGHT - 1.0, tab_w - 4.0, 2.0),
                    color: TAB_ACTIVE_BG,
                });
            } else {
                // Subtle separator between inactive tabs
                dl.push(DisplayCommand::DrawLine {
                    x1: tab_x + tab_w - 1.0,
                    y1: 12.0,
                    x2: tab_x + tab_w - 1.0,
                    y2: TAB_BAR_HEIGHT - 6.0,
                    color: Color::rgb(60, 64, 67),
                    thickness: 1.0,
                });
            }

            // Tab favicon dot / badge
            let dot_color = if is_active {
                Color::MANGO_ORANGE
            } else {
                Color::rgb(120, 124, 130)
            };
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(tab_x + 8.0, 14.0, 6.0, 6.0),
                color: dot_color,
            });

            // Tab title
            let title_text = if tab.title.len() > 18 {
                format!("{}...", &tab.title[..15])
            } else {
                tab.title.clone()
            };

            dl.push(DisplayCommand::draw_text(
                title_text,
                tab_x + 18.0,
                11.0,
                if is_active {
                    TAB_TEXT_ACTIVE
                } else {
                    TAB_TEXT_INACTIVE
                },
                12.0,
                if is_active {
                    FontWeight::Bold
                } else {
                    FontWeight::Regular
                },
                FontFamily::SansSerif,
            ));

            // Tab close button (×)
            let close_x = tab_x + tab_w - 20.0;
            let close_hovered = self.hovered_target == Some(HoverTarget::TabClose(i));
            if close_hovered {
                dl.push(DisplayCommand::FillRect {
                    rect: Rect::new(close_x - 3.0, 9.0, 16.0, 16.0),
                    color: TAB_CLOSE_HOVER,
                });
            }

            dl.push(DisplayCommand::draw_text(
                "x".to_string(),
                close_x,
                11.0,
                if close_hovered {
                    Color::WHITE
                } else {
                    TAB_TEXT_INACTIVE
                },
                11.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));
        }

        // New tab button (+)
        let new_tab_x = (tab_count as f32 * tab_w + 4.0).min(w - 30.0);
        let new_tab_hovered = self.hovered_target == Some(HoverTarget::NewTab);
        if new_tab_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(new_tab_x - 2.0, 7.0, 22.0, 22.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "+".to_string(),
            new_tab_x + 3.0,
            9.0,
            if new_tab_hovered {
                Color::WHITE
            } else {
                TAB_TEXT_INACTIVE
            },
            16.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        // ══════════════════════════════════════════════════════════════════════
        // ── CHROME REDESIGN: PART 2 — TOOLBAR & OMNIBOX (y = TAB_BAR_HEIGHT..HEADER_HEIGHT) ──
        // ══════════════════════════════════════════════════════════════════════

        // Toolbar background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, TAB_BAR_HEIGHT, w, TOOLBAR_HEIGHT),
            color: TOOLBAR_BG,
        });

        let btn_y = TAB_BAR_HEIGHT + 6.0;
        let can_back = self
            .tabs
            .get(self.active_tab_idx)
            .is_some_and(|t| t.can_go_back());
        let can_forward = self
            .tabs
            .get(self.active_tab_idx)
            .is_some_and(|t| t.can_go_forward());

        // Back button (◀ / <)
        let back_hovered = self.hovered_target == Some(HoverTarget::Back) && can_back;
        if back_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(6.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "<".to_string(),
            14.0,
            btn_y + 3.0,
            if can_back {
                TOOLBAR_TEXT
            } else {
                TOOLBAR_TEXT_DISABLED
            },
            15.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Forward button (▶ / >)
        let forward_hovered = self.hovered_target == Some(HoverTarget::Forward) && can_forward;
        if forward_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(36.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            ">".to_string(),
            44.0,
            btn_y + 3.0,
            if can_forward {
                TOOLBAR_TEXT
            } else {
                TOOLBAR_TEXT_DISABLED
            },
            15.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Reload button (↻ / R)
        let reload_hovered = self.hovered_target == Some(HoverTarget::Reload);
        if reload_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(66.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "R".to_string(),
            74.0,
            btn_y + 3.0,
            if reload_hovered {
                Color::WHITE
            } else {
                TOOLBAR_TEXT
            },
            14.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Home button (🏠 / H)
        let home_hovered = self.hovered_target == Some(HoverTarget::Home);
        if home_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(96.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "H".to_string(),
            104.0,
            btn_y + 3.0,
            if home_hovered {
                Color::MANGO_ORANGE
            } else {
                TOOLBAR_TEXT
            },
            14.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // ── Omnibox (Address Bar) ──
        let addr_x = 132.0;
        let addr_y = TAB_BAR_HEIGHT + 5.0;
        let addr_w = (w - 176.0).max(120.0);
        let addr_h = 30.0;

        // Omnibox background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(addr_x, addr_y, addr_w, addr_h),
            color: ADDRESSBAR_BG,
        });

        // Omnibox border when focused
        if self.address_focused {
            dl.push(DisplayCommand::DrawBorder {
                rect: Rect::new(addr_x, addr_y, addr_w, addr_h),
                color: ADDRESSBAR_BORDER,
                widths: BorderWidths {
                    top: 1.5,
                    right: 1.5,
                    bottom: 1.5,
                    left: 1.5,
                },
                radii: [4.0; 4],
            });
        }

        // Security / Mango badge on left of Omnibox
        let is_https = self.address_text.starts_with("https://");
        let badge_color = if is_https {
            Color::rgb(52, 168, 83)
        } else {
            Color::MANGO_ORANGE
        };
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(addr_x + 8.0, addr_y + 11.0, 8.0, 8.0),
            color: badge_color,
        });

        let display_text = if self.address_text.is_empty() {
            "Search or enter web address..."
        } else {
            &self.address_text
        };

        // Text selection highlight
        let text_render_x = addr_x + 22.0;
        if self.address_focused && self.is_all_selected && !self.address_text.is_empty() {
            let (sel_w, _) = mango_render::font_manager().measure_text(
                &self.address_text,
                13.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
            );
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(text_render_x - 2.0, addr_y + 4.0, sel_w + 4.0, addr_h - 8.0),
                color: Color::rgb(0, 120, 215),
            });
        }

        let text_color = if self.address_text.is_empty() {
            Color::rgb(130, 134, 140)
        } else if self.address_focused && self.is_all_selected {
            Color::WHITE
        } else {
            ADDRESSBAR_TEXT
        };

        dl.push(DisplayCommand::draw_text(
            display_text.to_string(),
            text_render_x,
            addr_y + 6.0,
            text_color,
            13.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        // Cursor
        if self.address_focused && !self.is_all_selected {
            let safe_pos = self.cursor_pos.min(self.address_text.len());
            let mut boundary = safe_pos;
            while boundary > 0 && !self.address_text.is_char_boundary(boundary) {
                boundary -= 1;
            }
            let cursor_offset = mango_render::font_manager()
                .measure_text(
                    &self.address_text[..boundary],
                    13.0,
                    FontWeight::Regular,
                    FontFamily::SansSerif,
                )
                .0;
            let cursor_x = text_render_x + cursor_offset;
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(cursor_x, addr_y + 6.0, 1.5, addr_h - 12.0),
                color: Color::MANGO_ORANGE,
            });
        }

        // Mango menu button (⋮)
        let menu_x = w - 34.0;
        let menu_hovered = self.hovered_target == Some(HoverTarget::Menu);
        if menu_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(menu_x - 3.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            ":".to_string(),
            menu_x + 5.0,
            btn_y + 1.0,
            TOOLBAR_TEXT,
            18.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Toolbar bottom separator line
        dl.push(DisplayCommand::DrawLine {
            x1: 0.0,
            y1: HEADER_HEIGHT,
            x2: w,
            y2: HEADER_HEIGHT,
            color: Color::rgb(70, 72, 77),
            thickness: 1.0,
        });

        // Loading indicator bar
        if self.is_loading {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(0.0, HEADER_HEIGHT - 2.0, w * 0.7, 2.5),
                color: Color::MANGO_ORANGE,
            });
        }

        // ══════════════════════════════════════════════════════════════════════
        // ── CHROME REDESIGN: PART 3 — STATUS BAR (y = h - STATUS_BAR_HEIGHT..h) ──
        // ══════════════════════════════════════════════════════════════════════

        let status_y = h - STATUS_BAR_HEIGHT;
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, status_y, w, STATUS_BAR_HEIGHT),
            color: STATUS_BAR_BG,
        });

        dl.push(DisplayCommand::DrawLine {
            x1: 0.0,
            y1: status_y,
            x2: w,
            y2: status_y,
            color: Color::rgb(60, 64, 67),
            thickness: 1.0,
        });

        dl.push(DisplayCommand::draw_text(
            self.status_text.clone(),
            8.0,
            status_y + 5.0,
            STATUS_BAR_TEXT,
            11.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        let title_str = format!("Mango 2026 | {}", self.page_title);
        let title_x = (w - (title_str.len() as f32 * 6.5) - 16.0).max(200.0);
        dl.push(DisplayCommand::draw_text(
            title_str,
            title_x,
            status_y + 5.0,
            STATUS_BAR_TEXT,
            11.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        // ══════════════════════════════════════════════════════════════════════
        // ── PART 4: RIGHT-CLICK CONTEXT MENU POPUP (Rendered on top of all) ──
        // ══════════════════════════════════════════════════════════════════════
        if let Some(menu) = &self.context_menu {
            let menu_rect = Rect::new(menu.x, menu.y, menu.width, menu.height);
            dl.push(DisplayCommand::FillRoundedRect {
                rect: menu_rect,
                color: Color::rgb(41, 42, 45),
                radii: [6.0, 6.0, 6.0, 6.0],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: menu_rect,
                color: Color::rgb(70, 72, 77),
                widths: mango_render::display_list::BorderWidths {
                    top: 1.0,
                    right: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                },
                radii: [6.0; 4],
            });

            for (i, item) in menu.items.iter().enumerate() {
                let item_y = menu.y + 4.0 + (i as f32 * 26.0);
                let item_rect = Rect::new(menu.x + 4.0, item_y, menu.width - 8.0, 24.0);

                if menu.hovered_idx == Some(i) && item.enabled {
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: item_rect,
                        color: Color::rgb(70, 72, 77),
                        radii: [4.0, 4.0, 4.0, 4.0],
                    });
                }

                let text_color = if item.enabled {
                    Color::rgb(232, 234, 237)
                } else {
                    Color::rgb(112, 115, 120)
                };

                dl.push(DisplayCommand::draw_text(
                    item.label.clone(),
                    menu.x + 12.0,
                    item_y + 4.0,
                    text_color,
                    13.0,
                    FontWeight::Regular,
                    FontFamily::SansSerif,
                ));
            }
        }

        // ══════════════════════════════════════════════════════════════════════
        // ── PART 5: SELECT DROPDOWN POPUP OVERLAY                           ──
        // ══════════════════════════════════════════════════════════════════════
        if let Some(dropdown) = &self.select_dropdown {
            let total_h = (dropdown.options.len() as f32 * dropdown.item_height + 8.0).min(300.0);
            let drop_rect = Rect::new(dropdown.x, dropdown.y, dropdown.width, total_h);
            dl.push(DisplayCommand::FillRoundedRect {
                rect: drop_rect,
                color: Color::rgb(255, 255, 255),
                radii: [6.0, 6.0, 6.0, 6.0],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: drop_rect,
                color: Color::rgb(180, 185, 195),
                widths: mango_render::display_list::BorderWidths {
                    top: 1.0,
                    right: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                },
                radii: [6.0; 4],
            });

            for (i, opt) in dropdown.options.iter().enumerate() {
                let item_y = dropdown.y + 4.0 + (i as f32 * dropdown.item_height);
                if item_y + dropdown.item_height > dropdown.y + total_h {
                    break;
                }
                let item_rect = Rect::new(
                    dropdown.x + 4.0,
                    item_y,
                    dropdown.width - 8.0,
                    dropdown.item_height - 2.0,
                );

                if dropdown.hovered_idx == Some(i) {
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: item_rect,
                        color: Color::rgb(225, 235, 255),
                        radii: [4.0, 4.0, 4.0, 4.0],
                    });
                } else if opt.selected {
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: item_rect,
                        color: Color::rgb(240, 243, 250),
                        radii: [4.0, 4.0, 4.0, 4.0],
                    });
                }

                let text_color = if opt.selected {
                    Color::rgb(24, 90, 188)
                } else {
                    Color::rgb(32, 33, 36)
                };

                dl.push(DisplayCommand::draw_text(
                    opt.text.clone(),
                    dropdown.x + 12.0,
                    item_y + 4.0,
                    text_color,
                    13.0,
                    FontWeight::Regular,
                    FontFamily::SansSerif,
                ));
            }
        }

        // ══════════════════════════════════════════════════════════════════════
        // ── PART 6: PICKER OVERLAY POPUP (color / date / time / file)         ──
        // ══════════════════════════════════════════════════════════════════════
        if let Some(picker) = &self.picker {
            let panel = Rect::new(picker.x, picker.y, picker.width, picker.height);
            dl.push(DisplayCommand::DrawBoxShadow {
                rect: panel,
                color: Color::rgba(0, 0, 0, 70),
                offset_x: 0.0,
                offset_y: 2.0,
                blur_radius: 6.0,
                spread_radius: 0.0,
                radii: [6.0; 4],
                inset: false,
            });
            dl.push(DisplayCommand::FillRoundedRect {
                rect: panel,
                color: Color::rgb(255, 255, 255),
                radii: [6.0; 4],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: panel,
                color: Color::rgb(180, 185, 195),
                widths: BorderWidths {
                    top: 1.0,
                    right: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                },
                radii: [6.0; 4],
            });

            match &picker.kind {
                PickerKind::Color => {
                    for (i, hex) in mango_layout::COLOR_SWATCHES.iter().enumerate() {
                        let col = (i % PICKER_SWATCH_COLS) as f32;
                        let row = (i / PICKER_SWATCH_COLS) as f32;
                        let rect = Rect::new(
                            picker.x
                                + PICKER_SWATCH_PAD
                                + col * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP),
                            picker.y
                                + PICKER_SWATCH_PAD
                                + row * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP),
                            PICKER_SWATCH_CELL,
                            PICKER_SWATCH_CELL,
                        );
                        let color = mango_css::values::Value::parse_color(hex)
                            .unwrap_or(Color::rgb(255, 0, 255));
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect,
                            color,
                            radii: [3.0; 4],
                        });
                        if picker.hover == Some(PickerHit::ColorSwatch(i as u32)) {
                            dl.push(DisplayCommand::DrawBorder {
                                rect,
                                color: Color::rgb(0, 120, 215),
                                widths: BorderWidths {
                                    top: 2.0,
                                    right: 2.0,
                                    bottom: 2.0,
                                    left: 2.0,
                                },
                                radii: [3.0; 4],
                            });
                        }
                    }
                }
                PickerKind::Date { year, month } => {
                    // Header: "<" / Month Year / ">" (ASCII arrows for glyph coverage).
                    let prev_rect =
                        Rect::new(picker.x + 4.0, picker.y + PICKER_DATE_HEADER_Y, 26.0, 22.0);
                    let next_rect = Rect::new(
                        picker.x + picker.width - 30.0,
                        picker.y + PICKER_DATE_HEADER_Y,
                        26.0,
                        22.0,
                    );
                    if picker.hover == Some(PickerHit::PrevMonth) {
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect: prev_rect,
                            color: Color::rgb(235, 238, 245),
                            radii: [4.0; 4],
                        });
                    }
                    if picker.hover == Some(PickerHit::NextMonth) {
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect: next_rect,
                            color: Color::rgb(235, 238, 245),
                            radii: [4.0; 4],
                        });
                    }
                    let arrow_font = 14.0;
                    dl.push(DisplayCommand::draw_text(
                        "<",
                        prev_rect.x()
                            + (prev_rect.width()
                                - mango_layout::measure_text_width("<", arrow_font))
                                / 2.0,
                        picker.y + PICKER_DATE_HEADER_Y + 4.0,
                        Color::rgb(60, 64, 67),
                        arrow_font,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));
                    dl.push(DisplayCommand::draw_text(
                        ">",
                        next_rect.x()
                            + (next_rect.width()
                                - mango_layout::measure_text_width(">", arrow_font))
                                / 2.0,
                        picker.y + PICKER_DATE_HEADER_Y + 4.0,
                        Color::rgb(60, 64, 67),
                        arrow_font,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));
                    let title = format!("{} {}", month_name(*month), year);
                    let title_w = mango_layout::measure_text_width(&title, 13.0);
                    dl.push(DisplayCommand::draw_text(
                        &title,
                        picker.x + (picker.width - title_w) / 2.0,
                        picker.y + PICKER_DATE_HEADER_Y + 4.0,
                        Color::rgb(32, 33, 36),
                        13.0,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));

                    // Weekday header row.
                    let weekdays = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
                    let col_w = (picker.width - PICKER_DATE_PAD * 2.0) / 7.0;
                    for (i, wd) in weekdays.iter().enumerate() {
                        let w = mango_layout::measure_text_width(wd, 10.0);
                        dl.push(DisplayCommand::draw_text(
                            *wd,
                            picker.x + PICKER_DATE_PAD + i as f32 * col_w + (col_w - w) / 2.0,
                            picker.y + 34.0,
                            Color::rgb(120, 124, 128),
                            10.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    }

                    // Day cells.
                    let lead = weekday_sunday0(days_from_civil(*year, *month, 1));
                    let dim = days_in_month(*year, *month);
                    let selected = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(picker.node_id, "value"))
                        .and_then(|v| parse_date_value(&v));
                    for day in 1..=dim {
                        let idx = lead + i64::from(day) - 1;
                        if !(0..42).contains(&idx) {
                            continue;
                        }
                        let row = idx / 7;
                        let col = idx % 7;
                        let cell = Rect::new(
                            picker.x + PICKER_DATE_PAD + col as f32 * col_w + 1.0,
                            picker.y + PICKER_DATE_GRID_Y + row as f32 * PICKER_DATE_ROW_H,
                            col_w - 2.0,
                            20.0,
                        );
                        let is_sel = selected == Some((*year, *month, day));
                        if is_sel {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect: cell,
                                color: Color::rgb(0, 120, 215),
                                radii: [4.0; 4],
                            });
                        } else if picker.hover == Some(PickerHit::Day(day)) {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect: cell,
                                color: Color::rgb(225, 235, 255),
                                radii: [4.0; 4],
                            });
                        }
                        let label = day.to_string();
                        let w = mango_layout::measure_text_width(&label, 12.0);
                        dl.push(DisplayCommand::draw_text(
                            &label,
                            cell.x() + (cell.width() - w) / 2.0,
                            cell.y() + 4.0,
                            if is_sel {
                                Color::WHITE
                            } else {
                                Color::rgb(32, 33, 36)
                            },
                            12.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    }
                }
                PickerKind::Time { hour, minute, .. } => {
                    let value = format!("{:02}:{:02}", hour, minute);
                    let vw = mango_layout::measure_text_width(&value, 18.0);
                    dl.push(DisplayCommand::draw_text(
                        &value,
                        picker.x + (picker.width - vw) / 2.0,
                        picker.y + 10.0,
                        Color::rgb(32, 33, 36),
                        18.0,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));

                    let stepper =
                        |dl: &mut DisplayList, x: f32, y: f32, label: &str, hovered: bool| {
                            let rect = Rect::new(x, y, PICKER_TIME_BTN_W, PICKER_TIME_BTN_H);
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect,
                                color: if hovered {
                                    Color::rgb(210, 226, 245)
                                } else {
                                    Color::rgb(240, 241, 243)
                                },
                                radii: [4.0; 4],
                            });
                            let w = mango_layout::measure_text_width(label, 13.0);
                            dl.push(DisplayCommand::draw_text(
                                label,
                                x + (PICKER_TIME_BTN_W - w) / 2.0,
                                y + 5.0,
                                Color::rgb(60, 64, 67),
                                13.0,
                                FontWeight::Bold,
                                FontFamily::SansSerif,
                            ));
                        };
                    let lx = picker.x + 10.0;
                    let rx = picker.x + picker.width - 10.0 - PICKER_TIME_BTN_W;
                    stepper(
                        &mut dl,
                        lx,
                        picker.y + PICKER_TIME_HOUR_Y,
                        "-",
                        picker.hover == Some(PickerHit::HourDown),
                    );
                    stepper(
                        &mut dl,
                        rx,
                        picker.y + PICKER_TIME_HOUR_Y,
                        "+",
                        picker.hover == Some(PickerHit::HourUp),
                    );
                    stepper(
                        &mut dl,
                        lx,
                        picker.y + PICKER_TIME_MIN_Y,
                        "-",
                        picker.hover == Some(PickerHit::MinDown),
                    );
                    stepper(
                        &mut dl,
                        rx,
                        picker.y + PICKER_TIME_MIN_Y,
                        "+",
                        picker.hover == Some(PickerHit::MinUp),
                    );

                    let center_label = |dl: &mut DisplayList, y: f32, text: &str| {
                        let w = mango_layout::measure_text_width(text, 14.0);
                        dl.push(DisplayCommand::draw_text(
                            text,
                            picker.x + (picker.width - w) / 2.0,
                            y,
                            Color::rgb(32, 33, 36),
                            14.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    };
                    let hour_text = hour.to_string();
                    let minute_text = minute.to_string();
                    center_label(&mut dl, picker.y + PICKER_TIME_HOUR_Y + 5.0, &hour_text);
                    center_label(&mut dl, picker.y + PICKER_TIME_MIN_Y + 5.0, &minute_text);

                    let set_rect = Rect::new(
                        picker.x + (picker.width - PICKER_TIME_SET_W) / 2.0,
                        picker.y + PICKER_TIME_SET_Y,
                        PICKER_TIME_SET_W,
                        PICKER_TIME_SET_H,
                    );
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: set_rect,
                        color: if picker.hover == Some(PickerHit::SetTime) {
                            Color::rgb(0, 100, 190)
                        } else {
                            Color::rgb(0, 120, 215)
                        },
                        radii: [4.0; 4],
                    });
                    let set_w = mango_layout::measure_text_width("Set", 13.0);
                    dl.push(DisplayCommand::draw_text(
                        "Set",
                        set_rect.x() + (set_rect.width() - set_w) / 2.0,
                        picker.y + PICKER_TIME_SET_Y + 6.0,
                        Color::WHITE,
                        13.0,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));
                }
                PickerKind::File {
                    dir,
                    entries,
                    offset,
                } => {
                    // Header showing the current directory path.
                    dl.push(DisplayCommand::FillRect {
                        rect: Rect::new(
                            picker.x + 1.0,
                            picker.y + 1.0,
                            picker.width - 2.0,
                            PICKER_FILE_ROW_Y - 1.0,
                        ),
                        color: Color::rgb(246, 247, 249),
                    });
                    dl.push(DisplayCommand::DrawLine {
                        x1: picker.x + 1.0,
                        y1: picker.y + PICKER_FILE_ROW_Y - 1.0,
                        x2: picker.x + picker.width - 1.0,
                        y2: picker.y + PICKER_FILE_ROW_Y - 1.0,
                        color: Color::rgb(216, 220, 226),
                        thickness: 1.0,
                    });
                    let path = truncate_label(&dir.display().to_string(), 46);
                    dl.push(DisplayCommand::draw_text(
                        &path,
                        picker.x + 8.0,
                        picker.y + 7.0,
                        Color::rgb(60, 64, 67),
                        11.0,
                        FontWeight::Regular,
                        FontFamily::SansSerif,
                    ));

                    let has_parent = dir.parent().is_some();
                    for row in 0..PICKER_FILE_ROWS {
                        let row_y = picker.y + PICKER_FILE_ROW_Y + row as f32 * PICKER_FILE_ROW_H;
                        if row_y + PICKER_FILE_ROW_H > picker.y + picker.height {
                            break;
                        }
                        let (row_hit, label, is_dir) = if has_parent && row == 0 {
                            (Some(PickerHit::FileParent), "..".to_string(), true)
                        } else {
                            let entry_row = row - usize::from(has_parent);
                            match entries.get(*offset + entry_row) {
                                Some(e) => (
                                    Some(PickerHit::FileRow((*offset + entry_row) as u32)),
                                    format!("{}{}", e.name, if e.is_dir { "/" } else { "" }),
                                    e.is_dir,
                                ),
                                None => (None, String::new(), false),
                            }
                        };
                        let Some(row_hit) = row_hit else {
                            continue;
                        };
                        let rect = Rect::new(
                            picker.x + 4.0,
                            row_y,
                            picker.width - 8.0,
                            PICKER_FILE_ROW_H - 1.0,
                        );
                        if picker.hover == Some(row_hit) {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect,
                                color: Color::rgb(225, 235, 255),
                                radii: [4.0; 4],
                            });
                        }
                        let label = truncate_label(&label, 44);
                        dl.push(DisplayCommand::draw_text(
                            &label,
                            rect.x() + 6.0,
                            row_y + 4.0,
                            if is_dir {
                                Color::rgb(0, 90, 170)
                            } else {
                                Color::rgb(32, 33, 36)
                            },
                            12.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    }
                }
            }
        }

        // ── DEVTOOLS & INSPECTOR PANEL (GAP-022) ──
        if self.devtools.is_open {
            // Borrowed view: the inspector repaints every frame while it is open.
            let doc = self.document_view();
            let highlight_cmds =
                self.devtools
                    .render_highlight(self.root_box.as_ref(), self.scroll_y, content_y);
            for cmd in highlight_cmds {
                dl.push(cmd);
            }
            let panel_cmds = self
                .devtools
                .render_panel(w, h, &doc, self.root_box.as_ref());
            for cmd in panel_cmds {
                dl.push(cmd);
            }
        }

        dl
    }

    /// Handles keyboard events.
    pub fn handle_key_event(&mut self, event: &KeyEvent) {
        // F12 or Ctrl+Shift+I toggles DevTools (GAP-022)
        if event.key == MangoKey::F12
            || (event.modifiers.ctrl
                && event.modifiers.shift
                && matches!(&event.key, MangoKey::Char('i') | MangoKey::Char('I')))
        {
            self.devtools.toggle();
            self.relayout();
            return;
        }

        if !self.address_focused {
            // Dismiss popups on Escape
            if event.key == MangoKey::Escape {
                if self.picker.is_some() {
                    self.picker = None;
                    return;
                }
                if self.select_dropdown.is_some() {
                    self.select_dropdown = None;
                    self.relayout();
                    return;
                }
                if self.context_menu.is_some() {
                    self.context_menu = None;
                    return;
                }
            }

            // Tab / Shift+Tab keyboard focus navigation (UX-3.4.1)
            if event.key == MangoKey::Tab && !event.modifiers.ctrl {
                self.focus_next_element(event.modifiers.shift);
                return;
            }

            // Keyboard navigation in open select dropdown
            if let Some(ref mut dropdown) = self.select_dropdown {
                match &event.key {
                    MangoKey::ArrowDown => {
                        let cur = dropdown.hovered_idx.unwrap_or(0);
                        if cur + 1 < dropdown.options.len() {
                            dropdown.hovered_idx = Some(cur + 1);
                        }
                        return;
                    }
                    MangoKey::ArrowUp => {
                        let cur = dropdown.hovered_idx.unwrap_or(0);
                        if cur > 0 {
                            dropdown.hovered_idx = Some(cur - 1);
                        }
                        return;
                    }
                    MangoKey::Enter => {
                        let idx = dropdown.hovered_idx.unwrap_or(0);
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
                        }
                        self.select_dropdown = None;
                        self.relayout();
                        return;
                    }
                    _ => {}
                }
            }

            // Global focus shortcut: Ctrl+L
            if event.modifiers.ctrl
                && matches!(&event.key, MangoKey::Char('l') | MangoKey::Char('L'))
            {
                self.clear_form_focus();
                self.address_focused = true;
                self.is_all_selected = true;
                self.cursor_pos = self.address_text.len();
                return;
            }

            // Tab / Shift+Tab keyboard focus navigation (UX-3.4.1)
            if event.key == MangoKey::Tab && !event.modifiers.ctrl {
                self.focus_next_element(event.modifiers.shift);
                return;
            }

            // Route key events to focused form input
            if let Some(node_id) = self.focused_control {
                let current_val = self
                    .js_runtime
                    .as_ref()
                    .and_then(|rt| rt.get_node_attribute(node_id, "value"))
                    .unwrap_or_default();

                // ArrowUp/ArrowDown step numeric controls (range slider / number spinner).
                if matches!(event.key, MangoKey::ArrowUp | MangoKey::ArrowDown)
                    && !event.modifiers.ctrl
                {
                    let input_type = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(node_id, "type"))
                        .map(|t| t.to_ascii_lowercase())
                        .unwrap_or_default();
                    if input_type == "range" || input_type == "number" {
                        let dir = if event.key == MangoKey::ArrowUp {
                            1.0
                        } else {
                            -1.0
                        };
                        self.step_numeric_control(node_id, dir);
                        return;
                    }
                }

                if event.modifiers.ctrl {
                    match &event.key {
                        MangoKey::Char('a') | MangoKey::Char('A') => {
                            self.control_cursor_pos = current_val.chars().count();
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    node_id,
                                    "_mango_cursor_pos",
                                    &self.control_cursor_pos.to_string(),
                                );
                            }
                            self.relayout();
                            return;
                        }
                        MangoKey::Char('c') | MangoKey::Char('C') => {
                            if !current_val.is_empty()
                                && let Ok(mut clipboard) = arboard::Clipboard::new()
                            {
                                let _ = clipboard.set_text(current_val);
                                self.status_text = "Copied text to clipboard".to_string();
                            }
                            return;
                        }
                        MangoKey::Char('v') | MangoKey::Char('V') => {
                            if let Ok(mut clipboard) = arboard::Clipboard::new()
                                && let Ok(paste_text) = clipboard.get_text()
                            {
                                let chars: Vec<char> = current_val.chars().collect();
                                let idx = self.control_cursor_pos.min(chars.len());
                                let mut new_chars =
                                    Vec::with_capacity(chars.len() + paste_text.len());
                                new_chars.extend_from_slice(&chars[..idx]);
                                new_chars.extend(paste_text.chars());
                                new_chars.extend_from_slice(&chars[idx..]);
                                self.control_cursor_pos = idx + paste_text.chars().count();
                                let new_val: String = new_chars.into_iter().collect();
                                self.set_control_value(node_id, &new_val);
                                if let Some(ref mut rt) = self.js_runtime {
                                    rt.set_node_attribute(
                                        node_id,
                                        "_mango_cursor_pos",
                                        &self.control_cursor_pos.to_string(),
                                    );
                                }
                                self.relayout();
                            }
                            return;
                        }
                        MangoKey::Char('x') | MangoKey::Char('X') => {
                            if !current_val.is_empty()
                                && let Ok(mut clipboard) = arboard::Clipboard::new()
                            {
                                let _ = clipboard.set_text(current_val);
                                self.control_cursor_pos = 0;
                                self.set_control_value(node_id, "");
                                if let Some(ref mut rt) = self.js_runtime {
                                    rt.set_node_attribute(node_id, "_mango_cursor_pos", "0");
                                }
                                self.relayout();
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                match &event.key {
                    MangoKey::Char(c) => {
                        let chars: Vec<char> = current_val.chars().collect();
                        let idx = self.control_cursor_pos.min(chars.len());
                        let mut new_chars = Vec::with_capacity(chars.len() + 1);
                        new_chars.extend_from_slice(&chars[..idx]);
                        new_chars.push(*c);
                        new_chars.extend_from_slice(&chars[idx..]);
                        self.control_cursor_pos = idx + 1;
                        let new_val: String = new_chars.into_iter().collect();
                        self.set_control_value(node_id, &new_val);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Space => {
                        let chars: Vec<char> = current_val.chars().collect();
                        let idx = self.control_cursor_pos.min(chars.len());
                        let mut new_chars = Vec::with_capacity(chars.len() + 1);
                        new_chars.extend_from_slice(&chars[..idx]);
                        new_chars.push(' ');
                        new_chars.extend_from_slice(&chars[idx..]);
                        self.control_cursor_pos = idx + 1;
                        let new_val: String = new_chars.into_iter().collect();
                        self.set_control_value(node_id, &new_val);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Backspace => {
                        let mut chars: Vec<char> = current_val.chars().collect();
                        if self.control_cursor_pos > 0 && !chars.is_empty() {
                            let idx = self.control_cursor_pos.min(chars.len()) - 1;
                            chars.remove(idx);
                            self.control_cursor_pos = idx;
                            let new_val: String = chars.into_iter().collect();
                            self.set_control_value(node_id, &new_val);
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    node_id,
                                    "_mango_cursor_pos",
                                    &self.control_cursor_pos.to_string(),
                                );
                            }
                            self.relayout();
                        }
                        return;
                    }
                    MangoKey::Delete => {
                        let mut chars: Vec<char> = current_val.chars().collect();
                        if self.control_cursor_pos < chars.len() {
                            chars.remove(self.control_cursor_pos);
                            let new_val: String = chars.into_iter().collect();
                            self.set_control_value(node_id, &new_val);
                            if let Some(ref mut rt) = self.js_runtime {
                                rt.set_node_attribute(
                                    node_id,
                                    "_mango_cursor_pos",
                                    &self.control_cursor_pos.to_string(),
                                );
                            }
                            self.relayout();
                        }
                        return;
                    }
                    MangoKey::ArrowLeft => {
                        self.control_cursor_pos = self.control_cursor_pos.saturating_sub(1);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::ArrowRight => {
                        let count = current_val.chars().count();
                        self.control_cursor_pos = (self.control_cursor_pos + 1).min(count);
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Home => {
                        self.control_cursor_pos = 0;
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::End => {
                        self.control_cursor_pos = current_val.chars().count();
                        if let Some(ref mut rt) = self.js_runtime {
                            rt.set_node_attribute(
                                node_id,
                                "_mango_cursor_pos",
                                &self.control_cursor_pos.to_string(),
                            );
                        }
                        self.relayout();
                        return;
                    }
                    MangoKey::Escape => {
                        self.clear_form_focus();
                        return;
                    }
                    MangoKey::Enter => {
                        self.status_text = "Submitting form...".to_string();
                        if let Some(ref mut rt) = self.js_runtime
                            && let Some(handler) = rt.get_node_attribute(node_id, "onchange")
                        {
                            let _ = rt.execute_script(&handler);
                        }
                        self.submit_form(node_id);
                        return;
                    }
                    _ => {}
                }
            }

            // Scroll content when address bar is not focused
            let content_h =
                (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
            let page_step = (content_h * 0.85) / 45.0;
            match &event.key {
                MangoKey::ArrowDown => {
                    self.handle_scroll(-1.0);
                }
                MangoKey::ArrowUp => {
                    self.handle_scroll(1.0);
                }
                MangoKey::PageDown => {
                    self.handle_scroll(-page_step);
                }
                MangoKey::PageUp => {
                    self.handle_scroll(page_step);
                }
                MangoKey::Space => {
                    if event.modifiers.shift {
                        self.handle_scroll(page_step);
                    } else {
                        self.handle_scroll(-page_step);
                    }
                }
                MangoKey::Home => {
                    self.scroll_y = 0.0;
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = 0.0;
                    }
                }
                MangoKey::End => {
                    self.scroll_y = self.max_scroll();
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = self.scroll_y;
                    }
                }
                _ => {}
            }
            return;
        }

        // ── Address Bar Focused ──
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
                    let safe_pos = self.cursor_pos.min(self.address_text.len());
                    let mut prev = safe_pos - 1;
                    while prev > 0 && !self.address_text.is_char_boundary(prev) {
                        prev -= 1;
                    }
                    self.address_text.drain(prev..safe_pos);
                    self.cursor_pos = prev;
                }
            }
            MangoKey::Delete => {
                if self.is_all_selected {
                    self.address_text.clear();
                    self.cursor_pos = 0;
                    self.is_all_selected = false;
                } else if self.cursor_pos < self.address_text.len() {
                    let safe_pos = self.cursor_pos.min(self.address_text.len());
                    let mut next = safe_pos + 1;
                    while next < self.address_text.len()
                        && !self.address_text.is_char_boundary(next)
                    {
                        next += 1;
                    }
                    self.address_text.drain(safe_pos..next);
                }
            }
            MangoKey::ArrowLeft => {
                self.is_all_selected = false;
                if self.cursor_pos > 0 {
                    let mut prev = self.cursor_pos.min(self.address_text.len()) - 1;
                    while prev > 0 && !self.address_text.is_char_boundary(prev) {
                        prev -= 1;
                    }
                    self.cursor_pos = prev;
                }
            }
            MangoKey::ArrowRight => {
                self.is_all_selected = false;
                if self.cursor_pos < self.address_text.len() {
                    let mut next = self.cursor_pos + 1;
                    while next < self.address_text.len()
                        && !self.address_text.is_char_boundary(next)
                    {
                        next += 1;
                    }
                    self.cursor_pos = next;
                }
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

    fn navigate_to_address(&mut self, push_history: bool) {
        self.is_all_selected = false;
        let mut input = self.address_text.trim().to_string();
        log::info!("Navigate to: {}", input);

        if input.starts_with('#')
            && let Some(base) = &self.base_url
            && let Ok(resolved) = base.resolve(&input)
        {
            input = resolved.to_string();
            self.address_text = input.clone();
        }

        if is_special_or_local_page(&input) {
            self.base_url = None;
            self.cached_stylesheets.clear();
            let html = resolve_navigation_input(&input);
            self.status_text = format!("Rendered: {}", input);
            self.load_html_internal(html, input, push_history);
            self.address_focused = false;
            return;
        }

        if is_search_query(&input) {
            let search_url = self.config.format_search_url(&input);
            log::info!(
                "Search query: routing to configured search engine: {}",
                search_url
            );
            input = search_url;
            self.address_text = input.clone();
        }

        let parsed_url = match Url::parse(&input) {
            Ok(url) => url,
            Err(e) => {
                let err_msg = format!("Invalid URL: {}", e);
                let err_html = error_page_html(&input, &err_msg);
                self.base_url = None;
                self.cached_stylesheets.clear();
                self.status_text = err_msg;
                self.load_html_internal(err_html, input, push_history);
                self.address_focused = false;
                return;
            }
        };

        // Same-document fragment navigation: avoid full network re-fetch when only the #hash changes
        if let Some(base) = &self.base_url
            && parsed_url.scheme == base.scheme
            && parsed_url.host == base.host
            && parsed_url.port == base.port
            && parsed_url.path == base.path
            && parsed_url.query == base.query
        {
            if push_history && let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.push_history(base.to_string(), self.page_title.clone());
            }
            self.base_url = Some(parsed_url.clone());
            self.address_text = parsed_url.to_string();
            self.address_focused = false;
            self.is_loading = false;
            if let Some(ref frag) = parsed_url.fragment {
                let frag_str = frag.clone();
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(Some(frag_str.clone()));
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(Some(frag_str.clone()));
                }
                let doc = self.active_document();
                if let Some(node_id) = find_element_by_id(&doc, &frag_str)
                    && let Some(root) = &self.root_box
                    && let Some(lbox) = root.find_box_for_node(node_id)
                {
                    let target_y = lbox.dimensions.content.y();
                    let max_s = self.max_scroll();
                    self.scroll_y = target_y.clamp(0.0, max_s);
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = self.scroll_y;
                    }
                }
            } else {
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(None);
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(None);
                }
            }
            self.relayout();
            return;
        }

        self.status_text = format!("Loading {}...", parsed_url);
        self.is_loading = true;

        if self.async_navigation {
            let loader = self.loader.clone();
            let target_url = parsed_url.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let res = loader.fetch_document(&target_url);
                let _ = tx.send(res);
            });
            self.pending_navigation = Some(PendingNavigation {
                target_url: parsed_url,
                rx,
                push_history,
            });
            return;
        }

        match self.loader.fetch_document(&parsed_url) {
            Ok(doc_result) => {
                self.is_loading = false;
                self.process_and_load_document(doc_result, push_history);
            }
            Err(e) => {
                self.is_loading = false;
                let err_msg = e.to_string();
                let err_html = error_page_html(&parsed_url.as_str(), &err_msg);
                self.base_url = None;
                self.cached_stylesheets.clear();
                self.status_text = format!("Error: {}", err_msg);
                self.load_html_internal(err_html, parsed_url.to_string(), push_history);
                self.address_focused = false;
            }
        }

        // Network responses may have deposited Set-Cookie headers in the jar;
        // flush them to the profile so sessions survive a restart (GAP-016).
        self.persist_cookies();
    }

    /// Polls background navigation. Returns true if a page navigation completed and UI needs redrawing.
    pub fn tick_navigation(&mut self) -> bool {
        let Some(pending) = self.pending_navigation.as_ref() else {
            return false;
        };

        match pending.rx.try_recv() {
            Ok(res) => {
                let pending = self.pending_navigation.take().unwrap();
                self.is_loading = false;
                match res {
                    Ok(doc_result) => {
                        self.process_and_load_document(doc_result, pending.push_history);
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let err_html = error_page_html(&pending.target_url.as_str(), &err_msg);
                        self.base_url = None;
                        self.cached_stylesheets.clear();
                        self.status_text = format!("Error: {}", err_msg);
                        self.load_html_internal(
                            err_html,
                            pending.target_url.to_string(),
                            pending.push_history,
                        );
                        self.address_focused = false;
                    }
                }
                self.persist_cookies();
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pending_navigation = None;
                self.is_loading = false;
                self.status_text = "Connection lost".to_string();
                true
            }
        }
    }

    /// Synchronously waits for any pending asynchronous navigation to complete.
    pub fn wait_for_navigation(&mut self) {
        if let Some(pending) = self.pending_navigation.take() {
            self.is_loading = false;
            if let Ok(res) = pending.rx.recv() {
                match res {
                    Ok(doc_result) => {
                        self.process_and_load_document(doc_result, pending.push_history);
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let err_html = error_page_html(&pending.target_url.as_str(), &err_msg);
                        self.base_url = None;
                        self.cached_stylesheets.clear();
                        self.status_text = format!("Error: {}", err_msg);
                        self.load_html_internal(
                            err_html,
                            pending.target_url.to_string(),
                            pending.push_history,
                        );
                        self.address_focused = false;
                    }
                }
                self.persist_cookies();
            }
        }
    }

    /// Loads a fetched document into the browser, recursively fetching external stylesheets,
    /// web fonts, and images, and updating the page metadata and history.
    pub fn process_and_load_document(
        &mut self,
        mut doc_result: FetchedDocument,
        push_history: bool,
    ) {
        let final_url = doc_result.url.clone();
        preprocess_wikipedia_appearance_html(&mut doc_result.html, &final_url.as_str());
        self.base_url = Some(final_url.clone());
        self.cached_stylesheets.clear();
        mango_render::clear_global_svg_symbols();

        let mut doc = parse_html(&doc_result.html);
        doc.character_set = doc_result.encoding.name().to_string();
        doc.content_type = doc_result.content_type.clone();

        let mut stylesheet_links = Vec::new();
        collect_stylesheet_links(&doc, doc.root(), &mut stylesheet_links);
        let mut visited_css = std::collections::HashSet::new();
        load_stylesheets_parallel(
            &self.loader,
            &final_url,
            &stylesheet_links,
            &mut visited_css,
            &mut self.cached_stylesheets,
        );

        load_web_fonts(&self.loader, Some(&final_url), &self.cached_stylesheets);

        let mut doc_images = Vec::new();
        collect_image_sources(&doc, doc.root(), &mut doc_images);

        let mut sheet_images = Vec::new();
        collect_stylesheet_images(&doc, &self.cached_stylesheets, &mut sheet_images);
        let inline_sheets = mango_layout::extract_style_elements(&doc);
        for sheet in &inline_sheets {
            for rule in &sheet.rules {
                if let mango_css::parser::Rule::Import(import_path) = rule {
                    load_stylesheet_recursive(
                        &self.loader,
                        &final_url,
                        import_path,
                        &mut visited_css,
                        &mut self.cached_stylesheets,
                    );
                }
            }
        }
        collect_stylesheet_images(&doc, &inline_sheets, &mut sheet_images);
        load_web_fonts(&self.loader, Some(&final_url), &inline_sheets);

        // Interleave stylesheet images (CSS header logos, icons, masks) and
        // document images (hero images, img tags) so the top of both sets are fetched eagerly.
        let mut image_sources = Vec::new();
        let sheet_take = 6.min(sheet_images.len());
        let doc_take = 6.min(doc_images.len());
        image_sources.extend(sheet_images[..sheet_take].iter().cloned());
        image_sources.extend(doc_images[..doc_take].iter().cloned());
        image_sources.extend(sheet_images[sheet_take..].iter().cloned());
        image_sources.extend(doc_images[doc_take..].iter().cloned());

        let mut seen = std::collections::HashSet::new();
        image_sources.retain(|src| seen.insert(src.clone()));

        // Prefetch eagerly up-front in parallel so above-the-fold media is ready
        let eager_count = image_sources.len().min(EAGER_IMAGE_PREFETCH);
        let (eager, queued) = image_sources.split_at(eager_count);

        let eager_to_fetch: Vec<String> = eager
            .iter()
            .filter(|src| get_cached_image(src).is_none())
            .cloned()
            .collect();

        if !eager_to_fetch.is_empty() {
            std::thread::scope(|s| {
                for src in &eager_to_fetch {
                    let loader = &self.loader;
                    let base = &final_url;
                    s.spawn(move || {
                        if src.trim_start().starts_with("data:") {
                            if let Some(decoded) = decode_data_uri(src) {
                                cache_image(src, decoded);
                            }
                            return;
                        }
                        if let Ok(bytes) = loader.fetch_image_bytes(base, src)
                            && let Some(decoded) = decode_image_bytes(&bytes)
                        {
                            cache_image(src, decoded.clone());
                            if let Ok(resolved) = base.resolve(src) {
                                cache_image(&resolved.as_str(), decoded);
                            }
                        }
                    });
                }
            });
        }

        // Spawn background worker for queued images to avoid any main-thread freeze
        let (tx, rx) = std::sync::mpsc::channel();
        self.image_rx = Some(rx);
        self.image_fetch_base = Some(final_url.clone());
        self.pending_image_fetches = queued.to_vec();

        if !queued.is_empty() {
            let loader = self.loader.clone();
            let base = final_url.clone();
            let to_fetch = queued.to_vec();
            std::thread::spawn(move || {
                for src in to_fetch {
                    if get_cached_image(&src).is_some() {
                        continue;
                    }
                    if src.trim_start().starts_with("data:") {
                        if let Some(decoded) = decode_data_uri(&src) {
                            cache_image(&src, decoded);
                            let _ = tx.send(src);
                        }
                        continue;
                    }
                    if let Ok(bytes) = loader.fetch_image_bytes(&base, &src)
                        && let Some(decoded) = decode_image_bytes(&bytes)
                    {
                        cache_image(&src, decoded.clone());
                        if let Ok(resolved) = base.resolve(&src) {
                            cache_image(&resolved.as_str(), decoded);
                        }
                        let _ = tx.send(src);
                    }
                }
            });
        }

        self.status_text = format!(
            "Loaded {} ({} bytes, HTTP {})",
            final_url,
            doc_result.html.len(),
            doc_result.status
        );
        self.devtools.record_network(
            final_url.to_string(),
            "GET",
            doc_result.status,
            doc_result.content_type.clone(),
            doc_result.html.len(),
        );
        self.devtools.log(
            crate::devtools::ConsoleLevel::Info,
            format!("Navigated to {}", final_url),
        );
        self.load_document_internal(
            doc,
            doc_result.html,
            final_url.to_string(),
            push_history,
            true,
        );
        self.address_focused = false;
    }

    /// Flushes the shared cookie jar to the profile file.
    pub fn persist_cookies(&self) {
        let Ok(jar) = self.cookie_jar.lock() else {
            return;
        };
        if let Err(e) = jar.save_to_file(&self.cookie_store_path) {
            log::warn!("Could not persist cookies: {e}");
        }
    }

    /// Flushes the shared localStorage store to the profile file (GAP-017).
    pub fn persist_local_storage(&self) {
        save_local_storage(&self.local_storage_path, &self.local_storage);
    }

    /// Flushes both profile-backed web storage stores. Call on shutdown.
    pub fn persist_profile(&self) {
        self.persist_cookies();
        self.persist_local_storage();
    }

    /// Downloads and decodes one image into the shared render cache.
    fn prefetch_image(&self, base: &Url, src: &str) {
        if get_cached_image(src).is_some() {
            return;
        }
        // `data:` URIs are self-contained — decode them without a round trip.
        if src.trim_start().starts_with("data:") {
            if let Some(decoded) = decode_data_uri(src) {
                cache_image(src, decoded);
            }
            return;
        }
        let Ok(img_bytes) = self.loader.fetch_image_bytes(base, src) else {
            return;
        };
        let Some(decoded) = decode_image_bytes(&img_bytes) else {
            return;
        };
        cache_image(src, decoded.clone());
        if let Ok(resolved) = base.resolve(src) {
            cache_image(&resolved.as_str(), decoded);
        }
    }

    /// True while background image prefetch work remains (OPT-009).
    pub fn has_pending_image_fetches(&self) -> bool {
        self.image_rx.is_some() || !self.pending_image_fetches.is_empty()
    }

    /// Fetches a slice of the queued image backlog. Returns `true` when at
    /// least one newly cached image is available for painting.
    pub fn drain_pending_images(&mut self) -> bool {
        let mut loaded_any = false;
        let mut disconnected = false;
        if let Some(ref rx) = self.image_rx {
            loop {
                match rx.try_recv() {
                    Ok(src) => {
                        loaded_any = true;
                        if let Some(pos) = self.pending_image_fetches.iter().position(|s| s == &src)
                        {
                            self.pending_image_fetches.remove(pos);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                self.image_rx = None;
                self.pending_image_fetches.clear();
            }
        } else if !self.pending_image_fetches.is_empty() {
            if let Some(base) = self.image_fetch_base.clone() {
                let take = IMAGE_PREFETCH_PER_FRAME.min(self.pending_image_fetches.len());
                let batch: Vec<String> = self.pending_image_fetches.drain(..take).collect();
                for src in batch {
                    if get_cached_image(&src).is_none() {
                        self.prefetch_image(&base, &src);
                    }
                    if get_cached_image(&src).is_some() {
                        loaded_any = true;
                    }
                }
            } else {
                self.pending_image_fetches.clear();
            }
        }
        if loaded_any {
            self.relayout();
        }
        loaded_any
    }

    /// Returns true if a navigation or page load is currently in progress.
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Sets the currently hovered DOM element, updating `data-mango-hover` in the active document.
    /// Returns `true` if the hovered element chain changed.
    pub fn set_hover_node(&mut self, node_id: Option<mango_html::dom::NodeId>) -> bool {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_hover_state(node_id)
        } else if let Some(ref mut doc) = self.cached_document {
            let mut new_chain = std::collections::HashSet::new();
            if let Some(target) = node_id {
                let mut cur = Some(target);
                let mut guard = 0;
                while let Some(nid) = cur {
                    if guard > 512 {
                        break;
                    }
                    guard += 1;
                    new_chain.insert(nid);
                    cur = doc.get(nid).and_then(|n| n.parent);
                }
            }
            if new_chain == self.hovered_dom_nodes {
                return false;
            }
            for nid in &self.hovered_dom_nodes {
                if !new_chain.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-hover"));
                }
            }
            for nid in &new_chain {
                if !self.hovered_dom_nodes.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .push(("data-mango-hover".to_string(), "true".to_string()));
                }
            }
            self.hovered_dom_nodes = new_chain;
            true
        } else {
            false
        }
    }

    /// Sets the currently active (mouse pressed) DOM element, updating `data-mango-active` in the active document.
    /// Returns `true` if the active element chain changed.
    pub fn set_active_node(&mut self, node_id: Option<mango_html::dom::NodeId>) -> bool {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_active_state(node_id)
        } else if let Some(ref mut doc) = self.cached_document {
            let mut new_chain = std::collections::HashSet::new();
            if let Some(target) = node_id {
                let mut cur = Some(target);
                let mut guard = 0;
                while let Some(nid) = cur {
                    if guard > 512 {
                        break;
                    }
                    guard += 1;
                    new_chain.insert(nid);
                    cur = doc.get(nid).and_then(|n| n.parent);
                }
            }
            if new_chain == self.active_dom_nodes {
                return false;
            }
            for nid in &self.active_dom_nodes {
                if !new_chain.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .retain(|(k, _)| !k.eq_ignore_ascii_case("data-mango-active"));
                }
            }
            for nid in &new_chain {
                if !self.active_dom_nodes.contains(nid)
                    && let Some(node) = doc.get_mut(*nid)
                    && let mango_html::dom::NodeData::Element(el) = &mut node.data
                {
                    el.attributes
                        .push(("data-mango-active".to_string(), "true".to_string()));
                }
            }
            self.active_dom_nodes = new_chain;
            true
        } else {
            false
        }
    }

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

    /// Handles mouse clicks on UI elements, context menu, form inputs, or links in content.
    pub fn handle_mouse_click(&mut self, button: MouseButton, state: KeyState) {
        if state == KeyState::Released && button == MouseButton::Left {
            self.scrollbar_dragging = false;

            let mut link_to_navigate = None;

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
                let hit_node = self
                    .root_box
                    .as_ref()
                    .and_then(|root| root.hit_test(mango_core::Point::new(content_x, content_y)));

                if let Some(target_href) = hit_link {
                    if let Some(node_id) = hit_node {
                        self.fire_dom_event(node_id, "click");
                    }
                    let nav_url = if let Some(base) = &self.base_url {
                        base.resolve(&target_href)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| target_href.clone())
                    } else {
                        target_href
                    };
                    log::info!("Navigating to clicked link: {}", nav_url);
                    link_to_navigate = Some(nav_url);
                } else if let Some(node_id) = hit_node {
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
            if let Some(nav_url) = link_to_navigate {
                self.navigate(&nav_url);
            }
            return;
        }

        if state != KeyState::Pressed {
            return;
        }

        if button == MouseButton::Left
            && self.mouse_y > HEADER_HEIGHT
            && self.mouse_y < self.height as f32 - STATUS_BAR_HEIGHT
        {
            let content_x = self.mouse_x;
            let content_y = self.mouse_y - HEADER_HEIGHT + self.scroll_y;
            let pt = mango_core::Point::new(content_x, content_y);
            let active_node = self.root_box.as_ref().and_then(|root| root.hit_test(pt));
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

    /// Collects all interactive elements in DOM tree order that can receive keyboard focus (UX-3.4.1).
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

    /// Advances or reverses focus among interactive elements on Tab / Shift+Tab (UX-3.4.1).
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
    fn handle_media_control_click(&mut self, hit: mango_layout::MediaControlHit) {
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
    fn control_content_rect(&self, node_id: NodeId) -> Option<(f32, f32, f32, f32)> {
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
    fn fire_dom_event(&mut self, node_id: NodeId, event_type: &str) {
        if let Some(ref mut rt) = self.js_runtime {
            rt.set_node_attribute(node_id, "data-mango-fire", event_type);
            let script = format!(
                "if (typeof _mangoWrap === 'function') {{ var __el = _mangoWrap({}); if (__el) __el.dispatchEvent({{ type: '{}' }}); }} else if (typeof document !== 'undefined') {{ var __el = document.querySelector('[data-mango-fire]'); if (__el) {{ __el.dispatchEvent({{ type: '{}' }}); __el.removeAttribute('data-mango-fire'); }} }}",
                node_id.raw(),
                event_type,
                event_type
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
    fn numeric_params(&self, node_id: NodeId) -> (Option<f32>, Option<f32>, f32) {
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
    fn range_value_for_fraction(&self, node_id: NodeId, frac: f32) -> f32 {
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
    fn step_numeric_control(&mut self, node_id: NodeId, dir: f32) {
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
    fn focus_text_control(&mut self, node_id: NodeId, hit: &mango_layout::FormControlHit) {
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

    /// Opens a picker overlay anchored below the clicked control (above it when
    /// there is no room below), clamped into the window's content area.
    fn open_picker(&mut self, node_id: NodeId, kind: PickerKind) {
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
    fn handle_picker_click(&mut self) {
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

    /// Recomputes a dragged slider's value from the current mouse position.
    fn update_range_drag(&mut self, node_id: NodeId) {
        let Some((cx, _cy, cw, _ch)) = self.control_content_rect(node_id) else {
            return;
        };
        if cw <= 0.0 {
            return;
        }
        let frac = (self.mouse_x - cx) / cw;
        let formatted = fmt_number(self.range_value_for_fraction(node_id, frac));
        let changed = self
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, "value"))
            .map(|v| v != formatted)
            .unwrap_or(true);
        if changed {
            self.set_control_value(node_id, &formatted);
            self.relayout();
            self.status_text = format!("Range: {}", formatted);
        }
    }

    /// Determines the initial `(year, month)` for the date picker from the
    /// control's value, falling back to today's date.
    fn picker_start_date(&self, node_id: NodeId) -> (i32, u32) {
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
    fn picker_start_time(&self, node_id: NodeId) -> (u32, u32, u32) {
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

    /// Handles clicks on interactive form controls.
    fn handle_form_control_click(&mut self, hit: mango_layout::FormControlHit) {
        let Some(node_id) = hit.node_id else {
            return;
        };

        match hit.form_type.as_str() {
            "checkbox" => {
                let new_checked = !hit.checked;
                // Real event order: `click` dispatches first (listeners observe the
                // old state), then the checked state flips, then `input` + `change`.
                self.fire_dom_event(node_id, "click");
                if let Some(ref mut rt) = self.js_runtime
                    && let Some(onclick) = rt.get_node_attribute(node_id, "onclick")
                {
                    let _ = rt.execute_script(&onclick);
                }
                if let Some(ref mut rt) = self.js_runtime {
                    if new_checked {
                        rt.set_node_attribute(node_id, "checked", "true");
                    } else {
                        rt.remove_node_attribute(node_id, "checked");
                    }
                }
                self.fire_dom_event(node_id, "input");
                self.fire_dom_event(node_id, "change");
                self.relayout();
                self.status_text = if new_checked {
                    "Checkbox checked".to_string()
                } else {
                    "Checkbox unchecked".to_string()
                };
            }
            "radio" => {
                // BUG-004 fix: Uncheck all sibling radio buttons with the same name
                self.fire_dom_event(node_id, "click");
                if let Some(ref mut rt) = self.js_runtime {
                    let radio_name = rt.get_node_attribute(node_id, "name").unwrap_or_default();
                    if !radio_name.is_empty() {
                        // Find the form ancestor to scope the radio group
                        let doc = rt.document_snapshot();
                        let form_root = {
                            let mut curr = Some(node_id);
                            let mut found_form = None;
                            while let Some(nid) = curr {
                                if let Some(node) = doc.get(nid) {
                                    if let NodeData::Element(el) = &node.data
                                        && el.tag_name.eq_ignore_ascii_case("form")
                                    {
                                        found_form = Some(nid);
                                        break;
                                    }
                                    curr = node.parent;
                                } else {
                                    break;
                                }
                            }
                            found_form.unwrap_or(doc.root())
                        };
                        // Collect all radio inputs with the same name under the form
                        let mut radios_to_uncheck = Vec::new();
                        let mut stack = vec![form_root];
                        while let Some(nid) = stack.pop() {
                            if let Some(node) = doc.get(nid) {
                                if let NodeData::Element(el) = &node.data
                                    && el.tag_name.eq_ignore_ascii_case("input")
                                    && el
                                        .get_attribute("type")
                                        .map(|t| t.eq_ignore_ascii_case("radio"))
                                        .unwrap_or(false)
                                    && el
                                        .get_attribute("name")
                                        .map(|n| n == radio_name)
                                        .unwrap_or(false)
                                    && nid != node_id
                                {
                                    radios_to_uncheck.push(nid);
                                }
                                let mut child = node.first_child;
                                while let Some(cid) = child {
                                    stack.push(cid);
                                    if let Some(cnode) = doc.get(cid) {
                                        child = cnode.next_sibling;
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                        // Uncheck all siblings
                        for rid in radios_to_uncheck {
                            rt.remove_node_attribute(rid, "checked");
                        }
                    }
                    // Check the clicked radio
                    rt.set_node_attribute(node_id, "checked", "true");
                }
                if let Some(ref mut doc) = self.cached_document {
                    let radio_name = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(node_id, "name"))
                        .unwrap_or_default();
                    if !radio_name.is_empty() {
                        let mut stack = vec![doc.root()];
                        while let Some(nid) = stack.pop() {
                            if let Some(node) = doc.get_mut(nid) {
                                if let NodeData::Element(ref mut el) = node.data
                                    && el.tag_name.eq_ignore_ascii_case("input")
                                    && el
                                        .get_attribute("type")
                                        .map(|t| t.eq_ignore_ascii_case("radio"))
                                        .unwrap_or(false)
                                    && el
                                        .get_attribute("name")
                                        .map(|n| n == radio_name)
                                        .unwrap_or(false)
                                {
                                    if nid == node_id {
                                        el.set_attribute("checked", "true");
                                    } else {
                                        el.remove_attribute("checked");
                                    }
                                }
                                let mut child = node.first_child;
                                while let Some(cid) = child {
                                    stack.push(cid);
                                    if let Some(cnode) = doc.get(cid) {
                                        child = cnode.next_sibling;
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                let (radio_name, radio_val) = if let Some(ref rt) = self.js_runtime {
                    let n = rt.get_node_attribute(node_id, "name").unwrap_or_default();
                    let v = rt.get_node_attribute(node_id, "value").unwrap_or_default();
                    if !n.is_empty() {
                        (n, v)
                    } else if let Some(ref doc) = self.cached_document {
                        let elem = doc.get(node_id).and_then(|n| match &n.data {
                            NodeData::Element(e) => Some(e),
                            _ => None,
                        });
                        (
                            elem.and_then(|e| e.get_attribute("name"))
                                .unwrap_or_default()
                                .to_string(),
                            elem.and_then(|e| e.get_attribute("value"))
                                .unwrap_or_default()
                                .to_string(),
                        )
                    } else {
                        (n, v)
                    }
                } else if let Some(ref doc) = self.cached_document {
                    let elem = doc.get(node_id).and_then(|n| match &n.data {
                        NodeData::Element(e) => Some(e),
                        _ => None,
                    });
                    (
                        elem.and_then(|e| e.get_attribute("name"))
                            .unwrap_or_default()
                            .to_string(),
                        elem.and_then(|e| e.get_attribute("value"))
                            .unwrap_or_default()
                            .to_string(),
                    )
                } else {
                    (String::new(), String::new())
                };

                if radio_name == "color" {
                    self.cached_stylesheets.retain(|s| {
                        !s.rules.iter().any(|r| {
                            if let mango_css::parser::Rule::Style(sr) = r {
                                format!("{:?}", sr.selectors).contains("mango-wiki-dark-theme")
                            } else {
                                false
                            }
                        })
                    });
                    if radio_val == "dark" {
                        let dark_css = r#"
                            html, body.mango-wiki-dark-theme, html.mango-wiki-dark-theme, body { background-color: #1a1a1a !important; color: #eaecf0 !important; }
                            h1, h2, h3, h4, h5, h6, #firstHeading, .mw-first-heading, .mw-heading, .mw-heading1, .mw-heading2 { color: #ffffff !important; }
                            .vector-header-container, .vector-sticky-header { background-color: #202122 !important; color: #eaecf0 !important; border-bottom-color: #3a3b3c !important; }
                            #content, .mw-body { background-color: #1a1a1a !important; color: #eaecf0 !important; border-color: #3a3b3c !important; }
                            .vector-pinned-container, .vector-appearance, .vector-toc { background-color: #202122 !important; color: #eaecf0 !important; }
                            .vector-pinnable-header-label, .vector-appearance-content, .vector-appearance-content div, .vector-appearance-content label { color: #eaecf0 !important; }
                            a { color: #6b9eff !important; }
                            .infobox, .thumbinner, .wikitable { background-color: #202122 !important; color: #eaecf0 !important; border-color: #3a3b3c !important; }
                            .infobox th, .infobox td, .wikitable th, .wikitable td { color: #eaecf0 !important; border-color: #3a3b3c !important; }
                            .cdx-text-input__input { background-color: #282a2b !important; color: #eaecf0 !important; border-color: #54595d !important; }
                            .cdx-button { background-color: #282a2b !important; color: #eaecf0 !important; border-color: #54595d !important; }
                        "#;
                        let sheet = mango_css::parser::parse_stylesheet(dark_css);
                        self.cached_stylesheets.push(sheet);
                        self.status_text = "Appearance: Dark mode enabled".to_string();
                    } else {
                        self.status_text = "Appearance: Light mode enabled".to_string();
                    }
                } else if radio_name == "width" {
                    self.cached_stylesheets.retain(|s| {
                        !s.rules.iter().any(|r| {
                            if let mango_css::parser::Rule::Style(sr) = r {
                                format!("{:?}", sr.selectors).contains("mango-wiki-wide-width")
                            } else {
                                false
                            }
                        })
                    });
                    if radio_val == "wide" {
                        let wide_css = ".mw-page-container.mango-wiki-wide-width, .mw-content-container, .vector-body, #bodyContent, .mw-parser-output { max-width: 100% !important; }";
                        let sheet = mango_css::parser::parse_stylesheet(wide_css);
                        self.cached_stylesheets.push(sheet);
                        self.status_text = "Appearance: Wide width enabled".to_string();
                    } else {
                        self.status_text = "Appearance: Standard width enabled".to_string();
                    }
                } else if radio_name == "font-size" {
                    self.cached_stylesheets.retain(|s| {
                        !s.rules.iter().any(|r| {
                            if let mango_css::parser::Rule::Style(sr) = r {
                                format!("{:?}", sr.selectors).contains("mango-wiki-font-size")
                            } else {
                                false
                            }
                        })
                    });
                    let size_css = match radio_val.as_str() {
                        "small" => {
                            "body.mango-wiki-font-size, #content, .mw-body { font-size: 13px !important; }"
                        }
                        "large" => {
                            "body.mango-wiki-font-size, #content, .mw-body { font-size: 17px !important; }"
                        }
                        _ => {
                            "body.mango-wiki-font-size, #content, .mw-body { font-size: 14.4px !important; }"
                        }
                    };
                    let sheet = mango_css::parser::parse_stylesheet(size_css);
                    self.cached_stylesheets.push(sheet);
                    self.status_text = format!("Appearance: Text size {}", radio_val);
                } else {
                    self.status_text = "Radio option selected".to_string();
                }

                self.fire_dom_event(node_id, "input");
                self.fire_dom_event(node_id, "change");
                self.relayout();
            }
            "submit" | "button" | "reset" => {
                self.fire_dom_event(node_id, "click");
                self.status_text = format!("Clicked <{}> button", hit.form_type);
                if let Some(ref mut rt) = self.js_runtime {
                    let class = rt.get_node_attribute(node_id, "class").unwrap_or_default();
                    if class.contains("vector-pinnable-header-unpin-button") {
                        let doc = rt.document_snapshot();
                        let mut content_nid = None;
                        for idx in 0..15000 {
                            let nid = mango_core::Id::from_raw(idx);
                            if let Some(node) = doc.get(nid)
                                && let NodeData::Element(el) = &node.data
                                && el.get_attribute("id") == Some("vector-appearance-content")
                            {
                                content_nid = Some(nid);
                                break;
                            }
                        }
                        if let Some(cnid) = content_nid {
                            let cur_style =
                                rt.get_node_attribute(cnid, "style").unwrap_or_default();
                            if cur_style.contains("display: none") {
                                rt.set_node_attribute(
                                    cnid,
                                    "style",
                                    "padding: 4px 0; font-family: sans-serif; color: #202122;",
                                );
                                self.status_text = "Appearance section expanded".to_string();
                            } else {
                                rt.set_node_attribute(cnid, "style", "display: none !important;");
                                self.status_text = "Appearance section collapsed".to_string();
                            }
                            self.relayout();
                            return;
                        }
                    }
                    if let Some(onclick) = rt.get_node_attribute(node_id, "onclick") {
                        let _ = rt.execute_script(&onclick);
                        self.relayout();
                    }
                }
                if hit.form_type == "submit" {
                    self.submit_form(node_id);
                }
            }
            "range" => {
                // Click (and subsequent drag) positions the slider thumb; the
                // trailing `change` fires when the drag is released. Focus first
                // so ArrowUp/ArrowDown can step the value from the keyboard.
                self.focus_text_control(node_id, &hit);
                if let Some((cx, _cy, cw, _ch)) = self.control_content_rect(node_id)
                    && cw > 0.0
                {
                    self.range_dragging = Some(node_id);
                    self.range_drag_start_value = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(node_id, "value"));
                    let frac = (self.mouse_x - cx) / cw;
                    let formatted = fmt_number(self.range_value_for_fraction(node_id, frac));
                    self.set_control_value(node_id, &formatted);
                    self.relayout();
                    self.status_text = format!("Range: {}", formatted);
                }
            }
            "number" => {
                // Spinner arrows occupy the right-hand 16px of the control,
                // split into top (increment) / bottom (decrement) halves.
                if let Some((cx, cy, cw, ch)) = self.control_content_rect(node_id)
                    && cw >= 28.0
                    && ch >= 16.0
                {
                    let screen_y = HEADER_HEIGHT + cy - self.scroll_y;
                    let rel_x = self.mouse_x - cx;
                    let rel_y = self.mouse_y - screen_y;
                    if rel_x >= cw - 16.0 && (0.0..=ch).contains(&rel_y) {
                        let dir = if rel_y < ch / 2.0 { 1.0 } else { -1.0 };
                        self.step_numeric_control(node_id, dir);
                        return;
                    }
                }
                self.focus_text_control(node_id, &hit);
            }
            "color" => {
                self.open_picker(node_id, PickerKind::Color);
                self.status_text = "Pick a color".to_string();
            }
            "date" => {
                let (year, month) = self.picker_start_date(node_id);
                self.focus_text_control(node_id, &hit);
                self.open_picker(node_id, PickerKind::Date { year, month });
                self.status_text = "Pick a date".to_string();
            }
            "time" => {
                let (hour, minute, minute_step) = self.picker_start_time(node_id);
                self.focus_text_control(node_id, &hit);
                self.open_picker(
                    node_id,
                    PickerKind::Time {
                        hour,
                        minute,
                        minute_step,
                    },
                );
                self.status_text = "Pick a time".to_string();
            }
            "file" => {
                let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let entries = list_directory(&dir);
                self.open_picker(
                    node_id,
                    PickerKind::File {
                        dir,
                        entries,
                        offset: 0,
                    },
                );
                self.status_text = "Choose a file".to_string();
            }
            "select" => {
                self.status_text = "Select dropdown opened".to_string();
                let doc = self.active_document();

                let mut options = Vec::new();
                if let Some(node) = doc.get(node_id) {
                    let mut child = node.first_child;
                    while let Some(cid) = child {
                        if let Some(cnode) = doc.get(cid) {
                            if let NodeData::Element(el) = &cnode.data
                                && el.tag_name.eq_ignore_ascii_case("option")
                            {
                                // BUG-008 fix: Use recursive text_content to handle nested inline elements
                                let opt_text = doc.text_content(cid);
                                let text = if opt_text.trim().is_empty() {
                                    el.get_attribute("value").unwrap_or("").to_string()
                                } else {
                                    opt_text.trim().to_string()
                                };
                                let value = el.get_attribute("value").unwrap_or(&text).to_string();
                                let selected =
                                    el.get_attribute("selected").is_some() || hit.value == value;
                                options.push(SelectDropdownOption {
                                    node_id: cid,
                                    text,
                                    value,
                                    selected,
                                });
                            }
                            child = cnode.next_sibling;
                        } else {
                            break;
                        }
                    }
                }

                if !options.is_empty() {
                    let mut drop_x = self.mouse_x;
                    let mut drop_y = self.mouse_y + 10.0;
                    let mut drop_w = 160.0f32;

                    if let Some(ref root) = self.root_box
                        && let Some(lbox) = root.find_box_for_node(node_id)
                    {
                        let bbox = lbox.dimensions.border_box();
                        drop_x = bbox.x();
                        drop_y = HEADER_HEIGHT + bbox.y() - self.scroll_y + bbox.height();
                        drop_w = bbox.width().max(140.0);
                    }

                    let item_height = 24.0f32;
                    let total_h = (options.len() as f32 * item_height + 8.0).min(300.0);
                    if drop_y + total_h > self.height as f32 - STATUS_BAR_HEIGHT {
                        drop_y = (drop_y - total_h - 26.0).max(HEADER_HEIGHT);
                    }

                    self.select_dropdown = Some(SelectDropdown {
                        select_node_id: node_id,
                        x: drop_x.min((self.width as f32 - drop_w - 4.0).max(0.0)),
                        y: drop_y,
                        width: drop_w,
                        item_height,
                        options,
                        hovered_idx: None,
                    });
                }
            }
            "summary" => {
                let doc = self.active_document();
                let mut curr = doc.get(node_id).and_then(|n| n.parent);
                let mut details_node = None;
                while let Some(nid) = curr {
                    if let Some(node) = doc.get(nid) {
                        if let NodeData::Element(el) = &node.data
                            && el.tag_name.eq_ignore_ascii_case("details")
                        {
                            details_node = Some(nid);
                            break;
                        }
                        curr = node.parent;
                    } else {
                        break;
                    }
                }

                if let Some(det_id) = details_node {
                    let is_open = if let Some(det_node) = doc.get(det_id)
                        && let NodeData::Element(el) = &det_node.data
                    {
                        el.get_attribute("open").is_some()
                    } else {
                        false
                    };

                    if let Some(ref mut rt) = self.js_runtime {
                        if is_open {
                            rt.remove_node_attribute(det_id, "open");
                            self.status_text = "Collapsed details".to_string();
                        } else {
                            rt.set_node_attribute(det_id, "open", "");
                            self.status_text = "Expanded details".to_string();
                        }
                    }
                    self.relayout();
                }
            }
            "label" => {
                let doc = self.active_document();

                let target_node = if let Some(lbl_node) = doc.get(node_id)
                    && let NodeData::Element(el) = &lbl_node.data
                    && let Some(for_id) = el.get_attribute("for")
                {
                    find_element_by_id(&doc, for_id)
                } else {
                    let mut found = None;
                    let mut stack = vec![node_id];
                    while let Some(nid) = stack.pop() {
                        if nid != node_id
                            && let Some(node) = doc.get(nid)
                            && let NodeData::Element(el) = &node.data
                            && matches!(
                                el.tag_name.to_ascii_lowercase().as_str(),
                                "input" | "select" | "textarea" | "button"
                            )
                        {
                            found = Some(nid);
                            break;
                        }
                        if let Some(node) = doc.get(nid) {
                            let mut child = node.first_child;
                            while let Some(cid) = child {
                                stack.push(cid);
                                child = doc.get(cid).and_then(|c| c.next_sibling);
                            }
                        }
                    }
                    found
                };

                if let Some(target_id) = target_node
                    && let Some(tnode) = doc.get(target_id)
                    && let NodeData::Element(el) = &tnode.data
                {
                    let t_type = el
                        .get_attribute("type")
                        .unwrap_or(if el.tag_name.eq_ignore_ascii_case("select") {
                            "select"
                        } else {
                            "text"
                        })
                        .to_ascii_lowercase();
                    let t_checked = el.get_attribute("checked").is_some();
                    self.handle_form_control_click(mango_layout::FormControlHit {
                        node_id: Some(target_id),
                        tag_name: el.tag_name.to_ascii_lowercase(),
                        form_type: t_type,
                        name: el.get_attribute("name").unwrap_or("").to_string(),
                        value: el.get_attribute("value").unwrap_or("").to_string(),
                        checked: t_checked,
                        click_offset_x: 0.0,
                    });
                }
            }
            _ => {
                // Text input / password / search / textarea / date / time / number
                self.focus_text_control(node_id, &hit);
            }
        }
    }
}

impl Drop for BrowserChrome {
    fn drop(&mut self) {
        self.persist_profile();
    }
}

/// Restores a `localStorage` profile (`<key>\t<value>` or `<origin>\t<key>\t<value>` per line) into the store.
fn load_local_storage(path: &std::path::Path, store: &mango_js::web_apis::SharedLocalStorage) {
    match mango_js::web_apis::load_local_storage_from_file(path, store) {
        Ok(count) if count > 0 => {
            log::info!(
                "Restored {count} localStorage entries from {}",
                path.display()
            );
        }
        _ => {}
    }
}

/// Writes the `localStorage` store to its profile file (GAP-017, Phase 6.4 / PRD 7.6).
fn save_local_storage(path: &std::path::Path, store: &mango_js::web_apis::SharedLocalStorage) {
    if let Err(e) = mango_js::web_apis::save_local_storage_to_file(path, store) {
        log::warn!("Could not persist localStorage: {e}");
    }
}

/// Truncates a string to at most `max` characters with an ASCII ellipsis.
fn truncate_label(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(3)).collect();
    format!("{}...", cut)
}

/// Recursively searches for an element with the given `id` attribute.
fn find_element_by_id(doc: &Document, target_id: &str) -> Option<NodeId> {
    fn search(doc: &Document, node_id: NodeId, target_id: &str) -> Option<NodeId> {
        if let Some(node) = doc.get(node_id)
            && let NodeData::Element(ref elem) = node.data
            && elem.id() == Some(target_id)
        {
            return Some(node_id);
        }
        for child in doc.children(node_id) {
            if let Some(found) = search(doc, child.id, target_id) {
                return Some(found);
            }
        }
        None
    }
    search(doc, doc.root(), target_id)
}

impl BrowserChrome {
    /// Submits the HTML form enclosing or associated with `control_node_id`.
    pub fn submit_form(&mut self, control_node_id: NodeId) {
        let doc = self.active_document();

        // 1. Find form ancestor or control with form="id"
        let mut curr = Some(control_node_id);
        let mut form_node = None;
        while let Some(nid) = curr {
            if let Some(node) = doc.get(nid) {
                if let NodeData::Element(el) = &node.data
                    && el.tag_name.eq_ignore_ascii_case("form")
                {
                    form_node = Some(nid);
                    break;
                }
                curr = node.parent;
            } else {
                break;
            }
        }

        let form_id = match form_node {
            Some(f) => f,
            None => {
                if let Some(node) = doc.get(control_node_id)
                    && let NodeData::Element(el) = &node.data
                    && let Some(target_form_id) = el.get_attribute("form")
                    && let Some(fid) = find_element_by_id(&doc, target_form_id)
                {
                    fid
                } else {
                    return;
                }
            }
        };

        // 2. Read form action and method
        let (action, method) = if let Some(node) = doc.get(form_id) {
            if let NodeData::Element(el) = &node.data {
                let a = el.get_attribute("action").unwrap_or("").to_string();
                let m = el
                    .get_attribute("method")
                    .unwrap_or("GET")
                    .to_ascii_uppercase();
                (a, m)
            } else {
                ("".to_string(), "GET".to_string())
            }
        } else {
            ("".to_string(), "GET".to_string())
        };

        // 3. Collect form inputs
        // 3. Collect form inputs
        let mut form_values = Vec::new();
        let mut stack = vec![form_id];
        while let Some(nid) = stack.pop() {
            if let Some(node) = doc.get(nid) {
                if let NodeData::Element(el) = &node.data {
                    let tag = el.tag_name.to_ascii_lowercase();
                    let is_disabled = el.get_attribute("disabled").is_some();
                    if !is_disabled
                        && (tag == "input" || tag == "textarea" || tag == "select")
                        && let Some(name) = el.get_attribute("name")
                        && !name.is_empty()
                    {
                        if tag == "textarea" {
                            let val = el
                                .get_attribute("value")
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| doc.text_content(nid));
                            form_values.push((name.to_string(), val));
                        } else if tag == "select" {
                            let is_multiple = el.get_attribute("multiple").is_some();
                            let mut options = Vec::new();
                            let mut opt_stack = vec![nid];
                            while let Some(curr_id) = opt_stack.pop() {
                                if let Some(curr_node) = doc.get(curr_id) {
                                    if curr_id != nid
                                        && let NodeData::Element(opt_el) = &curr_node.data
                                        && opt_el.tag_name.eq_ignore_ascii_case("option")
                                    {
                                        let opt_disabled =
                                            opt_el.get_attribute("disabled").is_some();
                                        let opt_selected =
                                            opt_el.get_attribute("selected").is_some();
                                        let opt_val = opt_el
                                            .get_attribute("value")
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| {
                                                doc.text_content(curr_id).trim().to_string()
                                            });
                                        options.push((opt_disabled, opt_selected, opt_val));
                                    }
                                    let mut child = curr_node.first_child;
                                    let mut children = Vec::new();
                                    while let Some(cid) = child {
                                        children.push(cid);
                                        if let Some(cnode) = doc.get(cid) {
                                            child = cnode.next_sibling;
                                        } else {
                                            break;
                                        }
                                    }
                                    for c in children.into_iter().rev() {
                                        opt_stack.push(c);
                                    }
                                }
                            }
                            if is_multiple {
                                for (opt_disabled, opt_selected, opt_val) in options {
                                    if !opt_disabled && opt_selected {
                                        form_values.push((name.to_string(), opt_val));
                                    }
                                }
                            } else {
                                let selected = options
                                    .iter()
                                    .find(|(d, s, _)| !d && *s)
                                    .or_else(|| options.iter().find(|(d, _, _)| !d));
                                if let Some((_, _, opt_val)) = selected {
                                    form_values.push((name.to_string(), opt_val.clone()));
                                } else {
                                    let val = el.get_attribute("value").unwrap_or("").to_string();
                                    form_values.push((name.to_string(), val));
                                }
                            }
                        } else {
                            let input_type = el
                                .get_attribute("type")
                                .unwrap_or("text")
                                .to_ascii_lowercase();
                            if input_type == "button"
                                || input_type == "reset"
                                || input_type == "image"
                            {
                                // Omitted
                            } else if input_type == "submit" {
                                if nid == control_node_id {
                                    let val = el.get_attribute("value").unwrap_or("").to_string();
                                    form_values.push((name.to_string(), val));
                                }
                            } else if input_type == "checkbox" || input_type == "radio" {
                                let is_checked = el.get_attribute("checked").is_some();
                                if is_checked {
                                    let val = el.get_attribute("value").unwrap_or("on").to_string();
                                    form_values.push((name.to_string(), val));
                                }
                            } else {
                                let val = el.get_attribute("value").unwrap_or("").to_string();
                                form_values.push((name.to_string(), val));
                            }
                        }
                    }
                }
                let mut child = node.first_child;
                let mut children = Vec::new();
                while let Some(cid) = child {
                    children.push(cid);
                    if let Some(cnode) = doc.get(cid) {
                        child = cnode.next_sibling;
                    } else {
                        break;
                    }
                }
                for c in children.into_iter().rev() {
                    stack.push(c);
                }
            }
        }

        // 4. Construct query string
        let mut query_parts = Vec::new();
        for (k, v) in form_values {
            let enc_k = url_encode(&k);
            let enc_v = url_encode(&v);
            query_parts.push(format!("{}={}", enc_k, enc_v));
        }
        let query_str = query_parts.join("&");

        let novalidate = if let Some(node) = doc.get(form_id) {
            if let NodeData::Element(el) = &node.data {
                el.get_attribute("novalidate").is_some()
            } else {
                false
            }
        } else {
            false
        };
        let submitter_novalidate = doc
            .get(control_node_id)
            .map(|node| {
                if let NodeData::Element(el) = &node.data {
                    el.get_attribute("formnovalidate").is_some()
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if !novalidate && !submitter_novalidate {
            let mut invalid_control = None;
            let mut val_stack = vec![form_id];
            while let Some(nid) = val_stack.pop() {
                if let Some(node) = doc.get(nid) {
                    if let NodeData::Element(el) = &node.data {
                        let tag = el.tag_name.to_ascii_lowercase();
                        if (tag == "input" || tag == "textarea" || tag == "select")
                            && !mango_css::selectors::element_is_valid(nid, el)
                        {
                            invalid_control = Some((nid, el.clone()));
                            break;
                        }
                    }
                    let mut child = node.first_child;
                    let mut children = Vec::new();
                    while let Some(cid) = child {
                        children.push(cid);
                        if let Some(cnode) = doc.get(cid) {
                            child = cnode.next_sibling;
                        } else {
                            break;
                        }
                    }
                    for c in children.into_iter().rev() {
                        val_stack.push(c);
                    }
                }
            }

            if let Some((inval_id, inval_el)) = invalid_control {
                self.focused_control = Some(inval_id);
                let msg = if inval_el.get_attribute("required").is_some() {
                    "Please fill out this field."
                } else if inval_el.get_attribute("pattern").is_some() {
                    "Please match the requested format."
                } else {
                    "Please provide a valid value."
                };
                self.status_text = msg.to_string();
                log::info!(
                    "Form submission blocked: control {:?} is invalid ({})",
                    inval_id,
                    msg
                );
                return;
            }
        }

        let resolved_action = if action.is_empty() {
            self.address_text.clone()
        } else if let Some(base) = &self.base_url {
            base.resolve(&action)
                .map(|u| u.to_string())
                .unwrap_or(action)
        } else if action.starts_with('/') {
            format!("https://html.duckduckgo.com{}", action)
        } else {
            action
        };

        if method == "POST" {
            log::info!("Form submitted (POST): sending to {}", resolved_action);
            if let Ok(parsed_url) = Url::parse(&resolved_action) {
                self.status_text = format!("Submitting to {}...", parsed_url);
                self.is_loading = true;
                match self.loader.post_document(
                    &parsed_url,
                    query_str.as_bytes(),
                    "application/x-www-form-urlencoded",
                ) {
                    Ok(doc_result) => {
                        self.is_loading = false;
                        self.address_text = doc_result.url.to_string();
                        self.process_and_load_document(doc_result, true);
                        self.persist_cookies();
                        return;
                    }
                    Err(e) => {
                        log::warn!(
                            "POST submission error: {}. Falling back to GET navigation.",
                            e
                        );
                    }
                }
            }
        }

        let target_url = if query_str.is_empty() {
            resolved_action
        } else if resolved_action.contains('?') {
            format!("{}&{}", resolved_action, query_str)
        } else {
            format!("{}?{}", resolved_action, query_str)
        };

        log::info!("Form submitted ({}): navigating to {}", method, target_url);
        self.navigate(&target_url);
    }

    /// Opens the page source in a new tab.
    pub fn open_view_source(&mut self) {
        let source_html = format!(
            "<!DOCTYPE html><html><head><title>Source: {}</title><style>body {{ background-color: #1e1e1e; color: #d4d4d4; font-family: monospace; font-size: 13px; padding: 16px; margin: 0; line-height: 1.5; }}</style></head><body><h2>View Source: {}</h2><hr/><p>{}</p></body></html>",
            self.address_text,
            self.address_text,
            html_escape(&self.current_html)
        );
        self.new_tab();
        self.load_html(source_html, format!("view-source:{}", self.address_text));
    }

    /// Exports the current page to a PDF document (GAP-021).
    pub fn print_to_pdf(
        &self,
        path: &std::path::Path,
        options: Option<mango_render::PdfOptions>,
    ) -> std::io::Result<()> {
        let opts = options.unwrap_or_default();
        mango_render::print_to_pdf(&self.page_display_list, path, &opts)
    }

    /// Builds the accessibility tree for the currently loaded document (GAP-020).
    pub fn accessibility_tree(&self) -> mango_layout::a11y::A11yTree {
        let doc = self.document_view();
        mango_layout::a11y::A11yTree::build(&doc, self.root_box.as_ref())
    }
}

fn is_js_script_type(type_attr: Option<&str>) -> bool {
    match type_attr {
        None => true,
        Some(t) => {
            let t = t.trim().to_ascii_lowercase();
            t.is_empty()
                || t == "text/javascript"
                || t == "application/javascript"
                || t == "text/ecmascript"
                || t == "application/ecmascript"
                || t == "javascript"
        }
    }
}

enum ScriptToRun {
    Inline(String),
    External(String),
}

fn collect_scripts(doc: &Document, node_id: NodeId, scripts: &mut Vec<ScriptToRun>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
        && elem.tag_name.eq_ignore_ascii_case("script")
        && is_js_script_type(elem.get_attribute("type"))
    {
        if let Some(src) = elem.get_attribute("src") {
            let trimmed = src.trim();
            if !trimmed.is_empty() {
                scripts.push(ScriptToRun::External(trimmed.to_string()));
            }
        } else {
            let code = doc.text_content(node_id);
            if !code.trim().is_empty() {
                scripts.push(ScriptToRun::Inline(code));
            }
        }
    }
    for child in doc.children(node_id) {
        collect_scripts(doc, child.id, scripts);
    }
}

fn collect_stylesheet_links(doc: &Document, node_id: NodeId, links: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(elem) = &node.data
        && elem.tag_name.eq_ignore_ascii_case("link")
    {
        let is_stylesheet = elem
            .get_attribute("rel")
            .map(|r| {
                r.split_whitespace()
                    .any(|t| t.eq_ignore_ascii_case("stylesheet"))
            })
            .unwrap_or(false);
        if is_stylesheet && let Some(href) = elem.get_attribute("href") {
            let trimmed = href.trim();
            if !trimmed.is_empty() {
                links.push(trimmed.to_string());
            }
        }
    }
    for child in doc.children(node_id) {
        collect_stylesheet_links(doc, child.id, links);
    }
}

fn load_stylesheets_parallel(
    loader: &ResourceLoader,
    base_url: &Url,
    links: &[String],
    visited: &mut std::collections::HashSet<String>,
    out: &mut Vec<Stylesheet>,
) {
    if links.is_empty() {
        return;
    }
    let mut to_fetch = Vec::new();
    for href in links {
        if let Ok(resolved) = base_url.resolve(href) {
            let key = resolved.to_string();
            if visited.insert(key) {
                to_fetch.push((href.clone(), resolved));
            }
        }
    }
    if to_fetch.is_empty() {
        return;
    }

    let results: Vec<Result<String, mango_net::NetworkError>> = std::thread::scope(|s| {
        let handles: Vec<_> = to_fetch
            .iter()
            .map(|(href, _)| s.spawn(|| loader.fetch_stylesheet(base_url, href)))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    Err(mango_net::NetworkError::Other("thread join failed".into()))
                })
            })
            .collect()
    });

    for ((_href, resolved), res) in to_fetch.into_iter().zip(results) {
        match res {
            Ok(css_text) => {
                log::info!("Loaded external stylesheet: {}", resolved);
                let sheet = parse_stylesheet(&css_text);
                for rule in &sheet.rules {
                    if let mango_css::parser::Rule::Import(import_path) = rule {
                        load_stylesheet_recursive(loader, &resolved, import_path, visited, out);
                    }
                }
                out.push(sheet);
            }
            Err(e) => {
                log::warn!("Failed to fetch external stylesheet {}: {}", resolved, e);
            }
        }
    }
}

fn load_stylesheet_recursive(
    loader: &ResourceLoader,
    base_url: &Url,
    href: &str,
    visited: &mut std::collections::HashSet<String>,
    out: &mut Vec<Stylesheet>,
) {
    if let Ok(resolved) = base_url.resolve(href) {
        let key = resolved.to_string();
        if !visited.insert(key) {
            return;
        }
        match loader.fetch_stylesheet(base_url, href) {
            Ok(css_text) => {
                log::info!("Loaded external stylesheet: {}", resolved);
                let sheet = parse_stylesheet(&css_text);
                for rule in &sheet.rules {
                    if let mango_css::parser::Rule::Import(import_path) = rule {
                        load_stylesheet_recursive(loader, &resolved, import_path, visited, out);
                    }
                }
                out.push(sheet);
            }
            Err(e) => {
                log::warn!("Failed to fetch external stylesheet {}: {}", resolved, e);
            }
        }
    }
}

fn load_web_fonts(loader: &ResourceLoader, base_url: Option<&Url>, stylesheets: &[Stylesheet]) {
    let mut font_faces = Vec::new();
    for sheet in stylesheets {
        for rule in &sheet.rules {
            if let mango_css::parser::Rule::FontFace(font_face) = rule {
                font_faces.push(font_face);
            }
        }
    }

    // Prefetch all unique remote font files in parallel
    if let Some(base) = base_url {
        let mut needed_urls: Vec<String> = Vec::new();
        for face in &font_faces {
            if !face.src_url.starts_with("data:") && !needed_urls.contains(&face.src_url) {
                needed_urls.push(face.src_url.clone());
            }
        }
        if !needed_urls.is_empty() {
            std::thread::scope(|s| {
                for url in &needed_urls {
                    s.spawn(|| {
                        let _ = loader.fetch_font_bytes(base, url);
                    });
                }
            });
        }
    }

    let try_load_face = |font_face: &mango_css::parser::FontFaceRule, weight: FontWeight| -> bool {
        if font_manager().has_web_font_weight(&font_face.font_family, weight) {
            return false;
        }

        // BUG-005 fix: Properly decode data: URI by extracting and base64-decoding the payload
        let font_bytes_res: Result<Vec<u8>, String> = if font_face.src_url.starts_with("data:") {
            decode_data_uri_font(&font_face.src_url).map_err(|e| e.to_string())
        } else if let Some(base) = base_url {
            loader
                .fetch_font_bytes(base, &font_face.src_url)
                .map_err(|e| e.to_string())
        } else {
            return false;
        };

        if let Ok(font_bytes) = font_bytes_res {
            match font_manager().register_web_font(&font_face.font_family, weight, &font_bytes) {
                Ok(id) => {
                    log::info!(
                        "Loaded and registered web font '{}' (ID: {}, weight: {:?})",
                        font_face.font_family,
                        id,
                        weight
                    );
                    true
                }
                Err(e) => {
                    log::warn!(
                        "Failed to register web font '{}' from {}: {}",
                        font_face.font_family,
                        font_face.src_url,
                        e
                    );
                    false
                }
            }
        } else {
            false
        }
    };

    // Pass 1: True Bold (weight >= 700 or Bold / Bolder) and Regular (weight <= 500 or Normal),
    // prioritizing normal font-style (not italic) so normal text gets upright glyphs.
    for face in &font_faces {
        if face.font_style != mango_css::values::FontStyle::Normal {
            continue;
        }
        let is_true_bold = match face.font_weight {
            mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
            mango_css::values::FontWeight::Numeric(w) => w >= 700,
            _ => false,
        };
        let is_regular = match face.font_weight {
            mango_css::values::FontWeight::Normal => true,
            mango_css::values::FontWeight::Numeric(w) => w <= 500,
            _ => false,
        };
        if is_true_bold {
            try_load_face(face, FontWeight::Bold);
        } else if is_regular {
            try_load_face(face, FontWeight::Regular);
        }
    }

    // Pass 2: Fallbacks for families still missing Bold (e.g. 600 SemiBold)
    for face in &font_faces {
        if face.font_style != mango_css::values::FontStyle::Normal {
            continue;
        }
        let is_semi_bold = match face.font_weight {
            mango_css::values::FontWeight::Numeric(w) => w >= 600,
            _ => false,
        };
        if is_semi_bold && !font_manager().has_web_font_weight(&face.font_family, FontWeight::Bold)
        {
            try_load_face(face, FontWeight::Bold);
        }
    }
}

/// BUG-005 fix: Decode a `data:` URI into raw bytes.
/// Supports both base64-encoded and plain text data URIs.
/// Format: `data:[<mediatype>][;base64],<data>`
fn decode_data_uri_font(data_uri: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let rest = data_uri.strip_prefix("data:").ok_or("Not a data: URI")?;
    if let Some(comma_pos) = rest.find(',') {
        let metadata = &rest[..comma_pos];
        let payload = &rest[comma_pos + 1..];
        if metadata.contains(";base64") {
            let decoded = base64_decode(payload.trim())?;
            Ok(decoded)
        } else {
            // Plain text / percent-encoded
            Ok(payload.as_bytes().to_vec())
        }
    } else {
        Err("Malformed data: URI — no comma separator".into())
    }
}

/// Minimal base64 decoder (RFC 4648). Avoids adding a crate dependency.
fn base64_decode(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    fn val(c: u8) -> Result<u8, Box<dyn std::error::Error>> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("Invalid base64 character: {}", c as char).into()),
        }
    }

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);

    let chunks = filtered.chunks(4);
    for chunk in chunks {
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push((b2 << 6) | b3);
        }
    }

    Ok(out)
}

fn collect_image_sources(doc: &Document, node_id: NodeId, sources: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(elem) = &node.data
    {
        // 1. If <img>, check src, srcset, data-src
        if elem.tag_name.eq_ignore_ascii_case("img") {
            if let Some(src) = elem.get_attribute("src") {
                let trimmed = src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
            if let Some(srcset) = elem.get_attribute("srcset") {
                for candidate in srcset.split(',') {
                    let url = candidate.split_whitespace().next().unwrap_or("");
                    if !url.is_empty() && !url.starts_with("data:") {
                        sources.push(url.to_string());
                    }
                }
            }
            if let Some(data_src) = elem.get_attribute("data-src") {
                let trimmed = data_src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
        }

        // 1b. If <source> (e.g. inside <picture>), collect srcset URLs for prefetching (GAP-019)
        if elem.tag_name.eq_ignore_ascii_case("source") {
            if let Some(srcset) = elem.get_attribute("srcset") {
                for candidate in srcset.split(',') {
                    let url = candidate.split_whitespace().next().unwrap_or("");
                    if !url.is_empty() && !url.starts_with("data:") {
                        sources.push(url.to_string());
                    }
                }
            }
            if let Some(src) = elem.get_attribute("src") {
                let trimmed = src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
        }

        // 2. Check inline style attribute for url(...)
        if let Some(style_attr) = elem.get_attribute("style") {
            let lower = style_attr.to_ascii_lowercase();
            let mut search_idx = 0;
            while let Some(pos) = lower[search_idx..].find("url(") {
                let start_idx = search_idx + pos + 4;
                if let Some(end_rel) = style_attr[start_idx..].find(')') {
                    let raw_url = &style_attr[start_idx..start_idx + end_rel];
                    let trimmed = raw_url.trim().trim_matches('\'').trim_matches('"').trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                        sources.push(trimmed.to_string());
                    }
                    search_idx = start_idx + end_rel + 1;
                } else {
                    break;
                }
            }
        }
    }
    for child in doc.children(node_id) {
        collect_image_sources(doc, child.id, sources);
    }
}

fn collect_stylesheet_images(
    _doc: &Document,
    stylesheets: &[mango_css::parser::Stylesheet],
    sources: &mut Vec<String>,
) {
    for sheet in stylesheets {
        for rule in &sheet.rules {
            match rule {
                mango_css::parser::Rule::Style(style_rule) => {
                    for decl in &style_rule.declarations {
                        extract_value_images(&decl.value, sources);
                    }
                }
                mango_css::parser::Rule::Media(media_rule) => {
                    for style_rule in &media_rule.rules {
                        for decl in &style_rule.declarations {
                            extract_value_images(&decl.value, sources);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn extract_value_images(val: &mango_css::values::Value, sources: &mut Vec<String>) {
    match val {
        mango_css::values::Value::Url(url) => {
            let trimmed = url.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                sources.push(trimmed.to_string());
            }
        }
        mango_css::values::Value::List(items) => {
            for item in items {
                extract_value_images(item, sources);
            }
        }
        _ => {}
    }
}

fn collect_box_background_images(b: &mango_layout::box_tree::LayoutBox, out: &mut Vec<String>) {
    if let Some(ref s) = b.style {
        if let Some(ref bg) = s.background_image {
            out.push(bg.clone());
        }
        if let Some(ref mask) = s.mask_image {
            out.push(mask.clone());
        }
    }
    for c in &b.children {
        collect_box_background_images(c, out);
    }
}

fn extract_title(doc: &Document) -> Option<String> {
    let title_id = doc.find_element_by_tag(doc.root(), "title")?;
    let text = doc.text_content(title_id).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn resolve_navigation_input(input: &str) -> String {
    if input.starts_with('<') {
        input.to_string()
    } else if let Some(html) = input.strip_prefix("data:text/html,") {
        html.to_string()
    } else if input == "about:blank" {
        "<!DOCTYPE html><html><body></body></html>".to_string()
    } else if input == "test:forms" {
        forms_test_html()
    } else if input == "test:svg" {
        svg_test_html()
    } else if input == "test:tables" {
        tables_test_html()
    } else if input == "test:css" {
        css_test_html()
    } else if input == "test:box" {
        box_model_test_html()
    } else if input == "test:inline" {
        inline_test_html()
    } else if input == "test:fonts" {
        fonts_test_html()
    } else if input == "test:image" {
        image_test_html()
    } else if input == "test:net" {
        test_net_html()
    } else {
        welcome_page_html()
    }
}

fn test_net_html() -> String {
    format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Networking Test Bench</title>
            <style>
                body {{ background-color: #f8fafc; margin: 20px; color: #1e293b; }}
                .hero {{
                    background-color: #ffa136;
                    color: #ffffff;
                    padding: 16px;
                    margin-bottom: 14px;
                }}
                h1 {{ font-size: 24px; color: #ffffff; margin: 0px 0px 6px 0px; }}
                .subtitle {{ font-size: 13px; color: #ffffff; margin: 0px; }}
                .card {{
                    background-color: #ffffff;
                    padding: 16px;
                    margin-bottom: 14px;
                    border-top-width: 1px;
                    border-right-width: 1px;
                    border-bottom-width: 1px;
                    border-left-width: 1px;
                    border-top-color: #e2e8f0;
                    border-right-color: #e2e8f0;
                    border-bottom-color: #e2e8f0;
                    border-left-color: #e2e8f0;
                }}
                h2 {{ font-size: 17px; color: #ffa136; margin: 0px 0px 8px 0px; }}
                p {{ font-size: 13px; color: #334155; margin: 4px 0px; }}
                .code {{ color: #0284c7; font-size: 13px; margin: 4px 0px; }}
            </style>
        </head>
        <body>
            <div class="hero">
                <h1>Networking & Resource Engine</h1>
                <p class="subtitle">Pure Rust HTTP/1.1 client, TLS 1.3/1.2 via rustls & webpki-roots, DNS cache & LRU cache</p>
            </div>
            <div class="card">
                <h2>Active Networking Engine</h2>
                <p>Backend: {}</p>
                <p>URL Parser: RFC 3986 with auto-scheme inference</p>
                <p>DNS Cache: Thread-safe TTL cache</p>
                <p>Resource Cache: In-memory LRU cache with HTTP Cache-Control support</p>
            </div>
            <div class="card">
                <h2>Real Websites to Test Live</h2>
                <p class="code">https://httpbin.org/html</p>
                <p class="code">https://example.com</p>
                <p class="code">https://info.cern.ch</p>
            </div>
        </body>
        </html>
        "#,
        mango_net::tls_backend_info()
    )
}

fn welcome_page_html() -> String {
    r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Mango Browser</title>
        <style>
            body {
                background-color: #ffffff;
                margin: 0;
                padding: 0;
                color: #202124;
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
                text-align: center;
            }
            .header-accent {
                height: 4px;
                background-color: #ffa136;
                width: 100%;
            }
            .container {
                max-width: 650px;
                margin: 0 auto;
                padding: 48px 24px 24px 24px;
            }
            .logo {
                font-size: 52px;
                font-weight: bold;
                color: #202124;
                margin-bottom: 4px;
                letter-spacing: -1px;
            }
            .logo-accent {
                color: #ffa136;
            }
            .tagline {
                font-size: 15px;
                color: #5f6368;
                margin-top: 0;
                margin-bottom: 28px;
            }
            .search-box {
                display: block;
                margin: 0 auto 20px auto;
                width: 100%;
            }
            input[type="text"] {
                width: 100%;
                max-width: 520px;
                padding: 12px 20px;
                font-size: 16px;
                color: #202124;
                background-color: #ffffff;
                border: 1px solid #dfe1e5;
                border-radius: 24px;
                box-sizing: border-box;
            }
            .buttons-row {
                margin-top: 18px;
                margin-bottom: 36px;
            }
            button {
                background-color: #f8f9fa;
                color: #3c4043;
                border: 1px solid #dadce0;
                border-radius: 4px;
                padding: 8px 18px;
                font-size: 14px;
                font-weight: 500;
                margin: 0 4px;
                cursor: pointer;
            }
            .btn-primary {
                background-color: #ffa136;
                color: #ffffff;
                border: 1px solid #e08b26;
                font-weight: bold;
            }
            .shortcuts {
                margin-top: 24px;
                padding-top: 20px;
                border-top: 1px solid #eeeeee;
            }
            .shortcuts-title {
                font-size: 13px;
                font-weight: bold;
                color: #70757a;
                text-transform: uppercase;
                letter-spacing: 0.5px;
                margin-bottom: 14px;
            }
            .links-grid {
                margin: 10px 0;
            }
            .link-pill {
                display: inline-block;
                background-color: #f1f3f4;
                color: #1a73e8;
                padding: 8px 16px;
                margin: 4px;
                border-radius: 16px;
                font-size: 13px;
                font-weight: 500;
                text-decoration: none;
            }
            .footer {
                margin-top: 40px;
                font-size: 12px;
                color: #9aa0a6;
            }
        </style>
    </head>
    <body>
        <div class="header-accent"></div>
        <div class="container">
            <div class="logo">
                <svg width="42" height="42" viewBox="0 0 32 32" style="vertical-align: middle; margin-right: 6px;">
                    <path d="M16 3 C9 3, 4 10, 6 19 C8 27, 17 31, 24 26 C29 22, 28 13, 23 7 C20 4, 18 3, 16 3 Z" fill="#ffa136"/>
                    <path d="M17 3 C18 1, 22 1, 23 4 C21 5, 19 5, 17 3 Z" fill="#16a34a"/>
                </svg>
                <span class="logo-accent">Mango</span>
            </div>
            <p class="tagline">Pure Rust Modern Web Browser</p>

            <form action="https://html.duckduckgo.com/html/" method="GET">
                <div class="search-box">
                    <input type="text" name="q" placeholder="Search with DuckDuckGo or type a URL..." autofocus="true" />
                </div>
                <div class="buttons-row">
                    <button type="submit" class="btn-primary">DuckDuckGo Search</button>
                    <button type="button" onclick="window.location.href='https://en.wikipedia.org/wiki/Special:Random'">I'm Feeling Lucky</button>
                </div>
            </form>

            <div class="shortcuts">
                <div class="shortcuts-title">Quick Navigation</div>
                <div class="links-grid">
                    <p>
                        <a class="link-pill" href="https://html.duckduckgo.com/html/">DuckDuckGo</a>
                        <a class="link-pill" href="https://en.wikipedia.org/wiki/Main_Page">Wikipedia</a>
                        <a class="link-pill" href="https://www.rust-lang.org/">Rust Lang</a>
                        <a class="link-pill" href="http://info.cern.ch/hypertext/WWW/TheProject.html">First Website (CERN)</a>
                    </p>
                    <p>
                        <a class="link-pill" href="test:forms">Form Controls</a>
                        <a class="link-pill" href="test:svg">Vector SVG</a>
                        <a class="link-pill" href="test:tables">CSS Tables</a>
                        <a class="link-pill" href="test:fonts">Typography</a>
                        <a class="link-pill" href="test:net">Net Benchmark</a>
                    </p>
                </div>
            </div>

            <div class="footer">
                Built with 100% Pure Safe Rust • Zero C/C++ • Servo & Google Inspired UI
            </div>
        </div>
        <script>
            console.log("Mango welcome page script executed successfully!");
        </script>
    </body>
    </html>
    "##.to_string()
}

fn forms_test_html() -> String {
    r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Forms & Input Controls - Mango</title>
        <style>
            body { background-color: #f1f5f9; color: #0f172a; font-family: sans-serif; margin: 24px; }
            .card { background-color: #ffffff; border: 1px solid #cbd5e1; border-radius: 8px; padding: 20px; margin-bottom: 20px; }
            h1 { font-size: 24px; color: #0f172a; margin-top: 0; }
            .field { margin-bottom: 14px; }
            label { display: block; font-weight: bold; margin-bottom: 4px; }
            input[type="text"], input[type="password"] { width: 320px; padding: 6px 10px; border: 1px solid #94a3b8; border-radius: 4px; font-size: 14px; }
            textarea { width: 320px; height: 60px; padding: 6px 10px; border: 1px solid #94a3b8; border-radius: 4px; font-size: 13px; }
            select { width: 260px; padding: 4px 8px; border: 1px solid #94a3b8; border-radius: 4px; }
            button { background-color: #ffa136; color: #ffffff; border: 1px solid #d97706; border-radius: 4px; padding: 6px 16px; cursor: pointer; font-weight: bold; }
            .btn-icon { background-color: #0284c7; border-color: #0369a1; }
        </style>
    </head>
    <body>
        <div class="card">
            <h1>Interactive Form Controls & Buttons</h1>
            <p>Click any input to type or test buttons with embedded SVG icons:</p>
            <div class="field">
                <label>User Name:</label>
                <input type="text" value="MangoUser" placeholder="Enter username"/>
            </div>
            <div class="field">
                <label>Password:</label>
                <input type="password" value="secret123" placeholder="Password"/>
            </div>
            <div class="field">
                <label>Comments:</label>
                <textarea placeholder="Write feedback here...">Fast pure-Rust rendering!</textarea>
            </div>
            <div class="field">
                <label><input type="checkbox" checked="true"/> Remember login session</label>
            </div>
            <div class="field">
                <label><input type="radio" checked="true"/> Enable High Performance JIT</label>
            </div>
            <div class="field">
                <label>Rendering Engine:</label>
                <select>
                    <option selected="true">Vector Anti-Aliasing (tiny-skia)</option>
                    <option>Standard Mode</option>
                </select>
            </div>
            <div class="field">
                <button onclick="alert('Submitted successfully!')">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="#ffffff">
                        <path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z"/>
                    </svg>
                    Submit Form
                </button>
                <button class="btn-icon">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="#ffffff">
                        <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 14z"/>
                    </svg>
                    Search
                </button>
            </div>
        </div>
    </body>
    </html>
    "##.to_string()
}

fn tables_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Table Formatting Model - Mango</title>
        <style>
            body { background-color: #f8fafc; color: #0f172a; font-family: sans-serif; margin: 24px; }
            .card { background-color: #ffffff; border: 1px solid #e2e8f0; border-radius: 8px; padding: 20px; }
            table { width: 100%; border: 1px solid #cbd5e1; margin-top: 12px; }
            th { background-color: #f1f5f9; border-bottom: 2px solid #cbd5e1; padding: 8px 12px; font-weight: bold; }
            td { border-bottom: 1px solid #e2e8f0; padding: 8px 12px; }
        </style>
    </head>
    <body>
        <div class="card">
            <h1>CSS Table Formatting Model</h1>
            <table>
                <tr>
                    <th>Component</th>
                    <th>Implementation</th>
                    <th>Status</th>
                </tr>
                <tr>
                    <td>HTML5 Parser</td>
                    <td>Pure Rust Tokenizer & Tree Builder</td>
                    <td>100% Passed</td>
                </tr>
                <tr>
                    <td>CSS3 Cascade & Selector</td>
                    <td>Specificity + Property Expansion</td>
                    <td>100% Passed</td>
                </tr>
                <tr>
                    <td>JavaScript Engine</td>
                    <td>Boa Engine + Web API Polyfills</td>
                    <td>100% Passed</td>
                </tr>
                <tr>
                    <td>Table Layout</td>
                    <td>Proportional Column Sizing</td>
                    <td>100% Passed</td>
                </tr>
            </table>
        </div>
    </body>
    </html>
    "#.to_string()
}

fn css_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>CSS Enhancements - Mango</title>
        <style>
            body { background-color: #f8fafc; color: #1e293b; font-family: sans-serif; margin: 24px; }
            .rounded-card { background-color: #ffffff; border: 2px solid #ffa136; border-radius: 12px; padding: 18px; margin-bottom: 16px; }
            .opacity-box { background-color: #0284c7; color: #ffffff; opacity: 0.8; padding: 12px; border-radius: 6px; }
            ol { list-style-type: decimal; }
            ul { list-style-type: square; }
        </style>
    </head>
    <body>
        <div class="rounded-card">
            <h1>CSS Enhancements & Styling</h1>
            <p>This card demonstrates border-radius: 12px with custom orange borders.</p>
            <div class="opacity-box">
                Opacity 80% Alpha Blended Overlay
            </div>
            <h3>Ordered List:</h3>
            <ol>
                <li>Item Alpha</li>
                <li>Item Beta</li>
                <li>Item Gamma</li>
            </ol>
            <h3>Unordered List:</h3>
            <ul>
                <li>Bullet One</li>
                <li>Bullet Two</li>
            </ul>
        </div>
    </body>
    </html>
    "#.to_string()
}

fn box_model_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Box Model Test</title>
        <style>
            body { background-color: #ffffff; margin: 16px; }
            h1 { font-size: 24px; color: #ffa136; margin: 0px 0px 12px 0px; }
            .box1 {
                background-color: #ffe0b2;
                padding: 12px;
                margin: 10px 0px 25px 0px;
                border-top-width: 2px;
                border-bottom-width: 2px;
                border-top-color: #ffa136;
                border-bottom-color: #ffa136;
            }
            .box2 {
                background-color: #e0f2fe;
                padding: 12px;
                margin: 25px 0px 10px 0px;
                border-top-width: 2px;
                border-bottom-width: 2px;
                border-top-color: #0284c7;
                border-bottom-color: #0284c7;
            }
            p { font-size: 14px; margin: 4px 0px; }
        </style>
    </head>
    <body>
        <h1>Box Model & Margin Collapsing</h1>
        <div class="box1">
            <p>Box 1: margin-bottom is 25px, padding is 12px.</p>
        </div>
        <div class="box2">
            <p>Box 2: margin-top is 25px. Sibling margins collapse to 25px.</p>
        </div>
    </body>
    </html>
    "#
    .to_string()
}

fn inline_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Inline Formatting Test</title>
        <style>
            body { background-color: #ffffff; margin: 20px; }
            h1 { font-size: 22px; color: #1e1e1e; margin: 0px 0px 12px 0px; }
            p { font-size: 14px; margin: 10px 0px; color: #333333; }
            .highlight { color: #ffa136; }
            .blue { color: #0000ee; }
        </style>
    </head>
    <body>
        <h1>Inline Formatting Context</h1>
        <p>This paragraph demonstrates automatic line breaking and inline formatting in Mango.</p>
        <p>Text wraps naturally across lines based on available container width.</p>
        <p>Different styled spans like <span class="highlight">orange highlight</span> and <span class="blue">blue links</span> are rendered smoothly.</p>
    </body>
    </html>
    "#.to_string()
}

fn fonts_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Font Rendering & Text Styles - Mango</title>
        <style>
            body { background-color: #ffffff; margin: 24px; color: #1e1e1e; font-family: sans-serif; }
            h1 { font-size: 26px; color: #ffa136; margin: 0px 0px 8px 0px; }
            h2 { font-size: 18px; color: #1e293b; margin: 16px 0px 6px 0px; }
            p { font-size: 14px; margin: 6px 0px; color: #334155; }
            .italic { font-style: italic; }
            .oblique { font-style: oblique; }
            .bold { font-weight: bold; }
            .bold-italic { font-weight: bold; font-style: italic; }
            .underline { text-decoration: underline; }
            .line-through { text-decoration: line-through; }
            .overline { text-decoration: overline; }
            .serif { font-family: serif; }
            .mono { font-family: monospace; }
            .box {
                background-color: #f8fafc;
                padding: 14px;
                margin: 10px 0px;
                border-left-width: 4px;
                border-left-color: #ffa136;
                border-top-width: 1px;
                border-right-width: 1px;
                border-bottom-width: 1px;
                border-top-color: #e2e8f0;
                border-right-color: #e2e8f0;
                border-bottom-color: #e2e8f0;
                border-radius: 4px;
            }
        </style>
    </head>
    <body>
        <h1>Font Styles & Text Decorations</h1>
        <p>Pure Rust font rendering with fontdue glyph rasterization, faux-italic shearing, and decorative strokes.</p>
        
        <h2>1. Font Styles (Italic & Oblique)</h2>
        <div class="box">
            <p>Normal style: The quick brown fox jumps over the lazy dog.</p>
            <p class="italic">Italic style: The quick brown fox jumps over the lazy dog.</p>
            <p class="oblique">Oblique style: The quick brown fox jumps over the lazy dog.</p>
            <p class="bold-italic">Bold + Italic: The quick brown fox jumps over the lazy dog.</p>
        </div>

        <h2>2. Text Decorations</h2>
        <div class="box">
            <p class="underline">Underline: Important links and highlighted headings.</p>
            <p class="line-through">Line-Through: Discounted price from $99 down to $49.</p>
            <p class="overline">Overline: Mathematical notation or decorative headers.</p>
        </div>

        <h2>3. Font Families</h2>
        <div class="box">
            <p>Sans-Serif: Modern clean typography for UI elements.</p>
            <p class="serif">Serif: Classic editorial typography for literature and articles.</p>
            <p class="mono">Monospace: fn render_svg() -> Result&lt;Pixmap, Error&gt;</p>
        </div>
    </body>
    </html>
    "#.to_string()
}

fn svg_test_html() -> String {
    r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Vector SVG Rendering - Mango</title>
        <style>
            body { background-color: #f8fafc; color: #1e293b; font-family: sans-serif; margin: 24px; }
            .card { background-color: #ffffff; border: 1px solid #e2e8f0; border-radius: 8px; padding: 20px; margin-bottom: 18px; }
            h1 { font-size: 24px; color: #ffa136; margin-top: 0; }
            h2 { font-size: 18px; color: #334155; margin-top: 14px; margin-bottom: 8px; }
            p { font-size: 13px; color: #475569; margin: 4px 0px; }
            .icon-row { margin: 12px 0; }
            button { background-color: #ffa136; color: #ffffff; border: 1px solid #d97706; border-radius: 4px; padding: 6px 14px; font-weight: bold; }
        </style>
    </head>
    <body>
        <div class="card">
            <h1>Pure Rust Vector SVG Rendering (tiny-skia)</h1>
            <p>Mango dynamically parses and rasterizes inline vector SVG elements with anti-aliasing.</p>

            <h2>1. Common Vector Icons</h2>
            <div class="icon-row">
                <!-- Search Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#0284c7">
                    <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 14z"/>
                </svg>
                <!-- Checkmark Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#10b981">
                    <path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z"/>
                </svg>
                <!-- Star Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#f59e0b">
                    <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                </svg>
                <!-- Home Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#6366f1">
                    <path d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/>
                </svg>
            </div>

            <h2>2. Basic SVG Shapes</h2>
            <div class="icon-row">
                <svg width="120" height="40" viewBox="0 0 120 40">
                    <rect x="5" y="5" width="30" height="30" fill="#3b82f6" rx="4"/>
                    <circle cx="60" cy="20" r="15" fill="#ef4444"/>
                    <line x1="85" y1="35" x2="115" y2="5" stroke="#10b981" stroke-width="4"/>
                </svg>
            </div>

            <h2>3. SVG Icons Nested in Buttons</h2>
            <div class="icon-row">
                <button>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="#ffffff">
                        <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
                    </svg>
                    New Item
                </button>
            </div>
        </div>
    </body>
    </html>
    "##.to_string()
}

fn image_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Image Rendering Test</title>
        <style>
            body { background-color: #ffffff; margin: 20px; color: #1e1e1e; }
            h1 { font-size: 24px; color: #ffa136; margin: 0px 0px 10px 0px; }
            p { font-size: 14px; margin: 8px 0px; color: #333333; }
            .card {
                background-color: #f8fafc;
                padding: 14px;
                margin: 10px 0px;
                border-top-width: 1px;
                border-right-width: 1px;
                border-bottom-width: 1px;
                border-left-width: 1px;
                border-top-color: #e2e8f0;
                border-right-color: #e2e8f0;
                border-bottom-color: #e2e8f0;
                border-left-color: #e2e8f0;
            }
        </style>
    </head>
    <body>
        <h1>Image Decoding & Rendering</h1>
        <p>Supports PNG, JPEG, GIF, and WebP via data: URIs and graceful placeholder fallback.</p>

        <div class="card">
            <p><b>1. Valid Inline Base64 PNG Image (Scaled 48x48):</b></p>
            <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==" width="48" height="48">
        </div>

        <div class="card">
            <p><b>2. Missing Image Graceful Fallback Placeholder:</b></p>
            <img src="https://example.com/network_image.png" width="64" height="64">
        </div>
    </body>
    </html>
    "#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_initialization() {
        let browser = BrowserChrome::new(1024, 768);
        assert_eq!(browser.tabs.len(), 1);
        assert_eq!(browser.active_tab_idx, 0);
        assert_eq!(browser.address_text, "about:welcome");
        assert!(browser.js_runtime.is_some());
    }

    #[test]
    fn test_multi_tab_operations() {
        let mut browser = BrowserChrome::new(1024, 768);

        // Add second tab
        browser.new_tab();
        assert_eq!(browser.tabs.len(), 2);
        assert_eq!(browser.active_tab_idx, 1);

        // Switch back to tab 0
        browser.switch_tab(0);
        assert_eq!(browser.active_tab_idx, 0);

        // Switch to next tab
        browser.next_tab();
        assert_eq!(browser.active_tab_idx, 1);

        // Close tab 1
        browser.close_tab(1);
        assert_eq!(browser.tabs.len(), 1);
        assert_eq!(browser.active_tab_idx, 0);
    }

    #[test]
    fn test_navigation_history_back_forward() {
        let mut browser = BrowserChrome::new(1024, 768);

        // Initially cannot go back or forward
        assert!(!browser.tabs[browser.active_tab_idx].can_go_back());
        assert!(!browser.tabs[browser.active_tab_idx].can_go_forward());

        // Navigate to test:box
        browser.navigate("test:box");
        assert_eq!(browser.address_text, "test:box");
        assert!(browser.tabs[browser.active_tab_idx].can_go_back());
        assert!(!browser.tabs[browser.active_tab_idx].can_go_forward());

        // Navigate to test:fonts
        browser.navigate("test:fonts");
        assert_eq!(browser.address_text, "test:fonts");

        // Go back
        browser.go_back();
        assert_eq!(browser.address_text, "test:box");
        assert!(browser.tabs[browser.active_tab_idx].can_go_forward());

        // Go forward
        browser.go_forward();
        assert_eq!(browser.address_text, "test:fonts");
    }

    #[test]
    fn test_inline_script_execution_in_browser() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body>
                <h1 id="dyn-title">Initial Title</h1>
                <script>
                    var el = document.getElementById("dyn-title");
                    el.textContent = "Live Script Output";
                </script>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:js".to_string());

        // Check that JS runtime executed the script and modified the live DOM
        let _root = browser.root_box.as_ref().expect("root box should exist");
        let dl = browser.build_display_list((1024, 768));
        let found_text = dl.iter().any(|cmd| {
            if let DisplayCommand::DrawText { text, .. } = cmd {
                text == "Live Script Output"
            } else {
                false
            }
        });
        assert!(
            found_text,
            "Rendered display list should contain text mutated by JS"
        );
    }

    #[test]
    fn test_ui_button_clicks() {
        let mut browser = BrowserChrome::new(1024, 768);

        // Hover & click New Tab (+)
        browser.hovered_target = Some(HoverTarget::NewTab);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.tabs.len(), 2);

        // Hover & click Home button
        browser.hovered_target = Some(HoverTarget::Home);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.address_text, "about:welcome");

        // Hover & click Tab Close (1)
        browser.hovered_target = Some(HoverTarget::TabClose(1));
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.tabs.len(), 1);
    }

    #[test]
    fn test_scroll_clamping() {
        let mut browser = BrowserChrome::new(1024, 768);
        // Scroll down
        browser.handle_scroll(-5.0);
        let s1 = browser.scroll_y;
        assert!(s1 >= 0.0);

        // Scroll way up (should clamp at 0.0)
        browser.handle_scroll(100.0);
        assert_eq!(browser.scroll_y, 0.0);
    }

    #[test]
    fn test_context_menu_open_and_click() {
        let mut browser = BrowserChrome::new(1024, 768);
        assert!(browser.context_menu.is_none());

        // Right click at (200, 300)
        browser.handle_mouse_move(200.0, 300.0);
        browser.handle_mouse_click(MouseButton::Right, KeyState::Pressed);

        assert!(browser.context_menu.is_some());
        let menu = browser.context_menu.as_ref().unwrap();
        assert!(menu.items.iter().any(|item| item.label == "Reload"));
        assert!(
            menu.items
                .iter()
                .any(|item| item.label == "View Page Source")
        );

        // Render display list with open context menu
        let dl = browser.build_display_list((1024, 768));
        let has_menu_text = dl.iter().any(|cmd| {
            if let DisplayCommand::DrawText { text, .. } = cmd {
                text == "Reload"
            } else {
                false
            }
        });
        assert!(has_menu_text, "Display list must render context menu items");

        // Left click outside dismisses context menu
        browser.handle_mouse_move(50.0, 50.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.context_menu.is_none());
    }

    #[test]
    fn test_interactive_form_controls() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body>
                <input type="text" id="username" value="hello" style="width: 200px; height: 30px; display: block;"/>
                <input type="checkbox" id="agree" style="width: 20px; height: 20px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:form_int".to_string());

        // Hit-test text input
        let hit = browser
            .root_box
            .as_ref()
            .and_then(|r| r.hit_test_form_control(mango_core::Point::new(10.0, 10.0)))
            .expect("should hit text input");
        assert_eq!(hit.tag_name, "input");
        assert_eq!(hit.form_type, "text");

        // Click text input to focus (click past "hello" to place cursor at end)
        browser.handle_mouse_move(100.0, 10.0 + HEADER_HEIGHT);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.focused_control.is_some());

        // Type character '!' into input
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('!'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });

        let val = browser
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(hit.node_id.unwrap(), "value"))
            .unwrap();
        assert_eq!(val, "hello!");

        // Backspace
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Backspace,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        let val2 = browser
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(hit.node_id.unwrap(), "value"))
            .unwrap();
        assert_eq!(val2, "hello");
    }

    #[test]
    fn test_svg_rendering_in_browser() {
        let mut browser = BrowserChrome::new(1024, 768);
        browser.navigate("test:svg");
        assert_eq!(browser.address_text, "test:svg");

        let dl = browser.build_display_list((1024, 768));
        let has_svg_images = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
        assert!(
            has_svg_images,
            "Display list should contain rasterized SVG images"
        );
    }

    #[test]
    fn test_font_styles_and_decorations_in_browser() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body>
                <p style="font-style: italic; text-decoration: underline;">Italic and Underlined</p>
                <p style="text-decoration: line-through;">Strikethrough</p>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:styled_fonts".to_string());

        let dl = browser.build_display_list((1024, 768));
        let found_italic_underline = dl.iter().any(|cmd| {
            if let DisplayCommand::DrawText {
                text,
                style,
                decoration,
                ..
            } = cmd
            {
                text == "Italic and Underlined"
                    && *style == mango_render::FontStyle::Italic
                    && *decoration == mango_render::TextDecoration::Underline
            } else {
                false
            }
        });
        assert!(
            found_italic_underline,
            "Should render text with Italic style and Underline decoration"
        );

        let found_line_through = dl.iter().any(|cmd| {
            if let DisplayCommand::DrawText {
                text, decoration, ..
            } = cmd
            {
                text == "Strikethrough" && *decoration == mango_render::TextDecoration::LineThrough
            } else {
                false
            }
        });
        assert!(
            found_line_through,
            "Should render text with LineThrough decoration"
        );
    }

    #[test]
    fn test_button_with_svg_icon_and_centering() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r##"
            <html>
            <body>
                <button style="height: 36px;">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="#000000">
                        <circle cx="12" cy="12" r="10"/>
                    </svg>
                    <span>Click Me</span>
                </button>
            </body>
            </html>
        "##;
        browser.load_html(html.to_string(), "test:btn_icon".to_string());

        let dl = browser.build_display_list((1024, 768));
        let has_svg = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
        let has_text = dl.iter().any(|cmd| {
            if let DisplayCommand::DrawText { text, .. } = cmd {
                text.contains("Click Me")
            } else {
                false
            }
        });
        assert!(has_svg, "Button should render SVG icon image");
        assert!(has_text, "Button should render text label");
    }

    #[test]
    fn test_scrollable_extent_deep_scan() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html style="height: 100%;">
            <body style="height: 100%; margin: 0;">
                <div style="height: 500px; background: red;">Top Box</div>
                <div style="height: 1500px; background: green;">Middle Box</div>
                <div style="height: 1000px; background: blue;">Bottom Box</div>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:extent".to_string());

        let doc_h = browser.scrollable_height();
        assert!(
            doc_h >= 3000.0,
            "Document extent must reflect all 3000px of child boxes, got {}",
            doc_h
        );
        assert!(
            browser.max_scroll() > 2000.0,
            "Max scroll must be > 2000px, got {}",
            browser.max_scroll()
        );
    }

    #[test]
    fn test_viewport_scissor_clipping() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body style="margin: 0;">
                <div style="height: 400px; background: red;">Box 1</div>
                <div style="height: 400px; background: green;">Box 2</div>
                <div style="height: 400px; background: blue;">Box 3</div>
                <div style="height: 400px; background: yellow;">Box 4</div>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:clipping".to_string());

        // Scroll down by 500px
        browser.handle_scroll(-500.0 / 45.0);
        assert!((browser.scroll_y - 500.0).abs() < 1.0);

        let dl = browser.build_display_list((1024, 768));
        let content_y = HEADER_HEIGHT + 1.0;
        let content_h = (768.0 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);

        // Verify that no page-related command extends above content_y or below content_y + content_h
        for cmd in dl.iter() {
            match cmd {
                DisplayCommand::FillRect { rect, .. }
                | DisplayCommand::FillRoundedRect { rect, .. } => {
                    // Chrome elements (tab bar at y=0..38, toolbar at 38..78, status bar at 744..768) are allowed
                    if rect.y() >= content_y && rect.y() < content_y + content_h {
                        assert!(
                            rect.y() >= content_y - 0.1,
                            "Rect top {:.1} must not bleed above content_y {:.1}",
                            rect.y(),
                            content_y
                        );
                        assert!(
                            rect.bottom() <= content_y + content_h + 0.1,
                            "Rect bottom {:.1} must not bleed below content bottom {:.1}",
                            rect.bottom(),
                            content_y + content_h
                        );
                    }
                }
                DisplayCommand::DrawText { y, .. }
                    if *y >= content_y && *y <= content_y + content_h =>
                {
                    assert!(*y >= content_y && *y <= content_y + content_h);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_scrollbar_rendering_and_interaction() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body style="height: 3000px; margin: 0;">
                <p>Long page</p>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:scrollbar".to_string());

        assert!(browser.max_scroll() > 0.0);

        // Verify scrollbar thumb is in display list
        let dl = browser.build_display_list((1024, 768));
        let scrollbar_w = 12.0f32;
        let has_thumb = dl.iter().any(|cmd| {
            if let DisplayCommand::FillRoundedRect { rect, radii, .. } = cmd {
                (rect.x() - (1024.0 - scrollbar_w + 2.0)).abs() < 2.0
                    && *radii == [4.0, 4.0, 4.0, 4.0]
            } else {
                false
            }
        });
        assert!(
            has_thumb,
            "Display list must render rounded scrollbar thumb"
        );

        // Hover over scrollbar thumb
        let content_y_top = HEADER_HEIGHT + 1.0;
        browser.handle_mouse_move(1020.0, content_y_top + 10.0);
        assert_eq!(browser.hovered_target, Some(HoverTarget::ScrollbarThumb));

        // Click and drag thumb down by 100px
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.scrollbar_dragging);
        browser.handle_mouse_move(1020.0, content_y_top + 110.0);
        assert!(
            browser.scroll_y > 0.0,
            "Dragging scrollbar thumb down must scroll page down"
        );

        // Release mouse click
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);
        assert!(!browser.scrollbar_dragging);

        // Click track near bottom jumps scroll
        browser.handle_mouse_move(1020.0, 700.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(
            browser.scroll_y > 1000.0,
            "Clicking bottom of scrollbar track must jump scroll position"
        );
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);
    }

    #[test]
    fn test_keyboard_page_navigation() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body style="height: 4000px; margin: 0;">
                <p>Very long page</p>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:keyboard_scroll".to_string());
        assert_eq!(browser.scroll_y, 0.0);

        // PageDown
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::PageDown,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        let scrolled_pagedown = browser.scroll_y;
        assert!(
            scrolled_pagedown > 400.0,
            "PageDown must scroll ~85% of viewport height"
        );

        // Space
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Space,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert!(
            browser.scroll_y > scrolled_pagedown,
            "Space must scroll down"
        );

        // Shift+Space
        let shift_mods = mango_platform::input::Modifiers {
            shift: true,
            ..Default::default()
        };
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Space,
            state: KeyState::Pressed,
            modifiers: shift_mods,
        });
        assert!(
            (browser.scroll_y - scrolled_pagedown).abs() < 2.0,
            "Shift+Space must scroll up"
        );

        // End
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::End,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(
            browser.scroll_y,
            browser.max_scroll(),
            "End must scroll to bottom"
        );

        // Home
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Home,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(browser.scroll_y, 0.0, "Home must scroll to top");
    }

    #[test]
    fn test_select_dropdown_open_and_select() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <select id="engine">
                    <option value="skia" selected="true">TinySkia</option>
                    <option value="sw">Software</option>
                </select>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:select".to_string());
        assert!(browser.select_dropdown.is_none());

        // Click select element
        browser.handle_mouse_move(30.0, HEADER_HEIGHT + 30.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        assert!(
            browser.select_dropdown.is_some(),
            "Clicking select must open select_dropdown"
        );
        let dropdown = browser.select_dropdown.as_ref().unwrap();
        assert_eq!(dropdown.options.len(), 2);
        assert_eq!(dropdown.options[0].text, "TinySkia");
        assert_eq!(dropdown.options[1].text, "Software");

        // Click second option
        let opt2_y = dropdown.y + 4.0 + dropdown.item_height + 4.0;
        browser.handle_mouse_move(dropdown.x + 20.0, opt2_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        assert!(
            browser.select_dropdown.is_none(),
            "Selecting option must close dropdown"
        );
        assert_eq!(browser.status_text, "Selected: Software");
    }

    #[test]
    fn test_details_summary_toggle() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <details id="my-details">
                    <summary id="my-summary">Advanced Settings</summary>
                    <p>Details content</p>
                </details>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:details".to_string());

        // Click summary to expand
        browser.handle_mouse_move(30.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let details_id = find_element_by_id(&doc, "my-details").unwrap();
        let el = match &doc.get(details_id).unwrap().data {
            NodeData::Element(e) => e.clone(),
            _ => panic!("Expected element"),
        };
        assert!(
            el.get_attribute("open").is_some(),
            "Clicking summary must open details"
        );

        // Click summary again to collapse
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let doc2 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let el2 = match &doc2.get(details_id).unwrap().data {
            NodeData::Element(e) => e.clone(),
            _ => panic!("Expected element"),
        };
        assert!(
            el2.get_attribute("open").is_none(),
            "Clicking summary again must close details"
        );
    }

    #[test]
    fn test_label_for_checkbox_toggle() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="checkbox" id="vector-dropdown-checkbox"/>
                <label for="vector-dropdown-checkbox">Main Menu</label>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:label".to_string());

        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let cb_id = find_element_by_id(&doc, "vector-dropdown-checkbox").unwrap();
        let is_checked = match &doc.get(cb_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("checked").is_some(),
            _ => false,
        };
        assert!(!is_checked, "Initially unchecked");

        // Click label
        browser.handle_mouse_move(45.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        let doc2 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let is_checked2 = match &doc2.get(cb_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("checked").is_some(),
            _ => false,
        };
        assert!(
            is_checked2,
            "Clicking label must toggle checkbox to checked"
        );
    }

    #[test]
    fn test_text_input_cursor_navigation_and_insert() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="text" id="q" value="hello"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:input".to_string());

        // Focus input at end of text (x=100.0 is inside input, past "hello")
        browser.handle_mouse_move(100.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.focused_control.is_some());
        assert_eq!(browser.control_cursor_pos, 5);

        // Click near the start (x=24.0) to reposition caret at beginning
        browser.handle_mouse_move(24.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.control_cursor_pos, 0);

        // Click again past the text (x=100.0) to place cursor back at end
        browser.handle_mouse_move(100.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.control_cursor_pos, 5);

        // Move cursor 2 characters left
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowLeft,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowLeft,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(browser.control_cursor_pos, 3);

        // Type 'X' at position 3 ("hel" + "X" + "lo" -> "helXlo")
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('X'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });

        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let input_id = find_element_by_id(&doc, "q").unwrap();
        let val = match &doc.get(input_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("value").unwrap_or("").to_string(),
            _ => "".to_string(),
        };
        assert_eq!(val, "helXlo");
    }

    #[test]
    fn test_video_and_audio_browser_interaction() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 0;">
                <video id="player" src="video.mp4" controls width="300" height="150" style="display: block; margin: 0;"></video>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:media".to_string());

        // 1. Hover over the video controls play button (x=15, y = HEADER_HEIGHT + 135)
        let hovered = browser.handle_mouse_move(15.0, HEADER_HEIGHT + 135.0);
        assert!(
            hovered || browser.status_text.starts_with("Media <video>"),
            "Hovering video controls should update status text"
        );
        assert!(
            browser.status_text.contains("Media <video>"),
            "Status should indicate video media"
        );

        // 2. Left click play button -> toggles playing
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.status_text, "Video playing");
        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let vid_id = find_element_by_id(&doc, "player").unwrap();
        let is_playing = match &doc.get(vid_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("data-mango-playing") == Some("true"),
            _ => false,
        };
        assert!(
            is_playing,
            "Clicking play button should set data-mango-playing to true"
        );

        // 3. Left click play button again -> toggles paused
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.status_text, "Video paused");
        let doc2 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let is_paused = match &doc2.get(vid_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("data-mango-playing") == Some("false"),
            _ => false,
        };
        assert!(
            is_paused,
            "Clicking play button again should set data-mango-playing to false"
        );

        // 4. Right click -> opens context menu with Play and Mute actions
        browser.handle_mouse_move(50.0, HEADER_HEIGHT + 50.0);
        browser.handle_mouse_click(MouseButton::Right, KeyState::Pressed);
        assert!(
            browser.context_menu.is_some(),
            "Right click on video must open context menu"
        );
        let menu = browser.context_menu.as_ref().unwrap();
        assert!(
            menu.items
                .iter()
                .any(|i| i.label == "Play" || i.label == "Pause")
        );
        assert!(
            menu.items
                .iter()
                .any(|i| i.label == "Mute" || i.label == "Unmute")
        );

        // 5. Click Mute action on context menu
        let mute_idx = menu.items.iter().position(|i| i.label == "Mute").unwrap();
        let item_y = menu.y + 4.0 + (mute_idx as f32) * 26.0 + 10.0;
        browser.handle_mouse_move(menu.x + 20.0, item_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.status_text, "Media muted");
        let doc3 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let is_muted = match &doc3.get(vid_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("data-mango-muted") == Some("true"),
            _ => false,
        };
        assert!(
            is_muted,
            "Activating Mute from context menu should set data-mango-muted to true"
        );
    }

    #[test]
    fn test_stylesheet_image_collection_is_not_capped_at_64() {
        // OPT-009: `collect_stylesheet_images` used to hard-stop at 64 URLs,
        // silently dropping images on image-heavy pages.
        let doc = parse_html("<html><body><div class=\"a\"></div></body></html>");
        let mut css = String::new();
        for i in 0..120 {
            css.push_str(&format!(
                ".a {{ background-image: url(https://img.example/bg{i}.png); }}\n"
            ));
        }
        let sheet = parse_stylesheet(&css);
        let mut sources = Vec::new();
        collect_stylesheet_images(&doc, std::slice::from_ref(&sheet), &mut sources);
        assert_eq!(
            sources.len(),
            120,
            "every background image must be collected (was capped at 64)"
        );
    }

    #[test]
    fn test_background_image_backlog_drains_a_few_per_frame() {
        let mut browser = BrowserChrome::new(1024, 768);
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        browser.pending_image_fetches = (0..10).map(|_| png.to_string()).collect();
        browser.image_fetch_base = Some(Url::parse("https://example.com/").unwrap());
        assert!(browser.has_pending_image_fetches());

        assert!(
            browser.drain_pending_images(),
            "first drain caches a new image and should trigger a repaint"
        );
        assert_eq!(
            browser.pending_image_fetches.len(),
            10 - IMAGE_PREFETCH_PER_FRAME,
            "exactly a few images per frame"
        );
        assert!(get_cached_image(png).is_some(), "drained image was cached");

        while browser.has_pending_image_fetches() {
            browser.drain_pending_images();
        }
        assert!(
            !browser.has_pending_image_fetches(),
            "the backlog empties rather than being dropped"
        );
    }

    #[test]
    fn test_local_storage_profile_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mango_ls_{}", std::process::id()));
        let path = dir.join("localStorage.txt");
        let _ = std::fs::remove_file(&path);

        let store: mango_js::web_apis::SharedLocalStorage =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        store
            .lock()
            .unwrap()
            .insert("theme\tname".to_string(), "dark\nvalue".to_string());
        store.lock().unwrap().insert(
            "https://example.com\x1fuser_token".to_string(),
            "token_12345".to_string(),
        );
        save_local_storage(&path, &store);
        assert!(path.exists(), "profile file was written");

        let restored: mango_js::web_apis::SharedLocalStorage =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        load_local_storage(&path, &restored);
        assert_eq!(
            restored
                .lock()
                .unwrap()
                .get("theme\tname")
                .map(|s| s.as_str()),
            Some("dark\nvalue"),
            "tabs/newlines inside values survive the roundtrip"
        );
        assert_eq!(
            restored
                .lock()
                .unwrap()
                .get("https://example.com\x1fuser_token")
                .map(|s| s.as_str()),
            Some("token_12345"),
            "origin-scoped keys survive the roundtrip"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── GAP-024: <input type="range/number/color/date/time/file"> controls ──

    /// Helper: reads a node attribute from the live JS runtime.
    fn attr(browser: &BrowserChrome, node_id: NodeId, name: &str) -> Option<String> {
        browser
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, name))
    }

    /// Helper: hit-tests the first form control at a document-space point.
    fn hit_control(browser: &BrowserChrome, x: f32, y: f32) -> mango_layout::FormControlHit {
        browser
            .root_box
            .as_ref()
            .and_then(|r| r.hit_test_form_control(mango_core::Point::new(x, y)))
            .expect("expected to hit a form control")
    }

    #[test]
    fn test_range_slider_click_drag_and_keyboard_step() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="range" id="vol" min="0" max="100" value="0" style="width: 200px; height: 24px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:range".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "range");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, cw, ch) = browser.control_content_rect(node_id).expect("range box");

        // Click at 75% across the track.
        browser.handle_mouse_move(cx + cw * 0.75, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("75"));
        assert_eq!(
            attr(&browser, node_id, "data-mango-value").as_deref(),
            Some("75"),
            "JS-facing data-mango-value mirrors the value attribute"
        );
        assert_eq!(
            browser.focused_control,
            Some(node_id),
            "slider takes keyboard focus so arrows can step it"
        );
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);

        // ArrowUp steps by `step` (default 1).
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowUp,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("76"));

        // Drag: press at 50%, move to 25%, release.
        browser.handle_mouse_move(cx + cw * 0.5, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("50"));
        browser.handle_mouse_move(cx + cw * 0.25, HEADER_HEIGHT + cy + ch / 2.0);
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some("25"),
            "dragging the thumb follows the mouse"
        );
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);

        // Rendering: the slider paints its blue thumb (GAP-024 paint check).
        let has_thumb = browser
            .build_display_list((800, 600))
            .into_iter()
            .any(|c| matches!(c, DisplayCommand::FillRoundedRect { color, .. } if color == Color::rgb(0, 120, 215)));
        assert!(has_thumb, "range slider thumb must be painted");
    }

    #[test]
    fn test_number_spinner_clicks_and_arrow_steps() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="number" id="n" value="4" min="0" max="10" step="2" style="width: 100px; height: 30px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:number".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "number");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, cw, ch) = browser.control_content_rect(node_id).expect("number box");

        // Up spinner (right edge, top half): 4 → 6.
        browser.handle_mouse_move(cx + cw - 8.0, HEADER_HEIGHT + cy + 4.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("6"));

        // Down spinner: 6 → 4.
        browser.handle_mouse_move(cx + cw - 8.0, HEADER_HEIGHT + cy + ch - 4.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("4"));

        // Click the text area to focus, then ArrowUp steps 4 → 6.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.focused_control, Some(node_id));
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowUp,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("6"));

        // Spin up to the max: 6 → 8 → 10 → clamped at 10.
        browser.handle_mouse_move(cx + cw - 8.0, HEADER_HEIGHT + cy + 4.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("8"));
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("10"));
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some("10"),
            "value clamps at max"
        );

        // Rendering: spinner arrows are painted on the field.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "▲"), "up arrow painted");
        assert!(texts.iter().any(|t| t == "▼"), "down arrow painted");
    }

    #[test]
    fn test_color_picker_opens_and_picks_swatch() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r##"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="color" id="c" value="#000000" style="width: 120px; height: 26px; display: block;"/>
            </body>
            </html>
        "##;
        browser.load_html(html.to_string(), "test:color".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "color");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("color box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("color picker opens");
            assert!(matches!(picker.kind, PickerKind::Color));
            (picker.x, picker.y)
        };

        // The swatch hex label is painted inside the field.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "#000000"), "hex label painted");

        // Swatch index 6 is "#ff0000" (row 0, column 6).
        let sx = px
            + PICKER_SWATCH_PAD
            + 6.0 * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP)
            + PICKER_SWATCH_CELL / 2.0;
        let sy = py + PICKER_SWATCH_PAD + PICKER_SWATCH_CELL / 2.0;
        browser.handle_mouse_move(sx, sy);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        assert!(browser.picker.is_none(), "picking closes the palette");
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("#ff0000"));
        assert_eq!(browser.status_text, "Color: #FF0000");
    }

    #[test]
    fn test_date_picker_calendar_navigation_and_selection() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="date" id="d" value="2026-03-10" style="width: 160px; height: 26px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:date".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "date");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("date box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("date picker opens");
            match picker.kind {
                PickerKind::Date { year, month } => assert_eq!((year, month), (2026, 3)),
                ref other => panic!("expected Date picker, got {:?}", other),
            }
            (picker.x, picker.y)
        };
        assert_eq!(browser.status_text, "Pick a date");

        // The calendar header shows the month/year.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "March 2026"),
            "month header painted"
        );
        assert!(texts.iter().any(|t| t == "15"), "day cell painted");

        // Click day 15 (March 2026 starts on a Sunday).
        let col_w = (210.0 - PICKER_DATE_PAD * 2.0) / 7.0;
        let lead = weekday_sunday0(days_from_civil(2026, 3, 1));
        let idx = lead + 15 - 1;
        let dx = px + PICKER_DATE_PAD + (idx % 7) as f32 * col_w + col_w / 2.0;
        let dy = py + PICKER_DATE_GRID_Y + (idx / 7) as f32 * PICKER_DATE_ROW_H + 10.0;
        browser.handle_mouse_move(dx, dy);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(
            browser.picker.is_none(),
            "picking a day closes the calendar"
        );
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some("2026-03-15")
        );
        assert_eq!(browser.status_text, "Date: 2026-03-15");

        // Reopen and navigate to the next month with the ">" header button.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("date picker reopens");
            (picker.x, picker.y)
        };
        browser.handle_mouse_move(px + 210.0 - 30.0 + 13.0, py + PICKER_DATE_HEADER_Y + 11.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser.picker.as_ref().expect("still open").kind {
            PickerKind::Date { year, month } => assert_eq!((*year, *month), (2026, 4)),
            other => panic!("expected Date picker, got {:?}", other),
        }

        // Clicking outside the panel dismisses it.
        browser.handle_mouse_move(10.0, HEADER_HEIGHT + 10.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.picker.is_none(), "outside click closes the picker");
    }

    #[test]
    fn test_time_picker_steppers_and_set() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="time" id="t" value="09:30" style="width: 140px; height: 26px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:time".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "time");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("time box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("time picker opens");
            assert!(matches!(picker.kind, PickerKind::Time { .. }));
            (picker.x, picker.y)
        };

        // Hour "+": 09 → 10.
        browser.handle_mouse_move(px + 123.0, py + PICKER_TIME_HOUR_Y + 12.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser.picker.as_ref().unwrap().kind {
            PickerKind::Time { hour, .. } => assert_eq!(*hour, 10),
            other => panic!("expected Time picker, got {:?}", other),
        }
        // Minute "-": 30 → 29.
        browser.handle_mouse_move(px + 27.0, py + PICKER_TIME_MIN_Y + 12.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser.picker.as_ref().unwrap().kind {
            PickerKind::Time { minute, .. } => assert_eq!(*minute, 29),
            other => panic!("expected Time picker, got {:?}", other),
        }

        // "Set" commits the value.
        browser.handle_mouse_move(px + 75.0, py + PICKER_TIME_SET_Y + 13.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.picker.is_none(), "Set closes the time picker");
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("10:29"));
        assert_eq!(browser.status_text, "Time: 10:29");
    }

    #[test]
    fn test_file_picker_lists_directory_and_selects() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="file" id="f" style="width: 260px; height: 28px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:file".to_string());

        // The unselected field renders a "Choose file..." button.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "Choose file..."),
            "button painted"
        );
        assert!(
            texts.iter().any(|t| t == "No file chosen"),
            "placeholder painted"
        );

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "file");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("file box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (root_dir, entries_len) = {
            let picker = browser.picker.as_ref().expect("file picker opens");
            match &picker.kind {
                PickerKind::File {
                    dir,
                    entries,
                    offset,
                } => {
                    assert!(!entries.is_empty(), "directory listing is populated");
                    assert_eq!(*offset, 0);
                    (dir.clone(), entries.len())
                }
                other => panic!("expected File picker, got {:?}", other),
            }
        };
        assert!(entries_len > 0);
        assert_eq!(browser.status_text, "Choose a file");

        // Pick the first file entry that fits in the visible first screen
        // (row 0 is the ".." parent row, so entry idx n sits at row n + 1).
        let file_pick = match &browser.picker.as_ref().unwrap().kind {
            PickerKind::File { entries, .. } => entries
                .iter()
                .enumerate()
                .find(|(i, e)| !e.is_dir && *i + 1 < PICKER_FILE_ROWS)
                .map(|(i, e)| (i, e.name.clone())),
            _ => None,
        };
        if let Some((idx, name)) = file_pick {
            let (px, py) = {
                let p = browser.picker.as_ref().unwrap();
                (p.x, p.y)
            };
            let row_y = py
                + PICKER_FILE_ROW_Y
                + (idx + 1) as f32 * PICKER_FILE_ROW_H
                + PICKER_FILE_ROW_H / 2.0;
            browser.handle_mouse_move(px + 60.0, row_y);
            browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
            assert!(browser.picker.is_none(), "picking a file closes the picker");
            assert_eq!(
                attr(&browser, node_id, "value").as_deref(),
                Some(name.as_str())
            );
            assert_eq!(
                attr(&browser, node_id, "data-mango-filename").as_deref(),
                Some(name.as_str())
            );
            let path = attr(&browser, node_id, "data-mango-filepath").unwrap();
            assert!(
                path.ends_with(&name),
                "data-mango-filepath points at the chosen file: {path}"
            );
            assert_eq!(browser.status_text, format!("Selected file: {}", name));

            // Reopen to exercise directory navigation.
            browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
            browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
            assert!(browser.picker.is_some(), "picker reopens");
        }

        // Navigate into the first directory (dirs sort first; row 0 is "..").
        let dir_name = match &browser.picker.as_ref().unwrap().kind {
            PickerKind::File { entries, .. } => entries
                .iter()
                .find(|e| e.is_dir)
                .map(|e| e.name.clone())
                .expect("repo contains at least one directory"),
            other => panic!("expected File picker, got {:?}", other),
        };
        let dir_idx = match &browser.picker.as_ref().unwrap().kind {
            PickerKind::File { entries, .. } => {
                entries.iter().position(|e| e.name == dir_name).unwrap()
            }
            _ => unreachable!(),
        };
        let (px, py) = {
            let p = browser.picker.as_ref().unwrap();
            (p.x, p.y)
        };
        let row_y = py
            + PICKER_FILE_ROW_Y
            + (dir_idx + 1) as f32 * PICKER_FILE_ROW_H
            + PICKER_FILE_ROW_H / 2.0;
        browser.handle_mouse_move(px + 60.0, row_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser
            .picker
            .as_ref()
            .expect("still open after entering dir")
            .kind
        {
            PickerKind::File {
                dir,
                entries,
                offset,
            } => {
                assert!(
                    dir.ends_with(&dir_name),
                    "entered {:?}, got {dir:?}",
                    dir_name
                );
                assert_eq!(*offset, 0);
                let _ = entries;
            }
            other => panic!("expected File picker, got {:?}", other),
        }

        // ".." (row 0) navigates back to the parent directory.
        let (px, py) = {
            let p = browser.picker.as_ref().unwrap();
            (p.x, p.y)
        };
        browser.handle_mouse_move(px + 60.0, py + PICKER_FILE_ROW_Y + PICKER_FILE_ROW_H / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser
            .picker
            .as_ref()
            .expect("still open after going up")
            .kind
        {
            PickerKind::File { dir, .. } => assert_eq!(dir, &root_dir, "back at the root"),
            other => panic!("expected File picker, got {:?}", other),
        }
    }

    #[test]
    fn test_input_and_change_events_reach_page_listeners() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="text" id="t" value="a" style="width: 150px; height: 24px; display: block;"/>
                <div id="pad" style="width: 300px; height: 40px; display: block;">pad</div>
                <script>
                    var el = document.getElementById('t');
                    el.addEventListener('input', function() {
                        el.setAttribute('data-hits', String(Number(el.getAttribute('data-hits') || '0') + 1));
                    });
                    el.addEventListener('change', function() {
                        el.setAttribute('data-changed', '1');
                    });
                </script>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:events".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("text box");

        // Focus and type two characters → two `input` events reach the listener.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.focused_control, Some(node_id));
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('b'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('c'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(
            attr(&browser, node_id, "data-hits").as_deref(),
            Some("2"),
            "page listeners received both input events"
        );
        assert_eq!(
            attr(&browser, node_id, "value").map(|v| v.chars().count()),
            Some(3),
            "both characters were inserted"
        );
        assert_eq!(
            attr(&browser, node_id, "data-changed"),
            None,
            "no change yet"
        );

        // Clicking normal content blurs the field → one trailing `change` event.
        browser.handle_mouse_move(30.0, HEADER_HEIGHT + 70.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(
            browser.focused_control.is_none(),
            "focus cleared by content click"
        );
        assert_eq!(
            attr(&browser, node_id, "data-changed").as_deref(),
            Some("1"),
            "change listener fired exactly once on blur"
        );
    }

    #[test]
    fn test_browser_smooth_scroll_to() {
        let mut browser = BrowserChrome::new(800, 600);
        browser.load_html_internal(
            "<div style='height: 2000px;'>Long Content</div>".to_string(),
            "http://example.com".to_string(),
            false,
        );
        assert_eq!(browser.scroll_y, 0.0);

        browser.scroll_smooth_to(300.0, Some(std::time::Duration::from_millis(200)));
        assert!(browser.smooth_scroll.is_some());
        assert!(browser.has_pending_timers());

        // Step animation forward after small delay
        std::thread::sleep(std::time::Duration::from_millis(30));
        let stepped = browser.tick_animations(30.0);
        assert!(stepped);
        assert!(browser.scroll_y > 0.0);

        // Advance to completion
        std::thread::sleep(std::time::Duration::from_millis(210));
        let _ = browser.tick_animations(150.0);
        assert!(browser.scroll_y >= 290.0);
    }
}
