//! Subsystem unit tests.

use super::super::navigation::resources::collect_stylesheet_images;
use super::super::*;
use super::{attr, hit_control};
use mango_core::Color;
use mango_css::parse_stylesheet;
use mango_html::dom::NodeData;
use mango_html::parse_html;
use mango_platform::input::{KeyEvent, KeyState, MangoKey, MouseButton};
use mango_render::display_list::DisplayCommand;
use mango_render::get_cached_image;

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
    // Verify `collect_stylesheet_images` does not prematurely stop at 64 URLs
    // on image-heavy pages.
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
    // Use distinct metadata keys so this test exercises batch draining rather
    // than the resource deduplicator removing ten identical URLs at once.
    browser.pending_image_fetches = (0..10)
        .map(|i| png.replacen("image/png", &format!("image/png;id={i}"), 1))
        .collect();
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
    let first_variant = png.replacen("image/png", "image/png;id=0", 1);
    assert!(
        get_cached_image(&first_variant).is_some(),
        "drained image was cached"
    );

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

// ── HTML5 interactive controls: <input type="range/number/color/date/time/file"> ──

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

    // Rendering: verify the slider paints its thumb
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
