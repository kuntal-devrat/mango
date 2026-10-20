use mango_core::Color;

use super::*;

#[test]
fn test_canvas_creation_and_fill_rect() {
    let mut canvas = Canvas2D::new(100, 100).expect("Canvas should create");
    canvas.set_fill_style(Color::rgb(255, 0, 0));
    canvas.fill_rect(10.0, 10.0, 20.0, 20.0);

    let pixels = canvas.get_pixels();
    assert_eq!(pixels.len(), 100 * 100);

    // Pixel at (15, 15) should be opaque red
    let idx = 15 * 100 + 15;
    assert_eq!(pixels[idx], 0xFFFF_0000);

    // Pixel outside (0, 0) should be transparent
    assert_eq!(pixels[0], 0);
}

#[test]
fn test_canvas_path_stroke_and_state_stack() {
    let mut canvas = Canvas2D::new(200, 200).expect("Canvas should create");
    canvas.save();
    canvas.set_line_width(4.0);
    canvas.set_stroke_style(Color::rgb(0, 255, 0));
    canvas.translate(50.0, 50.0);

    canvas.begin_path();
    canvas.move_to(0.0, 0.0);
    canvas.line_to(50.0, 0.0);
    canvas.stroke();

    canvas.restore();
    assert_eq!(canvas.line_width(), 1.0);
}

#[test]
fn test_canvas_get_put_image_data_and_data_url() {
    let mut canvas = Canvas2D::new(50, 50).expect("Canvas should create");
    canvas.set_fill_style(Color::rgb(0, 0, 255));
    canvas.fill_rect(0.0, 0.0, 50.0, 50.0);

    let data = canvas.get_image_data(0, 0, 50, 50);
    assert_eq!(data.len(), 50 * 50 * 4);
    assert_eq!(data[0], 0); // R
    assert_eq!(data[1], 0); // G
    assert_eq!(data[2], 255); // B
    assert_eq!(data[3], 255); // A

    let url = canvas.to_data_url("image/png");
    assert!(url.starts_with("data:image/png;base64,"));
    assert!(url.len() > 30);
}

#[test]
fn test_canvas_text_measurement() {
    let canvas = Canvas2D::new(100, 100).expect("Canvas should create");
    let w = canvas.measure_text("Hello World");
    assert!(w > 0.0, "Measured text width must be positive");
}

#[test]
fn test_canvas_arc_to() {
    let mut canvas = Canvas2D::new(100, 100).expect("Canvas should create");
    canvas.begin_path();
    canvas.move_to(10.0, 10.0);
    // Draw rounded corner via arc_to
    canvas.arc_to(50.0, 10.0, 50.0, 50.0, 10.0);
    canvas.line_to(50.0, 50.0);
    canvas.set_stroke_style(Color::rgb(255, 0, 0));
    canvas.stroke();

    assert_eq!(canvas.current_point(), Some((50.0, 50.0)));
}

#[test]
fn test_canvas_put_image_data_dirty_rect() {
    let mut canvas = Canvas2D::new(20, 20).expect("Canvas should create");
    // Create 10x10 red image data
    let mut img_data = vec![0u8; 10 * 10 * 4];
    for i in 0..100 {
        img_data[i * 4] = 255; // R
        img_data[i * 4 + 3] = 255; // A
    }
    // Put only a dirty sub-rectangle of 4x4 from offset (2, 2)
    canvas.put_image_data_with_stride(&img_data, 10, 5, 5, 2, 2, 4, 4);

    let out = canvas.get_image_data(0, 0, 20, 20);
    // Canvas (5+2, 5+2) = (7, 7) should be red
    let idx = ((7 * 20 + 7) * 4) as usize;
    assert_eq!(out[idx], 255);
    assert_eq!(out[idx + 3], 255);

    // Canvas (0, 0) should still be transparent
    assert_eq!(out[0], 0);
    assert_eq!(out[3], 0);
}
