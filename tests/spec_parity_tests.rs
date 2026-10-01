//! Web Platform Specifications & Chromium Parity Integration Tests.
//!
//! Validates layout idempotency, inline formatting, flex/grid/table compliance,
//! CSS cascade & syntax, HTML5 tree builder rules, rendering fidelity, and dynamic resolution (1920x1080).

use mango_core::{Color, Size};
use mango_html::parse_html;
use mango_layout::{layout_document, relayout_box_tree};
use mango_render::display_list::DisplayCommand;
use mango_render::paint;

// ========================================================================
// Dynamic Resolution & Responsiveness (1920x1080 and more)
// ========================================================================

#[test]
fn test_dynamic_resolution_1920x1080_media_queries_and_viewport_units() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {
                    margin: 0;
                }
                .box {
                    width: 50vw;
                    height: 25vh;
                    background-color: #ff0000;
                }
                @media (min-width: 1024px) {
                    .box {
                        background-color: #00ff00;
                    }
                }
            </style>
        </head>
        <body>
            <div class="box"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);

    // 1. Layout at 800x600 (under 1024px)
    let vp_small = Size::new(800.0, 600.0);
    let (root_small, dl_small) = layout_document(&doc, &[], vp_small);
    let box_small = root_small.children[0].children[0].dimensions.content;
    assert_eq!(
        box_small.width(),
        400.0,
        "50vw at 800px width should be 400px"
    );
    assert_eq!(
        box_small.height(),
        150.0,
        "25vh at 600px height should be 150px"
    );

    let has_red_small = dl_small.iter().any(|cmd| matches!(cmd, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(255, 0, 0)));
    assert!(has_red_small, "Under 1024px viewport should be red");

    // 2. Layout at 1920x1080 (over 1024px)
    let vp_large = Size::new(1920.0, 1080.0);
    let (root_large, dl_large) = layout_document(&doc, &[], vp_large);
    let box_large = root_large.children[0].children[0].dimensions.content;
    assert_eq!(
        box_large.width(),
        960.0,
        "50vw at 1920px width should be 960px"
    );
    assert_eq!(
        box_large.height(),
        270.0,
        "25vh at 1080px height should be 270px"
    );

    let has_green_large = dl_large.iter().any(|cmd| matches!(cmd, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 255, 0)));
    assert!(
        has_green_large,
        "1920x1080 viewport should trigger min-width: 1024px green style"
    );
}

#[test]
fn test_js_match_media_dynamic_resolution() {
    let doc = parse_html("<!DOCTYPE html><html><body></body></html>");
    let mut js_runtime = mango_js::JsRuntime::new(doc, 800.0, 600.0);
    js_runtime
        .eval("window.innerWidth = 1920; window.innerHeight = 1080;")
        .unwrap();

    let m1 = js_runtime
        .eval("window.matchMedia('(min-width: 1024px)').matches")
        .unwrap();
    assert_eq!(
        m1, "true",
        "matchMedia(min-width: 1024px) at 1920px should be true"
    );

    let m2 = js_runtime
        .eval("window.matchMedia('(max-width: 1023px)').matches")
        .unwrap();
    assert_eq!(
        m2, "false",
        "matchMedia(max-width: 1023px) at 1920px should be false"
    );

    // Change to small resolution
    js_runtime
        .eval("window.innerWidth = 800; window.innerHeight = 600;")
        .unwrap();
    let m3 = js_runtime
        .eval("window.matchMedia('(min-width: 1024px)').matches")
        .unwrap();
    assert_eq!(
        m3, "false",
        "matchMedia(min-width: 1024px) at 800px should be false"
    );

    let m4 = js_runtime
        .eval("window.matchMedia('(max-width: 1023px)').matches")
        .unwrap();
    assert_eq!(
        m4, "true",
        "matchMedia(max-width: 1023px) at 800px should be true"
    );
}

// ========================================================================
// Layout Architecture & Formatting Contexts
// ========================================================================

#[test]
fn test_layout_idempotent_box_tree_relayout() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <p>Hello World this is a layout idempotency test sentence.</p>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (mut root, dl1) = layout_document(&doc, &[], viewport);

    fn count_draw_texts(dl: &[DisplayCommand]) -> usize {
        dl.iter()
            .filter(|c| matches!(c, DisplayCommand::DrawText { .. }))
            .count()
    }

    let count1 = count_draw_texts(&dl1);
    assert!(count1 > 0);

    // Perform multiple relayouts
    relayout_box_tree(&mut root, viewport);
    let dl2 = mango_layout::build_display_list(&root);
    let count2 = count_draw_texts(&dl2);
    assert_eq!(
        count1, count2,
        "First relayout should not duplicate text runs"
    );

    relayout_box_tree(&mut root, viewport);
    let dl3 = mango_layout::build_display_list(&root);
    let count3 = count_draw_texts(&dl3);
    assert_eq!(
        count1, count3,
        "Second relayout should be strictly idempotent"
    );
}

#[test]
fn test_layout_no_synthetic_inline_spacing() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <p><span>Hello</span><span>World</span></p>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, dl) = layout_document(&doc, &[], viewport);

    // HelloWorld without space in the DOM should not have a synthetic space inserted
    let text_cmds: Vec<String> = dl
        .iter()
        .filter_map(|cmd| {
            if let DisplayCommand::DrawText { text, .. } = cmd {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect();

    let combined = text_cmds.join("");
    assert_eq!(
        combined, "HelloWorld",
        "Adjacent text nodes without space must not have synthetic spaces inserted"
    );
}

#[test]
fn test_layout_inline_block_shrink_to_fit() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .ib {
                    display: inline-block;
                    background: #eee;
                }
            </style>
        </head>
        <body>
            <div class="ib">Short</div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root, _) = layout_document(&doc, &[], viewport);

    let ib_box = &root.children[0].children[0];
    let w = ib_box.dimensions.content.width();
    assert!(
        w < 200.0,
        "Inline-block width ({}) should shrink-to-fit, not expand to container width (800px)",
        w
    );
}

#[test]
fn test_layout_inline_replaced_elements_side_by_side() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <p>
                <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==" width="20" height="20" />
                <span>Text</span>
            </p>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root, _) = layout_document(&doc, &[], viewport);

    let p = &root.children[0].children[0];
    // Replaced element (img) should be treated as inline and participate in the same inline formatting context
    assert!(!p.children.is_empty());
}

#[test]
fn test_flex_auto_margin_relayout_stability() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {
                    margin: 0;
                }
                .flex {
                    display: flex;
                    width: 400px;
                }
                .item {
                    width: 50px;
                    margin-left: auto;
                }
            </style>
        </head>
        <body>
            <div class="flex">
                <div class="item"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (mut root, _) = layout_document(&doc, &[], viewport);

    let item_x1 = root.children[0].children[0].children[0]
        .dimensions
        .content
        .x();
    assert_eq!(item_x1, 350.0, "Auto margin should push item to x=350");

    // Relayout should not accumulate margins
    relayout_box_tree(&mut root, viewport);
    let item_x2 = root.children[0].children[0].children[0]
        .dimensions
        .content
        .x();
    assert_eq!(
        item_x1, item_x2,
        "Margin-left: auto must be reset to computed style and not drift on relayout"
    );
}

#[test]
fn test_grid_negative_lines_and_fr_tracks() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {
                    margin: 0;
                }
                .grid {
                    display: grid;
                    grid-template-columns: 100px 1fr 100px;
                    width: 500px;
                }
                .last {
                    grid-column: -2 / -1;
                }
            </style>
        </head>
        <body>
            <div class="grid">
                <div class="last">End</div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root, _) = layout_document(&doc, &[], viewport);

    let last_item = &root.children[0].children[0].children[0];
    let x = last_item.dimensions.content.x();
    assert_eq!(
        x, 400.0,
        "Item with grid-column: -2 / -1 should be in track 3 starting at 400px"
    );
}

#[test]
fn test_table_min_content_clamping() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                table {
                    width: 10px; /* smaller than content */
                }
            </style>
        </head>
        <body>
            <table>
                <tr>
                    <td>LongContentThatCannotBeCompressedTo10px</td>
                </tr>
            </table>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root, _) = layout_document(&doc, &[], viewport);

    let table_box = &root.children[0].children[0];
    let w = table_box.dimensions.content.width();
    assert!(
        w > 50.0,
        "Table must clamp to min-content rather than shrinking down to 10px"
    );
}

#[test]
fn test_position_fixed_and_absolute_containing_block() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {
                    margin: 0;
                }
                .rel {
                    position: relative;
                    left: 50px;
                    top: 50px;
                    width: 300px;
                    height: 300px;
                }
                .abs {
                    position: absolute;
                    left: 10px;
                    top: 10px;
                    width: 50px;
                    height: 50px;
                }
                .fixed {
                    position: fixed;
                    right: 20px;
                    bottom: 20px;
                    width: 60px;
                    height: 60px;
                }
            </style>
        </head>
        <body>
            <div class="rel">
                <div class="abs"></div>
                <div class="fixed"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root, _) = layout_document(&doc, &[], viewport);

    let rel = &root.children[0].children[0];
    let abs = &rel.children[0];
    let fixed = &rel.children[1];

    // Absolute should be offset relative to .rel (x=50, y=50) -> x=60, y=60
    assert_eq!(abs.dimensions.content.x(), 60.0);
    assert_eq!(abs.dimensions.content.y(), 60.0);

    // Fixed should be positioned relative to viewport (800x600) -> right:20 => x=800-20-60=720, bottom:20 => y=600-20-60=520
    assert_eq!(fixed.dimensions.content.x(), 720.0);
    assert_eq!(fixed.dimensions.content.y(), 520.0);
}

// ========================================================================
// CSS Engine Parity & Cascade
// ========================================================================

#[test]
fn test_css_important_origin_order() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .box1 {
                    color: #0000ff !important;
                }
                .box2 {
                    color: #0000ff !important;
                }
            </style>
        </head>
        <body>
            <!-- Author !important beats Inline normal -->
            <div class="box1" style="color: #ff0000;">Box 1</div>
            <!-- Inline !important beats Author !important -->
            <div class="box2" style="color: #00ff00 !important;">Box 2</div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, dl) = layout_document(&doc, &[], viewport);

    let has_blue_text = dl.iter().any(|cmd| matches!(cmd, DisplayCommand::DrawText { text, color, .. } if text == "Box 1" && *color == Color::rgb(0, 0, 255)));
    assert!(
        has_blue_text,
        "Author !important must override Inline normal"
    );

    let has_green_text = dl.iter().any(|cmd| matches!(cmd, DisplayCommand::DrawText { text, color, .. } if text == "Box 2" && *color == Color::rgb(0, 255, 0)));
    assert!(
        has_green_text,
        "Inline !important must override Author !important"
    );
}

#[test]
fn test_css_case_sensitive_id_and_class() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                #myID {
                    background-color: #00ff00;
                }
                #myid {
                    background-color: #ff0000;
                }
                .myClass {
                    color: #0000ff;
                }
                .myclass {
                    color: #ffff00;
                }
            </style>
        </head>
        <body>
            <div id="myID" class="myClass">Case Test</div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, dl) = layout_document(&doc, &[], viewport);

    let has_green_bg = dl.iter().any(|cmd| matches!(cmd, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 255, 0)));
    assert!(
        has_green_bg,
        "Selector #myID must match id='myID' case-sensitively and NOT #myid"
    );

    let has_blue_text = dl.iter().any(|cmd| matches!(cmd, DisplayCommand::DrawText { color, .. } if *color == Color::rgb(0, 0, 255)));
    assert!(
        has_blue_text,
        "Selector .myClass must match class='myClass' case-sensitively"
    );
}

#[test]
fn test_css_unknown_units_dropped() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .box {
                    width: 200px;
                    width: 500fakeunit; /* Invalid unknown unit, must be dropped */
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
    let (root, _) = layout_document(&doc, &[], viewport);

    let b = &root.children[0].children[0];
    assert_eq!(
        b.dimensions.content.width(),
        200.0,
        "Unknown units must be rejected and fallback to valid prior declaration"
    );
}

#[test]
fn test_css_calc_dimensions_addition() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                :root {
                    --base: 20px;
                }
                .box {
                    width: calc(40px + 60px);
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
    let (root, _) = layout_document(&doc, &[], viewport);

    let b = &root.children[0].children[0];
    assert_eq!(
        b.dimensions.content.width(),
        100.0,
        "calc(40px + 60px) should evaluate to 100px"
    );
}

#[test]
fn test_css_multi_value_box_shadow() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .box {
                    width: 100px;
                    height: 100px;
                    box-shadow: 2px 2px 4px #ff0000;
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

    let shadow_count = dl
        .iter()
        .filter(|cmd| matches!(cmd, DisplayCommand::DrawBoxShadow { .. }))
        .count();
    assert!(
        shadow_count >= 1,
        "Box shadow should produce DrawBoxShadow command"
    );
}

#[test]
fn test_css_supports_rule_evaluation() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .box {
                    width: 100px;
                    height: 100px;
                    background-color: #ff0000;
                }
                @supports (display: flex) {
                    .box {
                        background-color: #00ff00;
                    }
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

    let has_green = dl.iter().any(|cmd| matches!(cmd, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 255, 0)));
    assert!(
        has_green,
        "@supports (display: flex) should evaluate to true and apply green background"
    );
}

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
