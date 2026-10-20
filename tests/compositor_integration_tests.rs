//! Integration tests for GPU Compositing, Tile Rasterization, and Damage-Rect Invalidation.

use mango_core::{Color, Rect};
use mango_render::SoftwareCompositor;
use mango_render::damage::DamageRegion;
use mango_render::display_list::{DisplayCommand, DisplayList};
use mango_render::layer::LayerTree;
use mango_render::painter::paint_with_clip;
use mango_render::tile::{DEFAULT_TILE_SIZE, TileCoord, TileGrid};

#[test]
fn test_damage_region_tracking_and_bounding() {
    let mut damage = DamageRegion::new();
    assert!(damage.is_empty());

    // Localized button hover damage at (10, 20) with size (100, 30)
    damage.add_rect(Rect::new(10.0, 20.0, 100.0, 30.0));
    assert_eq!(damage.len(), 1);
    assert_eq!(damage.bounds(), Some(Rect::new(10.0, 20.0, 100.0, 30.0)));

    // Second damage region for cursor blink at (200, 25) with size (2, 20)
    damage.add_rect(Rect::new(200.0, 25.0, 2.0, 20.0));
    assert_eq!(damage.len(), 2);

    let bounds = damage.bounds().expect("bounds should exist");
    assert_eq!(bounds.x(), 10.0);
    assert_eq!(bounds.y(), 20.0);
    assert!(bounds.right() >= 202.0);
    assert!(bounds.bottom() >= 50.0);

    // Test intersection
    assert!(damage.intersects(Rect::new(15.0, 25.0, 10.0, 10.0)));
    assert!(!damage.intersects(Rect::new(500.0, 500.0, 50.0, 50.0)));
}

#[test]
fn test_paint_with_clip_culls_out_of_bounds_commands() {
    let mut dl = DisplayList::new();

    // Command 1: Top-left button at (10, 10), 50x50, RED
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(10.0, 10.0, 50.0, 50.0),
        color: Color::rgb(255, 0, 0),
    });

    // Command 2: Far bottom-right box at (700, 700), 50x50, BLUE
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(700.0, 700.0, 50.0, 50.0),
        color: Color::rgb(0, 0, 255),
    });

    let mut buffer = vec![0u32; 800 * 800];

    // Paint restricted to top-left damage region (0..100, 0..100)
    let clip = Rect::new(0.0, 0.0, 100.0, 100.0);
    paint_with_clip(&dl, &mut buffer, 800, 800, clip);

    // Command 1 must be rasterized (pixel at (20, 20) is RED)
    let red_pixel = Color::rgb(255, 0, 0).to_rgb_u32();
    assert_eq!(buffer[20 * 800 + 20], red_pixel);

    // Command 2 must NOT have been rasterized (pixel at (720, 720) remains 0)
    assert_eq!(buffer[720 * 800 + 720], 0);
}

#[test]
fn test_tile_grid_incremental_rasterization_on_scroll() {
    let mut grid = TileGrid::new(DEFAULT_TILE_SIZE);
    grid.update_content_size(1024.0, 4000.0);

    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 1024.0, 4000.0),
        color: Color::rgb(240, 240, 240),
    });

    // Initial viewport at scroll_y = 0: (0..1024, 0..768)
    let vp0 = Rect::new(0.0, 0.0, 1024.0, 768.0);
    grid.ensure_tiles_rasterized(vp0, &dl, Color::rgb(255, 255, 255), 1);

    let initial_tile_count = grid.len();
    assert!(initial_tile_count > 0);

    // Check that top-left tile is clean
    let top_left = grid
        .get_tile(TileCoord::new(0, 0))
        .expect("tile (0, 0) should exist");
    assert!(!top_left.dirty);
    let top_left_version = top_left.version;

    // Small scroll: scroll down 100px. Viewport becomes (0..1024, 100..868)
    let vp1 = Rect::new(0.0, 100.0, 1024.0, 768.0);
    grid.ensure_tiles_rasterized(vp1, &dl, Color::rgb(255, 255, 255), 2);

    // Tile (0, 0) was NOT re-rasterized because it wasn't dirty!
    let top_left_after = grid.get_tile(TileCoord::new(0, 0)).unwrap();
    assert_eq!(top_left_after.version, top_left_version);
    assert!(!top_left_after.dirty);

    // Invalidate a specific 50x50 region in tile (0, 0)
    grid.invalidate_rect(Rect::new(20.0, 20.0, 50.0, 50.0));
    assert!(grid.get_tile(TileCoord::new(0, 0)).unwrap().dirty);

    // Re-rasterizing only updates the dirty tile
    grid.ensure_tiles_rasterized(vp1, &dl, Color::rgb(255, 255, 255), 3);
    assert!(!grid.get_tile(TileCoord::new(0, 0)).unwrap().dirty);
    assert_eq!(
        grid.get_tile(TileCoord::new(0, 0)).unwrap().version,
        top_left_version + 1
    );
}

#[test]
fn test_layer_tree_separation_and_compositor() {
    let mut layers = LayerTree::new(800, 600);
    layers.page_layer.bounds = Rect::new(0.0, 80.0, 800.0, 500.0);

    // Populate page content
    let mut page_dl = DisplayList::new();
    page_dl.push(DisplayCommand::FillRect {
        rect: Rect::new(50.0, 50.0, 200.0, 200.0),
        color: Color::rgb(0, 200, 0), // Green content
    });
    layers.page_layer.display_list = page_dl;

    // Populate chrome UI (tab bar at top)
    let mut chrome_dl = DisplayList::new();
    chrome_dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 800.0, 40.0),
        color: Color::rgb(40, 44, 52), // Dark chrome
    });
    layers.chrome_layer.display_list = chrome_dl;

    // Rasterize both layers
    layers
        .chrome_layer
        .rasterize_if_dirty(Color::TRANSPARENT, 1);
    layers.page_layer.rasterize_if_dirty(Color::WHITE, 1);

    assert!(!layers.chrome_layer.is_dirty);
    assert!(!layers.page_layer.is_dirty);

    // Invalidate page only
    layers.invalidate_page(Rect::new(60.0, 60.0, 10.0, 10.0));
    assert!(layers.page_layer.is_dirty);
    assert!(!layers.chrome_layer.is_dirty); // Chrome remains pristine!

    // Composite into window surface buffer
    let mut screen = vec![0u32; 800 * 600];
    let compositor = SoftwareCompositor::new();
    compositor.composite(&layers, &mut screen, 800, 600, Color::WHITE);

    // Chrome tab bar at (10, 10) must be dark chrome color
    let dark_chrome = Color::rgb(40, 44, 52).to_rgb_u32();
    assert_eq!(screen[10 * 800 + 10], dark_chrome);

    // Page content at (100, 150) must be visible green and not overwritten by chrome layer
    let green_content = Color::rgb(0, 200, 0).to_rgb_u32();
    assert_eq!(screen[150 * 800 + 100], green_content);
}

#[test]
fn test_compositor_handles_black_text_and_transparent_chrome_viewport() {
    let mut layers = LayerTree::new(800, 600);
    layers.page_layer.bounds = Rect::new(0.0, 100.0, 800.0, 480.0);

    // Page contains solid black text / rectangle (0x00000000)
    let mut page_dl = DisplayList::new();
    page_dl.push(DisplayCommand::FillRect {
        rect: Rect::new(50.0, 50.0, 100.0, 50.0),
        color: Color::rgb(0, 0, 0), // Pure black
    });
    layers.page_layer.display_list = page_dl;

    // Chrome UI with dark toolbar at top (y = 0..100) and status bar at bottom (y = 580..600)
    let mut chrome_dl = DisplayList::new();
    chrome_dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 800.0, 100.0),
        color: Color::rgb(32, 33, 36),
    });
    chrome_dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 580.0, 800.0, 20.0),
        color: Color::rgb(32, 33, 36),
    });
    layers.chrome_layer.display_list = chrome_dl;

    layers
        .chrome_layer
        .rasterize_if_dirty(Color::TRANSPARENT, 1);
    layers.page_layer.rasterize_if_dirty(Color::WHITE, 1);

    let mut screen = vec![0xFFFFFFFFu32; 800 * 600];
    let compositor = SoftwareCompositor::new();
    compositor.composite(&layers, &mut screen, 800, 600, Color::WHITE);

    // 1. Top chrome toolbar at (10, 50) is dark chrome
    assert_eq!(screen[50 * 800 + 10], Color::rgb(32, 33, 36).to_rgb_u32());

    // 2. Pure black rectangle at page (50, 50) which is screen (50, 150) is solid black (0)
    assert_eq!(screen[150 * 800 + 50], Color::BLACK.to_rgb_u32());

    // 3. Page white background at (200, 150) is white
    assert_eq!(screen[150 * 800 + 200], Color::WHITE.to_rgb_u32());

    // 4. Bottom chrome status bar at (10, 590) is dark chrome
    assert_eq!(screen[590 * 800 + 10], Color::rgb(32, 33, 36).to_rgb_u32());
}
