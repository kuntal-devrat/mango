//! Layer tree management, tile rasterization, and damage invalidation.

use mango_core::{Color, Rect};
use mango_render::display_list::DisplayList;

use super::super::BrowserChrome;
use crate::chrome_ui::*;

impl BrowserChrome {
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
}
