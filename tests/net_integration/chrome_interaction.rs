use mango_core::Color;

#[test]
fn test_hover_and_active_state_triggers_relayout_and_styling() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1024, 768);
    let html = r##"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                a { display: block; width: 200px; height: 50px; text-decoration: none; color: #0000ff; }
                a:hover { text-decoration: underline; color: #ff0000; }
                a:active { color: #00ff00; }
            </style>
        </head>
        <body>
            <a href="#testlink" id="testlink">Click me</a>
        </body>
        </html>
    "##;
    browser.load_html(html.to_string(), "https://example.com/".to_string());

    // Before hover: link is blue (#0000ff), not underlined
    let root = browser.root_box().expect("root box");
    let link_box = root
        .children
        .iter()
        .find(|c| c.tag_name.as_deref() == Some("body"))
        .and_then(|b| {
            b.children
                .iter()
                .find(|c| c.tag_name.as_deref() == Some("a"))
        })
        .expect("a element");
    let style_before = link_box.style.as_ref().unwrap();
    assert_eq!(
        style_before.text_decoration,
        mango_css::values::TextDecoration::None
    );
    assert_eq!(style_before.color, Color::rgba(0, 0, 255, 255));

    // Move mouse over the link (content y starts below HEADER_HEIGHT 81.0)
    let moved = browser.handle_mouse_move(50.0, 100.0);
    assert!(
        moved,
        "handle_mouse_move should return true when hover state changes"
    );

    // After hover: link is red (#ff0000), underlined
    let root = browser.root_box().expect("root box");
    let link_box = root
        .children
        .iter()
        .find(|c| c.tag_name.as_deref() == Some("body"))
        .and_then(|b| {
            b.children
                .iter()
                .find(|c| c.tag_name.as_deref() == Some("a"))
        })
        .expect("a element");
    let style_hovered = link_box.style.as_ref().unwrap();
    assert_eq!(
        style_hovered.text_decoration,
        mango_css::values::TextDecoration::Underline
    );
    assert_eq!(style_hovered.color, Color::rgba(255, 0, 0, 255));

    // Press mouse: :active styling activates (#00ff00)
    browser.handle_mouse_click(
        mango_platform::input::MouseButton::Left,
        mango_platform::input::KeyState::Pressed,
    );
    let root = browser.root_box().expect("root box");
    let link_box = root
        .children
        .iter()
        .find(|c| c.tag_name.as_deref() == Some("body"))
        .and_then(|b| {
            b.children
                .iter()
                .find(|c| c.tag_name.as_deref() == Some("a"))
        })
        .expect("a element");
    let style_active = link_box.style.as_ref().unwrap();
    assert_eq!(style_active.color, Color::rgba(0, 255, 0, 255));

    // Release mouse: returns to :hover styling (#ff0000)
    browser.handle_mouse_click(
        mango_platform::input::MouseButton::Left,
        mango_platform::input::KeyState::Released,
    );
    let root = browser.root_box().expect("root box");
    let link_box = root
        .children
        .iter()
        .find(|c| c.tag_name.as_deref() == Some("body"))
        .and_then(|b| {
            b.children
                .iter()
                .find(|c| c.tag_name.as_deref() == Some("a"))
        })
        .expect("a element");
    let style_after_release = link_box.style.as_ref().unwrap();
    assert_eq!(style_after_release.color, Color::rgba(255, 0, 0, 255));

    // Move mouse away to window top (header)
    let moved_away = browser.handle_mouse_move(10.0, 10.0);
    assert!(
        moved_away,
        "handle_mouse_move should return true when unhovering"
    );
    let root = browser.root_box().expect("root box");
    let link_box = root
        .children
        .iter()
        .find(|c| c.tag_name.as_deref() == Some("body"))
        .and_then(|b| {
            b.children
                .iter()
                .find(|c| c.tag_name.as_deref() == Some("a"))
        })
        .expect("a element");
    let style_after_leave = link_box.style.as_ref().unwrap();
    assert_eq!(
        style_after_leave.text_decoration,
        mango_css::values::TextDecoration::None
    );
    assert_eq!(style_after_leave.color, Color::rgba(0, 0, 255, 255));
}

#[test]
fn test_duckduckgo_background_image_parsing_and_drain() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .logo_homepage {
                    display: block;
                    width: 205px;
                    height: 200px;
                    background: no-repeat center url("/assets/logo_homepage.normal.v109.png");
                    background: no-repeat center/100% url("/assets/logo_homepage.normal.v109.svg"), linear-gradient(transparent, transparent);
                }
            </style>
        </head>
        <body>
            <a class="logo_homepage" href="/about.html">About</a>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "https://duckduckgo.com/html/".to_string());
    let root = browser.root_box().expect("root box");
    let logo_box = root
        .children
        .iter()
        .find(|c| c.tag_name.as_deref() == Some("body"))
        .and_then(|b| {
            b.children
                .iter()
                .find(|c| c.tag_name.as_deref() == Some("a"))
        })
        .expect("logo element");
    let style = logo_box.style.as_ref().unwrap();
    assert_eq!(
        style.background_image.as_deref(),
        Some("/assets/logo_homepage.normal.v109.svg"),
        "multi-background shorthand should preserve the primary url() as background_image"
    );
}
