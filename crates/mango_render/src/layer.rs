//! Compositor layers: separation of Chrome, Page content, and Overlays.
//!
//! Separating the static browser UI (tab bar, omnibox, toolbar) from dynamic
//! page content eliminates re-rasterizing the entire window during page scrolling,
//! cursor blinking, or text editing.

use mango_core::{Color, Rect};

use crate::damage::DamageRegion;
use crate::display_list::DisplayList;
use crate::painter::{paint, paint_damage};
use crate::tile::TileGrid;

/// Classification of a rendering layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    /// Browser chrome (tab strip, toolbar, address bar, bookmark bar, status bar).
    Chrome,
    /// Document content viewport (backed by a tile rasterization grid).
    Page,
    /// Transient overlays (menus, modal dialogs, autocomplete popup, find bar).
    Overlay,
}

/// A discrete renderable layer in the browser window.
#[derive(Debug, Clone)]
pub struct RenderLayer {
    pub kind: LayerKind,
    pub bounds: Rect,
    pub scroll_offset: (f32, f32),
    pub display_list: DisplayList,
    pub damage: DamageRegion,
    pub tile_grid: Option<TileGrid>,
    pub pixels: Option<Vec<u32>>,
    pub is_dirty: bool,
    pub opacity: f32,
    pub version: u64,
}

impl RenderLayer {
    /// Creates a chrome UI layer matching the window dimensions.
    pub fn new_chrome(width: u32, height: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let num_pixels = (w * h) as usize;
        Self {
            kind: LayerKind::Chrome,
            bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
            scroll_offset: (0.0, 0.0),
            display_list: DisplayList::new(),
            damage: DamageRegion::full(w as f32, h as f32),
            tile_grid: None,
            pixels: Some(vec![0; num_pixels]),
            is_dirty: true,
            opacity: 1.0,
            version: 0,
        }
    }

    /// Creates a tiled document page layer.
    pub fn new_page(width: u32, height: u32, tile_size: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let mut grid = TileGrid::new(tile_size);
        grid.update_content_size(w as f32, h as f32);

        Self {
            kind: LayerKind::Page,
            bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
            scroll_offset: (0.0, 0.0),
            display_list: DisplayList::new(),
            damage: DamageRegion::full(w as f32, h as f32),
            tile_grid: Some(grid),
            pixels: None,
            is_dirty: true,
            opacity: 1.0,
            version: 0,
        }
    }

    /// Creates an overlay layer (e.g. popup menu, devtools).
    pub fn new_overlay(bounds: Rect) -> Self {
        let w = bounds.width().max(1.0) as u32;
        let h = bounds.height().max(1.0) as u32;
        let num_pixels = (w * h) as usize;

        Self {
            kind: LayerKind::Overlay,
            bounds,
            scroll_offset: (0.0, 0.0),
            display_list: DisplayList::new(),
            damage: DamageRegion::from_rect(bounds),
            tile_grid: None,
            pixels: Some(vec![0; num_pixels]),
            is_dirty: true,
            opacity: 1.0,
            version: 0,
        }
    }

    /// Marks a specific rectangle within this layer as damaged.
    pub fn invalidate_rect(&mut self, rect: Rect) {
        self.damage.add_rect(rect);
        self.is_dirty = true;
        if let Some(grid) = &mut self.tile_grid {
            grid.invalidate_rect(rect);
        }
    }

    /// Marks the entire layer as damaged.
    pub fn invalidate_all(&mut self) {
        self.damage = DamageRegion::from_rect(self.bounds);
        self.is_dirty = true;
        if let Some(grid) = &mut self.tile_grid {
            grid.invalidate_all();
        }
    }

    /// Clears any accumulated damage rects after a paint cycle.
    pub fn clear_damage(&mut self) {
        self.damage.clear();
        self.is_dirty = false;
    }

    /// Rasterizes any dirty regions in the layer.
    pub fn rasterize_if_dirty(&mut self, bg_color: Color, current_frame: u64) {
        if !self.is_dirty && self.damage.is_empty() {
            return;
        }

        match self.kind {
            LayerKind::Page => {
                if let Some(grid) = &mut self.tile_grid {
                    let viewport = Rect::new(
                        self.scroll_offset.0,
                        self.scroll_offset.1,
                        self.bounds.width(),
                        self.bounds.height(),
                    );
                    grid.ensure_tiles_rasterized(
                        viewport,
                        &self.display_list,
                        bg_color,
                        current_frame,
                    );
                }
            }
            LayerKind::Chrome | LayerKind::Overlay => {
                if let Some(pixels) = &mut self.pixels {
                    let w = self.bounds.width().max(1.0) as u32;
                    let h = self.bounds.height().max(1.0) as u32;
                    if pixels.len() != (w * h) as usize {
                        pixels.resize((w * h) as usize, 0);
                        self.damage = DamageRegion::full(w as f32, h as f32);
                    }

                    if self.damage.len() > 8 || self.damage.is_empty() {
                        pixels.fill(bg_color.to_rgb_u32());
                        paint(&self.display_list, pixels, w, h);
                    } else {
                        paint_damage(&self.display_list, pixels, w, h, &self.damage);
                    }
                }
            }
        }

        self.version = self.version.wrapping_add(1);
        self.clear_damage();
    }
}

/// The hierarchical compositor layer tree managing all top-level browser layers.
#[derive(Debug, Clone)]
pub struct LayerTree {
    pub chrome_layer: RenderLayer,
    pub page_layer: RenderLayer,
    pub overlay_layer: Option<RenderLayer>,
    pub width: u32,
    pub height: u32,
}

impl LayerTree {
    /// Creates a new layer tree with the given window dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let chrome_layer = RenderLayer::new_chrome(width, height);
        let page_layer = RenderLayer::new_page(width, height, crate::tile::DEFAULT_TILE_SIZE);

        Self {
            chrome_layer,
            page_layer,
            overlay_layer: None,
            width,
            height,
        }
    }

    /// Resizes the layer tree and its backing layers.
    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        self.width = w;
        self.height = h;

        self.chrome_layer = RenderLayer::new_chrome(w, h);
        self.page_layer = RenderLayer::new_page(w, h, crate::tile::DEFAULT_TILE_SIZE);
        self.overlay_layer = None;
    }

    /// Marks a rectangle in the page layer as dirty.
    pub fn invalidate_page(&mut self, rect: Rect) {
        self.page_layer.invalidate_rect(rect);
    }

    /// Marks a rectangle in the chrome layer as dirty.
    pub fn invalidate_chrome(&mut self, rect: Rect) {
        self.chrome_layer.invalidate_rect(rect);
    }

    /// Marks all layers as dirty.
    pub fn invalidate_all(&mut self) {
        self.chrome_layer.invalidate_all();
        self.page_layer.invalidate_all();
        if let Some(overlay) = &mut self.overlay_layer {
            overlay.invalidate_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_creation_and_dirty_flags() {
        let layer_tree = LayerTree::new(800, 600);
        assert_eq!(layer_tree.chrome_layer.kind, LayerKind::Chrome);
        assert_eq!(layer_tree.page_layer.kind, LayerKind::Page);
        assert!(layer_tree.chrome_layer.is_dirty);
        assert!(layer_tree.page_layer.is_dirty);
    }

    #[test]
    fn test_layer_rasterize_and_damage_clearing() {
        let mut layer = RenderLayer::new_chrome(100, 100);
        assert!(layer.is_dirty);

        layer.rasterize_if_dirty(Color::rgb(255, 255, 255), 1);
        assert!(!layer.is_dirty);
        assert!(layer.damage.is_empty());
        assert_eq!(layer.version, 1);
    }
}
