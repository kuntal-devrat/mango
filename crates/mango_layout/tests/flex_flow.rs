use mango_core::Rect;
use mango_css::parser::parse_stylesheet;
use mango_html::parse_html;
use mango_layout::box_tree::build_box_tree;
use mango_layout::dimensions::Dimensions;
use mango_layout::flex_flow::layout_flex;
use mango_layout::float::FloatContext;
use mango_layout::style_tree::build_style_tree;

fn layout_html_flex(
    html: &str,
    css: &str,
    cb_w: f32,
    cb_h: f32,
) -> mango_layout::box_tree::LayoutBox {
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css);
    let style_tree = build_style_tree(&doc, &[&sheet]).expect("failed to build style tree");
    let mut box_tree = build_box_tree(&style_tree);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, cb_w, cb_h));
    let mut float_ctx = FloatContext::new();

    fn find_flex_mut(
        node: &mut mango_layout::box_tree::LayoutBox,
    ) -> Option<&mut mango_layout::box_tree::LayoutBox> {
        if let Some(s) = &node.style
            && matches!(
                s.display,
                mango_css::values::Display::Flex | mango_css::values::Display::InlineFlex
            )
        {
            return Some(node);
        }
        for child in &mut node.children {
            if let Some(f) = find_flex_mut(child) {
                return Some(f);
            }
        }
        None
    }

    let flex = find_flex_mut(&mut box_tree).expect("no flex container found in DOM");
    layout_flex(flex, &cb, &mut float_ctx);
    flex.clone()
}

#[test]
fn test_flex_flow_shorthand_expansion() {
    let html = r#"<div class="container"><div class="item">A</div><div class="item">B</div></div>"#;
    let css = r#"
        .container {
            display: flex;
            flex-flow: column wrap-reverse;
            width: 200px;
            height: 200px;
        }
        .item {
            height: 40px;
            width: 80px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    let style = container.style.unwrap();
    assert_eq!(
        style.flex_direction,
        mango_css::values::FlexDirection::Column
    );
    assert_eq!(style.flex_wrap, mango_css::values::FlexWrap::WrapReverse);
}

#[test]
fn test_align_content_distribution() {
    // 2 lines in a 200px high container with 40px lines. Total line height = 80px.
    // Remaining cross space = 120px.
    let html = r#"
        <div class="container">
            <div class="item">1</div><div class="item">2</div>
            <div class="item">3</div><div class="item">4</div>
        </div>
    "#;

    // 1. align-content: flex-start
    let css_start = r#"
        .container {
            display: flex;
            flex-wrap: wrap;
            align-content: flex-start;
            width: 100px;
            height: 200px;
        }
        .item {
            width: 50px;
            height: 40px;
        }
    "#;
    let c_start = layout_html_flex(html, css_start, 300.0, 300.0);
    // Line 1 item 0 y should be 0.0, line 2 item 2 y should be 40.0
    assert_eq!(c_start.children[0].dimensions.content.y(), 0.0);
    assert_eq!(c_start.children[2].dimensions.content.y(), 40.0);

    // 2. align-content: flex-end
    let css_end = r#"
        .container {
            display: flex;
            flex-wrap: wrap;
            align-content: flex-end;
            width: 100px;
            height: 200px;
        }
        .item {
            width: 50px;
            height: 40px;
        }
    "#;
    let c_end = layout_html_flex(html, css_end, 300.0, 300.0);
    // Free space = 120px. Line 1 y should start at 120.0, line 2 y at 160.0
    assert_eq!(c_end.children[0].dimensions.content.y(), 120.0);
    assert_eq!(c_end.children[2].dimensions.content.y(), 160.0);

    // 3. align-content: center
    let css_center = r#"
        .container {
            display: flex;
            flex-wrap: wrap;
            align-content: center;
            width: 100px;
            height: 200px;
        }
        .item {
            width: 50px;
            height: 40px;
        }
    "#;
    let c_center = layout_html_flex(html, css_center, 300.0, 300.0);
    // Free space = 120px / 2 = 60px offset. Line 1 y = 60.0, Line 2 y = 100.0
    assert_eq!(c_center.children[0].dimensions.content.y(), 60.0);
    assert_eq!(c_center.children[2].dimensions.content.y(), 100.0);

    // 4. align-content: space-between
    let css_sb = r#"
        .container {
            display: flex;
            flex-wrap: wrap;
            align-content: space-between;
            width: 100px;
            height: 200px;
        }
        .item {
            width: 50px;
            height: 40px;
        }
    "#;
    let c_sb = layout_html_flex(html, css_sb, 300.0, 300.0);
    // Line 1 at top (0.0), Line 2 at bottom (200 - 40 = 160.0)
    assert_eq!(c_sb.children[0].dimensions.content.y(), 0.0);
    assert_eq!(c_sb.children[2].dimensions.content.y(), 160.0);

    // 5. align-content: stretch
    let css_stretch = r#"
        .container {
            display: flex;
            flex-wrap: wrap;
            align-content: stretch;
            width: 100px;
            height: 200px;
        }
        .item {
            width: 50px;
        }
    "#;
    let c_stretch = layout_html_flex(html, css_stretch, 300.0, 300.0);
    // Both lines should stretch to 100px each (200px / 2)
    assert_eq!(c_stretch.children[0].dimensions.content.height(), 100.0);
    assert_eq!(c_stretch.children[2].dimensions.content.height(), 100.0);
    assert_eq!(c_stretch.children[2].dimensions.content.y(), 100.0);
}

#[test]
fn test_flex_container_intrinsic_sizing() {
    let html = r#"
        <div class="container">
            <div class="item">Short</div>
            <div class="item">LongerTextString</div>
        </div>
    "#;
    let css_min = r#"
        .container {
            display: flex;
            width: min-content;
        }
        .item {
            font-size: 16px;
        }
    "#;
    let c_min = layout_html_flex(html, css_min, 600.0, 600.0);
    // min-content width for row flex container is the max of the items' min-content
    assert!(c_min.dimensions.content.width() > 0.0);
    assert!(c_min.dimensions.content.width() < 300.0);

    let css_max = r#"
        .container {
            display: flex;
            width: max-content;
        }
        .item {
            font-size: 16px;
        }
    "#;
    let c_max = layout_html_flex(html, css_max, 600.0, 600.0);
    // max-content width is the sum of items' max-content
    assert!(c_max.dimensions.content.width() >= c_min.dimensions.content.width());
}

#[test]
fn test_flex_item_intrinsic_basis() {
    let html = r#"
        <div class="container">
            <div class="item-content">Content Basis Item</div>
            <div class="item-fixed">Fixed Item</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 500px;
        }
        .item-content {
            flex-basis: content;
        }
        .item-fixed {
            flex-basis: 100px;
        }
    "#;
    let container = layout_html_flex(html, css, 600.0, 600.0);
    assert_eq!(container.children[1].dimensions.content.width(), 100.0);
    assert!(container.children[0].dimensions.content.width() > 50.0);
}

#[test]
fn test_nested_flex_containers() {
    let html = r#"
        <div class="outer">
            <div class="nested-col">
                <div class="sub-item">C1</div>
                <div class="sub-item">C2</div>
            </div>
            <div class="sibling">Sibling</div>
        </div>
    "#;
    let css = r#"
        .outer {
            display: flex;
            flex-direction: row;
            width: 400px;
            height: 200px;
        }
        .nested-col {
            display: flex;
            flex-direction: column;
            width: 150px;
        }
        .sub-item {
            height: 50px;
        }
        .sibling {
            flex-grow: 1;
        }
    "#;
    let outer = layout_html_flex(html, css, 500.0, 500.0);
    assert_eq!(outer.children[0].dimensions.content.width(), 150.0);
    assert_eq!(outer.children[1].dimensions.content.width(), 250.0); // 400 - 150 = 250
    // Verify nested children positioned inside nested-col
    let nested = &outer.children[0];
    assert_eq!(nested.children[0].dimensions.content.height(), 50.0);
    assert_eq!(nested.children[1].dimensions.content.height(), 50.0);
    assert_eq!(nested.children[1].dimensions.content.y(), 50.0);
}

#[test]
fn test_flex_order_property_sorting() {
    let html = r#"
        <div class="container">
            <div id="first">A</div>
            <div id="second">B</div>
            <div id="third">C</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 300px;
        }
        #first { order: 2; width: 50px; }
        #second { order: -1; width: 70px; }
        #third { order: 1; width: 60px; }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Sorted order: #second (order: -1), #third (order: 1), #first (order: 2)
    assert_eq!(container.children[0].get_attribute("id"), Some("second"));
    assert_eq!(container.children[1].get_attribute("id"), Some("third"));
    assert_eq!(container.children[2].get_attribute("id"), Some("first"));

    // Verify horizontal positions follow sorted order
    assert_eq!(container.children[0].dimensions.content.x(), 0.0);
    assert_eq!(container.children[1].dimensions.content.x(), 70.0);
    assert_eq!(container.children[2].dimensions.content.x(), 130.0);
}

#[test]
fn test_flex_line_wrapping_with_gaps_and_align_content() {
    let html = r#"
        <div class="container">
            <div class="item">1</div><div class="item">2</div>
            <div class="item">3</div><div class="item">4</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            flex-wrap: wrap;
            row-gap: 10px;
            column-gap: 20px;
            align-content: space-around;
            width: 100px;
            height: 200px;
        }
        .item {
            width: 40px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 300.0, 300.0);
    // Two lines of 30px height with 10px row-gap:
    // Total lines cross = 30 + 10 + 30 = 70px.
    // Free cross space = 200 - 70 = 130px.
    // SpaceAround with 2 lines: per_line = 130 / 2 = 65px.
    // Line 1 y offset = 65 / 2 = 32.5px.
    // Line 2 y offset = 32.5 + 30 (line 1) + 10 (gap) + 65 (per_line) = 137.5px.
    assert_eq!(container.children[0].dimensions.content.y(), 32.5);
    assert_eq!(container.children[2].dimensions.content.y(), 137.5);
}
