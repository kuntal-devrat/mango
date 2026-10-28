use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::Position;
use mango_render::display_list::DisplayCommand;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::super::*;

#[test]
fn test_transform_emits_push_and_pop_around_subtree() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 100.0);

    let mut rotated = ComputedStyle::default();
    rotated.background_color = Color::RED;
    rotated.transform =
        mango_css::values::Transform(vec![mango_css::values::TransformFunction::Rotate(45.0)]);
    rotated.transform_origin_x = mango_css::values::Length::Percent(50.0);
    rotated.transform_origin_y = mango_css::values::Length::Percent(50.0);

    let mut child = LayoutBox::new(BoxType::BlockNode, Some(rotated.into()));
    child.dimensions.content = Rect::new(10.0, 10.0, 50.0, 50.0);
    parent.children.push(child);

    let dl = build_display_list(&parent);
    let push_idx = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::PushTransform { .. }))
        .expect("PushTransform emitted for transform: rotate()");
    let pop_idx = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::PopTransform))
        .expect("PopTransform emitted");
    assert!(
        push_idx < pop_idx,
        "transform must wrap the painted subtree"
    );

    // The push matrix must include the rotation (b != 0 for a rotated matrix).
    if let Some(DisplayCommand::PushTransform { matrix }) = dl.iter().nth(push_idx) {
        assert!(
            matrix[1].abs() > 0.1,
            "rotation must produce a non-zero b component"
        );
    }

    // Identity transforms must not emit any transform commands.
    let plain = LayoutBox::new(BoxType::BlockNode, Some(ComputedStyle::default().into()));
    let dl_plain = build_display_list(&plain);
    assert!(
        !dl_plain
            .iter()
            .any(|c| matches!(c, DisplayCommand::PushTransform { .. }))
    );
}

#[test]
fn test_gradient_background_emits_fill_gradient() {
    let mut style = ComputedStyle::default();
    style.background_gradient = Some(mango_css::values::Gradient::Linear {
        angle_deg: 90.0,
        stops: vec![
            mango_css::values::ColorStop::new(Color::RED, Some(0.0), None),
            mango_css::values::ColorStop::new(Color::BLUE, Some(1.0), None),
        ],
        repeating: false,
    });

    let mut root = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    root.dimensions.content = Rect::new(0.0, 0.0, 120.0, 60.0);

    let dl = build_display_list(&root);
    let gradient_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::FillGradient { .. }));
    match gradient_cmd {
        Some(DisplayCommand::FillGradient { rect, gradient, .. }) => {
            assert_eq!(rect.width(), 120.0);
            assert_eq!(gradient.stops().len(), 2);
        }
        _ => panic!("expected a FillGradient command for a gradient background"),
    }
}

#[test]
fn test_layout_gradients() {
    let mut style = ComputedStyle::default();
    style.color = Color::rgb(0, 128, 255);
    style.background_gradient = Some(mango_css::values::Gradient::Linear {
        angle_deg: 180.0,
        stops: vec![
            mango_css::values::ColorStop {
                color: Color::BLACK,
                position: Some(mango_css::values::StopPosition::Fraction(0.0)),
                end_position: None,
                is_current_color: true,
                var_name: None,
            },
            mango_css::values::ColorStop::new(Color::TRANSPARENT, Some(1.0), None),
        ],
        repeating: false,
    });

    let mut root = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    root.dimensions.content = Rect::new(10.0, 20.0, 200.0, 100.0);

    let dl = build_display_list(&root);
    let grad_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::FillGradient { .. }));
    assert!(grad_cmd.is_some());
    if let Some(DisplayCommand::FillGradient { gradient, rect, .. }) = grad_cmd {
        assert_eq!(rect.width(), 200.0);
        assert_eq!(rect.height(), 100.0);
        // CurrentColor must be resolved to style.color
        assert_eq!(gradient.stops()[0].color, Color::rgb(0, 128, 255));
    }
}

#[test]
fn test_text_shadow_emits_shadow_command_before_text() {
    let mut style = ComputedStyle::default();
    style.color = Color::BLACK;
    style.font_size = 16.0;
    style.text_shadow = Some(mango_css::values::TextShadow {
        offset_x: 1.0,
        offset_y: 2.0,
        blur_radius: 3.0,
        color: Color::rgba(0, 0, 0, 128),
    });

    let mut text_box = LayoutBox::new(
        BoxType::TextNode("Shadowed".to_string()),
        Some(style.into()),
    );
    text_box.dimensions.content = Rect::new(5.0, 5.0, 80.0, 20.0);

    let dl = build_display_list(&text_box);
    let shadow_idx = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::DrawTextShadow { .. }))
        .expect("DrawTextShadow emitted for text-shadow");
    let text_idx = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::DrawText { .. }))
        .expect("DrawText emitted");
    assert!(shadow_idx < text_idx, "shadow must paint behind the glyphs");

    if let Some(DisplayCommand::DrawTextShadow {
        x, y, blur_radius, ..
    }) = dl.iter().nth(shadow_idx)
    {
        assert_eq!(*x, 6.0);
        assert_eq!(*y, 7.0);
        assert_eq!(*blur_radius, 3.0);
    }
}

#[test]
fn test_build_display_list_commands() {
    let mut root = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.background_color = Color::RED;
    style.color = Color::BLACK;
    style.font_size = 16.0;
    style.border_top_width = 2.0;

    root.style = Some(style.into());
    root.dimensions.content = Rect::new(10.0, 10.0, 100.0, 50.0);
    root.dimensions.border.top = 2.0;

    let text_child = LayoutBox::new(
        BoxType::TextNode("Test Text".to_string()),
        root.style.clone(),
    );
    root.children.push(text_child);

    let dl = build_display_list(&root);
    // Expect: FillRect (background), DrawBorder, DrawText
    assert_eq!(dl.len(), 3);
}

#[test]
fn test_z_index_stacking_order() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.dimensions.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    // Child 1: positive z-index (+10)
    let mut style_pos = ComputedStyle::default();
    style_pos.position = Position::Relative;
    style_pos.z_index = Some(10);
    style_pos.background_color = Color::BLUE;
    let mut child_pos = LayoutBox::new(BoxType::BlockNode, Some(style_pos.into()));
    child_pos.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Child 2: negative z-index (-5)
    let mut style_neg = ComputedStyle::default();
    style_neg.position = Position::Relative;
    style_neg.z_index = Some(-5);
    style_neg.background_color = Color::RED;
    let mut child_neg = LayoutBox::new(BoxType::BlockNode, Some(style_neg.into()));
    child_neg.dimensions.content = Rect::new(10.0, 10.0, 100.0, 100.0);

    // Child 3: normal flow (static)
    let mut style_norm = ComputedStyle::default();
    style_norm.background_color = Color::GREEN;
    let mut child_norm = LayoutBox::new(BoxType::BlockNode, Some(style_norm.into()));
    child_norm.dimensions.content = Rect::new(20.0, 20.0, 100.0, 100.0);

    // Appended in order: pos (+10), neg (-5), norm (0)
    parent.children.push(child_pos);
    parent.children.push(child_neg);
    parent.children.push(child_norm);

    let dl = build_display_list(&parent);
    // Painted order must be: neg (-5) -> norm (static) -> pos (+10)
    assert_eq!(dl.len(), 3);
    match (&dl[0], &dl[1], &dl[2]) {
        (
            DisplayCommand::FillRect { color: c1, .. },
            DisplayCommand::FillRect { color: c2, .. },
            DisplayCommand::FillRect { color: c3, .. },
        ) => {
            assert_eq!(c1, &Color::RED, "Negative z-index (-5) must paint first");
            assert_eq!(c2, &Color::GREEN, "Normal flow must paint second");
            assert_eq!(c3, &Color::BLUE, "Positive z-index (+10) must paint last");
        }
        _ => panic!("Expected FillRect commands"),
    }
}

#[test]
fn test_border_radius_generates_fill_rounded_rect() {
    let mut root = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.background_color = Color::BLUE;
    style.border_top_left_radius = 8.0;
    style.border_top_right_radius = 8.0;
    style.border_bottom_right_radius = 8.0;
    style.border_bottom_left_radius = 8.0;

    root.style = Some(style.into());
    root.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

    let dl = build_display_list(&root);
    assert_eq!(dl.len(), 1);
    match &dl[0] {
        DisplayCommand::FillRoundedRect { radii, color, .. } => {
            assert_eq!(*radii, [8.0, 8.0, 8.0, 8.0]);
            assert_eq!(*color, Color::BLUE);
        }
        _ => panic!("Expected FillRoundedRect command"),
    }
}

#[test]
fn test_visibility_hidden_skips_rendering() {
    let mut root = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.background_color = Color::RED;
    style.visibility = mango_css::values::Visibility::Hidden;

    root.style = Some(style.into());
    root.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

    let dl = build_display_list(&root);
    assert!(dl.is_empty(), "Hidden element should generate no commands");
}

#[test]
fn test_opacity_scales_colors() {
    let mut root = LayoutBox::new(BoxType::BlockNode, None);
    let mut style = ComputedStyle::default();
    style.background_color = Color::rgba(255, 0, 0, 200);
    style.opacity = 0.5;

    root.style = Some(style.into());
    root.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

    let dl = build_display_list(&root);
    assert_eq!(dl.len(), 1);
    match &dl[0] {
        DisplayCommand::FillRect { color, .. } => {
            // 200 * 0.5 = 100
            assert_eq!(color.a, 100);
            assert_eq!(color.r, 255);
        }
        _ => panic!("Expected FillRect command"),
    }
}

#[test]
fn test_overflow_hidden_generates_push_and_pop_clip() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    let mut p_style = ComputedStyle::default();
    p_style.overflow_x = mango_css::values::Overflow::Hidden;
    p_style.overflow_y = mango_css::values::Overflow::Hidden;
    parent.style = Some(p_style.into());
    parent.dimensions.content = Rect::new(10.0, 10.0, 100.0, 100.0);

    let mut child = LayoutBox::new(BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.background_color = Color::RED;
    child.style = Some(c_style.into());
    child.dimensions.content = Rect::new(10.0, 10.0, 200.0, 200.0);
    parent.children.push(child);

    let dl = build_display_list(&parent);

    let push_clip_idx = dl
        .iter()
        .position(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
    let pop_clip_idx = dl
        .iter()
        .position(|cmd| matches!(cmd, DisplayCommand::PopClip));
    let fill_idx = dl.iter().position(
        |cmd| matches!(cmd, DisplayCommand::FillRect { color, .. } if *color == Color::RED),
    );

    assert!(
        push_clip_idx.is_some(),
        "PushClip must be emitted for overflow: hidden container"
    );
    assert!(
        pop_clip_idx.is_some(),
        "PopClip must be emitted after child rendering"
    );
    assert!(fill_idx.is_some(), "Child fill must be emitted");

    let push_idx = push_clip_idx.unwrap();
    let fill = fill_idx.unwrap();
    let pop_idx = pop_clip_idx.unwrap();

    assert!(
        push_idx < fill && fill < pop_idx,
        "Child fill must be surrounded by PushClip and PopClip"
    );
}

#[test]
fn test_form_controls_checkbox_checked() {
    let mut input_box = LayoutBox::new(BoxType::InlineBlock, None);
    input_box.tag_name = Some("input".to_string());
    input_box
        .attributes
        .push(("type".to_string(), "checkbox".to_string()));
    input_box
        .attributes
        .push(("checked".to_string(), "true".to_string()));
    input_box.style = Some(ComputedStyle::default().into());
    input_box.dimensions.content = Rect::new(10.0, 10.0, 14.0, 14.0);

    let dl = build_display_list(&input_box);
    // Expect FillRoundedRect (inner checkmark) and DrawText ("✓")
    assert_eq!(dl.len(), 2);
    match &dl[1] {
        DisplayCommand::DrawText { text, .. } => {
            assert_eq!(text, "✓");
        }
        _ => panic!("Expected DrawText checkmark"),
    }
}

#[test]
fn test_select_options() {
    let html_path = if std::path::Path::new("scratch/ddg_results.html").exists() {
        "scratch/ddg_results.html"
    } else {
        "../../scratch/ddg_results.html"
    };
    let css_path = if std::path::Path::new("scratch_ddg.css").exists() {
        "scratch_ddg.css"
    } else {
        "../../scratch_ddg.css"
    };
    if let Ok(html) = std::fs::read_to_string(html_path) {
        println!("Loaded html: {} bytes", html.len());
        let doc = mango_html::parse_html(&html);
        let css_text = std::fs::read_to_string(css_path).unwrap_or_default();
        println!("Loaded css: {} bytes", css_text.len());
        let sheet = mango_css::parser::parse_stylesheet(&css_text);
        let (_box_tree, dl) =
            crate::layout_document(&doc, &[&sheet], mango_core::Size::new(1280.0, 900.0));
        let texts: Vec<&str> = dl
            .iter()
            .filter_map(|cmd| {
                if let DisplayCommand::DrawText { text, .. } = cmd {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            texts.contains(&"All Regions"),
            "Select should render first option text 'All Regions'"
        );
        assert!(
            texts.contains(&"Any Time"),
            "Select should render first option text 'Any Time'"
        );
        assert!(
            !texts.contains(&"Select..."),
            "Select should not render fallback 'Select...'"
        );

        // Verify search button has DrawImage (vector icon) instead of missing emoji "🔍"
        let has_search_vector_icon = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
        assert!(
            has_search_vector_icon,
            "Search button should render crisp vector icon"
        );
        assert!(
            !texts.contains(&"🔍"),
            "Search button must not output missing emoji glyph '🔍'"
        );
    }
}

#[test]
fn test_input_caret_fill_rect() {
    let mut input_box = LayoutBox::new(BoxType::BlockNode, Some(ComputedStyle::default().into()));
    input_box.tag_name = Some("input".to_string());
    input_box
        .attributes
        .push(("type".to_string(), "text".to_string()));
    input_box
        .attributes
        .push(("value".to_string(), "hello".to_string()));
    input_box
        .attributes
        .push(("data-mango-focused".to_string(), "true".to_string()));
    input_box
        .attributes
        .push(("_mango_cursor_pos".to_string(), "3".to_string()));
    input_box.dimensions.content = Rect::new(10.0, 10.0, 150.0, 30.0);

    let dl = build_display_list(&input_box);

    // Verify text is drawn clean without '|' injected into it
    let has_clean_text = dl.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text == "hello",
        _ => false,
    });
    assert!(
        has_clean_text,
        "Input display_text must be clean 'hello' without '|'"
    );

    // Verify caret is drawn as a FillRect command
    let has_caret_rect = dl.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { rect, .. } => rect.width() == 1.5 && rect.x() > 10.0,
        _ => false,
    });
    assert!(
        has_caret_rect,
        "Input caret must be rendered as a FillRect line"
    );
}

#[test]
fn test_position_fixed_offsets_with_scroll() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.dimensions.content = Rect::new(0.0, 0.0, 500.0, 1000.0);

    let mut fixed_style = ComputedStyle::default();
    fixed_style.position = Position::Fixed;
    fixed_style.background_color = Color::BLUE;
    let mut fixed_child = LayoutBox::new(BoxType::BlockNode, Some(fixed_style.into()));
    fixed_child.dimensions.content = Rect::new(0.0, 10.0, 200.0, 40.0);
    parent.children.push(fixed_child);

    // At scroll_y = 0, y is 10.0
    let dl0 = build_display_list_with_scroll(&parent, 0.0);
    assert_eq!(dl0.len(), 1);
    if let DisplayCommand::FillRect { rect, .. } = &dl0[0] {
        assert_eq!(rect.y(), 10.0);
    } else {
        panic!("Expected FillRect");
    }

    // At scroll_y = 150, fixed element y is offset by +150 to 160.0
    let dl150 = build_display_list_with_scroll(&parent, 150.0);
    assert_eq!(dl150.len(), 1);
    if let DisplayCommand::FillRect { rect, .. } = &dl150[0] {
        assert_eq!(rect.y(), 160.0);
    } else {
        panic!("Expected FillRect");
    }
}

#[test]
fn test_position_sticky_clamping_with_scroll() {
    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.dimensions.content = Rect::new(0.0, 0.0, 500.0, 1000.0);

    let mut sticky_style = ComputedStyle::default();
    sticky_style.position = Position::Sticky;
    sticky_style.top = mango_css::values::Length::Px(10.0);
    sticky_style.background_color = Color::GREEN;
    let mut sticky_child = LayoutBox::new(BoxType::BlockNode, Some(sticky_style.into()));
    sticky_child.dimensions.content = Rect::new(0.0, 200.0, 200.0, 50.0);
    parent.children.push(sticky_child);

    // At scroll_y = 50: scroll_y + top (60) < 200 => stays at in-flow y = 200.0
    let dl50 = build_display_list_with_scroll(&parent, 50.0);
    if let DisplayCommand::FillRect { rect, .. } = &dl50[0] {
        assert_eq!(rect.y(), 200.0);
    } else {
        panic!("Expected FillRect");
    }

    // At scroll_y = 300: scroll_y + top (310) > 200 => sticks to y = 310.0
    let dl300 = build_display_list_with_scroll(&parent, 300.0);
    if let DisplayCommand::FillRect { rect, .. } = &dl300[0] {
        assert_eq!(rect.y(), 310.0);
    } else {
        panic!("Expected FillRect");
    }

    // At scroll_y = 1200: exceeds parent bottom (1000) => clamped to parent bottom - height = 950.0
    let dl1200 = build_display_list_with_scroll(&parent, 1200.0);
    if let DisplayCommand::FillRect { rect, .. } = &dl1200[0] {
        assert_eq!(rect.y(), 950.0);
    } else {
        panic!("Expected FillRect");
    }
}

#[test]
fn test_background_image_display_command() {
    let fake_url = "https://example.com/bg.png";
    let fake_img = mango_render::image_decode::DecodedImage {
        width: 10,
        height: 10,
        pixels: vec![0xFF00FF00; 100],
    };
    mango_render::cache_image(fake_url, fake_img);

    let mut style = ComputedStyle::default();
    style.background_image = Some(fake_url.to_string());
    style.background_repeat = mango_css::values::BackgroundRepeat::NoRepeat;
    style.background_size = mango_css::values::BackgroundSize::Explicit(
        mango_css::values::Length::Px(40.0),
        mango_css::values::Length::Px(40.0),
    );

    let mut box_node = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    box_node.dimensions.content = Rect::new(10.0, 20.0, 100.0, 100.0);

    let dl = build_display_list(&box_node);
    let draw_img_cmds: Vec<&DisplayCommand> = dl
        .iter()
        .filter(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }))
        .collect();
    assert_eq!(draw_img_cmds.len(), 1);
    if let DisplayCommand::DrawImage {
        x,
        y,
        width,
        height,
        pixels,
    } = draw_img_cmds[0]
    {
        assert_eq!(*x, 10.0);
        assert_eq!(*y, 20.0);
        assert_eq!(*width, 40.0);
        assert_eq!(*height, 40.0);
        assert_eq!(pixels.len(), 40 * 40);
    } else {
        panic!("Expected DrawImage");
    }
}

#[test]
fn test_box_shadow_display_command() {
    let mut style = ComputedStyle::default();
    style.box_shadow = Some(mango_css::values::BoxShadow {
        offset_x: 4.0,
        offset_y: 8.0,
        blur_radius: 12.0,
        spread_radius: 2.0,
        color: Color::rgba(0, 0, 0, 100),
        inset: false,
    });

    let mut box_node = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    box_node.dimensions.content = Rect::new(50.0, 50.0, 200.0, 100.0);

    let dl = build_display_list(&box_node);
    let shadow_cmds: Vec<&DisplayCommand> = dl
        .iter()
        .filter(|cmd| matches!(cmd, DisplayCommand::DrawBoxShadow { .. }))
        .collect();
    assert_eq!(shadow_cmds.len(), 1);
    if let DisplayCommand::DrawBoxShadow {
        rect,
        offset_x,
        offset_y,
        blur_radius,
        spread_radius,
        ..
    } = shadow_cmds[0]
    {
        assert_eq!(rect.x(), 50.0);
        assert_eq!(rect.y(), 50.0);
        assert_eq!(*offset_x, 4.0);
        assert_eq!(*offset_y, 8.0);
        assert_eq!(*blur_radius, 12.0);
        assert_eq!(*spread_radius, 2.0);
    } else {
        panic!("Expected DrawBoxShadow");
    }
}
