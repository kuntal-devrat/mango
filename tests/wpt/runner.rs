//! Official Web Platform Tests (WPT) execution harness for Mango.
//!
//! Loads and runs canonical W3C HTML test files, injects W3C `testharness.js`
//! and Mango's `testharnessreport.js`, executes script blocks in document order,
//! and extracts structured subtest pass/fail results.

use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;
use mango_js::JsRuntime;
use std::path::Path;

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct WptSubtest {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct WptFileResults {
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub tests: Vec<WptSubtest>,
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub passed: usize,
}

fn collect_scripts_in_order(
    doc: &Document,
    root_id: NodeId,
    out: &mut Vec<(Option<String>, String)>,
) {
    let mut stack = vec![root_id];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = doc.get(node_id) {
            if let NodeData::Element(ref elem) = node.data
                && elem.tag_name.eq_ignore_ascii_case("script")
            {
                let src = elem.get_attribute("src").map(|s| s.trim().to_string());
                let inline = doc.text_content(node_id);
                out.push((src, inline));
            }
            let mut kids: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            kids.reverse();
            stack.extend(kids);
        }
    }
}

/// Executes a single upstream WPT HTML test file and returns the parsed subtest results.
pub fn run_wpt_html_file(base_wpt_dir: &Path, rel_path: &str) -> Result<WptFileResults, String> {
    let test_path = if base_wpt_dir.join("upstream").join(rel_path).exists() {
        base_wpt_dir.join("upstream").join(rel_path)
    } else {
        base_wpt_dir.join(rel_path)
    };

    let content = std::fs::read_to_string(&test_path)
        .map_err(|e| format!("Could not read test file {}: {e}", test_path.display()))?;

    let doc = parse_html(&content);
    let mut scripts = Vec::new();
    collect_scripts_in_order(&doc, doc.root(), &mut scripts);

    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    // 1. Initialize canonical W3C testharness and reporting callback
    let testharness_code =
        std::fs::read_to_string(base_wpt_dir.join("upstream/resources/testharness.js"))
            .or_else(|_| std::fs::read_to_string(base_wpt_dir.join("testharness.js")))
            .map_err(|e| format!("Could not read testharness.js: {e}"))?;

    let report_code = std::fs::read_to_string(base_wpt_dir.join("testharnessreport.js"))
        .map_err(|e| format!("Could not read testharnessreport.js: {e}"))?;

    rt.execute_script(&testharness_code)
        .map_err(|e| format!("testharness.js execution error: {e}"))?;
    rt.execute_script(&report_code)
        .map_err(|e| format!("testharnessreport.js execution error: {e}"))?;

    // 2. Execute script tags in tree order
    let _ = rt.execute_script("if (typeof _syncWindowNamedProperties === 'function') { _syncWindowNamedProperties(); }");
    for (src_opt, inline_code) in scripts {
        let _ = rt.execute_script("if (typeof _syncWindowNamedProperties === 'function') { _syncWindowNamedProperties(); }");
        if let Some(src) = src_opt {
            if src.contains("testharness.js") || src.contains("testharnessreport.js") {
                continue;
            }
            let script_path = if let Some(stripped) = src.strip_prefix("/resources/") {
                base_wpt_dir.join("upstream/resources").join(stripped)
            } else if src.starts_with('/') {
                base_wpt_dir
                    .join("upstream")
                    .join(src.trim_start_matches('/'))
            } else {
                test_path.parent().unwrap_or(base_wpt_dir).join(&src)
            };

            if script_path.exists()
                && let Ok(code) = std::fs::read_to_string(&script_path)
            {
                let _ = rt.execute_script(&code);
            }
        } else if !inline_code.trim().is_empty() {
            let _ = rt.execute_script(&inline_code);
        }
    }

    // 3. Dispatch window 'load' event and invoke done()
    let _ = rt.execute_script(
        "if (typeof window !== 'undefined' && typeof Event !== 'undefined') { \
            window.dispatchEvent(new Event('load')); \
        } \
        if (typeof done === 'function') { done(); }",
    );
    rt.run_jobs();

    // 4. Retrieve results
    let extract = r#"
        globalThis.__wpt_json = JSON.stringify(globalThis.__wpt_results || {
            status: 0,
            tests: [],
            total: 0,
            passed: 0
        });
        console.log("__WPT_SUMMARY__:" + globalThis.__wpt_json);
    "#;
    let _ = rt.execute_script(extract);

    let msgs = rt.take_console_messages();
    let mut json_str = String::new();
    for m in msgs {
        if let Some(rest) = m.text.strip_prefix("__WPT_SUMMARY__:") {
            json_str = rest.to_string();
            break;
        }
    }

    if json_str.is_empty() {
        return Err("No WPT summary reported".to_string());
    }

    let results: WptFileResults = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse WPT JSON: {e} (got: {json_str})"))?;

    Ok(results)
}
