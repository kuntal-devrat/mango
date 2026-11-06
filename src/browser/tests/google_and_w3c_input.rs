//! Unit tests for Google Search form parity, textarea caret rendering,
//! W3C KeyboardEvent propagation, and IME commit.

use super::super::*;
use super::{attr, hit_control};
use mango_platform::input::{KeyEvent, KeyState, MangoKey, Modifiers, MouseButton};
use mango_render::display_list::DisplayCommand;

#[test]
fn test_google_search_textarea_caret_and_dynamic_value() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <form action="/search" method="GET">
                <div class="RNNXgb">
                    <textarea class="gLFyf" id="APjFqb" name="q" rows="1" style="width: 400px; height: 34px;">initial query</textarea>
                </div>
            </form>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://www.google.com/".to_string());

    // Before focus: no caret rendered
    let dl1 = browser.build_display_list((1024, 768));
    let has_initial_caret = dl1.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { rect, color } => {
            rect.width() <= 2.5 && color.a > 200 && rect.height() > 10.0
        }
        _ => false,
    });
    assert!(
        !has_initial_caret,
        "Unfocused textarea must not render caret"
    );

    // Hit-test and focus the textarea
    let hit = hit_control(&browser, 30.0, 30.0);
    assert_eq!(hit.form_type, "textarea");
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser
        .control_content_rect(node_id)
        .expect("textarea rect");

    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

    assert_eq!(browser.focused_control, Some(node_id));

    // Update dynamic value and position
    browser.set_control_value(node_id, "mango browser rust");
    if let Some(ref mut rt) = browser.js_runtime {
        rt.set_node_attribute(node_id, "_mango_cursor_pos", "5");
    }
    browser.relayout();

    // After focus: vertical caret bar must be rendered in display list
    let dl2 = browser.build_display_list((1024, 768));
    let has_caret = dl2.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { rect, color } => {
            rect.width() <= 2.5 && color.a > 200 && rect.height() > 10.0
        }
        _ => false,
    });
    assert!(has_caret, "Focused textarea must render vertical bar caret");

    // Display list must render the dynamic value
    let has_text = dl2.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("mango browser rust"),
        _ => false,
    });
    assert!(has_text, "Display list must render updated dynamic value");
}

#[test]
fn test_container_click_forwarding_and_w3c_keyboard_events() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <head>
            <style>
                body { margin: 0; padding: 0; }
                .RNNXgb { width: 500px; height: 50px; padding: 10px; }
                .gLFyf { width: 450px; height: 35px; }
            </style>
        </head>
        <body>
            <form action="/search" method="GET">
                <div class="RNNXgb" id="container">
                    <textarea class="gLFyf" id="APjFqb" name="q" rows="1"></textarea>
                </div>
            </form>
            <script>
                window.eventsFired = [];
                var ta = document.getElementById('APjFqb');
                ['keydown', 'keypress', 'input', 'keyup', 'focus', 'focusin'].forEach(function(evt) {
                    ta.addEventListener(evt, function(e) {
                        window.eventsFired.push({
                            type: e.type,
                            key: e.key,
                            code: e.code,
                            keyCode: e.keyCode
                        });
                    });
                });
            </script>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://www.google.com/".to_string());

    let hit = hit_control(&browser, 20.0, 20.0);
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser
        .control_content_rect(node_id)
        .expect("textarea rect");

    // Click inside the control
    browser.handle_mouse_move(cx + 10.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

    assert_eq!(browser.focused_control, Some(node_id));

    // Verify focus events fired to JS
    let focus_fired = browser
        .eval_devtools_console(
            "window.eventsFired.some(e => e.type === 'focus' || e.type === 'focusin')",
        )
        .1
        .unwrap();
    assert_eq!(focus_fired, "true");

    let active_id = browser
        .eval_devtools_console("document.activeElement ? document.activeElement.id : ''")
        .1
        .unwrap();
    assert_eq!(active_id, "APjFqb");

    // Type 'h'
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('h'),
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    browser.handle_key_release(&KeyEvent {
        key: MangoKey::Char('h'),
        state: KeyState::Released,
        modifiers: Modifiers::default(),
    });

    // Type 'i'
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('i'),
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    browser.handle_key_release(&KeyEvent {
        key: MangoKey::Char('i'),
        state: KeyState::Released,
        modifiers: Modifiers::default(),
    });

    let val = browser
        .eval_devtools_console("document.getElementById('APjFqb').value")
        .1
        .unwrap();
    assert_eq!(val, "hi");

    // Check keydown event count and attributes
    let kd_count = browser
        .eval_devtools_console("window.eventsFired.filter(e => e.type === 'keydown').length")
        .1
        .unwrap();
    assert_eq!(kd_count, "2");

    let ku_count = browser
        .eval_devtools_console("window.eventsFired.filter(e => e.type === 'keyup').length")
        .1
        .unwrap();
    assert_eq!(ku_count, "2");

    let first_kd_key = browser
        .eval_devtools_console("window.eventsFired.find(e => e.type === 'keydown').key")
        .1
        .unwrap();
    assert_eq!(first_kd_key, "h");

    // Backspace test
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Backspace,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    browser.handle_key_release(&KeyEvent {
        key: MangoKey::Backspace,
        state: KeyState::Released,
        modifiers: Modifiers::default(),
    });

    let val_after_bs = browser
        .eval_devtools_console("document.getElementById('APjFqb').value")
        .1
        .unwrap();
    assert_eq!(val_after_bs, "h");

    let total_kd = browser
        .eval_devtools_console("window.eventsFired.filter(e => e.type === 'keydown').length")
        .1
        .unwrap();
    assert_eq!(total_kd, "3");

    let total_ku = browser
        .eval_devtools_console("window.eventsFired.filter(e => e.type === 'keyup').length")
        .1
        .unwrap();
    assert_eq!(total_ku, "3");
}

#[test]
fn test_ime_commit_and_multiline_textarea() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <textarea id="multi" rows="5" style="width: 300px; height: 100px;"></textarea>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "test:ime".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("multi rect");

    browser.handle_mouse_move(cx + 10.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

    // Commit IME unicode text
    browser.handle_ime_commit("Mango 🥭 Engine");

    assert_eq!(
        attr(&browser, node_id, "value").as_deref(),
        Some("Mango 🥭 Engine")
    );

    // Enter in multiline textarea (rows=5): inserts newline
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Enter,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });

    assert_eq!(
        attr(&browser, node_id, "value").as_deref(),
        Some("Mango 🥭 Engine\n")
    );
}

#[test]
fn test_search_form_submission_on_enter() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <form action="/search" method="GET">
                <textarea id="APjFqb" name="q" rows="1" style="width: 300px; height: 30px;"></textarea>
            </form>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://example.com/start".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser
        .control_content_rect(node_id)
        .expect("textarea rect");

    browser.handle_mouse_move(cx + 10.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

    browser.handle_ime_commit("antigravity rust");

    // Enter on rows=1 textarea inside a form submits the form
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Enter,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });

    assert!(
        browser
            .address_text
            .contains("example.com/search?q=antigravity")
            || browser.address_text.contains("q=antigravity+rust")
            || browser.address_text.contains("q=antigravity%20rust")
            || browser.status_text.contains("Submitting"),
        "Enter in search box must trigger form submission: got address={}, status={}",
        browser.address_text,
        browser.status_text
    );
}

#[test]
fn test_keydown_prevent_default_cancels_character_insertion() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <input type="text" id="locked" value="start" style="width: 200px; height: 30px;">
            <script>
                document.getElementById('locked').addEventListener('keydown', function(e) {
                    if (e.key === 'x') {
                        e.preventDefault();
                    }
                });
            </script>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "test:prevent_default".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("input rect");

    browser.handle_mouse_move(cx + 10.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

    // Move cursor to end of input
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::End,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });

    // Type 'a' (not prevented)
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('a'),
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    assert_eq!(
        attr(&browser, node_id, "value").as_deref(),
        Some("starta"),
        "Key 'a' should be inserted"
    );

    // Type 'x' (prevented by e.preventDefault())
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('x'),
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    assert_eq!(
        attr(&browser, node_id, "value").as_deref(),
        Some("starta"),
        "Key 'x' should be cancelled by e.preventDefault()"
    );
}

#[test]
fn test_blur_and_focusout_reverts_active_element() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <input type="text" id="target_input" style="width: 200px; height: 30px;">
            <script>
                window.blurFired = false;
                document.getElementById('target_input').addEventListener('blur', function() {
                    window.blurFired = true;
                });
            </script>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "test:blur".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("input rect");

    browser.handle_mouse_move(cx + 10.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.focused_control, Some(node_id));

    // Clear focus (simulates clicking outside or Escape)
    browser.clear_form_focus();
    assert_eq!(browser.focused_control, None);

    let blur_fired = browser.eval_devtools_console("window.blurFired").1.unwrap();
    assert_eq!(blur_fired, "true", "blur event must fire on blur");

    let active_is_body_or_null = browser
        .eval_devtools_console(
            "document.activeElement === document.body || document.activeElement === null",
        )
        .1
        .unwrap();
    assert_eq!(active_is_body_or_null, "true");
}

#[test]
fn test_google_svg_sizing_in_flex_container_does_not_crush_input() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0;">
            <div style="display: flex; width: 600px; height: 44px; align-items: center;">
                <svg viewBox="0 0 24 24" style="height: 24px;"></svg>
                <input id="q" type="text" style="flex: 1; height: 34px;">
            </div>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://www.google.com/".to_string());
    let hit = hit_control(&browser, 100.0, 22.0);
    assert_eq!(hit.form_type, "text");
    let node_id = hit.node_id.unwrap();
    let (_cx, _cy, cw, _ch) = browser.control_content_rect(node_id).expect("input rect");
    assert!(
        cw > 300.0,
        "Input in flex container must not be crushed by SVG, cw={}",
        cw
    );
}

#[test]
fn test_indic_fallback_font_loading() {
    let fm = mango_render::FontManager::new();
    // Test Hindi ('ह' 0x0939), Bengali ('ব' 0x09AC), Telugu ('త' 0x0C24), Tamil ('த' 0x0BA4),
    // Gujarati ('ગ' 0x0A97), Kannada ('ಕ' 0x0C95), Malayalam ('മ' 0x0D2E), Gurmukhi ('ਪ' 0x0A2A)
    for ch in ['ह', 'ব', 'త', 'த', 'ગ', 'ಕ', 'മ', 'ਪ'] {
        let font = fm.select_font_for_char(
            mango_render::FontFamily::SansSerif,
            mango_render::FontWeight::Regular,
            ch,
        );
        let gid = font.lookup_glyph_index(ch);
        assert!(
            gid > 0,
            "Glyph for Indic character '{}' (U+{:04X}) must resolve to non-zero glyph index",
            ch,
            ch as u32
        );
    }
}

#[test]
fn test_form_typing_without_js_runtime_on_static_document() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <form action="/search" method="GET">
                <input type="text" id="duck_search" name="q" style="width: 300px; height: 30px;">
            </form>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "https://html.duckduckgo.com/html/".to_string(),
    );
    browser.js_runtime = None;

    let hit = hit_control(&browser, 30.0, 30.0);
    assert_eq!(hit.form_type, "text");
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("input rect");

    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.focused_control, Some(node_id));

    for c in "duck".chars() {
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char(c),
            state: KeyState::Pressed,
            modifiers: Modifiers::default(),
        });
    }

    assert_eq!(
        browser
            .cached_document
            .as_ref()
            .unwrap()
            .get(node_id)
            .and_then(|n| n.as_element())
            .and_then(|e| e.get_attribute("value")),
        Some("duck")
    );

    let dl = browser.build_display_list((1024, 768));
    let has_duck = dl.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::DrawText { text, .. } => text.contains("duck"),
        _ => false,
    });
    assert!(
        has_duck,
        "Display list must render typed text on static document without JS"
    );
}

#[test]
fn test_google_search_bar_elevation_rounded_clip_and_100dvh_layout() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <head>
            <style>
                body {
                    margin: 0;
                    padding: 0;
                    min-height: 100dvh;
                    display: flex;
                    flex-direction: column;
                }
                .main {
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                }
                .RNNXgb {
                    width: 584px;
                    height: 46px;
                    border-radius: 24px;
                    box-shadow: 0 1px 6px rgba(32,33,36,.28);
                    overflow: hidden;
                    display: flex;
                    align-items: center;
                    padding: 0 14px;
                    box-sizing: border-box;
                }
                #APjFqb {
                    flex: 1;
                    background: transparent;
                    border: none;
                    outline: none;
                    resize: none;
                    font-size: 16px;
                    height: 34px;
                }
            </style>
        </head>
        <body>
            <div class="main">
                <div class="RNNXgb">
                    <textarea id="APjFqb" name="q" rows="1"></textarea>
                </div>
            </div>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://www.google.com/".to_string());

    let dl = browser.build_display_list((1024, 768));

    // 1. Must emit elevation box shadow with 24px corner radii
    let shadow_cmd = dl.as_slice().iter().find(|cmd| match cmd {
        DisplayCommand::DrawBoxShadow {
            blur_radius, radii, ..
        } => *blur_radius >= 5.0 && radii == &[24.0, 24.0, 24.0, 24.0],
        _ => false,
    });
    assert!(
        shadow_cmd.is_some(),
        "Must emit DrawBoxShadow with 24px corner radii for search bar elevation"
    );

    // 2. Must emit PushClipRoundedRect with 24px corner radii for the overflow: hidden container
    let rounded_clip = dl.as_slice().iter().find(|cmd| match cmd {
        DisplayCommand::PushClipRoundedRect { radii, .. } => radii == &[24.0, 24.0, 24.0, 24.0],
        _ => false,
    });
    assert!(
        rounded_clip.is_some(),
        "Must emit PushClipRoundedRect with 24px radii for search bar container"
    );

    // 3. Textarea with border: none must NOT emit any DrawBorder command
    let has_unwanted_border = dl.as_slice().iter().any(|cmd| match cmd {
        DisplayCommand::DrawBorder { rect, .. } => rect.width() <= 584.0 && rect.height() <= 46.0,
        _ => false,
    });
    assert!(
        !has_unwanted_border,
        "Textarea with border: none must not emit DrawBorder bevels"
    );

    // 4. Hit-test and type into search textarea
    let (node_id, cx, cy, ch) = {
        let root = browser.root_box.as_ref().unwrap();
        let mut found = None;
        fn find_ta(
            b: &mango_layout::box_tree::LayoutBox,
            out: &mut Option<(mango_html::dom::NodeId, f32, f32, f32)>,
        ) {
            if b.tag_name.as_deref() == Some("textarea") {
                if let Some(nid) = b.node_id {
                    let c = b.dimensions.content;
                    *out = Some((nid, c.x(), c.y(), c.height()));
                    return;
                }
            }
            for child in &b.children {
                find_ta(child, out);
            }
        }
        find_ta(root, &mut found);
        found.expect("textarea in box tree")
    };

    let hit = hit_control(&browser, cx + 10.0, cy + ch / 2.0);
    assert_eq!(hit.form_type, "textarea");
    assert_eq!(hit.node_id, Some(node_id));

    browser.handle_mouse_move(cx + 10.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.focused_control, Some(node_id));

    browser.handle_ime_commit("Google Search Parity");
    assert_eq!(
        attr(&browser, node_id, "value").as_deref(),
        Some("Google Search Parity")
    );
}

#[test]
fn test_google_offered_in_multilingual_script_measurement_and_rendering() {
    let text = "Google offered in: हिन्दी বাংলা తెలుగు मराठी தமிழ் ગુજરાતી ಕನ್ನಡ മലയാളം ਪੰਜਾਬੀ";
    let fm = mango_render::font_manager();
    let (w, h) = fm.measure_text(
        text,
        13.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    );
    assert!(
        w > 200.0,
        "Multilingual text width must be measured, w={}",
        w
    );
    assert!(
        h >= 13.0,
        "Multilingual text height must be positive, h={}",
        h
    );

    // Rasterize text into a test buffer to ensure no panic and non-empty glyphs
    let mut buf = vec![0u32; 1000 * 50];
    fm.rasterize_text_clipped(
        &mut buf,
        1000,
        50,
        0,
        10,
        text,
        Color::RED,
        13.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
        mango_render::FontStyle::Normal,
        0.0,
        mango_core::Rect::new(0.0, 0.0, 1000.0, 50.0),
    );
    let non_zero_pixels = buf.iter().filter(|&&b| b > 0).count();
    assert!(
        non_zero_pixels > 100,
        "Rendered Indic text must produce non-zero glyph pixels, got {}",
        non_zero_pixels
    );
}

#[test]
fn test_omnibox_url_clipping_bounds() {
    let mut browser = BrowserChrome::new(1024, 768);
    browser.address_text = "https://html.duckduckgo.com/html/?q=super+long+search+query+that+extends+way+past+the+address+bar+bounds+and+should+be+clipped".to_string();
    let dl = browser.build_display_list((1024, 768));

    let mut has_push_clip = false;
    let mut has_pop_clip = false;
    for cmd in dl.as_slice() {
        match cmd {
            DisplayCommand::PushClip { rect } => {
                if rect.y() < HEADER_HEIGHT && rect.width() > 100.0 {
                    has_push_clip = true;
                }
            }
            DisplayCommand::PopClip => {
                has_pop_clip = true;
            }
            _ => {}
        }
    }
    assert!(has_push_clip, "Omnibox text must be bounded by PushClip");
    assert!(has_pop_clip, "Omnibox text must terminate with PopClip");
}
