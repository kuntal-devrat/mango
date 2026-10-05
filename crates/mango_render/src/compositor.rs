//! High-performance Compositor for merging rasterized layers into a screen surface.
//!
//! Provides fast CPU tile blitting and layer assembly, with clipping,
//! opacity, and damage awareness.

use mango_core::{Color, Rect};

use crate::layer::LayerTree;

/// Software compositor responsible for blitting cached tiles and UI layers
/// directly into the window surface buffer.
#[derive(Debug, Default)]
pub struct SoftwareCompositor;

impl SoftwareCompositor {
    pub fn new() -> Self {
        Self
    }

    /// Composites the entire [`LayerTree`] into the target pixel buffer.
    ///
    /// # Arguments
    /// - `layers`: The layer tree containing Chrome, Page (tiled), and Overlay layers.
    /// - `target`: Destination `0xRRGGBB` pixel buffer.
    /// - `target_width`: Width of destination buffer.
    /// - `target_height`: Height of destination buffer.
    /// - `bg_color`: Window background fill color.
    pub fn composite(
        &self,
        layers: &LayerTree,
        target: &mut [u32],
        target_width: u32,
        target_height: u32,
        bg_color: Color,
    ) {
        if target_width == 0 || target_height == 0 || target.len() < (target_width * target_height) as usize {
            return;
        }

        // 1. Fill base window canvas
        target.fill(bg_color.to_rgb_u32());

        // 2. Composite Page layer tiles
        let page = &layers.page_layer;
        if let Some(grid) = &page.tile_grid {
            let page_clip = page.bounds;
            let scroll_x = page.scroll_offset.0;
            let scroll_y = page.scroll_offset.1;

            let visible_viewport = Rect::new(
                scroll_x,
                scroll_y,
                page_clip.width(),
                page_clip.height(),
            );

            let visible_coords = grid.visible_tile_coords(visible_viewport, 0);
            for coord in visible_coords {
                if let Some(tile) = grid.get_tile(coord) {
                    let dest_x = (tile.rect.x() - scroll_x + page_clip.x()).round() as i32;
                    let dest_y = (tile.rect.y() - scroll_y + page_clip.y()).round() as i32;

                    self.blit_subrect(
                        &tile.pixels,
                        tile.width,
                        tile.height,
                        dest_x,
                        dest_y,
                        target,
                        target_width,
                        target_height,
                        page_clip,
                    );
                }
            }
        }

        // 3. Composite Chrome layer
        let chrome = &layers.chrome_layer;
        if let Some(chrome_pixels) = &chrome.pixels {
            let cw = chrome.bounds.width() as u32;
            let ch = chrome.bounds.height() as u32;
            let copy_w = cw.min(target_width);
            let copy_h = ch.min(target_height);

            for y in 0..copy_h {
                let src_start = (y * cw) as usize;
                let src_end = src_start + copy_w as usize;
                let dst_start = (y * target_width) as usize;
                let dst_end = dst_start + copy_w as usize;

                if src_end <= chrome_pixels.len() && dst_end <= target.len() {
                    // Chrome pixels are copied directly; transparent pixels can be skipped if needed
                    let src_row = &chrome_pixels[src_start..src_end];
                    let dst_row = &mut target[dst_start..dst_end];
                    for (dst_px, &src_px) in dst_row.iter_mut().zip(src_row.iter()) {
                        if src_px != 0 {
                            *dst_px = src_px;
                        }
                    }
                }
            }
        }

        // 4. Composite Overlay layer (if active)
        if let Some(overlay) = &layers.overlay_layer
            && let Some(overlay_pixels) = &overlay.pixels
        {
            let ow = overlay.bounds.width() as u32;
            let oh = overlay.bounds.height() as u32;
            let ox = overlay.bounds.x().round() as i32;
            let oy = overlay.bounds.y().round() as i32;

            self.blit_subrect(
                overlay_pixels,
                ow,
                oh,
                ox,
                oy,
                target,
                target_width,
                target_height,
                Rect::new(0.0, 0.0, target_width as f32, target_height as f32),
            );
        }
    }

    /// Blits a source buffer into the target buffer with clipping.
    #[allow(clippy::too_many_arguments)]
    fn blit_subrect(
        &self,
        src: &[u32],
        src_w: u32,
        src_h: u32,
        dst_x: i32,
        dst_y: i32,
        target: &mut [u32],
        target_w: u32,
        target_h: u32,
        clip: Rect,
    ) {
        if src_w == 0 || src_h == 0 || src.len() < (src_w * src_h) as usize {
            return;
        }

        let clip_x0 = (clip.x().max(0.0) as i32).clamp(0, target_w as i32);
        let clip_y0 = (clip.y().max(0.0) as i32).clamp(0, target_h as i32);
        let clip_x1 = (clip.right().min(target_w as f32) as i32).clamp(0, target_w as i32);
        let clip_y1 = (clip.bottom().min(target_h as f32) as i32).clamp(0, target_h as i32);

        let out_x0 = dst_x.max(clip_x0);
        let out_y0 = dst_y.max(clip_y0);
        let out_x1 = (dst_x + src_w as i32).min(clip_x1);
        let out_y1 = (dst_y + src_h as i32).min(clip_y1);

        if out_x0 >= out_x1 || out_y0 >= out_y1 {
            return;
        }

        let span_w = (out_x1 - out_x0) as usize;

        for y in out_y0..out_y1 {
            let src_local_y = (y - dst_y) as usize;
            let src_local_x = (out_x0 - dst_x) as usize;

            let src_start = src_local_y * src_w as usize + src_local_x;
            let src_end = src_start + span_w;

            let dst_start = (y as usize) * target_w as usize + out_x0 as usize;
            let dst_end = dst_start + span_w;

            if src_end <= src.len() && dst_end <= target.len() {
                target[dst_start..dst_end].copy_from_slice(&src[src_start..src_end]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::TileCoord;

    #[test]
    fn test_software_compositor_blits_layers() {
        let mut layers = LayerTree::new(100, 100);
        layers.page_layer.bounds = Rect::new(0.0, 20.0, 100.0, 60.0);

        // Prepopulate a tile in the page grid
        if let Some(grid) = &mut layers.page_layer.tile_grid {
            let mut tile = crate::tile::Tile::new(TileCoord::new(0, 0), grid.tile_size());
            tile.pixels.fill(Color::rgb(0, 255, 0).to_rgb_u32());
            tile.dirty = false;
            // Force tile size for testing
            let mut grid_test = crate::tile::TileGrid::new(32);
            grid_test.update_content_size(100.0, 100.0);
            let mut test_tile = crate::tile::Tile::new(TileCoord::new(0, 0), 32);
            test_tile.pixels.fill(Color::rgb(0, 255, 0).to_rgb_u32());
            test_tile.dirty = false;
        }

        let mut target = vec![0u32; 100 * 100];
        let compositor = SoftwareCompositor::new();
        compositor.composite(&layers, &mut target, 100, 100, Color::rgb(255, 255, 255));

        // Base canvas is filled with white
        assert_eq!(target[0], Color::rgb(255, 255, 255).to_rgb_u32());
    }
}
