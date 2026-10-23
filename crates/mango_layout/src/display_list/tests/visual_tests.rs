use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::super::*;

#[test]
fn test_outline_display_command() {
    let mut style = ComputedStyle::default();
    style.outline_width = 2.0;
    style.outline_style = mango_css::values::BorderStyle::Solid;
    style.outline_color = Color::RED;
    style.outline_offset = 3.0;

    let mut box_node = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    box_node.dimensions.content = Rect::new(50.0, 50.0, 200.0, 100.0);

    let dl = build_display_list(&box_node);
    let border_cmds: Vec<&DisplayCommand> = dl
        .iter()
        .filter(|cmd| matches!(cmd, DisplayCommand::DrawBorder { .. }))
        .collect();
    assert_eq!(
        border_cmds.len(),
        1,
        "Should emit 1 DrawBorder command for the outline"
    );
    if let DisplayCommand::DrawBorder {
        rect,
        color,
        widths,
        ..
    } = border_cmds[0]
    {
        assert_eq!(*color, Color::RED);
        assert_eq!(rect.x(), 45.0);
        assert_eq!(rect.y(), 45.0);
        assert_eq!(rect.width(), 210.0);
        assert_eq!(rect.height(), 110.0);
        assert_eq!(widths.top, 2.0);
        assert_eq!(widths.right, 2.0);
        assert_eq!(widths.bottom, 2.0);
        assert_eq!(widths.left, 2.0);
    } else {
        panic!("Expected DrawBorder");
    }
}

#[test]
fn test_iframe_display_commands() {
    let mut iframe_box = LayoutBox::new(
        BoxType::IFrame {
            src: "https://example.com/embed".to_string(),
            srcdoc: None,
            sandbox: crate::box_model::IFrameSandbox::from_attribute(Some("allow-scripts")),
            intrinsic_width: 300.0,
            intrinsic_height: 150.0,
        },
        None,
    );
    iframe_box.dimensions.content = Rect::new(20.0, 30.0, 300.0, 150.0);

    let dl = build_display_list(&iframe_box);

    // Verify white viewport fill is rendered
    let has_white_fill = dl.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, rect } => {
            *color == Color::WHITE && rect.width() == 300.0 && rect.height() == 150.0
        }
        _ => false,
    });
    assert!(
        has_white_fill,
        "IFrame should paint white viewport background"
    );

    // Verify embed card frame title is drawn
    let has_frame_title = dl.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("example.com/embed"),
        _ => false,
    });
    assert!(
        has_frame_title,
        "IFrame placeholder card should display source URL"
    );

    // Verify sandbox badge is drawn
    let has_sandbox_badge = dl.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("Sandboxed"),
        _ => false,
    });
    assert!(has_sandbox_badge, "IFrame should display sandbox badge");
}

#[test]
fn test_root_and_body_overflow_clip_exemption() {
    let mut style = ComputedStyle::default();
    style.overflow_x = mango_css::values::Overflow::Hidden;
    style.overflow_y = mango_css::values::Overflow::Visible;

    // 1. Root element (html) with overflow-x: hidden should NOT emit PushClip
    let mut html_box = LayoutBox::new(BoxType::BlockNode, Some(style.clone().into()));
    html_box.tag_name = Some("html".to_string());
    html_box.dimensions.content = Rect::new(0.0, 0.0, 1280.0, 800.0);

    let dl_html = build_display_list(&html_box);
    let has_clip_html = dl_html
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
    assert!(
        !has_clip_html,
        "html element must not emit PushClip for overflow"
    );

    // 2. Body element with overflow-x: hidden should NOT emit PushClip
    let mut body_box = LayoutBox::new(BoxType::BlockNode, Some(style.clone().into()));
    body_box.tag_name = Some("body".to_string());
    body_box.dimensions.content = Rect::new(0.0, 0.0, 1280.0, 800.0);

    let dl_body = build_display_list(&body_box);
    let has_clip_body = dl_body
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
    assert!(
        !has_clip_body,
        "body element must not emit PushClip for overflow"
    );

    // 3. Normal container with overflow-x: hidden and overflow-y: visible should have unconstrained vertical clip
    let mut div_box = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    div_box.tag_name = Some("div".to_string());
    div_box.dimensions.content = Rect::new(10.0, 10.0, 500.0, 300.0);

    let dl_div = build_display_list(&div_box);
    let clip_cmd = dl_div
        .iter()
        .find(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
    assert!(clip_cmd.is_some(), "div container should emit PushClip");
    if let Some(DisplayCommand::PushClip { rect }) = clip_cmd {
        assert_eq!(rect.x(), 10.0);
        assert_eq!(rect.width(), 500.0);
        // Vertical extent should be unconstrained so vertical scrolling content isn't clipped
        assert!(
            rect.height() > 10000.0,
            "Vertical extent should be unconstrained for overflow-y: visible"
        );
    }
}

#[test]
fn test_video_and_audio_display_commands() {
    // 1. Video with controls and poster
    let mut video_box = LayoutBox::new(
        BoxType::Video {
            src: "https://example.com/demo.mp4".to_string(),
            poster: Some("data:image/png;base64,demo".to_string()),
            has_controls: true,
            autoplay: false,
            is_loop: false,
            is_muted: false,
            is_playing: false,
            current_time: 15.0,
            duration: 90.0,
            intrinsic_width: 640.0,
            intrinsic_height: 360.0,
            poster_pixels: Some(vec![0xFF00FF; 100]),
            poster_width: 10,
            poster_height: 10,
        },
        None,
    );
    video_box.dimensions.content = Rect::new(0.0, 0.0, 640.0, 360.0);

    let dl_video = build_display_list(&video_box);

    // Verify dark video background
    let has_dark_bg = dl_video.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, rect } => {
            *color == Color::rgb(51, 51, 51) && rect.width() == 640.0
        }
        _ => false,
    });
    assert!(has_dark_bg, "Video should paint dark cinematic background");

    // Verify poster image was painted
    let has_poster = dl_video.iter().any(|cmd| match cmd {
        DisplayCommand::DrawImage { width, height, .. } => *width == 640.0 && *height == 360.0,
        _ => false,
    });
    assert!(has_poster, "Video should paint decoded poster image");

    // Verify controls bar elements (play icon, time, progress track)
    let has_play_btn = dl_video.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains('▶'),
        _ => false,
    });
    assert!(has_play_btn, "Video controls bar should paint play glyph");

    let has_time_text = dl_video.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("0:15 / 1:30"),
        _ => false,
    });
    assert!(
        has_time_text,
        "Video controls bar should display formatted elapsed and duration timestamps"
    );

    // 2. Audio with controls
    let mut audio_box = LayoutBox::new(
        BoxType::Audio {
            src: "https://example.com/podcast.mp3".to_string(),
            has_controls: true,
            autoplay: false,
            is_loop: false,
            is_muted: false,
            is_playing: true,
            current_time: 45.0,
            duration: 180.0,
            intrinsic_width: 300.0,
            intrinsic_height: 36.0,
        },
        None,
    );
    audio_box.dimensions.content = Rect::new(50.0, 50.0, 300.0, 36.0);

    let dl_audio = build_display_list(&audio_box);

    // Verify audio rounded player pill
    let has_audio_card = dl_audio.iter().any(|cmd| match cmd {
        DisplayCommand::FillRoundedRect { color, rect, .. } => {
            *color == Color::rgb(30, 41, 59) && rect.width() == 300.0
        }
        _ => false,
    });
    assert!(
        has_audio_card,
        "Audio should render dark slate rounded container"
    );

    // Verify pause glyph when playing
    let has_pause_btn = dl_audio.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("❚❚"),
        _ => false,
    });
    assert!(has_pause_btn, "Playing audio should render pause glyph");

    // Verify timestamp
    let has_audio_time = dl_audio.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("0:45 / 3:00"),
        _ => false,
    });
    assert!(has_audio_time, "Audio player should format timestamp");
}

#[test]
fn test_canvas_display_list_rendering_and_fallback_suppression() {
    // 1. Canvas with pixel buffer
    let pixels = vec![0xFFFF_0000; 100 * 50]; // 100x50 red pixels
    let mut canvas_box = LayoutBox::new(
        BoxType::Canvas {
            canvas_key: Some(999),
            width: 100,
            height: 50,
            intrinsic_width: 100.0,
            intrinsic_height: 50.0,
            pixels: Some(pixels),
        },
        None,
    );
    canvas_box.tag_name = Some("canvas".to_string());
    canvas_box.dimensions.content = Rect::new(20.0, 30.0, 100.0, 50.0);

    // Add a fallback child (e.g. <p>Your browser does not support canvas</p>)
    let mut fallback_child = LayoutBox::new(
        BoxType::TextNode("Your browser does not support canvas".to_string()),
        None,
    );
    fallback_child.dimensions.content = Rect::new(20.0, 30.0, 200.0, 20.0);
    canvas_box.children.push(fallback_child);

    let dl = build_display_list(&canvas_box);

    // Verify DrawImage command was emitted with canvas pixels
    let has_draw_image = dl.iter().any(|cmd| match cmd {
        DisplayCommand::DrawImage {
            x,
            y,
            width,
            height,
            pixels,
        } => {
            *x == 20.0
                && *y == 30.0
                && *width == 100.0
                && *height == 50.0
                && pixels.len() == 100 * 50
        }
        _ => false,
    });
    assert!(
        has_draw_image,
        "Canvas should emit DrawImage command with pixel data"
    );

    // Verify fallback child text is NOT drawn
    let has_fallback_text = dl.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("does not support canvas"),
        _ => false,
    });
    assert!(
        !has_fallback_text,
        "Canvas fallback text must not be rendered"
    );
}

#[test]
fn test_display_list_cache() {
    let mut root = LayoutBox::new(BoxType::BlockNode, None);
    root.dimensions.content = Rect::new(0.0, 0.0, 200.0, 100.0);
    let mut cache = DisplayListCache::new();

    assert!(!cache.is_cached());
    let dl1 = cache.get_or_build(&root, 0.0);
    assert!(cache.is_cached());

    // Mark clean and get again with same scroll_y: must return cached
    root.mark_clean();
    let dl2 = cache.get_or_build(&root, 0.0);
    assert_eq!(dl1.len(), dl2.len());

    // Invalidate dirty rect
    cache.invalidate(Some(Rect::new(10.0, 10.0, 50.0, 50.0)));
    assert!(!cache.is_cached());
    assert_eq!(cache.dirty_regions().len(), 1);

    let dl3 = cache.get_or_build(&root, 0.0);
    assert!(cache.is_cached());
    assert_eq!(cache.dirty_regions().len(), 0);
    assert_eq!(dl1.len(), dl3.len());
}

#[test]
fn test_display_list_cache_update_and_diff() {
    let mut cache = DisplayListCache::new();
    let mut dl1 = DisplayList::new();
    dl1.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        color: Color::RED,
    });
    dl1.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 100.0, 100.0, 100.0),
        color: Color::BLUE,
    });

    // Initial diff with empty cache
    let diff1 = cache.update_and_diff(dl1.clone(), 0.0);
    assert!(!diff1.is_identical);
    assert!(cache.is_cached());

    // Identical update: no dirty rects
    let diff2 = cache.update_and_diff(dl1.clone(), 0.0);
    assert!(diff2.is_identical);
    assert_eq!(diff2.damage_rect(), None);

    // Modified update: only changed command is damaged
    let mut dl2 = dl1.clone();
    dl2.push(DisplayCommand::FillRect {
        rect: Rect::new(50.0, 50.0, 20.0, 20.0),
        color: Color::GREEN,
    });
    let diff3 = cache.update_and_diff(dl2, 0.0);
    assert!(!diff3.is_identical);
    assert_eq!(diff3.damage_rect(), Some(Rect::new(50.0, 50.0, 20.0, 20.0)));
    assert!(
        cache
            .dirty_regions()
            .contains(&Rect::new(50.0, 50.0, 20.0, 20.0))
    );
}

#[test]
fn test_modal_dialog_backdrop() {
    let mut dialog_box = LayoutBox::new(BoxType::BlockNode, Some(ComputedStyle::default().into()));
    dialog_box.tag_name = Some("dialog".to_string());
    dialog_box
        .attributes
        .push(("open".to_string(), "".to_string()));
    dialog_box
        .attributes
        .push(("data-mango-modal".to_string(), "true".to_string()));
    dialog_box.dimensions.content = Rect::new(100.0, 100.0, 200.0, 100.0);

    let dl = build_display_list(&dialog_box);

    // Verify dimming backdrop command is emitted
    let has_backdrop = dl.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, rect } => {
            *color == Color::rgba(0, 0, 0, 102) && rect.width() >= 10_000.0
        }
        _ => false,
    });
    assert!(
        has_backdrop,
        "Modal dialog must render dimming backdrop covering viewport"
    );
}

#[test]
fn test_progress_and_meter_rendering() {
    // 1. Determinate progress
    let mut progress_box =
        LayoutBox::new(BoxType::InlineBlock, Some(ComputedStyle::default().into()));
    progress_box.tag_name = Some("progress".to_string());
    progress_box
        .attributes
        .push(("value".to_string(), "50".to_string()));
    progress_box
        .attributes
        .push(("max".to_string(), "100".to_string()));
    progress_box.dimensions.content = Rect::new(10.0, 10.0, 160.0, 16.0);

    let dl_progress = build_display_list(&progress_box);
    let has_fill = dl_progress.iter().any(|cmd| match cmd {
        DisplayCommand::FillRoundedRect { rect, color, .. } => {
            rect.width() == 80.0 && *color == Color::rgb(0, 120, 215)
        }
        _ => false,
    });
    assert!(
        has_fill,
        "Progress value 50/100 must render 50% fill (80px)"
    );

    // 2. Meter (optimal zone -> green)
    let mut meter_box = LayoutBox::new(BoxType::InlineBlock, Some(ComputedStyle::default().into()));
    meter_box.tag_name = Some("meter".to_string());
    meter_box
        .attributes
        .push(("min".to_string(), "0".to_string()));
    meter_box
        .attributes
        .push(("max".to_string(), "100".to_string()));
    meter_box
        .attributes
        .push(("low".to_string(), "33".to_string()));
    meter_box
        .attributes
        .push(("high".to_string(), "66".to_string()));
    meter_box
        .attributes
        .push(("optimum".to_string(), "80".to_string()));
    meter_box
        .attributes
        .push(("value".to_string(), "75".to_string()));
    meter_box.dimensions.content = Rect::new(10.0, 10.0, 100.0, 16.0);

    let dl_meter = build_display_list(&meter_box);
    let has_green_meter = dl_meter.iter().any(|cmd| match cmd {
        DisplayCommand::FillRoundedRect { rect, color, .. } => {
            rect.width() == 75.0 && *color == Color::rgb(40, 167, 69)
        }
        _ => false,
    });
    assert!(
        has_green_meter,
        "Optimal meter must render green fill of 75px"
    );
}

#[test]
fn test_select_optgroup_and_multiple() {
    // Multi-select rendering
    let mut select_box =
        LayoutBox::new(BoxType::InlineBlock, Some(ComputedStyle::default().into()));
    select_box.tag_name = Some("select".to_string());
    select_box
        .attributes
        .push(("_mango_is_multiple".to_string(), "true".to_string()));
    select_box.attributes.push((
        "_mango_selected_text".to_string(),
        "Apple, Cherry".to_string(),
    ));
    select_box.dimensions.content = Rect::new(10.0, 10.0, 150.0, 60.0);

    let dl = build_display_list(&select_box);
    let texts: Vec<String> = dl
        .iter()
        .filter_map(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();

    assert!(
        texts.contains(&"Apple".to_string()),
        "Multi-select must render option 'Apple'"
    );
    assert!(
        texts.contains(&"Cherry".to_string()),
        "Multi-select must render option 'Cherry'"
    );
    assert!(
        !texts.contains(&"▼".to_string()),
        "Multi-select should not render drop-down arrow"
    );
}

#[test]
fn test_visual_effects_display_commands() {
    use mango_css::values::{BlendMode, ClipPath, FilterFunction, Length};

    let mut style = ComputedStyle::default();
    style.mix_blend_mode = BlendMode::Multiply;
    style.clip_path = ClipPath::Circle {
        radius: Length::Px(25.0),
        center_x: Length::Percent(50.0),
        center_y: Length::Percent(50.0),
    };
    style.filter = vec![FilterFunction::Blur(4.0)];
    style.backdrop_filter = vec![FilterFunction::Blur(8.0)];

    let mut b = LayoutBox::new(BoxType::BlockNode, Some(style.into()));
    b.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

    let dl = build_display_list(&b);

    let has_push_blend = dl.iter().any(|cmd| {
        matches!(
            cmd,
            DisplayCommand::PushBlendMode {
                mode: BlendMode::Multiply
            }
        )
    });
    let has_pop_blend = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PopBlendMode));
    let has_push_clip = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushClipPath { .. }));
    let has_pop_clip = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PopClipPath));
    let has_push_filter = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushFilter { .. }));
    let has_pop_filter = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PopFilter));
    let has_push_backdrop = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushBackdropFilter { .. }));

    assert!(has_push_blend, "PushBlendMode must be present");
    assert!(has_pop_blend, "PopBlendMode must be present");
    assert!(has_push_clip, "PushClipPath must be present");
    assert!(has_pop_clip, "PopClipPath must be present");
    assert!(has_push_filter, "PushFilter must be present");
    assert!(has_pop_filter, "PopFilter must be present");
    assert!(has_push_backdrop, "PushBackdropFilter must be present");
}
