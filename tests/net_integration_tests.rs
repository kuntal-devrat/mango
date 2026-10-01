//! Integration tests for Mango's Networking & Resource Fetching pipeline.

use mango_core::{Color, Size};
use mango_css::parse_stylesheet;
use mango_html::parse_html;
use mango_layout::layout_document;
use mango_net::{HttpClient, ResourceLoader, Url};
use mango_render::display_list::DisplayCommand;
use mango_render::{cache_image, decode_image_bytes, get_cached_image};

#[test]
fn test_external_stylesheet_integration_pipeline() {
    let loader = ResourceLoader::new();
    let base_url = Url::parse("https://example.com/index.html").unwrap();
    let css_url = "https://example.com/theme.css";

    // Simulate pre-cached or downloaded stylesheet from network
    let css_content = r#"
        body { background-color: #ffffff; }
        .hero {
            background-color: #ffa136;
            margin-bottom: 25px;
            padding: 10px;
        }
        h1 {
            color: #123456;
            font-size: 20px;
        }
    "#;

    loader.cache().insert(
        css_url,
        "text/css",
        200,
        std::collections::HashMap::new(),
        css_content.as_bytes().to_vec(),
        None,
    );

    // Fetch external stylesheet via loader
    let fetched_css = loader
        .fetch_stylesheet(&base_url, "theme.css")
        .expect("fetch stylesheet");
    let sheet = parse_stylesheet(&fetched_css);

    // HTML document that references the hero class
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>External CSS Test</title></head>
        <body>
            <div class="hero">
                <h1>Hello Mango</h1>
            </div>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let author_sheets = vec![&sheet];
    let (_root_box, display_list) = layout_document(&doc, &author_sheets, viewport);

    // Verify that display list contains the background color from the external CSS rule
    let has_hero_bg = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, .. } => *color == Color::rgb(255, 161, 54),
        _ => false,
    });
    assert!(
        has_hero_bg,
        "Display list should include hero background from external stylesheet"
    );
}

#[test]
fn test_remote_image_download_and_layout_integration() {
    // 1x1 transparent PNG bytes
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let decoded = decode_image_bytes(&png_bytes).expect("decode valid PNG bytes");
    let image_url = "https://example.com/assets/logo.png";
    cache_image(image_url, decoded);

    assert!(get_cached_image(image_url).is_some());

    // Lay out HTML with remote image
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <img src="https://example.com/assets/logo.png" width="64" height="64">
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let viewport = Size::new(800.0, 600.0);
    let (_root_box, display_list) = layout_document(&doc, &[], viewport);

    // Verify that DisplayCommand::DrawImage was emitted
    let has_image_cmd = display_list.iter().any(|cmd| match cmd {
        DisplayCommand::DrawImage { width, height, .. } => *width == 64.0 && *height == 64.0,
        _ => false,
    });
    assert!(
        has_image_cmd,
        "Layout should emit DrawImage for cached remote image"
    );
}

#[test]
fn test_live_local_http_server_and_asset_pipeline() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    let server_handle = thread::spawn(move || {
        // Accept up to 2 incoming requests: document and stylesheet
        for _ in 0..2 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let req_str = String::from_utf8_lossy(&buf);

                if req_str.contains("GET /style.css") {
                    let body = "body { background-color: #ffa136; } h1 { font-size: 24px; }";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                } else {
                    let body = r#"<!DOCTYPE html><html><head><link rel="stylesheet" href="/style.css"></head><body><h1>Live Test</h1></body></html>"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
        }
    });

    let loader = ResourceLoader::new();
    let base_url = Url::parse(&format!("http://127.0.0.1:{}/", port)).unwrap();

    // 1. Fetch document from real live HTTP server over TCP socket
    let doc_res = loader
        .fetch_document(&base_url)
        .expect("fetch live document");
    assert_eq!(doc_res.status, 200);
    assert!(doc_res.html.contains("Live Test"));

    // 2. Fetch external stylesheet from live server over TCP socket
    let css_res = loader
        .fetch_stylesheet(&base_url, "style.css")
        .expect("fetch live css");
    assert!(css_res.contains("#ffa136"));
    let sheet = parse_stylesheet(&css_res);

    // 3. Layout with external stylesheet
    let doc = parse_html(&doc_res.html);
    let viewport = Size::new(800.0, 600.0);
    let author_sheets = vec![&sheet];
    let (_root_box, dl) = layout_document(&doc, &author_sheets, viewport);

    assert!(!dl.is_empty());
    let has_orange_bg = dl.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, .. } => *color == Color::rgb(255, 161, 54),
        _ => false,
    });
    assert!(
        has_orange_bg,
        "Live fetched external stylesheet applied to layout"
    );

    let _ = server_handle.join();
}

#[test]
#[ignore = "requires live internet connection"]
fn test_live_website_fetch_neverssl_or_example_com() {
    let mut client = HttpClient::new();
    client.timeout = std::time::Duration::from_secs(5);
    let target_url = Url::parse("http://neverssl.com/").unwrap();

    match client.fetch(&target_url) {
        Ok(resp) => {
            println!("Live fetch success! Status: {}", resp.status);
            assert_eq!(resp.status, 200);
            let html = resp.body_as_string();
            println!(
                "Fetched {} bytes of HTML: {:?}",
                html.len(),
                &html[..html.len().min(200)]
            );
            assert!(!html.is_empty(), "HTML body should not be empty");

            // Parse live HTML into Mango DOM
            let doc = parse_html(&html);
            let viewport = Size::new(800.0, 600.0);
            let (_root_box, display_list) = layout_document(&doc, &[], viewport);
            assert!(
                !display_list.is_empty(),
                "Display list should be non-empty for live website"
            );
        }
        Err(e) => {
            println!(
                "Notice: live website fetch skipped or network unreachable: {}",
                e
            );
        }
    }
}

#[test]
#[ignore = "requires live internet connection"]
fn test_live_website_cern_historical_first_website() {
    let mut client = HttpClient::new();
    client.timeout = std::time::Duration::from_secs(5);
    let target_url = Url::parse("http://info.cern.ch/hypertext/WWW/TheProject.html").unwrap();

    match client.fetch(&target_url) {
        Ok(resp) => {
            println!("CERN live fetch success! Status: {}", resp.status);
            assert_eq!(resp.status, 200);
            let html = resp.body_as_string();
            assert!(html.contains("WorldWideWeb"));

            let doc = parse_html(&html);
            let viewport = Size::new(800.0, 600.0);
            let (_root_box, display_list) = layout_document(&doc, &[], viewport);
            assert!(!display_list.is_empty());
        }
        Err(e) => {
            println!("Notice: CERN fetch skipped or network unreachable: {}", e);
        }
    }
}

#[test]
fn test_example_com_rendering_structure() {
    let html = r#"<!doctype html><html><head><title>Example</title><style>
    body {
        background: #eee;
        width: 60vw;
        margin: 15vh auto;
        padding: 2em;
        background-color: #fff;
    }
    h1 { font-size: 1.5em; margin-bottom: 0.5em; }
    p { margin-top: 0.5em; margin-bottom: 0.5em; }
    a:link, a:visited { color: #348; }
    </style></head><body><div><h1>Example Domain</h1><p>This domain is for illustrative examples.</p><p><a href="https://example.com">More...</a></p></div></body></html>"#;
    let doc = parse_html(html);
    let (root_box, dl) = layout_document(&doc, &[], Size::new(800.0, 600.0));

    // The body element is the child of html
    let body_box = &root_box.children[0];
    assert!(
        (body_box.dimensions.content.width() - 480.0).abs() < 0.1,
        "60vw of 800px should be 480px"
    );
    assert!(
        (body_box.dimensions.margin.top - 90.0).abs() < 0.1,
        "15vh of 600px should be 90px"
    );
    assert!(
        (body_box.dimensions.margin.left - 128.0).abs() < 0.1,
        "Auto margins should center the card with 2em padding: (800 - 544)/2 = 128px"
    );
    assert!(
        (body_box.dimensions.content.x() - 160.0).abs() < 0.1,
        "Content origin X should be margin_left (128) + padding_left (32) = 160px"
    );

    // Verify link color #348 is applied via :link pseudo-class
    let link_cmd = dl
        .iter()
        .find(|cmd| matches!(cmd, DisplayCommand::DrawText { text, .. } if text == "More..."));
    assert!(
        link_cmd.is_some(),
        "Link text 'More...' should be present in display list"
    );
    if let Some(DisplayCommand::DrawText { color, .. }) = link_cmd {
        assert_eq!(
            *color,
            Color::rgb(51, 68, 136),
            "#348 should be rgb(51, 68, 136)"
        );
    }

    // Verify h1 heading is drawn with FontWeight::Bold
    let h1_cmd = dl.iter().find(
        |cmd| matches!(cmd, DisplayCommand::DrawText { text, .. } if text.contains("Example")),
    );
    assert!(
        h1_cmd.is_some(),
        "H1 heading text should be present in display list"
    );
    if let Some(DisplayCommand::DrawText {
        weight, font_size, ..
    }) = h1_cmd
    {
        assert_eq!(
            *weight,
            mango_render::FontWeight::Bold,
            "H1 text must be rendered bold"
        );
        assert_eq!(*font_size, 24.0, "1.5em of 16px body font is 24px");
    }
}

#[test]
fn test_cern_and_httpbin_typography_and_link_hittest() {
    use mango_core::Point;
    use mango_render::{FontFamily, FontWeight};

    let html = r#"<!DOCTYPE html>
    <html>
    <head><title>Typography & Links</title></head>
    <body>
        <h1>Herman Melville - Moby-Dick</h1>
        <p><a href="http://info.cern.ch/hypertext/WWW/TheProject.html">Browse the first website</a></p>
    </body>
    </html>"#;

    let doc = parse_html(html);
    let (root_box, dl) = layout_document(&doc, &[], Size::new(800.0, 600.0));

    // 1. Heading verification: consolidated run with intact spaces, bold weight, and default Serif font
    let heading_cmd = dl.iter().find(|cmd| matches!(cmd, DisplayCommand::DrawText { text, .. } if text.contains("Herman Melville")));
    assert!(heading_cmd.is_some(), "Heading text run must be present");
    if let Some(DisplayCommand::DrawText {
        text,
        weight,
        family,
        ..
    }) = heading_cmd
    {
        assert_eq!(
            text, "Herman Melville - Moby-Dick",
            "Heading text run must contain full text with proper spacing"
        );
        assert_eq!(
            *weight,
            FontWeight::Bold,
            "Heading should be rendered with Bold font weight"
        );
        assert_eq!(
            *family,
            FontFamily::Serif,
            "Default UA stylesheet specifies serif for body/headings"
        );
    }

    // 2. Link verification: blue color and default Serif font
    let link_cmd = dl.iter().find(|cmd| matches!(cmd, DisplayCommand::DrawText { text, .. } if text.contains("Browse the first website")));
    assert!(link_cmd.is_some(), "Link text run must be present");
    let mut link_coords = (0.0, 0.0);
    if let Some(DisplayCommand::DrawText {
        text,
        color,
        family,
        x,
        y,
        ..
    }) = link_cmd
    {
        assert_eq!(text, "Browse the first website");
        assert_eq!(
            *color,
            Color::rgb(0, 0, 238),
            "Unvisited link should have default browser blue color"
        );
        assert_eq!(*family, FontFamily::Serif);
        link_coords = (*x, *y);
    }

    // 3. Link hit-testing verification: clicking on the link text resolves the correct href
    let hit = root_box.hit_test_link(Point::new(link_coords.0 + 10.0, link_coords.1 + 4.0));
    assert_eq!(
        hit,
        Some("http://info.cern.ch/hypertext/WWW/TheProject.html"),
        "Hit test on link coordinates should return target href"
    );
}

#[test]
#[ignore = "requires live internet connection"]
fn test_navigate_neverssl_repro() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1024, 768);
    browser.navigate("http://neverssl.com");
    let dl = browser.build_display_list((1024, 768));
    assert!(
        !dl.is_empty(),
        "Display list should not be empty for neverssl.com"
    );
}

#[test]
#[ignore = "requires live internet connection"]
fn test_wikipedia_rendering_and_display_list() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1280, 900);
    browser.navigate("https://en.wikipedia.org/wiki/Main_Page");
    let dl = browser.build_display_list((1280, 900));
    assert!(
        !dl.is_empty(),
        "Display list should not be empty for Wikipedia"
    );
    let has_content_text = dl.iter().any(|cmd| match cmd {
        mango_render::DisplayCommand::DrawText { text, .. } => {
            text.contains("Wikipedia")
                || text.contains("article")
                || text.contains("free encyclopedia")
        }
        _ => false,
    });
    assert!(
        has_content_text,
        "Wikipedia display list should contain page content text"
    );
}

#[test]
#[ignore = "requires live internet connection"]
fn test_duckduckgo_search_rendering() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1280, 900);
    browser.navigate("https://html.duckduckgo.com/html/?q=rust");
    let dl = browser.build_display_list((1280, 900));
    assert!(
        !dl.is_empty(),
        "Display list should not be empty for DuckDuckGo"
    );
    let has_result = dl.iter().any(|cmd| match cmd {
        mango_render::DisplayCommand::DrawText { text, .. } => {
            text.contains("Rust") || text.contains("rust-lang.org")
        }
        _ => false,
    });
    assert!(
        has_result,
        "DuckDuckGo display list should contain search result items"
    );

    // Verify scroll geometry for long DuckDuckGo result lists
    assert!(
        browser.scrollable_height() > 900.0,
        "DuckDuckGo search results should exceed 900px viewport height (got {})",
        browser.scrollable_height()
    );
    assert!(
        browser.max_scroll() > 0.0,
        "Max scroll should be positive for DuckDuckGo results"
    );

    // Scroll down several lines
    browser.handle_scroll(-10.0);
    assert!(
        browser.scroll_y() > 0.0,
        "Scroll position should advance after scroll"
    );

    let dl_scrolled = browser.build_display_list((1280, 900));
    assert!(
        !dl_scrolled.is_empty(),
        "Display list should not be empty after scrolling down"
    );

    // Verify search results are still visible and not clipped away when scrolled
    let has_scrolled_results = dl_scrolled.iter().any(|cmd| match cmd {
        mango_render::DisplayCommand::DrawText { text, .. } => {
            text.contains("Rust") || text.contains("rust-lang.org") || text.contains("programming")
        }
        _ => false,
    });
    assert!(
        has_scrolled_results,
        "Search results must continue rendering after scrolling down"
    );
}

#[test]
#[ignore = "requires live internet connection"]
fn test_hackernews_rendering_and_table_layout() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1280, 900);
    browser.navigate("https://news.ycombinator.com/");
    let dl = browser.build_display_list((1280, 900));
    assert!(
        !dl.is_empty(),
        "Display list should not be empty for Hacker News"
    );
    let has_hn_text = dl.iter().any(|cmd| match cmd {
        mango_render::DisplayCommand::DrawText { text, .. } => {
            text.contains("Hacker News")
                || text.contains("points")
                || text.contains("comments")
                || text.contains("past")
        }
        _ => false,
    });
    assert!(
        has_hn_text,
        "Hacker News display list should contain stories or navbar text"
    );
}

#[test]
fn test_import_stylesheet_chaining_and_inline_style() {
    let mut browser = mango_browser::browser::BrowserChrome::new(1024, 768);
    let cache = browser.loader().cache();

    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                @import "css/theme.css";
                .local-card { margin-top: 15px; }
            </style>
        </head>
        <body>
            <div class="local-card">
                <h1 class="heading">Chained Import Test</h1>
            </div>
        </body>
        </html>
    "#;

    let theme_css = r#"
        @import "base.css";
        .local-card { background-color: #ff00aa; }
    "#;

    let base_css = r#"
        .heading { color: #00ffcc; font-size: 24px; }
    "#;

    cache.insert(
        "https://example.com/app/index.html",
        "text/html",
        200,
        std::collections::HashMap::new(),
        html.as_bytes().to_vec(),
        None,
    );

    cache.insert(
        "https://example.com/app/css/theme.css",
        "text/css",
        200,
        std::collections::HashMap::new(),
        theme_css.as_bytes().to_vec(),
        None,
    );

    cache.insert(
        "https://example.com/app/css/base.css",
        "text/css",
        200,
        std::collections::HashMap::new(),
        base_css.as_bytes().to_vec(),
        None,
    );

    browser.navigate("https://example.com/app/index.html");

    // Verify both imported stylesheets (base.css and theme.css) were fetched into cached_stylesheets
    assert_eq!(
        browser.cached_stylesheets().len(),
        2,
        "Should have loaded base.css and theme.css via @import chain"
    );

    let dl = browser.build_display_list((1024, 768));
    let has_card_bg = dl.iter().any(|cmd| match cmd {
        DisplayCommand::FillRect { color, .. } => *color == Color::rgb(255, 0, 170),
        _ => false,
    });
    assert!(
        has_card_bg,
        "Display list must render background from @import stylesheet"
    );
}

#[test]
fn test_character_encoding_section_5_4_pipeline() {
    let loader = ResourceLoader::new();

    // 1. BOM sniffing wins over conflicting meta charset
    let bom_url_str = "https://example.com/bom.html";
    let bom_url = Url::parse(bom_url_str).unwrap();
    let mut bom_bytes = vec![0xEF, 0xBB, 0xBF];
    bom_bytes.extend_from_slice(b"<!DOCTYPE html><html><head><meta charset=\"windows-1252\"></head><body>BOM Cafe \xC3\xA9</body></html>");
    loader.cache().insert(
        bom_url_str,
        "text/html",
        200,
        std::collections::HashMap::new(),
        bom_bytes,
        None,
    );
    let fetched_bom = loader.fetch_document(&bom_url).expect("fetch bom doc");
    assert_eq!(fetched_bom.encoding, mango_net::encoding::Encoding::Utf8);
    assert!(fetched_bom.html.contains("BOM Cafe é"));

    // 2. Content-Type charset parameter (Windows-1252 with smart quote \x93 and euro \x80)
    let ct_url_str = "https://example.com/win1252.html";
    let ct_url = Url::parse(ct_url_str).unwrap();
    let win_bytes = b"<!DOCTYPE html><html><body>Price: \x80100 and \x93quoted\x94</body></html>";
    loader.cache().insert(
        ct_url_str,
        "text/html; charset=windows-1252",
        200,
        std::collections::HashMap::new(),
        win_bytes.to_vec(),
        None,
    );
    let fetched_win = loader.fetch_document(&ct_url).expect("fetch win doc");
    assert_eq!(
        fetched_win.encoding,
        mango_net::encoding::Encoding::Windows1252
    );
    assert!(fetched_win.html.contains("Price: €100 and “quoted”"));

    // 3. <meta charset> prescan without HTTP header charset
    let meta_url_str = "https://example.com/meta.html";
    let meta_url = Url::parse(meta_url_str).unwrap();
    let meta_bytes = b"<!DOCTYPE html><html><head><meta charset=\"iso-8859-1\"></head><body>caf\xE9</body></html>";
    loader.cache().insert(
        meta_url_str,
        "text/html",
        200,
        std::collections::HashMap::new(),
        meta_bytes.to_vec(),
        None,
    );
    let fetched_meta = loader.fetch_document(&meta_url).expect("fetch meta doc");
    assert_eq!(
        fetched_meta.encoding,
        mango_net::encoding::Encoding::Windows1252
    );
    assert!(fetched_meta.html.contains("café"));

    // 4. <meta http-equiv="Content-Type"> fallback
    let http_equiv_url_str = "https://example.com/http_equiv.html";
    let http_equiv_url = Url::parse(http_equiv_url_str).unwrap();
    let he_bytes = b"<!DOCTYPE html><html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=windows-1252\"></head><body>\x99 brand</body></html>";
    loader.cache().insert(
        http_equiv_url_str,
        "text/html",
        200,
        std::collections::HashMap::new(),
        he_bytes.to_vec(),
        None,
    );
    let fetched_he = loader
        .fetch_document(&http_equiv_url)
        .expect("fetch he doc");
    assert_eq!(
        fetched_he.encoding,
        mango_net::encoding::Encoding::Windows1252
    );
    assert!(fetched_he.html.contains("™ brand"));
}

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
