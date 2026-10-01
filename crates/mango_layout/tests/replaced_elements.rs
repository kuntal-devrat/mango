#![allow(clippy::field_reassign_with_default)]

use mango_core::Rect;
use mango_css::parser::parse_stylesheet;
use mango_css::values::{Length, ObjectFit};
use mango_html::parse_html;
use mango_layout::block_flow::layout_block;
use mango_layout::box_model::BoxType;
use mango_layout::box_tree::build_box_tree;
use mango_layout::dimensions::Dimensions;
use mango_layout::display_list::build_display_list;
use mango_layout::float::FloatContext;
use mango_layout::style_tree::build_style_tree;
use mango_render::display_list::DisplayCommand;

#[test]
fn test_image_with_only_width_maintains_aspect_ratio() {
    let mut img_box = mango_layout::box_tree::LayoutBox::new(
        BoxType::ReplacedElement {
            intrinsic_width: 800.0,
            intrinsic_height: 400.0, // 2:1 aspect ratio
            pixels: vec![],
        },
        None,
    );
    let mut style = mango_css::ComputedStyle::default();
    style.width = Length::Px(300.0);
    style.height = Length::Auto;
    img_box.style = Some(style);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 800.0));
    let mut float_ctx = FloatContext::new();
    layout_block(&mut img_box, &cb, &mut float_ctx);

    assert_eq!(img_box.dimensions.content.width(), 300.0);
    assert_eq!(
        img_box.dimensions.content.height(),
        150.0,
        "300 / 2.0 = 150"
    );
}

#[test]
fn test_image_with_only_height_maintains_aspect_ratio() {
    let mut img_box = mango_layout::box_tree::LayoutBox::new(
        BoxType::ReplacedElement {
            intrinsic_width: 600.0,
            intrinsic_height: 200.0, // 3:1 aspect ratio
            pixels: vec![],
        },
        None,
    );
    let mut style = mango_css::ComputedStyle::default();
    style.width = Length::Auto;
    style.height = Length::Px(100.0);
    img_box.style = Some(style);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 800.0));
    let mut float_ctx = FloatContext::new();
    layout_block(&mut img_box, &cb, &mut float_ctx);

    assert_eq!(img_box.dimensions.content.height(), 100.0);
    assert_eq!(img_box.dimensions.content.width(), 300.0, "100 * 3.0 = 300");
}

#[test]
fn test_css_aspect_ratio_overrides_intrinsic() {
    let mut img_box = mango_layout::box_tree::LayoutBox::new(
        BoxType::ReplacedElement {
            intrinsic_width: 800.0,
            intrinsic_height: 800.0, // 1:1 intrinsic
            pixels: vec![],
        },
        None,
    );
    let mut style = mango_css::ComputedStyle::default();
    style.aspect_ratio = Some(16.0 / 9.0); // 16:9 CSS override
    style.width = Length::Px(320.0);
    style.height = Length::Auto;
    img_box.style = Some(style);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 800.0));
    let mut float_ctx = FloatContext::new();
    layout_block(&mut img_box, &cb, &mut float_ctx);

    assert_eq!(img_box.dimensions.content.width(), 320.0);
    assert!(
        (img_box.dimensions.content.height() - 180.0).abs() < 1.0,
        "320 / (16/9) = 180"
    );
}

#[test]
fn test_svg_viewbox_intrinsic_sizing() {
    let html = r#"<svg viewBox="0 0 400 200"></svg>"#;
    let css = r#"svg { width: 200px; height: auto; }"#;
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css);
    let style_tree = build_style_tree(&doc, &[&sheet]).expect("failed to build style tree");
    let mut box_tree = build_box_tree(&style_tree);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, 800.0, 600.0));
    let mut float_ctx = FloatContext::new();
    layout_block(&mut box_tree, &cb, &mut float_ctx);

    fn find_tag<'a>(
        node: &'a mango_layout::box_tree::LayoutBox,
        tag: &str,
    ) -> Option<&'a mango_layout::box_tree::LayoutBox> {
        if node.tag_name.as_deref() == Some(tag) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_tag(child, tag) {
                return Some(found);
            }
        }
        None
    }

    let svg_box = find_tag(&box_tree, "svg").expect("svg box found in tree");

    assert_eq!(svg_box.dimensions.content.width(), 200.0);
    assert_eq!(
        svg_box.dimensions.content.height(),
        100.0,
        "viewBox 400x200 (2:1) preserves height = 100 for width = 200"
    );
}

#[test]
fn test_object_fit_and_object_position_display_commands() {
    let mut img_box = mango_layout::box_tree::LayoutBox::new(
        BoxType::ReplacedElement {
            intrinsic_width: 200.0,
            intrinsic_height: 100.0, // 2:1 natural
            pixels: vec![0; 200 * 100 * 4],
        },
        None,
    );
    // Container box: 100x100 (1:1 aspect ratio)
    img_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    // 1. object-fit: contain (with 50% 50% center)
    let mut style_contain = mango_css::ComputedStyle::default();
    style_contain.object_fit = ObjectFit::Contain;
    img_box.style = Some(style_contain);

    let dl_contain = build_display_list(&img_box);
    let cmd = dl_contain
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    assert!(cmd.is_some());
    if let Some(DisplayCommand::DrawImage {
        width, height, y, ..
    }) = cmd
    {
        assert_eq!(*width, 100.0);
        assert_eq!(*height, 50.0);
        assert_eq!(
            *y, 25.0,
            "Centered vertically in 100px height: (100 - 50)/2 = 25"
        );
    }

    // 2. object-fit: cover (emits PushClip)
    let mut style_cover = mango_css::ComputedStyle::default();
    style_cover.object_fit = ObjectFit::Cover;
    img_box.style = Some(style_cover);

    let dl_cover = build_display_list(&img_box);
    let has_clip = dl_cover
        .iter()
        .any(|c| matches!(c, DisplayCommand::PushClip { .. }));
    assert!(has_clip, "Cover must emit PushClip");
    let cmd_cover = dl_cover
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage {
        width, height, x, ..
    }) = cmd_cover
    {
        assert_eq!(*width, 200.0);
        assert_eq!(*height, 100.0);
        assert_eq!(
            *x, -50.0,
            "Centered horizontally overflow: (100 - 200)/2 = -50"
        );
    }

    // 3. object-fit: scale-down and custom object-position: 0% 0% (top-left)
    let mut style_pos = mango_css::ComputedStyle::default();
    style_pos.object_fit = ObjectFit::Contain;
    style_pos.object_position = (Length::Percent(0.0), Length::Percent(0.0));
    img_box.style = Some(style_pos);

    let dl_pos = build_display_list(&img_box);
    let cmd_pos = dl_pos
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage {
        x,
        y,
        width,
        height,
        ..
    }) = cmd_pos
    {
        assert_eq!(*width, 100.0);
        assert_eq!(*height, 50.0);
        assert_eq!(*x, 0.0, "Positioned at top-left x=0");
        assert_eq!(*y, 0.0, "Positioned at top-left y=0");
    }
}
