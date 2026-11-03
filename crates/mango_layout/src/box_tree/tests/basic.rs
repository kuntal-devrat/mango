use mango_core::{Point, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::Display;

use crate::box_model::BoxType;
use crate::dimensions::Dimensions;
use crate::style_tree::StyledNode;

use super::super::builder::build_box_tree;
use super::super::types::{AreaShape, MapArea, register_image_map};

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
    img_node.tag_name = Some("img".into());
    img_node
        .attributes
        .push(("width".into(), "120".to_string()));
    img_node
        .attributes
        .push(("height".into(), "80".to_string()));
    img_node.attributes.push(("src".into(), "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string()));

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
    iframe_node.tag_name = Some("iframe".into());
    iframe_node
        .attributes
        .push(("width".into(), "450".to_string()));
    iframe_node
        .attributes
        .push(("height".into(), "250".to_string()));
    iframe_node
        .attributes
        .push(("sandbox".into(), "allow-scripts allow-forms".to_string()));
    iframe_node
        .attributes
        .push(("srcdoc".into(), "<p>Nested Document</p>".to_string()));

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
    img_node.tag_name = Some("img".into());
    // 1x1 red PNG data URI
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let sources_attr = format!("{data_url}\t(min-width: 300px)\timage/png");
    img_node
        .attributes
        .push(("_mango_picture_sources".into(), sources_attr));
    img_node
        .attributes
        .push(("src".into(), "http://example.com/fallback.jpg".to_string()));

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
    img_node.tag_name = Some("img".into());
    img_node
        .attributes
        .push(("usemap".into(), "#navmap".to_string()));
    img_node
        .attributes
        .push(("width".into(), "300".to_string()));
    img_node
        .attributes
        .push(("height".into(), "200".to_string()));

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
    embed_node.tag_name = Some("embed".into());
    embed_node
        .attributes
        .push(("width".into(), "200".to_string()));
    embed_node
        .attributes
        .push(("height".into(), "100".to_string()));
    let embed_box = build_box_tree(&embed_node);
    assert!(embed_box.is_replaced());

    // 2. Object with invalid data gracefully falls back to children
    let mut object_node = StyledNode::new(None, ComputedStyle::default());
    object_node.tag_name = Some("object".into());
    object_node
        .attributes
        .push(("data".into(), "invalid://missing.swf".to_string()));

    let mut child_p = StyledNode::new(None, ComputedStyle::default());
    child_p.tag_name = Some("p".into());
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
    svg_node1.tag_name = Some("svg".into());
    svg_node1
        .attributes
        .push(("viewBox".into(), "0 0 200 100".to_string()));

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
    svg_node2.tag_name = Some("svg".into());
    svg_node2
        .attributes
        .push(("viewBox".into(), "0 0 200 100".to_string()));
    svg_node2
        .attributes
        .push(("width".into(), "400".to_string()));

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
    svg_node3.tag_name = Some("svg".into());
    svg_node3
        .attributes
        .push(("viewBox".into(), "0 0 100 200".to_string()));
    svg_node3
        .attributes
        .push(("height".into(), "100".to_string()));

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
    svg_node4.tag_name = Some("svg".into());
    svg_node4
        .attributes
        .push(("viewBox".into(), "0 0 200 100".to_string()));
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

    let anon = super::super::types::LayoutBox::new_anonymous_block(Some(parent_style.into()));
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
