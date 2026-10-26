use mango_core::Size;
use mango_html::parse_html;
use mango_layout::box_tree::LayoutBox;
use mango_layout::layout_document;
use mango_render::display_list::DisplayCommand;

#[test]
fn test_web_font_face_layout_integration() {
    let fm = mango_render::font_manager();
    // Register custom web font
    let font_bytes = include_bytes!("../../crates/mango_render/src/fonts/DejaVuSans.ttf");
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
fn find_box<'a>(root: &'a LayoutBox, tag: &str) -> Option<&'a LayoutBox> {
    if root.tag_name.as_deref() == Some(tag) {
        return Some(root);
    }
    root.children.iter().find_map(|child| find_box(child, tag))
}

#[test]
fn test_explicit_line_height_matches_chromium_block_height() {
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

#[test]
fn test_rust_lang_footer() {
    let html = r#"<!doctype html><html><head><style>
        body {
            margin: 0;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }
        body > main {
            flex: 1;
            display: flex;
            flex-direction: column;
        }
        body > main > section:last-child {
            flex: 1;
        }
        nav { height: 60px; }
        section { height: 500px; }
        footer { height: 100px; }
    </style></head><body>
        <nav>Nav</nav>
        <main>
            <section>Section 1</section>
            <section>Section 2</section>
        </main>
        <footer>Footer</footer>
    </body></html>"#;
    let doc = parse_html(html);
    let (root_box, _) = layout_document(&doc, &[], Size::new(1280.0, 900.0));

    let footer = find_box(&root_box, "footer").expect("footer must exist");
    println!(
        "FOOTER Y = {}, HEIGHT = {}",
        footer.dimensions.content.y(),
        footer.dimensions.content.height()
    );
    let main_box = find_box(&root_box, "main").expect("main must exist");
    println!(
        "MAIN Y = {}, HEIGHT = {}",
        main_box.dimensions.content.y(),
        main_box.dimensions.content.height()
    );
    assert!(
        footer.dimensions.content.y() >= 1000.0,
        "footer must be below main content, got {}",
        footer.dimensions.content.y()
    );
}

#[test]
fn test_rust_lang_footer_sticky_short_content() {
    let html = r#"<!doctype html><html><head><style>
        body {
            margin: 0;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }
        body > main {
            flex: 1;
        }
        nav { height: 60px; }
        footer { height: 100px; }
    </style></head><body>
        <nav>Nav</nav>
        <main>
            <p>Short content</p>
        </main>
        <footer>Footer</footer>
    </body></html>"#;
    let doc = parse_html(html);
    let (root_box, _) = layout_document(&doc, &[], Size::new(1280.0, 900.0));

    let footer = find_box(&root_box, "footer").expect("footer must exist");
    println!(
        "STICKY FOOTER Y = {}, HEIGHT = {}",
        footer.dimensions.content.y(),
        footer.dimensions.content.height()
    );
    assert!(
        footer.dimensions.content.y() >= 800.0,
        "footer must be at bottom of 900px viewport, got {}",
        footer.dimensions.content.y()
    );
}

#[test]
fn test_inline_image_spacing() {
    let html = r#"<!doctype html><html><body style="margin:0"><p style="margin:0"><img src="a.png" style="width:50px;height:50px"> <img src="b.png" style="width:50px;height:50px"></p></body></html>"#;
    let doc = parse_html(html);
    let (root_box, _) = layout_document(&doc, &[], Size::new(1280.0, 900.0));

    let p = find_box(&root_box, "p").expect("p must exist");
    println!("P children count = {}", p.children.len());
    for (i, c) in p.children.iter().enumerate() {
        println!(
            "Child {}: type={:?}, x={}, width={}",
            i,
            c.box_type,
            c.dimensions.content.x(),
            c.dimensions.content.width()
        );
    }
    assert_eq!(
        p.children.len(),
        3,
        "p must have 3 children (img, space, img)"
    );
    assert!(
        p.children[2].dimensions.content.x() > 50.0,
        "second image must be spaced after first image + whitespace, got x={}",
        p.children[2].dimensions.content.x()
    );
}

#[test]
fn test_button_border_zero_removal() {
    let html = r#"<!doctype html><html><body style="margin:0"><button style="border: 0; padding: 10px;">Click me</button></body></html>"#;
    let doc = parse_html(html);
    let (root_box, dl) = layout_document(&doc, &[], Size::new(1280.0, 900.0));

    let btn = find_box(&root_box, "button").expect("button must exist");
    assert_eq!(btn.dimensions.border.top, 0.0);
    assert_eq!(btn.dimensions.border.bottom, 0.0);
    assert_eq!(btn.dimensions.border.left, 0.0);
    assert_eq!(btn.dimensions.border.right, 0.0);

    // Verify display list contains NO DrawBorder commands for this button
    let has_border_cmd = dl
        .iter()
        .any(|cmd| matches!(cmd, mango_render::DisplayCommand::DrawBorder { .. }));
    assert!(
        !has_border_cmd,
        "button with border: 0 must not emit DrawBorder command"
    );
}

#[test]
fn test_transition_shorthand_retarget() {
    use mango_css::ComputedStyle;
    use mango_css::animation::TransitionEngine;
    use mango_css::values::{TimingFunction, Transition};
    use mango_html::dom::NodeId;

    let mut engine = TransitionEngine::new();
    let node = NodeId::from_raw(1);

    let old_style = ComputedStyle {
        background_color: mango_core::Color::BLACK,
        ..Default::default()
    };

    let new_style = ComputedStyle {
        background_color: mango_core::Color::WHITE,
        transitions: vec![Transition {
            property: "background".to_string(),
            duration_ms: 150.0,
            delay_ms: 0.0,
            timing: TimingFunction::Ease,
        }],
        ..Default::default()
    };

    let retargeted = engine.retarget(node, &old_style, &new_style);
    assert!(
        retargeted,
        "transition: background must match background-color and trigger a transition"
    );
    assert!(
        engine.is_animating(),
        "engine should be animating background transition"
    );
}

#[test]
fn test_flex_column_cross_axis_auto_margins_and_max_width() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0;">
            <main style="display: flex; flex-direction: column; width: 1000px;">
                <header style="width: 100%; max-width: 600px; margin-left: auto; margin-right: auto; height: 100px;">
                    <h1>Title</h1>
                </header>
            </main>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(1000.0, 800.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    // Find the header element
    fn find_header(b: &LayoutBox) -> Option<&LayoutBox> {
        if b.tag_name.as_deref() == Some("header") {
            return Some(b);
        }
        for child in &b.children {
            if let Some(found) = find_header(child) {
                return Some(found);
            }
        }
        None
    }

    let header = find_header(&root_box).expect("header box should exist");
    let header_box = header.dimensions.border_box();
    assert_eq!(
        header_box.width(),
        600.0,
        "header width should be clamped to max-width (600px)"
    );
    assert_eq!(
        header_box.origin.x, 200.0,
        "header should be horizontally centered at x = (1000 - 600) / 2 = 200px"
    );
}

#[test]
fn test_sticky_positioning_scroll_containment_reftest() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 0; padding: 0; }
                .top-spacer { height: 100px; background-color: #ffffff; }
                .container { height: 600px; background-color: #f0f0f0; }
                .sticky-header {
                    position: sticky;
                    top: 0px;
                    height: 50px;
                    background-color: #ff0000;
                }
                .content { height: 550px; }
            </style>
        </head>
        <body>
            <div class="top-spacer"></div>
            <div class="container">
                <div class="sticky-header">Sticky Header</div>
                <div class="content"></div>
            </div>
            <div style="height: 1000px;"></div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (root_box, _) = layout_document(&doc, &[], viewport);

    // 1. At scroll_y = 0: header is in-flow at y = 100px.
    let dl_unscrolled = mango_layout::display_list::build_display_list_with_scroll(&root_box, 0.0);
    let header_rect_0 = dl_unscrolled
        .iter()
        .find_map(|cmd| match cmd {
            DisplayCommand::FillRect { rect, color }
                if *color == mango_core::Color::rgb(255, 0, 0) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("sticky header rect found at scroll 0");
    assert_eq!(
        header_rect_0.y(),
        100.0,
        "header should stay in-flow before reaching scroll threshold"
    );

    // 2. At scroll_y = 150: page scrolled past 100px threshold.
    // Sticky offset moves the header to stay at viewport top (scrollport y = 150px in root coordinates).
    let dl_scrolled = mango_layout::display_list::build_display_list_with_scroll(&root_box, 150.0);
    let header_rect_150 = dl_scrolled
        .iter()
        .find_map(|cmd| match cmd {
            DisplayCommand::FillRect { rect, color }
                if *color == mango_core::Color::rgb(255, 0, 0) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("sticky header rect found at scroll 150");
    assert_eq!(
        header_rect_150.y(),
        150.0,
        "sticky header should be pinned to viewport top (y = scroll_y)"
    );

    // 3. At scroll_y = 800: scrolled past the containing block (container ends at y = 100 + 600 = 700px).
    // Sticky header cannot escape containing block: max target = 700 - 50 = 650px.
    let dl_clamped = mango_layout::display_list::build_display_list_with_scroll(&root_box, 800.0);
    let header_rect_800 = dl_clamped
        .iter()
        .find_map(|cmd| match cmd {
            DisplayCommand::FillRect { rect, color }
                if *color == mango_core::Color::rgb(255, 0, 0) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("sticky header rect found at scroll 800");
    assert_eq!(
        header_rect_800.y(),
        650.0,
        "sticky header must be clamped within containing block bounds"
    );

    // 4. Verify rasterization / visual paint at scroll_y = 150
    let mut pixels = vec![0u32; (800 * 600) as usize];
    mango_render::paint(&dl_scrolled, &mut pixels, 800, 600);
    assert!(
        !pixels.iter().all(|&b| b == 0),
        "pixels buffer should be rasterized"
    );
}
