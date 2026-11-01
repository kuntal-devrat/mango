use mango_browser::browser::BrowserChrome;
use std::time::Instant;

fn get_rss_kb() -> usize {
    #[cfg(target_os = "linux")]
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:")
                && let Some(val) = rest.split_whitespace().next()
                && let Ok(kb) = val.parse::<usize>()
            {
                return kb;
            }
        }
    }
    0
}

fn generate_large_html(num_items: usize) -> String {
    let mut html = String::with_capacity(num_items * 300);
    html.push_str(r#"<!DOCTYPE html><html><head><style>
        body { margin: 0; padding: 20px; font-family: sans-serif; background: #f8f9fa; }
        .feed { display: block; max-width: 900px; margin: 0 auto; }
        .card { position: relative; background: #ffffff; border: 1px solid #e1e4e8; border-radius: 6px; padding: 16px; margin-bottom: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
        .badge { position: absolute; top: 12px; right: 12px; background: #0366d6; color: #ffffff; padding: 2px 6px; font-size: 11px; border-radius: 10px; }
        .title { font-size: 16px; font-weight: bold; margin-bottom: 8px; color: #24292e; }
        .desc { font-size: 14px; color: #586069; line-height: 1.5; }
        .tags { margin-top: 8px; display: flex; gap: 6px; }
        .tag { background: #f1f8ff; color: #0366d6; padding: 2px 8px; font-size: 12px; border-radius: 4px; }
    </style></head><body><div class="feed">"#);

    for i in 0..num_items {
        html.push_str(&format!(
            r#"<div class="card"><div class="badge">#{}</div><div class="title">Article Heading {}</div><div class="desc">This is a substantial paragraph of text inside article {} simulating realistic feed content on heavy news or social platforms.</div><div class="tags"><span class="tag">Rust</span><span class="tag">Browser</span><span class="tag">Performance</span></div></div>"#,
            i, i, i
        ));
    }

    html.push_str("</div></body></html>");
    html
}

#[test]
fn test_large_page_memory_and_display_list_metrics() {
    let initial_rss = get_rss_kb();
    let num_cards = 2500; // 2500 cards = ~17,500 DOM elements
    let html = generate_large_html(num_cards);
    let start_load = Instant::now();

    let mut browser = BrowserChrome::new(1280, 900);
    browser.load_html(html.clone(), "http://localhost/large-feed".to_string());
    let load_time = start_load.elapsed();

    let post_load_rss = get_rss_kb();
    let dl = browser.build_display_list((1280, 900));
    let post_dl_rss = get_rss_kb();

    let dl_commands = dl.len();
    let tab_mem = browser
        .active_tab_mut()
        .map(|t| t.approx_memory_usage())
        .unwrap_or(0);

    println!(
        "\n================ LARGE PAGE BENCHMARK (2500 cards / ~17,500 DOM elements) ================"
    );
    println!(
        "Initial Process RSS:       {:>8} KB ({:.2} MB)",
        initial_rss,
        initial_rss as f64 / 1024.0
    );
    println!(
        "Post-Load Process RSS:      {:>8} KB ({:.2} MB)",
        post_load_rss,
        post_load_rss as f64 / 1024.0
    );
    println!(
        "Post-DisplayList RSS:      {:>8} KB ({:.2} MB)",
        post_dl_rss,
        post_dl_rss as f64 / 1024.0
    );
    println!(
        "Net RSS Increase:          {:>8} KB ({:.2} MB)",
        post_dl_rss.saturating_sub(initial_rss),
        (post_dl_rss.saturating_sub(initial_rss)) as f64 / 1024.0
    );
    println!("Display List Commands:     {:>8}", dl_commands);
    let v8_heap = browser.js_runtime_heap_size();
    println!(
        "Tab approx memory:         {:>8} KB ({:.2} MB)",
        tab_mem / 1024,
        tab_mem as f64 / (1024.0 * 1024.0)
    );
    println!(
        "V8 Isolate Heap Size:      {:>8} KB ({:.2} MB)",
        v8_heap / 1024,
        v8_heap as f64 / (1024.0 * 1024.0)
    );
    println!(
        "Load & Layout Time:        {:>8.2} ms",
        load_time.as_secs_f64() * 1000.0
    );
    println!(
        "========================================================================================\n"
    );

    // Perform 3 relayout cycles (as when scrolling or animating)
    let start_relayout = Instant::now();
    for _ in 0..3 {
        browser.relayout();
        let _ = browser.build_display_list((1280, 900));
    }
    let relayout_time = start_relayout.elapsed();
    let post_relayout_rss = get_rss_kb();

    let v8_heap_post = browser.js_runtime_heap_size();
    println!(
        "Post 3x Relayout RSS:      {:>8} KB ({:.2} MB)",
        post_relayout_rss,
        post_relayout_rss as f64 / 1024.0
    );
    println!(
        "V8 Heap Post Relayout:     {:>8} KB ({:.2} MB)",
        v8_heap_post / 1024,
        v8_heap_post as f64 / (1024.0 * 1024.0)
    );
    println!(
        "Net RSS After Relayouts:   {:>8} KB ({:.2} MB)",
        post_relayout_rss.saturating_sub(initial_rss),
        (post_relayout_rss.saturating_sub(initial_rss)) as f64 / 1024.0
    );
    println!(
        "3x Relayout Time:          {:>8.2} ms",
        relayout_time.as_secs_f64() * 1000.0
    );
    println!(
        "========================================================================================\n"
    );
}
