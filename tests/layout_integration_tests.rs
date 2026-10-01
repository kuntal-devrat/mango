//! End-to-end integration tests for Mango's layout and rendering pipeline:
//! Box model, Flexbox, Positioning, Inline-Block, Stacking Contexts, and Rasterization.

use mango_core::{Color, Size};
use mango_html::parse_html;
use mango_layout::layout_document;
use mango_render::display_list::DisplayCommand;
use mango_render::paint;

#[test]
fn test_full_pipeline_layout_and_paint() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Integration Test</title>
            <style>
                body {
                    margin: 20px;
                    background-color: #ffffff;
                }
                .banner {
                    background-color: #ffa136;
                    padding: 10px;
                    margin-bottom: 15px;
                }
                h1 {
                    font-size: 24px;
                    color: #ffffff;
                    margin: 0px;
                }
                .content {
                    padding: 8px;
                    border-top-width: 2px;
                    border-top-color: #000000;
                }
                p {
                    font-size: 14px;
                    color: #333333;
                }
            </style>
        </head>
        <body>
            <div class="banner">
                <h1>Mango Browser Integration Test</h1>
            </div>
            <div class="content">
                <p>Testing the complete pipeline from HTML to rasterized pixels.</p>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, display_list) = layout_document(&doc, &[], viewport);

    assert!(root_box.dimensions.content.width() > 0.0);
    assert!(root_box.dimensions.content.height() > 0.0);
    assert!(!display_list.is_empty());

    let mut has_banner_bg = false;
    let mut has_border = false;
    let mut has_text = false;

    for cmd in display_list.iter() {
        match cmd {
            DisplayCommand::FillRect { color, .. } if *color == Color::MANGO_ORANGE => {
                has_banner_bg = true;
            }
            DisplayCommand::DrawBorder { .. } => {
                has_border = true;
            }
            DisplayCommand::DrawText { text, .. } if text.contains("Mango") => {
                has_text = true;
            }
            _ => {}
        }
    }

    assert!(has_banner_bg, "Banner background color was not painted");
    assert!(has_border, "Content border was not painted");
    assert!(has_text, "Heading text was not painted");

    // Test rasterization into pixel buffer
    let width = 800;
    let height = 600;
    let mut buffer = vec![0u32; width * height];
    paint(&display_list, &mut buffer, width as u32, height as u32);

    let non_zero_pixels = buffer.iter().filter(|&&p| p != 0).count();
    assert!(non_zero_pixels > 0, "No pixels were drawn to the buffer");
}

#[test]
fn test_anonymous_blocks_in_mixed_containers() {
    let html = r#"
        <div>
            Inline text before
            <p>Block paragraph</p>
            Inline text after
        </div>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(600.0, 400.0);
    let (root_box, display_list) = layout_document(&doc, &[], viewport);

    assert!(root_box.dimensions.content.height() > 0.0);
    assert!(!display_list.is_empty());
}

#[test]
fn test_flexbox_row_space_between_and_gap() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .container {
                    display: flex;
                    justify-content: space-between;
                    gap: 10px;
                    width: 500px;
                    height: 100px;
                }
                .item {
                    width: 100px;
                    height: 50px;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <div class="item" id="item1"></div>
                <div class="item" id="item2"></div>
                <div class="item" id="item3"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let container = &body.children[0];
    assert_eq!(
        container.children.len(),
        3,
        "Container should have 3 flex items"
    );

    let item1 = &container.children[0];
    let item2 = &container.children[1];
    let item3 = &container.children[2];

    assert_eq!(item1.dimensions.content.x(), 0.0);
    assert_eq!(item1.dimensions.content.width(), 100.0);

    assert_eq!(item3.dimensions.content.x(), 400.0);
    assert_eq!(item3.dimensions.content.width(), 100.0);

    assert_eq!(item2.dimensions.content.x(), 200.0);
}

#[test]
fn test_flexbox_column_align_items_center() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .container {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    width: 400px;
                    height: 200px;
                }
                .item {
                    width: 120px;
                    height: 40px;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <div class="item"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let container = &body.children[0];
    let item = &container.children[0];

    assert_eq!(item.dimensions.content.x(), 140.0);
    assert_eq!(item.dimensions.content.width(), 120.0);
}

#[test]
fn test_flex_grow_distribution() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .container {
                    display: flex;
                    width: 600px;
                }
                .item1 {
                    width: 100px;
                    flex-grow: 1;
                }
                .item2 {
                    width: 100px;
                    flex-grow: 3;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <div class="item1"></div>
                <div class="item2"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let container = &body.children[0];
    let item1 = &container.children[0];
    let item2 = &container.children[1];

    assert_eq!(item1.dimensions.content.width(), 200.0);
    assert_eq!(item2.dimensions.content.width(), 400.0);
    assert_eq!(item2.dimensions.content.x(), 200.0);
}

#[test]
fn test_flex_wrap_multiline() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .container {
                    display: flex;
                    flex-wrap: wrap;
                    width: 250px;
                }
                .item {
                    width: 100px;
                    height: 50px;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <div class="item"></div>
                <div class="item"></div>
                <div class="item"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let container = &body.children[0];
    assert_eq!(container.children.len(), 3);

    let item1 = &container.children[0];
    let item2 = &container.children[1];
    let item3 = &container.children[2];

    assert_eq!(item1.dimensions.content.y(), 0.0);
    assert_eq!(item2.dimensions.content.y(), 0.0);
    assert!(item3.dimensions.content.y() >= 50.0);
    assert_eq!(item3.dimensions.content.x(), 0.0);
}

#[test]
fn test_inline_block_side_by_side() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .ib {
                    display: inline-block;
                    width: 120px;
                    height: 40px;
                }
            </style>
        </head>
        <body>
            <div>
                <div class="ib" id="ib1"></div>
                <div class="ib" id="ib2"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let outer_div = &body.children[0];
    assert_eq!(outer_div.children.len(), 2);

    let ib1 = &outer_div.children[0];
    let ib2 = &outer_div.children[1];

    assert_eq!(ib1.dimensions.content.width(), 120.0);
    assert_eq!(ib2.dimensions.content.width(), 120.0);
    assert_eq!(ib1.dimensions.content.y(), ib2.dimensions.content.y());
    assert!(ib2.dimensions.content.x() >= ib1.dimensions.content.right());
}

#[test]
fn test_position_relative_displacement() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .box {
                    width: 100px;
                    height: 50px;
                }
                .rel {
                    position: relative;
                    left: 30px;
                    top: 20px;
                }
            </style>
        </head>
        <body>
            <div class="box rel"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let box_rel = &body.children[0];

    assert_eq!(box_rel.dimensions.content.x(), 30.0);
    assert_eq!(box_rel.dimensions.content.y(), 20.0);
}

#[test]
fn test_position_absolute_containing_block() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .container {
                    position: relative;
                    width: 400px;
                    height: 300px;
                }
                .abs {
                    position: absolute;
                    right: 25px;
                    bottom: 15px;
                    width: 80px;
                    height: 40px;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <div class="abs"></div>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let container = &body.children[0];
    let abs_child = &container.children[0];

    assert_eq!(abs_child.dimensions.content.x(), 295.0);
    assert_eq!(abs_child.dimensions.content.y(), 245.0);
    assert_eq!(abs_child.dimensions.content.width(), 80.0);
    assert_eq!(abs_child.dimensions.content.height(), 40.0);
}

#[test]
fn test_z_index_stacking_order_in_display_list() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                .box1 {
                    position: relative;
                    z-index: 10;
                    background-color: #0000ff;
                    width: 100px;
                    height: 100px;
                }
                .box2 {
                    position: relative;
                    z-index: 2;
                    background-color: #ff0000;
                    width: 100px;
                    height: 100px;
                }
            </style>
        </head>
        <body>
            <div class="box1"></div>
            <div class="box2"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, display_list) = layout_document(&doc, &[], viewport);

    let colored_fills: Vec<Color> = display_list
        .iter()
        .filter_map(|cmd| match cmd {
            DisplayCommand::FillRect { color, .. } if *color != Color::WHITE => Some(*color),
            _ => None,
        })
        .collect();

    assert_eq!(colored_fills.len(), 2);
    assert_eq!(
        colored_fills[0],
        Color::RED,
        "z-index: 2 (Red) should paint before z-index: 10"
    );
    assert_eq!(
        colored_fills[1],
        Color::BLUE,
        "z-index: 10 (Blue) should paint after z-index: 2"
    );
}

#[test]
fn test_border_radius_rounded_rect_rendering() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .card {
                    background-color: #ff0000;
                    border-radius: 12px;
                    width: 200px;
                    height: 100px;
                }
            </style>
        </head>
        <body>
            <div class="card"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, display_list) = layout_document(&doc, &[], viewport);

    let has_rounded_rect = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::FillRoundedRect { radii, color, .. } => {
            *color == Color::RED && *radii == [12.0, 12.0, 12.0, 12.0]
        }
        _ => false,
    });

    assert!(
        has_rounded_rect,
        "Expected FillRoundedRect command for border-radius"
    );

    let mut buffer = vec![0u32; 800 * 600];
    paint(&display_list, &mut buffer, 800, 600);
}

#[test]
fn test_table_layout_dimensions() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                table {
                    width: 600px;
                }
                td {
                    padding: 5px;
                }
            </style>
        </head>
        <body>
            <table>
                <tr>
                    <td>Col 1</td>
                    <td>Col 2</td>
                    <td>Col 3</td>
                </tr>
            </table>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, display_list) = layout_document(&doc, &[], viewport);

    assert!(root_box.dimensions.content.width() > 0.0);
    assert!(!display_list.is_empty());

    let mut buffer = vec![0u32; 800 * 600];
    paint(&display_list, &mut buffer, 800, 600);
}

#[test]
fn test_form_controls_layout_and_paint() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <input type="text" value="Testing Forms" style="width: 200px; height: 28px;"/>
            <button style="width: 80px; height: 26px;">Click</button>
            <input type="checkbox" checked="true"/>
            <input type="radio" checked="true"/>
            <select style="width: 120px; height: 26px;">
                <option selected="true">Opt 1</option>
            </select>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, display_list) = layout_document(&doc, &[], viewport);

    let has_form_text = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text == "Testing Forms",
        _ => false,
    });
    assert!(has_form_text, "Expected form input text in display list");

    let has_checkbox_check = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text == "✓",
        _ => false,
    });
    assert!(
        has_checkbox_check,
        "Expected checkmark icon in display list"
    );

    let mut buffer = vec![0u32; 800 * 600];
    paint(&display_list, &mut buffer, 800, 600);
}

#[test]
fn test_list_items_marker_layout() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <ol>
                <li>Numbered 1</li>
                <li>Numbered 2</li>
            </ol>
            <ul>
                <li>Bulleted A</li>
            </ul>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_, display_list) = layout_document(&doc, &[], viewport);

    let has_number_marker = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.starts_with("1."),
        _ => false,
    });
    assert!(has_number_marker, "Expected list decimal marker '1.'");

    let has_bullet_marker = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.starts_with('•'),
        _ => false,
    });
    assert!(has_bullet_marker, "Expected disc bullet marker '•'");
}

#[test]
fn test_visibility_and_opacity_display() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .invisible {
                    visibility: hidden;
                    background-color: #ff0000;
                    width: 100px;
                    height: 50px;
                }
                .semi-trans {
                    opacity: 0.5;
                    background-color: #0000ff;
                    width: 100px;
                    height: 50px;
                }
            </style>
        </head>
        <body>
            <div class="invisible">Hidden Content</div>
            <div class="semi-trans">Semi Transparent</div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, display_list) = layout_document(&doc, &[], viewport);

    assert!(root_box.dimensions.content.height() >= 100.0);

    let has_red_bg = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, .. } | DisplayCommand::FillRoundedRect { color, .. } => {
            *color == Color::RED
        }
        _ => false,
    });
    assert!(
        !has_red_bg,
        "visibility: hidden element should not paint background"
    );

    let has_semi_blue = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, .. } | DisplayCommand::FillRoundedRect { color, .. } => {
            color.b == 255 && color.a == 128
        }
        _ => false,
    });
    assert!(has_semi_blue, "opacity: 0.5 element should have alpha ~128");
}

#[test]
fn test_web_font_face_layout_integration() {
    let fm = mango_render::font_manager();
    // Register custom web font
    let font_bytes = include_bytes!("../crates/mango_render/src/fonts/DejaVuSans.ttf");
    let font_id = fm
        .register_web_font(
            "WikipediaModern",
            mango_render::FontWeight::Regular,
            font_bytes,
        )
        .expect("web font should register successfully");

    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                h1 {
                    font-family: 'WikipediaModern', sans-serif;
                    font-size: 28px;
                }
            </style>
        </head>
        <body>
            <h1>Wikipedia Today</h1>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_root_box, display_list) = layout_document(&doc, &[], viewport);

    let text_cmd = display_list.iter().find(|cmd| matches!(cmd, DisplayCommand::DrawText { text, .. } if text.contains("Wikipedia Today")));
    assert!(text_cmd.is_some(), "Text run for heading must be generated");

    if let Some(DisplayCommand::DrawText {
        text,
        family,
        font_size,
        ..
    }) = text_cmd
    {
        assert_eq!(text, "Wikipedia Today");
        assert_eq!(*font_size, 28.0);
        assert_eq!(
            *family,
            mango_render::FontFamily::Custom(font_id),
            "Heading should resolve to registered custom web font"
        );
    }
}

#[test]
fn test_table_border_collapse_and_cellspacing() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                table {
                    border-collapse: collapse;
                    width: 400px;
                }
                td {
                    padding: 0px;
                    border: 1px solid black;
                }
            </style>
        </head>
        <body>
            <table>
                <tr>
                    <td id="c1">Cell 1</td>
                    <td id="c2">Cell 2</td>
                </tr>
            </table>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let table = &body.children[0];
    let row = if table.children[0].tag_name.as_deref() == Some("tbody") {
        &table.children[0].children[0]
    } else {
        &table.children[0]
    };
    let cell1 = &row.children[0];
    let cell2 = &row.children[1];

    // Under border-collapse: collapse, cell2 begins exactly where cell1 ends (single 1px border, no 2px spacing gap!)
    assert_eq!(cell1.dimensions.content.x(), 1.0);
    assert_eq!(
        cell2.dimensions.content.x(),
        cell1.dimensions.content.x() + cell1.dimensions.content.width() + 1.0
    );
}

#[test]
fn test_table_cell_vertical_alignment() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                table { width: 400px; }
                .tall { height: 100px; }
                .mid { vertical-align: middle; }
                .top { vertical-align: top; }
                .bot { vertical-align: bottom; }
            </style>
        </head>
        <body>
            <table>
                <tr>
                    <td class="top">Top</td>
                    <td class="mid">Middle</td>
                    <td class="bot">Bottom</td>
                    <td class="tall">Tall Content</td>
                </tr>
            </table>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let table = &body.children[0];
    let row = if table.children[0].tag_name.as_deref() == Some("tbody") {
        &table.children[0].children[0]
    } else {
        &table.children[0]
    };

    let cell_top = &row.children[0];
    let cell_mid = &row.children[1];
    let cell_bot = &row.children[2];

    let top_text = &cell_top.children[0];
    let mid_text = &cell_mid.children[0];
    let bot_text = &cell_bot.children[0];

    // top_text is at top of cell, mid_text shifted down to middle, bot_text shifted to bottom!
    assert!(
        mid_text.dimensions.content.y() > top_text.dimensions.content.y(),
        "middle text y ({}) must be > top text y ({})",
        mid_text.dimensions.content.y(),
        top_text.dimensions.content.y()
    );
    assert!(
        bot_text.dimensions.content.y() > mid_text.dimensions.content.y(),
        "bottom text y ({}) must be > middle text y ({})",
        bot_text.dimensions.content.y(),
        mid_text.dimensions.content.y()
    );
}

#[test]
fn test_table_rowspan_layout_and_stretch() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0px; }
                table { width: 300px; border-spacing: 0px; }
                td { padding: 0px; margin: 0px; }
                .r1c2 { height: 40px; }
                .r2c2 { height: 60px; }
            </style>
        </head>
        <body>
            <table>
                <tr>
                    <td rowspan="2" id="span-cell">Spanned 2 Rows</td>
                    <td class="r1c2" id="r1c2">Row 1 Col 2</td>
                </tr>
                <tr>
                    <td class="r2c2" id="r2c2">Row 2 Col 2</td>
                </tr>
            </table>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    let body = &root_box.children[0];
    let table = &body.children[0];
    let rows_container = if table.children[0].tag_name.as_deref() == Some("tbody") {
        &table.children[0]
    } else {
        table
    };

    let row1 = &rows_container.children[0];
    let row2 = &rows_container.children[1];

    let span_cell = &row1.children[0];
    let r1c2 = &row1.children[1];
    let r2c2 = &row2.children[0];

    // Row 2 only has 1 cell in HTML, but because Col 0 is occupied by rowspan,
    // its X coordinate must align with Col 1 (r1c2), NOT at x = 0!
    assert_eq!(
        r2c2.dimensions.content.x(),
        r1c2.dimensions.content.x(),
        "Row 2 cell must be placed in Column 1"
    );

    // Span cell must stretch across both rows (height >= row1 height + row2 height = 100px)
    assert!(
        span_cell.dimensions.content.height() >= 100.0,
        "Span cell height ({}) should stretch across both rows (>= 100.0)",
        span_cell.dimensions.content.height()
    );
}

/// Recursively searches the layout tree for the first box with the given tag name.
fn find_box<'a>(
    root: &'a mango_layout::box_tree::LayoutBox,
    tag: &str,
) -> Option<&'a mango_layout::box_tree::LayoutBox> {
    if root.tag_name.as_deref() == Some(tag) {
        return Some(root);
    }
    root.children.iter().find_map(|child| find_box(child, tag))
}

#[test]
fn test_explicit_line_height_matches_chromium_block_height() {
    // Chromium's block height for a single line with `line-height: 40px` is exactly
    // 40px: the line box defines the height and glyphs overflow it. Mango used to
    // grow the block by the text's font box, which pushed every following block down
    // a few pixels and desynchronised stacked sections against real browsers.
    let html = r#"<!doctype html><html><body style="margin:0">
        <div id="a" style="font:40px/40px Arial,sans-serif">H1</div>
        <div id="b" style="font:16px/16px Arial,sans-serif">x</div>
    </body></html>"#;
    let doc = parse_html(html);
    let (root_box, _) = layout_document(&doc, &[], Size::new(1280.0, 900.0));

    let a = find_box(&root_box, "div").expect("first div must exist");
    assert!(
        (a.dimensions.content.height() - 40.0).abs() < 0.5,
        "line-height 40px must produce a 40px block, got {}",
        a.dimensions.content.height()
    );

    let b = find_box(&root_box, "body")
        .and_then(|body| body.children.get(1))
        .expect("second div must exist");
    assert!(
        (b.dimensions.content.y() - 40.0).abs() < 0.5,
        "the second block must start at y=40, got {}",
        b.dimensions.content.y()
    );
}
