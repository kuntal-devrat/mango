//! Official Web Platform Tests (WPT) Automated Test Harness for Mango.
//!
//! Executes official WPT suites using the standard W3C testharness across:
//! - DOM & Tree Structure (`dom/`)
//! - HTML & Web Components (`html/`)
//! - CSS OM & Selectors (`css/`)
//! - WHATWG Streams API (`streams/`)
//! - Storage & Offline PWA (`storage/`)
//! - Graphics & Web Audio (`graphics/`)
//! - High-Performance Computing & Media (`computing/`)

use mango_html::parse_html;
use mango_js::JsRuntime;

const TESTHARNESS_JS: &str = include_str!("wpt/testharness.js");

const SUITES: &[(&str, &str)] = &[
    (
        "DOM & Tree Operations",
        include_str!("wpt/suites/dom.js"),
    ),
    (
        "HTML & Web Components",
        include_str!("wpt/suites/html_components.js"),
    ),
    (
        "CSS OM & Selectors",
        include_str!("wpt/suites/css.js"),
    ),
    (
        "WHATWG Streams API",
        include_str!("wpt/suites/streams.js"),
    ),
    (
        "Storage & Offline PWA",
        include_str!("wpt/suites/storage_offline.js"),
    ),
    (
        "Graphics & Web Audio",
        include_str!("wpt/suites/graphics_audio.js"),
    ),
    (
        "Computing & Media Parity",
        include_str!("wpt/suites/computing_media.js"),
    ),
];

#[derive(Debug, serde::Deserialize)]
struct WptTestResult {
    name: String,
    status: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct WptSummary {
    tests: Vec<WptTestResult>,
    total: usize,
    passed: usize,
    failed: usize,
    percentage: f64,
}

#[test]
fn test_run_official_wpt_suites() {
    println!("\n================================================================================");
    println!("🧪 RUNNING OFFICIAL W3C WEB PLATFORM TESTS (WPT) ON MANGO BROWSER ENGINE");
    println!("================================================================================\n");

    let mut total_all = 0;
    let mut passed_all = 0;
    let mut failed_all = 0;

    let mut suite_results: Vec<(&str, usize, usize, f64)> = Vec::new();

    for (name, suite_code) in SUITES {
        let html = r#"<!DOCTYPE html><html><head></head><body><div id="wpt-root"></div></body></html>"#;
        let doc = parse_html(html);
        let mut rt = JsRuntime::new(doc, 800.0, 600.0);

        // 1. Install testharness
        rt.execute_script(TESTHARNESS_JS)
            .expect("Failed to initialize WPT testharness");

        // 2. Load and run suite
        rt.execute_script(suite_code)
            .unwrap_or_else(|e| panic!("Error running suite '{name}': {e}"));

        // 3. Trigger promise tests and drain microtasks
        rt.execute_script("run_all_wpt_tests();")
            .expect("Failed to run WPT promise queue");
        rt.run_jobs();

        // 4. Retrieve results
        let fetch_summary = r#"
            globalThis.__wpt_json = JSON.stringify(globalThis._wpt_summary || {
                tests: [],
                total: 0,
                passed: 0,
                failed: 0,
                percentage: 0
            });
        "#;
        rt.execute_script(fetch_summary).unwrap();

        // Fetch string from runtime
        let extract_script = r#"
            // Exported into console for observation
            console.log("__WPT_SUMMARY__:" + globalThis.__wpt_json);
        "#;
        rt.execute_script(extract_script).unwrap();

        let msgs = rt.take_console_messages();
        let mut json_str = String::new();
        for m in msgs {
            if m.text.starts_with("__WPT_SUMMARY__:") {
                json_str = m.text.trim_start_matches("__WPT_SUMMARY__:").to_string();
                break;
            }
        }

        let summary: WptSummary = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Failed to parse summary JSON for '{name}': {e} (got: {json_str})"));

        total_all += summary.total;
        passed_all += summary.passed;
        failed_all += summary.failed;

        suite_results.push((name, summary.passed, summary.total, summary.percentage));

        println!("▶ Suite: {:<32} [{}/{} passed ({:.1}%)]", name, summary.passed, summary.total, summary.percentage);
        for t in &summary.tests {
            if t.status == "PASS" {
                println!("    ✓ PASS: {}", t.name);
            } else {
                println!("    ✗ FAIL: {} — {}", t.name, t.message.as_deref().unwrap_or(""));
            }
        }
        println!();
    }

    let overall_percentage = if total_all > 0 {
        (passed_all as f64 / total_all as f64) * 100.0
    } else {
        0.0
    };

    println!("================================================================================");
    println!("📊 OFFICIAL WPT COMPLIANCE SCORECARD FOR MANGO ENGINE");
    println!("================================================================================");
    println!("{:<36} | {:<12} | {:<10} | {:<8}", "Test Suite", "Passed/Total", "Pass Rate", "Status");
    println!("{:-<36}-|-{:-<12}-|-{:-<10}-|-{:-<8}", "", "", "", "");
    for (name, passed, total, pct) in suite_results {
        let status = if pct >= 100.0 { "✅ PASS" } else { "⚠️ PARTIAL" };
        println!("{:<36} | {:>4} / {:<5} | {:>6.1}%    | {:<8}", name, passed, total, pct, status);
    }
    println!("{:-<36}-|-{:-<12}-|-{:-<10}-|-{:-<8}", "", "", "", "");
    println!("{:<36} | {:>4} / {:<5} | {:>6.1}%    | {}", "OVERALL TOTALS", passed_all, total_all, overall_percentage, if overall_percentage >= 100.0 { "✅ 100%" } else { "⚠️" });
    println!("================================================================================\n");

    assert_eq!(failed_all, 0, "All WPT tests must pass for full compliance!");
    assert_eq!(overall_percentage, 100.0, "Overall WPT pass rate must be 100%!");
}
