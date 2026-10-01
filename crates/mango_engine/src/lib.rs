//! # mango_engine
//!
//! Core browser engine orchestrating Document → StyleTree → LayoutTree → DisplayList (ARCH-001).
//!
//! Exposes a clean, high-level `Engine` API that encapsulates the complete rendering
//! pipeline and decouples browser chrome / UI from layout and rendering execution:
//! - **DOM**: Owns the parsed [`Document`] tree.
//! - **CSS**: Owns author stylesheets and resolves the cascade into [`StyledNode`].
//! - **Layout**: Generates immutable [`LayoutSnapshot`] trees for the viewport.
//! - **Painting**: Emits and diffs [`DisplayList`] render commands.
//! - **Events**: Manages W3C DOM capture/bubble event propagation.
//! - **Incremental Updates**: Tracks dirty DOM subtrees and avoids full re-layout/re-paint.
//!
//! # Example
//! ```
//! use mango_engine::Engine;
//! use mango_core::Size;
//!
//! let mut engine = Engine::new();
//! let display_list = engine.load(
//!     "<html><body><h1>Hello Mango Engine</h1></body></html>",
//!     "h1 { color: #ff5500; font-size: 28px; }"
//! );
//!
//! assert!(!display_list.is_empty());
//! assert_eq!(engine.viewport(), Size::new(800.0, 600.0));
//! ```

use std::collections::HashMap;

use mango_core::{Point, Rect, Size};
use mango_css::parser::{parse_stylesheet, Stylesheet};
use mango_events::{DispatchResult, Event, EventDispatcher, ListenerOptions, TargetId};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;
use mango_layout::{
    DisplayListCache, LayoutSnapshot, StyleInvalidator, StyledNode,
};
pub use mango_layout::box_tree::FormControlHit;
use mango_render::display_list::{DisplayList, DisplayListDiff};

/// Configuration options for initializing the engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Initial viewport size in CSS pixels (default: 800x600).
    pub viewport: Size,
    /// Default scroll offset in CSS pixels.
    pub scroll_y: f32,
    /// Whether to enable incremental style invalidation.
    pub enable_incremental_style: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            viewport: Size::new(800.0, 600.0),
            scroll_y: 0.0,
            enable_incremental_style: true,
        }
    }
}

/// The core Mango browser engine (ARCH-001).
///
/// Encapsulates all subsystems (DOM, CSS cascade, layout, events, display list)
/// behind a clean, decoupled interface. Browser Chrome instances interact with
/// the page strictly through `Engine`.
pub struct Engine {
    document: Document,
    shared_document: Option<std::rc::Rc<std::cell::RefCell<Document>>>,
    author_stylesheets: Vec<Stylesheet>,
    style_tree: Option<StyledNode>,
    layout_snapshot: Option<LayoutSnapshot>,
    display_list: DisplayList,
    display_list_cache: DisplayListCache,
    style_invalidator: StyleInvalidator,
    event_dispatcher: EventDispatcher,
    viewport: Size,
    scroll_y: f32,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Creates a new `Engine` instance with default configuration.
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }

    /// Creates an `Engine` with a custom initial viewport size.
    pub fn with_viewport(viewport: Size) -> Self {
        Self::with_config(EngineConfig {
            viewport,
            ..Default::default()
        })
    }

    /// Creates an `Engine` with the given configuration.
    pub fn with_config(config: EngineConfig) -> Self {
        Self {
            document: Document::new(),
            shared_document: None,
            author_stylesheets: Vec::new(),
            style_tree: None,
            layout_snapshot: None,
            display_list: DisplayList::new(),
            display_list_cache: DisplayListCache::new(),
            style_invalidator: StyleInvalidator::new(),
            event_dispatcher: EventDispatcher::new(),
            viewport: config.viewport,
            scroll_y: config.scroll_y,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Loading & Pipeline Execution
    // ─────────────────────────────────────────────────────────────────────────

    /// Loads HTML markup and author CSS into the engine, executing the full pipeline:
    /// Parse HTML → Parse CSS → Cascade StyleTree → Layout Snapshot → DisplayList (ARCH-001).
    pub fn load(&mut self, html: &str, css: &str) -> DisplayList {
        let doc = parse_html(html);
        let mut sheets = Vec::new();
        if !css.trim().is_empty() {
            sheets.push(parse_stylesheet(css));
        }
        self.load_document(doc, sheets)
    }

    /// Loads an HTML string into the engine, automatically extracting and parsing
    /// embedded `<style>` elements from the document.
    pub fn load_html(&mut self, html: &str) -> DisplayList {
        self.load(html, "")
    }

    /// Sets the active DOM document and author stylesheets, then executes full layout.
    pub fn load_document(
        &mut self,
        doc: Document,
        author_stylesheets: Vec<Stylesheet>,
    ) -> DisplayList {
        self.document = doc;
        self.shared_document = None;
        self.author_stylesheets = author_stylesheets;
        self.style_invalidator.clear();
        self.display_list_cache.invalidate(None);

        self.rebuild_pipeline();
        self.display_list.clone()
    }

    /// Sets a shared DOM document reference between Engine and JsRuntime,
    /// unifying the Document arena so DOM mutations immediately update both.
    pub fn set_shared_document(
        &mut self,
        doc: std::rc::Rc<std::cell::RefCell<Document>>,
    ) -> DisplayList {
        self.document = doc.borrow().clone();
        self.shared_document = Some(doc);
        self.style_invalidator.clear();
        self.display_list_cache.invalidate(None);
        self.rebuild_pipeline();
        self.display_list.clone()
    }

    /// Returns the shared Document handle if one is configured.
    pub fn shared_document(&self) -> Option<std::rc::Rc<std::cell::RefCell<Document>>> {
        self.shared_document.clone()
    }

    /// Synchronizes the engine's internal document from the shared document if configured.
    pub fn sync_from_shared_document(&mut self) -> bool {
        if let Some(ref shared) = self.shared_document {
            self.document = shared.borrow().clone();
            true
        } else {
            false
        }
    }

    /// Notifies the engine of a DOM mutation on `node_id`, immediately triggering
    /// incremental style recalculation and relayout.
    pub fn on_dom_mutation(&mut self, node_id: NodeId) -> DisplayList {
        self.sync_from_shared_document();
        self.style_invalidator.mark_dirty(node_id);
        self.display_list_cache.invalidate(None);
        self.rebuild_pipeline();
        self.display_list.clone()
    }

    /// Adds an additional author stylesheet to the engine and updates the pipeline.
    pub fn add_stylesheet(&mut self, css: &str) -> DisplayList {
        let sheet = parse_stylesheet(css);
        self.author_stylesheets.push(sheet);
        self.rebuild_pipeline();
        self.display_list.clone()
    }

    /// Clears all author stylesheets.
    pub fn clear_stylesheets(&mut self) {
        self.author_stylesheets.clear();
    }

    /// Rebuilds the complete pipeline from the current document and stylesheets.
    fn rebuild_pipeline(&mut self) {
        self.sync_from_shared_document();
        let author_refs: Vec<&Stylesheet> = self.author_stylesheets.iter().collect();

        // 1. Build Style Tree with actual viewport dimensions
        let styled_root = mango_layout::style_tree::build_style_tree_with_size(
            &self.document,
            &author_refs,
            self.viewport.width,
            self.viewport.height,
        );
        self.style_tree = styled_root.clone();

        // 2. Build Immutable Layout Snapshot
        if let Some(ref root) = styled_root {
            let snapshot = LayoutSnapshot::from_styled_tree(root, self.viewport);
            let raw_dl = snapshot.display_list().clone();
            self.layout_snapshot = Some(snapshot);

            // 3. Diff and Cache Display List
            let _diff = self.display_list_cache.update_and_diff(raw_dl.clone(), self.scroll_y);
            self.display_list = raw_dl;
        } else {
            self.layout_snapshot = None;
            self.display_list = DisplayList::new();
        }
    }

    /// Re-executes layout for the current viewport and updates the display list.
    pub fn relayout(&mut self) -> DisplayList {
        if let Some(ref styled_root) = self.style_tree {
            let snapshot = LayoutSnapshot::from_styled_tree(styled_root, self.viewport);
            let raw_dl = if self.scroll_y == 0.0 {
                snapshot.display_list().clone()
            } else {
                snapshot.display_list_with_scroll(self.scroll_y)
            };
            self.layout_snapshot = Some(snapshot);
            let _diff = self.display_list_cache.update_and_diff(raw_dl.clone(), self.scroll_y);
            self.display_list = raw_dl;
        }
        self.display_list.clone()
    }

    /// Updates the viewport size and recalculates layout.
    pub fn set_viewport(&mut self, viewport: Size) -> DisplayList {
        if self.viewport != viewport {
            self.viewport = viewport;
            self.relayout();
        }
        self.display_list.clone()
    }

    /// Returns the current viewport dimensions.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Sets the vertical scroll offset in CSS pixels.
    pub fn set_scroll_y(&mut self, scroll_y: f32) -> DisplayList {
        if (self.scroll_y - scroll_y).abs() > f32::EPSILON {
            self.scroll_y = scroll_y;
            self.relayout();
        }
        self.display_list.clone()
    }

    /// Returns the current vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Inspection & State Accessors
    // ─────────────────────────────────────────────────────────────────────────

    /// Returns an immutable reference to the DOM [`Document`].
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Returns a mutable reference to the DOM [`Document`].
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// Returns the current style tree root, if built.
    pub fn style_tree(&self) -> Option<&StyledNode> {
        self.style_tree.as_ref()
    }

    /// Returns the current immutable layout snapshot, if built.
    pub fn layout_snapshot(&self) -> Option<&LayoutSnapshot> {
        self.layout_snapshot.as_ref()
    }

    /// Returns the current paint-ready display list.
    pub fn display_list(&self) -> &DisplayList {
        &self.display_list
    }

    /// Returns the page title from `<title>` if present in the document.
    pub fn title(&self) -> Option<String> {
        fn find_title(doc: &Document, nid: NodeId) -> Option<String> {
            if let Some(node) = doc.get(nid) {
                if let NodeData::Element(elem) = &node.data {
                    if elem.tag_name.eq_ignore_ascii_case("title") {
                        return Some(doc.text_content(nid).trim().to_string());
                    }
                }
                for child in doc.children(nid) {
                    if let Some(t) = find_title(doc, child.id) {
                        return Some(t);
                    }
                }
            }
            None
        }
        find_title(&self.document, self.document.root())
    }

    /// Performs hit-testing at a point in viewport coordinates, returning the hit DOM node ID.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        let adjusted = Point::new(point.x, point.y + self.scroll_y);
        self.layout_snapshot.as_ref().and_then(|s| s.hit_test(adjusted))
    }

    /// Performs hit-testing at a point in viewport coordinates for clickable links.
    pub fn hit_test_link(&self, point: Point) -> Option<String> {
        let adjusted = Point::new(point.x, point.y + self.scroll_y);
        self.layout_snapshot
            .as_ref()
            .and_then(|s| s.hit_test_link(adjusted).map(|s| s.to_string()))
    }

    /// Performs hit-testing at a point for interactive form controls.
    pub fn hit_test_form_control(&self, point: Point) -> Option<FormControlHit> {
        let adjusted = Point::new(point.x, point.y + self.scroll_y);
        self.layout_snapshot
            .as_ref()
            .and_then(|s| s.hit_test_form_control(adjusted))
    }

    /// Returns the border-box bounds of a specific DOM node.
    pub fn get_node_bounds(&self, node_id: NodeId) -> Option<Rect> {
        self.layout_snapshot.as_ref().and_then(|s| s.get_node_rect(node_id))
    }

    /// Collects layout bounds for all DOM elements, keyed by raw `u32` node ID.
    pub fn collect_layout_bounds(&self) -> HashMap<u32, [f32; 4]> {
        self.layout_snapshot
            .as_ref()
            .map(|s| s.box_rects().clone())
            .unwrap_or_default()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Incremental Mutation & Recalculation (ARCH-002 & ARCH-004)
    // ─────────────────────────────────────────────────────────────────────────

    /// Mutates the DOM via a closure, then applies incremental style invalidation
    /// and layout snapshot regeneration, returning the diff between old and new display lists.
    ///
    /// This avoids full $O(N)$ re-cascade and full screen repainting.
    pub fn mutate_and_recalc<F, R>(&mut self, f: F) -> (R, DisplayListDiff)
    where
        F: FnOnce(&mut Document) -> R,
    {
        let res = f(&mut self.document);

        let dirty = self.document.take_dirty_nodes();
        if !dirty.is_empty() && self.style_tree.is_some() {
            self.style_invalidator.mark_all_dirty(dirty);
            let author_refs: Vec<&Stylesheet> = self.author_stylesheets.iter().collect();

            // 1. Incremental Style Recalculation
            self.style_invalidator.invalidate_and_update(
                self.style_tree.as_mut().unwrap(),
                &self.document,
                &author_refs,
            );

            // 2. Regenerate Layout Snapshot
            let snapshot = LayoutSnapshot::from_styled_tree(
                self.style_tree.as_ref().unwrap(),
                self.viewport,
            );
            let new_dl = if self.scroll_y == 0.0 {
                snapshot.display_list().clone()
            } else {
                snapshot.display_list_with_scroll(self.scroll_y)
            };
            self.layout_snapshot = Some(snapshot);

            // 3. Compute DisplayListDiff
            let diff = self.display_list_cache.update_and_diff(new_dl.clone(), self.scroll_y);
            self.display_list = new_dl;

            (res, diff)
        } else {
            let empty_diff = DisplayListDiff::default();
            (res, empty_diff)
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Event System Delegation (ARCH-006)
    // ─────────────────────────────────────────────────────────────────────────

    /// Registers a W3C DOM event listener on a target ID.
    pub fn add_event_listener<F>(
        &mut self,
        target: TargetId,
        event_type: &str,
        callback: F,
        options: ListenerOptions,
    ) where
        F: FnMut(&mut Event) + Send + 'static,
    {
        self.event_dispatcher.add_listener(target, event_type, callback, options);
    }

    /// Removes an event listener.
    pub fn remove_event_listener(&mut self, target: TargetId, event_type: &str, capture: bool) {
        self.event_dispatcher.remove_listener(target, event_type, capture);
    }

    /// Dispatches an event through the target propagation path with capture,
    /// at-target, and bubbling phases (ARCH-006).
    pub fn dispatch_event(&mut self, event: Event, target_path: &[TargetId]) -> DispatchResult {
        self.event_dispatcher.dispatch(event, target_path)
    }

    /// Dispatches a DOM event to a target NodeId by computing the ancestor chain from
    /// Document root down to `target_node`, running through capture, at-target, and bubble phases (ARCH-006).
    pub fn dispatch_dom_event(&mut self, event: Event, target_node: NodeId) -> DispatchResult {
        let mut path = Vec::new();
        let mut curr = Some(target_node);
        while let Some(nid) = curr {
            path.push(nid.raw());
            curr = self.document().get(nid).and_then(|n| n.parent);
        }
        path.reverse();
        self.dispatch_event(event, &path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_load_end_to_end() {
        let mut engine = Engine::new();
        let dl = engine.load(
            r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Test Page Title</title>
                </head>
                <body>
                    <h1 class="header">Hello Mango Engine</h1>
                    <p>Testing ARCH-001 decoupled engine architecture.</p>
                </body>
            </html>
            "#,
            "h1.header { color: #ff5500; font-size: 32px; }",
        );

        assert!(!dl.is_empty());
        assert_eq!(engine.title().as_deref(), Some("Test Page Title"));
        assert!(engine.layout_snapshot().is_some());
        assert!(engine.style_tree().is_some());
    }

    #[test]
    fn test_engine_mutate_and_incremental_diff() {
        let mut engine = Engine::new();
        engine.load(
            r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <style>
                        .item { color: #333333; }
                        .highlight { color: #ff0000; }
                    </style>
                </head>
                <body>
                    <div id="i1" class="item">Item 1</div>
                    <div id="i2" class="item">Item 2</div>
                </body>
            </html>
            "#,
            "",
        );

        let i2_id = engine
            .document()
            .find_element_by_id(engine.document().root(), "i2")
            .unwrap();

        // Mutate i2 using mutate_and_recalc
        let (_, diff) = engine.mutate_and_recalc(|doc| {
            doc.add_class(i2_id, "highlight");
        });

        // Display list diff should report changes!
        assert!(diff.has_changes());
    }

    #[test]
    fn test_engine_event_dispatch() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut engine = Engine::new();
        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = Arc::clone(&clicked);

        // Register click on target 42
        engine.add_event_listener(
            42,
            "click",
            move |_evt| {
                clicked_clone.store(true, Ordering::SeqCst);
            },
            ListenerOptions::default(),
        );

        let evt = Event::new("click");
        let result = engine.dispatch_event(evt, &[1, 10, 42]);

        assert!(!result.default_prevented);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn test_engine_dom_event_dispatch() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut engine = Engine::new();
        engine.load(
            r#"<html><body><div id="container"><button id="btn">Click me</button></div></body></html>"#,
            "",
        );

        let btn_id = engine
            .document()
            .find_element_by_id(engine.document().root(), "btn")
            .unwrap();

        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = Arc::clone(&clicked);

        engine.add_event_listener(
            btn_id.raw(),
            "click",
            move |_evt| {
                clicked_clone.store(true, Ordering::SeqCst);
            },
            ListenerOptions::default(),
        );

        let evt = Event::new("click");
        let result = engine.dispatch_dom_event(evt, btn_id);

        assert!(!result.default_prevented);
        assert!(clicked.load(Ordering::SeqCst));
    }
}
