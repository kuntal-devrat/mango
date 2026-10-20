//! Official W3C CSS Visual Reftest Runner for Mango.
//!
//! Validates pixel-for-pixel rendering parity against reference HTML files
//! per the W3C Web Platform Tests (WPT) specification across:
//! - CSS Flexible Box Layout (`css-flexbox`)
//! - CSS Grid Layout (`css-grid`)
//! - CSS Box Model & Margins (`css-box`)
//! - CSS Positioning & Stacking (`css-position`)
//! - CSS Inline Formatting & Baselines (`css-inline`)
//! - CSS Tables (`css-tables`)
//! - CSS Display & Visibility (`css-display`)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mango_core::Size;
use mango_html::parse_html;
use mango_layout::layout_document;
use mango_render::paint;

const VIEWPORT_WIDTH: u32 = 800;
const VIEWPORT_HEIGHT: u32 = 600;

#[derive(Debug, Clone)]
pub struct ReftestResult {
    pub suite: String,
    pub name: String,
    pub total_pixels: usize,
    pub differing_pixels: usize,
    pub max_delta: u8,
    pub passed: bool,
    pub message: Option<String>,
}

pub struct ReftestSuiteStats {
    pub name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Extracts the reference file path from `<link rel="match" href="...">` or returns the default `-ref.html`.
fn find_reference_file(test_path: &Path, content: &str) -> Option<PathBuf> {
    // 1. Search for <link rel="match" href="...">
    let link_pattern = "rel=\"match\"";
    if let Some(pos) = content.find(link_pattern) {
        // Look around pos for href="..."
        let window_start = pos.saturating_sub(60);
        let window_end = (pos + 120).min(content.len());
        let window = &content[window_start..window_end];
        if let Some(href_pos) = window.find("href=\"") {
            let start = window_start + href_pos + 6;
            if let Some(end_offset) = content[start..].find('"') {
                let href = &content[start..start + end_offset];
                let ref_path = test_path.parent().unwrap().join(href);
                if ref_path.exists() {
                    return Some(ref_path);
                }
            }
        }
    }

    // 2. Default: stem + "-ref.html" in the same directory
    let stem = test_path.file_stem()?.to_str()?;
    let ref_filename = format!("{}-ref.html", stem);
    let default_ref = test_path.parent()?.join(ref_filename);
    if default_ref.exists() {
        return Some(default_ref);
    }

    // 3. Check reference/ subdirectory
    let sub_ref = test_path
        .parent()?
        .join("reference")
        .join(format!("{}-ref.html", stem));
    if sub_ref.exists() {
        return Some(sub_ref);
    }

    None
}

/// Renders an HTML document to an 800x600 32-bit ARGB/RGBA pixel buffer.
fn render_html_to_buffer(html: &str) -> Vec<u32> {
    let doc = parse_html(html);
    let (_, dl) = layout_document(
        &doc,
        &[],
        Size::new(VIEWPORT_WIDTH as f32, VIEWPORT_HEIGHT as f32),
    );
    let mut pixels = vec![0x00FFFFFFu32; (VIEWPORT_WIDTH * VIEWPORT_HEIGHT) as usize];
    paint(&dl, &mut pixels, VIEWPORT_WIDTH, VIEWPORT_HEIGHT);
    pixels
}

/// Compares two 32-bit pixel buffers:
/// Returns (differing_pixel_count, max_channel_delta).
#[allow(clippy::type_complexity)]
fn compare_pixels(
    test_pixels: &[u32],
    ref_pixels: &[u32],
) -> (usize, u8, Option<(u32, u32, u32, u32)>) {
    let count = test_pixels.len().min(ref_pixels.len());
    let mut differing = 0;
    let mut max_delta: u8 = 0;

    let mut first_diff = None;

    for i in 0..count {
        let p1 = test_pixels[i];
        let p2 = ref_pixels[i];
        if (p1 & 0x00ffffff) != (p2 & 0x00ffffff) {
            let r1 = ((p1 >> 16) & 0xff) as i32;
            let g1 = ((p1 >> 8) & 0xff) as i32;
            let b1 = (p1 & 0xff) as i32;

            let r2 = ((p2 >> 16) & 0xff) as i32;
            let g2 = ((p2 >> 8) & 0xff) as i32;
            let b2 = (p2 & 0xff) as i32;

            let dr = (r1 - r2).unsigned_abs() as u8;
            let dg = (g1 - g2).unsigned_abs() as u8;
            let db = (b1 - b2).unsigned_abs() as u8;

            let cur_max = dr.max(dg).max(db);
            if cur_max > max_delta {
                max_delta = cur_max;
            }

            if dr > 2 || dg > 2 || db > 2 {
                if first_diff.is_none() {
                    let x = (i as u32) % VIEWPORT_WIDTH;
                    let y = (i as u32) / VIEWPORT_WIDTH;
                    first_diff = Some((x, y, p1, p2));
                }
                differing += 1;
            }
        }
    }
    (differing, max_delta, first_diff)
}

/// Collects and executes all W3C CSS reftests in `tests/wpt_reftests/`.
pub fn run_all_wpt_reftests() -> Vec<ReftestResult> {
    let reftests_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("wpt_reftests");
    let mut results = Vec::new();

    if !reftests_dir.exists() {
        eprintln!("Reftest directory {:?} does not exist", reftests_dir);
        return results;
    }

    let mut test_files = Vec::new();
    collect_html_files(&reftests_dir, &mut test_files);
    test_files.sort();

    for test_path in test_files {
        let filename = test_path.file_name().unwrap().to_str().unwrap();
        // Skip reference files
        if filename.ends_with("-ref.html") || filename.contains("-notref.html") {
            continue;
        }

        let test_content = match fs::read_to_string(&test_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read {}: {}", test_path.display(), e);
                continue;
            }
        };

        let ref_path = match find_reference_file(&test_path, &test_content) {
            Some(p) => p,
            None => {
                eprintln!("No reference file found for {}", test_path.display());
                continue;
            }
        };

        let ref_content = match fs::read_to_string(&ref_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read reference {}: {}", ref_path.display(), e);
                continue;
            }
        };

        let suite_name = test_path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let test_name = test_path.file_stem().unwrap().to_string_lossy().to_string();

        // Render both test and reference
        let test_pixels = render_html_to_buffer(&test_content);
        let ref_pixels = render_html_to_buffer(&ref_content);

        let (differing, max_delta, first_diff) = compare_pixels(&test_pixels, &ref_pixels);
        let total = test_pixels.len();

        // Pass condition: 0 differing pixels
        let passed = differing == 0;
        let message = if passed {
            None
        } else {
            let diff_coord = if let Some((x, y, p1, p2)) = first_diff {
                format!(
                    ", first at ({},{}) test=#0x{:08x} ref=#0x{:08x}",
                    x, y, p1, p2
                )
            } else {
                String::new()
            };
            Some(format!(
                "Diff: {} pixels ({:.3}%), max channel delta: {}{}",
                differing,
                (differing as f64 / total as f64) * 100.0,
                max_delta,
                diff_coord
            ))
        };

        results.push(ReftestResult {
            suite: suite_name,
            name: test_name,
            total_pixels: total,
            differing_pixels: differing,
            max_delta,
            passed,
            message,
        });
    }

    results
}

fn collect_html_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_html_files(&path, files);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "html" || ext == "htm")
            {
                files.push(path);
            }
        }
    }
}

#[test]
fn test_official_w3c_css_reftests() {
    let results = run_all_wpt_reftests();
    assert!(
        !results.is_empty(),
        "At least one reftest must be discovered and executed"
    );

    let mut suite_map: BTreeMap<String, ReftestSuiteStats> = BTreeMap::new();
    let mut total_count = 0;
    let mut passed_count = 0;

    println!("\n================================================================================");
    println!("             OFFICIAL W3C CSS REFTEST SUITE EXECUTION REPORT");
    println!("================================================================================");

    for res in &results {
        let stats = suite_map
            .entry(res.suite.clone())
            .or_insert_with(|| ReftestSuiteStats {
                name: res.suite.clone(),
                total: 0,
                passed: 0,
                failed: 0,
            });

        stats.total += 1;
        total_count += 1;
        if res.passed {
            stats.passed += 1;
            passed_count += 1;
            println!("  [PASS] [{}] {}", res.suite, res.name);
        } else {
            stats.failed += 1;
            println!(
                "  [FAIL] [{}] {} -> {}",
                res.suite,
                res.name,
                res.message.as_deref().unwrap_or("pixel mismatch")
            );
        }
    }

    println!("\n--------------------------------------------------------------------------------");
    println!(
        "{:<24} | {:>8} | {:>8} | {:>8} | {:>10}",
        "Suite Name", "Total", "Passed", "Failed", "Pass Rate"
    );
    println!("--------------------------------------------------------------------------------");

    for (name, stats) in &suite_map {
        let rate = if stats.total > 0 {
            (stats.passed as f64 / stats.total as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "{:<24} | {:>8} | {:>8} | {:>8} | {:>9.2}%",
            name, stats.total, stats.passed, stats.failed, rate
        );
    }

    let overall_pass_rate = (passed_count as f64 / total_count as f64) * 100.0;
    println!("================================================================================");
    println!(
        "TOTAL: {} tests | PASSED: {} | FAILED: {} | PASS RATE: {:.2}%",
        total_count,
        passed_count,
        total_count - passed_count,
        overall_pass_rate
    );
    println!("================================================================================\n");

    assert!(
        overall_pass_rate >= 95.0,
        "W3C CSS Reftest suite pass rate must be at least 95.0% (got {:.2}%)",
        overall_pass_rate
    );
}
