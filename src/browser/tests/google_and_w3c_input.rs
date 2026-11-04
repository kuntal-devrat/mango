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
