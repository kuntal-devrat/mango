//! Comprehensive integration tests for Mango's WebRender backend.

use super::*;
use crate::display_list::{BorderWidths, DisplayCommand, DisplayList};
use mango_core::{Color, Rect};
use std::sync::Arc;

#[test]
fn test_webrender_solid_rect() {
    let mut dl = DisplayList::new();
    // Top half red: y in 0..50
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 50.0),
        color: Color::rgba(255, 0, 0, 255),
    });
    // Bottom half blue: y in 50..100
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 50.0, 100.0, 50.0),
        color: Color::rgba(0, 0, 255, 255),
    });

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render frame");
    assert_eq!(pixels.len(), 100 * 100);

    let top_pixel = pixels[10 * 100 + 50];
    let bottom_pixel = pixels[60 * 100 + 50];

    assert_eq!(top_pixel, 0xFFFF0000, "Top pixel should be opaque red");
    assert_eq!(
        bottom_pixel, 0xFF0000FF,
        "Bottom pixel should be opaque blue"
    );
}

#[test]
fn test_webrender_rounded_rect() {
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::FillRoundedRect {
        rect: Rect::new(10.0, 10.0, 80.0, 80.0),
        color: Color::rgba(0, 255, 0, 255),
        radii: [15.0, 15.0, 15.0, 15.0],
    });

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render frame");
    assert_eq!(pixels.len(), 100 * 100);

    // Center of rectangle (x=50, y=50) must be Green (0xFF00FF00)
    let center_pixel = pixels[50 * 100 + 50];
    assert_eq!(center_pixel, 0xFF00FF00, "Center pixel should be green");

    // Top-left corner outside the rounded arc (x=11, y=11) must be background (0x00000000)
    let outside_pixel = pixels[11 * 100 + 11];
    assert_eq!(
        outside_pixel, 0x00000000,
        "Corner outside rounded radius should remain transparent"
    );
}

#[test]
fn test_webrender_border() {
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::DrawBorder {
        rect: Rect::new(10.0, 10.0, 80.0, 80.0),
        color: Color::rgba(255, 255, 0, 255),
        widths: BorderWidths::all(5.0),
        radii: [0.0; 4],
    });

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render frame");
    assert_eq!(pixels.len(), 100 * 100);

    // On the top border edge (y=12, x=50) should be yellow (0xFFFFFF00)
    let border_pixel = pixels[12 * 100 + 50];
    assert_eq!(border_pixel, 0xFFFFFF00, "Border pixel should be yellow");

    // Inside the border interior (y=50, x=50) should be empty/transparent
    let interior_pixel = pixels[50 * 100 + 50];
    assert_eq!(
        interior_pixel, 0x00000000,
        "Interior of border should be transparent"
    );
}

#[test]
fn test_webrender_box_shadow() {
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::DrawBoxShadow {
        rect: Rect::new(20.0, 20.0, 40.0, 40.0),
        color: Color::rgba(255, 0, 0, 255),
        offset_x: 10.0,
        offset_y: 10.0,
        blur_radius: 0.0,
        spread_radius: 5.0,
        radii: [0.0; 4],
        inset: false,
    });

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render frame");
    assert_eq!(pixels.len(), 100 * 100);

    // Shadow offset location (x=60, y=60) should have shadow red pixels
    let shadow_pixel = pixels[60 * 100 + 60];
    assert_ne!(
        shadow_pixel, 0x00000000,
        "Shadow pixel should be non-transparent"
    );
}

#[test]
fn test_webrender_image() {
    let mut dl = DisplayList::new();
    // 2x2 image: Red, Green, Blue, White
    let img_pixels: Arc<[u32]> = Arc::new([0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFFFF]);

    dl.push(DisplayCommand::DrawImage {
        x: 10.0,
        y: 10.0,
        width: 20.0,
        height: 20.0,
        pixels: img_pixels,
    });

    let pixels = render_with_webrender(&dl, 50, 50).expect("WebRender should render image");
    assert_eq!(pixels.len(), 50 * 50);

    // Top-left pixel of image region (x=12, y=12) should be Red
    let p_tl = pixels[12 * 50 + 12];
    assert_eq!(p_tl, 0xFFFF0000, "Top-left of 2x2 image should be red");
}

#[test]
fn test_webrender_transform_translation() {
    let mut dl = DisplayList::new();
    // Translate by (30, 30)
    dl.push(DisplayCommand::PushTransform {
        matrix: [1.0, 0.0, 0.0, 1.0, 30.0, 30.0],
    });
    // Draw 20x20 rect at (0, 0), which translates to (30, 30)
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 20.0, 20.0),
        color: Color::rgba(255, 0, 255, 255),
    });
    dl.push(DisplayCommand::PopTransform);

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render frame");
    assert_eq!(pixels.len(), 100 * 100);

    // At original position (x=5, y=5) should be empty
    assert_eq!(pixels[5 * 100 + 5], 0x00000000);
    // At translated position (x=35, y=35) should be Magenta (0xFFFF00FF)
    assert_eq!(pixels[35 * 100 + 35], 0xFFFF00FF);
}

#[test]
fn test_webrender_clip() {
    let mut dl = DisplayList::new();
    // Clip to rect (20, 20, 20, 20)
    dl.push(DisplayCommand::PushClip {
        rect: Rect::new(20.0, 20.0, 20.0, 20.0),
    });
    // Draw 100x100 rect
    dl.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        color: Color::rgba(0, 255, 255, 255),
    });
    dl.push(DisplayCommand::PopClip);

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render frame");
    assert_eq!(pixels.len(), 100 * 100);

    // Outside clip (x=10, y=10) should be transparent
    assert_eq!(pixels[10 * 100 + 10], 0x00000000);
    // Inside clip (x=25, y=25) should be Cyan (0xFF00FFFF)
    assert_eq!(pixels[25 * 100 + 25], 0xFF00FFFF);
    // Outside clip (x=50, y=50) should be transparent
    assert_eq!(pixels[50 * 100 + 50], 0x00000000);
}

#[test]
fn test_webrender_line() {
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::DrawLine {
        x1: 10.0,
        y1: 50.0,
        x2: 90.0,
        y2: 50.0,
        color: Color::rgba(255, 128, 0, 255),
        thickness: 4.0,
    });

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render line");
    assert_eq!(pixels.len(), 100 * 100);

    // On the line (x=50, y=50) should be colored
    let line_pixel = pixels[50 * 100 + 50];
    assert_ne!(line_pixel, 0x00000000, "Pixel on line should be drawn");
    // Away from line (x=50, y=10) should be transparent
    assert_eq!(pixels[10 * 100 + 50], 0x00000000);
}

#[test]
fn test_webrender_text() {
    let mut dl = DisplayList::new();
    dl.push(DisplayCommand::draw_text(
        "Mango WebRender",
        10.0,
        10.0,
        Color::rgba(255, 0, 0, 255),
        16.0,
        crate::font::FontWeight::Regular,
        crate::font::FontFamily::SansSerif,
    ));

    let pixels = render_with_webrender(&dl, 200, 50).expect("WebRender should render text");
    assert_eq!(pixels.len(), 200 * 50);

    // Text must produce rendered non-zero pixels
    let non_zero_count = pixels.iter().filter(|&&p| p != 0).count();
    assert!(
        non_zero_count > 50,
        "Text must produce non-zero pixels, found {non_zero_count}"
    );
}

#[test]
fn test_webrender_gradient() {
    let mut dl = DisplayList::new();
    let grad = mango_css::values::Gradient::Linear {
        angle_deg: 180.0,
        stops: vec![
            mango_css::values::ColorStop::new(Color::rgba(255, 0, 0, 255), Some(0.0), None),
            mango_css::values::ColorStop::new(Color::rgba(0, 0, 255, 255), Some(1.0), None),
        ],
        repeating: false,
    };
    dl.push(DisplayCommand::FillGradient {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        gradient: Box::new(grad),
        radii: [0.0; 4],
        opacity: 1.0,
    });

    let pixels = render_with_webrender(&dl, 100, 100).expect("WebRender should render gradient");
    assert_eq!(pixels.len(), 100 * 100);

    // Top pixel (y=2, x=50) should be mostly Red
    let top_pixel = pixels[2 * 100 + 50];
    let top_r = (top_pixel >> 16) & 0xFF;
    let top_b = top_pixel & 0xFF;
    assert!(
        top_r > 200 && top_b < 50,
        "Top of gradient should be red, got {top_pixel:#010x}"
    );

    // Bottom pixel (y=98, x=50) should be mostly Blue
    let bottom_pixel = pixels[98 * 100 + 50];
    let bottom_r = (bottom_pixel >> 16) & 0xFF;
    let bottom_b = bottom_pixel & 0xFF;
    assert!(
        bottom_b > 200 && bottom_r < 50,
        "Bottom of gradient should be blue, got {bottom_pixel:#010x}"
    );
}

#[test]
fn test_transform_matrix_order() {
    let t = to_layout_transform([1.0, 0.0, 0.0, 1.0, 30.0, 40.0]);
    let p = t.transform_point2d(to_layout_point(0.0, 0.0));
    println!("transformed point: {p:?}");
}
