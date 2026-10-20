//! Mango WPT (Web Platform Tests) Official Test Suite Runner CLI.
//!
//! Executes canonical W3C WPT test files from `web-platform-tests/wpt`
//! and outputs structured subtest scorecard with pass/fail counts.
//!
//! Usage:
//!   cargo run --bin wpt-runner
//!   cargo run --bin wpt-runner -- [rel_path_to_wpt_test.html]

use std::env;
use std::path::Path;

#[path = "../../tests/wpt/runner.rs"]
mod runner;
use runner::run_wpt_html_file;

const DEFAULT_SUITES: &[(&str, &str)] = &[
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

fn main() {
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    println!("\x1b[1;33m🥭 MANGO BROWSER ENGINE — CANONICAL W3C WPT RUNNER\x1b[0m");
    println!(
        "\x1b[1;36m================================================================================\x1b[0m\n"
    );

    let args: Vec<String> = env::args().collect();
    let base_dir = Path::new("tests/wpt");

    let tests_to_run: Vec<(&str, String)> = if args.len() > 1 && !args[1].starts_with('-') {
        let path = args[1].clone();
        vec![("CLI Test Target", path)]
    } else {
        DEFAULT_SUITES
            .iter()
            .map(|(label, path)| (*label, path.to_string()))
            .collect()
    };

    let mut total_tests = 0;
    let mut passed_tests = 0;
    let mut failed_tests = 0;

    for (label, test_rel_path) in tests_to_run {
        println!(
            "\x1b[1;35m▶ Running WPT:\x1b[0m {} ({})",
            label, test_rel_path
        );
        match run_wpt_html_file(base_dir, &test_rel_path) {
            Ok(results) => {
                for t in &results.tests {
                    if t.status == "PASS" {
                        println!("    \x1b[32m✓ PASS:\x1b[0m {}", t.name);
                        passed_tests += 1;
                    } else {
                        println!(
                            "    \x1b[31m✗ {}:\x1b[0m {} \x1b[90m(msg: {:?})\x1b[0m",
                            t.status, t.name, t.message
                        );
                        failed_tests += 1;
                    }
                    total_tests += 1;
                }
            }
            Err(e) => {
                eprintln!("    \x1b[31m[ERROR]\x1b[0m Failed to execute '{test_rel_path}': {e}");
            }
        }
    }

    println!(
        "\n\x1b[1;36m================================================================================\x1b[0m"
    );
    println!("\x1b[1;33m📊 OFFICIAL WPT COMPLIANCE SCORECARD FOR MANGO ENGINE\x1b[0m");
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    let pass_rate = if total_tests > 0 {
        (passed_tests as f64 / total_tests as f64) * 100.0
    } else {
        0.0
    };
    println!("Total WPT Assertions : {total_tests}");
    println!("Passed Assertions   : {passed_tests}");
    println!("Failed Assertions   : {failed_tests}");
    println!("Pass Rate           : {pass_rate:.1}%");
    println!(
        "\x1b[1;36m================================================================================\x1b[0m\n"
    );
}
