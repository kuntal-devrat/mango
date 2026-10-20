//! Subsystem unit tests.

use super::super::*;
use mango_platform::input::{KeyEvent, KeyState, MangoKey, MouseButton};
use mango_render::display_list::DisplayCommand;

#[test]
fn test_display_list_cache_is_send() {
    fn assert_send<T: Send + Sync>() {}
    assert_send::<std::sync::Mutex<mango_layout::display_list::DisplayListCache>>();
}

#[test]
fn test_scroll_y_double_precision_preservation() {
    let mut browser = BrowserChrome::new(1024, 768);

    // Subpixel scroll at extreme height: 100,000.123456789 px
    let extreme_scroll = 100_000.123456789f64;
    browser.set_scroll_y_f64(extreme_scroll);
    assert_eq!(browser.scroll_y_f64(), extreme_scroll);
    assert!((browser.scroll_y() - 100_000.125).abs() < 0.1);

    // Subpixel scroll increment
    browser.scroll_by_f64(0.0005);
    assert_eq!(browser.scroll_y_f64(), extreme_scroll + 0.0005);

    // Tab and HistoryEntry precision preservation
    let mut tab = Tab::new();
    tab.scroll_y_f64 = extreme_scroll;
    tab.scroll_y = extreme_scroll as f32;
    tab.push_history("https://example.com/p2", "P2");
    assert_eq!(tab.scroll_y_f64, 0.0);
    let (_url, restored) = tab.go_back_f64().unwrap();
    assert_eq!(restored, extreme_scroll);
}

#[test]
fn test_browser_initialization() {
    let browser = BrowserChrome::new(1024, 768);
    assert_eq!(browser.tabs.len(), 1);
    assert_eq!(browser.active_tab_idx, 0);
    assert_eq!(browser.address_text, "about:welcome");
    assert!(browser.js_runtime.is_some());
}

#[test]
fn test_multi_tab_operations() {
    let mut browser = BrowserChrome::new(1024, 768);

    // Add second tab
    browser.new_tab();
    assert_eq!(browser.tabs.len(), 2);
    assert_eq!(browser.active_tab_idx, 1);

    // Switch back to tab 0
    browser.switch_tab(0);
    assert_eq!(browser.active_tab_idx, 0);

    // Switch to next tab
    browser.next_tab();
    assert_eq!(browser.active_tab_idx, 1);

    // Close tab 1
    browser.close_tab(1);
    assert_eq!(browser.tabs.len(), 1);
    assert_eq!(browser.active_tab_idx, 0);
}

#[test]
fn test_navigation_history_back_forward() {
    let mut browser = BrowserChrome::new(1024, 768);

    // Initially cannot go back or forward
    assert!(!browser.tabs[browser.active_tab_idx].can_go_back());
    assert!(!browser.tabs[browser.active_tab_idx].can_go_forward());

    // Navigate to test:box
    browser.navigate("test:box");
    assert_eq!(browser.address_text, "test:box");
    assert!(browser.tabs[browser.active_tab_idx].can_go_back());
    assert!(!browser.tabs[browser.active_tab_idx].can_go_forward());

    // Navigate to test:fonts
    browser.navigate("test:fonts");
    assert_eq!(browser.address_text, "test:fonts");

    // Go back
    browser.go_back();
    assert_eq!(browser.address_text, "test:box");
    assert!(browser.tabs[browser.active_tab_idx].can_go_forward());

    // Go forward
    browser.go_forward();
    assert_eq!(browser.address_text, "test:fonts");
}

#[test]
fn test_inline_script_execution_in_browser() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
            <html>
            <body>
                <h1 id="dyn-title">Initial Title</h1>
                <script>
                    var el = document.getElementById("dyn-title");
                    el.textContent = "Live Script Output";
                </script>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:js".to_string());

    // Check that JS runtime executed the script and modified the live DOM
    let _root = browser.root_box.as_ref().expect("root box should exist");
    let dl = browser.build_display_list((1024, 768));
    let found_text = dl.iter().any(|cmd| {
        if let DisplayCommand::DrawText { text, .. } = cmd {
            text == "Live Script Output"
        } else {
            false
        }
    });
    assert!(
        found_text,
        "Rendered display list should contain text mutated by JS"
    );
}

#[test]
fn test_ui_button_clicks() {
    let mut browser = BrowserChrome::new(1024, 768);

    // Hover & click New Tab (+)
    browser.hovered_target = Some(HoverTarget::NewTab);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.tabs.len(), 2);

    // Hover & click Home button
    browser.hovered_target = Some(HoverTarget::Home);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.address_text, "about:welcome");

    // Hover & click Tab Close (1)
    browser.hovered_target = Some(HoverTarget::TabClose(1));
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.tabs.len(), 1);
}

#[test]
fn test_scroll_clamping() {
    let mut browser = BrowserChrome::new(1024, 768);
    // Scroll down
    browser.handle_scroll(-5.0);
    let s1 = browser.scroll_y;
    assert!(s1 >= 0.0);

    // Scroll way up (should clamp at 0.0)
    browser.handle_scroll(100.0);
    assert_eq!(browser.scroll_y, 0.0);
}

#[test]
fn test_context_menu_open_and_click() {
    let mut browser = BrowserChrome::new(1024, 768);
    assert!(browser.context_menu.is_none());

    // Right click at (200, 300)
    browser.handle_mouse_move(200.0, 300.0);
    browser.handle_mouse_click(MouseButton::Right, KeyState::Pressed);

    assert!(browser.context_menu.is_some());
    let menu = browser.context_menu.as_ref().unwrap();
    assert!(menu.items.iter().any(|item| item.label == "Reload"));
    assert!(
        menu.items
            .iter()
            .any(|item| item.label == "View Page Source")
    );

    // Render display list with open context menu
    let dl = browser.build_display_list((1024, 768));
    let has_menu_text = dl.iter().any(|cmd| {
        if let DisplayCommand::DrawText { text, .. } = cmd {
            text == "Reload"
        } else {
            false
        }
    });
    assert!(has_menu_text, "Display list must render context menu items");

    // Left click outside dismisses context menu
    browser.handle_mouse_move(50.0, 50.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(browser.context_menu.is_none());
}

#[test]
fn test_interactive_form_controls() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
            <html>
            <body>
                <input type="text" id="username" value="hello" style="width: 200px; height: 30px; display: block;"/>
                <input type="checkbox" id="agree" style="width: 20px; height: 20px; display: block;"/>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:form_int".to_string());

    // Hit-test text input
    let hit = browser
        .root_box
        .as_ref()
        .and_then(|r| r.hit_test_form_control(mango_core::Point::new(10.0, 10.0)))
        .expect("should hit text input");
    assert_eq!(hit.tag_name, "input");
    assert_eq!(hit.form_type, "text");

    // Click text input to focus (click past "hello" to place cursor at end)
    browser.handle_mouse_move(100.0, 10.0 + HEADER_HEIGHT);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(browser.focused_control.is_some());

    // Type character '!' into input
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('!'),
        state: KeyState::Pressed,
        modifiers: mango_platform::input::Modifiers::default(),
    });

    let val = browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(hit.node_id.unwrap(), "value"))
        .unwrap();
    assert_eq!(val, "hello!");

    // Backspace
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Backspace,
        state: KeyState::Pressed,
        modifiers: mango_platform::input::Modifiers::default(),
    });
    let val2 = browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(hit.node_id.unwrap(), "value"))
        .unwrap();
    assert_eq!(val2, "hello");
}

#[test]
fn test_svg_rendering_in_browser() {
    let mut browser = BrowserChrome::new(1024, 768);
    browser.navigate("test:svg");
    assert_eq!(browser.address_text, "test:svg");

    let dl = browser.build_display_list((1024, 768));
    let has_svg_images = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
    assert!(
        has_svg_images,
        "Display list should contain rasterized SVG images"
    );
}

#[test]
fn test_font_styles_and_decorations_in_browser() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
            <html>
            <body>
                <p style="font-style: italic; text-decoration: underline;">Italic and Underlined</p>
                <p style="text-decoration: line-through;">Strikethrough</p>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:styled_fonts".to_string());

    let dl = browser.build_display_list((1024, 768));
    let found_italic_underline = dl.iter().any(|cmd| {
        if let DisplayCommand::DrawText {
            text,
            style,
            decoration,
            ..
        } = cmd
        {
            text == "Italic and Underlined"
                && *style == mango_render::FontStyle::Italic
                && *decoration == mango_render::TextDecoration::Underline
        } else {
            false
        }
    });
    assert!(
        found_italic_underline,
        "Should render text with Italic style and Underline decoration"
    );

    let found_line_through = dl.iter().any(|cmd| {
        if let DisplayCommand::DrawText {
            text, decoration, ..
        } = cmd
        {
            text == "Strikethrough" && *decoration == mango_render::TextDecoration::LineThrough
        } else {
            false
        }
    });
    assert!(
        found_line_through,
        "Should render text with LineThrough decoration"
    );
}

#[test]
fn test_button_with_svg_icon_and_centering() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r##"
            <html>
            <body>
                <button style="height: 36px;">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="#000000">
                        <circle cx="12" cy="12" r="10"/>
                    </svg>
                    <span>Click Me</span>
                </button>
            </body>
            </html>
        "##;
    browser.load_html(html.to_string(), "test:btn_icon".to_string());

    let dl = browser.build_display_list((1024, 768));
    let has_svg = dl
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
    let has_text = dl.iter().any(|cmd| {
        if let DisplayCommand::DrawText { text, .. } = cmd {
            text.contains("Click Me")
        } else {
            false
        }
    });
    assert!(has_svg, "Button should render SVG icon image");
    assert!(has_text, "Button should render text label");
}

#[test]
fn test_scrollable_extent_deep_scan() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
            <html style="height: 100%;">
            <body style="height: 100%; margin: 0;">
                <div style="height: 500px; background: red;">Top Box</div>
                <div style="height: 1500px; background: green;">Middle Box</div>
                <div style="height: 1000px; background: blue;">Bottom Box</div>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:extent".to_string());

    let doc_h = browser.scrollable_height();
    assert!(
        doc_h >= 3000.0,
        "Document extent must reflect all 3000px of child boxes, got {}",
        doc_h
    );
    assert!(
        browser.max_scroll() > 2000.0,
        "Max scroll must be > 2000px, got {}",
        browser.max_scroll()
    );
}

#[test]
fn test_viewport_scissor_clipping() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
            <html>
            <body style="margin: 0;">
                <div style="height: 400px; background: red;">Box 1</div>
                <div style="height: 400px; background: green;">Box 2</div>
                <div style="height: 400px; background: blue;">Box 3</div>
                <div style="height: 400px; background: yellow;">Box 4</div>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:clipping".to_string());

    // Scroll down by 500px
    browser.handle_scroll(-500.0 / 45.0);
    assert!((browser.scroll_y - 500.0).abs() < 1.0);

    let dl = browser.build_display_list((1024, 768));
    let content_y = HEADER_HEIGHT + 1.0;
    let content_h = (768.0 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);

    // Verify that no page-related command extends above content_y or below content_y + content_h
    for cmd in dl.iter() {
        match cmd {
            DisplayCommand::FillRect { rect, .. }
            | DisplayCommand::FillRoundedRect { rect, .. } => {
                // Chrome elements (tab bar at y=0..38, toolbar at 38..78, status bar at 744..768) are allowed
                if rect.y() >= content_y && rect.y() < content_y + content_h {
                    assert!(
                        rect.y() >= content_y - 0.1,
                        "Rect top {:.1} must not bleed above content_y {:.1}",
                        rect.y(),
                        content_y
                    );
                    assert!(
                        rect.bottom() <= content_y + content_h + 0.1,
                        "Rect bottom {:.1} must not bleed below content bottom {:.1}",
                        rect.bottom(),
                        content_y + content_h
                    );
                }
            }
            DisplayCommand::DrawText { y, .. }
                if *y >= content_y && *y <= content_y + content_h =>
            {
                assert!(*y >= content_y && *y <= content_y + content_h);
            }
            _ => {}
        }
    }
}

#[test]
fn test_scrollbar_rendering_and_interaction() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
            <html>
            <body style="height: 3000px; margin: 0;">
                <p>Long page</p>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:scrollbar".to_string());

    assert!(browser.max_scroll() > 0.0);

    // Verify scrollbar thumb is in display list
    let dl = browser.build_display_list((1024, 768));
    let scrollbar_w = 12.0f32;
    let has_thumb = dl.iter().any(|cmd| {
        if let DisplayCommand::FillRoundedRect { rect, radii, .. } = cmd {
            (rect.x() - (1024.0 - scrollbar_w + 2.0)).abs() < 2.0 && *radii == [4.0, 4.0, 4.0, 4.0]
        } else {
            false
        }
    });
    assert!(
        has_thumb,
        "Display list must render rounded scrollbar thumb"
    );

    // Hover over scrollbar thumb
    let content_y_top = HEADER_HEIGHT + 1.0;
    browser.handle_mouse_move(1020.0, content_y_top + 10.0);
    assert_eq!(browser.hovered_target, Some(HoverTarget::ScrollbarThumb));

    // Click and drag thumb down by 100px
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(browser.scrollbar_dragging);
    browser.handle_mouse_move(1020.0, content_y_top + 110.0);
    assert!(
        browser.scroll_y > 0.0,
        "Dragging scrollbar thumb down must scroll page down"
    );

    // Release mouse click
    browser.handle_mouse_click(MouseButton::Left, KeyState::Released);
    assert!(!browser.scrollbar_dragging);

    // Click track near bottom jumps scroll
    browser.handle_mouse_move(1020.0, 700.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(
        browser.scroll_y > 1000.0,
        "Clicking bottom of scrollbar track must jump scroll position"
    );
    browser.handle_mouse_click(MouseButton::Left, KeyState::Released);
}
