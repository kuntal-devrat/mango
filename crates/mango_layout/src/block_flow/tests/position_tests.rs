use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::{BoxSizing, Direction, Display, Float, Length, Overflow, Position};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::super::container::layout_block;
use super::super::positioned::{clear_anchors, layout_positioned_element, register_anchor};

#[test]
fn test_positioned_element_auto_width_shrink_to_fit() {
    // B1: width: auto on absolute element must shrink to fit, not expand to container width
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 600.0, 400.0);

    let mut abs_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut abs_style = ComputedStyle::default();
    abs_style.position = Position::Absolute;
    abs_style.display = Display::Block;
    abs_style.width = Length::Auto;
    abs_style.height = Length::Px(50.0);
    abs_box.style = Some(abs_style.into());

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Block;
    c_style.width = Length::Px(120.0);
    c_style.height = Length::Px(30.0);
    child.style = Some(c_style.into());
    abs_box.children.push(child);

    root.children.push(abs_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let abs_result = &root.children[0];
    assert_eq!(abs_result.dimensions.content.width(), 120.0);
}

#[test]
fn test_positioned_element_percentage_margins() {
    // B2: Percentage margins on positioned elements are relative to containing block WIDTH
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    // Width 500, Height 200
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 200.0);

    let mut abs_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut abs_style = ComputedStyle::default();
    abs_style.position = Position::Absolute;
    abs_style.display = Display::Block;
    abs_style.width = Length::Px(100.0);
    abs_style.height = Length::Px(100.0);
    // 10% of width 500 = 50px
    abs_style.margin_top = Length::Percent(10.0);
    abs_style.margin_bottom = Length::Percent(10.0);
    abs_style.margin_left = Length::Percent(10.0);
    abs_style.margin_right = Length::Percent(10.0);
    abs_box.style = Some(abs_style.into());

    root.children.push(abs_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let abs_result = &root.children[0];
    assert_eq!(abs_result.dimensions.margin.top, 50.0);
    assert_eq!(abs_result.dimensions.margin.bottom, 50.0);
    assert_eq!(abs_result.dimensions.margin.left, 50.0);
    assert_eq!(abs_result.dimensions.margin.right, 50.0);
}

#[test]
fn test_positioned_element_static_position_after_block_sibling() {
    // B3: Static position for auto top on absolute element takes sibling order into account
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut sibling = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s_style = ComputedStyle::default();
    s_style.display = Display::Block;
    s_style.width = Length::Px(500.0);
    s_style.height = Length::Px(150.0);
    sibling.style = Some(s_style.into());

    let mut abs_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut abs_style = ComputedStyle::default();
    abs_style.position = Position::Absolute;
    abs_style.display = Display::Block;
    abs_style.top = Length::Auto;
    abs_style.left = Length::Auto;
    abs_style.width = Length::Px(100.0);
    abs_style.height = Length::Px(80.0);
    abs_box.style = Some(abs_style.into());

    root.children.push(sibling);
    root.children.push(abs_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let abs_result = &root.children[1];
    assert_eq!(abs_result.dimensions.content.y(), 150.0);
}

#[test]
fn test_float_downward_stepping_when_overflowing() {
    // B4: Floats step downward past earlier floats if horizontal span is insufficient
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 600.0);

    let mut float1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut f1_style = ComputedStyle::default();
    f1_style.float = Float::Right;
    f1_style.width = Length::Px(300.0);
    f1_style.height = Length::Px(100.0);
    float1.style = Some(f1_style.into());

    let mut float2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut f2_style = ComputedStyle::default();
    f2_style.float = Float::Right;
    f2_style.width = Length::Px(300.0);
    f2_style.height = Length::Px(100.0);
    float2.style = Some(f2_style.into());

    root.children.push(float1);
    root.children.push(float2);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let f1_res = &root.children[0];
    let f2_res = &root.children[1];
    assert_eq!(f1_res.dimensions.content.y(), 0.0);
    // Float 2 cannot fit alongside float 1 (300 + 300 > 500), so it steps below float 1 (y = 100)
    assert_eq!(f2_res.dimensions.content.y(), 100.0);
}

#[test]
fn test_tentative_height_border_box() {
    // B5: Tentative height resolution deducts padding & borders for box-sizing: border-box
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut parent = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut p_style = ComputedStyle::default();
    p_style.display = Display::Block;
    p_style.box_sizing = BoxSizing::BorderBox;
    p_style.height = Length::Px(200.0);
    p_style.padding_top = Length::Px(20.0);
    p_style.padding_bottom = Length::Px(20.0);
    p_style.border_top_width = 10.0;
    p_style.border_bottom_width = 10.0;
    parent.style = Some(p_style.into());

    root.children.push(parent);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let p_res = &root.children[0];
    // Content height: 200 - (20 + 20 + 10 + 10) = 140
    assert_eq!(p_res.dimensions.content.height(), 140.0);
    assert_eq!(p_res.dimensions.border_box().height(), 200.0);
}

#[test]
fn test_relative_offset_percentage_height_in_auto_container() {
    // B6: Relative top: 50% in an auto-height container resolves to 0px, not magic 600px fallback
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 0.0); // auto / indefinite height

    let mut rel_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut r_style = ComputedStyle::default();
    r_style.display = Display::Block;
    r_style.position = Position::Relative;
    r_style.top = Length::Percent(50.0);
    r_style.width = Length::Px(200.0);
    r_style.height = Length::Px(100.0);
    rel_box.style = Some(r_style.into());

    root.children.push(rel_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let rel_res = &root.children[0];
    assert_eq!(rel_res.dimensions.content.y(), 0.0);
}

#[test]
fn test_bfc_root_margin_containment() {
    // B7: BFC roots (overflow: hidden) do not collapse margins with in-flow children
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut bfc_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut bfc_style = ComputedStyle::default();
    bfc_style.display = Display::Block;
    bfc_style.overflow_x = Overflow::Hidden;
    bfc_style.overflow_y = Overflow::Hidden;
    bfc_style.height = Length::Auto;
    bfc_style.margin_top = Length::Px(10.0);
    bfc_box.style = Some(bfc_style.into());

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Block;
    c_style.height = Length::Px(80.0);
    c_style.margin_top = Length::Px(30.0);
    c_style.margin_bottom = Length::Px(20.0);
    child.style = Some(c_style.into());

    bfc_box.children.push(child);
    root.children.push(bfc_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let bfc_res = &root.children[0];
    let child_res = &bfc_res.children[0];
    // Parent's margin-top stays 10 (not collapsed to 30)
    assert_eq!(bfc_res.dimensions.margin.top, 10.0);
    // Child's margin-top pushes it down from parent's content top
    assert_eq!(
        child_res.dimensions.content.y(),
        bfc_res.dimensions.content.y() + 30.0
    );
    // Parent's height encloses child's margin box: 80 + 30 + 20 = 130
    assert_eq!(bfc_res.dimensions.content.height(), 130.0);
}

#[test]
fn test_bfc_root_auto_height_encloses_floats() {
    // B8: BFC roots auto height encloses floating descendants per CSS 2.1 §10.6.7
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut bfc_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut bfc_style = ComputedStyle::default();
    bfc_style.display = Display::Block;
    bfc_style.overflow_x = Overflow::Hidden;
    bfc_style.overflow_y = Overflow::Hidden;
    bfc_style.height = Length::Auto;
    bfc_box.style = Some(bfc_style.into());

    let mut float_child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut f_style = ComputedStyle::default();
    f_style.float = Float::Left;
    f_style.width = Length::Px(150.0);
    f_style.height = Length::Px(120.0);
    float_child.style = Some(f_style.into());

    bfc_box.children.push(float_child);
    root.children.push(bfc_box);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let bfc_res = &root.children[0];
    assert_eq!(bfc_res.dimensions.content.height(), 120.0);
}

#[test]
fn test_rtl_overconstrained_underflow_absorbs_left_margin() {
    // G1: Over-constrained blocks in direction: rtl absorb underflow into margin-left
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut block = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.display = Display::Block;
    style.direction = Direction::Rtl;
    style.width = Length::Px(200.0);
    style.margin_left = Length::Px(10.0);
    style.margin_right = Length::Px(10.0);
    block.style = Some(style.into());

    root.children.push(block);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let res = &root.children[0];
    // 500 - 200 - 10 - 10 = 280 underflow added to margin-left: 10 + 280 = 290
    assert_eq!(res.dimensions.margin.left, 290.0);
    assert_eq!(res.dimensions.margin.right, 10.0);
}

#[test]
fn test_bfc_child_float_avoidance() {
    // G2: In-flow BFC child avoids active floats rather than overlapping
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    let mut float_left = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut f_style = ComputedStyle::default();
    f_style.float = Float::Left;
    f_style.width = Length::Px(150.0);
    f_style.height = Length::Px(100.0);
    float_left.style = Some(f_style.into());

    let mut bfc_child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut bfc_style = ComputedStyle::default();
    bfc_style.display = Display::Block;
    bfc_style.overflow_x = Overflow::Hidden;
    bfc_style.overflow_y = Overflow::Hidden;
    bfc_style.width = Length::Px(200.0);
    bfc_style.height = Length::Px(80.0);
    bfc_child.style = Some(bfc_style.into());

    root.children.push(float_left);
    root.children.push(bfc_child);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    let bfc_res = &root.children[1];
    // bfc_child starts to the right of float_left (x >= 150)
    assert!(bfc_res.dimensions.content.x() >= 150.0);
    assert_eq!(bfc_res.dimensions.content.y(), 0.0);
}

#[test]
fn test_layout_with_css_clamp_width() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 800.0, 600.0);

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.width = Length::parse("clamp(200px, 50%, 600px)").expect("clamp should parse");
    child.style = Some(style.into());
    root.children.push(child);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    // 50% of 800 = 400px (clamped between 200px and 600px)
    assert_eq!(root.children[0].dimensions.content.width(), 400.0);

    // Now test containing block width = 1500px -> 50% = 750px -> clamped to 600px
    containing_block.content = Rect::new(0.0, 0.0, 1500.0, 600.0);
    let mut root_large = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut child_large = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style_large = ComputedStyle::default();
    style_large.width = Length::parse("clamp(200px, 50%, 600px)").unwrap();
    child_large.style = Some(style_large.into());
    root_large.children.push(child_large);

    let mut float_ctx2 = FloatContext::new();
    layout_block(&mut root_large, &containing_block, &mut float_ctx2);
    assert_eq!(root_large.children[0].dimensions.content.width(), 600.0);

    // Now test containing block width = 300px -> 50% = 150px -> clamped to 200px
    containing_block.content = Rect::new(0.0, 0.0, 300.0, 600.0);
    let mut root_small = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut child_small = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style_small = ComputedStyle::default();
    style_small.width = Length::parse("clamp(200px, 50%, 600px)").unwrap();
    child_small.style = Some(style_small.into());
    root_small.children.push(child_small);

    let mut float_ctx3 = FloatContext::new();
    layout_block(&mut root_small, &containing_block, &mut float_ctx3);
    assert_eq!(root_small.children[0].dimensions.content.width(), 200.0);
}

#[test]
fn test_layout_with_css_min_width() {
    let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 1000.0, 600.0);

    let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    // min(50%, 300px) -> for 1000px, min(500, 300) = 300px
    style.width = Length::parse("min(50%, 300px)").expect("min should parse");
    child.style = Some(style.into());
    root.children.push(child);

    let mut float_ctx = FloatContext::new();
    layout_block(&mut root, &containing_block, &mut float_ctx);

    assert_eq!(root.children[0].dimensions.content.width(), 300.0);
}

#[test]
fn test_anchor_positioning_and_fallbacks() {
    clear_anchors();

    // 1. Setup anchor element at (100, 200) with size (120, 40)
    let mut anchor_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    anchor_box.dimensions.content = Rect::new(100.0, 200.0, 120.0, 40.0);
    let mut anchor_style = ComputedStyle::default();
    anchor_style.anchor_name = Some("--my-button".to_string());
    anchor_box.style = Some(anchor_style.into());

    register_anchor("--my-button", anchor_box.dimensions.border_box());

    // 2. Setup positioned tooltip anchored below the button: top: anchor(bottom); left: anchor(left);
    let mut tooltip_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    tooltip_box.dimensions.content = Rect::new(0.0, 0.0, 80.0, 30.0);
    let mut tooltip_style = ComputedStyle::default();
    tooltip_style.position = Position::Absolute;
    tooltip_style.position_anchor = Some("--my-button".to_string());
    tooltip_style.top = Length::parse("anchor(bottom)").expect("parse anchor(bottom)");
    tooltip_style.left = Length::parse("anchor(left)").expect("parse anchor(left)");
    tooltip_box.style = Some(tooltip_style.into());

    let containing_block = Dimensions::new(Rect::new(0.0, 0.0, 800.0, 600.0));
    let mut float_ctx = FloatContext::new();
    layout_positioned_element(&mut tooltip_box, &containing_block, &mut float_ctx);

    // Tooltip top should align with anchor bottom (200 + 40 = 240)
    assert_eq!(tooltip_box.dimensions.content.y(), 240.0);
    // Tooltip left should align with anchor left (100)
    assert_eq!(tooltip_box.dimensions.content.x(), 100.0);

    // 3. Test @position-try fallback flip-block when anchor is near the bottom of viewport
    let mut bottom_anchor = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    bottom_anchor.dimensions.content = Rect::new(100.0, 560.0, 120.0, 30.0);
    register_anchor("--bottom-btn", bottom_anchor.dimensions.border_box());

    let mut flipping_tooltip = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    flipping_tooltip.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);
    let mut flip_style = ComputedStyle::default();
    flip_style.position = Position::Absolute;
    flip_style.position_anchor = Some("--bottom-btn".to_string());
    flip_style.top = Length::parse("anchor(bottom)").unwrap();
    // If bottom overflow occurs, flip-block should place it above the anchor (at 560 - 50 = 510)
    flip_style.position_try_fallbacks = vec!["flip-block".to_string()];
    flipping_tooltip.style = Some(flip_style.into());

    layout_positioned_element(&mut flipping_tooltip, &containing_block, &mut float_ctx);

    // Because bottom is at 590 and height is 50, placing below would end at 590 + 50 = 640 > 600.
    // It must flip above the anchor: top = 560 - 50 = 510!
    assert_eq!(flipping_tooltip.dimensions.content.y(), 510.0);
}

#[test]
fn test_positioned_element_top_bottom_stretches_auto_height_input() {
    // Absolute input element with top: 0, bottom: 0, height: auto must stretch to fill
    // the containing block padding box span without being clobbered by default input height (24px).
    let mut btn_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    btn_box.tag_name = Some("input".to_string());
    let mut btn_style = ComputedStyle::default();
    btn_style.position = Position::Absolute;
    btn_style.display = Display::Block;
    btn_style.top = Length::Px(0.0);
    btn_style.bottom = Length::Px(0.0);
    btn_style.height = Length::Auto;
    btn_style.margin_top = Length::Px(-1.0);
    btn_style.margin_bottom = Length::Px(-1.0);
    btn_style.min_height = Length::Px(32.4);
    btn_box.style = Some(btn_style.into());

    // Containing block height is 38.3 (e.g. form with 40.3 border-box, 1px borders)
    let containing_block = Dimensions::new(Rect::new(100.0, 423.0, 500.0, 38.3));
    let mut float_ctx = FloatContext::new();
    layout_positioned_element(&mut btn_box, &containing_block, &mut float_ctx);

    // Height should be 38.3 - 0 - 0 - (-1) - (-1) = 40.3px
    assert!((btn_box.dimensions.content.height() - 40.3).abs() < 0.01);
    // Border box y should be 423.0 - 1.0 = 422.0
    assert!((btn_box.dimensions.border_box().y() - 422.0).abs() < 0.01);
    // Bottom should align with 422.0 + 40.3 = 462.3
    assert!((btn_box.dimensions.border_box().bottom() - 462.3).abs() < 0.01);
}
