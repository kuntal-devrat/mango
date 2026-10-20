use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::Length;

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::super::container::layout_block;

#[test]
fn test_block_width_calculation_auto() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.margin_left = Length::Px(10.0);
    style.margin_right = Length::Px(20.0);
    style.padding_left = Length::Px(5.0);
    style.padding_right = Length::Px(15.0);
    child.style = Some(style.into());
    root.children.push(child);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let child_box = &root.children[0];
    // 500 - (10 + 20 + 5 + 15) = 450
    assert_eq!(child_box.dimensions.content.width(), 450.0);
}

#[test]
fn test_block_margin_auto_centering() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 600.0, 500.0);

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.width = Length::Px(200.0);
    style.margin_left = Length::Auto;
    style.margin_right = Length::Auto;
    child.style = Some(style.into());
    root.children.push(child);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let child_box = &root.children[0];
    // (600 - 200) / 2 = 200 margin each
    assert_eq!(child_box.dimensions.margin.left, 200.0);
    assert_eq!(child_box.dimensions.margin.right, 200.0);
    assert_eq!(child_box.dimensions.content.x(), 200.0);
}

#[test]
fn test_block_max_width_margin_auto_centering() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 1000.0, 500.0);

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    // Modern web container pattern: width auto (or 100%), max-width 600px, margin: 0 auto
    style.width = Length::Auto;
    style.max_width = Length::Px(600.0);
    style.margin_left = Length::Auto;
    style.margin_right = Length::Auto;
    child.style = Some(style.into());
    root.children.push(child);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let child_box = &root.children[0];
    assert_eq!(child_box.dimensions.content.width(), 600.0);
    // (1000 - 600) / 2 = 200 margin each
    assert_eq!(child_box.dimensions.margin.left, 200.0);
    assert_eq!(child_box.dimensions.margin.right, 200.0);
    assert_eq!(child_box.dimensions.content.x(), 200.0);
}

#[test]
fn test_block_min_max_height_clamping() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    // Child 1: content height is 0, but min-height is 150px
    let mut child1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.min_height = Length::Px(150.0);
    child1.style = Some(s1.into());

    // Child 2: explicit height is 500px, but max-height is 200px
    let mut child2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.height = Length::Px(500.0);
    s2.max_height = Length::Px(200.0);
    child2.style = Some(s2.into());

    root.children.push(child1);
    root.children.push(child2);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    assert_eq!(root.children[0].dimensions.content.height(), 150.0);
    assert_eq!(root.children[1].dimensions.content.height(), 200.0);
}

#[test]
fn test_margin_collapsing() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut b1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.height = Length::Px(50.0);
    s1.margin_bottom = Length::Px(30.0);
    b1.style = Some(s1.into());

    let mut b2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.height = Length::Px(50.0);
    s2.margin_top = Length::Px(20.0); // 30.max(20) = 30 collapsed gap
    b2.style = Some(s2.into());

    root.children.push(b1);
    root.children.push(b2);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let box1 = &root.children[0];
    let box2 = &root.children[1];

    // Box 1 bottom border box is at 0 + 50 = 50.
    // Box 2 top border box is at 50 + 30 = 80!
    assert_eq!(box1.dimensions.border_box().bottom(), 50.0);
    assert_eq!(box2.dimensions.border_box().y(), 80.0);
}

#[test]
fn test_negative_and_mixed_margin_collapsing() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    // Child 1: height 50, margin-bottom: -20
    let mut b1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.height = Length::Px(50.0);
    s1.margin_bottom = Length::Px(-20.0);
    b1.style = Some(s1.into());

    // Child 2: height 50, margin-top: 30
    // Mixed: 30 + (-20) = 10 collapsed margin
    let mut b2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.height = Length::Px(50.0);
    s2.margin_top = Length::Px(30.0);
    s2.margin_bottom = Length::Px(-15.0);
    b2.style = Some(s2.into());

    // Child 3: height 50, margin-top: -25
    // Both negative: min(-15, -25) = -25 collapsed margin
    let mut b3 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s3 = ComputedStyle::default();
    s3.height = Length::Px(50.0);
    s3.margin_top = Length::Px(-25.0);
    b3.style = Some(s3.into());

    root.children.push(b1);
    root.children.push(b2);
    root.children.push(b3);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let box1 = &root.children[0];
    let box2 = &root.children[1];
    let box3 = &root.children[2];

    // Box 1: top = 0, bottom = 50
    assert_eq!(box1.dimensions.border_box().bottom(), 50.0);
    // Box 2: y = 50 + 10 = 60, bottom = 60 + 50 = 110
    assert_eq!(box2.dimensions.border_box().y(), 60.0);
    assert_eq!(box2.dimensions.border_box().bottom(), 110.0);
    // Box 3: y = 110 + (-25) = 85
    assert_eq!(box3.dimensions.border_box().y(), 85.0);
}

#[test]
fn test_parent_child_margin_collapsing() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    // Parent has no border and no padding
    let mut parent = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut p_style = ComputedStyle::default();
    p_style.height = Length::Auto;
    p_style.margin_top = Length::Px(10.0);
    p_style.margin_bottom = Length::Px(10.0);
    parent.style = Some(p_style.into());

    // Child has height 100, margin-top: 40, margin-bottom: 50
    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.height = Length::Px(100.0);
    c_style.margin_top = Length::Px(40.0);
    c_style.margin_bottom = Length::Px(50.0);
    child.style = Some(c_style.into());

    parent.children.push(child);
    root.children.push(parent);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let parent_box = &root.children[0];
    // Parent's top margin collapsed with child's: max(10, 40) = 40
    assert_eq!(parent_box.dimensions.margin.top, 40.0);
    // Parent's content height should be 100.0 (child's border box, not stretched by child's margin)
    assert_eq!(parent_box.dimensions.content.height(), 100.0);
    // Parent's bottom margin collapsed with child's: max(10, 50) = 50
    assert_eq!(parent_box.dimensions.margin.bottom, 50.0);
}

#[test]
fn test_float_right_contour_text_wrapping() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 600.0);

    // Floated right box (e.g. Wikipedia infobox / thumbnail)
    let mut float_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut float_style = ComputedStyle::default();
    float_style.float = mango_css::values::Float::Right;
    float_style.width = Length::Px(200.0);
    float_style.height = Length::Px(80.0);
    float_box.style = Some(float_style.into());

    // Paragraph containing text
    let mut p_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let p_style = ComputedStyle::default();
    p_box.style = Some(p_style.into());

    let text_child = LayoutBox::new(
            crate::box_model::BoxType::TextNode(
                "Wikipedia is a free online encyclopedia written and maintained by a community of volunteers."
                    .to_string(),
            ),
            Some(ComputedStyle::default().into()),
        );
    p_box.children.push(text_child);

    root.children.push(float_box);
    root.children.push(p_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let floated = &root.children[0];
    let p = &root.children[1];
    assert_eq!(floated.dimensions.content.x(), 300.0);
    assert_eq!(floated.dimensions.content.y(), 0.0);
    assert_eq!(floated.dimensions.content.width(), 200.0);

    // The paragraph starts at y = 0 (beside the float, not below it!)
    assert_eq!(p.dimensions.content.y(), 0.0);

    // None of the text lines inside the paragraph exceed the left edge of the float (300.0)
    for line_box in &p.children {
        if line_box.dimensions.content.y() < 80.0 {
            assert!(
                line_box.dimensions.content.right() <= 300.5,
                "Line box at y={} right={} exceeded float left boundary 300.0",
                line_box.dimensions.content.y(),
                line_box.dimensions.content.right()
            );
        }
    }
}

#[test]
fn test_multicolumn_layout_balances_columns() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 600.0);

    let mut multi_style = ComputedStyle::default();
    multi_style.column_count = Some(2);
    multi_style.column_gap = Length::Px(20.0);
    root.style = Some(multi_style.into());

    // 4 child items of height 40px each
    for i in 0..4 {
        let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut child_style = ComputedStyle::default();
        child_style.height = Length::Px(40.0);
        child.style = Some(child_style.into());
        child.tag_name = Some(format!("item-{}", i));
        root.children.push(child);
    }

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    assert_eq!(root.children.len(), 4);
    // Column 0 width = (500 - 20) / 2 = 240
    // Children 0 and 1 are in col 0 (x = 0)
    assert_eq!(root.children[0].dimensions.content.x(), 0.0);
    assert_eq!(root.children[0].dimensions.content.width(), 240.0);
    assert_eq!(root.children[1].dimensions.content.x(), 0.0);

    // Children 2 and 3 are in col 1 (x = 240 + 20 = 260)
    assert_eq!(root.children[2].dimensions.content.x(), 260.0);
    assert_eq!(root.children[2].dimensions.content.width(), 240.0);
    assert_eq!(root.children[3].dimensions.content.x(), 260.0);
}

#[test]
fn test_replaced_element_aspect_ratio_auto_height_and_width() {
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 800.0, 600.0);

    // Case 1: Image has intrinsic 400x200 (aspect ratio 2:1). CSS sets width: 200px, height: auto.
    // Height must resolve to 100px.
    let mut img_box = LayoutBox::new(
        crate::box_model::BoxType::ReplacedElement {
            intrinsic_width: 400.0,
            intrinsic_height: 200.0,
            pixels: vec![],
        },
        None,
    );
    let mut style = ComputedStyle::default();
    style.width = Length::Px(200.0);
    style.height = Length::Auto;
    img_box.style = Some(style.into());

    let mut float_ctx = FloatContext::new();
    layout_block(&mut img_box, &containing_block, &mut float_ctx);

    assert_eq!(img_box.dimensions.content.width(), 200.0);
    assert_eq!(
        img_box.dimensions.content.height(),
        100.0,
        "Auto height should preserve 2:1 aspect ratio"
    );

    // Case 2: Image has intrinsic 400x200 (aspect ratio 2:1). CSS sets width: auto, height: 50px.
    // Width must resolve to 100px.
    let mut img_box2 = LayoutBox::new(
        crate::box_model::BoxType::ReplacedElement {
            intrinsic_width: 400.0,
            intrinsic_height: 200.0,
            pixels: vec![],
        },
        None,
    );
    let mut style2 = ComputedStyle::default();
    style2.width = Length::Auto;
    style2.height = Length::Px(50.0);
    img_box2.style = Some(style2.into());

    let mut float_ctx2 = FloatContext::new();
    layout_block(&mut img_box2, &containing_block, &mut float_ctx2);

    assert_eq!(
        img_box2.dimensions.content.width(),
        100.0,
        "Auto width should preserve 2:1 aspect ratio"
    );
    assert_eq!(img_box2.dimensions.content.height(), 50.0);

    // Case 3: Image has intrinsic 400x200 (2:1) but CSS aspect-ratio is 16/9.
    // width: 320px, height: auto -> height should be 320 / (16/9) = 180px.
    let mut img_box3 = LayoutBox::new(
        crate::box_model::BoxType::ReplacedElement {
            intrinsic_width: 400.0,
            intrinsic_height: 200.0,
            pixels: vec![],
        },
        None,
    );
    let mut style3 = ComputedStyle::default();
    style3.width = Length::Px(320.0);
    style3.height = Length::Auto;
    style3.aspect_ratio = Some(16.0 / 9.0);
    img_box3.style = Some(style3.into());

    let mut float_ctx3 = FloatContext::new();
    layout_block(&mut img_box3, &containing_block, &mut float_ctx3);

    assert_eq!(img_box3.dimensions.content.width(), 320.0);
    assert!(
        (img_box3.dimensions.content.height() - 180.0).abs() < 1.0,
        "CSS aspect-ratio should override intrinsic aspect ratio"
    );

    // Case 4: Image has intrinsic 400x200 (2:1) but CSS aspect-ratio is 16/9.
    // width: auto, height: 180px -> width should be 180 * (16/9) = 320px.
    let mut img_box4 = LayoutBox::new(
        crate::box_model::BoxType::ReplacedElement {
            intrinsic_width: 400.0,
            intrinsic_height: 200.0,
            pixels: vec![],
        },
        None,
    );
    let mut style4 = ComputedStyle::default();
    style4.width = Length::Auto;
    style4.height = Length::Px(180.0);
    style4.aspect_ratio = Some(16.0 / 9.0);
    img_box4.style = Some(style4.into());

    let mut float_ctx4 = FloatContext::new();
    layout_block(&mut img_box4, &containing_block, &mut float_ctx4);

    assert!(
        (img_box4.dimensions.content.width() - 320.0).abs() < 1.0,
        "CSS aspect-ratio should override intrinsic aspect ratio for auto width"
    );
    assert_eq!(img_box4.dimensions.content.height(), 180.0);
}

#[test]
fn test_main_page_shrink_to_fit() {
    let html = r#"<style>
.vector-menu-tabs .vector-menu-content-list { display: flex; }
.vector-menu-tabs li { display: block; float: left; margin: 0; padding: 0; }
.vector-menu-tabs li a { display: inline-flex; align-items: center; }
</style>
<nav class="vector-menu vector-menu-tabs">
<div class="vector-menu-content">
<ul class="vector-menu-content-list">
<li id="ca-nstab-main" class="selected vector-tab-noicon mw-list-item"><a href="/wiki/Main_Page"><span>Main Page</span></a></li>
</ul>
</div>
</nav>"#;
    let doc = mango_html::parse_html(html);
    let st = crate::style_tree::build_style_tree(&doc, &[]).unwrap();
    let mut bt = crate::box_tree::build_box_tree(&st);
    let cb = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 0.0));
    let mut float_ctx = FloatContext::new();
    layout_block(&mut bt, &cb, &mut float_ctx);
    fn print_tree(b: &LayoutBox, depth: usize) {
        println!(
            "{}Box {:?}: tag={:?} w={} h={}",
            " ".repeat(depth),
            b.box_type,
            b.tag_name,
            b.dimensions.content.width(),
            b.dimensions.content.height()
        );
        for c in &b.children {
            print_tree(c, depth + 2);
        }
    }
    print_tree(&bt, 0);
}
