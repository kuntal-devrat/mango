//! Viewport layout, geometry calculation, and zoom controls.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, mpsc};

use mango_core::{Color, Size};
use mango_css::Stylesheet;
use mango_css::computed::ComputedStyle;
use mango_html::parse_html;
use mango_layout::{extract_style_elements, layout_document, layout_document_with_style_index};

use super::super::BrowserChrome;
use super::super::types::DocumentView;
use crate::chrome_ui::*;

struct LayoutRequest {
    generation: u64,
    document: mango_html::dom::Document,
    stylesheets: Vec<Stylesheet>,
    viewport: Size,
}

pub(crate) struct LayoutResult {
    pub generation: u64,
    pub root_box: mango_layout::box_tree::LayoutBox,
    pub page_display_list: mango_render::display_list::DisplayList,
    pub canvas_bg: Color,
    pub scrollable_height: f32,
    pub background_images: Vec<String>,
    pub box_rects: HashMap<u32, [f32; 4]>,
}

#[derive(Default)]
struct LayoutQueueState {
    pending: Option<LayoutRequest>,
    running: bool,
}

/// Coalesces layout requests per window and runs expensive style/layout work
/// away from the native event thread. A newer request replaces queued stale
/// input instead of building an unbounded backlog during resize or mutation.
pub(crate) struct AsyncLayoutCoordinator {
    state: Arc<Mutex<LayoutQueueState>>,
    tx: mpsc::Sender<LayoutResult>,
    rx: mpsc::Receiver<LayoutResult>,
    generation: u64,
}

static LAYOUT_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn layout_pool() -> &'static rayon::ThreadPool {
    LAYOUT_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|i| format!("mango-layout-{i}"))
            .build()
            .expect("failed to initialize layout worker pool")
    })
}

impl AsyncLayoutCoordinator {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            state: Arc::new(Mutex::new(LayoutQueueState::default())),
            tx,
            rx,
            generation: 0,
        }
    }

    pub(crate) fn request(
        &mut self,
        document: mango_html::dom::Document,
        stylesheets: Vec<Stylesheet>,
        viewport: Size,
    ) {
        self.generation = self.generation.wrapping_add(1);
        let request = LayoutRequest {
            generation: self.generation,
            document,
            stylesheets,
            viewport,
        };
        let start_worker = if let Ok(mut state) = self.state.lock() {
            state.pending = Some(request);
            if state.running {
                false
            } else {
                state.running = true;
                true
            }
        } else {
            false
        };

        if start_worker {
            let state = self.state.clone();
            let tx = self.tx.clone();
            layout_pool().spawn(move || {
                loop {
                    let request = match state.lock() {
                        Ok(mut state) => match state.pending.take() {
                            Some(request) => request,
                            None => {
                                state.running = false;
                                return;
                            }
                        },
                        Err(_) => return,
                    };

                    let result = compute_layout(request);
                    if tx.send(result).is_err() {
                        if let Ok(mut state) = state.lock() {
                            state.running = false;
                            state.pending = None;
                        }
                        return;
                    }
                    mango_core::wake_event_loop();
                }
            });
        }
    }

    pub(crate) fn poll_latest(&mut self) -> Option<LayoutResult> {
        let mut latest = None;
        loop {
            match self.rx.try_recv() {
                Ok(result) if result.generation == self.generation => latest = Some(result),
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }
        latest
    }

    pub(crate) fn is_active(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.running || state.pending.is_some())
    }
}

fn compute_layout(request: LayoutRequest) -> LayoutResult {
    let mut stylesheets = request.stylesheets;
    stylesheets.extend(extract_style_elements(&request.document));
    let mut media = mango_css::current_media_environment();
    media.viewport_width = request.viewport.width;
    media.viewport_height = request.viewport.height;
    let style_index = mango_layout::PageStyleIndex::from_stylesheets(stylesheets, media);
    let (root_box, page_display_list) =
        layout_document_with_style_index(&request.document, &style_index, request.viewport);
    let canvas_bg = canvas_background_color(&root_box);
    let scrollable_height = crate::scroll::compute_scrollable_height(Some(&root_box));
    let mut background_images = Vec::new();
    crate::browser::navigation::resources::collect_box_background_images(
        &root_box,
        &mut background_images,
    );
    let mut box_rects = HashMap::new();
    mango_layout::collect_box_rects(&root_box, &mut box_rects);
    LayoutResult {
        generation: request.generation,
        root_box,
        page_display_list,
        canvas_bg,
        scrollable_height,
        background_images,
        box_rects,
    }
}

pub fn canvas_background_color(root: &mango_layout::box_tree::LayoutBox) -> Color {
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

impl BrowserChrome {
    /// Borrows the currently active DOM without cloning it.
    pub(crate) fn document_view(&self) -> DocumentView<'_> {
        if let Some(ref rt) = self.js_runtime {
            DocumentView::Runtime(rt.document_ref())
        } else if let Some(ref cached) = self.cached_document {
            DocumentView::Cached(cached)
        } else {
            DocumentView::Parsed(Box::new(parse_html(&self.current_html)))
        }
    }

    /// Recomputes document layout using the active DOM (either from JS runtime or parsed HTML).
    ///
    /// Borrows the live DOM instead of cloning it (see [`DocumentView`]): layout runs
    /// on every resize, JS mutation and animation frame, so a deep copy here would
    /// dominate the frame cost on large documents.
    pub fn relayout(&mut self) {
        self.layout_pending = false;
        let viewport = Size::new(self.width as f32, self.content_height());

        if self.async_navigation {
            let document = (*self.document_view()).clone();
            self.style_index_cache = None;
            self.async_layout
                .request(document, self.cached_stylesheets.clone(), viewport);
            return;
        }

        // Do not retain the previous full-page representation while building
        // the replacement. On large documents the old layout tree, old paint
        // list, and cache can otherwise coexist with the new ones and create
        // a multi-gigabyte peak even though only one page state is needed.
        self.root_box = None;
        self.page_display_list = mango_render::display_list::DisplayList::new();
        self.display_list_cache.lock().unwrap().clear();

        // Build the page stylesheet index once per DOM version. Animations,
        // image completion, scrolling, and other relayouts reuse it without
        // reparsing or re-indexing every external rule.
        let (doc_version, embedded_sheets) = {
            let doc = self.document_view();
            (doc.version, extract_style_elements(&doc))
        };
        let viewport_key = (self.width, self.height);
        if self.style_index_cache.is_none()
            || self.style_index_version != doc_version
            || self.style_index_viewport != viewport_key
        {
            let mut sheets = self.cached_stylesheets.clone();
            sheets.extend(embedded_sheets);
            self.style_index_cache = Some(mango_layout::PageStyleIndex::from_stylesheets(
                sheets,
                mango_css::current_media_environment(),
            ));
            if let Some(index) = &self.style_index_cache {
                log::debug!(
                    "built page stylesheet index: {} sheets, {} indexed entries",
                    index.stylesheet_count(),
                    index.indexed_entry_count()
                );
            }
            self.style_index_version = doc_version;
            self.style_index_viewport = viewport_key;
        }

        let (root_box, page_dl, canvas_color) = {
            let doc = self.document_view();
            let (root_box, page_dl) = if let Some(index) = &self.style_index_cache {
                layout_document_with_style_index(&doc, index, viewport)
            } else {
                let author_sheets: Vec<&Stylesheet> = self.cached_stylesheets.iter().collect();
                layout_document(&doc, &author_sheets, viewport)
            };
            let canvas_color = canvas_background_color(&root_box);
            (root_box, page_dl, canvas_color)
        };

        self.canvas_bg = canvas_color;
        self.page_display_list = page_dl;
        self.root_box = Some(root_box);
        let background_images = self.root_box.as_ref().map(|root| {
            let mut images = Vec::new();
            crate::browser::navigation::resources::collect_box_background_images(root, &mut images);
            images
        });
        if let (Some(base), Some(background_images)) = (self.base_url.clone(), background_images) {
            self.queue_additional_image_sources(background_images, base);
        }
        self.cached_scrollable_height =
            crate::scroll::compute_scrollable_height(self.root_box.as_ref());
        self.page_dirty = true;
        self.display_list_cache.lock().unwrap().invalidate(None);

        self.refresh_animation_targets();
        self.publish_layout_bounds();
    }

    /// Applies the newest completed worker layout on the UI thread.
    pub fn tick_async_layout(&mut self) -> bool {
        let Some(result) = self.async_layout.poll_latest() else {
            return false;
        };
        self.root_box = Some(result.root_box);
        self.page_display_list = result.page_display_list;
        self.canvas_bg = result.canvas_bg;
        self.cached_scrollable_height = result.scrollable_height;
        self.page_dirty = true;
        self.display_list_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .invalidate(None);
        if let Some(base) = self.base_url.clone() {
            self.queue_additional_image_sources(result.background_images, base);
        }
        self.refresh_animation_targets();
        if let Some(rt) = &self.js_runtime {
            rt.set_layout_bounds(result.box_rects);
        }
        true
    }

    /// True while a background style/layout result is queued or being built.
    pub fn has_pending_layout(&self) -> bool {
        self.async_layout.is_active()
    }

    /// Flushes a DOM/style invalidation once per event-loop turn.
    pub fn flush_pending_layout(&mut self) -> bool {
        if self.layout_pending {
            self.relayout();
            true
        } else {
            false
        }
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
    ///
    /// Deliberately performs NO second layout here: the animated blend is applied
    /// on the next [`Self::tick_animations`] frame, so each frame lays out at
    /// most once instead of twice (P0 double-relayout fix).
    fn refresh_animation_targets(&mut self) {
        let Some(root) = self.root_box.as_mut() else {
            return;
        };

        let mut fresh: HashMap<u32, ComputedStyle> = HashMap::new();
        mango_layout::collect_box_styles(root, &mut fresh);
        let previous = std::mem::take(&mut self.style_snapshot);

        let mut starts: Vec<(mango_html::dom::NodeId, ComputedStyle)> = Vec::new();
        mango_layout::apply_box_style_overrides(root, &mut |node, style| {
            if let Some(node) = node {
                starts.push((node, style.clone()));
            }
        });

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
        // No second `relayout_box_tree` here by design (see doc comment).
    }

    pub(crate) fn content_width(&self) -> f32 {
        self.width as f32
    }

    /// Height of the page content area in CSS pixels.
    pub(crate) fn content_height(&self) -> f32 {
        ((self.height as f32) - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.layer_tree.resize(width, height);
        self.relayout();
        let cw = self.content_width();
        let ch = self.content_height();
        if let Some(ref mut js_rt) = self.js_runtime {
            let _ = js_rt.dispatch_window_resize(cw, ch);
        }
    }

    /// Returns the active canvas background color.
    pub fn canvas_bg(&self) -> Color {
        self.canvas_bg
    }

    /// Returns the active document's layout root box, if layout has completed.
    pub fn root_box(&self) -> Option<&mango_layout::box_tree::LayoutBox> {
        self.root_box.as_ref()
    }

    /// Increases zoom level by 10% (Ctrl++).
    pub fn zoom_in(&mut self) {
        self.zoom_level = (self.zoom_level + 0.1).min(3.0);
        self.status_text = format!("Zoom: {}%", (self.zoom_level * 100.0).round() as u32);
        self.relayout();
    }

    /// Decreases zoom level by 10% (Ctrl+-).
    pub fn zoom_out(&mut self) {
        self.zoom_level = (self.zoom_level - 0.1).max(0.25);
        self.status_text = format!("Zoom: {}%", (self.zoom_level * 100.0).round() as u32);
        self.relayout();
    }

    /// Resets zoom level to 100% (Ctrl+0).
    pub fn zoom_reset(&mut self) {
        self.zoom_level = 1.0;
        self.status_text = "Zoom: 100%".to_string();
        self.relayout();
    }

    /// Toggles fullscreen mode (F11).
    pub fn toggle_fullscreen(&mut self) {
        self.is_fullscreen = !self.is_fullscreen;
        self.status_text = if self.is_fullscreen {
            "Fullscreen enabled (F11)".to_string()
        } else {
            "Fullscreen disabled (F11)".to_string()
        };
        self.page_dirty = true;
    }

    /// Returns the true scrollable document height scanning all layout boxes.
    pub fn scrollable_height(&self) -> f32 {
        if self.cached_scrollable_height > 0.0 {
            self.cached_scrollable_height
        } else {
            crate::scroll::compute_scrollable_height(self.root_box.as_ref())
        }
    }

    /// Computes maximum allowed scroll offset based on rendered content height.
    pub fn max_scroll(&self) -> f32 {
        let content_h = (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let doc_h = self.scrollable_height();
        (doc_h - content_h).max(0.0)
    }
}
