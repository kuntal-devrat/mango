use mango_core::Rect;
use mango_css::parser::parse_stylesheet;
use mango_html::parse_html;
use mango_layout::box_tree::build_box_tree;
use mango_layout::dimensions::Dimensions;
use mango_layout::float::FloatContext;
use mango_layout::style_tree::build_style_tree;
use mango_layout::table_flow::layout_table;

fn layout_html_table(html: &str, css: &str, cb_w: f32, cb_h: f32) -> mango_layout::box_tree::LayoutBox {
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css);
    let style_tree = build_style_tree(&doc, &[&sheet]).expect("failed to build style tree");
    let mut box_tree = build_box_tree(&style_tree);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, cb_w, cb_h));
    let mut float_ctx = FloatContext::new();

    // Find the table box in the tree
    fn find_table_mut<'a>(node: &'a mut mango_layout::box_tree::LayoutBox) -> Option<&'a mut mango_layout::box_tree::LayoutBox> {
        if node.tag_name.as_deref() == Some("table")
            || node.style.as_ref().map(|s| s.display) == Some(mango_css::values::Display::Table)
        {
            return Some(node);
        }
        for child in &mut node.children {
            if let Some(t) = find_table_mut(child) {
                return Some(t);
            }
        }
        None
    }

    let table = find_table_mut(&mut box_tree).expect("no table found in DOM");
    layout_table(table, &cb, &mut float_ctx);
    table.clone()
}

fn get_table_rows(table: &mango_layout::box_tree::LayoutBox) -> Vec<&mango_layout::box_tree::LayoutBox> {
    let mut rows = Vec::new();
    for c in &table.children {
        if c.tag_name.as_deref() == Some("tr") {
            rows.push(c);
        } else if matches!(c.tag_name.as_deref(), Some("tbody") | Some("thead") | Some("tfoot")) {
            for r in &c.children {
                if r.tag_name.as_deref() == Some("tr") {
                    rows.push(r);
                }
            }
        }
    }
    rows
}

fn get_box_text(node: &mango_layout::box_tree::LayoutBox) -> String {
    let mut s = String::new();
    if let Some(t) = node.text() {
        s.push_str(t);
    }
    for c in &node.children {
        s.push_str(&get_box_text(c));
    }
    s
}

#[test]
fn test_fixed_table_layout() {
    // In fixed table layout with 400px table and 2 columns:
    // First column has col style width 100px.
    // Second column has no explicit width.
    // The second column receives the remaining width (approx 300px - spacing).
    let html = r#"
        <table style="table-layout: fixed; width: 400px; border-spacing: 0;">
            <colgroup>
                <col style="width: 100px;">
                <col>
            </colgroup>
            <tr>
                <td>First col content is quite long and should not affect width</td>
                <td>Second</td>
            </tr>
            <tr>
                <td>Row 2 cell 1</td>
                <td>Row 2 cell 2</td>
            </tr>
        </table>
    "#;
    let table = layout_html_table(html, "", 800.0, 600.0);
    assert_eq!(table.dimensions.content.width(), 400.0);

    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 2);
    let row0 = rows[0];
    let cell0 = &row0.children[0];
    let cell1 = &row0.children[1];

    let w0 = cell0.dimensions.margin_box().width();
    let w1 = cell1.dimensions.margin_box().width();
    assert!((w0 - 100.0).abs() < 2.0, "cell0 expected ~100px, got {}", w0);
    assert!((w1 - 300.0).abs() < 2.0, "cell1 expected ~300px, got {}", w1);
}

#[test]
fn test_colspan_and_rowspan_spanning() {
    let html = r#"
        <table style="width: 300px; border-spacing: 0;">
            <tr>
                <td colspan="2" style="height: 40px;">Header spanning 2</td>
                <td rowspan="2" style="height: 100px;">Side</td>
            </tr>
            <tr>
                <td style="height: 30px;">Cell A</td>
                <td style="height: 30px;">Cell B</td>
            </tr>
        </table>
    "#;
    let table = layout_html_table(html, "", 500.0, 500.0);
    assert_eq!(table.dimensions.content.width(), 300.0);

    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 2);

    // Row 0 has the 2-col header and the 2-row side cell
    // Row 1 has Cell A and Cell B
    // Side cell requires 100px height. Row 0 was 40px, Row 1 was 30px (sum 70px).
    // The deficit of 30px should be distributed across rows so the sum of row heights is >= 100px!
    let r0_h = rows[0].dimensions.content.height();
    let r1_h = rows[1].dimensions.content.height();
    assert!(r0_h + r1_h >= 99.0, "Sum of row heights must accommodate 100px rowspan cell, got {}", r0_h + r1_h);

    // Spanning header cell should span 2 columns and be wider than single column side cell
    let cell_hdr = &rows[0].children[0];
    let cell_side = &rows[0].children[1];
    assert!(cell_hdr.dimensions.margin_box().width() > cell_side.dimensions.margin_box().width());
}

#[test]
fn test_border_collapse_model() {
    let html = r#"
        <table style="border-collapse: collapse; width: 200px;">
            <tr>
                <td style="border: 2px solid black;">A</td>
                <td style="border: 4px solid red;">B</td>
            </tr>
            <tr>
                <td style="border: 1px solid green;">C</td>
                <td style="border: 2px solid blue;">D</td>
            </tr>
        </table>
    "#;
    let table = layout_html_table(html, "", 400.0, 400.0);

    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 2);

    // In collapsed borders, border-spacing is 0
    let cell_a = &rows[0].children[0];
    let cell_b = &rows[0].children[1];
    // Between A and B, B has 4px border which overrides A's 2px border!
    // A's right border should be 4px and B's left border collapsed to 0px
    assert_eq!(cell_a.dimensions.border.right, 4.0);
    assert_eq!(cell_b.dimensions.border.left, 0.0);
}

#[test]
fn test_caption_positioning() {
    let html = r#"
        <table style="width: 200px; border-spacing: 0;">
            <caption style="caption-side: top; height: 30px;">Top Caption</caption>
            <tr><td style="height: 50px;">Row 1</td></tr>
            <caption style="caption-side: bottom; height: 25px;">Bottom Caption</caption>
        </table>
    "#;
    let table = layout_html_table(html, "", 500.0, 500.0);

    let cap_top = table.children.iter().find(|c| get_box_text(c).contains("Top Caption")).expect("top caption");
    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    let cap_bot = table.children.iter().find(|c| get_box_text(c).contains("Bottom Caption")).expect("bottom caption");

    assert!(cap_top.dimensions.content.origin.y < row.dimensions.content.origin.y, "Top caption should be above row");
    assert!(row.dimensions.content.origin.y < cap_bot.dimensions.content.origin.y, "Bottom caption should be below row");
}

#[test]
fn test_colgroup_width_distribution() {
    let html = r#"
        <table style="width: 300px; border-spacing: 0;">
            <colgroup>
                <col width="80">
                <col width="120">
                <col width="100">
            </colgroup>
            <tr>
                <td>C1</td>
                <td>C2</td>
                <td>C3</td>
            </tr>
        </table>
    "#;
    let table = layout_html_table(html, "", 500.0, 500.0);
    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    let c1 = &row.children[0];
    let c2 = &row.children[1];
    let c3 = &row.children[2];

    assert!((c1.dimensions.margin_box().width() - 80.0).abs() < 2.0);
    assert!((c2.dimensions.margin_box().width() - 120.0).abs() < 2.0);
    assert!((c3.dimensions.margin_box().width() - 100.0).abs() < 2.0);
}

#[test]
fn test_nested_tables() {
    let html = r#"
        <table style="border-spacing: 0;">
            <tr>
                <td>
                    <table style="width: 150px; border-spacing: 0;">
                        <tr><td style="width: 150px;">Nested cell</td></tr>
                    </table>
                </td>
                <td style="width: 100px;">Outer cell 2</td>
            </tr>
        </table>
    "#;
    let table = layout_html_table(html, "", 600.0, 400.0);
    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    let cell1 = &row.children[0];
    // Cell 1 contains a 150px nested table, so its width must be at least 150px
    assert!(cell1.dimensions.content.width() >= 149.0, "Outer cell 1 containing 150px nested table must be at least 150px, got {}", cell1.dimensions.content.width());
}

#[test]
fn test_table_percentage_heights() {
    let html = r#"
        <table style="width: 200px; height: 200px; border-spacing: 0;">
            <tr style="height: 50%;">
                <td>Row 1 (50%)</td>
            </tr>
            <tr style="height: 50%;">
                <td>Row 2 (50%)</td>
            </tr>
        </table>
    "#;
    let table = layout_html_table(html, "", 400.0, 400.0);
    let rows = get_table_rows(&table);
    assert_eq!(rows.len(), 2);

    let h0 = rows[0].dimensions.content.height();
    let h1 = rows[1].dimensions.content.height();

    assert!((h0 - 100.0).abs() < 5.0, "Row 0 expected ~100px (50% of 200px), got {}", h0);
    assert!((h1 - 100.0).abs() < 5.0, "Row 1 expected ~100px (50% of 200px), got {}", h1);
    assert!((table.dimensions.content.height() - 200.0).abs() < 5.0);
}
