//! Official Web Platform Tests (WPT) conformance runner for Mango.
//!
//! This test executes **canonical upstream WPT files** — the byte-exact
//! `web-platform-tests/wpt` snapshot vendored under `tests/wpt/upstream/` —
//! through the W3C `testharness.js`, and reports a conformance scorecard
//! broken down by WPT category.
//!
//! There are no hand-written approximations here: every assertion counted below
//! comes from an upstream W3C file.
//!
//! The suite is split into three layers:
//!
//! 1. `wpt_dom_character_data_is_100_percent` — a hard gate on the WHATWG DOM
//!    § 4.4 `CharacterData` suite, which Mango claims full support for.
//! 2. `wpt_official_dom_category` — the whole vendored `dom/` category, with a
//!    per-file conformance threshold.
//! 3. `wpt_official_css_category` — the whole vendored `css/` category.
//!
//! Run with `cargo test --test wpt_official_runner -- --nocapture` to see the
//! full scorecard.

#[path = "wpt/discovery.rs"]
pub mod discovery;
#[path = "wpt/harness.rs"]
pub mod harness;
#[path = "wpt/isolation.rs"]
pub mod isolation;
#[path = "wpt/report.rs"]
pub mod report;

use harness::{WptFileStatus, WptSuiteResult};
use std::collections::BTreeMap;
use std::path::Path;

/// The vendored, byte-exact upstream WPT root.
fn upstream_root() -> std::path::PathBuf {
    harness::upstream_root()
}

/// Runs every test in `subdir` of the vendored upstream tree.
///
/// Each file runs in its own child process (see [`isolation`]) so that a
/// runaway or crashing upstream test is reported as a failure instead of
/// aborting the whole conformance run — the same isolation model `wptrunner`
/// uses.
fn run_category(subdir: &str) -> WptSuiteResult {
    let root = upstream_root();
    let dir = root.join(subdir);
    let mut suite = WptSuiteResult::default();

    let mut skipped = Vec::new();
    let mut to_run = Vec::new();
    for rel in harness::discover_tests(&root, &dir) {
        if discovery::is_skipped(&root, &rel) {
            skipped.push(rel);
        } else {
            to_run.push(rel);
        }
    }

    isolation::stream_write(&format!(
        "\n\x1b[1;36m▶ Running WPT category: {}\x1b[0m ({} tests to run, {} skipped)\n",
        subdir,
        to_run.len(),
        skipped.len()
    ));

    for rel in skipped {
        suite.push(harness::WptFileResult {
            path: rel,
            status: WptFileStatus::Skipped,
            subtests: Vec::new(),
            message: Some("skipped by META.yml".to_string()),
            duration: std::time::Duration::ZERO,
        });
    }

    for result in isolation::run_files_isolated(&root, &to_run) {
        suite.push(result);
    }

    suite
}

/// Prints the conformance scorecard for a suite.
fn print_scorecard(title: &str, suite: &WptSuiteResult) {
    let mut out = String::new();
    out.push_str("\n================================================================================\n");
    out.push_str(&format!("📊 {title}\n"));
    out.push_str("================================================================================\n");
    out.push_str(&format!(
        "Files run          : {} ({} passed, {} errored, {} timed out)\n",
        suite.files_total(),
        suite.files_passed(),
        suite.files_errored(),
        suite.files_timed_out()
    ));
    out.push_str(&format!(
        "Assertions         : {} total | {} passed | {} failed | {} skipped\n",
        suite.total(),
        suite.passed(),
        suite.failed(),
        suite.results.iter().map(|r| r.skipped()).sum::<usize>()
    ));
    out.push_str(&format!("Assertion pass rate: {:.2}%\n", suite.pass_percentage()));
    out.push_str("================================================================================\n");
    isolation::stream_write(&out);
    println!("{out}");
}

/// Prints the first `limit` failing files with their failure messages.
fn print_failures(suite: &WptSuiteResult, limit: usize) {
    let mut out = String::new();
    let mut shown = 0;
    for r in &suite.results {
        if shown >= limit {
            break;
        }
        if r.status == WptFileStatus::Pass {
            continue;
        }
        out.push_str(&format!("  [{}] {}\n", r.status.as_str(), r.path));
        if let Some(m) = &r.message {
            let first = m.lines().next().unwrap_or("");
            out.push_str(&format!("      {first}\n"));
        }
        for t in r.subtests.iter().filter(|t| !t.is_pass()).take(3) {
            out.push_str(&format!("      - {}: {}\n", t.name, t.status));
        }
        shown += 1;
    }
    let failing = suite
        .results
        .iter()
        .filter(|r| r.counts_as_run() && r.status != WptFileStatus::Pass)
        .count();
    if failing > shown {
        out.push_str(&format!("  … and {} more non-passing files\n", failing - shown));
    }
    isolation::stream_write(&out);
    println!("{out}");
}

/// The WHATWG DOM § 4.4 `CharacterData` suite, which Mango targets at 100 %.
#[test]
fn wpt_dom_character_data_is_100_percent() {
    let root = upstream_root();
    let dir = root.join("dom/nodes");

    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let mut files: Vec<String> = harness::discover_tests(&root, &dir)
        .into_iter()
        .filter(|rel| rel.contains("CharacterData"))
        .collect();
    files.sort();

    isolation::stream_write(&format!(
        "\n\x1b[1;36m▶ Running WPT: CharacterData (100% target)\x1b[0m ({} test files)\n",
        files.len()
    ));

    for result in isolation::run_files_isolated(&root, &files) {
        for t in &result.subtests {
            total += 1;
            if t.is_pass() {
                passed += 1;
            } else {
                failures.push(format!(
                    "{} :: {} [{}] {:?}",
                    result.path, t.name, t.status, t.message
                ));
            }
        }
    }

    let summary = format!("\n[CharacterData] {passed}/{total} assertions passed\n");
    isolation::stream_write(&summary);
    println!("{summary}");
    for f in failures.iter().take(20) {
        let line = format!("  FAIL {f}\n");
        isolation::stream_write(&line);
        print!("{line}");
    }

    assert!(total > 0, "no CharacterData assertions were executed");
    assert_eq!(
        passed, total,
        "Mango claims 100% WPT conformance for the WHATWG DOM CharacterData suite"
    );
}

/// The whole vendored `dom/` category.
#[test]
fn wpt_official_dom_category() {
    let suite = run_category("dom");
    print_scorecard("OFFICIAL WPT — DOM CATEGORY", &suite);
    print_failures(&suite, 25);

    assert!(
        suite.total() > 0,
        "the vendored dom/ category produced no assertions"
    );
    // Mango's DOM support is a headline feature; the category must stay above
    // this floor so regressions are caught in CI.
    assert!(
        suite.pass_percentage() >= 45.0,
        "DOM category pass rate regressed to {:.2}% (floor: 45%)",
        suite.pass_percentage()
    );
}

/// The whole vendored `css/` category.
#[test]
fn wpt_official_css_category() {
    let suite = run_category("css");
    print_scorecard("OFFICIAL WPT — CSS CATEGORY", &suite);
    print_failures(&suite, 25);

    assert!(
        suite.total() > 0,
        "the vendored css/ category produced no assertions"
    );
    assert!(
        suite.pass_percentage() >= 5.0,
        "CSS category pass rate regressed to {:.2}% (floor: 5%)",
        suite.pass_percentage()
    );
}

/// Writes a machine-readable conformance report next to the build artifacts.
///
/// The report is the input for the `docs/WPT_COMPLIANCE.md` scorecard and for
/// the 0.1.1 release gates.
#[test]
fn wpt_compliance_report_is_generated() {
    let mut by_category: BTreeMap<String, WptSuiteResult> = BTreeMap::new();

    for category in ["dom", "css"] {
        by_category.insert(category.to_string(), run_category(category));
    }

    let mut report = report::WptReport::new("Mango", env!("CARGO_PKG_VERSION"));
    for (category, suite) in &by_category {
        for r in &suite.results {
            if !r.counts_as_run() {
                continue;
            }
            report.add_result(report::TestFileResult {
                test: format!("{category}/{}", r.path),
                status: r.status.as_str().to_string(),
                subtests: r
                    .subtests
                    .iter()
                    .map(|t| report::SubtestResult {
                        name: t.name.clone(),
                        status: t.status.clone(),
                        message: t.message.clone(),
                    })
                    .collect(),
                message: r.message.clone(),
            });
        }
    }

    let out_dir = Path::new("target");
    let path = out_dir.join("wpt_compliance.json");
    report.write_json(&path).expect("failed to write WPT report");
    let wrote_msg = format!("Wrote {}\n", path.display());
    isolation::stream_write(&wrote_msg);
    println!("{wrote_msg}");

    // A quick per-category summary for humans.
    for (category, suite) in &by_category {
        let summary = format!(
            "{category:<6} files {:>4}/{:>4} | assertions {:>5}/{:>5} | {:.2}%\n",
            suite.files_passed(),
            suite.files_total(),
            suite.passed(),
            suite.total(),
            suite.pass_percentage()
        );
        isolation::stream_write(&summary);
        print!("{summary}");
    }

    assert!(report.total_subtests() > 0, "empty conformance report");
}
