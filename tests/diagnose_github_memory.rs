use mango_browser::browser::BrowserChrome;
use std::time::{Duration, Instant};

fn get_vm_rss_kb() -> usize {
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

#[test]
#[ignore]
fn test_diagnose_github_live_memory() {
    println!("\n=== DIAGNOSTIC: Navigating to https://github.com ===");
    let start_rss = get_vm_rss_kb();
    println!("Initial RSS: {:.2} MB", start_rss as f64 / 1024.0);

    let mut browser = BrowserChrome::new(1280, 900);
    browser.navigate("https://github.com");
    browser.wait_for_navigation();

    let post_nav_rss = get_vm_rss_kb();
    println!("Post-Nav RSS: {:.2} MB", post_nav_rss as f64 / 1024.0);

    let start = Instant::now();
    let duration = Duration::from_secs(12);

    while start.elapsed() < duration {
        let t_nav_start = Instant::now();
        let _ = browser.tick_navigation();
        let t_nav = t_nav_start.elapsed();

        let t_anim_start = Instant::now();
        let _ = browser.tick_animations(16.0);
        let t_anim = t_anim_start.elapsed();

        let t_js_start = Instant::now();
        let mutated = browser.tick_js();
        let t_js = t_js_start.elapsed();

        let t_layout_start = Instant::now();
        let relayout = browser.flush_pending_layout();
        let _ = browser.tick_async_layout();
        let t_layout = t_layout_start.elapsed();

        let t_img_start = Instant::now();
        let _ = browser.drain_pending_images();
        let t_img = t_img_start.elapsed();

        let total = t_nav + t_anim + t_js + t_layout + t_img;
        if total > Duration::from_millis(50) || relayout || mutated {
            let rss = get_vm_rss_kb();
            println!(
                "[{:>5.2}s] RSS: {:>7.2} MB | TOTAL: {:>7.2}ms | nav: {:>6.2}ms | anim: {:>6.2}ms | JS: {:>7.2}ms (mut={}) | layout: {:>7.2}ms (rel={}) | img: {:>6.2}ms",
                start.elapsed().as_secs_f64(),
                rss as f64 / 1024.0,
                total.as_secs_f64() * 1000.0,
                t_nav.as_secs_f64() * 1000.0,
                t_anim.as_secs_f64() * 1000.0,
                t_js.as_secs_f64() * 1000.0,
                mutated,
                t_layout.as_secs_f64() * 1000.0,
                relayout,
                t_img.as_secs_f64() * 1000.0,
            );
        }

        std::thread::sleep(Duration::from_millis(16));
    }
}
