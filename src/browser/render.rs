//! Page layout, styling, display list construction, and rendering.

use std::collections::HashMap;
use mango_core::{Color, Rect, Size};
use mango_css::computed::ComputedStyle;
use mango_css::Stylesheet;
use mango_html::parse_html;
use mango_layout::layout_document;
use mango_render::display_list::{BorderWidths, DisplayCommand, DisplayList};
use mango_render::{FontFamily, FontWeight};
use crate::chrome_ui::*;
use crate::form_handler::*;
use super::BrowserChrome;
use super::forms::truncate_label;
use super::types::DocumentView;

pub(crate) fn canvas_background_color(root: &mango_layout::box_tree::LayoutBox) -> Color {
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
                .lock()
                .unwrap()
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
                    if translated_y + rect.height() >= content_y
                        && translated_y <= content_y + content_h
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
                    }
                }
                DisplayCommand::DrawBorder {
                    rect,
                    color,
                    widths,
                    radii,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    if translated_y + rect.height() >= content_y
                        && translated_y <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawBorder {
                            rect: Rect::new(
                                rect.x(),
                                translated_y,
                                rect.width(),
                                rect.height(),
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

        // Render Find-in-Page match highlights
        if self.find_in_page.active && !self.find_in_page.matches.is_empty() {
            for (i, m) in self.find_in_page.matches.iter().enumerate() {
                let trans_y = m.y() + content_y - self.scroll_y;
                let clamped_top = trans_y.max(content_y);
                let clamped_bottom = (trans_y + m.height()).min(content_y + content_h);
                if clamped_bottom > clamped_top {
                    let is_curr = i == self.find_in_page.current_match;
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: Rect::new(
                            m.x() - 2.0,
                            clamped_top,
                            m.width() + 4.0,
                            clamped_bottom - clamped_top,
                        ),
                        color: if is_curr {
                            Color::rgba(255, 140, 0, 220)
                        } else {
                            Color::rgba(255, 235, 59, 150)
                        },
                        radii: [3.0; 4],
                    });
                }
            }
        }

        dl.push(DisplayCommand::PopClip);

        // Append Chrome UI display commands
        let chrome_dl = self.build_chrome_display_list((width, height));
        for cmd in chrome_dl.into_iter() {
            dl.push(cmd);
        }

        dl
    }

    /// Updates the layer tree with the latest chrome and page display lists,
    /// driving incremental tile rasterization and damage repair.
    pub fn update_layers(&mut self, (width, height): (u32, u32)) {
        if self.width != width || self.height != height {
            self.resize(width, height);
        }
        self.frame_counter = self.frame_counter.wrapping_add(1);
        let w = width as f32;
        let h = height as f32;
        let content_y = HEADER_HEIGHT + 1.0;
        let content_h = (h - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let scroll_h = self.scrollable_height();
        let chrome_dl = self.build_chrome_display_list((width, height));

        // 1. Page Content Layer (unscrolled coordinates, tiled)
        self.layer_tree.page_layer.bounds = Rect::new(0.0, content_y, w, content_h);
        self.layer_tree.page_layer.scroll_offset = (0.0, self.scroll_y);

        if self.page_dirty || self.layer_tree.page_layer.display_list.is_empty() {
            let page_raw = if !self.page_display_list.is_empty() {
                self.page_display_list.clone()
            } else if let Some(root) = &self.root_box {
                let dl = mango_layout::display_list::build_display_list(root);
                self.page_display_list = dl.clone();
                dl
            } else {
                DisplayList::new()
            };
            self.layer_tree.page_layer.display_list = page_raw;
            self.layer_tree.page_layer.invalidate_all();
            self.page_dirty = false;
        }

        if let Some(grid) = &mut self.layer_tree.page_layer.tile_grid {
            grid.update_content_size(w, scroll_h);
        }

        // 2. Chrome UI Layer
        if self.layer_tree.chrome_layer.display_list != chrome_dl {
            self.layer_tree.chrome_layer.display_list = chrome_dl;
            self.layer_tree.chrome_layer.invalidate_all();
        }

        // 3. Rasterize dirty regions
        self.layer_tree
            .chrome_layer
            .rasterize_if_dirty(Color::TRANSPARENT, self.frame_counter);
        self.layer_tree
            .page_layer
            .rasterize_if_dirty(self.canvas_bg, self.frame_counter);

        // 4. Stale tile eviction
        if let Some(grid) = &mut self.layer_tree.page_layer.tile_grid {
            grid.prune_unused_tiles(self.frame_counter, 300);
        }
    }

    /// Marks a specific rectangle in the page content area as damaged.
    pub fn invalidate_page_rect(&mut self, rect: Rect) {
        self.layer_tree.page_layer.invalidate_rect(rect);
    }

    /// Marks a specific rectangle in the chrome area as damaged.
    pub fn invalidate_chrome_rect(&mut self, rect: Rect) {
        self.layer_tree.chrome_layer.invalidate_rect(rect);
    }

    /// Builds the display list for the browser chrome UI (scrollbar, tab strip,
    /// toolbar, omnibox, bookmark bar, status bar, popups, and devtools).
    pub fn build_chrome_display_list(&self, (width, height): (u32, u32)) -> DisplayList {
        let mut dl = DisplayList::new();
        let w = width as f32;
        let h = height as f32;

        let content_y = HEADER_HEIGHT + 1.0;
        let content_h = (h - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);

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

        // ── FIND-IN-PAGE FLOATING OVERLAY (Ctrl+F) ──
        if self.find_in_page.active {
            let bar_w = 320.0f32;
            let bar_h = 38.0f32;
            let bar_x = (w - bar_w - 24.0).max(10.0);
            let bar_y = content_y + 10.0;

            // Background card with shadow & border
            dl.push(DisplayCommand::FillRoundedRect {
                rect: Rect::new(bar_x - 1.0, bar_y - 1.0, bar_w + 2.0, bar_h + 2.0),
                color: Color::rgba(0, 0, 0, 40),
                radii: [8.0; 4],
            });
            dl.push(DisplayCommand::FillRoundedRect {
                rect: Rect::new(bar_x, bar_y, bar_w, bar_h),
                color: Color::rgb(255, 255, 255),
                radii: [7.0; 4],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: Rect::new(bar_x, bar_y, bar_w, bar_h),
                widths: BorderWidths::all(1.0),
                color: Color::rgb(218, 220, 224),
                radii: [7.0; 4],
            });

            // Search query text or placeholder
            let query_display = if self.find_in_page.query.is_empty() {
                "Find in page..."
            } else {
                &self.find_in_page.query
            };
            let text_color = if self.find_in_page.query.is_empty() {
                Color::rgb(140, 145, 155)
            } else {
                Color::rgb(32, 33, 36)
            };
            dl.push(DisplayCommand::draw_text(
                query_display,
                bar_x + 12.0,
                bar_y + 11.0,
                text_color,
                14.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
            ));

            // Match count badge e.g. "1/5" or "0/0"
            let badge_str = if self.find_in_page.query.is_empty() {
                String::new()
            } else if self.find_in_page.matches.is_empty() {
                "0/0".to_string()
            } else {
                format!(
                    "{}/{}",
                    self.find_in_page.current_match + 1,
                    self.find_in_page.matches.len()
                )
            };
            if !badge_str.is_empty() {
                let badge_w = mango_layout::measure_text_width(&badge_str, 12.0);
                let badge_x = bar_x + bar_w - 95.0 - badge_w;
                dl.push(DisplayCommand::draw_text(
                    &badge_str,
                    badge_x,
                    bar_y + 12.0,
                    if self.find_in_page.matches.is_empty() {
                        Color::rgb(217, 48, 37)
                    } else {
                        Color::rgb(95, 99, 104)
                    },
                    12.0,
                    FontWeight::Bold,
                    FontFamily::SansSerif,
                ));
            }

            // Prev button (▲)
            dl.push(DisplayCommand::draw_text(
                "▲",
                bar_x + bar_w - 80.0,
                bar_y + 12.0,
                Color::rgb(95, 99, 104),
                12.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));

            // Next button (▼)
            dl.push(DisplayCommand::draw_text(
                "▼",
                bar_x + bar_w - 55.0,
                bar_y + 12.0,
                Color::rgb(95, 99, 104),
                12.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));

            // Close button (✕)
            dl.push(DisplayCommand::draw_text(
                "✕",
                bar_x + bar_w - 28.0,
                bar_y + 11.0,
                Color::rgb(95, 99, 104),
                14.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));
        }

        dl
    }

}
