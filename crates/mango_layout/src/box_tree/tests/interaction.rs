use mango_core::{Point, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::{Display, Length, Position};
use mango_html::dom::NodeId;

use crate::box_model::BoxType;
use crate::style_tree::StyledNode;

use super::super::builder::build_box_tree;
use super::super::replaced::serialize_svg_styled_node;
use super::super::types::{AreaShape, LayoutBox};

#[test]
fn test_scroll_container_hit_testing_with_offsets() {
    let mut container_style = ComputedStyle::default();
    container_style.display = Display::Block;
    container_style.overflow_y = mango_css::values::Overflow::Scroll;
    let mut container = LayoutBox::new(BoxType::BlockNode, Some(container_style.into()));
    container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 200.0);
    container.dimensions.padding.top = 0.0;
    container.dimensions.padding.bottom = 0.0;
    container.dimensions.padding.left = 0.0;
    container.dimensions.padding.right = 0.0;

    let mut child_style = ComputedStyle::default();
    child_style.display = Display::Block;
    let mut child = LayoutBox::new(BoxType::BlockNode, Some(child_style.into()));
    child.node_id = Some(NodeId::from_raw(42));
    // Child is at y = 100..150
    child.dimensions.content = Rect::new(10.0, 100.0, 200.0, 50.0);
    child.link_target = Some("https://example.com/item".to_string());
    container.children.push(child);

    // Without scroll: clicking at (50, 120) hits child
    assert_eq!(
        container.hit_test(Point::new(50.0, 120.0)),
        Some(NodeId::from_raw(42))
    );
    assert_eq!(
        container.hit_test_link(Point::new(50.0, 120.0)),
        Some("https://example.com/item")
    );

    // Now scroll down by 50px: child visually appears at y = 50..100
    container.scroll_offset_y = 50.0;

    // Screen click at (50, 70) should hit the scrolled child (70 + 50 = 120 in child coordinates)
    assert_eq!(
        container.hit_test(Point::new(50.0, 70.0)),
        Some(NodeId::from_raw(42))
    );
    assert_eq!(
        container.hit_test_link(Point::new(50.0, 70.0)),
        Some("https://example.com/item")
    );

    // Screen click at (50, 120) now misses the child (120 + 50 = 170, beyond child bottom 150)
    assert_eq!(container.hit_test_link(Point::new(50.0, 120.0)), None);

    // Screen click outside padding box (e.g. y = 250) is clipped and returns None
    assert_eq!(container.hit_test(Point::new(50.0, 250.0)), None);
}

#[test]
fn test_hit_testing_visibility_hidden_exclusion() {
    let mut style = ComputedStyle::default();
    style.display = Display::Block;
    style.visibility = mango_css::values::Visibility::Hidden;

    let mut hidden_box = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    hidden_box.node_id = Some(NodeId::from_raw(10));
    hidden_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);
    hidden_box.link_target = Some("https://example.com/hidden".to_string());

    assert!(!hidden_box.is_visible());
    assert_eq!(hidden_box.hit_test(Point::new(50.0, 50.0)), None);
    assert_eq!(hidden_box.hit_test_link(Point::new(50.0, 50.0)), None);

    // But a visible child inside a hidden parent DOES receive hits
    let mut child_style = ComputedStyle::default();
    child_style.display = Display::Block;
    child_style.visibility = mango_css::values::Visibility::Visible;
    let mut visible_child = LayoutBox::new(BoxType::BlockNode, Some(child_style.into()));
    visible_child.node_id = Some(NodeId::from_raw(20));
    visible_child.dimensions.content = Rect::new(10.0, 10.0, 50.0, 50.0);
    visible_child.link_target = Some("https://example.com/visible-child".to_string());
    hidden_box.children.push(visible_child);

    assert_eq!(
        hidden_box.hit_test(Point::new(25.0, 25.0)),
        Some(NodeId::from_raw(20))
    );
    assert_eq!(
        hidden_box.hit_test_link(Point::new(25.0, 25.0)),
        Some("https://example.com/visible-child")
    );
    // Outside the child but inside hidden parent still returns None
    assert_eq!(hidden_box.hit_test(Point::new(80.0, 80.0)), None);
}

#[test]
fn test_hit_testing_z_index_stacking_order() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.node_id = Some(NodeId::from_raw(1));
    parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);

    // Child 1: earlier in DOM, but higher z-index (10)
    let mut style1 = ComputedStyle::default();
    style1.display = Display::Block;
    style1.position = Position::Relative;
    style1.z_index = Some(10);
    let mut child1 = LayoutBox::new(BoxType::BlockNode, Some(style1.into()));
    child1.node_id = Some(NodeId::from_raw(10));
    child1.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Child 2: later in DOM, but lower z-index (1)
    let mut style2 = ComputedStyle::default();
    style2.display = Display::Block;
    style2.position = Position::Relative;
    style2.z_index = Some(1);
    let mut child2 = LayoutBox::new(BoxType::BlockNode, Some(style2.into()));
    child2.node_id = Some(NodeId::from_raw(20));
    child2.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    parent.children.push(child1);
    parent.children.push(child2);

    // Hit-test at (50, 50) must hit Child 1 (z-index 10) because it paints above Child 2
    assert_eq!(
        parent.hit_test(Point::new(50.0, 50.0)),
        Some(NodeId::from_raw(10))
    );
}

#[test]
fn test_hit_testing_pointer_events_none() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.node_id = Some(NodeId::from_raw(1));
    parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);

    let mut style = ComputedStyle::default();
    style.display = Display::Block;
    style.pointer_events = mango_css::values::PointerEvents::None;
    let mut child = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    child.node_id = Some(NodeId::from_raw(2));
    child.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);
    child.link_target = Some("https://example.com/unclickable".to_string());

    parent.children.push(child);

    // pointer-events: none on child passes hit through to parent
    assert_eq!(
        parent.hit_test(Point::new(50.0, 50.0)),
        Some(NodeId::from_raw(1))
    );
    assert_eq!(parent.hit_test_link(Point::new(50.0, 50.0)), None);
}

#[test]
fn test_hit_testing_border_radius_corner_clipping() {
    let mut style = ComputedStyle::default();
    style.display = Display::Block;
    style.border_top_left_radius = 50.0;
    style.border_top_right_radius = 50.0;
    style.border_bottom_right_radius = 50.0;
    style.border_bottom_left_radius = 50.0;

    let mut rounded_box = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    rounded_box.node_id = Some(NodeId::from_raw(42));
    rounded_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Center (50, 50) is inside circle -> hits
    assert_eq!(
        rounded_box.hit_test(Point::new(50.0, 50.0)),
        Some(NodeId::from_raw(42))
    );

    // Corner (2, 2) is within [0, 100]x[0, 100] bounding rect, but outside rounded corner -> returns None
    assert_eq!(rounded_box.hit_test(Point::new(2.0, 2.0)), None);
}

#[test]
fn test_hit_testing_clip_path_clipping() {
    let mut style = ComputedStyle::default();
    style.display = Display::Block;
    style.clip_path = mango_css::values::ClipPath::Circle {
        radius: Length::Px(20.0),
        center_x: Length::Px(50.0),
        center_y: Length::Px(50.0),
    };

    let mut clipped_box = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    clipped_box.node_id = Some(NodeId::from_raw(99));
    clipped_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Inside circle (50, 50) -> hits
    assert_eq!(
        clipped_box.hit_test(Point::new(50.0, 50.0)),
        Some(NodeId::from_raw(99))
    );

    // Outside circle (10, 10) -> clipped out, returns None
    assert_eq!(clipped_box.hit_test(Point::new(10.0, 10.0)), None);
}

#[test]
fn test_hit_testing_negative_z_index_on_non_stacking_context() {
    // Non-stacking context parent (normal in-flow block)
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.node_id = Some(NodeId::from_raw(1));
    parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);

    // Child with negative z-index
    let mut child_style = ComputedStyle::default();
    child_style.display = Display::Block;
    child_style.position = Position::Relative;
    child_style.z_index = Some(-1);
    let mut child = LayoutBox::new(BoxType::BlockNode, Some(child_style.into()));
    child.node_id = Some(NodeId::from_raw(2));
    child.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    parent.children.push(child);

    // Parent is NOT a stacking context, so parent's block background is painted ON TOP OF
    // the negative z-index child. Hit-test at (50, 50) must return the parent (NodeId 1).
    assert_eq!(
        parent.hit_test(Point::new(50.0, 50.0)),
        Some(NodeId::from_raw(1))
    );
}

#[test]
fn test_hit_testing_negative_z_index_on_stacking_context() {
    // Stacking context parent (e.g. position: relative)
    let mut parent_style = ComputedStyle::default();
    parent_style.display = Display::Block;
    parent_style.position = Position::Relative;
    let mut parent = LayoutBox::new(BoxType::BlockNode, Some(parent_style.into()));
    parent.node_id = Some(NodeId::from_raw(1));
    parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);

    // Child with negative z-index
    let mut child_style = ComputedStyle::default();
    child_style.display = Display::Block;
    child_style.position = Position::Relative;
    child_style.z_index = Some(-1);
    let mut child = LayoutBox::new(BoxType::BlockNode, Some(child_style.into()));
    child.node_id = Some(NodeId::from_raw(2));
    child.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    parent.children.push(child);

    // Parent IS a stacking context, so the negative z-index child is painted ON TOP OF
    // the parent's background. Hit-test at (50, 50) must return the child (NodeId 2).
    assert_eq!(
        parent.hit_test(Point::new(50.0, 50.0)),
        Some(NodeId::from_raw(2))
    );
}

#[test]
fn test_hit_testing_clip_rect_property() {
    let mut style = ComputedStyle::default();
    style.display = Display::Block;
    style.position = Position::Absolute;
    style.clip = mango_css::values::ClipRect::Rect {
        top: Length::Px(10.0),
        right: Length::Px(50.0),
        bottom: Length::Px(50.0),
        left: Length::Px(10.0),
    };

    let mut clipped_box = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    clipped_box.node_id = Some(NodeId::from_raw(77));
    clipped_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Inside clip rect: (25, 25) -> hits
    assert_eq!(
        clipped_box.hit_test(Point::new(25.0, 25.0)),
        Some(NodeId::from_raw(77))
    );

    // Outside clip rect: (5, 5) is within 100x100 content, but outside clip: rect(10, 50, 50, 10) -> None
    assert_eq!(clipped_box.hit_test(Point::new(5.0, 5.0)), None);
}

#[test]
fn test_image_map_circle_zero_radius() {
    let zero_circle = AreaShape::Circle {
        cx: 50.0,
        cy: 50.0,
        r: 0.0,
    };
    assert!(
        !zero_circle.contains_point(50.0, 50.0),
        "Zero radius circle area must have no effect"
    );

    let neg_circle = AreaShape::Circle {
        cx: 50.0,
        cy: 50.0,
        r: -5.0,
    };
    assert!(
        !neg_circle.contains_point(50.0, 50.0),
        "Negative radius circle area must have no effect"
    );

    let valid_circle = AreaShape::Circle {
        cx: 50.0,
        cy: 50.0,
        r: 10.0,
    };
    assert!(valid_circle.contains_point(50.0, 50.0));
    assert!(valid_circle.contains_point(58.0, 50.0));
    assert!(!valid_circle.contains_point(65.0, 50.0));
}

#[test]
fn test_scrollable_extent_containing_block_and_visible_overflow() {
    let mut rel_parent_style = ComputedStyle::default();
    rel_parent_style.display = Display::Block;
    rel_parent_style.position = mango_css::values::Position::Relative;

    let mut rel_parent = LayoutBox::new(BoxType::BlockNode, Some(rel_parent_style.into()));
    rel_parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);

    // Absolute child whose containing block is rel_parent
    let mut abs_style = ComputedStyle::default();
    abs_style.display = Display::Block;
    abs_style.position = mango_css::values::Position::Absolute;
    let mut abs_child = LayoutBox::new(BoxType::BlockNode, Some(abs_style.into()));
    // Positioned out at (250, 300) with size 100x100
    abs_child.dimensions.content = Rect::new(250.0, 300.0, 100.0, 100.0);
    rel_parent.children.push(abs_child);

    let (ext_x, ext_y) = rel_parent.scrollable_extent();
    assert_eq!(
        ext_x, 350.0,
        "Absolute child must contribute to containing block scrollable extent"
    );
    assert_eq!(ext_y, 400.0);

    // Non-containing block parent (Static) must NOT include absolute child
    let mut static_parent_style = ComputedStyle::default();
    static_parent_style.display = Display::Block;
    static_parent_style.position = mango_css::values::Position::Static;
    let mut static_parent = LayoutBox::new(BoxType::BlockNode, Some(static_parent_style.into()));
    static_parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);
    static_parent.children = rel_parent.children.clone();

    let (stat_x, stat_y) = static_parent.scrollable_extent();
    assert_eq!(stat_x, 200.0);
    assert_eq!(stat_y, 200.0);
}

#[test]
fn test_picture_source_dynamic_viewport() {
    let mut style = ComputedStyle::default();
    style.display = Display::Inline;

    let mut img_node = StyledNode::new(None, style);
    img_node.tag_name = Some("img".into());
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let sources_attr = format!("{data_url}\t(max-width: 500px)\timage/png");
    img_node
        .attributes
        .push(("_mango_picture_sources".into(), sources_attr));
    img_node
        .attributes
        .push(("src".into(), "http://example.com/fallback.jpg".to_string()));

    // Under 350px viewport: source matches
    mango_css::set_current_viewport(350.0, 600.0);
    let box1 = build_box_tree(&img_node);
    assert!(box1.is_replaced());
    assert_eq!(box1.intrinsic_size(), Some((1.0, 1.0)));

    // Under 800px viewport: source fails (max-width: 500px), falls back
    mango_css::set_current_viewport(800.0, 600.0);
    let box2 = build_box_tree(&img_node);
    assert!(box2.is_replaced());

    // Restore viewport
    mango_css::set_current_viewport(800.0, 600.0);
}

#[test]
fn test_form_control_dynamic_checked_and_click_offset() {
    let mut style = ComputedStyle::default();
    style.display = Display::InlineBlock;

    let mut input_box = LayoutBox::new(BoxType::InlineBlock, Some(style.into()));
    input_box.node_id = Some(NodeId::from_raw(99));
    input_box.tag_name = Some("input".into());
    input_box.dimensions.content = Rect::new(20.0, 30.0, 100.0, 25.0);
    input_box
        .attributes
        .push(("type".into(), "checkbox".to_string()));
    input_box
        .attributes
        .push(("data-mango-checked".into(), "true".to_string()));

    let hit = input_box.hit_test_form_control(Point::new(35.0, 40.0));
    assert!(hit.is_some());
    let hit = hit.unwrap();
    assert_eq!(hit.node_id, Some(NodeId::from_raw(99)));
    assert_eq!(hit.form_type, "checkbox");
    assert!(
        hit.checked,
        "data-mango-checked='true' must be recognized as checked"
    );
    assert_eq!(
        hit.click_offset_x, 15.0,
        "Click offset must accurately measure from content x (35 - 20 = 15)"
    );
}

#[test]
fn test_svg_xml_escaping_special_characters() {
    let mut svg_node = StyledNode::new(None, ComputedStyle::default());
    svg_node.tag_name = Some("svg".into());
    svg_node
        .attributes
        .push(("data-label".into(), "test <tag> & \"quotes\"".to_string()));

    let mut text_node = StyledNode::new(None, ComputedStyle::default());
    text_node.tag_name = Some("text".into());
    text_node.text = Some("A < B & C > D".to_string());
    svg_node.children.push(text_node);

    let xml = serialize_svg_styled_node(&svg_node);
    assert!(xml.contains("data-label=\"test &lt;tag&gt; &amp; &quot;quotes&quot;\""));
    assert!(xml.contains("<text>A &lt; B &amp; C &gt; D</text>"));
}

#[test]
fn test_link_target_and_frame_propagation() {
    let mut link_node = StyledNode::new(None, ComputedStyle::default());
    link_node.tag_name = Some("a".into());
    link_node
        .attributes
        .push(("href".into(), "https://example.com/popup".to_string()));
    link_node
        .attributes
        .push(("target".into(), "_blank".to_string()));

    let mut child_node = StyledNode::new(None, ComputedStyle::default());
    child_node.tag_name = Some("span".into());
    link_node.children.push(child_node);

    let mut box_tree = build_box_tree(&link_node);
    box_tree.dimensions.content = Rect::new(10.0, 10.0, 100.0, 30.0);
    if let Some(child_box) = box_tree.children.first_mut() {
        child_box.dimensions.content = Rect::new(15.0, 15.0, 50.0, 20.0);
    }

    let hit = box_tree.hit_test_link_full(Point::new(20.0, 20.0));
    assert!(hit.is_some());
    let (href, frame) = hit.unwrap();
    assert_eq!(href, "https://example.com/popup");
    assert_eq!(frame, Some("_blank"));

    // hit_test_link backwards compatibility
    let simple_hit = box_tree.hit_test_link(Point::new(20.0, 20.0));
    assert_eq!(simple_hit, Some("https://example.com/popup"));
}
