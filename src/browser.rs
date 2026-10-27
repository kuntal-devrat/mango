//! Browser chrome: Chrome-like tab bar, toolbar, address bar, content area, and status bar.
//!
//! This module owns the browser's UI layout and coordinates with `mango_layout`
//! to render HTML documents dynamically in the content area, as well as integrating
//! the `mango_js` V8-powered JavaScript engine for interactive DOM manipulation and timers.

use std::collections::{HashMap, HashSet};

use mango_core::Color;
use mango_css::Stylesheet;
use mango_css::computed::ComputedStyle;
use mango_html::dom::Document;
use mango_js::JsRuntime;
use mango_net::{ResourceLoader, Url};
use mango_render::display_list::DisplayList;

use crate::config::Config;
use crate::tab::Tab;

// Subsystems modularized under src/browser/
pub mod animation;
pub mod forms;
pub mod images;
pub mod input;
pub mod navigation;
pub mod profile;
pub mod render;
pub mod resource_scheduler;
pub mod tabs;
pub mod types;
pub mod welcome;

#[cfg(test)]
mod tests;

pub use forms::*;
pub use navigation::{
    prepare_document_pipeline, prepare_document_pipeline_cancellable,
    prepare_document_pipeline_from_doc,
};
pub use profile::*;
pub use types::*;
pub use welcome::*;

// Subsystem module re-exports
pub use crate::chrome_ui::*;
pub use crate::context_menu::*;
pub use crate::form_handler::*;
pub use crate::navigation::*;
pub use crate::scroll::*;

/// The browser chrome - manages tab bar, navigation toolbar, address bar, content area, and status bar.
pub struct BrowserChrome {
    /// Current window dimensions.
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Text in the address bar.
    pub(crate) address_text: String,
    /// Whether the address bar is focused (accepting input).
    pub(crate) address_focused: bool,
    /// Whether all text in the address bar is currently selected.
    pub(crate) is_all_selected: bool,
    /// Text cursor position in the address bar.
    pub(crate) cursor_pos: usize,
    /// Current mouse cursor X coordinate.
    pub(crate) mouse_x: f32,
    /// Current mouse cursor Y coordinate.
    pub(crate) mouse_y: f32,
    /// Currently hovered UI element.
    pub(crate) hovered_target: Option<HoverTarget>,
    /// Status bar text (e.g., "Ready", hovered link URL).
    pub(crate) status_text: String,
    /// Current page title.
    pub(crate) page_title: String,
    /// Open tabs.
    pub(crate) tabs: Vec<Tab>,
    /// Index of the active tab.
    pub(crate) active_tab_idx: usize,
    /// Raw HTML content currently loaded in the active tab.
    pub(crate) current_html: String,
    /// Cached parsed HTML Document of `current_html` to prevent redundant re-parsing.
    pub(crate) cached_document: Option<Document>,
    /// Base URL of current page, if loaded over the network.
    pub(crate) base_url: Option<Url>,
    /// External author stylesheets downloaded via <link rel="stylesheet">.
    pub(crate) cached_stylesheets: Vec<Stylesheet>,
    /// Page-owned stylesheet/index pair reused across relayouts. The cache is
    /// invalidated when the DOM mutation version changes.
    pub(crate) style_index_cache: Option<mango_layout::PageStyleIndex>,
    pub(crate) style_index_version: u64,
    pub(crate) style_index_viewport: (u32, u32),
    /// DOM/style changes requested during a JS turn. The UI loop flushes this
    /// once after script and animation work instead of relayouting per callback.
    pub(crate) layout_pending: bool,
    /// Coalesced asynchronous layout pipeline used by the desktop UI thread.
    pub(crate) async_layout: render::layout::AsyncLayoutCoordinator,
    /// HTTP and resource loader.
    pub(crate) loader: ResourceLoader,
    /// Vertical scroll offset in the content area (f32 for rendering/layout interoperability).
    pub(crate) scroll_y: f32,
    /// High-precision 64-bit vertical scroll offset to prevent precision loss on long pages.
    pub(crate) scroll_y_f64: f64,
    /// Browser configuration.
    #[allow(dead_code)]
    pub(crate) config: Config,
    /// Root layout box of the currently displayed document for hit-testing.
    pub(crate) root_box: Option<mango_layout::box_tree::LayoutBox>,
    /// Live JavaScript runtime for the active page.
    pub(crate) js_runtime: Option<JsRuntime>,
    /// Whether a network load is currently in progress (shows progress bar).
    pub(crate) is_loading: bool,
    /// Active right-click context menu, if open.
    pub(crate) context_menu: Option<ContextMenu>,
    /// Active custom select dropdown overlay, if open.
    pub(crate) select_dropdown: Option<SelectDropdown>,
    /// Active popup picker overlay (color/date/time/file), if open.
    pub(crate) picker: Option<PickerOverlay>,
    /// Active HTML5 constraint validation bubble overlay, if open.
    pub(crate) validation_bubble: Option<ValidationBubble>,
    /// Node id of a range slider currently being dragged.
    pub(crate) range_dragging: Option<mango_html::dom::NodeId>,
    /// Slider value captured when the current drag started.
    pub(crate) range_drag_start_value: Option<String>,
    /// Value of the focused text control when focus began.
    pub(crate) focused_initial_value: Option<String>,
    /// Focused form control node ID in the content area, if any.
    pub(crate) focused_control: Option<mango_html::dom::NodeId>,
    /// Text cursor position within the focused form control.
    pub(crate) control_cursor_pos: usize,
    /// Cached layout display list of the current page.
    pub(crate) page_display_list: DisplayList,
    /// Cached scrollable content height to avoid box tree traversals on every frame/mouse-move.
    pub(crate) cached_scrollable_height: f32,
    /// Tracks whether the page layout/display list has changed and needs upload to page_layer.
    pub(crate) page_dirty: bool,
    /// Display list cache with dirty-tracking and scroll-invalidation.
    pub(crate) display_list_cache: std::sync::Mutex<mango_layout::display_list::DisplayListCache>,
    /// Background color of the content canvas.
    pub(crate) canvas_bg: Color,
    /// Whether the user is dragging the scrollbar thumb.
    pub(crate) scrollbar_dragging: bool,
    /// Mouse Y coordinate when scrollbar drag started.
    pub(crate) drag_start_mouse_y: f32,
    /// Scroll offset when scrollbar drag started.
    pub(crate) drag_start_scroll_y: f32,
    /// CSS transition and keyframes animation runtime state.
    pub(crate) anim_state: mango_css::AnimationState,
    /// Base (pre-animation) computed styles per DOM node, captured at layout time.
    pub(crate) style_snapshot: HashMap<u32, ComputedStyle>,
    /// Shared cookie jar.
    pub(crate) cookie_jar: std::sync::Arc<std::sync::Mutex<mango_net::CookieJar>>,
    /// Profile file the cookie jar is persisted to.
    pub(crate) cookie_store_path: std::path::PathBuf,
    /// Shared localStorage store.
    pub(crate) local_storage: mango_js::web_apis::SharedLocalStorage,
    /// Profile file localStorage is persisted to.
    pub(crate) local_storage_path: std::path::PathBuf,
    /// Image URLs queued for background prefetch.
    pub(crate) pending_image_fetches: Vec<String>,
    /// Number of per-frame image-drain batches currently executing in the
    /// bounded image pool.
    pub(crate) image_drain_in_flight: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Base URL that `pending_image_fetches` entries resolve against.
    pub(crate) image_fetch_base: Option<Url>,
    /// Built-in Developer Tools & Element Inspector.
    pub devtools: crate::devtools::DevTools,
    /// Browser extension & add-on manager.
    pub extension_manager: crate::extensions::ExtensionManager,
    /// DOM nodes currently marked as hovered (:hover).
    pub(crate) hovered_dom_nodes: HashSet<mango_html::dom::NodeId>,
    /// DOM nodes currently marked as active (:active).
    pub(crate) active_dom_nodes: HashSet<mango_html::dom::NodeId>,
    /// Active smooth scrolling animation controller.
    pub smooth_scroll: Option<crate::scroll::SmoothScrollAnimation>,
    /// Dedicated actor handle for concurrent background navigation and pipeline preparation.
    pub nav_actor: crate::navigation_actor::NavigationActorHandle,
    /// Whether network navigations should proceed asynchronously.
    pub async_navigation: bool,
    /// Channel receiver for images decoded by background workers.
    pub(crate) image_rx:
        Option<std::sync::mpsc::Receiver<crate::browser::images::ImageFetchResult>>,
    /// Epoch guarding background image workers.
    pub(crate) image_epoch: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Resource policy and diagnostics for the active document.
    pub(crate) resource_scheduler: resource_scheduler::DocumentResourceScheduler,
    /// Generation of web fonts loaded in `FontManager`; when this advances, layout/display-list must re-render.
    pub(crate) last_font_epoch: u64,
    /// Pending deferred/async scripts queued for non-blocking incremental execution.
    pub(crate) pending_scripts: std::collections::VecDeque<crate::browser::types::ScheduledScript>,
    /// Compositor layer tree for GPU/tiled layer presentation.
    pub layer_tree: mango_render::LayerTree,
    /// Monotonic frame counter for tile cache lifecycle.
    pub frame_counter: u64,
    /// Find-in-page interactive search overlay state (Ctrl+F).
    pub find_in_page: FindInPage,
    /// Zoom level factor for page contents (1.0 = 100%).
    pub zoom_level: f32,
    /// Fullscreen mode toggle (F11).
    pub is_fullscreen: bool,
    /// Dedicated actor handle for concurrent out-of-process renderer execution.
    pub(crate) renderer_actor: Option<crate::renderer_actor::RendererActorHandle>,
    /// Generation counter guarding out-of-process frames.
    pub(crate) renderer_generation: u64,
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

        let loader = ResourceLoader::with_cookie_jar(cookie_jar.clone());
        let nav_actor = crate::navigation_actor::NavigationActorHandle::spawn(loader.clone());

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
            status_text: "Ready".to_string(),
            page_title: initial_title,
            tabs: vec![default_tab],
            active_tab_idx: 0,
            current_html: initial_html.clone(),
            cached_document: None,
            base_url: None,
            cached_stylesheets: Vec::new(),
            style_index_cache: None,
            style_index_version: u64::MAX,
            style_index_viewport: (0, 0),
            layout_pending: false,
            async_layout: render::layout::AsyncLayoutCoordinator::new(),
            scroll_y: 0.0,
            scroll_y_f64: 0.0,
            config: Config::new(),
            root_box: None,
            js_runtime: None,
            is_loading: false,
            context_menu: None,
            select_dropdown: None,
            picker: None,
            validation_bubble: None,
            range_dragging: None,
            range_drag_start_value: None,
            focused_initial_value: None,
            focused_control: None,
            control_cursor_pos: 0,
            page_display_list: DisplayList::new(),
            cached_scrollable_height: 0.0,
            page_dirty: true,
            display_list_cache: std::sync::Mutex::new(
                mango_layout::display_list::DisplayListCache::new(),
            ),
            canvas_bg: CONTENT_BG,
            scrollbar_dragging: false,
            drag_start_mouse_y: 0.0,
            drag_start_scroll_y: 0.0,
            anim_state: mango_css::AnimationState::new(),
            style_snapshot: HashMap::new(),
            loader,
            nav_actor,
            cookie_jar,
            cookie_store_path,
            local_storage,
            local_storage_path,
            pending_image_fetches: Vec::new(),
            image_drain_in_flight: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            image_fetch_base: None,
            devtools: crate::devtools::DevTools::new(),
            extension_manager: crate::extensions::ExtensionManager::new(),
            hovered_dom_nodes: HashSet::new(),
            active_dom_nodes: HashSet::new(),
            smooth_scroll: None,
            async_navigation: false,
            image_rx: None,
            image_epoch: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            resource_scheduler: resource_scheduler::DocumentResourceScheduler::default(),
            last_font_epoch: mango_render::web_font_epoch(),
            pending_scripts: std::collections::VecDeque::new(),
            layer_tree: mango_render::LayerTree::new(width, height),
            frame_counter: 0,
            find_in_page: FindInPage::default(),
            zoom_level: 1.0,
            is_fullscreen: false,
            renderer_actor: None,
            renderer_generation: 1,
        };

        mark_ui_thread();
        chrome.load_html_internal(initial_html, "about:welcome".to_string(), false);
        chrome
    }

    pub fn page_title(&self) -> &str {
        &self.page_title
    }

    /// Returns true if there are queued deferred or async scripts waiting to run.
    pub fn has_pending_scripts(&self) -> bool {
        !self.pending_scripts.is_empty()
    }

    /// Returns resource accounting for the active document.
    pub fn resource_report(&self) -> &resource_scheduler::ResourceReport {
        self.resource_scheduler.report()
    }

    #[allow(dead_code)]
    pub fn current_html(&self) -> &str {
        &self.current_html
    }

    /// Returns the current mouse X position in window coordinates.
    pub fn mouse_x(&self) -> f32 {
        self.mouse_x
    }

    /// Returns the current mouse Y position in window coordinates.
    pub fn mouse_y(&self) -> f32 {
        self.mouse_y
    }

    /// Returns the currently active HTML document (from JS runtime snapshot or parsed cache).
    ///
    /// This deep-clones the DOM. Prefer [`BrowserChrome::document_view`] on hot paths.
    #[allow(dead_code)]
    pub fn active_document(&self) -> Document {
        self.document_view().clone()
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

    /// Returns the active vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// Returns the active high-precision vertical scroll offset.
    pub fn scroll_y_f64(&self) -> f64 {
        self.scroll_y_f64
    }

    /// Sets the vertical scroll offset.
    pub fn set_scroll_y(&mut self, y: f32) {
        self.set_scroll_y_f64(y as f64);
    }

    /// Sets the vertical scroll offset with 64-bit precision.
    pub fn set_scroll_y_f64(&mut self, y: f64) {
        self.scroll_y_f64 = y.max(0.0);
        self.scroll_y = self.scroll_y_f64 as f32;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.scroll_y = self.scroll_y;
            tab.scroll_y_f64 = self.scroll_y_f64;
        }
        self.reprioritize_visible_images();
    }

    /// Adjusts the vertical scroll offset by delta.
    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll_by_f64(dy as f64);
    }

    /// Adjusts the vertical scroll offset by 64-bit delta.
    pub fn scroll_by_f64(&mut self, dy: f64) {
        self.scroll_y_f64 = (self.scroll_y_f64 + dy).max(0.0);
        self.scroll_y = self.scroll_y_f64 as f32;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.scroll_y = self.scroll_y;
            tab.scroll_y_f64 = self.scroll_y_f64;
        }
    }

    /// Returns the current active validation bubble, if any.
    pub fn validation_bubble(&self) -> Option<&ValidationBubble> {
        self.validation_bubble.as_ref()
    }

    /// Returns the current status bar text.
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Evaluates a JavaScript expression and returns its string result.
    pub fn eval_js(&mut self, expr: &str) -> Result<String, String> {
        if let Some(ref mut rt) = self.js_runtime {
            rt.eval(expr)
        } else {
            Err("No JS runtime".to_string())
        }
    }

    /// Polls and drains asynchronous messages from the isolated Out-of-Process Renderer.
    /// Returns true if any applied frame changed visible presentation.
    pub fn tick_renderer(&mut self) -> bool {
        let Some(ref mut renderer) = self.renderer_actor else {
            return false;
        };
        let mut messages = Vec::new();
        while let Ok(msg) = renderer.result_rx.try_recv() {
            messages.push(msg);
        }
        let mut updated = false;
        for msg in messages {
            match msg {
                crate::renderer_actor::RendererResult::FrameReady(frame) => {
                    if frame.generation >= self.renderer_generation {
                        self.page_display_list = frame.display_list;
                        self.cached_scrollable_height = frame.scrollable_height;
                        self.canvas_bg = frame.canvas_bg;
                        if let Some(title) = frame.title {
                            self.page_title = title.clone();
                            if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                                tab.set_title(title);
                            }
                        }
                        if let Some(root) = frame.root_box {
                            self.root_box = Some(root);
                        }
                        if let Some(doc) = frame.doc_mirror {
                            self.cached_document = Some(doc);
                        }
                        self.style_snapshot = frame.style_snapshot;
                        self.page_dirty = true;
                        updated = true;
                    }
                }
                crate::renderer_actor::RendererResult::OpenWindow { url, features: _ } => {
                    let nav_url = if let Some(base) = &self.base_url {
                        base.resolve(&url).map(|u| u.to_string()).unwrap_or(url)
                    } else {
                        url
                    };
                    self.new_tab_and_navigate(&nav_url);
                    updated = true;
                }
                crate::renderer_actor::RendererResult::TitleChanged(title) => {
                    self.page_title = title.clone();
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.set_title(title);
                    }
                    updated = true;
                }
                crate::renderer_actor::RendererResult::ConsoleMessage { level, text } => {
                    self.devtools.log(level, text);
                }
                crate::renderer_actor::RendererResult::NavigationRequested { url } => {
                    self.navigate(&url);
                    updated = true;
                }
                crate::renderer_actor::RendererResult::Error { message } => {
                    log::warn!("Isolated renderer warning: {message}");
                }
            }
        }
        updated
    }
}

impl Drop for BrowserChrome {
    fn drop(&mut self) {
        self.persist_profile();
        self.js_runtime = None;
        while let Some(tab) = self.tabs.pop() {
            drop(tab);
        }
    }
}
