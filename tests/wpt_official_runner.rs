//! Official Web Platform Tests (WPT) Automated Test Harness for Mango.
//!
//! Executes real, canonical W3C HTML test suites from `web-platform-tests/wpt`
//! using the standard W3C `testharness.js` across the DOM specification.

#[path = "wpt/report.rs"]
pub mod report;
#[path = "wpt/runner.rs"]
pub mod runner;
#[path = "wpt/server.rs"]
pub mod server;

use runner::run_wpt_html_file;
use std::path::Path;

/// Canonical upstream WPT test files executed by this harness.
const UPSTREAM_DOM_TESTS: &[(&str, &str)] = &[
    ("Node.nodeName", "dom/nodes/Node-nodeName.html"),
    ("Node.nodeValue", "dom/nodes/Node-nodeValue.html"),
    ("Node.textContent", "dom/nodes/Node-textContent.html"),
    ("Node.isSameNode", "dom/nodes/Node-isSameNode.html"),
    ("CharacterData.data", "dom/nodes/CharacterData-data.html"),
    (
        "CharacterData.appendData",
        "dom/nodes/CharacterData-appendData.html",
    ),
    (
        "CharacterData.deleteData",
        "dom/nodes/CharacterData-deleteData.html",
    ),
    (
        "CharacterData.insertData",
        "dom/nodes/CharacterData-insertData.html",
    ),
    (
        "CharacterData.replaceData",
        "dom/nodes/CharacterData-replaceData.html",
    ),
    (
        "CharacterData.substringData",
        "dom/nodes/CharacterData-substringData.html",
    ),
    ("Element.tagName", "dom/nodes/Element-tagName.html"),
    (
        "Document.createComment",
        "dom/nodes/Document-createComment.html",
    ),
    (
        "Document.createAttribute",
        "dom/nodes/Document-createAttribute.html",
    ),
];

#[test]
fn test_run_official_wpt_suites() {
    println!("\n================================================================================");
    println!("🧪 RUNNING CANONICAL W3C WEB PLATFORM TESTS (WPT) ON MANGO ENGINE");
    println!("================================================================================\n");

    let base_dir = Path::new("tests/wpt");
    let mut total_tests = 0;
    let mut passed_tests = 0;
    let mut failed_tests = 0;

    for (label, test_rel_path) in UPSTREAM_DOM_TESTS {
        println!("▶ Running WPT: {label} ({test_rel_path})");
        match run_wpt_html_file(base_dir, test_rel_path) {
            Ok(results) => {
                for t in &results.tests {
                    if t.status == "PASS" {
                        println!("    ✓ PASS: {}", t.name);
                        passed_tests += 1;
                    } else {
                        println!("    ✗ {}: {} (msg: {:?})", t.status, t.name, t.message);
                        failed_tests += 1;
                    }
                    total_tests += 1;
                }
            }
            Err(e) => {
                eprintln!("    [ERROR] Failed to execute test file '{test_rel_path}': {e}");
            }
        }
    }

    println!("\n================================================================================");
    println!("📊 OFFICIAL WPT COMPLIANCE SCORECARD FOR MANGO ENGINE");
    println!("================================================================================");
    let pass_rate = if total_tests > 0 {
        (passed_tests as f64 / total_tests as f64) * 100.0
    } else {
        0.0
    };
    println!("Total WPT Assertions : {total_tests}");
    println!("Passed Assertions   : {passed_tests}");
    println!("Failed Assertions   : {failed_tests}");
    println!("Pass Rate           : {pass_rate:.1}%");
    println!("================================================================================\n");

    assert!(
        total_tests > 0,
        "Expected WPT tests to run, but 0 assertions were executed"
    );
    assert!(
        passed_tests > 0,
        "Expected at least some WPT assertions to pass"
    );
}
