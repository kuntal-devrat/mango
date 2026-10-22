use mango_core::{Color, Size};
use mango_html::parse_html;
use mango_layout::layout_document;
use mango_render::display_list::DisplayCommand;
use mango_render::paint;

// ========================================================================
// HTML5 Parser & Tree Construction
// ========================================================================

#[test]
fn test_html5_self_closing_slash_on_non_void_element_ignored() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div/>
            <p>Text</p>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    // Under HTML5 WHATWG spec, <div/> does NOT self-close; the slash is ignored and it acts as an open tag.
    let root = doc.root();
    let body_id = doc.find_element_by_tag(root, "body").unwrap();
    let div_id = doc.find_element_by_tag(body_id, "div").unwrap();
    let p_inside_div = doc.find_element_by_tag(div_id, "p");
    assert!(
        p_inside_div.is_some(),
        "<div/> acts as open tag, containing the subsequent <p>"
    );
}

#[test]
fn test_html5_entities_decoding() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <p>&copy; &reg; &euro; &alpha; &beta;</p>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let root = doc.root();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();
    let text = doc.text_content(p_id);
    assert!(text.contains('©'));
    assert!(text.contains('®'));
    assert!(text.contains('€'));
    assert!(text.contains('α'));
    assert!(text.contains('β'));
}

#[test]
fn test_html5_rcdata_textarea_and_title() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>&lt;Hello&gt; &amp; World</title>
        </head>
        <body>
            <textarea><b>No Bold</b> &amp; &lt;raw&gt;</textarea>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let root = doc.root();
    let title_id = doc.find_element_by_tag(root, "title").unwrap();
    let title = doc.text_content(title_id);
    assert_eq!(
        title, "<Hello> & World",
        "RCDATA in <title> should decode entities without creating element nodes"
    );

    let textarea_id = doc.find_element_by_tag(root, "textarea").unwrap();
    let text = doc.text_content(textarea_id);
    assert_eq!(
        text, "<b>No Bold</b> & <raw>",
        "RCDATA in <textarea> should preserve tags as literal text while decoding entities"
    );
}

// ========================================================================
// Rendering Engine Fidelity & 2D Effects
// ========================================================================

#[test]
fn test_render_inset_box_shadow() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .box {
                    width: 100px;
                    height: 100px;
                    box-shadow: inset 5px 5px 10px #000000;
                }
            </style>
        </head>
        <body>
            <div class="box"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, dl) = layout_document(&doc, &[], viewport);

    let has_inset_shadow = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::DrawBoxShadow { inset: true, .. }));
    assert!(
        has_inset_shadow,
        "box-shadow: inset ... should emit DrawBoxShadow with inset: true"
    );

    // Also verify painter rasterization does not panic
    let mut pixels = vec![0u32; 800 * 600];
    paint(&dl, &mut pixels, 800, 600);
}

#[test]
fn test_render_drop_shadow_filter() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .box {
                    width: 100px;
                    height: 100px;
                    filter: drop-shadow(4px 4px 8px #ff0000);
                }
            </style>
        </head>
        <body>
            <div class="box"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, dl) = layout_document(&doc, &[], viewport);

    let has_filter = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::PushFilter { .. }));
    assert!(
        has_filter,
        "filter: drop-shadow should emit PushFilter with DropShadow"
    );

    let mut pixels = vec![0u32; 800 * 600];
    paint(&dl, &mut pixels, 800, 600);
}

#[test]
fn test_svg_fill_none_and_stroke() {
    let html = r##"
        <!DOCTYPE html>
        <html>
        <body>
            <svg width="50" height="50" fill="none">
                <circle cx="25" cy="25" r="20" stroke="#0000ff" stroke-width="2"/>
            </svg>
        </body>
        </html>
    "##;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, dl) = layout_document(&doc, &[], viewport);

    let has_image = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
    assert!(has_image, "SVG should be rendered to an image atom");
}

// ========================================================================
// Text Shaping & Typography
// ========================================================================

#[test]
fn test_text_shaping_and_bidi() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <p>English and العربية RTL and 漢字 CJK shaping.</p>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root, dl) = layout_document(&doc, &[], viewport);

    assert!(root.dimensions.content.height() > 0.0);
    let has_text = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::DrawText { .. }));
    assert!(
        has_text,
        "Complex multilingual text should shape and emit DrawText commands"
    );

    let mut pixels = vec![0u32; 800 * 600];
    paint(&dl, &mut pixels, 800, 600);
}

#[test]
fn test_canvas_2d_precision() {
    use mango_render::canvas::Canvas2D;
    let mut canvas = Canvas2D::new(64, 64).expect("Canvas should create");

    // Test arc_to tangent rounding
    canvas.begin_path();
    canvas.move_to(10.0, 10.0);
    canvas.arc_to(50.0, 10.0, 50.0, 50.0, 15.0);
    canvas.line_to(50.0, 50.0);
    canvas.set_stroke_style(Color::rgb(0, 255, 0));
    canvas.stroke();
    assert_eq!(canvas.current_point(), Some((50.0, 50.0)));

    // Test put_image_data with dirty offset
    let mut img_data = vec![0u8; 8 * 8 * 4];
    for i in 0..64 {
        img_data[i * 4] = 255; // Red
        img_data[i * 4 + 3] = 255;
    }
    canvas.put_image_data_with_stride(&img_data, 8, 10, 10, 2, 2, 4, 4);
    let output = canvas.get_image_data(0, 0, 64, 64);
    // At canvas (10+2, 10+2) = (12, 12), pixel must be red
    let pixel_idx = ((12 * 64 + 12) * 4) as usize;
    assert_eq!(output[pixel_idx], 255);
    assert_eq!(output[pixel_idx + 3], 255);
}
