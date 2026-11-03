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

#[test]
fn test_github_simulation_memory_and_cpu() {
    let mut html = std::fs::read_to_string("/tmp/github.html").unwrap_or_else(|_| {
        let mut s = String::from("<!DOCTYPE html><html><head><title>GitHub Idle Sim</title></head><body>");
        for i in 0..500 {
            s.push_str(&format!(
                "<div id='card-{}' class='box' style='padding: 8px; margin: 4px; border: 1px solid #ccc;'>\
                 <h3>Header {}</h3><p>Description text for card {}</p><a href='#'>Link</a></div>",
                i, i, i
            ));
        }
        s.push_str("</body></html>");
        s
    });

    html.push_str(
        r#"<script>
        function rafLoop() {
            requestAnimationFrame(rafLoop);
        }
        requestAnimationFrame(rafLoop);

        setInterval(function() {
            var t = Date.now();
        }, 50);
    </script>"#,
    );

    let start_rss = get_rss_kb();
    println!("Start RSS: {:.2} MB", start_rss as f64 / 1024.0);

    let mut browser = BrowserChrome::new(1280, 900);
    let start_load = Instant::now();
    browser.load_html(html, "https://github.com".to_string());
    println!(
        "Load time: {:.2}s, RSS: {:.2} MB",
        start_load.elapsed().as_secs_f64(),
        get_rss_kb() as f64 / 1024.0
    );

    let t_all = Instant::now();
    for tick in 1..=200 {
        let t0 = Instant::now();
        let mutated = browser.tick_js();
        let js_time = t0.elapsed();
        let relayout_done = if mutated {
            browser.flush_pending_layout()
        } else {
            false
        };
        let relayout_time = t0.elapsed() - js_time;
        let t_dl = Instant::now();
        if relayout_done {
            let _ = browser.build_display_list((1280, 900));
        }
        let dl_time = t_dl.elapsed();
        if tick % 20 == 0 || tick <= 5 {
            println!(
                "Tick {:>3}: JS: {:>6.2}ms, Relayout: {:>6.2}ms (done={}), DL: {:>6.2}ms, RSS: {:>6.2} MB, V8 Heap: {:>6.2} MB",
                tick,
                js_time.as_secs_f64() * 1000.0,
                relayout_time.as_secs_f64() * 1000.0,
                relayout_done,
                dl_time.as_secs_f64() * 1000.0,
                get_rss_kb() as f64 / 1024.0,
                browser.js_runtime_heap_size() as f64 / (1024.0 * 1024.0),
            );
        }
        if tick > 1 {
            assert!(
                !relayout_done,
                "Timer tick without DOM mutations must not trigger relayout!"
            );
        }
    }
    println!("200 ticks elapsed: {:.4}s", t_all.elapsed().as_secs_f64());
}
