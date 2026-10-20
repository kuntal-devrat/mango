use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::DisplayCommand;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::super::*;

#[test]
fn test_display_list_text_features() {
    use mango_css::values::{Length, TextDecoration, TextDecorationThickness, TextEmphasisStyle};

    // 1. Text underline with custom offset and thickness emits DrawLine
    let mut style1 = ComputedStyle::default();
    style1.font_size = 20.0;
    style1.color = Color::rgb(0, 0, 255);
    style1.text_decoration = TextDecoration::Underline;
    style1.text_underline_offset = Length::Px(5.0);
    style1.text_decoration_thickness = TextDecorationThickness::Length(Length::Px(3.0));

    let mut text_box1 = LayoutBox::new(
        BoxType::TextNode("Underlined Text".to_string()),
        Some(style1.into()),
    );
    text_box1.dimensions.content = Rect::new(10.0, 20.0, 150.0, 24.0);

    let dl1 = build_display_list(&text_box1);

    let has_custom_underline = dl1.iter().any(|cmd| match cmd {
        DisplayCommand::DrawLine {
            x1,
            y1,
            x2,
            y2,
            color,
            thickness,
        } => {
            *x1 == 10.0
                && *x2 == 160.0
                && (*y1 - y2).abs() < 0.01
                && *thickness == 3.0
                && *color == Color::rgb(0, 0, 255)
        }
        _ => false,
    });
    assert!(
        has_custom_underline,
        "Custom text-underline-offset and thickness must emit DrawLine"
    );

    // 2. Text emphasis emits DrawText for mark characters with emphasis color
    let mut style2 = ComputedStyle::default();
    style2.font_size = 20.0;
    style2.color = Color::rgb(0, 0, 0);
    style2.text_emphasis_style = TextEmphasisStyle::FilledCircle;
    style2.text_emphasis_color = Some(Color::rgb(255, 0, 0));

    let mut text_box2 = LayoutBox::new(BoxType::TextNode("ABC".to_string()), Some(style2.into()));
    text_box2.dimensions.content = Rect::new(10.0, 50.0, 60.0, 24.0);

    let dl2 = build_display_list(&text_box2);

    let emphasis_marks: Vec<_> = dl2
        .iter()
        .filter_map(|cmd| match cmd {
            DisplayCommand::DrawText { text, color, .. } if text == "●" => Some(*color),
            _ => None,
        })
        .collect();

    assert_eq!(
        emphasis_marks.len(),
        3,
        "Each non-whitespace character must have an emphasis mark '●'"
    );
    assert_eq!(
        emphasis_marks[0],
        Color::rgb(255, 0, 0),
        "Emphasis mark must use text-emphasis-color"
    );
}

#[test]
fn test_form_controls_accent_color() {
    let accent = Color::rgb(220, 50, 100);

    // 1. Checkbox
    let mut cb_style = ComputedStyle::default();
    cb_style.accent_color = Some(accent);
    let mut cb_box = LayoutBox::new(BoxType::InlineBlock, Some(cb_style.into()));
    cb_box.tag_name = Some("input".to_string());
    cb_box
        .attributes
        .push(("type".to_string(), "checkbox".to_string()));
    cb_box
        .attributes
        .push(("checked".to_string(), "true".to_string()));
    cb_box.dimensions.content = Rect::new(0.0, 0.0, 16.0, 16.0);
    let cb_dl = build_display_list(&cb_box);
    let cb_has_accent = cb_dl.iter().any(
        |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
    );
    assert!(cb_has_accent, "Checked checkbox should use accent-color");

    // 2. Radio
    let mut r_style = ComputedStyle::default();
    r_style.accent_color = Some(accent);
    let mut r_box = LayoutBox::new(BoxType::InlineBlock, Some(r_style.into()));
    r_box.tag_name = Some("input".to_string());
    r_box
        .attributes
        .push(("type".to_string(), "radio".to_string()));
    r_box
        .attributes
        .push(("checked".to_string(), "true".to_string()));
    r_box.dimensions.content = Rect::new(0.0, 0.0, 16.0, 16.0);
    let r_dl = build_display_list(&r_box);
    let r_has_accent = r_dl.iter().any(
        |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
    );
    assert!(r_has_accent, "Checked radio should use accent-color");

    // 3. Range
    let mut rng_style = ComputedStyle::default();
    rng_style.accent_color = Some(accent);
    let mut rng_box = LayoutBox::new(BoxType::InlineBlock, Some(rng_style.into()));
    rng_box.tag_name = Some("input".to_string());
    rng_box
        .attributes
        .push(("type".to_string(), "range".to_string()));
    rng_box
        .attributes
        .push(("value".to_string(), "50".to_string()));
    rng_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 20.0);
    let rng_dl = build_display_list(&rng_box);
    let rng_has_accent = rng_dl.iter().any(
        |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
    );
    assert!(rng_has_accent, "Range slider should use accent-color");

    // 4. Progress
    let mut prog_style = ComputedStyle::default();
    prog_style.accent_color = Some(accent);
    let mut prog_box = LayoutBox::new(BoxType::InlineBlock, Some(prog_style.into()));
    prog_box.tag_name = Some("progress".to_string());
    prog_box
        .attributes
        .push(("value".to_string(), "50".to_string()));
    prog_box
        .attributes
        .push(("max".to_string(), "100".to_string()));
    prog_box.dimensions.content = Rect::new(0.0, 0.0, 160.0, 16.0);
    let prog_dl = build_display_list(&prog_box);
    let prog_has_accent = prog_dl.iter().any(
        |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
    );
    assert!(prog_has_accent, "Progress bar should use accent-color");
}

#[test]
fn test_object_fit_contain_cover_none_scale_down() {
    use mango_css::values::ObjectFit;

    // Helper to create a replaced element img box with given object-fit
    let make_img = |fit: ObjectFit, box_w: f32, box_h: f32, iw: f32, ih: f32| -> LayoutBox {
        let mut style = ComputedStyle::default();
        style.object_fit = fit;
        // Default object-position is 50% 50% (centered)
        let mut b = LayoutBox::new(
            BoxType::ReplacedElement {
                intrinsic_width: iw,
                intrinsic_height: ih,
                pixels: vec![0xFFFFFF; (iw as u32 * ih as u32) as usize],
            },
            Some(style.into()),
        );
        b.dimensions.content = Rect::new(10.0, 20.0, box_w, box_h);
        b
    };

    // Intrinsic 200x100 (2:1 ratio) in a 100x100 box
    // Contain: fit to width → rendered 100x50, centered → offset_y = 25
    let img_contain = make_img(ObjectFit::Contain, 100.0, 100.0, 200.0, 100.0);
    let dl = build_display_list(&img_contain);
    let draw_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    assert!(draw_cmd.is_some(), "Contain should emit DrawImage");
    if let Some(DisplayCommand::DrawImage {
        x,
        y,
        width,
        height,
        ..
    }) = draw_cmd
    {
        assert!(
            (width - 100.0).abs() < 1.0,
            "contain: width should be 100, got {width}"
        );
        assert!(
            (height - 50.0).abs() < 1.0,
            "contain: height should be 50, got {height}"
        );
        // Centered: y offset = 20 + (100 - 50) * 0.5 = 45
        assert!((y - 45.0).abs() < 1.0, "contain: y should be 45, got {y}");
        assert!((x - 10.0).abs() < 1.0, "contain: x should be 10, got {x}");
    }

    // Cover: 200x100 in 100x100 → fit to height → rendered 200x100, centered → offset_x = -50
    let img_cover = make_img(ObjectFit::Cover, 100.0, 100.0, 200.0, 100.0);
    let dl = build_display_list(&img_cover);
    // Cover should emit PushClip before DrawImage
    let has_clip = dl
        .iter()
        .any(|c| matches!(c, DisplayCommand::PushClip { .. }));
    assert!(has_clip, "cover should emit PushClip");
    let draw_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage {
        x,
        y: _,
        width,
        height,
        ..
    }) = draw_cmd
    {
        assert!(
            (width - 200.0).abs() < 1.0,
            "cover: width should be 200, got {width}"
        );
        assert!(
            (height - 100.0).abs() < 1.0,
            "cover: height should be 100, got {height}"
        );
        // Centered: x offset = 10 + (100 - 200) * 0.5 = -40
        assert!((x - (-40.0)).abs() < 1.0, "cover: x should be -40, got {x}");
    }

    // None: use intrinsic size 200x100 in 100x100 box
    let img_none = make_img(ObjectFit::None, 100.0, 100.0, 200.0, 100.0);
    let dl = build_display_list(&img_none);
    let draw_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage { width, height, .. }) = draw_cmd {
        assert!(
            (width - 200.0).abs() < 1.0,
            "none: width should be 200, got {width}"
        );
        assert!(
            (height - 100.0).abs() < 1.0,
            "none: height should be 100, got {height}"
        );
    }

    // Scale-down: intrinsic 50x25 in 100x100 → contain would produce 100x50 > intrinsic → use none (50x25)
    let img_sd = make_img(ObjectFit::ScaleDown, 100.0, 100.0, 50.0, 25.0);
    let dl = build_display_list(&img_sd);
    let draw_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage { width, height, .. }) = draw_cmd {
        assert!(
            (width - 50.0).abs() < 1.0,
            "scale-down (none path): width should be 50, got {width}"
        );
        assert!(
            (height - 25.0).abs() < 1.0,
            "scale-down (none path): height should be 25, got {height}"
        );
    }

    // Scale-down: intrinsic 400x200 in 100x100 → contain produces 100x50 < intrinsic → use contain
    let img_sd2 = make_img(ObjectFit::ScaleDown, 100.0, 100.0, 400.0, 200.0);
    let dl = build_display_list(&img_sd2);
    let draw_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage { width, height, .. }) = draw_cmd {
        assert!(
            (width - 100.0).abs() < 1.0,
            "scale-down (contain path): width should be 100, got {width}"
        );
        assert!(
            (height - 50.0).abs() < 1.0,
            "scale-down (contain path): height should be 50, got {height}"
        );
    }

    // Fill: should use exact box dimensions
    let img_fill = make_img(ObjectFit::Fill, 100.0, 100.0, 200.0, 100.0);
    let dl = build_display_list(&img_fill);
    let draw_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
    if let Some(DisplayCommand::DrawImage {
        x,
        y,
        width,
        height,
        ..
    }) = draw_cmd
    {
        assert!(
            (width - 100.0).abs() < 1.0,
            "fill: width should be 100, got {width}"
        );
        assert!(
            (height - 100.0).abs() < 1.0,
            "fill: height should be 100, got {height}"
        );
        assert!((x - 10.0).abs() < 1.0, "fill: x should be 10, got {x}");
        assert!((y - 20.0).abs() < 1.0, "fill: y should be 20, got {y}");
    }
}

#[test]
fn test_multicolor_borders_split_per_side() {
    let mut style = ComputedStyle::default();
    style.border_top_width = 2.0;
    style.border_bottom_width = 4.0;
    style.border_top_color = Color::RED;
    style.border_bottom_color = Color::BLUE;
    style.border_top_style = mango_css::values::BorderStyle::Solid;
    style.border_bottom_style = mango_css::values::BorderStyle::Solid;

    let mut b = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    b.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);
    b.dimensions.border.top = 2.0;
    b.dimensions.border.bottom = 4.0;

    let dl = build_display_list(&b);
    let border_cmds: Vec<_> = dl
        .iter()
        .filter(|c| matches!(c, DisplayCommand::DrawBorder { .. }))
        .collect();
    assert_eq!(
        border_cmds.len(),
        2,
        "Should emit 2 DrawBorder commands for distinct side colors"
    );
    if let DisplayCommand::DrawBorder { color, widths, .. } = border_cmds[0] {
        assert_eq!(*color, Color::RED);
        assert_eq!(widths.top, 2.0);
        assert_eq!(widths.bottom, 0.0);
    }
    if let DisplayCommand::DrawBorder { color, widths, .. } = border_cmds[1] {
        assert_eq!(*color, Color::BLUE);
        assert_eq!(widths.top, 0.0);
        assert_eq!(widths.bottom, 4.0);
    }
}

#[test]
fn test_scrollbar_width_none_and_custom_color() {
    use mango_css::values::{Overflow, ScrollbarWidth};

    let make_container =
        |sb_width: ScrollbarWidth, sb_color: Option<(Color, Color)>| -> LayoutBox {
            let mut style = ComputedStyle::default();
            style.overflow_y = Overflow::Scroll;
            style.scrollbar_width = sb_width;
            style.scrollbar_color = sb_color;
            let mut b = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
            b.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);
            b.dimensions.padding.bottom = 0.0;
            // Add a child taller than the container to ensure scrollable extent
            let mut child = LayoutBox::new(BoxType::BlockNode, None);
            child.dimensions.content = Rect::new(0.0, 0.0, 100.0, 300.0);
            b.children.push(child);
            b
        };

    // None: no scrollbars
    let b_none = make_container(ScrollbarWidth::None, None);
    let dl_none = build_display_list(&b_none);
    let has_scrollbar = dl_none
        .iter()
        .any(|c| matches!(c, DisplayCommand::FillRoundedRect { .. }));
    assert!(
        !has_scrollbar,
        "scrollbar-width: none must suppress scrollbar"
    );

    // Custom colors: thumb = red, track = blue
    let b_custom = make_container(ScrollbarWidth::Auto, Some((Color::RED, Color::BLUE)));
    let dl_custom = build_display_list(&b_custom);
    let track_cmd = dl_custom
        .iter()
        .find(|c| matches!(c, DisplayCommand::FillRect { color, .. } if *color == Color::BLUE));
    assert!(
        track_cmd.is_some(),
        "Custom track color (blue) should be emitted"
    );
    let thumb_cmd = dl_custom.iter().find(
        |c| matches!(c, DisplayCommand::FillRoundedRect { color, .. } if *color == Color::RED),
    );
    assert!(
        thumb_cmd.is_some(),
        "Custom thumb color (red) should be emitted"
    );

    // Thin: track_thickness is 4.0
    let b_thin = make_container(ScrollbarWidth::Thin, None);
    let dl_thin = build_display_list(&b_thin);
    let thin_track = dl_thin.iter().find_map(|c| {
        if let DisplayCommand::FillRect { rect, .. } = c {
            if rect.width() == 4.0 {
                Some(rect.width())
            } else {
                None
            }
        } else {
            None
        }
    });
    assert_eq!(
        thin_track,
        Some(4.0),
        "scrollbar-width: thin should use 4px track width"
    );
}

#[test]
fn test_fixed_element_trapped_by_filter_or_contain() {
    use mango_css::values::{Contain, Overflow, Position};

    // Container with overflow: hidden and contain: paint
    let mut parent_style = ComputedStyle::default();
    parent_style.overflow_x = Overflow::Hidden;
    parent_style.overflow_y = Overflow::Hidden;
    parent_style.contain = Contain::Paint;
    let mut parent = LayoutBox::new(BoxType::BlockNode, Some(parent_style.into()));
    parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 200.0);

    let mut fixed_style = ComputedStyle::default();
    fixed_style.position = Position::Fixed;
    let mut fixed_child = LayoutBox::new(BoxType::BlockNode, Some(fixed_style.into()));
    fixed_child.dimensions.content = Rect::new(10.0, 10.0, 50.0, 50.0);
    parent.children.push(fixed_child);

    let dl = build_display_list(&parent);
    // Under contain: paint, the fixed child is trapped inside parent's clip.
    // There should be exactly 1 PushClip and 1 PopClip for the container, not extra PopClip/PushClip around child.
    let push_clips = dl
        .iter()
        .filter(|c| matches!(c, DisplayCommand::PushClip { .. }))
        .count();
    let pop_clips = dl
        .iter()
        .filter(|c| matches!(c, DisplayCommand::PopClip))
        .count();
    assert_eq!(
        push_clips, 1,
        "Child must remain clipped; exactly 1 PushClip"
    );
    assert_eq!(pop_clips, 1, "Child must remain clipped; exactly 1 PopClip");
}

#[test]
fn test_stacking_context_floats_and_backdrop_filter() {
    use mango_css::values::{Float, Position};

    let mut parent = LayoutBox::new(BoxType::BlockNode, None);
    parent.dimensions.content = Rect::new(0.0, 0.0, 400.0, 400.0);

    // Child 1: Positioned with z-index: auto (ZeroOrAutoZ)
    let mut pos_style = ComputedStyle::default();
    pos_style.position = Position::Relative;
    pos_style.background_color = Color::RED;
    let mut pos_child = LayoutBox::new(BoxType::BlockNode, Some(pos_style.into()));
    pos_child.dimensions.content = Rect::new(0.0, 0.0, 50.0, 50.0);

    // Child 2: Static Float (Float layer)
    let mut float_style = ComputedStyle::default();
    float_style.position = Position::Static;
    float_style.float = Float::Left;
    float_style.background_color = Color::BLUE;
    let mut float_child = LayoutBox::new(BoxType::BlockNode, Some(float_style.into()));
    float_child.dimensions.content = Rect::new(50.0, 0.0, 50.0, 50.0);

    // Child 3: In-flow normal box (NormalFlow layer)
    let mut normal_style = ComputedStyle::default();
    normal_style.background_color = Color::GREEN;
    let mut normal_child = LayoutBox::new(BoxType::BlockNode, Some(normal_style.into()));
    normal_child.dimensions.content = Rect::new(100.0, 0.0, 50.0, 50.0);

    // Add in reverse order: Pos, Float, Normal
    parent.children.push(pos_child);
    parent.children.push(float_child);
    parent.children.push(normal_child);

    let dl = build_display_list(&parent);
    // In CSS 2.1 App E:
    // NormalFlow (GREEN) < Float (BLUE) < Positioned ZeroOrAutoZ (RED)
    let green_pos = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::FillRect { color, .. } if *color == Color::GREEN))
        .expect("green rendered");
    let blue_pos = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::FillRect { color, .. } if *color == Color::BLUE))
        .expect("blue rendered");
    let red_pos = dl
        .iter()
        .position(|c| matches!(c, DisplayCommand::FillRect { color, .. } if *color == Color::RED))
        .expect("red rendered");

    assert!(
        green_pos < blue_pos,
        "NormalFlow must paint before Float: green={green_pos}, blue={blue_pos}"
    );
    assert!(
        blue_pos < red_pos,
        "Float must paint before Positioned: blue={blue_pos}, red={red_pos}"
    );
}

#[test]
fn test_background_clip_text_emits_gradient_text() {
    use mango_css::values::{BackgroundClip, ColorStop, Gradient};

    let mut style = ComputedStyle::default();
    style.background_clip = BackgroundClip::Text;
    style.color = Color::BLACK;
    style.font_size = 20.0;
    style.background_gradient = Some(Gradient::Linear {
        angle_deg: 90.0,
        stops: vec![
            ColorStop::new(Color::RED, None, None),
            ColorStop::new(Color::BLUE, None, None),
        ],
        repeating: false,
    });

    let mut text_box = LayoutBox::new(
        BoxType::TextNode("Hello Gradient".to_string()),
        Some(style.into()),
    );
    text_box.dimensions.content = Rect::new(10.0, 10.0, 100.0, 20.0);

    let dl = build_display_list(&text_box);
    let grad_text = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawTextWithGradient { .. }));
    assert!(
        grad_text.is_some(),
        "background-clip: text with gradient must emit DrawTextWithGradient"
    );
    if let Some(DisplayCommand::DrawTextWithGradient { text, .. }) = grad_text {
        assert_eq!(text, "Hello Gradient");
    }
}

#[test]
fn test_border_image_display_command() {
    use mango_css::values::{BorderImage, BorderImageRepeat, Length};
    use mango_render::image_decode::{DecodedImage, cache_image};

    let dummy_img = DecodedImage {
        width: 30,
        height: 30,
        pixels: vec![0xFF00FF00; 30 * 30],
    };
    cache_image("border.png", dummy_img);

    let mut style = ComputedStyle::default();
    style.border_image = Some(BorderImage {
        source: Some("border.png".to_string()),
        gradient: None,
        slice: [
            Length::Px(10.0),
            Length::Px(10.0),
            Length::Px(10.0),
            Length::Px(10.0),
        ],
        width: [Length::Px(5.0); 4],
        outset: [Length::Px(0.0); 4],
        repeat_h: BorderImageRepeat::Stretch,
        repeat_v: BorderImageRepeat::Stretch,
        fill: false,
    });
    let mut b = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    b.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);
    b.dimensions.border.top = 5.0;
    b.dimensions.border.right = 5.0;
    b.dimensions.border.bottom = 5.0;
    b.dimensions.border.left = 5.0;

    let dl = build_display_list(&b);
    let bi_cmd = dl
        .iter()
        .find(|c| matches!(c, DisplayCommand::DrawBorderImage { .. }));
    assert!(
        bi_cmd.is_some(),
        "DrawBorderImage must be emitted when border-image source is present"
    );
    if let Some(DisplayCommand::DrawBorderImage { slice, widths, .. }) = bi_cmd {
        assert_eq!(*slice, [10.0, 10.0, 10.0, 10.0]);
        assert_eq!(widths.top, 5.0);
    }
}

#[test]
fn test_build_display_list_with_scroll_xy() {
    use mango_css::values::Position;

    let mut root = LayoutBox::new(BoxType::BlockNode, None);
    root.dimensions.content = Rect::new(0.0, 0.0, 500.0, 500.0);

    // Child with position: fixed
    let mut fixed_style = ComputedStyle::default();
    fixed_style.position = Position::Fixed;
    fixed_style.background_color = Color::RED;
    let mut fixed_child = LayoutBox::new(BoxType::BlockNode, Some(fixed_style.into()));
    fixed_child.dimensions.content = Rect::new(20.0, 30.0, 50.0, 50.0);
    root.children.push(fixed_child);

    // In-flow child
    let mut inflow_style = ComputedStyle::default();
    inflow_style.background_color = Color::BLUE;
    let mut inflow_child = LayoutBox::new(BoxType::BlockNode, Some(inflow_style.into()));
    inflow_child.dimensions.content = Rect::new(20.0, 30.0, 50.0, 50.0);
    root.children.push(inflow_child);

    // When scroll_x = 100.0 and scroll_y = 200.0:
    // Fixed child moves with the viewport (+100.0, +200.0) -> x=120, y=230
    // In-flow child stays at document coordinates (x=20, y=30)
    let dl = build_display_list_with_scroll_xy(&root, 100.0, 200.0);

    let fixed_rect = dl
        .iter()
        .find_map(|c| {
            if let DisplayCommand::FillRect { rect, color } = c {
                if *color == Color::RED {
                    Some(*rect)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("fixed child FillRect");

    let inflow_rect = dl
        .iter()
        .find_map(|c| {
            if let DisplayCommand::FillRect { rect, color } = c {
                if *color == Color::BLUE {
                    Some(*rect)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("inflow child FillRect");

    assert_eq!(fixed_rect.x(), 120.0);
    assert_eq!(fixed_rect.y(), 230.0);
    assert_eq!(inflow_rect.x(), 20.0);
    assert_eq!(inflow_rect.y(), 30.0);
}
