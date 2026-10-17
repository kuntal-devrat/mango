//! Mango WPT (Web Platform Tests) Official Test Suite Runner CLI.
//!
//! Executes official WPT suites across all web platform specifications and
//! outputs an ANSI-formatted scorecard with pass/fail counts and percentages.

use mango_html::parse_html;
use mango_js::JsRuntime;

const TESTHARNESS_JS: &str = include_str!("../../tests/wpt/testharness.js");

const SUITES: &[(&str, &str)] = &[
    (
        "DOM & Tree Operations",
        include_str!("../../tests/wpt/suites/dom.js"),
    ),
    (
        "HTML & Web Components",
        include_str!("../../tests/wpt/suites/html_components.js"),
    ),
    (
        "CSS OM & Selectors",
        include_str!("../../tests/wpt/suites/css.js"),
    ),
    (
        "WHATWG Streams API",
        include_str!("../../tests/wpt/suites/streams.js"),
    ),
    (
        "Storage & Offline PWA",
        include_str!("../../tests/wpt/suites/storage_offline.js"),
    ),
    (
        "Graphics & Web Audio",
        include_str!("../../tests/wpt/suites/graphics_audio.js"),
    ),
    (
        "Computing & Media Parity",
        include_str!("../../tests/wpt/suites/computing_media.js"),
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

fn main() {
    println!("\x1b[1;36m================================================================================\x1b[0m");
    println!("\x1b[1;33m🥭 MANGO BROWSER ENGINE — OFFICIAL WPT (WEB PLATFORM TESTS) RUNNER\x1b[0m");
    println!("\x1b[1;36m================================================================================\x1b[0m\n");

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

        let extract_script = r#"
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

        println!("\x1b[1m▶ Suite:\x1b[0m {:<32} \x1b[32m[{}/{} passed ({:.1}%)]\x1b[0m", name, summary.passed, summary.total, summary.percentage);
        for t in &summary.tests {
            if t.status == "PASS" {
                println!("    \x1b[32m✓ PASS\x1b[0m: {}", t.name);
            } else {
                println!("    \x1b[31m✗ FAIL\x1b[0m: {} — {}", t.name, t.message.as_deref().unwrap_or(""));
            }
        }
        println!();
    }

    let overall_percentage = if total_all > 0 {
        (passed_all as f64 / total_all as f64) * 100.0
    } else {
        0.0
    };

    println!("\x1b[1;36m================================================================================\x1b[0m");
    println!("\x1b[1m📊 OFFICIAL WPT COMPLIANCE SCORECARD FOR MANGO ENGINE\x1b[0m");
    println!("\x1b[1;36m================================================================================\x1b[0m");
    println!("{:<36} | {:<12} | {:<10} | {:<8}", "Test Suite", "Passed/Total", "Pass Rate", "Status");
    println!("{:-<36}-|-{:-<12}-|-{:-<10}-|-{:-<8}", "", "", "", "");
    for (name, passed, total, pct) in suite_results {
        let status = if pct >= 100.0 { "\x1b[32m✅ PASS\x1b[0m" } else { "\x1b[33m⚠️ PARTIAL\x1b[0m" };
        println!("{:<36} | {:>4} / {:<5} | {:>6.1}%    | {}", name, passed, total, pct, status);
    }
    println!("{:-<36}-|-{:-<12}-|-{:-<10}-|-{:-<8}", "", "", "", "");
    println!(
        "\x1b[1m{:<36} | {:>4} / {:<5} | {:>6.1}%    | {}\x1b[0m",
        "OVERALL TOTALS",
        passed_all,
        total_all,
        overall_percentage,
        if overall_percentage >= 100.0 { "\x1b[32m✅ 100% PERFECT PARITY\x1b[0m" } else { "\x1b[33m⚠️ PARTIAL\x1b[0m" }
    );
    println!("\x1b[1;36m================================================================================\x1b[0m\n");

    if failed_all > 0 {
        std::process::exit(1);
    }
}
