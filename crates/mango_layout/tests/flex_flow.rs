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

#[test]
fn test_flex_basis_zero_equal_distribution() {
    // When flex: 1 (flex-basis: 0) is specified, items with different content length
    // must receive strictly equal width distribution (B2 fix).
    let html = r#"
        <div class="container">
            <div class="item">Short</div>
            <div class="item">ExtremelyLongContentTextStringThatTakesALotOfSpace</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 300px;
        }
        .item {
            flex-grow: 1;
            flex-shrink: 1;
            flex-basis: 0px;
            min-width: 0px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Both items must receive exactly 150px
    assert_eq!(container.children[0].dimensions.content.width(), 150.0);
    assert_eq!(container.children[1].dimensions.content.width(), 150.0);
}

#[test]
fn test_flex_grow_max_width_constraint_and_redistribution() {
    // When an item's growth violates max-width, it freezes at max-width and remaining
    // free space is redistributed to other unfrozen items (B3 fix, CSS Flexbox §9.7).
    let html = r#"
        <div class="container">
            <div class="item constrained">Item 1</div>
            <div class="item unconstrained">Item 2</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 300px;
        }
        .item {
            flex-grow: 1;
            flex-basis: 0px;
            min-width: 0px;
        }
        .constrained {
            max-width: 100px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Item 1 frozen at max-width 100px; remaining 200px redistributed to Item 2
    assert_eq!(container.children[0].dimensions.content.width(), 100.0);
    assert_eq!(container.children[1].dimensions.content.width(), 200.0);
}

#[test]
fn test_flex_out_of_flow_children_ignored() {
    // Absolute children must NOT participate as flex items and must not take flex space (B1 fix).
    let html = r#"
        <div class="container">
            <div class="item">Item 1</div>
            <div class="abs">Floating Note</div>
            <div class="item">Item 2</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            position: relative;
            width: 200px;
            height: 100px;
        }
        .item {
            flex-grow: 1;
            flex-basis: 0px;
            min-width: 0px;
        }
        .abs {
            position: absolute;
            top: 10px;
            left: 20px;
            width: 50px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // In-flow items receive 100px each
    assert_eq!(container.children[0].dimensions.content.width(), 100.0);
    assert_eq!(container.children[2].dimensions.content.width(), 100.0);

    // Absolute child is placed at its specified coordinates
    assert_eq!(container.children[1].dimensions.content.x(), 20.0);
    assert_eq!(container.children[1].dimensions.content.y(), 10.0);
    assert_eq!(container.children[1].dimensions.content.width(), 50.0);
    assert_eq!(container.children[1].dimensions.content.height(), 30.0);
}

#[test]
fn test_flex_row_reverse_and_column_reverse() {
    // In row-reverse, main-start is the right edge. Item 1 is at main-start (right edge),
    // and Item 2 is placed to its left (B4 fix).
    let html = r#"
        <div class="container">
            <div class="item1">1</div>
            <div class="item2">2</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            flex-direction: row-reverse;
            width: 300px;
        }
        .item1 {
            width: 60px;
        }
        .item2 {
            width: 80px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Item 1 (first flex item in DOM) is packed at right edge: x = 300 - 60 = 240
    assert_eq!(container.children[0].dimensions.content.x(), 240.0);
    // Item 2 is placed to the left of Item 1: x = 240 - 80 = 160
    assert_eq!(container.children[1].dimensions.content.x(), 160.0);
}

#[test]
fn test_flex_baseline_alignment() {
    // In row flex with align-items: baseline, text baselines of all items align (B5 fix).
    let html = r#"
        <div class="container">
            <div class="small">Small</div>
            <div class="large">Large</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            align-items: baseline;
            width: 300px;
        }
        .small {
            font-size: 10px;
            width: 50px;
        }
        .large {
            font-size: 30px;
            width: 50px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Real font metrics with half-leading (replacing old fs * 0.8 approximation):
    // large (30px) ascent is ~27.046px, small (10px) ascent is ~8.374px.
    // small item must be pushed down by (27.046 - 8.374) = ~18.672px so their baselines match!
    assert_eq!(container.children[0].dimensions.content.y(), 18.671875);
    assert_eq!(container.children[1].dimensions.content.y(), 0.0);
}

#[test]
fn test_flex_container_min_max_height() {
    // Flex container final height clamps by min-height and max-height (G1 fix).
    let html = r#"
        <div class="container">
            <div class="item">Content</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            min-height: 120px;
            max-height: 200px;
            width: 200px;
        }
        .item {
            height: 40px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    assert_eq!(container.dimensions.content.height(), 120.0);
}

#[test]
fn test_inline_flex_shrink_to_fit_width() {
    // inline-flex container with auto width should shrink-to-fit its items (G2 fix).
    let html = r#"
        <div class="container">
            <div class="item">A</div>
            <div class="item">B</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: inline-flex;
        }
        .item {
            width: 60px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 500.0, 500.0);
    // Content width should be sum of items (60 + 60 = 120px), not 500px containing block width!
    assert_eq!(container.dimensions.content.width(), 120.0);
}

#[test]
fn test_column_flex_align_items_shrink_to_fit() {
    // In column flex with align-items: center, auto-width items should shrink-to-fit
    // and not stretch to 100% container width (G3 fix).
    let html = r#"
        <div class="container">
            <div class="item">Fixed</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            flex-direction: column;
            align-items: center;
            width: 200px;
        }
        .item {
            width: 80px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Item width remains 80px and is centered horizontally at (200 - 80) / 2 = 60px
    assert_eq!(container.children[0].dimensions.content.width(), 80.0);
    assert_eq!(container.children[0].dimensions.content.x(), 60.0);
}

#[test]
fn test_flex_item_auto_min_width_prevents_zero_collapse() {
    // Under CSS Flexbox §4.5, flex items default to min-width: auto.
    // When flex-basis is 0 and flex-shrink is 1 with content, the item must not collapse to 0.
    let html = r#"
        <div class="container">
            <div class="item"><div class="inner" style="width: 120px; height: 20px;"></div></div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 200px;
        }
        .item {
            flex-shrink: 1;
            flex-basis: 0px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Item content width should not shrink below its child's intrinsic width 120px
    assert!(container.children[0].dimensions.content.width() >= 120.0);
}

#[test]
fn test_github_underlinenav_item_width() {
    let html = r#"
    <ul class="list">
      <li class="item">
        <a class="link">
          <span class="icon" style="display:inline-flex; width:16px; height:16px; margin-right:8px;"></span>
          <span class="text" data-content="Code">Code</span>
        </a>
      </li>
    </ul>
    "#;
    let css = r#"
    .list { display: flex; flex-wrap: wrap; }
    .item { display: flex; flex-direction: column; align-items: center; overflow: hidden; }
    .link { display: inline-flex; align-items: center; padding: 0 8px; margin: 0 7px; height: 32px; max-width: 100%; text-decoration: none; }
    .text { text-overflow: ellipsis; white-space: nowrap; overflow: hidden; }
    .text[data-content]:before { content: attr(data-content); font-weight: 600; visibility: hidden; white-space: nowrap; height: 0; display: block; }
    "#;
    let root = layout_html_flex(html, css, 1280.0, 900.0);
    let item = &root.children[0];
    let link = &item.children[0];
    let text = &link.children[1];
    let mut memo = std::collections::HashMap::new();
    let item_meas = mango_layout::table_flow::measure_box_intrinsic_widths(
        item,
        14.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
        &mut memo,
    );
    let mut memo2 = std::collections::HashMap::new();
    let link_meas = mango_layout::table_flow::measure_box_intrinsic_widths(
        link,
        14.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
        &mut memo2,
    );
    let mut memo3 = std::collections::HashMap::new();
    let text_meas = mango_layout::table_flow::measure_box_intrinsic_widths(
        text,
        14.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
        &mut memo3,
    );
    println!("item_meas: {:?}", item_meas);
    println!("link_meas: {:?}", link_meas);
    println!("text_meas: {:?}", text_meas);
    println!("item border_box: {:?}", item.dimensions.border_box());
    println!("link border_box: {:?}", link.dimensions.border_box());
    println!("text border_box: {:?}", text.dimensions.border_box());
    println!("text content: {:?}", text.dimensions.content);

    for c in &text.children {
        println!(
            "  text child: {:?} dims={:?}",
            c.box_type,
            c.dimensions.border_box()
        );
    }
    // Link content width should fit icon (16) + margin (8) + text (~35) = ~59px
    // Link border-box width = 59 + 16 (padding) = ~75px
    // Item border-box width = 75 + 14 (margin) = ~89px
    assert!(
        item.dimensions.border_box().width() >= 80.0,
        "item border box width must be >= 80px, got {}",
        item.dimensions.border_box().width()
    );
}

#[test]
fn test_flexbox_percentage_gap_resolution() {
    let html = r#"
        <div class="container">
            <div class="item">1</div>
            <div class="item">2</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 200px;
            gap: 10%;
        }
        .item {
            width: 50px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    assert_eq!(container.children[0].dimensions.content.x(), 0.0);
    assert_eq!(container.children[0].dimensions.content.width(), 50.0);
    assert_eq!(container.children[1].dimensions.content.x(), 70.0);
    assert_eq!(container.children[1].dimensions.content.width(), 50.0);
}

#[test]
fn test_flexbox_column_percentage_gap_resolution() {
    let html = r#"
        <div class="container">
            <div class="item">1</div>
            <div class="item">2</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            flex-direction: column;
            height: 200px;
            gap: 10%;
        }
        .item {
            width: 50px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    assert_eq!(container.children[0].dimensions.content.y(), 0.0);
    assert_eq!(container.children[0].dimensions.content.height(), 30.0);
    assert_eq!(container.children[1].dimensions.content.y(), 50.0);
    assert_eq!(container.children[1].dimensions.content.height(), 30.0);
}

#[test]
fn test_flexbox_percentage_gap_with_min_width() {
    let html = r#"
        <div class="container">
            <div class="item">1</div>
            <div class="item">2</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            width: 100px;
            min-width: 200px;
            gap: 10%;
        }
        .item {
            width: 50px;
            height: 30px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    assert_eq!(container.children[0].dimensions.content.x(), 0.0);
    assert_eq!(container.children[1].dimensions.content.x(), 70.0);
}

#[test]
fn test_flex_wrap_reverse_line_stacking() {
    let html = r#"
        <div class="container">
            <div class="item" id="i1">1</div>
            <div class="item" id="i2">2</div>
            <div class="item" id="i3">3</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            flex-wrap: wrap-reverse;
            width: 100px;
        }
        .item {
            width: 60px;
            height: 40px;
            margin-top: 5px;
        }
    "#;
    let container = layout_html_flex(html, css, 400.0, 400.0);
    // Line 0 has i1 (60px). Next item i2 (60px) wraps to Line 1. Next item i3 wraps to Line 2.
    // In wrap-reverse, Line 0 is at cross-bottom, Line 2 is at cross-top.
    // Line 2 (i3): y = 0 + margin-top = 5.0
    // Line 1 (i2): y = 45 + margin-top = 50.0
    // Line 0 (i1): y = 90 + margin-top = 95.0
    assert_eq!(container.children[2].dimensions.content.y(), 5.0);
    assert_eq!(container.children[1].dimensions.content.y(), 50.0);
    assert_eq!(container.children[0].dimensions.content.y(), 95.0);
}

#[test]
fn test_column_reverse_multiline_item_position() {
    let html = r#"
        <div id="target">
            <div class="short"></div>
            <div class="tall"></div>
        </div>
    "#;
    let css = r#"
        #target {
            display: flex;
            flex-flow: column-reverse wrap;
            align-content: flex-start;
            align-items: flex-start;
            width: 200px;
            max-height: 100px;
            margin: 0;
            border: 0;
            padding: 0;
        }
        #target > div {
            width: 50px;
        }
        .short { height: 40px; }
        .tall { height: 80px; }
    "#;
    let container = layout_html_flex(html, css, 800.0, 600.0);
    assert_eq!(container.dimensions.content.size.height, 80.0);
    assert_eq!(container.children[0].dimensions.content.x(), 0.0);
    assert_eq!(container.children[0].dimensions.content.y(), 40.0);
    assert_eq!(container.children[0].dimensions.content.size.width, 50.0);
    assert_eq!(container.children[0].dimensions.content.size.height, 40.0);
    assert_eq!(container.children[1].dimensions.content.x(), 50.0);
    assert_eq!(container.children[1].dimensions.content.y(), 0.0);
    assert_eq!(container.children[1].dimensions.content.size.width, 50.0);
    assert_eq!(container.children[1].dimensions.content.size.height, 80.0);
}

#[test]
fn test_flexible_order_dimensions() {
    let test_html = r#"<div class="box"><div class="red">A</div><div class="blue">B</div><div class="black">C</div></div>"#;
    let test_css = r#"
        .box { margin:0 auto; background:#CCC; border-radius:5px; width:600px; display:flex; flex-flow:row; }
        .box div { text-align:center; }
        .box .red { order:-2; flex:0 1 auto; width:200px; background:#F00; }
        .box .blue { order:-4; flex:0 1 auto; width:200px; background:#00F; }
        .box .black { order:-1; flex:0 1 auto; width:200px; background:#000; color:#FFF; }
    "#;
    let container = layout_html_flex(test_html, test_css, 800.0, 600.0);
    assert_eq!(container.dimensions.content.origin.x, 100.0);
    assert_eq!(container.dimensions.content.size.width, 600.0);
    assert_eq!(container.children[0].dimensions.content.origin.x, 100.0);
    assert_eq!(container.children[0].dimensions.content.size.width, 200.0);
    assert_eq!(container.children[1].dimensions.content.origin.x, 300.0);
    assert_eq!(container.children[1].dimensions.content.size.width, 200.0);
    assert_eq!(container.children[2].dimensions.content.origin.x, 500.0);
    assert_eq!(container.children[2].dimensions.content.size.width, 200.0);
}
