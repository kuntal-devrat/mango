//! Comprehensive unit tests for BrowserChrome and its subsystems.

use super::*;
use super::navigation::collect_stylesheet_images;
use mango_core::Color;
use mango_css::parse_stylesheet;
use mango_html::dom::{NodeData, NodeId};
use mango_html::parse_html;
use mango_platform::input::{KeyEvent, KeyState, MangoKey, MouseButton};
use mango_render::display_list::DisplayCommand;
use mango_render::get_cached_image;


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
                (rect.x() - (1024.0 - scrollbar_w + 2.0)).abs() < 2.0
                    && *radii == [4.0, 4.0, 4.0, 4.0]
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

    #[test]
    fn test_keyboard_page_navigation() {
        let mut browser = BrowserChrome::new(1024, 768);
        let html = r#"
            <html>
            <body style="height: 4000px; margin: 0;">
                <p>Very long page</p>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:keyboard_scroll".to_string());
        assert_eq!(browser.scroll_y, 0.0);

        // PageDown
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::PageDown,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        let scrolled_pagedown = browser.scroll_y;
        assert!(
            scrolled_pagedown > 400.0,
            "PageDown must scroll ~85% of viewport height"
        );

        // Space
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Space,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert!(
            browser.scroll_y > scrolled_pagedown,
            "Space must scroll down"
        );

        // Shift+Space
        let shift_mods = mango_platform::input::Modifiers {
            shift: true,
            ..Default::default()
        };
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Space,
            state: KeyState::Pressed,
            modifiers: shift_mods,
        });
        assert!(
            (browser.scroll_y - scrolled_pagedown).abs() < 2.0,
            "Shift+Space must scroll up"
        );

        // End
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::End,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(
            browser.scroll_y,
            browser.max_scroll(),
            "End must scroll to bottom"
        );

        // Home
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Home,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(browser.scroll_y, 0.0, "Home must scroll to top");
    }

    #[test]
    fn test_select_dropdown_open_and_select() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <select id="engine">
                    <option value="skia" selected="true">TinySkia</option>
                    <option value="sw">Software</option>
                </select>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:select".to_string());
        assert!(browser.select_dropdown.is_none());

        // Click select element
        browser.handle_mouse_move(30.0, HEADER_HEIGHT + 30.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        assert!(
            browser.select_dropdown.is_some(),
            "Clicking select must open select_dropdown"
        );
        let dropdown = browser.select_dropdown.as_ref().unwrap();
        assert_eq!(dropdown.options.len(), 2);
        assert_eq!(dropdown.options[0].text, "TinySkia");
        assert_eq!(dropdown.options[1].text, "Software");

        // Click second option
        let opt2_y = dropdown.y + 4.0 + dropdown.item_height + 4.0;
        browser.handle_mouse_move(dropdown.x + 20.0, opt2_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        assert!(
            browser.select_dropdown.is_none(),
            "Selecting option must close dropdown"
        );
        assert_eq!(browser.status_text, "Selected: Software");
    }

    #[test]
    fn test_details_summary_toggle() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <details id="my-details">
                    <summary id="my-summary">Advanced Settings</summary>
                    <p>Details content</p>
                </details>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:details".to_string());

        // Click summary to expand
        browser.handle_mouse_move(30.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let details_id = find_element_by_id(&doc, "my-details").unwrap();
        let el = match &doc.get(details_id).unwrap().data {
            NodeData::Element(e) => e.clone(),
            _ => panic!("Expected element"),
        };
        assert!(
            el.get_attribute("open").is_some(),
            "Clicking summary must open details"
        );

        // Click summary again to collapse
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let doc2 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let el2 = match &doc2.get(details_id).unwrap().data {
            NodeData::Element(e) => e.clone(),
            _ => panic!("Expected element"),
        };
        assert!(
            el2.get_attribute("open").is_none(),
            "Clicking summary again must close details"
        );
    }

    #[test]
    fn test_label_for_checkbox_toggle() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="checkbox" id="vector-dropdown-checkbox"/>
                <label for="vector-dropdown-checkbox">Main Menu</label>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:label".to_string());

        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let cb_id = find_element_by_id(&doc, "vector-dropdown-checkbox").unwrap();
        let is_checked = match &doc.get(cb_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("checked").is_some(),
            _ => false,
        };
        assert!(!is_checked, "Initially unchecked");

        // Click label
        browser.handle_mouse_move(45.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        let doc2 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let is_checked2 = match &doc2.get(cb_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("checked").is_some(),
            _ => false,
        };
        assert!(
            is_checked2,
            "Clicking label must toggle checkbox to checked"
        );
    }

    #[test]
    fn test_text_input_cursor_navigation_and_insert() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="text" id="q" value="hello"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:input".to_string());

        // Focus input at end of text (x=100.0 is inside input, past "hello")
        browser.handle_mouse_move(100.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.focused_control.is_some());
        assert_eq!(browser.control_cursor_pos, 5);

        // Click near the start (x=24.0) to reposition caret at beginning
        browser.handle_mouse_move(24.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.control_cursor_pos, 0);

        // Click again past the text (x=100.0) to place cursor back at end
        browser.handle_mouse_move(100.0, HEADER_HEIGHT + 25.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.control_cursor_pos, 5);

        // Move cursor 2 characters left
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowLeft,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowLeft,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(browser.control_cursor_pos, 3);

        // Type 'X' at position 3 ("hel" + "X" + "lo" -> "helXlo")
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('X'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });

        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let input_id = find_element_by_id(&doc, "q").unwrap();
        let val = match &doc.get(input_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("value").unwrap_or("").to_string(),
            _ => "".to_string(),
        };
        assert_eq!(val, "helXlo");
    }

    #[test]
    fn test_video_and_audio_browser_interaction() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 0;">
                <video id="player" src="video.mp4" controls width="300" height="150" style="display: block; margin: 0;"></video>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:media".to_string());

        // 1. Hover over the video controls play button (x=15, y = HEADER_HEIGHT + 135)
        let hovered = browser.handle_mouse_move(15.0, HEADER_HEIGHT + 135.0);
        assert!(
            hovered || browser.status_text.starts_with("Media <video>"),
            "Hovering video controls should update status text"
        );
        assert!(
            browser.status_text.contains("Media <video>"),
            "Status should indicate video media"
        );

        // 2. Left click play button -> toggles playing
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.status_text, "Video playing");
        let doc = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let vid_id = find_element_by_id(&doc, "player").unwrap();
        let is_playing = match &doc.get(vid_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("data-mango-playing") == Some("true"),
            _ => false,
        };
        assert!(
            is_playing,
            "Clicking play button should set data-mango-playing to true"
        );

        // 3. Left click play button again -> toggles paused
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.status_text, "Video paused");
        let doc2 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let is_paused = match &doc2.get(vid_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("data-mango-playing") == Some("false"),
            _ => false,
        };
        assert!(
            is_paused,
            "Clicking play button again should set data-mango-playing to false"
        );

        // 4. Right click -> opens context menu with Play and Mute actions
        browser.handle_mouse_move(50.0, HEADER_HEIGHT + 50.0);
        browser.handle_mouse_click(MouseButton::Right, KeyState::Pressed);
        assert!(
            browser.context_menu.is_some(),
            "Right click on video must open context menu"
        );
        let menu = browser.context_menu.as_ref().unwrap();
        assert!(
            menu.items
                .iter()
                .any(|i| i.label == "Play" || i.label == "Pause")
        );
        assert!(
            menu.items
                .iter()
                .any(|i| i.label == "Mute" || i.label == "Unmute")
        );

        // 5. Click Mute action on context menu
        let mute_idx = menu.items.iter().position(|i| i.label == "Mute").unwrap();
        let item_y = menu.y + 4.0 + (mute_idx as f32) * 26.0 + 10.0;
        browser.handle_mouse_move(menu.x + 20.0, item_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.status_text, "Media muted");
        let doc3 = browser.js_runtime.as_ref().unwrap().document_snapshot();
        let is_muted = match &doc3.get(vid_id).unwrap().data {
            NodeData::Element(e) => e.get_attribute("data-mango-muted") == Some("true"),
            _ => false,
        };
        assert!(
            is_muted,
            "Activating Mute from context menu should set data-mango-muted to true"
        );
    }

    #[test]
    fn test_stylesheet_image_collection_is_not_capped_at_64() {
        // OPT-009: `collect_stylesheet_images` used to hard-stop at 64 URLs,
        // silently dropping images on image-heavy pages.
        let doc = parse_html("<html><body><div class=\"a\"></div></body></html>");
        let mut css = String::new();
        for i in 0..120 {
            css.push_str(&format!(
                ".a {{ background-image: url(https://img.example/bg{i}.png); }}\n"
            ));
        }
        let sheet = parse_stylesheet(&css);
        let mut sources = Vec::new();
        collect_stylesheet_images(&doc, std::slice::from_ref(&sheet), &mut sources);
        assert_eq!(
            sources.len(),
            120,
            "every background image must be collected (was capped at 64)"
        );
    }

    #[test]
    fn test_background_image_backlog_drains_a_few_per_frame() {
        let mut browser = BrowserChrome::new(1024, 768);
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        browser.pending_image_fetches = (0..10).map(|_| png.to_string()).collect();
        browser.image_fetch_base = Some(Url::parse("https://example.com/").unwrap());
        assert!(browser.has_pending_image_fetches());

        assert!(
            browser.drain_pending_images(),
            "first drain caches a new image and should trigger a repaint"
        );
        assert_eq!(
            browser.pending_image_fetches.len(),
            10 - IMAGE_PREFETCH_PER_FRAME,
            "exactly a few images per frame"
        );
        assert!(get_cached_image(png).is_some(), "drained image was cached");

        while browser.has_pending_image_fetches() {
            browser.drain_pending_images();
        }
        assert!(
            !browser.has_pending_image_fetches(),
            "the backlog empties rather than being dropped"
        );
    }

    #[test]
    fn test_local_storage_profile_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mango_ls_{}", std::process::id()));
        let path = dir.join("localStorage.txt");
        let _ = std::fs::remove_file(&path);

        let store: mango_js::web_apis::SharedLocalStorage =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        store
            .lock()
            .unwrap()
            .insert("theme\tname".to_string(), "dark\nvalue".to_string());
        store.lock().unwrap().insert(
            "https://example.com\x1fuser_token".to_string(),
            "token_12345".to_string(),
        );
        save_local_storage(&path, &store);
        assert!(path.exists(), "profile file was written");

        let restored: mango_js::web_apis::SharedLocalStorage =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        load_local_storage(&path, &restored);
        assert_eq!(
            restored
                .lock()
                .unwrap()
                .get("theme\tname")
                .map(|s| s.as_str()),
            Some("dark\nvalue"),
            "tabs/newlines inside values survive the roundtrip"
        );
        assert_eq!(
            restored
                .lock()
                .unwrap()
                .get("https://example.com\x1fuser_token")
                .map(|s| s.as_str()),
            Some("token_12345"),
            "origin-scoped keys survive the roundtrip"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── GAP-024: <input type="range/number/color/date/time/file"> controls ──

    /// Helper: reads a node attribute from the live JS runtime.
    fn attr(browser: &BrowserChrome, node_id: NodeId, name: &str) -> Option<String> {
        browser
            .js_runtime
            .as_ref()
            .and_then(|rt| rt.get_node_attribute(node_id, name))
    }

    /// Helper: hit-tests the first form control at a document-space point.
    fn hit_control(browser: &BrowserChrome, x: f32, y: f32) -> mango_layout::FormControlHit {
        browser
            .root_box
            .as_ref()
            .and_then(|r| r.hit_test_form_control(mango_core::Point::new(x, y)))
            .expect("expected to hit a form control")
    }

    #[test]
    fn test_range_slider_click_drag_and_keyboard_step() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="range" id="vol" min="0" max="100" value="0" style="width: 200px; height: 24px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:range".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "range");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, cw, ch) = browser.control_content_rect(node_id).expect("range box");

        // Click at 75% across the track.
        browser.handle_mouse_move(cx + cw * 0.75, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("75"));
        assert_eq!(
            attr(&browser, node_id, "data-mango-value").as_deref(),
            Some("75"),
            "JS-facing data-mango-value mirrors the value attribute"
        );
        assert_eq!(
            browser.focused_control,
            Some(node_id),
            "slider takes keyboard focus so arrows can step it"
        );
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);

        // ArrowUp steps by `step` (default 1).
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowUp,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("76"));

        // Drag: press at 50%, move to 25%, release.
        browser.handle_mouse_move(cx + cw * 0.5, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("50"));
        browser.handle_mouse_move(cx + cw * 0.25, HEADER_HEIGHT + cy + ch / 2.0);
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some("25"),
            "dragging the thumb follows the mouse"
        );
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);

        // Rendering: the slider paints its blue thumb (GAP-024 paint check).
        let has_thumb = browser
            .build_display_list((800, 600))
            .into_iter()
            .any(|c| matches!(c, DisplayCommand::FillRoundedRect { color, .. } if color == Color::rgb(0, 120, 215)));
        assert!(has_thumb, "range slider thumb must be painted");
    }

    #[test]
    fn test_number_spinner_clicks_and_arrow_steps() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="number" id="n" value="4" min="0" max="10" step="2" style="width: 100px; height: 30px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:number".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "number");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, cw, ch) = browser.control_content_rect(node_id).expect("number box");

        // Up spinner (right edge, top half): 4 → 6.
        browser.handle_mouse_move(cx + cw - 8.0, HEADER_HEIGHT + cy + 4.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("6"));

        // Down spinner: 6 → 4.
        browser.handle_mouse_move(cx + cw - 8.0, HEADER_HEIGHT + cy + ch - 4.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("4"));

        // Click the text area to focus, then ArrowUp steps 4 → 6.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.focused_control, Some(node_id));
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::ArrowUp,
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("6"));

        // Spin up to the max: 6 → 8 → 10 → clamped at 10.
        browser.handle_mouse_move(cx + cw - 8.0, HEADER_HEIGHT + cy + 4.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("8"));
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("10"));
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some("10"),
            "value clamps at max"
        );

        // Rendering: spinner arrows are painted on the field.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "▲"), "up arrow painted");
        assert!(texts.iter().any(|t| t == "▼"), "down arrow painted");
    }

    #[test]
    fn test_color_picker_opens_and_picks_swatch() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r##"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="color" id="c" value="#000000" style="width: 120px; height: 26px; display: block;"/>
            </body>
            </html>
        "##;
        browser.load_html(html.to_string(), "test:color".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "color");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("color box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("color picker opens");
            assert!(matches!(picker.kind, PickerKind::Color));
            (picker.x, picker.y)
        };

        // The swatch hex label is painted inside the field.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "#000000"), "hex label painted");

        // Swatch index 6 is "#ff0000" (row 0, column 6).
        let sx = px
            + PICKER_SWATCH_PAD
            + 6.0 * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP)
            + PICKER_SWATCH_CELL / 2.0;
        let sy = py + PICKER_SWATCH_PAD + PICKER_SWATCH_CELL / 2.0;
        browser.handle_mouse_move(sx, sy);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

        assert!(browser.picker.is_none(), "picking closes the palette");
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("#ff0000"));
        assert_eq!(browser.status_text, "Color: #FF0000");
    }

    #[test]
    fn test_date_picker_calendar_navigation_and_selection() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="date" id="d" value="2026-03-10" style="width: 160px; height: 26px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:date".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "date");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("date box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("date picker opens");
            match picker.kind {
                PickerKind::Date { year, month } => assert_eq!((year, month), (2026, 3)),
                ref other => panic!("expected Date picker, got {:?}", other),
            }
            (picker.x, picker.y)
        };
        assert_eq!(browser.status_text, "Pick a date");

        // The calendar header shows the month/year.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "March 2026"),
            "month header painted"
        );
        assert!(texts.iter().any(|t| t == "15"), "day cell painted");

        // Click day 15 (March 2026 starts on a Sunday).
        let col_w = (210.0 - PICKER_DATE_PAD * 2.0) / 7.0;
        let lead = weekday_sunday0(days_from_civil(2026, 3, 1));
        let idx = lead + 15 - 1;
        let dx = px + PICKER_DATE_PAD + (idx % 7) as f32 * col_w + col_w / 2.0;
        let dy = py + PICKER_DATE_GRID_Y + (idx / 7) as f32 * PICKER_DATE_ROW_H + 10.0;
        browser.handle_mouse_move(dx, dy);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(
            browser.picker.is_none(),
            "picking a day closes the calendar"
        );
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some("2026-03-15")
        );
        assert_eq!(browser.status_text, "Date: 2026-03-15");

        // Reopen and navigate to the next month with the ">" header button.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("date picker reopens");
            (picker.x, picker.y)
        };
        browser.handle_mouse_move(px + 210.0 - 30.0 + 13.0, py + PICKER_DATE_HEADER_Y + 11.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser.picker.as_ref().expect("still open").kind {
            PickerKind::Date { year, month } => assert_eq!((*year, *month), (2026, 4)),
            other => panic!("expected Date picker, got {:?}", other),
        }

        // Clicking outside the panel dismisses it.
        browser.handle_mouse_move(10.0, HEADER_HEIGHT + 10.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.picker.is_none(), "outside click closes the picker");
    }

    #[test]
    fn test_time_picker_steppers_and_set() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="time" id="t" value="09:30" style="width: 140px; height: 26px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:time".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "time");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("time box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (px, py) = {
            let picker = browser.picker.as_ref().expect("time picker opens");
            assert!(matches!(picker.kind, PickerKind::Time { .. }));
            (picker.x, picker.y)
        };

        // Hour "+": 09 → 10.
        browser.handle_mouse_move(px + 123.0, py + PICKER_TIME_HOUR_Y + 12.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser.picker.as_ref().unwrap().kind {
            PickerKind::Time { hour, .. } => assert_eq!(*hour, 10),
            other => panic!("expected Time picker, got {:?}", other),
        }
        // Minute "-": 30 → 29.
        browser.handle_mouse_move(px + 27.0, py + PICKER_TIME_MIN_Y + 12.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser.picker.as_ref().unwrap().kind {
            PickerKind::Time { minute, .. } => assert_eq!(*minute, 29),
            other => panic!("expected Time picker, got {:?}", other),
        }

        // "Set" commits the value.
        browser.handle_mouse_move(px + 75.0, py + PICKER_TIME_SET_Y + 13.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.picker.is_none(), "Set closes the time picker");
        assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("10:29"));
        assert_eq!(browser.status_text, "Time: 10:29");
    }

    #[test]
    fn test_file_picker_lists_directory_and_selects() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="file" id="f" style="width: 260px; height: 28px; display: block;"/>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:file".to_string());

        // The unselected field renders a "Choose file..." button.
        let texts: Vec<String> = browser
            .build_display_list((800, 600))
            .into_iter()
            .filter_map(|c| match c {
                DisplayCommand::DrawText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "Choose file..."),
            "button painted"
        );
        assert!(
            texts.iter().any(|t| t == "No file chosen"),
            "placeholder painted"
        );

        let hit = hit_control(&browser, 30.0, 30.0);
        assert_eq!(hit.form_type, "file");
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("file box");

        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        let (root_dir, entries_len) = {
            let picker = browser.picker.as_ref().expect("file picker opens");
            match &picker.kind {
                PickerKind::File {
                    dir,
                    entries,
                    offset,
                } => {
                    assert!(!entries.is_empty(), "directory listing is populated");
                    assert_eq!(*offset, 0);
                    (dir.clone(), entries.len())
                }
                other => panic!("expected File picker, got {:?}", other),
            }
        };
        assert!(entries_len > 0);
        assert_eq!(browser.status_text, "Choose a file");

        // Pick the first file entry that fits in the visible first screen
        // (row 0 is the ".." parent row, so entry idx n sits at row n + 1).
        let file_pick = match &browser.picker.as_ref().unwrap().kind {
            PickerKind::File { entries, .. } => entries
                .iter()
                .enumerate()
                .find(|(i, e)| !e.is_dir && *i + 1 < PICKER_FILE_ROWS)
                .map(|(i, e)| (i, e.name.clone())),
            _ => None,
        };
        if let Some((idx, name)) = file_pick {
            let (px, py) = {
                let p = browser.picker.as_ref().unwrap();
                (p.x, p.y)
            };
            let row_y = py
                + PICKER_FILE_ROW_Y
                + (idx + 1) as f32 * PICKER_FILE_ROW_H
                + PICKER_FILE_ROW_H / 2.0;
            browser.handle_mouse_move(px + 60.0, row_y);
            browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
            assert!(browser.picker.is_none(), "picking a file closes the picker");
            assert_eq!(
                attr(&browser, node_id, "value").as_deref(),
                Some(name.as_str())
            );
            assert_eq!(
                attr(&browser, node_id, "data-mango-filename").as_deref(),
                Some(name.as_str())
            );
            let path = attr(&browser, node_id, "data-mango-filepath").unwrap();
            assert!(
                path.ends_with(&name),
                "data-mango-filepath points at the chosen file: {path}"
            );
            assert_eq!(browser.status_text, format!("Selected file: {}", name));

            // Reopen to exercise directory navigation.
            browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
            browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
            assert!(browser.picker.is_some(), "picker reopens");
        }

        // Navigate into the first directory (dirs sort first; row 0 is "..").
        let dir_name = match &browser.picker.as_ref().unwrap().kind {
            PickerKind::File { entries, .. } => entries
                .iter()
                .find(|e| e.is_dir)
                .map(|e| e.name.clone())
                .expect("repo contains at least one directory"),
            other => panic!("expected File picker, got {:?}", other),
        };
        let dir_idx = match &browser.picker.as_ref().unwrap().kind {
            PickerKind::File { entries, .. } => {
                entries.iter().position(|e| e.name == dir_name).unwrap()
            }
            _ => unreachable!(),
        };
        let (px, py) = {
            let p = browser.picker.as_ref().unwrap();
            (p.x, p.y)
        };
        let row_y = py
            + PICKER_FILE_ROW_Y
            + (dir_idx + 1) as f32 * PICKER_FILE_ROW_H
            + PICKER_FILE_ROW_H / 2.0;
        browser.handle_mouse_move(px + 60.0, row_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser
            .picker
            .as_ref()
            .expect("still open after entering dir")
            .kind
        {
            PickerKind::File {
                dir,
                entries,
                offset,
            } => {
                assert!(
                    dir.ends_with(&dir_name),
                    "entered {:?}, got {dir:?}",
                    dir_name
                );
                assert_eq!(*offset, 0);
                let _ = entries;
            }
            other => panic!("expected File picker, got {:?}", other),
        }

        // ".." (row 0) navigates back to the parent directory.
        let (px, py) = {
            let p = browser.picker.as_ref().unwrap();
            (p.x, p.y)
        };
        browser.handle_mouse_move(px + 60.0, py + PICKER_FILE_ROW_Y + PICKER_FILE_ROW_H / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        match &browser
            .picker
            .as_ref()
            .expect("still open after going up")
            .kind
        {
            PickerKind::File { dir, .. } => assert_eq!(dir, &root_dir, "back at the root"),
            other => panic!("expected File picker, got {:?}", other),
        }
    }

    #[test]
    fn test_input_and_change_events_reach_page_listeners() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="text" id="t" value="a" style="width: 150px; height: 24px; display: block;"/>
                <div id="pad" style="width: 300px; height: 40px; display: block;">pad</div>
                <script>
                    var el = document.getElementById('t');
                    el.addEventListener('input', function() {
                        el.setAttribute('data-hits', String(Number(el.getAttribute('data-hits') || '0') + 1));
                    });
                    el.addEventListener('change', function() {
                        el.setAttribute('data-changed', '1');
                    });
                </script>
            </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "test:events".to_string());

        let hit = hit_control(&browser, 30.0, 30.0);
        let node_id = hit.node_id.unwrap();
        let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("text box");

        // Focus and type two characters → two `input` events reach the listener.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert_eq!(browser.focused_control, Some(node_id));
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('b'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('c'),
            state: KeyState::Pressed,
            modifiers: mango_platform::input::Modifiers::default(),
        });
        assert_eq!(
            attr(&browser, node_id, "data-hits").as_deref(),
            Some("2"),
            "page listeners received both input events"
        );
        assert_eq!(
            attr(&browser, node_id, "value").map(|v| v.chars().count()),
            Some(3),
            "both characters were inserted"
        );
        assert_eq!(
            attr(&browser, node_id, "data-changed"),
            None,
            "no change yet"
        );

        // Clicking normal content blurs the field → one trailing `change` event.
        browser.handle_mouse_move(30.0, HEADER_HEIGHT + 70.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(
            browser.focused_control.is_none(),
            "focus cleared by content click"
        );
        assert_eq!(
            attr(&browser, node_id, "data-changed").as_deref(),
            Some("1"),
            "change listener fired exactly once on blur"
        );
    }

    #[test]
    fn test_browser_smooth_scroll_to() {
        let mut browser = BrowserChrome::new(800, 600);
        browser.load_html_internal(
            "<div style='height: 2000px;'>Long Content</div>".to_string(),
            "http://example.com".to_string(),
            false,
        );
        assert_eq!(browser.scroll_y, 0.0);

        browser.scroll_smooth_to(300.0, Some(std::time::Duration::from_millis(200)));
        assert!(browser.smooth_scroll.is_some());
        assert!(browser.has_pending_timers());

        // Step animation forward after small delay
        std::thread::sleep(std::time::Duration::from_millis(30));
        let stepped = browser.tick_animations(30.0);
        assert!(stepped);
        assert!(browser.scroll_y > 0.0);

        // Advance to completion
        std::thread::sleep(std::time::Duration::from_millis(210));
        let _ = browser.tick_animations(150.0);
        assert!(browser.scroll_y >= 290.0);
    }

    #[test]
    fn test_browser_dom_content_loaded_and_load_events() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script>
                    window.dcl_fired = false;
                    window.load_fired = false;
                    document.addEventListener('DOMContentLoaded', function() {
                        window.dcl_fired = true;
                        var el = document.getElementById('target');
                        if (el) el.setAttribute('data-dcl', 'yes');
                    });
                    window.addEventListener('load', function() {
                        window.load_fired = true;
                        var el = document.getElementById('target');
                        if (el) el.setAttribute('data-load', 'yes');
                    });
                </script>
            </head>
            <body>
                <div id="target">Content</div>
            </body>
            </html>
        "#;
        browser.load_html_internal(html.to_string(), "http://example.com".to_string(), false);

        let doc = browser.active_document();
        fn find_id(doc: &mango_html::Document, nid: NodeId, id: &str) -> Option<NodeId> {
            if let Some(node) = doc.get(nid)
                && let mango_html::NodeData::Element(el) = &node.data
                && el.get_attribute("id") == Some(id)
            {
                return Some(nid);
            }
            for ch in doc.children(nid) {
                if let Some(found) = find_id(doc, ch.id, id) {
                    return Some(found);
                }
            }
            None
        }
        let target_id = find_id(&doc, doc.root(), "target").expect("target found");
        assert_eq!(
            attr(&browser, target_id, "data-dcl").as_deref(),
            Some("yes"),
            "DOMContentLoaded event listener executed"
        );
        assert_eq!(
            attr(&browser, target_id, "data-load").as_deref(),
            Some("yes"),
            "window load event listener executed"
        );
    }

    #[test]
    fn test_find_in_page_and_shortcuts() {
        let mut browser = BrowserChrome::new(800, 600);
        let html = r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <p>Mango is a fast browser.</p>
                    <p>Another Mango instance here.</p>
                    <p>Third mango mention.</p>
                </body>
            </html>
        "#;
        browser.load_html(html.to_string(), "https://example.com/find".to_string());

        // 1. Test Ctrl+F toggle
        let ctrl_mod = mango_platform::input::Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let none_mod = mango_platform::input::Modifiers::default();

        assert!(!browser.find_in_page.active);
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('f'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert!(browser.find_in_page.active);

        // 2. Test search matches
        browser.find_in_page_search("Mango");
        assert_eq!(browser.find_in_page.matches.len(), 3);
        assert_eq!(browser.find_in_page.current_match, 0);

        // 3. Test find_next and find_previous
        browser.find_next();
        assert_eq!(browser.find_in_page.current_match, 1);
        browser.find_next();
        assert_eq!(browser.find_in_page.current_match, 2);
        browser.find_next();
        assert_eq!(browser.find_in_page.current_match, 0);
        browser.find_previous();
        assert_eq!(browser.find_in_page.current_match, 2);

        // 4. Test Escape closes find bar
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Escape,
            state: KeyState::Pressed,
            modifiers: none_mod,
        });
        assert!(!browser.find_in_page.active);

        // 5. Test Zoom shortcuts: Ctrl+=, Ctrl+-, Ctrl+0
        assert_eq!(browser.zoom_level, 1.0);
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('='),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert!((browser.zoom_level - 1.1).abs() < 1e-4);
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('-'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert!((browser.zoom_level - 1.0).abs() < 1e-4);
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('+'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('0'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert_eq!(browser.zoom_level, 1.0);

        // 6. Test F11 Fullscreen toggle
        assert!(!browser.is_fullscreen);
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::F11,
            state: KeyState::Pressed,
            modifiers: none_mod,
        });
        assert!(browser.is_fullscreen);
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::F11,
            state: KeyState::Pressed,
            modifiers: none_mod,
        });
        assert!(!browser.is_fullscreen);

        // 7. Test Tab shortcuts: Ctrl+T, Ctrl+W, Ctrl+L
        let initial_tabs = browser.tabs.len();
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('t'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert_eq!(browser.tabs.len(), initial_tabs + 1);

        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('l'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert!(browser.address_focused);

        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char('w'),
            state: KeyState::Pressed,
            modifiers: ctrl_mod,
        });
        assert_eq!(browser.tabs.len(), initial_tabs);

        // 8. Test display list and chrome rendering contains find bar when active
        browser.toggle_find();
        let chrome_dl = browser.build_chrome_display_list((800, 600));
        assert!(!chrome_dl.is_empty());
        let page_dl = browser.build_display_list((800, 600));
        assert!(!page_dl.is_empty());
    }

    #[test]
    fn test_queue_image_backlog_bounded_cap() {
        use crate::browser::images::MAX_PENDING_IMAGE_FETCHES;
        let mut browser = BrowserChrome::new(1024, 768);
        let urls: Vec<String> = (0..1000)
            .map(|i| format!("https://example.com/img{i}.png"))
            .collect();
        browser.queue_image_backlog(urls, Url::parse("https://example.com/").unwrap());
        assert!(browser.pending_image_fetches.len() <= MAX_PENDING_IMAGE_FETCHES);
        assert_eq!(browser.pending_image_fetches.len(), MAX_PENDING_IMAGE_FETCHES);
    }

