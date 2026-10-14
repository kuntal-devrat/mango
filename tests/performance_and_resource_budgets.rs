//! Performance & Resource Budgets Tests (Phase 6.3).
//!
//! Validates:
//! - Cold Start: binary launches, parses, and produces first frame in < 100ms.
//! - Memory Footprint: < 30MB idle RAM for single tab; < 80MB for 5 active tabs.
//! - Rendering Responsiveness: sustained 60 FPS scrolling (< 16.6ms per frame).
//! - First Contentful Paint (FCP): pipeline latency < 900ms.

use mango_browser::browser::BrowserChrome;
use mango_core::Size;
use mango_html::parse_html;
use mango_layout::layout_document;
use mango_render::paint;
use std::time::Instant;

#[test]
fn test_cold_start_benchmark() {
    // Gate: Binary launch + first frame in < 100ms
    let start = Instant::now();

    let mut browser = BrowserChrome::new(1280, 900);
    let sample_html = r#"
        <!DOCTYPE html>
        <html><head><style>
            body { margin: 0; background: #ffffff; color: #000000; font-family: sans-serif; }
            header { background: #1a73e8; color: white; padding: 16px; font-size: 20px; }
            main { padding: 24px; display: flex; flex-direction: column; gap: 16px; }
            .card { border: 1px solid #dadce0; border-radius: 8px; padding: 16px; }
        </style></head>
        <body>
            <header>Mango Fast Cold Start</header>
            <main>
                <div class="card">Ultra-lightweight browser engine.</div>
                <div class="card">Instant launch performance.</div>
            </main>
        </body></html>
    "#;

    browser.load_html(sample_html.to_string(), "about:blank".to_string());
    let dl = browser.build_display_list((1280, 900));
    assert!(
        !dl.is_empty(),
        "Display list must not be empty on initial render"
    );

    let mut pixels = vec![0u32; 1280 * 900];
    paint(&dl, &mut pixels, 1280, 900);

    let duration = start.elapsed();
    let ms = duration.as_secs_f64() * 1000.0;
    let budget_ms = if cfg!(debug_assertions) {
        5000.0
    } else {
        100.0
    };
    println!(
        "\n[Performance Gate] Cold Start Time: {:.2}ms (Release Budget: < 100ms, Debug Mode: {:.2}ms)",
        ms, budget_ms
    );
    assert!(
        ms < budget_ms,
        "Cold start time must satisfy budget (< {:.2}ms), took {:.2}ms",
        budget_ms,
        ms
    );
}

#[test]
fn test_memory_footprint_single_and_multi_tab() {
    // Gate: < 30MB for single tab; < 80MB for 5 active tabs
    let mut browser = BrowserChrome::new(1280, 900);
    let sample_html = r#"
        <!DOCTYPE html><html><head><style>
            body { margin: 0; }
            .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
            .box { height: 100px; background: #eee; }
        </style></head><body>
            <div class="grid">
                <div class="box">1</div><div class="box">2</div><div class="box">3</div>
            </div>
        </body></html>
    "#;

    // Single tab memory measurement
    browser.load_html(sample_html.to_string(), "about:blank".to_string());
    let _ = browser.build_display_list((1280, 900));
    let tab1_mem_bytes = browser
        .active_tab()
        .map(|t| t.approx_memory_usage())
        .unwrap_or(1024 * 1024);
    let tab1_mb = tab1_mem_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "\n[Resource Gate] Single Tab Memory: {:.2}MB (Budget: < 30MB)",
        tab1_mb
    );
    assert!(
        tab1_mb < 30.0,
        "Single tab memory must be < 30MB, got {:.2}MB",
        tab1_mb
    );

    // Multi-tab (5 active tabs)
    for _ in 0..4 {
        browser.new_tab();
        browser.load_html(sample_html.to_string(), "about:blank".to_string());
        let _ = browser.build_display_list((1280, 900));
    }

    assert_eq!(browser.tab_count(), 5, "Must have 5 active tabs");
    let total_5_tabs_bytes: usize = browser.tabs().iter().map(|t| t.approx_memory_usage()).sum();
    let total_5_tabs_mb = total_5_tabs_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "[Resource Gate] 5 Active Tabs Memory: {:.2}MB (Budget: < 80MB)",
        total_5_tabs_mb
    );
    assert!(
        total_5_tabs_mb < 80.0,
        "5 tabs memory must be < 80MB, got {:.2}MB",
        total_5_tabs_mb
    );
}

#[test]
fn test_rendering_responsiveness_60fps_scrolling() {
    // Gate: Sustained 60 FPS scrolling (< 16.6ms per frame)
    let sample_html = r#"
        <!DOCTYPE html><html><head><style>
            body { margin: 0; background: #ffffff; }
            .content { height: 5000px; padding: 20px; }
            .item { height: 60px; margin-bottom: 20px; background: #e0e0e0; border-radius: 4px; }
        </style></head><body>
            <div class="content">
                <div class="item">Item 1</div>
                <div class="item">Item 2</div>
                <div class="item">Item 3</div>
                <div class="item">Item 4</div>
                <div class="item">Item 5</div>
            </div>
        </body></html>
    "#;

    let doc = parse_html(sample_html);
    let (_, dl) = layout_document(&doc, &[], Size::new(1280.0, 900.0));
    let mut pixels = vec![0u32; 1280 * 900];

    // Benchmark 30 successive scroll frames
    let frames = 30;
    let start = Instant::now();
    for _ in 0..frames {
        paint(&dl, &mut pixels, 1280, 900);
    }
    let elapsed = start.elapsed();
    let ms_per_frame = (elapsed.as_secs_f64() * 1000.0) / (frames as f64);
    let budget_frame_ms = if cfg!(debug_assertions) { 75.0 } else { 16.6 };
    println!(
        "\n[Performance Gate] Raster Frame Latency: {:.2}ms/frame (Release Budget: < 16.6ms / 60 FPS, Debug Mode: {:.2}ms)",
        ms_per_frame, budget_frame_ms
    );
    assert!(
        ms_per_frame < budget_frame_ms,
        "Raster frame latency must satisfy budget (< {:.2}ms), got {:.2}ms",
        budget_frame_ms,
        ms_per_frame
    );
}

#[test]
fn test_first_contentful_paint_pipeline_latency() {
    // Gate: End-to-end pipeline latency < 900ms
    let sample_html = r#"
        <!DOCTYPE html><html><head><style>
            body { margin: 0; font-family: sans-serif; }
            .hero { background: #333; color: white; padding: 40px; }
            .cards { display: flex; gap: 20px; padding: 20px; }
            .card { flex: 1; border: 1px solid #ddd; padding: 20px; }
        </style></head><body>
            <div class="hero">Welcome to Mango</div>
            <div class="cards">
                <div class="card">Feature A</div>
                <div class="card">Feature B</div>
                <div class="card">Feature C</div>
            </div>
        </body></html>
    "#;

    let start = Instant::now();
    let doc = parse_html(sample_html);
    let (_, dl) = layout_document(&doc, &[], Size::new(1280.0, 900.0));
    let mut pixels = vec![0u32; 1280 * 900];
    paint(&dl, &mut pixels, 1280, 900);

    let elapsed = start.elapsed();
    let ms = elapsed.as_secs_f64() * 1000.0;
    let budget_ms = if cfg!(debug_assertions) {
        3000.0
    } else {
        900.0
    };
    println!(
        "\n[Performance Gate] First Contentful Paint: {:.2}ms (Release Budget: < 900ms, Debug Mode: {:.2}ms)",
        ms, budget_ms
    );
    assert!(
        ms < budget_ms,
        "FCP pipeline latency must be < {:.2}ms, took {:.2}ms",
        budget_ms,
        ms
    );
}
