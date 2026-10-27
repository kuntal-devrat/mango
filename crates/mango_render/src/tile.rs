//! Hardware-accelerated tile rasterization grid and caching.
//!
//! Divides large, scrollable content surfaces into fixed-size grid tiles
//! (e.g. 256x256 px). Only visible and recently accessed tiles are rasterized
//! and cached in memory or on the GPU. When scrolling occurs, unchanged tiles
//! are blitted or composited with zero re-layout or re-rasterization cost.

use std::collections::HashMap;

use mango_core::{Color, Rect};

use crate::display_list::{DisplayCommand, DisplayList};
use crate::painter::paint;

/// Default tile dimension in pixels (width and height).
pub const DEFAULT_TILE_SIZE: u32 = 256;
/// Maximum number of rasterized tiles cached simultaneously (64 tiles * 256KB = 16MB max).
pub const MAX_CACHED_TILES: usize = 64;

/// 2D coordinate identifying a tile in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub col: u32,
    pub row: u32,
}

impl TileCoord {
    pub const fn new(col: u32, row: u32) -> Self {
        Self { col, row }
    }
}

/// A single rasterized tile within a content layer.
#[derive(Debug, Clone)]
pub struct Tile {
    pub coord: TileCoord,
    pub rect: Rect,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
    pub dirty: bool,
    pub version: u64,
    pub last_used_frame: u64,
}

impl Tile {
    /// Creates a new uninitialized tile for the given grid coordinate.
    pub fn new(coord: TileCoord, tile_size: u32) -> Self {
        Self::new_with_bg(coord, tile_size, Color::WHITE)
    }

    /// Creates a new tile initialized with the given background color.
    pub fn new_with_bg(coord: TileCoord, tile_size: u32, bg_color: Color) -> Self {
        let x = (coord.col * tile_size) as f32;
        let y = (coord.row * tile_size) as f32;
        let w = tile_size as f32;
        let h = tile_size as f32;

        let num_pixels = (tile_size * tile_size) as usize;
        Self {
            coord,
            rect: Rect::new(x, y, w, h),
            width: tile_size,
            height: tile_size,
            pixels: vec![bg_color.to_rgb_u32(); num_pixels],
            dirty: true,
            version: 0,
            last_used_frame: 0,
        }
    }

    /// Rasterizes the given display list commands intersecting this tile's bounds.
    pub fn rasterize(&mut self, display_list: &DisplayList, bg_color: Color) {
        // Clear tile backing buffer with the layer/canvas background color
        self.pixels.fill(bg_color.to_rgb_u32());

        // Prepare local display list translated into tile-local coordinates
        let mut tile_dl = DisplayList::new();
        tile_dl.push(DisplayCommand::PushTransform {
            matrix: [1.0, 0.0, 0.0, 1.0, -self.rect.x(), -self.rect.y()],
        });

        for cmd in display_list.iter() {
            match cmd {
                DisplayCommand::PushClip { .. }
                | DisplayCommand::PopClip
                | DisplayCommand::PushTransform { .. }
                | DisplayCommand::PopTransform
                | DisplayCommand::PushFilter { .. }
                | DisplayCommand::PopFilter
                | DisplayCommand::PushBackdropFilter { .. }
                | DisplayCommand::PopBackdropFilter
                | DisplayCommand::PushBlendMode { .. }
                | DisplayCommand::PopBlendMode
                | DisplayCommand::PushClipPath { .. }
                | DisplayCommand::PopClipPath => {
                    tile_dl.push(cmd.clone());
                }
                _ => {
                    if let Some(b) = cmd.bounds()
                        && !b.intersects(&self.rect)
                    {
                        continue;
                    }
                    tile_dl.push(cmd.clone());
                }
            }
        }

        tile_dl.push(DisplayCommand::PopTransform);

        paint(&tile_dl, &mut self.pixels, self.width, self.height);

        self.dirty = false;
        self.version = self.version.wrapping_add(1);
    }
}

/// Manages a 2D grid of cacheable tiles covering the document content area.
#[derive(Debug, Clone)]
pub struct TileGrid {
    tiles: HashMap<TileCoord, Tile>,
    tile_size: u32,
    content_width: f32,
    content_height: f32,
}

impl TileGrid {
    /// Creates a new tile grid with the specified tile size.
    pub fn new(tile_size: u32) -> Self {
        Self {
            tiles: HashMap::new(),
            tile_size: tile_size.max(64),
            content_width: 0.0,
            content_height: 0.0,
        }
    }

    /// Updates the total content dimensions covered by this tile grid.
    pub fn update_content_size(&mut self, width: f32, height: f32) {
        self.content_width = width.max(0.0);
        self.content_height = height.max(0.0);
    }

    /// Returns the tile dimensions in pixels.
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }

    /// Returns the number of currently allocated/cached tiles.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Returns `true` if there are no tiles in the grid.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Looks up an existing tile.
    pub fn get_tile(&self, coord: TileCoord) -> Option<&Tile> {
        self.tiles.get(&coord)
    }

    /// Marks all tiles intersecting the given damaged rectangle as dirty.
    pub fn invalidate_rect(&mut self, damage: Rect) {
        if damage.width() <= 0.0 || damage.height() <= 0.0 {
            return;
        }

        let start_col = (damage.x().max(0.0) / self.tile_size as f32).floor() as u32;
        let end_col = (damage.right().max(0.0) / self.tile_size as f32).ceil() as u32;
        let start_row = (damage.y().max(0.0) / self.tile_size as f32).floor() as u32;
        let end_row = (damage.bottom().max(0.0) / self.tile_size as f32).ceil() as u32;

        for col in start_col..end_col {
            for row in start_row..end_row {
                let coord = TileCoord::new(col, row);
                if let Some(tile) = self.tiles.get_mut(&coord) {
                    tile.dirty = true;
                }
            }
        }
    }

    /// Marks all tiles in the grid as dirty.
    pub fn invalidate_all(&mut self) {
        for tile in self.tiles.values_mut() {
            tile.dirty = true;
        }
    }

    /// Returns the grid coordinates of all tiles visible within the given viewport
    /// rectangle (including an optional prefetch margin).
    pub fn visible_tile_coords(&self, viewport: Rect, prefetch_margin: u32) -> Vec<TileCoord> {
        if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
            return Vec::new();
        }

        let margin_px = (prefetch_margin * self.tile_size) as f32;
        let expanded = Rect::new(
            (viewport.x() - margin_px).max(0.0),
            (viewport.y() - margin_px).max(0.0),
            viewport.width() + margin_px * 2.0,
            viewport.height() + margin_px * 2.0,
        );

        let start_col = (expanded.x() / self.tile_size as f32).floor() as u32;
        let end_col = (expanded.right() / self.tile_size as f32).ceil() as u32;
        let start_row = (expanded.y() / self.tile_size as f32).floor() as u32;
        let end_row = (expanded.bottom() / self.tile_size as f32).ceil() as u32;

        let max_col = if self.content_width > 0.0 {
            (self.content_width / self.tile_size as f32).ceil() as u32
        } else {
            end_col
        };
        let max_row = if self.content_height > 0.0 {
            (self.content_height / self.tile_size as f32).ceil() as u32
        } else {
            end_row
        };

        let mut coords = Vec::new();
        for col in start_col..end_col.min(max_col) {
            for row in start_row..end_row.min(max_row) {
                coords.push(TileCoord::new(col, row));
            }
        }
        coords
    }

    /// Ensures that all tiles visible within `viewport` are rasterized and up-to-date.
    ///
    /// Reuses already rasterized, non-dirty tiles with 0 rasterization cost.
    pub fn ensure_tiles_rasterized(
        &mut self,
        viewport: Rect,
        display_list: &DisplayList,
        bg_color: Color,
        current_frame: u64,
    ) {
        self.ensure_tiles_rasterized_limited(
            viewport,
            display_list,
            bg_color,
            current_frame,
            usize::MAX,
        );
    }

    /// Rasterizes at most `max_tiles` dirty visible tiles, prioritizing tiles
    /// nearest the viewport origin. Remaining tiles stay dirty and can be
    /// processed by a later frame without monopolizing the window thread.
    pub fn ensure_tiles_rasterized_limited(
        &mut self,
        viewport: Rect,
        display_list: &DisplayList,
        bg_color: Color,
        current_frame: u64,
        max_tiles: usize,
    ) {
        use rayon::prelude::*;

        let viewport_coords = self.visible_tile_coords(viewport, 0);
        let expanded_coords = self.visible_tile_coords(viewport, 1);
        let tile_size = self.tile_size;

        for &coord in &expanded_coords {
            let tile = self
                .tiles
                .entry(coord)
                .or_insert_with(|| Tile::new_with_bg(coord, tile_size, bg_color));

            tile.last_used_frame = current_frame;
        }

        let visible_set: std::collections::HashSet<TileCoord> =
            expanded_coords.iter().copied().collect();

        let mut selected_dirty = std::collections::HashSet::new();
        // Prioritize on-screen viewport tiles first
        for coord in &viewport_coords {
            if selected_dirty.len() >= max_tiles {
                break;
            }
            if self.tiles.get(coord).is_some_and(|tile| tile.dirty) {
                selected_dirty.insert(*coord);
            }
        }
        // Then off-screen pre-roll tiles within the remaining budget
        if selected_dirty.len() < max_tiles {
            for coord in &expanded_coords {
                if selected_dirty.len() >= max_tiles {
                    break;
                }
                if self.tiles.get(coord).is_some_and(|tile| tile.dirty) {
                    selected_dirty.insert(*coord);
                }
            }
        }
        let mut dirty_tiles: Vec<&mut Tile> = self
            .tiles
            .iter_mut()
            .filter(|(coord, tile)| selected_dirty.contains(coord) && tile.dirty)
            .map(|(_, tile)| tile)
            .collect();

        if dirty_tiles.len() >= 2 {
            dirty_tiles.par_iter_mut().for_each(|tile| {
                tile.rasterize(display_list, bg_color);
            });
        } else {
            for tile in dirty_tiles {
                tile.rasterize(display_list, bg_color);
            }
        }

        // Bounded tile eviction: ensure grid memory cannot grow uncontrollably
        if self.tiles.len() > MAX_CACHED_TILES {
            let mut entries: Vec<(TileCoord, u64)> = self
                .tiles
                .iter()
                .map(|(c, t)| (*c, t.last_used_frame))
                .collect();
            entries.sort_unstable_by_key(|&(_, last_used)| last_used);
            let to_remove = self.tiles.len().saturating_sub(MAX_CACHED_TILES);
            for (coord, _) in entries.into_iter().take(to_remove) {
                if !visible_set.contains(&coord) {
                    self.tiles.remove(&coord);
                }
            }
        }
    }

    /// Returns whether any tile intersecting the expanded visible area still
    /// needs rasterization.
    pub fn has_dirty_visible_tiles(&self, viewport: Rect) -> bool {
        self.visible_tile_coords(viewport, 1)
            .iter()
            .any(|coord| self.tiles.get(coord).is_some_and(|tile| tile.dirty))
    }

    /// Evicts tiles that have not been visible for `max_idle_frames` to keep RAM usage low.
    pub fn prune_unused_tiles(&mut self, current_frame: u64, max_idle_frames: u64) {
        self.tiles.retain(|_, tile| {
            current_frame.saturating_sub(tile.last_used_frame) <= max_idle_frames
        });
        if self.tiles.len() > MAX_CACHED_TILES {
            let mut entries: Vec<(TileCoord, u64)> = self
                .tiles
                .iter()
                .map(|(c, t)| (*c, t.last_used_frame))
                .collect();
            entries.sort_unstable_by_key(|&(_, last_used)| last_used);
            let to_remove = self.tiles.len().saturating_sub(MAX_CACHED_TILES);
            for (coord, _) in entries.into_iter().take(to_remove) {
                self.tiles.remove(&coord);
            }
        }
    }

    /// Clears all tiles.
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    /// Returns an iterator over all currently stored tiles.
    pub fn iter(&self) -> impl Iterator<Item = (&TileCoord, &Tile)> {
        self.tiles.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_creation_and_rasterize() {
        let coord = TileCoord::new(0, 0);
        let mut tile = Tile::new(coord, 64);
        assert!(tile.dirty);
        assert_eq!(tile.width, 64);
        assert_eq!(tile.height, 64);
        assert_eq!(tile.pixels.len(), 64 * 64);

        let mut dl = DisplayList::new();
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            color: Color::rgb(255, 0, 0),
        });

        tile.rasterize(&dl, Color::rgb(255, 255, 255));
        assert!(!tile.dirty);
        assert_eq!(tile.version, 1);

        // Check background pixel at (0, 0)
        assert_eq!(tile.pixels[0], Color::rgb(255, 255, 255).to_rgb_u32());
        // Check filled red pixel at (15, 15) -> 15 * 64 + 15 = 975
        assert_eq!(
            tile.pixels[15 * 64 + 15],
            Color::rgb(255, 0, 0).to_rgb_u32()
        );
    }

    #[test]
    fn test_tile_grid_visible_coords_and_invalidation() {
        let mut grid = TileGrid::new(256);
        grid.update_content_size(1024.0, 2048.0);

        let viewport = Rect::new(0.0, 0.0, 512.0, 512.0);
        let coords = grid.visible_tile_coords(viewport, 0);

        // Viewport (0..512, 0..512) with tile_size=256 covers col 0..=2, row 0..=2
        assert!(!coords.is_empty());
        assert!(coords.contains(&TileCoord::new(0, 0)));
        assert!(coords.contains(&TileCoord::new(1, 1)));

        let dl = DisplayList::new();
        grid.ensure_tiles_rasterized(viewport, &dl, Color::rgb(255, 255, 255), 1);
        assert!(!grid.is_empty());
        assert!(!grid.get_tile(TileCoord::new(0, 0)).unwrap().dirty);

        // Invalidate region overlapping tile (0, 0)
        grid.invalidate_rect(Rect::new(10.0, 10.0, 50.0, 50.0));
        assert!(grid.get_tile(TileCoord::new(0, 0)).unwrap().dirty);
    }
}
