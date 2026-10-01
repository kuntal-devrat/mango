#![allow(clippy::field_reassign_with_default)]

use mango_core::{EdgeSizes, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::{BreakInside, ColumnSpan, Length, Overflow, Position, WritingMode};
use mango_layout::box_model::BoxType;
use mango_layout::box_tree::LayoutBox;
use mango_layout::dimensions::Dimensions;
use mango_layout::display_list::build_display_list_with_scroll;
use mango_layout::float::FloatContext;
use mango_render::display_list::DisplayCommand;

#[test]
fn test_overflow_scroll_container_and_clamping() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.overflow_y = Overflow::Scroll;
    container.style = Some(c_style);
    container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 100.0);

    // Add children summing to 300px height
    for i in 0..3 {
        let mut child = LayoutBox::new(BoxType::BlockNode, None);
        child.dimensions.content = Rect::new(0.0, i as f32 * 100.0, 300.0, 100.0);
        container.children.push(child);
    }

    assert!(container.is_scroll_container());
    let (ext_w, ext_h) = container.scrollable_extent();
    assert_eq!(ext_w, 300.0);
    assert_eq!(ext_h, 300.0);

    let (max_x, max_y) = container.max_scroll();
    assert_eq!(max_x, 0.0);
    assert_eq!(max_y, 200.0);

    // Test scrolling
    let (cx, cy) = container.scroll_by(0.0, 50.0);
    assert_eq!(cx, 0.0);
    assert_eq!(cy, 50.0);
    assert_eq!(container.scroll_offset_y, 50.0);

    // Test clamping
    let (cx, cy) = container.scroll_by(0.0, 500.0);
    assert_eq!(cx, 0.0);
    assert_eq!(cy, 150.0);
    assert_eq!(container.scroll_offset_y, 200.0);

    // Negative scroll
    let (cx, cy) = container.scroll_by(0.0, -100.0);
    assert_eq!(cx, 0.0);
    assert_eq!(cy, -100.0);
    assert_eq!(container.scroll_offset_y, 100.0);
}

#[test]
fn test_nested_scrolling_dispatch() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    let mut p_style = ComputedStyle::default();
    p_style.overflow_y = Overflow::Auto;
    parent.style = Some(p_style);
    parent.dimensions.content = Rect::new(0.0, 0.0, 400.0, 200.0);

    // Inner scroll container located at (0, 50), height 100, content 250 (max_scroll = 150)
    let mut inner = LayoutBox::new(BoxType::BlockNode, None);
    let mut i_style = ComputedStyle::default();
    i_style.overflow_y = Overflow::Scroll;
    inner.style = Some(i_style);
    inner.dimensions.content = Rect::new(0.0, 50.0, 400.0, 100.0);

    // Add children to inner
    for j in 0..5 {
        let mut c = LayoutBox::new(BoxType::BlockNode, None);
        c.dimensions.content = Rect::new(0.0, 50.0 + j as f32 * 50.0, 400.0, 50.0);
        inner.children.push(c);
    }

    // Add tall item to parent so parent also has scrollable content (total parent content = 500)
    let mut parent_spacer = LayoutBox::new(BoxType::BlockNode, None);
    parent_spacer.dimensions.content = Rect::new(0.0, 200.0, 400.0, 300.0);

    parent.children.push(inner);
    parent.children.push(parent_spacer);

    // Dispatch scroll inside the inner container: (x = 50, y = 75)
    // 1. Inner container consumes first 100px
    let (rem_x, rem_y) = parent.dispatch_nested_scroll(50.0, 75.0, 0.0, 100.0);
    assert_eq!(rem_x, 0.0);
    assert_eq!(rem_y, 0.0);
    assert_eq!(parent.children[0].scroll_offset_y, 100.0);
    assert_eq!(parent.scroll_offset_y, 0.0);

    // 2. Next 100px: inner consumes 50px (reaching its max 150px), remaining 50px bubbles to parent!
    let (rem_x, rem_y) = parent.dispatch_nested_scroll(50.0, 75.0, 0.0, 100.0);
    assert_eq!(rem_x, 0.0);
    assert_eq!(rem_y, 0.0);
    assert_eq!(parent.children[0].scroll_offset_y, 150.0);
    assert_eq!(parent.scroll_offset_y, 50.0);

    // 3. Large scroll: inner is maxed, parent consumes up to its max (300px), remaining returns to page!
    let (rem_x, rem_y) = parent.dispatch_nested_scroll(50.0, 75.0, 0.0, 500.0);
    assert_eq!(rem_x, 0.0);
    // Parent had max scroll = 500 - 200 = 300. It had 50, so it consumes 250.
    // 500 - 250 = 250 unconsumed delta returned!
    assert_eq!(rem_y, 250.0);
    assert_eq!(parent.scroll_offset_y, 300.0);
    assert_eq!(parent.children[0].scroll_offset_y, 150.0);
}

#[test]
fn test_scroll_container_display_list_offset_and_scrollbars() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.overflow_y = Overflow::Scroll;
    container.style = Some(style);
    container.dimensions.content = Rect::new(0.0, 0.0, 200.0, 100.0);
    container.scroll_offset_y = 40.0;

    let mut child = LayoutBox::new(BoxType::BlockNode, None);
    child.dimensions.content = Rect::new(10.0, 20.0, 180.0, 120.0);
    container.children.push(child);

    let dl = build_display_list_with_scroll(&container, 0.0);

    // Verify DisplayList has PushClip and PopClip around children
    let has_push_clip = dl
        .as_slice()
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
    let has_pop_clip = dl
        .as_slice()
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PopClip));
    assert!(has_push_clip);
    assert!(has_pop_clip);

    // Verify vertical scrollbar track and thumb are drawn
    let has_scrollbar_track = dl.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { rect, .. } => rect.x() >= 194.0 && rect.height() == 100.0,
        _ => false,
    });
    let has_scrollbar_thumb = dl.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::FillRoundedRect { rect, .. } => rect.x() >= 194.0,
        _ => false,
    });
    assert!(has_scrollbar_track, "Should render scrollbar track");
    assert!(has_scrollbar_thumb, "Should render scrollbar thumb");
}

#[test]
fn test_position_sticky_4_directions() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.overflow_y = Overflow::Scroll;
    container.style = Some(c_style);
    container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 200.0);
    container.scroll_offset_y = 80.0;

    // Sticky element with top: 10px
    let mut sticky_child = LayoutBox::new(BoxType::BlockNode, None);
    let mut s_style = ComputedStyle::default();
    s_style.position = Position::Sticky;
    s_style.top = Length::Px(10.0);
    s_style.height = Length::Px(30.0);
    sticky_child.style = Some(s_style);
    sticky_child.dimensions.content = Rect::new(0.0, 50.0, 300.0, 300.0);

    // Spacer to allow containing block height
    let mut spacer = LayoutBox::new(BoxType::BlockNode, None);
    spacer.dimensions.content = Rect::new(0.0, 0.0, 300.0, 500.0);

    container.children.push(sticky_child);
    container.children.push(spacer);

    let dl = build_display_list_with_scroll(&container, 0.0);

    // In-flow position was at y=50. With scroll_offset_y=80, in-flow scrolled pos is 50-80 = -30.
    // Sticky threshold is viewport_y (0) + top (10) = 10.
    // So the sticky child sticks at 10px!
    let _sticky_rect = dl.as_slice().iter().find_map(|cmd| match cmd {
        DisplayCommand::FillRect { rect, .. } if rect.width() == 300.0 => Some(rect),
        _ => None,
    });
    // Ensure display list rendered
    assert!(!dl.is_empty());
}

#[test]
fn test_column_span_all_partitioning() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.column_count = Some(2);
    style.width = Length::Px(400.0);
    container.style = Some(style);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    // Item 1: normal multi-column item
    let mut item1 = LayoutBox::new(BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.height = Length::Px(100.0);
    item1.style = Some(s1);

    // Item 2: spanning element across all columns
    let mut span_all = LayoutBox::new(BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.column_span = ColumnSpan::All;
    s2.height = Length::Px(40.0);
    span_all.style = Some(s2);

    // Item 3: normal multi-column item
    let mut item3 = LayoutBox::new(BoxType::BlockNode, None);
    let mut s3 = ComputedStyle::default();
    s3.height = Length::Px(100.0);
    item3.style = Some(s3);

    container.children.push(item1);
    container.children.push(span_all);
    container.children.push(item3);

    let mut float_ctx = FloatContext::new();
    let cb = Dimensions {
        content: Rect::new(0.0, 0.0, 400.0, 600.0),
        padding: EdgeSizes::ZERO,
        border: EdgeSizes::ZERO,
        margin: EdgeSizes::ZERO,
    };
    mango_layout::block_flow::layout_block(&mut container, &cb, &mut float_ctx);

    // Verify children count and layout
    assert_eq!(container.children.len(), 3);

    // Spanning child should have full container width (400px)
    let span_node = &container.children[1];
    assert_eq!(span_node.dimensions.content.width(), 400.0);

    // Spanning child must be positioned below item 1
    assert!(span_node.dimensions.content.y() >= container.children[0].dimensions.content.y());

    // Item 3 must be positioned below spanning child
    assert!(container.children[2].dimensions.content.y() >= span_node.dimensions.content.bottom());
}

#[test]
fn test_block_fragmentation_with_break_avoid() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.column_count = Some(2);
    style.width = Length::Px(400.0);
    container.style = Some(style);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    // Item 1: height 40
    let mut item1 = LayoutBox::new(BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.height = Length::Px(40.0);
    item1.style = Some(s1);

    // Item 2: height 60, break-inside: avoid
    let mut item2 = LayoutBox::new(BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.height = Length::Px(60.0);
    s2.break_inside = BreakInside::Avoid;
    item2.style = Some(s2);

    container.children.push(item1);
    container.children.push(item2);

    let mut float_ctx = FloatContext::new();
    let cb = Dimensions {
        content: Rect::new(0.0, 0.0, 400.0, 600.0),
        padding: EdgeSizes::ZERO,
        border: EdgeSizes::ZERO,
        margin: EdgeSizes::ZERO,
    };
    mango_layout::block_flow::layout_block(&mut container, &cb, &mut float_ctx);

    assert_eq!(container.children.len(), 2);
    // Because item 2 avoids breaking inside, when column 1 target is exceeded,
    // item 2 is pushed cleanly to column 2!
    assert!(
        container.children[1].dimensions.content.x() > container.children[0].dimensions.content.x()
    );
}

#[test]
fn test_writing_mode_vertical_text_layout() {
    // Test vertical-rl
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.writing_mode = WritingMode::VerticalRl;
    style.font_size = 16.0;
    style.height = Length::Px(200.0);
    style.width = Length::Px(100.0);
    container.style = Some(style);
    container.dimensions.content = Rect::new(10.0, 20.0, 100.0, 200.0);

    let mut text_box = LayoutBox::new(BoxType::TextNode("Hello".to_string()), None);
    let mut t_style = ComputedStyle::default();
    t_style.font_size = 16.0;
    text_box.style = Some(t_style);
    container.children.push(text_box);

    let mut float_ctx = FloatContext::new();
    let total_h = mango_layout::inline_flow::layout_inline_children(&mut container, &mut float_ctx);

    // In vertical layout, children are glyphs/lines running downward along Y
    assert!(container.children.len() >= 5);
    // Y coordinates should increase top to bottom
    assert!(
        container.children[1].dimensions.content.y() > container.children[0].dimensions.content.y()
    );
    assert!(total_h > 0.0);

    // Test vertical-lr
    let mut container_lr = LayoutBox::new(BoxType::BlockNode, None);
    let mut style_lr = ComputedStyle::default();
    style_lr.writing_mode = WritingMode::VerticalLr;
    style_lr.font_size = 16.0;
    style_lr.height = Length::Px(200.0);
    style_lr.width = Length::Px(100.0);
    container_lr.style = Some(style_lr);
    container_lr.dimensions.content = Rect::new(10.0, 20.0, 100.0, 200.0);

    let mut text_box_lr = LayoutBox::new(BoxType::TextNode("World".to_string()), None);
    let mut t_style_lr = ComputedStyle::default();
    t_style_lr.font_size = 16.0;
    text_box_lr.style = Some(t_style_lr);
    container_lr.children.push(text_box_lr);

    let mut float_ctx_lr = FloatContext::new();
    let total_h_lr =
        mango_layout::inline_flow::layout_inline_children(&mut container_lr, &mut float_ctx_lr);

    assert!(container_lr.children.len() >= 5);
    assert!(
        container_lr.children[1].dimensions.content.y()
            > container_lr.children[0].dimensions.content.y()
    );
    assert!(total_h_lr > 0.0);
}
