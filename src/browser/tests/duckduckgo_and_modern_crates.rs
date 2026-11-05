//! Tests for DuckDuckGo form interactions, grapheme editing, word boundaries,
//! kurbo cubic-bezier evaluation, and IME composition.

use super::super::*;
use mango_platform::input::{KeyEvent, KeyState, MangoKey, Modifiers, MouseButton};

#[test]
fn test_duckduckgo_search_wrapper_click_focuses_input() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0; padding: 20px;">
            <form action="/" method="GET">
                <div class="searchbox_searchbox" style="padding: 10px; background: #eee; width: 500px;">
                    <div class="searchbox_inputWrap" style="padding: 5px;">
                        <span class="searchbox_icon" style="display: inline-block; width: 20px; height: 20px;">🔍</span>
                        <input type="text" name="q" id="searchbox_input" placeholder="Search without being tracked" style="width: 300px; height: 30px;" />
                    </div>
                </div>
            </form>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://duckduckgo.com/".to_string());

    // Hit-test on the searchbox wrapper or icon (outside the input's direct boundary)
    // Icon is at x ~ 35, y ~ 35
    browser.handle_mouse_move(35.0, HEADER_HEIGHT + 35.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

    assert!(
        browser.focused_control.is_some(),
        "Clicking search wrapper icon must focus the enclosed search input"
    );

    let node_id = browser.focused_control.unwrap();

    // Type "rust lang" into the focused input
    for ch in "rust lang".chars() {
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char(ch),
            state: KeyState::Pressed,
            modifiers: Modifiers::default(),
        });
    }

    let val = if let Some(ref rt) = browser.js_runtime {
        rt.get_node_attribute(node_id, "value")
            .or_else(|| rt.get_node_attribute(node_id, "data-mango-value"))
    } else {
        browser.cached_document.as_ref().and_then(|doc| {
            doc.get(node_id)
                .and_then(|n| n.as_element())
                .and_then(|el| el.get_attribute("value"))
                .map(|s| s.to_string())
        })
    };

    assert_eq!(
        val.as_deref(),
        Some("rust lang"),
        "Input should contain typed query 'rust lang'"
    );
}

#[test]
fn test_form_control_grapheme_and_word_navigation() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <input type="text" id="target" value="initial" style="width: 300px; height: 30px;" />
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://example.com/".to_string());

    // Focus input
    browser.handle_mouse_move(250.0, HEADER_HEIGHT + 30.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let node_id = browser.focused_control.expect("focused input");

    // Replace value with emoji-containing text: "Hello 🥭 world"
    browser.set_control_value(node_id, "Hello 🥭 world");
    browser.control_cursor_pos = "Hello 🥭 world".len();
    browser.sync_cursor_pos(node_id);

    // Ctrl+Backspace deletes "world"
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Backspace,
        state: KeyState::Pressed,
        modifiers: Modifiers {
            ctrl: true,
            ..Default::default()
        },
    });

    let val1 = browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(node_id, "value"))
        .unwrap();
    assert_eq!(val1, "Hello 🥭 ");

    // Backspace deletes space
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Backspace,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });

    // Backspace deletes entire multi-byte emoji "🥭" in a single stroke
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Backspace,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });

    let val2 = browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(node_id, "value"))
        .unwrap();
    assert_eq!(val2, "Hello ");

    // Ctrl+ArrowLeft jumps to start
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::ArrowLeft,
        state: KeyState::Pressed,
        modifiers: Modifiers {
            ctrl: true,
            ..Default::default()
        },
    });
    assert_eq!(browser.control_cursor_pos, 0);
}

#[test]
fn test_textarea_text_content_and_dynamic_value() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <textarea id="box" name="comment" style="width: 300px; height: 100px;">first line</textarea>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://example.com/".to_string());

    browser.handle_mouse_move(250.0, HEADER_HEIGHT + 30.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let node_id = browser.focused_control.expect("focused textarea");

    // Initial value must be synthesized from text content if value attr is missing
    assert_eq!(browser.control_cursor_pos, "first line".len());

    // Type a space and "second"
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Space,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    for ch in "second".chars() {
        browser.handle_key_event(&KeyEvent {
            key: MangoKey::Char(ch),
            state: KeyState::Pressed,
            modifiers: Modifiers::default(),
        });
    }

    let val = browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(node_id, "value"))
        .unwrap();
    assert_eq!(val, "first line second");
}

#[test]
fn test_ime_preedit_and_commit() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <html>
        <body style="margin: 0; padding: 20px;">
            <input type="text" id="target" value="" style="width: 300px; height: 30px;" />
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://example.com/".to_string());

    browser.handle_mouse_move(250.0, HEADER_HEIGHT + 30.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let node_id = browser.focused_control.expect("focused input");

    // IME Preedit composition updates status
    browser.handle_ime_preedit("toukyou", Some((0, 7)));
    assert!(browser.status_text.contains("toukyou"));

    // IME Commit inserts the committed text "東京"
    browser.handle_ime_commit("東京");

    let val = browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(node_id, "value"))
        .unwrap();
    assert_eq!(val, "東京");
    assert_eq!(browser.control_cursor_pos, "東京".len());
}

#[test]
fn test_cubic_bezier_kurbo_evaluation() {
    use mango_css::values::TimingFunction;

    let ease = TimingFunction::Ease;
    assert_eq!(ease.evaluate(0.0), 0.0);
    assert_eq!(ease.evaluate(1.0), 1.0);
    let mid = ease.evaluate(0.5);
    assert!(
        mid > 0.5 && mid < 1.0,
        "Ease curve at 0.5 should be > 0.5 (got {mid})"
    );

    let linear = TimingFunction::Linear;
    assert_eq!(linear.evaluate(0.35), 0.35);

    let custom = TimingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0);
    assert_eq!(custom.evaluate(0.0), 0.0);
    assert_eq!(custom.evaluate(1.0), 1.0);
    let v = custom.evaluate(0.5);
    assert!(v > 0.0 && v < 1.0);
}

#[test]
fn test_google_search_bar_layout_and_input() {
    let html = std::fs::read_to_string("/tmp/google.html").unwrap_or_else(|_| {
        r#"
        <!DOCTYPE html>
        <html>
        <head>
        <style>
        .A8SBwf { margin: 0 auto; max-width: 688px; width: auto; }
        .RNNXgb { display: flex; width: auto; border: 1px solid #dfe1e5; border-radius: 24px; height: 46px; }
        .SDkEP { display: flex; flex: 1; flex-wrap: wrap; }
        .iblpc { display: flex; align-items: center; padding-left: 14px; }
        .iblpc svg { width: 24px; height: 24px; }
        .a4bIc { display: flex; flex: 1; flex-wrap: wrap; }
        .gLFyf { width: 100%; height: 42px; border: none; outline: none; }
        .dRYYxd { display: flex; flex: 0 0 auto; align-items: center; }
        .M2vVEd svg { viewBox: "0 -960 960 960"; }
        </style>
        </head>
        <body>
          <div class="A8SBwf">
            <div class="RNNXgb">
              <div class="SDkEP">
                <div class="iblpc"><svg viewBox="0 0 24 24"><path d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/></svg></div>
                <div class="a4bIc"><textarea class="gLFyf" title="Search"></textarea></div>
                <div class="dRYYxd"><div class="M2vVEd"><svg viewBox="0 -960 960 960"><path d="M440-440H200v-80h240v-240h80v240h240v80H520v240h-80v-240Z"/></svg></div></div>
              </div>
            </div>
          </div>
        </body>
        </html>
        "#.to_string()
    });

    let mut browser = BrowserChrome::new(1366, 768);
    browser.load_html(html, "https://www.google.com/".to_string());
    let dl = browser.build_display_list((1366, 768));
    for cmd in dl.into_iter() {
        if let mango_render::display_list::DisplayCommand::DrawImage {
            x: _,
            y,
            width,
            height,
            ..
        } = cmd
        {
            // Inside search bar area (below logo at ~150-200px), icons must stay compact <= 48px
            if y > 250.0 && y < 350.0 {
                assert!(
                    width <= 48.0,
                    "SVG icon must not exceed 48px width in search bar (got {width})"
                );
                assert!(
                    height <= 48.0,
                    "SVG icon must not exceed 48px height in search bar (got {height})"
                );
            }
        }
    }

    // Test clicking the Google search textarea
    if let Some(ref root) = browser.root_box {
        fn find_textarea(b: &mango_layout::LayoutBox) -> Option<mango_core::Rect> {
            if b.tag_name.as_deref() == Some("textarea") {
                return Some(b.dimensions.content);
            }
            for child in &b.children {
                if let Some(r) = find_textarea(child) {
                    return Some(r);
                }
            }
            None
        }

        if let Some(r) = find_textarea(root) {
            assert!(
                r.width() > 300.0,
                "Google search textarea width must be > 300px (got {})",
                r.width()
            );
            let click_x = r.x() + 50.0;
            let click_y = crate::chrome_ui::HEADER_HEIGHT + r.y() + 20.0;
            browser.handle_mouse_move(click_x, click_y);
            browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);

            assert!(
                browser.focused_control.is_some(),
                "Textarea should be focused on click"
            );
            let node_id = browser.focused_control.unwrap();

            for ch in "rust language".chars() {
                browser.handle_key_event(&KeyEvent {
                    key: MangoKey::Char(ch),
                    state: KeyState::Pressed,
                    modifiers: Modifiers::default(),
                });
            }

            let val = browser
                .js_runtime
                .as_ref()
                .and_then(|rt| rt.get_node_attribute(node_id, "value"))
                .unwrap();
            assert_eq!(val, "rust language");
        }
    }
}
