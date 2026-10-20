//! Multi-Window, Popups, and Navigation Openings Test Suite (Phase 6).
//!
//! Validates:
//! - Phase 6.1 & 6.4: WindowProxy, `window.open`, and popup creation.
//! - Phase 6.2: `target="_blank"` link navigation opening a new active tab.
//! - Phase 6.3: Middle-click auxiliary click on hyperlinks opening background tabs without stealing focus.
//! - Phase 6.5: `window.opener` reference and cross-window `postMessage` communication.

use mango_browser::browser::BrowserChrome;
use mango_browser::chrome_ui::HEADER_HEIGHT;
use mango_html::parse_html;
use mango_js::JsRuntime;
use mango_platform::input::{KeyState, MouseButton};

#[test]
fn test_window_proxy_and_cross_window_post_message() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head><title>Opener Window</title></head>
            <body><h1>Opener</h1></body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);

    let script = r##"
        (function() {
            if (typeof window.open !== 'function') throw new Error("window.open is not a function");

            // Open child popup
            var popup = window.open("https://example.com/auth", "_blank", "width=600,height=400");
            if (!popup) throw new Error("window.open returned null");
            if (popup.closed !== false) throw new Error("popup should not be closed");
            if (popup.opener !== window) throw new Error("popup.opener must point to opener window");

            globalThis.receivedMessages = [];

            // Add listener on popup
            popup.addEventListener("message", function(e) {
                globalThis.receivedMessages.push({
                    target: 'popup',
                    data: e.data,
                    origin: e.origin,
                    hasSource: (e.source === window)
                });
            });

            // Add listener on opener window
            window.addEventListener("message", function(e) {
                globalThis.receivedMessages.push({
                    target: 'opener',
                    data: e.data,
                    origin: e.origin,
                    hasSource: (e.source === window)
                });
            });

            // Post message to popup
            popup.postMessage({ token: "secret123" }, "*");

            // Post message to window
            window.postMessage("ping", "*");

            return "ok";
        })()
    "##;

    let res = rt.eval(script).expect("eval window proxy script");
    assert_eq!(res, "ok");

    // Verify pending window open op was captured
    let pending = rt.take_pending_open_window();
    assert!(pending.is_some(), "Pending window open must be captured");
    let open_req = pending.unwrap();
    assert_eq!(open_req.url, "https://example.com/auth");
    assert_eq!(open_req.target, "_blank");
    assert_eq!(open_req.features, "width=600,height=400");

    // Tick the event loop to deliver the deferred microtask postMessages
    rt.tick();

    let count_str = rt
        .eval("String(globalThis.receivedMessages.length)")
        .expect("eval count");
    assert_eq!(count_str, "2", "Both messages must be received");

    let popup_msg = rt
        .eval("JSON.stringify(globalThis.receivedMessages[0])")
        .expect("eval popup msg");
    assert!(
        popup_msg.contains("\"target\":\"popup\""),
        "First message should be delivered to popup"
    );
    assert!(
        popup_msg.contains("secret123"),
        "Message payload must be preserved"
    );

    let opener_msg = rt
        .eval("JSON.stringify(globalThis.receivedMessages[1])")
        .expect("eval opener msg");
    assert!(
        opener_msg.contains("\"target\":\"opener\""),
        "Second message should be delivered to opener"
    );
    assert!(
        opener_msg.contains("ping"),
        "Window message payload must be preserved"
    );
}

#[test]
fn test_target_blank_hyperlink_opens_new_tab() {
    let mut browser = BrowserChrome::new(1024, 768);
    browser.async_navigation = false;

    let html = r#"
        <!DOCTYPE html>
        <html>
            <head><title>Tab 1</title></head>
            <body style="margin: 0; padding: 0;">
                <a href="about:welcome" target="_blank" style="display: block; width: 200px; height: 50px;">
                    Open Welcome Tab
                </a>
            </body>
        </html>
    "#;

    browser.load_html(html.to_string(), "http://example.com/test.html".to_string());
    assert_eq!(browser.tab_count(), 1);
    assert_eq!(browser.active_tab_index(), 0);

    // Mouse move onto the link
    let link_x = 20.0f32;
    let link_y = HEADER_HEIGHT + 20.0f32;
    browser.handle_mouse_move(link_x, link_y);

    // Simulate Left click release
    browser.handle_mouse_click(MouseButton::Left, KeyState::Released);

    // Verify a new tab was opened and switched to
    assert_eq!(browser.tab_count(), 2, "A new tab should have opened");
    assert_eq!(
        browser.active_tab_index(),
        1,
        "The new tab should be active"
    );
    assert_eq!(
        browser.active_tab().unwrap().url,
        "about:welcome",
        "Active tab should navigate to target href"
    );
}

#[test]
fn test_middle_click_hyperlink_opens_background_tab() {
    let mut browser = BrowserChrome::new(1024, 768);
    browser.async_navigation = false;

    let html = r#"
        <!DOCTYPE html>
        <html>
            <head><title>Initial Tab</title></head>
            <body style="margin: 0; padding: 0;">
                <a href="about:welcome" style="display: block; width: 200px; height: 50px;">
                    Middle Click Link
                </a>
            </body>
        </html>
    "#;

    browser.load_html(
        html.to_string(),
        "http://example.com/initial.html".to_string(),
    );
    assert_eq!(browser.tab_count(), 1);
    assert_eq!(browser.active_tab_index(), 0);

    // Mouse move onto the link
    let link_x = 20.0f32;
    let link_y = HEADER_HEIGHT + 20.0f32;
    browser.handle_mouse_move(link_x, link_y);

    // Simulate Middle click release (auxiliary click)
    browser.handle_mouse_click(MouseButton::Middle, KeyState::Released);

    // Verify background tab was opened WITHOUT switching focus
    assert_eq!(browser.tab_count(), 2, "A background tab should be added");
    assert_eq!(
        browser.active_tab_index(),
        0,
        "Active tab must NOT change on middle click"
    );
    assert_eq!(
        browser.tabs()[1].url,
        "about:welcome",
        "Background tab should have target URL"
    );
}

#[test]
fn test_window_open_script_opens_tab() {
    let mut browser = BrowserChrome::new(1024, 768);
    browser.async_navigation = false;

    let html = r#"
        <!DOCTYPE html>
        <html>
            <head><title>Script Open Test</title></head>
            <body>
                <script>
                    window.open("about:welcome", "_blank");
                </script>
            </body>
        </html>
    "#;

    browser.load_html(
        html.to_string(),
        "http://example.com/script_open.html".to_string(),
    );

    // Verify script-initiated window.open created a new tab
    assert_eq!(
        browser.tab_count(),
        2,
        "Script window.open must spawn a new tab"
    );
    assert_eq!(
        browser.active_tab_index(),
        1,
        "The newly opened tab should be active"
    );
    assert_eq!(
        browser.active_tab().unwrap().url,
        "about:welcome",
        "New tab must have navigated to about:welcome"
    );
}
