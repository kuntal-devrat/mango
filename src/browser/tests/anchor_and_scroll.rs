//! In-page anchor jumping and element `scrollIntoView()` integration tests.

use super::super::*;

#[test]
fn test_in_page_anchor_navigation_by_id() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0; padding: 0;">
            <div style="height: 1000px; background: red;">Spacer</div>
            <div id="section-target" style="height: 500px; background: green;">Target Section</div>
            <div style="height: 1000px; background: blue;">Footer Spacer</div>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "http://localhost/test.html".to_string());
    assert_eq!(browser.scroll_y, 0.0);

    // Navigate to in-page anchor by id
    browser.navigate("#section-target");
    assert_eq!(browser.scroll_y, 1000.0);
    assert_eq!(browser.scroll_y_f64, 1000.0);

    // Navigate to top
    browser.navigate("#top");
    assert_eq!(browser.scroll_y, 0.0);
    assert_eq!(browser.scroll_y_f64, 0.0);
}

#[test]
fn test_in_page_anchor_initial_load() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0; padding: 0;">
            <div style="height: 800px;">Spacer</div>
            <div id="heading-anchor" style="height: 300px;">Heading</div>
            <div style="height: 1000px;">Footer</div>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/page#heading-anchor".to_string(),
    );
    assert_eq!(browser.scroll_y, 800.0);
    assert_eq!(browser.scroll_y_f64, 800.0);
}

#[test]
fn test_in_page_anchor_by_anchor_name_and_percent_decoding() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0; padding: 0;">
            <div style="height: 600px;">Spacer</div>
            <a name="my target" style="display: block; height: 100px;">Named Anchor</a>
            <div style="height: 1200px;">Footer</div>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "http://localhost/doc".to_string());
    assert_eq!(browser.scroll_y, 0.0);

    // Navigate with percent-encoded space: %20 -> "my target"
    browser.navigate("#my%20target");
    assert_eq!(browser.scroll_y, 600.0);
    assert_eq!(browser.scroll_y_f64, 600.0);
}

#[test]
fn test_scroll_into_view_javascript() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0; padding: 0;">
            <div style="height: 1200px;">Tall Spacer</div>
            <div id="js-scroll-target" style="height: 200px;">JS Target</div>
            <div style="height: 1000px;">End Spacer</div>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "http://localhost/js-scroll".to_string());
    assert_eq!(browser.scroll_y, 0.0);

    // Call element.scrollIntoView() via script
    browser
        .execute_script("document.getElementById('js-scroll-target').scrollIntoView()")
        .unwrap();

    assert_eq!(browser.scroll_y, 1200.0);
    assert_eq!(browser.scroll_y_f64, 1200.0);
}

#[test]
fn test_scroll_into_view_smooth_behavior() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="margin: 0; padding: 0;">
            <div style="height: 900px;">Tall Spacer</div>
            <div id="smooth-target" style="height: 200px;">Smooth Target</div>
            <div style="height: 1000px;">End Spacer</div>
        </body>
        </html>
    "#;
    browser.load_html(html.to_string(), "http://localhost/smooth".to_string());
    assert_eq!(browser.scroll_y, 0.0);

    // Trigger smooth scrollIntoView
    browser
        .execute_script(
            "document.getElementById('smooth-target').scrollIntoView({ behavior: 'smooth' })",
        )
        .unwrap();

    assert!(browser.smooth_scroll.is_some());
    assert!(browser.has_pending_timers());

    // Advance smooth scroll animation
    std::thread::sleep(std::time::Duration::from_millis(30));
    let stepped = browser.tick_animations(50.0);
    assert!(stepped);
    assert!(browser.scroll_y > 0.0);

    // Advance to completion
    std::thread::sleep(std::time::Duration::from_millis(300));
    let _ = browser.tick_animations(300.0);
    assert_eq!(browser.scroll_y, 900.0);
}

#[test]
fn test_window_scroll_to_syncs_browser_scroll() {
    let mut browser = BrowserChrome::new(1024, 768);
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body style="height: 3000px; margin: 0;">
            <div>Page</div>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/window-scroll".to_string(),
    );
    assert_eq!(browser.scroll_y, 0.0);

    browser.execute_script("window.scrollTo(0, 450)").unwrap();
    assert_eq!(browser.scroll_y, 450.0);
    assert_eq!(browser.scroll_y_f64, 450.0);
}
