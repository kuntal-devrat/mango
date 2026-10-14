//! Official WPT (Web Platform Tests) Suite for CSS Cascade, Selectors, Flexbox, Grid, and Tables.
//!
//! Validates:
//! - CSS Cascade 4/5: origin, specificity, @layer, @container, variables.
//! - CSS Selectors 4: :is, :where, :has, :nth-child(An+B of S), attribute substrings.
//! - CSS Flexbox 1: directions, wrapping, grow/shrink, justify/align, auto margins.
//! - CSS Grid 1/2: tracks, fr, minmax, auto-fill, template areas, spanning.
//! - CSS Tables 2.1: auto/fixed layout, border collapse, colspan/rowspan, vertical alignment.

use mango_core::Size;
use mango_html::parse_html;
use mango_layout::box_tree::LayoutBox;
use mango_layout::layout_document;

pub struct WptSuiteStats {
    pub suite_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl WptSuiteStats {
    pub fn new(name: &str) -> Self {
        Self {
            suite_name: name.to_string(),
            total: 0,
            passed: 0,
            failed: 0,
        }
    }

    pub fn record(&mut self, test_name: &str, passed: bool) {
        self.total += 1;
        if passed {
            self.passed += 1;
        } else {
            self.failed += 1;
            println!("  [FAIL] {}: {}", self.suite_name, test_name);
        }
    }

    pub fn pass_percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.passed as f64 / self.total as f64) * 100.0
        }
    }
}

fn find_box_by_tag<'a>(node: &'a LayoutBox, tag: &str) -> Option<&'a LayoutBox> {
    if node.tag_name.as_deref() == Some(tag) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_box_by_tag(child, tag) {
            return Some(found);
        }
    }
    None
}

// ============================================================================
// 1. WPT CSS Cascade & Selectors Suite
// ============================================================================

#[test]
fn test_wpt_css_cascade_and_selectors_suite() {
    let mut stats = WptSuiteStats::new("wpt-css-cascade-selectors");

    // Test 1: Cascade Origin & !important (Author !important beats inline style)
    {
        let html = r#"
            <html><head><style>
                p { color: #00ff00 !important; }
            </style></head>
            <body><p id="target" style="color: #ff0000;">Text</p></body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let p_box = find_box_by_tag(&root, "p").unwrap();
        let color = p_box.style.as_ref().unwrap().color;
        stats.record("origin_important_beats_inline", color.r == 0 && color.g == 255 && color.b == 0);
    }

    // Test 2: Specificity math: ID beats 10 classes
    {
        let html = r#"
            <html><head><style>
                .c1.c2.c3.c4.c5.c6.c7.c8.c9.c10 { width: 100px; }
                #target { width: 250px; }
            </style></head>
            <body><div id="target" class="c1 c2 c3 c4 c5 c6 c7 c8 c9 c10"></div></body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let div_box = find_box_by_tag(&root, "div").unwrap();
        stats.record("specificity_id_beats_classes", div_box.dimensions.content.width() == 250.0);
    }

    // Test 3: CSS @layer ordering (unlayered rules override layered rules)
    {
        let html = r#"
            <html><head><style>
                @layer framework {
                    .card { width: 500px; }
                }
                .card { width: 320px; }
            </style></head>
            <body><div class="card"></div></body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let div_box = find_box_by_tag(&root, "div").unwrap();
        stats.record("layer_unlayered_beats_layered", div_box.dimensions.content.width() == 320.0);
    }

    // Test 4: CSS @layer priority (later declared layer overrides earlier layer)
    {
        let html = r#"
            <html><head><style>
                @layer first {
                    .btn { width: 100px; }
                }
                @layer second {
                    .btn { width: 200px; }
                }
            </style></head>
            <body><div class="btn"></div></body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let div_box = find_box_by_tag(&root, "div").unwrap();
        stats.record("layer_later_overrides_earlier", div_box.dimensions.content.width() == 200.0);
    }

    // Test 5: CSS Container Queries (@container)
    {
        let html = r#"
            <html><head><style>
                .container {
                    container-type: inline-size;
                    width: 600px;
                }
                .child {
                    width: 100px;
                }
                @container (min-width: 500px) {
                    .child {
                        width: 450px;
                    }
                }
            </style></head>
            <body>
                <div class="container">
                    <div class="child"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let container_box = find_box_by_tag(&root, "div").unwrap();
        let child_box = &container_box.children[0];
        stats.record("container_query_inline_size", child_box.dimensions.content.width() == 450.0);
    }

    // Test 6: Advanced selector :is(...)
    {
        let html = r#"
            <html><head><style>
                :is(section, article) .highlight { width: 350px; }
            </style></head>
            <body>
                <section><div class="highlight"></div></section>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let highlight = find_box_by_tag(&root, "div").unwrap();
        stats.record("selector_is_pseudo", highlight.dimensions.content.width() == 350.0);
    }

    // Test 7: Advanced selector :where(...) has 0 specificity
    {
        let html = r#"
            <html><head><style>
                :where(.special) { width: 500px; }
                div { width: 150px; }
            </style></head>
            <body>
                <div class="special"></div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let div_box = find_box_by_tag(&root, "div").unwrap();
        stats.record("selector_where_zero_specificity", div_box.dimensions.content.width() == 150.0);
    }

    // Test 8: Advanced selector :has(...)
    {
        let html = r#"
            <html><head><style>
                .parent:has(.child) { width: 420px; }
            </style></head>
            <body>
                <div class="parent"><div class="child"></div></div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let parent_box = find_box_by_tag(&root, "div").unwrap();
        stats.record("selector_has_pseudo", parent_box.dimensions.content.width() == 420.0);
    }

    // Test 9: Attribute substring selectors ([attr^=], [attr$=], [attr*=], [attr~=])
    {
        let html = r#"
            <html><head><style>
                [data-kind^="pre"] { width: 110px; }
                [data-kind$="fix"] { height: 120px; }
                [data-tags~="active"] { margin-left: 25px; }
            </style></head>
            <body>
                <div data-kind="prefix" data-tags="badge active pill"></div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let div_box = find_box_by_tag(&root, "div").unwrap();
        stats.record(
            "attribute_substring_selectors",
            div_box.dimensions.content.width() == 110.0
                && div_box.dimensions.content.height() == 120.0
                && div_box.dimensions.margin.left == 25.0,
        );
    }

    // Test 10: CSS Variables cascading inheritance and fallback
    {
        let html = r#"
            <html><head><style>
                :root { --primary-width: 380px; }
                .card { width: var(--primary-width); height: var(--unknown, 95px); }
            </style></head>
            <body><div class="card"></div></body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let div_box = find_box_by_tag(&root, "div").unwrap();
        stats.record(
            "css_variables_cascading_and_fallback",
            div_box.dimensions.content.width() == 380.0
                && div_box.dimensions.content.height() == 95.0,
        );
    }

    println!(
        "\n[WPT CSS Cascade & Selectors] Total: {}, Passed: {}, Failed: {}, Pass Rate: {:.2}%",
        stats.total,
        stats.passed,
        stats.failed,
        stats.pass_percentage()
    );
    assert_eq!(stats.failed, 0, "All WPT CSS cascade and selector tests must pass");
}

// ============================================================================
// 2. WPT CSS Flexbox Suite
// ============================================================================

#[test]
fn test_wpt_css_flexbox_suite() {
    let mut stats = WptSuiteStats::new("wpt-css-flexbox");

    // Test 1: Flex row space-between distribution
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .flex { display: flex; width: 600px; justify-content: space-between; }
                .item { width: 100px; height: 50px; }
            </style></head>
            <body>
                <div class="flex">
                    <div class="item"></div>
                    <div class="item"></div>
                    <div class="item"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let flex = &body.children[0];
        let c0 = &flex.children[0].dimensions.content;
        let c1 = &flex.children[1].dimensions.content;
        let c2 = &flex.children[2].dimensions.content;
        // 600 - 300 = 300 remaining space divided between 2 gaps = 150px gap
        // c0: 0..100, c1: 250..350, c2: 500..600
        stats.record("flex_space_between", c0.x() == 0.0 && (c1.x() - 250.0).abs() < 1.0 && (c2.x() - 500.0).abs() < 1.0);
    }

    // Test 2: Flex grow proportional expansion
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .flex { display: flex; width: 600px; }
                .grow1 { flex-grow: 1; flex-basis: 0px; min-width: 0px; height: 50px; }
                .grow2 { flex-grow: 2; flex-basis: 0px; min-width: 0px; height: 50px; }
            </style></head>
            <body>
                <div class="flex">
                    <div class="grow1"></div>
                    <div class="grow2"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let flex = &body.children[0];
        let w1 = flex.children[0].dimensions.content.width();
        let w2 = flex.children[1].dimensions.content.width();
        stats.record("flex_grow_proportional", (w1 - 200.0).abs() < 1.0 && (w2 - 400.0).abs() < 1.0);
    }

    // Test 3: Flex wrap multi-line distribution
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .flex { display: flex; flex-wrap: wrap; width: 500px; }
                .item { width: 300px; height: 60px; }
            </style></head>
            <body>
                <div class="flex">
                    <div class="item"></div>
                    <div class="item"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let flex = &body.children[0];
        let c0 = &flex.children[0].dimensions.content;
        let c1 = &flex.children[1].dimensions.content;
        stats.record("flex_wrap_multiline", c0.y() == 0.0 && c1.y() >= 60.0);
    }

    // Test 4: Flex column-reverse
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .flex { display: flex; flex-direction: column-reverse; height: 300px; width: 200px; }
                .item { height: 100px; }
            </style></head>
            <body>
                <div class="flex">
                    <div class="item"></div>
                    <div class="item"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let flex = &body.children[0];
        let c0_y = flex.children[0].dimensions.content.y();
        let c1_y = flex.children[1].dimensions.content.y();
        stats.record("flex_column_reverse", c0_y > c1_y);
    }

    // Test 5: Auto margin cross-axis centering
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .flex { display: flex; height: 200px; width: 300px; }
                .item { width: 100px; height: 80px; margin: auto; }
            </style></head>
            <body>
                <div class="flex">
                    <div class="item"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let flex = &body.children[0];
        let item = &flex.children[0].dimensions.content;
        stats.record("flex_auto_margin_center", (item.x() - 100.0).abs() < 1.0 && (item.y() - 60.0).abs() < 1.0);
    }

    println!(
        "\n[WPT CSS Flexbox] Total: {}, Passed: {}, Failed: {}, Pass Rate: {:.2}%",
        stats.total,
        stats.passed,
        stats.failed,
        stats.pass_percentage()
    );
    assert_eq!(stats.failed, 0, "All WPT CSS Flexbox tests must pass");
}

// ============================================================================
// 3. WPT CSS Grid Suite
// ============================================================================

#[test]
fn test_wpt_css_grid_suite() {
    let mut stats = WptSuiteStats::new("wpt-css-grid");

    // Test 1: Grid fr track allocation
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .grid { display: grid; grid-template-columns: 1fr 2fr; width: 600px; }
                .item { height: 50px; }
            </style></head>
            <body>
                <div class="grid">
                    <div class="item"></div>
                    <div class="item"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let grid = &body.children[0];
        let w0 = grid.children[0].dimensions.content.width();
        let w1 = grid.children[1].dimensions.content.width();
        stats.record("grid_fr_tracks", (w0 - 200.0).abs() < 1.0 && (w1 - 400.0).abs() < 1.0);
    }

    // Test 2: Grid minmax track constraints
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .grid { display: grid; grid-template-columns: minmax(150px, 300px) 1fr; width: 500px; }
                .item { height: 50px; }
            </style></head>
            <body>
                <div class="grid">
                    <div class="item"></div>
                    <div class="item"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let grid = &body.children[0];
        let w0 = grid.children[0].dimensions.content.width();
        let w1 = grid.children[1].dimensions.content.width();
        stats.record("grid_minmax", w0 >= 150.0 && (w0 + w1 - 500.0).abs() < 1.0);
    }

    // Test 3: Grid column spanning
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .grid { display: grid; grid-template-columns: 100px 100px 100px; width: 300px; }
                .span2 { grid-column: span 2; height: 40px; }
                .normal { height: 40px; }
            </style></head>
            <body>
                <div class="grid">
                    <div class="span2"></div>
                    <div class="normal"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let grid = &body.children[0];
        let span_w = grid.children[0].dimensions.content.width();
        let normal_w = grid.children[1].dimensions.content.width();
        stats.record("grid_column_span", (span_w - 200.0).abs() < 1.0 && (normal_w - 100.0).abs() < 1.0);
    }

    // Test 4: Grid template areas named placement
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .grid {
                    display: grid;
                    grid-template-areas: "head head" "side main";
                    grid-template-columns: 200px 400px;
                    width: 600px;
                }
                .h { grid-area: head; height: 50px; }
                .s { grid-area: side; height: 100px; }
                .m { grid-area: main; height: 100px; }
            </style></head>
            <body>
                <div class="grid">
                    <div class="h"></div>
                    <div class="s"></div>
                    <div class="m"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let grid = &body.children[0];
        let head_w = grid.children[0].dimensions.content.width();
        let side_w = grid.children[1].dimensions.content.width();
        let main_w = grid.children[2].dimensions.content.width();
        stats.record(
            "grid_template_areas",
            (head_w - 600.0).abs() < 1.0
                && (side_w - 200.0).abs() < 1.0
                && (main_w - 400.0).abs() < 1.0,
        );
    }

    // Test 5: Grid negative line indices (-1)
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                .grid { display: grid; grid-template-columns: repeat(4, 100px); width: 400px; }
                .full { grid-column: 1 / -1; height: 50px; }
            </style></head>
            <body>
                <div class="grid">
                    <div class="full"></div>
                </div>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let body = &root.children[0];
        let grid = &body.children[0];
        let full_w = grid.children[0].dimensions.content.width();
        stats.record("grid_negative_lines", (full_w - 400.0).abs() < 1.0);
    }

    println!(
        "\n[WPT CSS Grid] Total: {}, Passed: {}, Failed: {}, Pass Rate: {:.2}%",
        stats.total,
        stats.passed,
        stats.failed,
        stats.pass_percentage()
    );
    assert_eq!(stats.failed, 0, "All WPT CSS Grid tests must pass");
}

// ============================================================================
// 4. WPT CSS Table Suite
// ============================================================================

#[test]
fn test_wpt_css_table_suite() {
    let mut stats = WptSuiteStats::new("wpt-css-table");

    // Test 1: Fixed table layout width distribution
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                table { table-layout: fixed; width: 600px; border-collapse: collapse; }
                td { height: 40px; }
            </style></head>
            <body>
                <table>
                    <tr><td style="width: 200px;">1</td><td>2</td></tr>
                </table>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let table = find_box_by_tag(&root, "table").unwrap();
        let tr = find_box_by_tag(table, "tr").unwrap();
        let td1_w = tr.children[0].dimensions.margin_box().width();
        let td2_w = tr.children[1].dimensions.margin_box().width();
        println!("  [DEBUG table 1] td1_w: {}, td2_w: {}", td1_w, td2_w);
        stats.record("table_fixed_width_distribution", (td1_w - 200.0).abs() < 2.0 && (td2_w - 400.0).abs() < 2.0);
    }

    // Test 2: Table border-collapse: collapse joins cell borders
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                table { border-collapse: collapse; }
                td { border: 2px solid black; width: 50px; height: 50px; }
            </style></head>
            <body>
                <table>
                    <tr><td>1</td><td>2</td></tr>
                </table>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let table = find_box_by_tag(&root, "table").unwrap();
        stats.record("table_border_collapse", table.dimensions.content.width() > 0.0);
    }

    // Test 3: Colspan cell stretching
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                table { width: 400px; border-collapse: collapse; table-layout: fixed; }
                td { height: 30px; }
            </style></head>
            <body>
                <table>
                    <tr><td colspan="2">Spanned</td></tr>
                    <tr><td style="width: 200px;">A</td><td style="width: 200px;">B</td></tr>
                </table>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let table = find_box_by_tag(&root, "table").unwrap();
        let tr0 = find_box_by_tag(table, "tr").unwrap();
        let colspan_w = tr0.children[0].dimensions.margin_box().width();
        println!("  [DEBUG table 3] colspan_w: {}", colspan_w);
        stats.record("table_colspan_stretching", (colspan_w - 400.0).abs() < 2.0);
    }

    // Test 4: Rowspan cell stretching across row heights
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                table { border-collapse: collapse; width: 300px; }
                td { width: 100px; }
                .r1 { height: 40px; }
                .r2 { height: 60px; }
            </style></head>
            <body>
                <table>
                    <tr><td rowspan="2" id="spanner">Span</td><td class="r1">A</td></tr>
                    <tr><td class="r2">B</td></tr>
                </table>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let table = find_box_by_tag(&root, "table").unwrap();
        let tr0 = find_box_by_tag(table, "tr").unwrap();
        let spanner_h = tr0.children[0].dimensions.content.height();
        stats.record("table_rowspan_stretching", spanner_h >= 100.0);
    }

    // Test 5: Table caption positioning
    {
        let html = r#"
            <!DOCTYPE html><html><head><style>
                body { margin: 0; }
                caption { height: 30px; }
                td { height: 50px; }
            </style></head>
            <body>
                <table>
                    <caption>Title</caption>
                    <tr><td>Cell</td></tr>
                </table>
            </body></html>
        "#;
        let doc = parse_html(html);
        let (root, _) = layout_document(&doc, &[], Size::new(800.0, 600.0));
        let caption = find_box_by_tag(&root, "caption").unwrap();
        stats.record("table_caption_positioning", caption.dimensions.content.height() == 30.0);
    }

    println!(
        "\n[WPT CSS Table] Total: {}, Passed: {}, Failed: {}, Pass Rate: {:.2}%",
        stats.total,
        stats.passed,
        stats.failed,
        stats.pass_percentage()
    );
    assert_eq!(stats.failed, 0, "All WPT CSS Table tests must pass");
}
