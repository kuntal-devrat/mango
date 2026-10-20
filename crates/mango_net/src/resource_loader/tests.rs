use super::*;
use crate::url::Url;

#[test]
fn test_fetch_image_data_uri_returns_bytes_without_network() {
    let loader = ResourceLoader::new();
    let base_url = Url::parse("https://example.com/index.html").unwrap();
    let data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

    let result = loader.fetch_image_bytes(&base_url, data_uri);
    assert!(result.is_ok());
    let bytes = result.unwrap();
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "must decode base64 PNG header"
    );
}

#[test]
fn test_fetch_font_data_uri_returns_bytes_without_network() {
    let loader = ResourceLoader::new();
    let base_url = Url::parse("https://example.com/index.html").unwrap();
    let font_data_uri = "data:font/woff2;base64,d09GMgABAAAAA";

    let result = loader.fetch_font_bytes(&base_url, font_data_uri);
    assert!(result.is_ok());
    let bytes = result.unwrap();
    assert!(!bytes.is_empty());
    assert_ne!(
        bytes,
        font_data_uri.as_bytes(),
        "must decode from ASCII base64"
    );
}

#[test]
fn test_decode_percent_data_uri() {
    let uri = "data:text/plain,Hello%20World%21";
    let bytes = decode_data_uri_bytes(uri).unwrap();
    assert_eq!(String::from_utf8(bytes).unwrap(), "Hello World!");
}

#[test]
fn test_fetch_with_invalid_url_returns_error() {
    let loader = ResourceLoader::new();
    let base_url = Url::parse("https://example.com/").unwrap();

    let result = loader.fetch_stylesheet(&base_url, "http://[invalid-ipv6");
    assert!(result.is_err());
}

#[test]
fn test_loader_cache_prepopulation() {
    let loader = ResourceLoader::new();
    let url_key = "https://example.com/style.css";
    loader.cache.insert(
        url_key,
        "text/css",
        200,
        std::collections::HashMap::new(),
        b"body { color: red; }".to_vec(),
        None,
    );

    let base_url = Url::parse("https://example.com/page.html").unwrap();
    let css = loader.fetch_stylesheet(&base_url, "style.css").unwrap();
    assert_eq!(css, "body { color: red; }");
}

#[test]
fn test_csp_blocks_disallowed_script_origin() {
    let loader = ResourceLoader::new();
    let page = Url::parse("https://example.com/page").unwrap();
    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "content-security-policy".to_string(),
        "default-src 'self'; script-src 'self' https://cdn.example.com".to_string(),
    );
    loader.remember_csp(&page, &headers);

    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &page,
                "https://cdn.example.com/app.js",
                true
            )
            .is_ok(),
        "allow-listed CDN script must load"
    );
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &page,
                "https://evil.example.net/x.js",
                true
            )
            .is_err(),
        "off-origin script must be blocked"
    );
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Style,
                &page,
                "https://example.com/site.css",
                true
            )
            .is_ok(),
        "'self' allows same-origin stylesheets"
    );
}

#[test]
fn test_csp_multi_policy_intersection() {
    let loader = ResourceLoader::new();
    let page = Url::parse("https://example.com/page").unwrap();
    let mut headers = std::collections::HashMap::new();
    // Two comma-separated CSP policies (or multiple headers merged with comma):
    // Policy 1 allows example.com and cdn1.example.com.
    // Policy 2 allows example.com and cdn2.example.com.
    // Therefore cdn1 is blocked by policy 2, and cdn2 is blocked by policy 1.
    headers.insert(
        "content-security-policy".to_string(),
        "script-src 'self' https://cdn1.example.com, script-src 'self' https://cdn2.example.com"
            .to_string(),
    );
    loader.remember_csp(&page, &headers);
    assert_eq!(loader.csp_policies_for(&page).len(), 2);

    // Same origin is allowed by both policies
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &page,
                "https://example.com/main.js",
                true
            )
            .is_ok(),
        "same-origin script allowed by both policies"
    );

    // cdn1 is rejected by policy 2
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &page,
                "https://cdn1.example.com/app.js",
                true
            )
            .is_err(),
        "cdn1 blocked by second policy"
    );

    // cdn2 is rejected by policy 1
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &page,
                "https://cdn2.example.com/app.js",
                true
            )
            .is_err(),
        "cdn2 blocked by first policy"
    );
}

#[test]
fn test_mixed_content_is_blocked_on_https_pages() {
    let loader = ResourceLoader::new();
    let page = Url::parse("https://secure.example.com/").unwrap();

    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &page,
                "http://secure.example.com/legacy.js",
                true
            )
            .is_err(),
        "active mixed content must be blocked"
    );
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Image,
                &page,
                "http://secure.example.com/pic.png",
                false
            )
            .is_err(),
        "passive mixed content must also be blocked on HTTPS pages"
    );

    // An http:// page has no mixed-content constraint.
    let insecure = Url::parse("http://secure.example.com/").unwrap();
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &insecure,
                "http://cdn.example.com/x.js",
                true
            )
            .is_ok()
    );
}

#[test]
fn test_csp_policies_are_keyed_per_origin() {
    let loader = ResourceLoader::new();
    let a = Url::parse("https://a.example/").unwrap();
    let b = Url::parse("https://b.example/").unwrap();
    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "content-security-policy".to_string(),
        "script-src 'none'".to_string(),
    );
    loader.remember_csp(&a, &headers);

    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &a,
                "https://a.example/x.js",
                true
            )
            .is_err(),
        "a.example is locked down"
    );
    assert!(
        loader
            .guard_subresource(
                crate::security::ResourceDirective::Script,
                &b,
                "https://b.example/x.js",
                true
            )
            .is_ok(),
        "b.example has no policy yet"
    );
}

#[test]
fn test_charset_detection_decodes_stylesheets() {
    // A <meta charset> prescan (the HTML5 first-1024-byte rule) decides it.
    let mut body: Vec<u8> = b"<meta charset=\"iso-8859-1\">/* caf".to_vec();
    body.push(0xE9); // é in latin-1
    body.extend_from_slice(b" */\nbody{color:red}");
    let text = ResourceLoader::decode_text(&body, None);
    assert!(text.contains("body{color:red}"));
    assert!(
        text.contains('é'),
        "meta-declared latin-1 byte 0xE9 must decode to U+00E9, got: {text}"
    );

    // Content-Type charset wins over the default.
    let latin = b"/* caf\xe9 */ body{}".to_vec();
    let text = ResourceLoader::decode_text(&latin, Some("text/css; charset=iso-8859-1"));
    assert!(text.contains('é'), "got: {text}");

    // UTF-8 BOM is honored and stripped.
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice("body{content:'✓'}".as_bytes());
    let text = ResourceLoader::decode_text(&bom, None);
    assert!(text.contains('✓'), "got: {text:?}");
    assert!(!text.starts_with('\u{feff}'), "BOM stripped: {text:?}");

    // Without a declaration, invalid UTF-8 does not panic — it degrades
    // to U+FFFD exactly like a browser's fallback decoder.
    let text = ResourceLoader::decode_text(b"a\xffb", None);
    assert!(
        text.starts_with('a') && text.ends_with('b'),
        "got: {text:?}"
    );
}

#[test]
fn test_preload_scanner_and_priority_hints() {
    let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <link rel="dns-prefetch" href="//cdn.example.com">
                <link rel="preconnect" href="https://api.example.com">
                <link rel="preload" href="/fonts/inter.woff2" as="font" fetchpriority="high">
                <link rel="preload" href="/css/theme.css" as="style">
                <link rel="preload" href="/img/hero.webp" as="image" fetchpriority="low">
                <script src="/js/app.js" fetchpriority="high"></script>
            </head>
            <body></body>
            </html>
        "#;

    let base_url = Url::parse("https://example.com/index.html").unwrap();
    let result = scan_html_for_preloads(html, &base_url);

    assert_eq!(result.dns_prefetch_hosts, vec!["cdn.example.com"]);
    assert_eq!(result.preconnect_urls, vec!["https://api.example.com/"]);

    assert_eq!(result.preloads.len(), 4);
    // High priority first
    assert_eq!(result.preloads[0].priority, FetchPriority::High);
    assert_eq!(result.preloads[1].priority, FetchPriority::High);
    // Auto second
    assert_eq!(result.preloads[2].priority, FetchPriority::Auto);
    // Low third
    assert_eq!(result.preloads[3].priority, FetchPriority::Low);
    assert_eq!(result.preloads[3].url, "https://example.com/img/hero.webp");
}

#[test]
fn test_data_uri_size_cap_and_csp_enforcement() {
    let loader = ResourceLoader::new();
    let page_url = Url::parse("https://example.com/page.html").unwrap();

    // 1. Data URI exceeding 10MB cap is rejected
    let huge_data_uri = format!("data:text/css,{}", "a".repeat(MAX_DATA_URI_BYTES + 1));
    assert!(decode_data_uri_bytes(&huge_data_uri).is_err());

    // 2. Data URI within limit is allowed
    let valid_data_uri = "data:text/css,body{color:red}";
    assert!(decode_data_uri_bytes(valid_data_uri).is_ok());

    // 3. CSP blocking data: URI for script
    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "content-security-policy".to_string(),
        "script-src 'self'".to_string(),
    );
    loader.remember_csp(&page_url, &headers);

    let script_res = loader.fetch_script(&page_url, "data:text/javascript,console.log('hi')");
    assert!(
        script_res.is_err(),
        "CSP should block data: script when script-src is 'self'"
    );
}
