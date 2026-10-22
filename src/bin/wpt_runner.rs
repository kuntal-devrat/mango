//! Mango WPT (Web Platform Tests) runner CLI.
//!
//! Executes canonical W3C WPT test files from the vendored, byte-exact
//! `web-platform-tests/wpt` snapshot under `tests/wpt/upstream/` and produces a
//! structured conformance scorecard.
//!
//! Usage:
//!   cargo run --release --bin wpt-runner --                     # whole upstream tree
//!   cargo run --release --bin wpt-runner -- dom/nodes           # one category
//!   cargo run --release --bin wpt-runner -- --json-batch list.txt
//!       Runs the files listed in `list.txt` (one relative path per line) and
//!       prints one JSON object per file. This mode is used by the integration
//!       tests to isolate each WPT file in its own process, exactly like
//!       `wptrunner` does, so that a runaway test cannot abort the whole run.

#[path = "../../tests/wpt/expectations.rs"]
mod expectations;
#[path = "../../tests/wpt/harness.rs"]
mod harness;

use harness::{WptFileStatus, WptSuiteResult};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = harness::upstream_root();

    if let Some(idx) = args.iter().position(|a| a == "--json-batch") {
        let list_path = args
            .get(idx + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("-"));
        run_json_batch(&root, &list_path);
        return;
    }

    if let Some(idx) = args.iter().position(|a| a == "--json")
        && let Some(rel) = args.get(idx + 1)
    {
        let result = harness::run_wpt_file(&root, rel, harness::DEFAULT_FILE_TIMEOUT);
        println!(
            "{}",
            serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }

    print_banner();

    let targets: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect();

    let suite = if targets.is_empty() {
        run_all(&root)
    } else {
        let mut s = WptSuiteResult::default();
        for t in targets {
            let dir = root.join(&t);
            if dir.is_dir() {
                for rel in harness::discover_tests(&root, &dir) {
                    s.push(harness::run_wpt_file(
                        &root,
                        &rel,
                        harness::DEFAULT_FILE_TIMEOUT,
                    ));
                }
            } else {
                s.push(harness::run_wpt_file(
                    &root,
                    &t,
                    harness::DEFAULT_FILE_TIMEOUT,
                ));
            }
        }
        s
    };

    print_scorecard(&suite);
    print_failures(&suite, 40);

    let update_expectations = args.iter().any(|a| a == "--update-expectations");
    let check_expectations = args.iter().any(|a| a == "--check-expectations");
    let expectations_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("wpt")
        .join("expectations");

    if update_expectations {
        let mut store =
            expectations::ExpectationStore::load_from_dir(&expectations_dir).unwrap_or_default();
        for res in &suite.results {
            if res.counts_as_run() {
                store.record_result(res);
            }
        }
        if let Err(e) = store.save_to_dir(&expectations_dir) {
            eprintln!("Failed to save expectations: {e}");
        } else {
            println!(
                "✨ Successfully updated modular WPT expectations in {}",
                expectations_dir.display()
            );
        }
    } else if check_expectations {
        let store =
            expectations::ExpectationStore::load_from_dir(&expectations_dir).unwrap_or_default();
        let audit = store.audit_suite(&suite);
        let summary = audit.summary("MANGO WPT SUITE");
        println!("{summary}");
        if !audit.is_clean() {
            std::process::exit(1);
        }
    }
}

fn print_banner() {
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    println!("\x1b[1;33m🥭 MANGO BROWSER ENGINE — CANONICAL W3C WPT RUNNER\x1b[0m");
    println!(
        "\x1b[1;36m================================================================================\x1b[0m\n"
    );
}

fn run_all(root: &Path) -> WptSuiteResult {
    let mut suite = WptSuiteResult::default();
    for rel in harness::discover_tests(root, root) {
        suite.push(harness::run_wpt_file(
            root,
            &rel,
            harness::DEFAULT_FILE_TIMEOUT,
        ));
    }
    suite
}

/// Runs every file listed in `list_path` and prints one JSON object per line.
///
/// A file that crashes the process (V8 OOM, Rust panic in native code) simply
/// produces no output line; the caller detects the missing line and re-runs
/// that file individually.
fn run_json_batch(root: &Path, list_path: &Path) {
    let list = match std::fs::read_to_string(list_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("could not read {}: {e}", list_path.display());
            return;
        }
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for rel in list.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let result = harness::run_wpt_file(root, rel, harness::DEFAULT_FILE_TIMEOUT);
        if let Ok(json) = serde_json::to_string(&result) {
            let _ = writeln!(out, "{json}");
            let _ = out.flush();
        }
    }
}

fn print_scorecard(suite: &WptSuiteResult) {
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    println!("\x1b[1;33m📊 OFFICIAL WPT COMPLIANCE SCORECARD FOR MANGO ENGINE\x1b[0m");
    println!(
        "\x1b[1;36m================================================================================\x1b[0m"
    );
    println!("Files run          : {}", suite.files_total());
    println!("  passed           : {}", suite.files_passed());
    println!("  errored          : {}", suite.files_errored());
    println!("  timed out        : {}", suite.files_timed_out());
    println!("Total WPT Assertions : {}", suite.total());
    println!("Passed Assertions   : {}", suite.passed());
    println!("Failed Assertions   : {}", suite.failed());
    println!("Pass Rate           : {:.2}%", suite.pass_percentage());
    println!(
        "\x1b[1;36m================================================================================\x1b[0m\n"
    );
}

fn print_failures(suite: &WptSuiteResult, limit: usize) {
    let mut shown = 0;
    for r in &suite.results {
        if shown >= limit {
            break;
        }
        if r.status == WptFileStatus::Pass {
            continue;
        }
        println!("  [{}] {}", r.status.as_str(), r.path);
        if let Some(m) = &r.message {
            println!("      {}", m.lines().next().unwrap_or(""));
        }
        shown += 1;
    }
}
