use mango_core::{Point, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::{Display, Length, Position};
use mango_html::dom::NodeId;

use crate::box_model::BoxType;
use crate::dimensions::Dimensions;
use crate::style_tree::StyledNode;

use super::builder::build_box_tree;
use super::replaced::serialize_svg_styled_node;
use super::types::{AreaShape, LayoutBox, MapArea, register_image_map};

#[test]
fn test_anonymous_block_box_generation() {
    let mut parent_style = ComputedStyle::default();
    parent_style.display = Display::Block;

    let mut inline_style = ComputedStyle::default();
    inline_style.display = Display::Inline;

    let mut block_style = ComputedStyle::default();
    block_style.display = Display::Block;

    // Parent block with: [Inline, Block, Text]
    let mut parent = StyledNode::new(None, parent_style);
    let child_inline = StyledNode::new(None, inline_style.clone());
    let child_block = StyledNode::new(None, block_style);
    let child_text = StyledNode::with_text(None, inline_style, "World".to_string());

    parent.children.push(child_inline);
    parent.children.push(child_block);
    parent.children.push(child_text);

    let box_tree = build_box_tree(&parent);
    assert_eq!(box_tree.box_type, BoxType::BlockNode);

    // Children should be:
    // 1. AnonymousBlock(containing child_inline)
    // 2. BlockNode
    // 3. AnonymousBlock(containing child_text)
    assert_eq!(box_tree.children.len(), 3);
    assert!(box_tree.children[0].is_anonymous());
    assert_eq!(box_tree.children[0].children.len(), 1);
    assert!(box_tree.children[0].children[0].is_inline());

    assert!(box_tree.children[1].is_block());
    assert!(!box_tree.children[1].is_anonymous());

    assert!(box_tree.children[2].is_anonymous());
    assert_eq!(box_tree.children[2].children.len(), 1);
    assert!(box_tree.children[2].children[0].is_text());
}

#[test]
fn test_replaced_element_img_generation() {
    let mut style = ComputedStyle::default();
    style.display = Display::Inline;

    let mut img_node = StyledNode::new(None, style);
    img_node.tag_name = Some("img".to_string());
    img_node
        .attributes
        .push(("width".to_string(), "120".to_string()));
    img_node
        .attributes
        .push(("height".to_string(), "80".to_string()));
    img_node.attributes.push(("src".to_string(), "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string()));

    let box_tree = build_box_tree(&img_node);
    assert!(box_tree.is_replaced());
    if let BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    } = &box_tree.box_type
    {
        assert_eq!(*intrinsic_width, 120.0);
        assert_eq!(*intrinsic_height, 80.0);
        assert_eq!(pixels.len(), 120 * 80);
    } else {
        panic!("Expected ReplacedElement");
    }
}

#[test]
fn test_iframe_box_tree_generation_with_srcdoc() {
    let mut style = ComputedStyle::default();
    style.display = Display::InlineBlock;

    let mut iframe_node = StyledNode::new(None, style);
    iframe_node.tag_name = Some("iframe".to_string());
    iframe_node
        .attributes
        .push(("width".to_string(), "450".to_string()));
    iframe_node
        .attributes
        .push(("height".to_string(), "250".to_string()));
    iframe_node.attributes.push((
        "sandbox".to_string(),
        "allow-scripts allow-forms".to_string(),
    ));
    iframe_node
        .attributes
        .push(("srcdoc".to_string(), "<p>Nested Document</p>".to_string()));

    let box_tree = build_box_tree(&iframe_node);
    assert!(box_tree.is_replaced());
    assert!(box_tree.is_iframe());

    if let BoxType::IFrame {
        src: _,
        srcdoc,
        sandbox,
        intrinsic_width,
        intrinsic_height,
    } = &box_tree.box_type
    {
        assert_eq!(*intrinsic_width, 450.0);
        assert_eq!(*intrinsic_height, 250.0);
        assert!(sandbox.is_sandboxed);
        assert!(sandbox.allow_scripts);
        assert!(sandbox.allow_forms);
        assert!(!sandbox.allow_same_origin);
        assert_eq!(srcdoc.as_deref(), Some("<p>Nested Document</p>"));
    } else {
        panic!("Expected BoxType::IFrame");
    }

    // Inner document must be parsed and attached as child boxes
    assert!(
        !box_tree.children.is_empty(),
        "Inner srcdoc must produce child layout boxes"
    );
}

#[test]
fn test_picture_source_selection() {
    let mut style = ComputedStyle::default();
    style.display = Display::Inline;

    let mut img_node = StyledNode::new(None, style);
    img_node.tag_name = Some("img".to_string());
    // 1x1 red PNG data URI
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let sources_attr = format!("{data_url}\t(min-width: 300px)\timage/png");
    img_node
        .attributes
        .push(("_mango_picture_sources".to_string(), sources_attr));
    img_node.attributes.push((
        "src".to_string(),
        "http://example.com/fallback.jpg".to_string(),
    ));

    let box_tree = build_box_tree(&img_node);
    assert!(box_tree.is_replaced());
    if let BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    } = &box_tree.box_type
    {
        assert_eq!(*intrinsic_width, 1.0);
        assert_eq!(*intrinsic_height, 1.0);
        assert_eq!(pixels.len(), 1);
    } else {
        panic!("Expected ReplacedElement from <picture> source");
    }
}

#[test]
fn test_image_map_shapes_and_hit_testing() {
    // Register an image map with rect, circle, poly, and default shapes
    let areas = vec![
        MapArea {
            shape: AreaShape::Rect {
                left: 10.0,
                top: 10.0,
                right: 50.0,
                bottom: 50.0,
            },
            href: Some("/rect-target".to_string()),
            alt: Some("Rectangle".to_string()),
            target: None,
        },
        MapArea {
            shape: AreaShape::Circle {
                cx: 100.0,
                cy: 100.0,
                r: 20.0,
            },
            href: Some("/circle-target".to_string()),
            alt: Some("Circle".to_string()),
            target: None,
        },
        MapArea {
            shape: AreaShape::Poly {
                points: vec![(150.0, 10.0), (200.0, 10.0), (175.0, 60.0)],
            },
            href: Some("/poly-target".to_string()),
            alt: Some("Triangle".to_string()),
            target: None,
        },
        MapArea {
            shape: AreaShape::Default,
            href: Some("/default-target".to_string()),
            alt: Some("Default".to_string()),
            target: None,
        },
    ];
    register_image_map("navmap", areas);

    let mut img_style = ComputedStyle::default();
    img_style.display = Display::InlineBlock;
    let mut img_node = StyledNode::new(None, img_style);
    img_node.tag_name = Some("img".to_string());
    img_node
        .attributes
        .push(("usemap".to_string(), "#navmap".to_string()));
    img_node
        .attributes
        .push(("width".to_string(), "300".to_string()));
    img_node
        .attributes
        .push(("height".to_string(), "200".to_string()));

    let mut img_box = build_box_tree(&img_node);
    assert_eq!(img_box.map_areas.len(), 4);

    // Position the box at (0, 0, 300, 200)
    img_box.dimensions.content = Rect::new(0.0, 0.0, 300.0, 200.0);

    // Hit inside rectangle (25, 25)
    let hit_rect = img_box.hit_test_link(Point::new(25.0, 25.0));
    assert_eq!(hit_rect, Some("/rect-target"));

    // Hit inside circle (105, 105)
    let hit_circle = img_box.hit_test_link(Point::new(105.0, 105.0));
    assert_eq!(hit_circle, Some("/circle-target"));

    // Hit inside triangle polygon (175, 25)
    let hit_poly = img_box.hit_test_link(Point::new(175.0, 25.0));
    assert_eq!(hit_poly, Some("/poly-target"));

    // Hit outside specific shapes falls to default
    let hit_default = img_box.hit_test_link(Point::new(280.0, 180.0));
    assert_eq!(hit_default, Some("/default-target"));

    // Hit outside the image bounds returns None
    let hit_outside = img_box.hit_test_link(Point::new(400.0, 400.0));
    assert_eq!(hit_outside, None);
}

#[test]
fn test_object_graceful_fallback_and_embed() {
    // 1. Embed element produces replaced element
    let mut embed_node = StyledNode::new(None, ComputedStyle::default());
    embed_node.tag_name = Some("embed".to_string());
    embed_node
        .attributes
        .push(("width".to_string(), "200".to_string()));
    embed_node
        .attributes
        .push(("height".to_string(), "100".to_string()));
    let embed_box = build_box_tree(&embed_node);
    assert!(embed_box.is_replaced());

    // 2. Object with invalid data gracefully falls back to children
    let mut object_node = StyledNode::new(None, ComputedStyle::default());
    object_node.tag_name = Some("object".to_string());
    object_node
        .attributes
        .push(("data".to_string(), "invalid://missing.swf".to_string()));

    let mut child_p = StyledNode::new(None, ComputedStyle::default());
    child_p.tag_name = Some("p".to_string());
    child_p.text = Some("Flash plugin not supported".to_string());
    object_node.children.push(child_p);

    let object_box = build_box_tree(&object_node);
    // Falls through to render children!
    assert!(!object_box.is_replaced());
    assert_eq!(object_box.children.len(), 1);
    assert_eq!(
        object_box.children[0].text(),
        Some("Flash plugin not supported")
    );
}

#[test]
fn test_svg_viewbox_intrinsic_sizing_with_width_or_height() {
    let style = ComputedStyle::default();

    // Case 1: Pure viewBox "0 0 200 100" with no width/height attributes
    let mut svg_node1 = StyledNode::new(None, style.clone());
    svg_node1.tag_name = Some("svg".to_string());
    svg_node1
        .attributes
        .push(("viewBox".to_string(), "0 0 200 100".to_string()));

    let box_tree1 = build_box_tree(&svg_node1);
    assert!(box_tree1.is_replaced());
    if let BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        ..
    } = &box_tree1.box_type
    {
        assert_eq!(
            *intrinsic_width, 200.0,
            "Intrinsic width should be taken from viewBox"
        );
        assert_eq!(
            *intrinsic_height, 100.0,
            "Intrinsic height should be taken from viewBox"
        );
    } else {
        panic!("Expected ReplacedElement for SVG");
    }

    // Case 2: SVG with viewBox "0 0 200 100" and only width="400" attribute
    // Height should be derived from viewBox aspect ratio: 400 / (200/100) = 200
    let mut svg_node2 = StyledNode::new(None, style.clone());
    svg_node2.tag_name = Some("svg".to_string());
    svg_node2
        .attributes
        .push(("viewBox".to_string(), "0 0 200 100".to_string()));
    svg_node2
        .attributes
        .push(("width".to_string(), "400".to_string()));

    let box_tree2 = build_box_tree(&svg_node2);
    assert!(box_tree2.is_replaced());
    if let BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        ..
    } = &box_tree2.box_type
    {
        assert_eq!(*intrinsic_width, 400.0);
        assert_eq!(
            *intrinsic_height, 200.0,
            "Height should be derived from viewBox aspect ratio"
        );
    } else {
        panic!("Expected ReplacedElement for SVG");
    }

    // Case 3: SVG with viewBox "0 0 100 200" and only height="100" attribute
    // Width should be derived from viewBox aspect ratio: 100 * (100/200) = 50
    let mut svg_node3 = StyledNode::new(None, style.clone());
    svg_node3.tag_name = Some("svg".to_string());
    svg_node3
        .attributes
        .push(("viewBox".to_string(), "0 0 100 200".to_string()));
    svg_node3
        .attributes
        .push(("height".to_string(), "100".to_string()));

    let box_tree3 = build_box_tree(&svg_node3);
    assert!(box_tree3.is_replaced());
    if let BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        ..
    } = &box_tree3.box_type
    {
        assert_eq!(
            *intrinsic_width, 50.0,
            "Width should be derived from viewBox aspect ratio"
        );
        assert_eq!(*intrinsic_height, 100.0);
    } else {
        panic!("Expected ReplacedElement for SVG");
    }

    // Case 4: SVG with viewBox "0 0 200 100" placed in block layout with CSS width: 600px, height: auto
    let mut svg_node4 = StyledNode::new(None, style);
    svg_node4.tag_name = Some("svg".to_string());
    svg_node4
        .attributes
        .push(("viewBox".to_string(), "0 0 200 100".to_string()));
    let mut box_tree4 = build_box_tree(&svg_node4);
    let mut css_style = ComputedStyle::default();
    css_style.width = mango_css::Length::Px(600.0);
    css_style.height = mango_css::Length::Auto;
    box_tree4.style = Some(css_style.into());

    let cb = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 800.0));
    let mut float_ctx = crate::FloatContext::new();
    crate::block_flow::layout_block(&mut box_tree4, &cb, &mut float_ctx);

    assert_eq!(box_tree4.dimensions.content.width(), 600.0);
    assert_eq!(
        box_tree4.dimensions.content.height(),
        300.0,
        "Auto height in block layout should preserve viewBox 2:1 ratio"
    );
}

#[test]
fn test_anonymous_block_style_isolation() {
    let mut parent_style = ComputedStyle::default();
    parent_style.display = Display::Block;
    parent_style.overflow_x = mango_css::values::Overflow::Scroll;
    parent_style.overflow_y = mango_css::values::Overflow::Auto;
    parent_style.z_index = Some(999);
    parent_style.opacity = 0.5;

    let anon = LayoutBox::new_anonymous_block(Some(parent_style.into()));
    assert_eq!(anon.box_type, BoxType::AnonymousBlock);
    assert!(
        !anon.is_scroll_container(),
        "Anonymous block must not establish a scroll container"
    );
    let style = anon.style.as_ref().unwrap();
    assert_eq!(style.overflow_x, mango_css::values::Overflow::Visible);
    assert_eq!(style.overflow_y, mango_css::values::Overflow::Visible);
    assert_eq!(style.z_index, None);
    assert_eq!(style.opacity, 1.0);
    assert!(style.transform.is_identity());
}

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
    assert_eq!(parent.hit_test(Point::new(50.0, 50.0)), Some(NodeId::from_raw(10)));
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
    assert_eq!(parent.hit_test(Point::new(50.0, 50.0)), Some(NodeId::from_raw(1)));
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
    assert_eq!(rounded_box.hit_test(Point::new(50.0, 50.0)), Some(NodeId::from_raw(42)));

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
    assert_eq!(clipped_box.hit_test(Point::new(50.0, 50.0)), Some(NodeId::from_raw(99)));

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
    assert_eq!(parent.hit_test(Point::new(50.0, 50.0)), Some(NodeId::from_raw(1)));
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
    assert_eq!(parent.hit_test(Point::new(50.0, 50.0)), Some(NodeId::from_raw(2)));
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
    assert_eq!(clipped_box.hit_test(Point::new(25.0, 25.0)), Some(NodeId::from_raw(77)));

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
    img_node.tag_name = Some("img".to_string());
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let sources_attr = format!("{data_url}\t(max-width: 500px)\timage/png");
    img_node
        .attributes
        .push(("_mango_picture_sources".to_string(), sources_attr));
    img_node.attributes.push((
        "src".to_string(),
        "http://example.com/fallback.jpg".to_string(),
    ));

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
    input_box.tag_name = Some("input".to_string());
    input_box.dimensions.content = Rect::new(20.0, 30.0, 100.0, 25.0);
    input_box
        .attributes
        .push(("type".to_string(), "checkbox".to_string()));
    input_box
        .attributes
        .push(("data-mango-checked".to_string(), "true".to_string()));

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
    svg_node.tag_name = Some("svg".to_string());
    svg_node.attributes.push((
        "data-label".to_string(),
        "test <tag> & \"quotes\"".to_string(),
    ));

    let mut text_node = StyledNode::new(None, ComputedStyle::default());
    text_node.tag_name = Some("text".to_string());
    text_node.text = Some("A < B & C > D".to_string());
    svg_node.children.push(text_node);

    let xml = serialize_svg_styled_node(&svg_node);
    assert!(xml.contains("data-label=\"test &lt;tag&gt; &amp; &quot;quotes&quot;\""));
    assert!(xml.contains("<text>A &lt; B &amp; C &gt; D</text>"));
}

#[test]
fn test_link_target_and_frame_propagation() {
    let mut link_node = StyledNode::new(None, ComputedStyle::default());
    link_node.tag_name = Some("a".to_string());
    link_node
        .attributes
        .push(("href".to_string(), "https://example.com/popup".to_string()));
    link_node
        .attributes
        .push(("target".to_string(), "_blank".to_string()));

    let mut child_node = StyledNode::new(None, ComputedStyle::default());
    child_node.tag_name = Some("span".to_string());
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
