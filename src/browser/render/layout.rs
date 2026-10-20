//! Viewport layout, geometry calculation, and zoom controls.

use std::collections::HashMap;

use mango_core::{Color, Size};
use mango_css::Stylesheet;
use mango_css::computed::ComputedStyle;
use mango_html::parse_html;
use mango_layout::layout_document;

use super::super::BrowserChrome;
use super::super::types::DocumentView;
use crate::chrome_ui::*;

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
        self.cached_scrollable_height =
            crate::scroll::compute_scrollable_height(self.root_box.as_ref());
        self.page_dirty = true;
        self.display_list_cache.lock().unwrap().invalidate(None);

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
