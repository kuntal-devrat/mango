//! Official Web Platform Tests (WPT) execution harness for Mango.
//!
//! This module runs *canonical upstream WPT files* (vendored byte-exact under
//! `tests/wpt/upstream/`) through the W3C `testharness.js` and collects the
//! structured per-subtest results.
//!
//! Design notes
//! ------------
//! * The harness speaks the **toHarness** protocol (`testharness.js` +
//!   `add_result_callback` / `add_completion_callback`), which is what the vast
//!   majority of WPT's DOM/CSS `.html` files use.
//! * The harness is **asynchronous**: after the synchronous script phase it
//!   pumps Mango's event loop (`JsRuntime::tick`) until the upstream harness
//!   reports completion or the per-file timeout expires. Without this, every
//!   `promise_test` / `async_test` file would silently report zero assertions.
//! * File-level harness status (`ERROR`, `TIMEOUT`) is surfaced separately from
//!   subtest results, so a test file whose `setup()` throws is reported as an
//!   error instead of vanishing.
//! * Every upstream file is executed *as-is*. There are no hand-written
//!   approximations and no silent skips: when a required sub-resource is
//!   missing, the harness says so loudly.

use mango_html::dom::{Document, NodeData, NodeId, node_id_to_js};
use mango_html::parse_html;
use mango_js::JsRuntime;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Marker prefix used by the report shim to publish the machine-readable summary.
const SUMMARY_PREFIX: &str = "__WPT_SUMMARY__:";
/// Marker prefix used by the report shim to publish the completion signal.
const COMPLETE_PREFIX: &str = "__WPT_COMPLETE__:";

/// Default per-file timeout. Matches the upstream `testharness.js` default of
/// 10 s, with a little head-room for slow CI machines.
pub const DEFAULT_FILE_TIMEOUT: Duration = Duration::from_secs(20);

/// The outcome of running a single upstream WPT file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WptFileStatus {
    /// The harness ran and every reported subtest passed.
    Pass,
    /// The harness ran and at least one subtest failed.
    Fail,
    /// The harness itself errored (e.g. `setup()` threw) or a script threw.
    Error,
    /// The harness did not finish within the timeout.
    Timeout,
    /// The file was skipped by an upstream `META.yml` directive.
    Skipped,
    /// The file is not a toHarness test (reftest or manual test) and is
    /// therefore handled by a different runner.
    NotToHarness,
    /// The file could not be read or parsed.
    Missing,
}

impl WptFileStatus {
    /// Returns the wptrunner-compatible status name.
    pub fn as_str(&self) -> &'static str {
        match self {
            WptFileStatus::Pass => "OK",
            WptFileStatus::Fail => "FAIL",
            WptFileStatus::Error => "ERROR",
            WptFileStatus::Timeout => "TIMEOUT",
            WptFileStatus::Skipped => "SKIPPED",
            WptFileStatus::NotToHarness => "NOT_TO_HARNESS",
            WptFileStatus::Missing => "MISSING",
        }
    }
}

/// Per-subtest result reported by the upstream harness.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WptSubtest {
    pub name: String,
    /// `PASS`, `FAIL`, `TIMEOUT`, `NOTRUN` or `PRECONDITION_FAILED`.
    pub status: String,
    pub message: Option<String>,
}

impl WptSubtest {
    /// A subtest counts as passed only when the upstream harness says `PASS`.
    pub fn is_pass(&self) -> bool {
        self.status == "PASS"
    }
}

/// The result of running one upstream WPT file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WptFileResult {
    /// Path of the file relative to the WPT root (e.g. `dom/nodes/Node-name.html`).
    pub path: String,
    pub status: WptFileStatus,
    pub subtests: Vec<WptSubtest>,
    /// Harness-level message (setup error, timeout reason, …).
    pub message: Option<String>,
    pub duration: Duration,
}

impl WptFileResult {
    /// Number of subtests the harness reported.
    pub fn total(&self) -> usize {
        self.subtests.len()
    }

    /// Number of subtests that passed.
    pub fn passed(&self) -> usize {
        self.subtests.iter().filter(|s| s.is_pass()).count()
    }

    /// Number of subtests that failed (a failed assertion, not an error).
    pub fn failed(&self) -> usize {
        self.subtests.iter().filter(|s| s.status == "FAIL").count()
    }

    /// Subtests that never ran or were skipped by a precondition.
    #[allow(dead_code)]
    pub fn skipped(&self) -> usize {
        self.subtests
            .iter()
            .filter(|s| s.status == "NOTRUN" || s.status == "PRECONDITION_FAILED")
            .count()
    }

    /// Whether this file counts towards the pass rate.
    pub fn counts_as_run(&self) -> bool {
        !matches!(
            self.status,
            WptFileStatus::Skipped | WptFileStatus::NotToHarness | WptFileStatus::Missing
        )
    }
}

/// Aggregated results for a whole suite or category.
#[derive(Debug, Clone, Default)]
pub struct WptSuiteResult {
    pub results: Vec<WptFileResult>,
}

impl WptSuiteResult {
    pub fn push(&mut self, r: WptFileResult) {
        self.results.push(r);
    }

    pub fn total(&self) -> usize {
        self.results.iter().map(|r| r.total()).sum()
    }

    pub fn passed(&self) -> usize {
        self.results.iter().map(|r| r.passed()).sum()
    }

    pub fn failed(&self) -> usize {
        self.results.iter().map(|r| r.failed()).sum()
    }

    pub fn files_total(&self) -> usize {
        self.results.iter().filter(|r| r.counts_as_run()).count()
    }

    pub fn files_passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.counts_as_run() && r.status == WptFileStatus::Pass)
            .count()
    }

    pub fn files_errored(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.counts_as_run() && matches!(r.status, WptFileStatus::Error))
            .count()
    }

    pub fn files_timed_out(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.counts_as_run() && r.status == WptFileStatus::Timeout)
            .count()
    }

    /// Assertion-level pass rate in percent.
    pub fn pass_percentage(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            (self.passed() as f64 / total as f64) * 100.0
        }
    }
}

/// Collects the `<script>` elements of a document in document order.
fn collect_scripts_in_order(
    doc: &Document,
    root_id: NodeId,
    out: &mut Vec<(NodeId, Option<String>, String)>,
) {
    let mut stack = vec![root_id];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = doc.get(node_id) {
            if let NodeData::Element(ref elem) = node.data
                && elem.tag_name.eq_ignore_ascii_case("script")
            {
                let src = elem.get_attribute("src").map(|s| s.trim().to_string());
                let inline = doc.text_content(node_id);
                out.push((node_id, src, inline));
            }
            let mut kids: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            kids.reverse();
            stack.extend(kids);
        }
    }
}

/// Resolves an upstream `<script src="...">` reference to a file on disk.
///
/// Mirrors the wptrunner URL resolution rules:
/// * `/resources/foo.js`  -> `<root>/resources/foo.js`
/// * `/dir/foo.js`         -> `<root>/dir/foo.js`
/// * `foo.js`              -> relative to the directory of the test file
pub fn resolve_script_path(root: &Path, test_path: &Path, src: &str) -> PathBuf {
    let src = src
        .split('#')
        .next()
        .unwrap_or(src)
        .split('?')
        .next()
        .unwrap_or(src);
    if let Some(stripped) = src.strip_prefix("/resources/") {
        root.join("resources").join(stripped)
    } else if let Some(stripped) = src.strip_prefix('/') {
        root.join(stripped)
    } else {
        test_path.parent().unwrap_or(root).join(src)
    }
}

/// Collects the `<iframe>` elements of a document with their resolved subresource contents.
fn collect_iframes_in_order(
    doc: &Document,
    root_id: NodeId,
    out: &mut Vec<(String, String)>,
    root: &Path,
    test_path: &Path,
) {
    let mut stack = vec![root_id];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = doc.get(node_id) {
            if let NodeData::Element(ref elem) = node.data
                && elem.tag_name.eq_ignore_ascii_case("iframe")
                && let Some(src) = elem.get_attribute("src")
            {
                let sub_path = resolve_script_path(root, test_path, src);
                if sub_path.exists()
                    && let Ok(c) = std::fs::read_to_string(&sub_path)
                {
                    out.push((src.trim().to_string(), c));
                }
            }
            let mut kids: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            kids.reverse();
            stack.extend(kids);
        }
    }
}

/// Parses every `<link rel=stylesheet>` in a document, resolving relative and
/// root-relative hrefs inside the vendored WPT tree.
fn collect_external_stylesheets(
    doc: &Document,
    base_dir: Option<&Path>,
) -> Vec<mango_css::parser::Stylesheet> {
    let mut sheets = Vec::new();
    let mut stack = vec![doc.root()];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = doc.get(node_id) {
            if let NodeData::Element(ref elem) = node.data
                && elem.tag_name.eq_ignore_ascii_case("link")
            {
                let is_stylesheet = elem
                    .get_attribute("rel")
                    .map(|rel| {
                        rel.split_whitespace()
                            .any(|part| part.eq_ignore_ascii_case("stylesheet"))
                    })
                    .unwrap_or(false);
                if is_stylesheet && let Some(href) = elem.get_attribute("href") {
                    let href = href.split(['?', '#']).next().unwrap_or(&href);
                    let path = if let Some(rest) = href.strip_prefix('/') {
                        upstream_root().join(rest)
                    } else if let Some(dir) = base_dir {
                        dir.join(href)
                    } else {
                        continue;
                    };
                    if let Ok(css) = std::fs::read_to_string(path) {
                        sheets.push(mango_css::parser::parse_stylesheet(&css));
                    }
                }
            }
            let mut kids: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            kids.reverse();
            stack.extend(kids);
        }
    }
    sheets
}

/// Publishes layout geometry to the JS runtime before a layout-dependent
/// callback runs. Upstream suites such as `check-layout-th.js` measure real
/// boxes through `getBoundingClientRect()`, which is meaningless without it.
fn publish_layout_bounds(
    rt: &JsRuntime,
    viewport: (f32, f32),
    styles: &[&mango_css::parser::Stylesheet],
) {
    let doc = rt.document_snapshot();
    let (root, _) = mango_layout::layout_document_with_scroll_xy(
        &doc,
        styles,
        mango_core::Size::new(viewport.0, viewport.1),
        0.0,
        0.0,
    );
    let mut rects = std::collections::HashMap::new();
    mango_layout::display_list::stacking::collect_box_rects_with_scroll(
        &root, 0.0, 0.0, &mut rects,
    );
    rt.set_layout_bounds(rects);
}

/// The shared upstream WPT root (the vendored, byte-exact snapshot).
pub fn upstream_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("wpt")
        .join("upstream")
}

/// Runs a single upstream WPT file and returns its structured result.
pub fn run_wpt_file(root: &Path, rel_path: &str, timeout: Duration) -> WptFileResult {
    let started = Instant::now();
    let test_path = root.join(rel_path);

    let content = match std::fs::read_to_string(&test_path) {
        Ok(c) => c,
        Err(e) => {
            return WptFileResult {
                path: rel_path.to_string(),
                status: WptFileStatus::Missing,
                subtests: Vec::new(),
                message: Some(format!("could not read {}: {e}", test_path.display())),
                duration: started.elapsed(),
            };
        }
    };

    let mut doc = parse_html(&content);
    let external_sheets = collect_external_stylesheets(&doc, test_path.parent());
    let external_refs: Vec<&mango_css::parser::Stylesheet> = external_sheets.iter().collect();
    if rel_path.ends_with(".xhtml") || rel_path.ends_with(".xht") {
        doc.content_type = "application/xhtml+xml".to_string();
    } else if rel_path.ends_with(".xml") {
        doc.content_type = "application/xml".to_string();
    } else if rel_path.ends_with(".svg") {
        doc.content_type = "image/svg+xml".to_string();
    } else {
        doc.content_type = "text/html".to_string();
    }
    let mut scripts = Vec::new();
    collect_scripts_in_order(&doc, doc.root(), &mut scripts);

    // Not every upstream `.html` file is a toHarness test: CSS reftests and
    // "manual" tests have their own protocols and are executed by the reftest
    // runner instead. Detect them up front so they are reported instantly
    // rather than burning the whole timeout.
    if !scripts
        .iter()
        .any(|(_, src, _)| src.as_deref().is_some_and(|s| s.contains("testharness.js")))
    {
        return WptFileResult {
            path: rel_path.to_string(),
            status: WptFileStatus::NotToHarness,
            subtests: Vec::new(),
            message: Some("not a toHarness test (reftest or manual test)".to_string()),
            duration: started.elapsed(),
        };
    }

    let mut iframes = Vec::new();
    collect_iframes_in_order(&doc, doc.root(), &mut iframes, root, &test_path);

    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    for (src, sub_content) in iframes {
        let script = format!(
            "if (typeof __registerSubresource === 'function') {{ __registerSubresource({}, {}); }}",
            serde_json::to_string(&src).unwrap_or_else(|_| "\"\"".to_string()),
            serde_json::to_string(&sub_content).unwrap_or_else(|_| "\"\"".to_string()),
        );
        let _ = rt.execute_script(&script);
    }

    // ── 1. Load the canonical W3C harness and Mango's report shim ──────────
    let testharness_code = match std::fs::read_to_string(root.join("resources/testharness.js")) {
        Ok(c) => c,
        Err(e) => {
            return WptFileResult {
                path: rel_path.to_string(),
                status: WptFileStatus::Error,
                subtests: Vec::new(),
                message: Some(format!("could not read resources/testharness.js: {e}")),
                duration: started.elapsed(),
            };
        }
    };
    let report_path = root.parent().unwrap_or(root).join("testharnessreport.js");
    let report_code = match std::fs::read_to_string(&report_path) {
        Ok(c) => c,
        Err(e) => {
            return WptFileResult {
                path: rel_path.to_string(),
                status: WptFileStatus::Error,
                subtests: Vec::new(),
                message: Some(format!("could not read testharnessreport.js: {e}")),
                duration: started.elapsed(),
            };
        }
    };

    if let Err(e) = rt.execute_script(&testharness_code) {
        return WptFileResult {
            path: rel_path.to_string(),
            status: WptFileStatus::Error,
            subtests: Vec::new(),
            message: Some(format!("testharness.js failed: {e}")),
            duration: started.elapsed(),
        };
    }
    if let Err(e) = rt.execute_script(&report_code) {
        return WptFileResult {
            path: rel_path.to_string(),
            status: WptFileStatus::Error,
            subtests: Vec::new(),
            message: Some(format!("testharnessreport.js failed: {e}")),
            duration: started.elapsed(),
        };
    }

    // ── 2. Execute the test's scripts in tree order ─────────────────────────
    let mut script_errors: Vec<String> = Vec::new();
    let mut missing_resources: Vec<String> = Vec::new();

    for (script_id, src_opt, inline_code) in scripts {
        // The upstream harness exposes named properties on `window`; Mango needs
        // a nudge to re-sync them after each script block.
        let _ = rt.execute_script(
            "if (typeof _syncWindowNamedProperties === 'function') { _syncWindowNamedProperties(); }",
        );
        let _ = rt.execute_script(&format!(
            "if (typeof __setCurrentScript === 'function') {{ __setCurrentScript({}); }}",
            node_id_to_js(script_id)
        ));
        match src_opt {
            Some(src) => {
                if src.contains("testharness.js") || src.contains("testharnessreport.js") {
                    let _ = rt.execute_script("if (typeof __setCurrentScript === 'function') { __setCurrentScript(null); }");
                    continue;
                }
                let script_path = resolve_script_path(root, &test_path, &src);
                if script_path.exists()
                    && let Ok(code) = std::fs::read_to_string(&script_path)
                    && let Err(e) = rt.execute_script(&code)
                {
                    script_errors.push(format!("{src}: {e}"));
                } else if !script_path.exists() {
                    missing_resources.push(src.clone());
                }
            }
            None => {
                if !inline_code.trim().is_empty()
                    && let Err(e) = rt.execute_script(&inline_code)
                {
                    script_errors.push(format!("<inline>: {e}"));
                }
            }
        }
        let _ = rt.execute_script(
            "if (typeof __setCurrentScript === 'function') { __setCurrentScript(null); }",
        );
    }

    // Layout is available to `onload`/test callbacks in a browser. Publish it
    // before dispatching load so geometry assertions see real boxes.
    publish_layout_bounds(&rt, (800.0, 600.0), &external_refs);

    // ── 3. Dispatch `DOMContentLoaded`/`load`, then pump the event loop until
    // the harness ends. `dispatch_load` also invokes `<body onload=...>`, which
    // is how `check-layout-th.js` suites start their assertions. Do not call
    // `done()` here: those suites request `explicit_done` and call it from
    // `checkLayout()`, so an early call would discard every assertion.
    let _ = rt.dispatch_dom_content_loaded();
    let _ = rt.dispatch_load();

    let mut console: Vec<String> = Vec::new();
    let mut saw_completion = false;
    let deadline = Instant::now() + timeout;

    loop {
        let mutated = rt.tick();
        if mutated {
            publish_layout_bounds(&rt, (800.0, 600.0), &external_refs);
        }

        for m in rt.take_console_messages() {
            if m.text.starts_with(COMPLETE_PREFIX) {
                saw_completion = true;
            }
            console.push(m.text);
        }

        if saw_completion {
            break;
        }
        if Instant::now() >= deadline {
            return WptFileResult {
                path: rel_path.to_string(),
                status: WptFileStatus::Timeout,
                subtests: Vec::new(),
                message: Some(format!("harness did not complete within {timeout:?}")),
                duration: started.elapsed(),
            };
        }

        // Sleep until the next timer is due (bounded) so that `setTimeout`
        // based asynchronous tests can actually make progress.
        let sleep = rt
            .next_timer_deadline()
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|| Duration::from_millis(2))
            .min(Duration::from_millis(25));
        std::thread::sleep(sleep);
    }

    // Give the harness one last chance to flush its final results.
    rt.tick();
    for m in rt.take_console_messages() {
        console.push(m.text);
    }
    // ── 4. Extract the structured summary ──────────────────────────────────
    let mut summary_json = String::new();
    for line in &console {
        if let Some(rest) = line.strip_prefix(SUMMARY_PREFIX) {
            summary_json = rest.to_string();
            break;
        }
    }

    if summary_json.is_empty() {
        // Fall back to the completion payload, which carries the same shape.
        for line in &console {
            if let Some(rest) = line.strip_prefix(COMPLETE_PREFIX) {
                summary_json = rest.to_string();
                break;
            }
        }
    }

    if summary_json.is_empty() {
        let detail = script_errors
            .first()
            .cloned()
            .or_else(|| missing_resources.first().cloned())
            .unwrap_or_else(|| "harness produced no results".to_string());
        return WptFileResult {
            path: rel_path.to_string(),
            status: WptFileStatus::Error,
            subtests: Vec::new(),
            message: Some(detail),
            duration: started.elapsed(),
        };
    }

    let parsed: serde_json::Value = match serde_json::from_str(&summary_json) {
        Ok(v) => v,
        Err(e) => {
            return WptFileResult {
                path: rel_path.to_string(),
                status: WptFileStatus::Error,
                subtests: Vec::new(),
                message: Some(format!("could not parse harness summary: {e}")),
                duration: started.elapsed(),
            };
        }
    };

    let mut subtests = Vec::new();
    if let Some(tests) = parsed.get("tests").and_then(|t| t.as_array()) {
        for t in tests {
            subtests.push(WptSubtest {
                name: t
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: t
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string(),
                message: {
                    let msg = t.get("message").and_then(|v| v.as_str());
                    let stack = t.get("stack").and_then(|v| v.as_str());
                    match (msg, stack) {
                        (Some(m), Some(s)) => Some(format!("{m}\nStack:\n{s}")),
                        (Some(m), None) => Some(m.to_string()),
                        (None, Some(s)) => Some(format!("Stack:\n{s}")),
                        (None, None) => None,
                    }
                },
            });
        }
    }

    let harness_status = parsed.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    let harness_message = parsed
        .get("message")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let failed = subtests.iter().filter(|s| s.status == "FAIL").count();
    let status = if harness_status == 1 {
        // Upstream `TestsStatus.ERROR`: `setup()` or the harness itself threw.
        WptFileStatus::Error
    } else if !script_errors.is_empty() && subtests.is_empty() {
        WptFileStatus::Error
    } else if failed > 0 {
        WptFileStatus::Fail
    } else if subtests.is_empty() {
        WptFileStatus::Error
    } else {
        WptFileStatus::Pass
    };

    let mut message = harness_message;
    if message.is_none() && !script_errors.is_empty() {
        message = Some(script_errors.join("; "));
    }
    if message.is_none() && !missing_resources.is_empty() {
        message = Some(format!(
            "missing sub-resource(s): {}",
            missing_resources.join(", ")
        ));
    }

    WptFileResult {
        path: rel_path.to_string(),
        status,
        subtests,
        message,
        duration: started.elapsed(),
    }
}

/// Discovers every upstream WPT test file under `dir`, relative to `root`.
///
/// Only the file extensions the toHarness protocol supports are collected:
/// `.html` and `.htm` (and the XML variants `.xhtml` / `.xht`, which Mango's
/// HTML parser handles with the same rules).
pub fn discover_tests(root: &Path, dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_tests_recursive(root, dir, &mut out);
    out.sort();
    out
}

fn collect_tests_recursive(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            // Reference files are never test subjects.
            if name == "reference" || name == "support" {
                continue;
            }
            collect_tests_recursive(root, &path, out);
        } else if is_test_file(&name)
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Returns true if `name` looks like a WPT test file.
fn is_test_file(name: &str) -> bool {
    // `*-ref.html` files are the reference renderings used by reftests.
    if name.ends_with("-ref.html") || name.ends_with("-notref.html") {
        return false;
    }
    matches!(
        std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("html") | Some("htm") | Some("xhtml") | Some("xht")
    )
}
