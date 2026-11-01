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
//! 1. Hard 100% WPT Conformance Gates on core WHATWG DOM specifications:
//!    - WHATWG DOM § 4.4 `CharacterData` suite (`appendData`, `replaceData`, `substringData`, etc.)
//!    - WHATWG DOM § 4.2.7 `ChildNode` suite (`before`, `after`, `replaceWith`, `remove`)
//!    - WHATWG DOM § 4.2.6 `ParentNode` mutations suite (`append`, `prepend`, `replaceChildren`)
//!    - WHATWG DOM § 4.4/4.5 `Text` and `Comment` suite (constructors, `splitText`, `wholeText`)
//!    - WHATWG DOM § 4.2.3 `Node` tree predicates suite (`contains`, `isSameNode`, `isConnected`, `parentElement`)
//! 2. `wpt_official_dom_category` — the whole vendored `dom/` category, with a
//!    per-file conformance threshold.
//! 3. `wpt_official_css_category` — the whole vendored `css/` category.
//!
//! Run with `cargo test --test wpt_official_runner -- --nocapture` to see the
//! full scorecard.

#[path = "wpt/discovery.rs"]
pub mod discovery;
#[path = "wpt/expectations.rs"]
pub mod expectations;
#[path = "wpt/harness.rs"]
pub mod harness;
#[path = "wpt/isolation.rs"]
pub mod isolation;
#[path = "wpt/report.rs"]
pub mod report;

use harness::{WptFileStatus, WptSuiteResult};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The vendored, byte-exact upstream WPT root.
fn upstream_root() -> std::path::PathBuf {
    harness::upstream_root()
}

/// The root directory for modular WPT expectation baselines.
pub fn expectations_root() -> std::path::PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("wpt")
        .join("expectations")
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
    let filter = std::env::var("WPT_TEST_FILTER").ok();
    for rel in harness::discover_tests(&root, &dir) {
        if let Some(filter) = &filter
            && !rel.contains(filter.as_str())
        {
            continue;
        }
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
    out.push_str(
        "\n================================================================================\n",
    );
    out.push_str(&format!("📊 {title}\n"));
    out.push_str(
        "================================================================================\n",
    );
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
    out.push_str(&format!(
        "Assertion pass rate: {:.2}%\n",
        suite.pass_percentage()
    ));
    out.push_str(
        "================================================================================\n",
    );
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
            let detail = t
                .message
                .as_deref()
                .and_then(|m| m.lines().next())
                .unwrap_or("");
            out.push_str(&format!("      - {}: {} {}\n", t.name, t.status, detail));
        }
        shown += 1;
    }
    let failing = suite
        .results
        .iter()
        .filter(|r| r.counts_as_run() && r.status != WptFileStatus::Pass)
        .count();
    if failing > shown {
        out.push_str(&format!(
            "  … and {} more non-passing files\n",
            failing - shown
        ));
    }
    isolation::stream_write(&out);
    println!("{out}");
}

fn assert_suite_100_percent(title: &str, files: &[String]) {
    let root = upstream_root();
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failures: Vec<String> = Vec::new();

    isolation::stream_write(&format!(
        "\n\x1b[1;36m▶ Running WPT: {} (100% target)\x1b[0m ({} test files)\n",
        title,
        files.len()
    ));

    for result in isolation::run_files_isolated(&root, files) {
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

    let summary = format!("\n[{title}] {passed}/{total} assertions passed\n");
    isolation::stream_write(&summary);
    println!("{summary}");
    for f in failures.iter().take(20) {
        let line = format!("  FAIL {f}\n");
        isolation::stream_write(&line);
        print!("{line}");
    }

    assert!(total > 0, "no {title} assertions were executed");
    assert_eq!(
        passed, total,
        "Mango claims 100% WPT conformance for the WHATWG DOM {title} suite"
    );
}

/// The WHATWG DOM § 4.4 `CharacterData` suite, which Mango targets at 100 %.
#[test]
fn wpt_dom_character_data_is_100_percent() {
    let root = upstream_root();
    let dir = root.join("dom/nodes");
    let mut files: Vec<String> = harness::discover_tests(&root, &dir)
        .into_iter()
        .filter(|rel| rel.contains("CharacterData"))
        .collect();
    files.sort();
    assert_suite_100_percent("CharacterData", &files);
}

/// The WHATWG DOM § 4.2.7 `ChildNode` suite (`before`, `after`, `replaceWith`, `remove`), which Mango targets at 100 %.
#[test]
fn wpt_dom_child_node_is_100_percent() {
    let root = upstream_root();
    let dir = root.join("dom/nodes");
    let mut files: Vec<String> = harness::discover_tests(&root, &dir)
        .into_iter()
        .filter(|rel| rel.contains("ChildNode-") || rel.contains("NonDocumentTypeChildNode-"))
        .collect();
    files.sort();
    assert_suite_100_percent("ChildNode", &files);
}

/// The WHATWG DOM § 4.2.6 `ParentNode` mutations suite (`append`, `prepend`, `replaceChildren`), which Mango targets at 100 %.
#[test]
fn wpt_dom_parent_node_mutations_is_100_percent() {
    let root = upstream_root();
    let dir = root.join("dom/nodes");
    let mut files: Vec<String> = harness::discover_tests(&root, &dir)
        .into_iter()
        .filter(|rel| {
            rel.ends_with("ParentNode-append.html")
                || rel.ends_with("ParentNode-prepend.html")
                || rel.ends_with("ParentNode-replaceChildren.html")
        })
        .collect();
    files.sort();
    assert_suite_100_percent("ParentNode Mutations", &files);
}

/// The WHATWG DOM § 4.4/4.5 `Text` and `Comment` suite (constructors, `splitText`, `wholeText`), which Mango targets at 100 %.
#[test]
fn wpt_dom_text_and_comment_is_100_percent() {
    let root = upstream_root();
    let dir = root.join("dom/nodes");
    let mut files: Vec<String> = harness::discover_tests(&root, &dir)
        .into_iter()
        .filter(|rel| {
            rel.ends_with("Text-constructor.html")
                || rel.ends_with("Text-splitText.html")
                || rel.ends_with("Text-wholeText.html")
                || rel.ends_with("Comment-constructor.html")
                || rel.ends_with("Comment-in-doctype.xhtml")
        })
        .collect();
    files.sort();
    assert_suite_100_percent("Text and Comment", &files);
}

/// The WHATWG DOM § 4.2.3 `Node` tree relationship and predicate suite (`contains`, `isSameNode`, `isConnected`, `parentElement`), which Mango targets at 100 %.
#[test]
fn wpt_dom_node_tree_predicates_is_100_percent() {
    let root = upstream_root();
    let dir = root.join("dom/nodes");
    let mut files: Vec<String> = harness::discover_tests(&root, &dir)
        .into_iter()
        .filter(|rel| {
            rel.ends_with("Node-contains.html")
                || rel.ends_with("Node-isSameNode.html")
                || rel.ends_with("Node-isConnected.html")
                || rel.ends_with("Node-parentElement.html")
        })
        .collect();
    files.sort();
    assert_suite_100_percent("Node Tree Predicates", &files);
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

    let exp_dir = expectations_root();
    if let Ok(store) = expectations::ExpectationStore::load_from_dir(&exp_dir)
        && !store.suites.is_empty()
    {
        let audit = store.audit_suite(&suite);
        let summary = audit.summary("DOM CATEGORY");
        isolation::stream_write(&summary);
        println!("{summary}");
        assert!(
            audit.regressions.is_empty(),
            "Regressions detected in DOM category:\n{}",
            audit.regressions.join("\n")
        );
    }
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

    let exp_dir = expectations_root();
    if let Ok(store) = expectations::ExpectationStore::load_from_dir(&exp_dir)
        && !store.suites.is_empty()
    {
        let audit = store.audit_suite(&suite);
        let summary = audit.summary("CSS CATEGORY");
        isolation::stream_write(&summary);
        println!("{summary}");
        assert!(
            audit.regressions.is_empty(),
            "Regressions detected in CSS category:\n{}",
            audit.regressions.join("\n")
        );
    }
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
    report
        .write_json(&path)
        .expect("failed to write WPT report");
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
