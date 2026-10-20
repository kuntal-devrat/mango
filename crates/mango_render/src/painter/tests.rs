//! Unit tests for the painter rasterizer.

use mango_core::{Color, Rect};

use crate::display_list::{DisplayCommand, DisplayList};
use crate::painter::blend::blend_pixel;
use crate::painter::core::paint;
use crate::painter::shapes::fill_rounded_rect;

#[test]
fn test_blend_pixel() {
    let white = 0x00FFFFFF;
    let black_half = Color::rgba(0, 0, 0, 128);
    let blended = blend_pixel(black_half, white);
    let r = (blended >> 16) & 0xFF;
    let g = (blended >> 8) & 0xFF;
    let b = blended & 0xFF;
    // ~127
    assert!((120..=135).contains(&r));
    assert!((120..=135).contains(&g));
    assert!((120..=135).contains(&b));
}

#[test]
fn test_fill_rounded_rect_corners() {
    let mut buffer = vec![0u32; 100 * 100];
    let rect = Rect::new(10.0, 10.0, 80.0, 80.0);
    let radii = [20.0, 20.0, 20.0, 20.0];
    let red = Color::rgb(255, 0, 0);

    fill_rounded_rect(
        &mut buffer,
        100,
        100,
        &rect,
        red,
        radii,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        mango_css::values::BlendMode::Normal,
    );

    // Center should definitely be red
    assert_eq!(buffer[50 * 100 + 50], 0x00FF0000);

    // Very top-left pixel (10, 10) of the bounding box should NOT be red due to radius 20
    assert_eq!(buffer[10 * 100 + 10], 0);

    // (30, 30) should be inside
    assert_eq!(buffer[30 * 100 + 30], 0x00FF0000);
}

#[test]
fn test_push_transform_moves_fill() {
    let mut buffer = vec![0u32; 60 * 60];
    let mut dl = DisplayList::new();
    // Draw a red 10x10 square, then the same square translated by (30, 30).
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        color: Color::RED,
    });
    dl.push(DisplayCommand::PushTransform {
        matrix: [1.0, 0.0, 0.0, 1.0, 30.0, 30.0],
    });
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        color: Color::GREEN,
    });
    dl.push(DisplayCommand::PopTransform);

    paint(&dl, &mut buffer, 60, 60);

    assert_eq!(
        buffer[5 * 60 + 5],
        0x00FF0000,
        "untransformed square stays red"
    );
    // CSS `green` is #008000, i.e. 0x00008000 in the framebuffer (`0xAARRGGBB` with legacy `0x00RRGGBB` opaque encoding).
    assert_eq!(
        buffer[35 * 60 + 35],
        0x00008000,
        "translated square lands at +30,+30"
    );
    assert_eq!(
        buffer[5 * 60 + 35],
        0,
        "translation does not smear horizontally"
    );
}

#[test]
fn test_push_transform_scales_fill() {
    let mut buffer = vec![0u32; 40 * 40];
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::PushTransform {
        matrix: [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
    });
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        color: Color::BLUE,
    });
    dl.push(DisplayCommand::PopTransform);

    paint(&dl, &mut buffer, 40, 40);

    assert_eq!(
        buffer[10 * 40 + 10],
        0x000000FF,
        "scaled fill covers the scaled area"
    );
    assert_eq!(
        buffer[25 * 40 + 25],
        0,
        "scaled fill stops at 2x the source rect"
    );
}

#[test]
fn test_fill_gradient_paints_stops() {
    let mut buffer = vec![0u32; 40 * 10];
    let gradient = mango_css::values::Gradient::Linear {
        angle_deg: 90.0, // to right
        stops: vec![
            mango_css::values::ColorStop::new(Color::RED, Some(0.0), None),
            mango_css::values::ColorStop::new(Color::BLUE, Some(1.0), None),
        ],
        repeating: false,
    };
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::FillGradient {
        rect: Rect::new(0.0, 0.0, 40.0, 10.0),
        gradient: Box::new(gradient),
        radii: [0.0; 4],
        opacity: 1.0,
    });
    paint(&dl, &mut buffer, 40, 10);

    let left = buffer[5 * 40 + 1];
    let right = buffer[5 * 40 + 38];
    assert!(
        (left >> 16) & 0xFF > 150,
        "left edge is mostly red, got {left:#08x}"
    );
    assert!(
        right & 0xFF > 150,
        "right edge is mostly blue, got {right:#08x}"
    );
}

#[test]
fn test_render_gradients_and_colors() {
    let mut buffer = vec![0u32; 50 * 50];

    // 1. Render conic gradient
    let conic = mango_css::values::Gradient::Conic {
        angle_deg: 45.0,
        stops: vec![
            mango_css::values::ColorStop::new(Color::rgb(255, 0, 0), Some(0.0), None),
            mango_css::values::ColorStop::new(Color::rgb(0, 255, 0), Some(0.5), None),
            mango_css::values::ColorStop::new(Color::rgb(0, 0, 255), Some(1.0), None),
        ],
        repeating: false,
    };
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::FillGradient {
        rect: Rect::new(0.0, 0.0, 50.0, 50.0),
        gradient: Box::new(conic),
        radii: [4.0; 4],
        opacity: 0.9,
    });
    paint(&dl, &mut buffer, 50, 50);
    assert!(buffer.iter().any(|&p| p != 0));

    // 2. Render repeating radial gradient
    let mut buf2 = vec![0u32; 60 * 60];
    let rep_rad = mango_css::values::Gradient::Radial {
        stops: vec![
            mango_css::values::ColorStop::new(Color::rgb(255, 255, 255), Some(0.0), None),
            mango_css::values::ColorStop::new(Color::rgb(0, 0, 0), Some(0.2), None),
        ],
        repeating: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: None,
        radius_y: None,
    };
    let mut dl2 = DisplayList::new();
    dl2.push(DisplayCommand::FillGradient {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        gradient: Box::new(rep_rad),
        radii: [0.0; 4],
        opacity: 1.0,
    });
    paint(&dl2, &mut buf2, 60, 60);
    assert!(buf2.iter().any(|&p| p != 0));
}

#[test]
fn test_clipping_rect_pipeline() {
    let mut buffer = vec![0u32; 100 * 100];
    let mut dl = DisplayList::new();
    // Clip to 20..80 in both X and Y
    dl.push(DisplayCommand::PushClip {
        rect: Rect::new(20.0, 20.0, 60.0, 60.0),
    });
    // Draw a giant red rectangle covering 0..100
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        color: Color::RED,
    });
    dl.push(DisplayCommand::PopClip);

    paint(&dl, &mut buffer, 100, 100);

    // Outside clip: pixel (10, 10) should be untouched (0)
    assert_eq!(buffer[10 * 100 + 10], 0);
    assert_eq!(buffer[90 * 100 + 90], 0);

    // Inside clip: pixel (30, 30) and (70, 70) should be red
    assert_eq!(buffer[30 * 100 + 30], 0x00FF0000);
    assert_eq!(buffer[70 * 100 + 70], 0x00FF0000);
}

#[test]
fn test_backdrop_filter_and_clip_path() {
    let mut buffer = vec![0x00FFFFFFu32; 100 * 100];
    let mut dl = DisplayList::new();
    // Invert backdrop filter over (20, 20, 40, 40)
    dl.push(DisplayCommand::PushBackdropFilter {
        filters: vec![mango_css::values::FilterFunction::Invert(1.0)],
        rect: Rect::new(20.0, 20.0, 40.0, 40.0),
    });
    paint(&dl, &mut buffer, 100, 100);

    // Outside filter: white (0x00FFFFFF)
    assert_eq!(buffer[10 * 100 + 10] & 0x00ffffff, 0x00ffffff);
    // Inside inverted filter: black (0x00000000)
    assert_eq!(buffer[30 * 100 + 30] & 0x00ffffff, 0x00000000);

    // Test clip path circle
    let mut dl2 = DisplayList::new();
    dl2.push(DisplayCommand::PushClipPath {
        clip_path: Box::new(mango_css::values::ClipPath::Circle {
            radius: mango_css::values::Length::Px(20.0),
            center_x: mango_css::values::Length::Px(50.0),
            center_y: mango_css::values::Length::Px(50.0),
        }),
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
    });
    dl2.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        color: Color::RED,
    });
    dl2.push(DisplayCommand::PopClipPath);

    let mut buffer2 = vec![0u32; 100 * 100];
    paint(&dl2, &mut buffer2, 100, 100);
    // Center (50, 50) is inside circle radius 20: RED
    assert_eq!(buffer2[50 * 100 + 50], 0x00FF0000);
    // (10, 10) is outside: untouched (0)
    assert_eq!(buffer2[10 * 100 + 10], 0);
}

#[test]
fn test_clip_path_with_intersected_subclip() {
    let mut buffer = vec![0x00FFFFFFu32; 100 * 100];
    let mut dl = DisplayList::new();
    // Outer clip restricts to (20, 20, 60, 60)
    dl.push(DisplayCommand::PushClip {
        rect: Rect::new(20.0, 20.0, 60.0, 60.0),
    });
    // Element rect is larger (0, 0, 100, 100)
    dl.push(DisplayCommand::PushClipPath {
        clip_path: Box::new(mango_css::values::ClipPath::Circle {
            radius: mango_css::values::Length::Px(15.0),
            center_x: mango_css::values::Length::Px(50.0),
            center_y: mango_css::values::Length::Px(50.0),
        }),
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
    });
    // Fill full rect with blue
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        color: Color::rgba(0, 0, 255, 255),
    });
    dl.push(DisplayCommand::PopClipPath);
    dl.push(DisplayCommand::PopClip);

    paint(&dl, &mut buffer, 100, 100);

    // Center (50, 50) is inside circle: Blue
    assert_eq!(buffer[50 * 100 + 50] & 0x00FFFFFF, 0x000000FF);
    // (30, 30) is inside outer clip (20..80) but outside circle: restored to white (0x00FFFFFF)
    assert_eq!(buffer[30 * 100 + 30] & 0x00FFFFFF, 0x00FFFFFF);
    // (10, 10) is outside outer clip: untouched white (0x00FFFFFF)
    assert_eq!(buffer[10 * 100 + 10] & 0x00FFFFFF, 0x00FFFFFF);
}
