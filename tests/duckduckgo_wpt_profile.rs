//! Focused canonical WPT/reftest inventory for DuckDuckGo HTML results.
//!
//! This profile deliberately does not claim full WPT or Chromium parity. It
//! verifies that the selected tests are real vendored upstream files and that
//! visual tests have canonical `rel="match"` references. Set
//! `MANGO_RUN_DDG_WPT=1` to execute the behavioral WPT entries through the
//! existing upstream harness; reftests remain covered by the CSS reftest
//! runner because they require pixel comparison.

use std::fs;
use std::path::{Path, PathBuf};

#[path = "wpt/runner.rs"]
mod wpt_runner;

const PROFILE: &str = include_str!("wpt/duckduckgo/profile.toml");

fn upstream_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wpt/upstream")
}

fn profile_entries() -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut path = None;
    let mut kind = None;
    for line in PROFILE.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("path = \"") {
            path = value.strip_suffix('"').map(str::to_string);
        } else if let Some(value) = line.strip_prefix("kind = \"") {
            kind = value.strip_suffix('"').map(str::to_string);
        }
        if path.is_some() && kind.is_some() {
            entries.push((path.take().unwrap(), kind.take().unwrap()));
        }
    }
    entries
}

fn canonical_match_reference(test_path: &Path, source: &str) -> Option<PathBuf> {
    let marker = "rel=\"match\"";
    let start = source.find(marker)?;
    let tag_start = source[..start].rfind("<link")?;
    let tag_end = source[start..].find('>')? + start;
    let tag = &source[tag_start..tag_end];
    let href_start = tag.find("href=\"")? + "href=\"".len();
    let href_end = tag[href_start..].find('"')? + href_start;
    let href = &tag[href_start..href_end];
    Some(if let Some(root_relative) = href.strip_prefix('/') {
        upstream_root().join(root_relative)
    } else {
        test_path.parent()?.join(href)
    })
}

#[test]
fn duckduckgo_profile_contains_only_real_upstream_tests() {
    let root = upstream_root();
    let entries = profile_entries();
    assert!(!entries.is_empty(), "DuckDuckGo WPT profile is empty");

    for (relative, kind) in entries {
        let path = root.join(&relative);
        assert!(
            path.is_file(),
            "missing canonical upstream test: {relative}"
        );
        let source = fs::read_to_string(&path).expect("upstream test should be readable");
        if kind == "reftest" {
            let reference = canonical_match_reference(&path, &source)
                .unwrap_or_else(|| panic!("reftest has no canonical rel=match: {relative}"));
            assert!(
                reference.is_file(),
                "missing canonical reftest reference for {relative}: {}",
                reference.display()
            );
        }
    }
}

#[test]
#[ignore = "opt-in: executes canonical upstream WPT files and may be slow"]
fn duckduckgo_profile_runs_canonical_wpt_entries() {
    if std::env::var_os("MANGO_RUN_DDG_WPT").is_none() {
        return;
    }

    // Keep this opt-in test intentionally small and serial. The reftest
    // entries are executed by wpt_css_reftests_runner, not by this harness.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wpt");
    let expected = [
        "dom/nodes/ParentNode-querySelector-matches.html",
        "dom/nodes/ParentNode-querySelectorAll-scope-colon-id-class.html",
        "css/css-cascade/inline-style-background.html",
    ];
    for relative in expected {
        let result = wpt_runner::run_wpt_html_file(&root, relative)
            .unwrap_or_else(|error| panic!("failed to execute canonical WPT {relative}: {error}"));
        assert_eq!(
            result.status,
            Some(0),
            "canonical WPT failed: {relative}: {result:?}"
        );
    }
}
