//! Upstream W3C CSS visual reftest report for Mango.
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

#[path = "wpt/reftest_engine.rs"]
pub mod reftest_engine;

use std::fs;
use std::path::Path;

use mango_core::Size;
use mango_html::parse_html;
use mango_layout::layout_document_with_scroll_xy;
use reftest_engine::*;

#[test]
fn test_wpt_upstream_w3c_css_reftests() {
    let results = run_upstream_wpt_reftests(None);
    if results.is_empty() {
        return;
    }
    let (passed, total) = report_reftest_results("FULL UPSTREAM W3C CSS REFTEST REPORT", &results);
    // This is deliberately report-only. A non-zero result is not a
    // conformance claim; the current renderer is not at Chromium parity.
    let _ = (passed, total);
}

#[test]
fn test_wpt_upstream_w3c_css_box_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-box"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) = report_reftest_results("UPSTREAM W3C CSS-BOX REFTEST REPORT", &results);
}

#[test]
fn test_wpt_upstream_w3c_css_cascade_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-cascade"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) =
        report_reftest_results("UPSTREAM W3C CSS-CASCADE REFTEST REPORT", &results);
}

#[test]
fn test_wpt_upstream_w3c_css_position_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-position"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) =
        report_reftest_results("UPSTREAM W3C CSS-POSITION REFTEST REPORT", &results);
}

#[test]
fn test_wpt_upstream_w3c_css_flexbox_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-flexbox"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) =
        report_reftest_results("UPSTREAM W3C CSS-FLEXBOX REFTEST REPORT", &results);
}

#[test]
fn test_wpt_upstream_w3c_css_grid_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-grid"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) =
        report_reftest_results("UPSTREAM W3C CSS-GRID REFTEST REPORT", &results);
}

#[test]
fn test_wpt_upstream_w3c_css_tables_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-tables"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) =
        report_reftest_results("UPSTREAM W3C CSS-TABLES REFTEST REPORT", &results);
}

#[test]
fn test_wpt_upstream_w3c_css_inline_reftests() {
    let results = run_upstream_wpt_reftests(Some("css-inline"));
    if results.is_empty() {
        return;
    }
    let (_passed, _total) =
        report_reftest_results("UPSTREAM W3C CSS-INLINE REFTEST REPORT", &results);
}

#[test]
#[ignore = "in-progress vertical scrollbars investigation"]
fn test_wpt_overflow_auto_005_pixel() {
    assert_flexbox_reftest(
        "overflow-auto-005.html",
        "reference/overflow-auto-005-ref.html",
    );
}

#[test]
#[ignore = "in-progress vertical-lr scrollbars investigation"]
fn test_wpt_cross_axis_scrollbar_pixel() {
    assert_flexbox_reftest(
        "cross-axis-scrollbar.html",
        "reference/cross-axis-scrollbar-ref.html",
    );
}

#[test]
#[ignore = "in-progress content-height-with-scrollbars investigation"]
fn test_wpt_content_height_with_scrollbars_pixel() {
    assert_flexbox_reftest(
        "content-height-with-scrollbars.html",
        "reference/content-height-with-scrollbars-ref.html",
    );
}

#[test]
fn test_wpt_percentage_heights_002_pixel() {
    assert_flexbox_reftest(
        "percentage-heights-002.html",
        "percentage-heights-002-ref.html",
    );
}

#[test]
fn test_wpt_percentage_heights_004_pixel() {
    assert_flexbox_reftest(
        "percentage-heights-004.html",
        "percentage-heights-004-ref.html",
    );
}

#[test]
fn test_wpt_percentage_heights_015_pixel() {
    assert_flexbox_reftest(
        "percentage-heights-015.html",
        "percentage-heights-015-ref.html",
    );
}

#[test]
fn test_wpt_align_items_baseline_overflow_non_visible_pixel() {
    assert_flexbox_reftest(
        "align-items-baseline-overflow-non-visible.html",
        "reference/align-items-baseline-overflow-non-visible-ref.html",
    );
}

#[test]
fn test_wpt_auto_margins_001_pixel() {
    assert_flexbox_reftest("auto-margins-001.html", "auto-margins-001-ref.html");
}

#[test]
fn test_wpt_flexbox_fbfc_pixel() {
    assert_flexbox_reftest("flexbox_fbfc.html", "flexbox_fbfc-ref.html");
}

#[test]
fn test_wpt_flexbox_fbfc2_pixel() {
    assert_flexbox_reftest("flexbox_fbfc2.html", "flexbox_fbfc2-ref.html");
}

#[test]
fn test_wpt_position_fixed_001_pixel() {
    assert_flexbox_reftest(
        "position-fixed-001.html",
        "reference/position-fixed-001-ref.html",
    );
}

#[test]
fn test_wpt_stretch_flex_item_checkbox_input_pixel() {
    assert_flexbox_reftest(
        "stretch-flex-item-checkbox-input.html",
        "stretch-flex-item-checkbox-input-ref.html",
    );
}

#[test]
fn test_wpt_stretch_flex_item_radio_input_pixel() {
    assert_flexbox_reftest(
        "stretch-flex-item-radio-input.html",
        "stretch-flex-item-radio-input-ref.html",
    );
}

#[test]
fn test_wpt_select_element_zero_height_pixel() {
    assert_flexbox_reftest(
        "select-element-zero-height-001.html",
        "select-element-zero-height-001-ref.html",
    );
    assert_flexbox_reftest(
        "select-element-zero-height-002.html",
        "select-element-zero-height-002-ref.html",
    );
}

#[test]
#[ignore = "in-progress vertical-rl scrollbars investigation"]
fn test_wpt_scrollbars_pixel() {
    let base = upstream_root().join("css").join("css-flexbox");
    let test_file = base.join("scrollbars.html");
    let ref_file = base.join("scrollbars-ref.html");
    let test_html = std::fs::read_to_string(&test_file).unwrap();
    let ref_html = std::fs::read_to_string(&ref_file).unwrap();

    fn dump_flex(html: &str, base: &Path, label: &str) {
        let doc = parse_html(html);
        let sheets = collect_external_stylesheets(&doc, Some(base));
        let mut scripts = Vec::new();
        collect_scripts_in_order(&doc, doc.root(), &mut scripts);
        let mut rt = mango_js::JsRuntime::new(doc, 800.0, 600.0);
        let style_refs: Vec<&mango_css::parser::Stylesheet> = sheets.iter().collect();
        refresh_layout_bounds(&rt, 0.0, 0.0, &style_refs);
        for (src, inline) in scripts {
            if let Some(src) = src {
                if let Some(path) = resolve_script_path(&src, Some(base))
                    && let Ok(source) = fs::read_to_string(path)
                {
                    let _ = rt.execute_script(&source);
                }
            } else if !inline.trim().is_empty() {
                let _ = rt.execute_script(&inline);
            }
        }
        let doc2 = rt.document_ref().clone();
        let (box_tree, _) =
            layout_document_with_scroll_xy(&doc2, &style_refs, Size::new(800.0, 600.0), 0.0, 0.0);
        let mut count = 0;
        fn walk(b: &mango_layout::LayoutBox, label: &str, count: &mut usize) {
            let cls = b.get_attribute("class").unwrap_or("");
            if cls.contains("container ") {
                *count += 1;
                if *count <= 6 {
                    eprintln!(
                        "{label} CONTAINER #{count}: cls='{cls}' b_box={:?} content={:?} pad={:?} border={:?} margin={:?}",
                        b.dimensions.border_box(),
                        b.dimensions.content,
                        b.dimensions.padding,
                        b.dimensions.border,
                        b.dimensions.margin,
                    );
                    fn print_children(b: &mango_layout::LayoutBox, depth: usize) {
                        let indent = "   ".repeat(depth);
                        let ccls = b.get_attribute("class").unwrap_or("");
                        let tag = b.tag_name.as_deref().unwrap_or("?");
                        eprintln!(
                            "{indent}<{tag}> cls='{ccls}' b_box={:?} content={:?} disp={:?}",
                            b.dimensions.border_box(),
                            b.dimensions.content,
                            b.style.as_ref().map(|s| s.display),
                        );
                        for c in &b.children {
                            print_children(c, depth + 1);
                        }
                    }
                    for c in &b.children {
                        print_children(c, 1);
                    }
                }
            }
            for c in &b.children {
                walk(c, label, count);
            }
        }
        walk(&box_tree, label, &mut count);
    }
    dump_flex(&test_html, &base, "TEST");
    dump_flex(&ref_html, &base, "REF");

    let test_buf = render_html_to_buffer(&test_html, Some(&base));
    let ref_buf = render_html_to_buffer(&ref_html, Some(&base));
    let (diff_count, max_channel, first_diff, max_diff) = compare_pixels(&test_buf, &ref_buf);
    eprintln!(
        "scrollbars.html: diff={diff_count}, max_channel={max_channel}, first={first_diff:?}, max={max_diff:?}"
    );
    assert_eq!(diff_count, 0);
}
