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
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_LAYER_ID: AtomicU64 = AtomicU64::new(1);

fn next_layer_id() -> u64 {
    NEXT_LAYER_ID.fetch_add(1, Ordering::Relaxed)
}

/// Classification of a rendering layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    /// Browser chrome (tab strip, toolbar, address bar, bookmark bar, status bar).
    Chrome,
    /// Document content viewport (backed by a tile rasterization grid).
    Page,
    /// Transient overlays (menus, modal dialogs, autocomplete popup, find bar).
    Overlay,
    /// Element promoted to an independent compositor layer (e.g. `will-change: transform`).
    Promoted(LayerPromotionReason),
}

/// Criteria or trigger that prompted promotion of an element to an isolated compositor layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerPromotionReason {
    WillChangeTransform,
    WillChangeOpacity,
    WillChangeScrollPosition,
    ThreeDTransform,
    CanvasWebGL,
    VideoElement,
    FixedPosition,
}

/// A discrete renderable layer in the browser window.
#[derive(Debug, Clone)]
pub struct RenderLayer {
    /// Stable identity for compositor resource caches across vector reorderings.
    pub id: u64,
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
    pub promotion_reason: Option<LayerPromotionReason>,
}

impl RenderLayer {
    /// Creates a chrome UI layer matching the window dimensions.
    pub fn new_chrome(width: u32, height: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let num_pixels = (w * h) as usize;
        Self {
            id: next_layer_id(),
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
            promotion_reason: None,
        }
    }

    /// Creates a tiled document page layer.
    pub fn new_page(width: u32, height: u32, tile_size: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let mut grid = TileGrid::new(tile_size);
        grid.update_content_size(w as f32, h as f32);

        Self {
            id: next_layer_id(),
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
            promotion_reason: None,
        }
    }

    /// Creates an overlay layer (e.g. popup menu, devtools).
    pub fn new_overlay(bounds: Rect) -> Self {
        let w = bounds.width().max(1.0) as u32;
        let h = bounds.height().max(1.0) as u32;
        let num_pixels = (w * h) as usize;

        Self {
            id: next_layer_id(),
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
            promotion_reason: None,
        }
    }

    /// Creates a layer promoted for hardware acceleration (e.g. `will-change`).
    pub fn new_promoted(bounds: Rect, reason: LayerPromotionReason) -> Self {
        let w = bounds.width().max(1.0) as u32;
        let h = bounds.height().max(1.0) as u32;
        let num_pixels = (w * h) as usize;

        Self {
            id: next_layer_id(),
            kind: LayerKind::Promoted(reason),
            bounds,
            scroll_offset: (0.0, 0.0),
            display_list: DisplayList::new(),
            damage: DamageRegion::from_rect(bounds),
            tile_grid: None,
            pixels: Some(vec![0; num_pixels]),
            is_dirty: true,
            opacity: 1.0,
            version: 0,
            promotion_reason: Some(reason),
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

    /// Clears any cached rasterized tiles to immediately free memory.
    pub fn clear_tiles(&mut self) {
        if let Some(grid) = &mut self.tile_grid {
            grid.clear();
        }
    }

    /// Rasterizes any dirty regions in the layer.
    pub fn rasterize_if_dirty(&mut self, bg_color: Color, current_frame: u64) -> bool {
        self.rasterize_if_dirty_limited(bg_color, current_frame, usize::MAX)
    }

    /// Rasterizes at most `max_page_tiles` page tiles for incremental window
    /// frames. The general `rasterize_if_dirty` API remains a complete pass.
    pub fn rasterize_if_dirty_limited(
        &mut self,
        bg_color: Color,
        current_frame: u64,
        max_page_tiles: usize,
    ) -> bool {
        match self.kind {
            LayerKind::Page => {
                let mut pending = false;
                if let Some(grid) = &mut self.tile_grid {
                    let viewport = Rect::new(
                        self.scroll_offset.0,
                        self.scroll_offset.1,
                        self.bounds.width(),
                        self.bounds.height(),
                    );
                    grid.ensure_tiles_rasterized_limited(
                        viewport,
                        &self.display_list,
                        bg_color,
                        current_frame,
                        max_page_tiles,
                    );
                    grid.prune_unused_tiles(current_frame, 15);
                    pending = grid.has_dirty_visible_tiles(viewport);
                }
                self.version = self.version.wrapping_add(1);
                if pending {
                    self.is_dirty = true;
                } else {
                    self.clear_damage();
                }
                pending
            }
            LayerKind::Chrome | LayerKind::Overlay | LayerKind::Promoted(_) => {
                if !self.is_dirty && self.damage.is_empty() {
                    return false;
                }
                if let Some(pixels) = &mut self.pixels {
                    let w = self.bounds.width().max(1.0) as u32;
                    let h = self.bounds.height().max(1.0) as u32;
                    if pixels.len() != (w * h) as usize {
                        pixels.resize((w * h) as usize, 0);
                        self.damage = DamageRegion::full(w as f32, h as f32);
                    }

                    let fill_color = match self.kind {
                        LayerKind::Overlay | LayerKind::Promoted(_) => 0x0000_0000,
                        _ => bg_color.to_rgb_u32(),
                    };

                    if self.damage.len() > 8 || self.damage.is_empty() {
                        pixels.fill(fill_color);
                        paint(&self.display_list, pixels, w, h);
                    } else {
                        for rect in self.damage.rects() {
                            let rx0 = (rect.x().max(0.0) as u32).min(w);
                            let rx1 = (rect.right().max(0.0).ceil() as u32).min(w);
                            let ry0 = (rect.y().max(0.0) as u32).min(h);
                            let ry1 = (rect.bottom().max(0.0).ceil() as u32).min(h);
                            for y in ry0..ry1 {
                                let row_start = (y * w + rx0) as usize;
                                let row_end = (y * w + rx1) as usize;
                                pixels[row_start..row_end].fill(fill_color);
                            }
                        }
                        paint_damage(&self.display_list, pixels, w, h, &self.damage);
                    }
                }
                self.version = self.version.wrapping_add(1);
                self.clear_damage();
                false
            }
        }
    }
}

/// The hierarchical compositor layer tree managing all top-level browser layers.
#[derive(Debug, Clone)]
pub struct LayerTree {
    pub chrome_layer: RenderLayer,
    pub page_layer: RenderLayer,
    pub overlay_layer: Option<RenderLayer>,
    pub promoted_layers: Vec<RenderLayer>,
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
            promoted_layers: Vec::new(),
            width,
            height,
        }
    }

    /// Determines whether a CSS `will-change` value should trigger layer promotion.
    pub fn should_promote(will_change: &str) -> Option<LayerPromotionReason> {
        for token in will_change.split(',') {
            match token.trim().to_ascii_lowercase().as_str() {
                "transform" => return Some(LayerPromotionReason::WillChangeTransform),
                "opacity" => return Some(LayerPromotionReason::WillChangeOpacity),
                "scroll-position" => return Some(LayerPromotionReason::WillChangeScrollPosition),
                _ => {}
            }
        }
        None
    }

    /// Promotes an element to its own hardware-accelerated layer.
    pub fn promote_layer(&mut self, bounds: Rect, reason: LayerPromotionReason) -> usize {
        let layer = RenderLayer::new_promoted(bounds, reason);
        self.promoted_layers.push(layer);
        self.promoted_layers.len() - 1
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
        self.promoted_layers.clear();
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
        for layer in &mut self.promoted_layers {
            layer.invalidate_all();
        }
    }

    /// Clears all cached page tiles to free memory upon navigation.
    pub fn clear_page_tiles(&mut self) {
        self.page_layer.clear_tiles();
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

    #[test]
    fn test_will_change_layer_promotion() {
        assert_eq!(
            LayerTree::should_promote("transform"),
            Some(LayerPromotionReason::WillChangeTransform)
        );
        assert_eq!(
            LayerTree::should_promote("opacity, filter"),
            Some(LayerPromotionReason::WillChangeOpacity)
        );
        assert_eq!(
            LayerTree::should_promote("scroll-position"),
            Some(LayerPromotionReason::WillChangeScrollPosition)
        );
        assert_eq!(LayerTree::should_promote("auto"), None);

        let mut tree = LayerTree::new(800, 600);
        let idx = tree.promote_layer(
            Rect::new(10.0, 10.0, 100.0, 100.0),
            LayerPromotionReason::WillChangeTransform,
        );
        assert_eq!(idx, 0);
        assert_eq!(tree.promoted_layers.len(), 1);
        assert_eq!(
            tree.promoted_layers[0].kind,
            LayerKind::Promoted(LayerPromotionReason::WillChangeTransform)
        );
    }

    #[test]
    fn test_page_layer_scrolling_rasterizes_tiles() {
        let mut layer = RenderLayer::new_page(800, 600, 256);
        layer.clear_damage();
        assert!(!layer.is_dirty);
        assert!(layer.damage.is_empty());

        // Initial rasterize
        layer.rasterize_if_dirty(Color::rgb(255, 255, 255), 1);
        let grid = layer.tile_grid.as_ref().unwrap();
        assert!(grid.get_tile(crate::tile::TileCoord::new(0, 0)).is_some());
        // Tile at row 15 (y = 15 * 256 = 3840) should not be rasterized yet
        assert!(grid.get_tile(crate::tile::TileCoord::new(0, 15)).is_none());

        // Now scroll to y = 3840 WITHOUT setting is_dirty or damage
        layer.scroll_offset = (0.0, 3840.0);
        layer
            .tile_grid
            .as_mut()
            .unwrap()
            .update_content_size(800.0, 5000.0);
        layer.rasterize_if_dirty(Color::rgb(255, 255, 255), 200);

        let grid = layer.tile_grid.as_ref().unwrap();
        // Tile at row 15 must now be rasterized!
        assert!(grid.get_tile(crate::tile::TileCoord::new(0, 15)).is_some());
        // Tile (0, 0) idle for 199 frames (> 120 max_idle) must be pruned to save RAM!
        assert!(grid.get_tile(crate::tile::TileCoord::new(0, 0)).is_none());
    }
}
