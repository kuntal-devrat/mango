//! Automated Headless Acid3 Test Runner for Mango.
//!
//! Spawns local WptServer on an ephemeral port, navigates BrowserChrome to
//! `/acid/acid3/test.html`, pumps the event loop until `reftest-wait` is removed,
//! programmatically extracts score and failure diagnostics, and outputs a
//! structured conformance scorecard.
//!
//! Usage:
//!   cargo test --test wpt_acid3_runner -- --nocapture

#![allow(dead_code)]

#[path = "wpt/acid3_metadata.rs"]
pub mod acid3_metadata;

#[path = "wpt/server.rs"]
mod server;

use mango_browser::browser::BrowserChrome;
use server::WptServer;
use std::time::{Duration, Instant};

const ACID3_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Acid3RunResult {
    pub score: u32,
    pub raw_result_text: String,
    pub completed: bool,
    pub duration: Duration,
    pub raw_log: String,
    pub failures: Vec<Acid3Failure>,
    pub bucket_passed: [usize; 7], // 0: Special, 1-6: Buckets 1-6
    pub bucket_totals: [usize; 7],
    pub current_index: u32,
    pub current_retry: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Acid3Failure {
    pub test_number: u32,
    pub bucket: u8,
    pub title: String,
    pub message: String,
}

#[allow(unused_imports)]
pub use acid3_metadata::{ACID3_METADATA, TestMeta, bucket_for_test};

pub fn run_acid3_headless(
    server_base_url: &str,
    timeout: Duration,
) -> Result<Acid3RunResult, String> {
    let mut browser = BrowserChrome::new(800, 600);
    let test_url = format!(
        "{}/acid/acid3/test.html",
        server_base_url.trim_end_matches('/')
    );

    browser.navigate(&test_url);
    browser.wait_for_navigation();

    // Ensure update() is started if load event didn't trigger it automatically
    let _ = browser.eval_js(
        "if (typeof update === 'function' && typeof score === 'undefined') { try { update(); } catch(e) {} }",
    );

    let started = Instant::now();
    let tick_interval_ms = 16.0;
    let mut completed = false;

    while started.elapsed() < timeout {
        let _ = browser.tick_navigation();
        let _ = browser.tick_animations(tick_interval_ms);
        let _ = browser.tick_js();
        let _ = browser.flush_pending_layout();
        if browser.has_pending_image_fetches() {
            let _ = browser.drain_pending_images();
        }

        // Check if test.html is initialized and reftest-wait class has been removed.
        // Requires:
        // 1. document.documentElement exists
        // 2. document.getElementById('result') !== null (DOM structure loaded)
        // 3. typeof score !== 'undefined' (Acid3 test harness script executed)
        // 4. !document.documentElement.classList.contains('reftest-wait') (completion signaled)
        let check_completed_js = "Boolean(document.documentElement && \
            document.getElementById('result') !== null && \
            typeof score !== 'undefined' && \
            document.documentElement.classList && \
            !document.documentElement.classList.contains('reftest-wait'))";

        if let Ok(finished) = browser.eval_js(check_completed_js)
            && finished == "true"
        {
            completed = true;
            break;
        }

        let sleep = browser
            .next_timer_deadline()
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|| Duration::from_millis(2))
            .min(Duration::from_millis(15));
        std::thread::sleep(sleep);
    }

    let duration = started.elapsed();

    // Atomic JSON extraction
    let extract_js = r#"
        (function() {
            var scoreVal = null;
            if (typeof score === 'number') {
                scoreVal = score;
            } else {
                var scoreEl = document.getElementById('score');
                if (scoreEl && scoreEl.firstChild && scoreEl.firstChild.data) {
                    var parsed = parseInt(scoreEl.firstChild.data, 10);
                    if (!isNaN(parsed)) scoreVal = parsed;
                }
            }
            var resultEl = document.getElementById('result');
            var resultText = resultEl ? resultEl.textContent.trim() : '';
            if (scoreVal === null && resultText) {
                var match = resultText.match(/^(\d+)\s*\/\s*\d+$/);
                if (match) {
                    scoreVal = parseInt(match[1], 10);
                }
            }
            var wait = document.documentElement ? document.documentElement.classList.contains('reftest-wait') : true;
            var logText = typeof window !== 'undefined' && window.log ? window.log : '';
            var curIndex = typeof index !== 'undefined' ? index : 0;
            var curRetry = typeof retry !== 'undefined' ? retry : 0;
            return JSON.stringify({
                score: scoreVal,
                resultText: resultText,
                reftestWait: wait,
                log: logText,
                currentIndex: curIndex,
                currentRetry: curRetry
            });
        })()
    "#;

    let json_str = browser
        .eval_js(extract_js)
        .map_err(|e| format!("Eval error: {e}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Parse error: {e} (got: {json_str})"))?;

    let score = parsed.get("score").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let raw_result_text = parsed
        .get("resultText")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let raw_log = parsed
        .get("log")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let current_index = parsed
        .get("currentIndex")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let current_retry = parsed
        .get("currentRetry")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let failures = parse_failures(&raw_log);
    let (bucket_passed, bucket_totals) = compute_bucket_stats(&failures, current_index, completed);

    Ok(Acid3RunResult {
        score,
        raw_result_text,
        completed,
        duration,
        raw_log,
        failures,
        bucket_passed,
        bucket_totals,
        current_index,
        current_retry,
    })
}

fn parse_failures(log: &str) -> Vec<Acid3Failure> {
    let mut failures = Vec::new();
    for line in log.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Test ")
            && trimmed.contains(" failed:")
            && let Some((test_part, msg_part)) = trimmed.split_once(" failed:")
        {
            let num_str = test_part.trim_start_matches("Test ").trim();
            if let Ok(num) = num_str.parse::<u32>() {
                let meta = ACID3_METADATA.iter().find(|m| m.number == num);
                let (bucket, title) = match meta {
                    Some(m) => (m.bucket, m.title),
                    None => (bucket_for_test(num), "Acid3 Subtest"),
                };
                failures.push(Acid3Failure {
                    test_number: num,
                    bucket,
                    title: title.to_string(),
                    message: msg_part.trim().to_string(),
                });
            }
        }
    }
    failures
}

fn compute_bucket_stats(
    failures: &[Acid3Failure],
    current_index: u32,
    completed: bool,
) -> ([usize; 7], [usize; 7]) {
    let totals: [usize; 7] = [4, 16, 16, 16, 16, 16, 16]; // Index 0: Special (4 tests), Index 1-6: Buckets 1-6 (16 each)
    let mut passed = [0usize; 7];

    for test_num in 0..100 {
        let b = bucket_for_test(test_num) as usize;
        let executed = completed || test_num < current_index;
        if executed {
            let failed = failures.iter().any(|f| f.test_number == test_num);
            if !failed {
                passed[b] += 1;
            }
        }
    }
    (passed, totals)
}

pub fn print_scorecard(res: &Acid3RunResult) {
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    println!("\x1b[1;33m🥭 MANGO BROWSER ENGINE — ACID3 CONFORMANCE SCORECARD\x1b[0m");
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    println!(
        "Execution Status   : {}",
        if res.completed {
            "\x1b[1;32mCOMPLETED (reftest-wait cleared)\x1b[0m"
        } else {
            "\x1b[1;31mTIMED OUT / INCOMPLETE\x1b[0m"
        }
    );
    println!("Elapsed Time       : {:.2}s", res.duration.as_secs_f32());
    println!("DOM Result Text    : {}", res.raw_result_text);
    println!("Final Score        : \x1b[1;33m{}/100\x1b[0m", res.score);
    println!(
        "Subtests Executed  : {} / 100",
        if res.completed {
            100
        } else {
            res.current_index
        }
    );
    println!("Failed Subtests    : {}", res.failures.len());
    println!(
        "\n\x1b[1;37m--------------------------------------------------------------------------------\x1b[0m"
    );
    println!("\x1b[1;37mBUCKET CONFORMANCE BREAKDOWN:\x1b[0m");
    println!(
        "\x1b[1;37m--------------------------------------------------------------------------------\x1b[0m"
    );
    let bucket_names = [
        "Special Tests (00, 97-99)",
        "Bucket 1: DOM Traversal & Range (01-16)",
        "Bucket 2: DOM Core & Events (17-32)",
        "Bucket 3: CSS Selectors & Style (33-48)",
        "Bucket 4: HTML DOM, Tables, Forms (49-64)",
        "Bucket 5: SVG DOM, XML & Frames (65-80)",
        "Bucket 6: ECMAScript Semantics (81-96)",
    ];
    for (i, name) in bucket_names.iter().enumerate().skip(1) {
        let pct = (res.bucket_passed[i] as f32 / res.bucket_totals[i] as f32) * 100.0;
        println!(
            "[{}] : {:2} / {:2} passed ({:5.1}%)",
            name, res.bucket_passed[i], res.bucket_totals[i], pct
        );
    }
    let sp_pct = (res.bucket_passed[0] as f32 / res.bucket_totals[0] as f32) * 100.0;
    println!(
        "[{}] : {:2} / {:2} passed ({:5.1}%)",
        bucket_names[0], res.bucket_passed[0], res.bucket_totals[0], sp_pct
    );

    if !res.failures.is_empty() {
        println!(
            "\n\x1b[1;37m--------------------------------------------------------------------------------\x1b[0m"
        );
        println!("\x1b[1;37mFAILING SUBTEST DETAILS (Showing first 25 failures):\x1b[0m");
        println!(
            "\x1b[1;37m--------------------------------------------------------------------------------\x1b[0m"
        );
        for f in res.failures.iter().take(25) {
            println!(
                "  ❌ Test {:02}: {} -> {}",
                f.test_number, f.title, f.message
            );
        }
        if res.failures.len() > 25 {
            println!(
                "  ... and {} more failing subtests",
                res.failures.len() - 25
            );
        }
    }
    println!(
        "\x1b[1;36m================================================================================\x1b[0m\n"
    );
}

#[test]
fn test_acid3_baseline() {
    let server = WptServer::new("tests/wpt").expect("Failed to start local WptServer");
    let result = run_acid3_headless(&server.base_url(), ACID3_TIMEOUT)
        .expect("Headless Acid3 test run failed");

    print_scorecard(&result);

    // Baseline validation assertion: verify execution ran
    assert!(
        result.duration > Duration::from_millis(50),
        "Runner should execute"
    );
}
