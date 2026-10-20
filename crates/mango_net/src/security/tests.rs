use super::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[test]
fn test_csp_parse_and_self_matching() {
    let policy = CspPolicy::parse(
        "default-src 'self'; script-src 'self' https://cdn.example.com; img-src *",
        false,
    )
    .unwrap();

    assert!(policy.allows(
        ResourceDirective::Script,
        "https://example.com/app.js",
        "https://example.com"
    ));
    assert!(policy.allows(
        ResourceDirective::Script,
        "https://cdn.example.com/lib.js",
        "https://example.com"
    ));
    assert!(!policy.allows(
        ResourceDirective::Script,
        "https://evil.example.net/x.js",
        "https://example.com"
    ));
    // img-src * allows anything
    assert!(policy.allows(
        ResourceDirective::Image,
        "http://any.test/a.png",
        "https://example.com"
    ));
    // Directives fall back to default-src
    assert!(policy.allows(
        ResourceDirective::Font,
        "https://example.com/f.woff2",
        "https://example.com"
    ));
    assert!(!policy.allows(
        ResourceDirective::Connect,
        "https://api.other.com",
        "https://example.com"
    ));
}

#[test]
fn test_csp_none_and_wildcards() {
    let policy = CspPolicy::parse("default-src 'none'", false).unwrap();
    assert!(!policy.allows(
        ResourceDirective::Image,
        "https://example.com/a.png",
        "https://example.com"
    ));

    let wildcard = CspPolicy::parse("script-src *.example.com", false).unwrap();
    assert!(wildcard.allows(
        ResourceDirective::Script,
        "https://sub.example.com/a.js",
        "https://example.com"
    ));
    assert!(wildcard.allows(
        ResourceDirective::Script,
        "https://example.com/a.js",
        "https://example.com"
    ));
    assert!(!wildcard.allows(
        ResourceDirective::Script,
        "https://notexample.com/a.js",
        "https://example.com"
    ));
}

#[test]
fn test_csp_data_urls_allowed() {
    // data: requires an explicit `data:` source — 'self' alone denies.
    let policy = CspPolicy::parse("img-src 'self'", false).unwrap();
    assert!(!policy.allows(
        ResourceDirective::Image,
        "data:image/png;base64,AAAA",
        "https://example.com"
    ));
    // Explicit data: source allows.
    let explicit = CspPolicy::parse("img-src 'self' data:", false).unwrap();
    assert!(explicit.allows(
        ResourceDirective::Image,
        "data:image/png;base64,AAAA",
        "https://example.com"
    ));
    // `*` does not allow data: (explicit source required).
    let star = CspPolicy::parse("img-src *", false).unwrap();
    assert!(!star.allows(
        ResourceDirective::Image,
        "data:image/png;base64,AAAA",
        "https://example.com"
    ));
    // blob: similarly requires explicit source.
    assert!(!policy.allows(
        ResourceDirective::Image,
        "blob:https://example.com/uuid",
        "https://example.com"
    ));
    let blob_ok = CspPolicy::parse("img-src 'self' blob:", false).unwrap();
    assert!(blob_ok.allows(
        ResourceDirective::Image,
        "blob:https://example.com/uuid",
        "https://example.com"
    ));
}

#[test]
fn test_csp_port_and_path_matching() {
    let policy = CspPolicy::parse("script-src https://example.com:8443/js/", false).unwrap();
    assert!(policy.allows(
        ResourceDirective::Script,
        "https://example.com:8443/js/app.js",
        "https://example.com"
    ));
    assert!(!policy.allows(
        ResourceDirective::Script,
        "https://example.com:8443/other/app.js",
        "https://example.com"
    ));
    assert!(!policy.allows(
        ResourceDirective::Script,
        "https://example.com:9000/js/app.js",
        "https://example.com"
    ));
    // Bare host matches any scheme but still checks port/path when given.
    let bare = CspPolicy::parse("script-src example.com", false).unwrap();
    assert!(bare.allows(
        ResourceDirective::Script,
        "https://example.com/app.js",
        "https://other.test"
    ));
    assert!(bare.allows(
        ResourceDirective::Script,
        "http://example.com/app.js",
        "https://other.test"
    ));
    // 'self' respects ports.
    let self_policy = CspPolicy::parse("script-src 'self'", false).unwrap();
    assert!(self_policy.allows(
        ResourceDirective::Script,
        "https://example.com/app.js",
        "https://example.com"
    ));
    assert!(!self_policy.allows(
        ResourceDirective::Script,
        "https://example.com:8443/app.js",
        "https://example.com"
    ));
}

#[test]
fn test_hsts_parse_requires_max_age() {
    assert!(HstsDirective::parse("includeSubDomains").is_none());
    assert!(HstsDirective::parse("").is_none());
    assert!(HstsDirective::parse("preload").is_none());
    let d = HstsDirective::parse("max-age=100; includeSubDomains").unwrap();
    assert_eq!(d.max_age_secs, 100);
}

#[test]
fn test_multiple_csp_headers_enforce_all() {
    let headers = vec![
        (
            "Content-Security-Policy".to_string(),
            "script-src 'self'".to_string(),
        ),
        (
            "Content-Security-Policy".to_string(),
            "script-src https://cdn.example.com".to_string(),
        ),
    ];
    let parsed = SecurityHeaders::from_headers_multi(&headers);
    assert_eq!(parsed.content_security_policies.len(), 2);
    // Must satisfy BOTH policies.
    assert!(!parsed.allows_all(
        ResourceDirective::Script,
        "https://cdn.example.com/app.js",
        "https://example.com"
    ));
    assert!(!parsed.allows_all(
        ResourceDirective::Script,
        "https://example.com/app.js",
        "https://example.com"
    ));
    let compatible = vec![
        (
            "Content-Security-Policy".to_string(),
            "script-src 'self' https://cdn.example.com".to_string(),
        ),
        (
            "Content-Security-Policy".to_string(),
            "script-src https://cdn.example.com 'self'".to_string(),
        ),
    ];
    let parsed2 = SecurityHeaders::from_headers_multi(&compatible);
    assert!(parsed2.allows_all(
        ResourceDirective::Script,
        "https://cdn.example.com/app.js",
        "https://example.com"
    ));
}

#[test]
fn test_csp_report_only_never_blocks() {
    let policy = CspPolicy::parse("default-src 'none'", true).unwrap();
    assert!(policy.report_only);
    assert!(policy.is_allowed(
        ResourceDirective::Script,
        "https://x.test/a.js",
        "https://example.com"
    ));
}

#[test]
fn test_csp_frame_ancestors() {
    let policy = CspPolicy::parse("frame-ancestors 'none'", false).unwrap();
    assert!(policy.blocks_framing());
    let policy = CspPolicy::parse("frame-ancestors https://example.com", false).unwrap();
    assert!(!policy.blocks_framing());
}

#[test]
fn test_hsts_store_and_upgrade() {
    let mut store = HstsStore::new();
    assert!(!store.should_upgrade("http://example.com/a"));

    store.record(
        "example.com",
        HstsDirective {
            max_age_secs: 3600,
            include_subdomains: true,
            preload: true,
        },
    );
    assert!(store.should_upgrade("http://example.com/a"));
    assert!(store.should_upgrade("http://www.example.com/a"));
    assert!(!store.should_upgrade("http://other.test/a"));
    assert!(!store.should_upgrade("https://example.com/a"));
    assert_eq!(store.policy_for("example.com").unwrap().max_age_secs, 3600);
    assert!(store.policy_for("www.example.com").is_some());
    assert!(store.policy_for("unrelated.test").is_none());
}

#[test]
fn test_hsts_max_age_zero_clears() {
    let mut store = HstsStore::new();
    store.record(
        "example.com",
        HstsDirective {
            max_age_secs: 100,
            include_subdomains: false,
            preload: false,
        },
    );
    assert!(store.should_upgrade("http://example.com/"));
    store.record(
        "example.com",
        HstsDirective {
            max_age_secs: 0,
            include_subdomains: false,
            preload: false,
        },
    );
    assert!(!store.should_upgrade("http://example.com/"));
}

#[test]
fn test_hsts_store_automatic_pruning_and_cap() {
    let mut store = HstsStore::new();
    // Insert an entry that will immediately expire (duration 0 simulated or very short)
    store.entries.insert(
        "expired.test".to_string(),
        (
            HstsDirective {
                max_age_secs: 1,
                include_subdomains: false,
                preload: false,
            },
            Instant::now() - Duration::from_secs(10),
        ),
    );
    store.subdomains.insert("expired.test".to_string(), false);
    assert_eq!(store.len(), 1);

    // Recording a new entry triggers auto-prune on modulo/capacity check.
    store.record(
        "fresh.test",
        HstsDirective {
            max_age_secs: 300,
            include_subdomains: true,
            preload: false,
        },
    );
    // The expired entry was pruned automatically!
    assert_eq!(store.len(), 1);
    assert!(store.policy_for("fresh.test").is_some());
    assert!(store.policy_for("expired.test").is_none());

    // Test capacity cap
    for i in 0..(MAX_HSTS_ENTRIES + 10) {
        store.record(
            &format!("host{i}.test"),
            HstsDirective {
                max_age_secs: 3600,
                include_subdomains: false,
                preload: false,
            },
        );
    }
    assert!(store.len() <= MAX_HSTS_ENTRIES);
}

#[test]
fn test_parse_hsts_header() {
    let d = HstsDirective::parse("max-age=63072000; includeSubDomains; preload").unwrap();
    assert_eq!(d.max_age_secs, 63_072_000);
    assert!(d.include_subdomains);
    assert!(d.preload);
}

#[test]
fn test_security_headers_from_map() {
    let mut headers = HashMap::new();
    headers.insert(
        "Content-Security-Policy".to_string(),
        "default-src 'self'".to_string(),
    );
    headers.insert(
        "Strict-Transport-Security".to_string(),
        "max-age=100".to_string(),
    );
    headers.insert("X-Content-Type-Options".to_string(), "nosniff".to_string());
    headers.insert("X-Frame-Options".to_string(), "DENY".to_string());

    let parsed = SecurityHeaders::from_headers(&headers);
    assert!(parsed.nosniff());
    assert!(parsed.blocks_framing());
    assert!(parsed.requires_https());
    assert!(parsed.content_security_policy.is_some());
}

#[test]
fn test_mixed_content_rules() {
    // Active mixed content on an HTTPS page is blocked
    assert!(blocks_mixed_content(
        "https://example.com",
        "http://cdn.test/app.js",
        true
    ));
    // Passive mixed content on an HTTPS page is also blocked
    assert!(blocks_mixed_content(
        "https://example.com",
        "http://cdn.test/a.png",
        false
    ));
    // Plain HTTP pages are unaffected
    assert!(!blocks_mixed_content(
        "http://example.com",
        "http://cdn.test/app.js",
        true
    ));
}

#[test]
fn test_csp_none_precedence() {
    // Even if permissive source `*` or domain appears before/after `'none'`,
    // `'none'` must strictly deny all requests.
    let policy1 = CspPolicy::parse("script-src * 'none'", false).unwrap();
    assert!(!policy1.allows(
        ResourceDirective::Script,
        "https://example.com/app.js",
        "https://example.com"
    ));

    let policy2 = CspPolicy::parse("script-src 'none' *", false).unwrap();
    assert!(!policy2.allows(
        ResourceDirective::Script,
        "https://example.com/app.js",
        "https://example.com"
    ));
}

#[test]
fn test_upgrade_to_https() {
    assert_eq!(upgrade_to_https("http://a.test/x"), "https://a.test/x");
    assert_eq!(upgrade_to_https("https://a.test/x"), "https://a.test/x");
    assert_eq!(upgrade_to_https("data:text/plain,hi"), "data:text/plain,hi");
}

#[test]
fn test_same_origin_policy() {
    assert!(is_same_origin(
        "https://example.com/foo",
        "https://example.com/bar"
    ));
    assert!(is_same_origin(
        "https://example.com:443/foo",
        "https://example.com/bar"
    ));
    assert!(is_same_origin(
        "http://example.com:80/foo",
        "http://example.com/bar"
    ));
    // Different schemes
    assert!(!is_same_origin("http://example.com", "https://example.com"));
    // Different hosts
    assert!(!is_same_origin(
        "https://sub.example.com",
        "https://example.com"
    ));
    // Different ports
    assert!(!is_same_origin(
        "https://example.com:8443",
        "https://example.com:443"
    ));

    let orig = Origin::parse("https://mango.dev:9000/path?query").unwrap();
    assert_eq!(orig.scheme, "https");
    assert_eq!(orig.host, "mango.dev");
    assert_eq!(orig.port, 9000);
    assert_eq!(orig.serialize(), "https://mango.dev:9000");
}

#[test]
fn test_cors_simple_request_classification() {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "*/*".to_string());
    headers.insert("Content-Type".to_string(), "text/plain".to_string());
    assert!(is_cors_simple_request("GET", &headers));
    assert!(is_cors_simple_request("POST", &headers));

    // Non-simple method
    assert!(!is_cors_simple_request("PUT", &headers));
    assert!(!is_cors_simple_request("DELETE", &headers));

    // Non-simple content type
    let mut json_headers = headers.clone();
    json_headers.insert("Content-Type".to_string(), "application/json".to_string());
    assert!(!is_cors_simple_request("POST", &json_headers));

    // Custom header
    let mut custom_headers = headers.clone();
    custom_headers.insert("X-Requested-With".to_string(), "XMLHttpRequest".to_string());
    assert!(!is_cors_simple_request("GET", &custom_headers));
}

#[test]
fn test_cors_preflight_and_response() {
    let mut preflight_headers = HashMap::new();
    preflight_headers.insert(
        "access-control-allow-origin".to_string(),
        "https://app.mango.dev".to_string(),
    );
    preflight_headers.insert(
        "access-control-allow-methods".to_string(),
        "GET, POST, PUT, DELETE".to_string(),
    );
    preflight_headers.insert(
        "access-control-allow-headers".to_string(),
        "content-type, authorization".to_string(),
    );
    preflight_headers.insert("access-control-max-age".to_string(), "600".to_string());

    let max_age = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://app.mango.dev",
        "PUT",
        &["Content-Type", "Authorization"],
    )
    .unwrap();
    assert_eq!(max_age, Duration::from_secs(600));

    // Preflight failed origin
    let err = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://evil.attacker.com",
        "PUT",
        &["Content-Type"],
    )
    .unwrap_err();
    assert!(matches!(err, CorsError::OriginNotAllowed(_)));

    // Preflight failed method
    let err2 = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://app.mango.dev",
        "PATCH",
        &["Content-Type"],
    )
    .unwrap_err();
    assert!(matches!(err2, CorsError::MethodNotAllowed(_)));

    // Preflight failed header
    let err3 = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://app.mango.dev",
        "PUT",
        &["X-Custom-Secret"],
    )
    .unwrap_err();
    assert!(matches!(err3, CorsError::HeaderNotAllowed(_)));

    // Response validation
    let mut resp_headers = HashMap::new();
    resp_headers.insert("access-control-allow-origin".to_string(), "*".to_string());
    resp_headers.insert(
        "access-control-expose-headers".to_string(),
        "X-Total-Count, X-Request-ID".to_string(),
    );
    let exposed = validate_cors_response(&resp_headers, "https://app.mango.dev", false).unwrap();
    assert!(exposed.contains(&"x-total-count".to_string()));
    assert!(exposed.contains(&"x-request-id".to_string()));
    assert!(exposed.contains(&"content-type".to_string()));

    // Response validation with credentials rejects wildcard
    let cred_err =
        validate_cors_response(&resp_headers, "https://app.mango.dev", true).unwrap_err();
    assert_eq!(cred_err, CorsError::WildcardOriginWithCredentialsNotAllowed);
}

#[test]
fn test_cryptographic_hashes_and_sri() {
    let msg = b"hello world";
    // Standard SHA-256 for "hello world" = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
    let d256 = sha256(msg);
    let hex256 = d256.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        hex256,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );

    // Standard SHA-384 for "hello world"
    let d384 = sha384(msg);
    let hex384 = d384.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        hex384,
        "fdbd8e75a67f29f701a4e040385e2e23986303ea10239211af907fcbb83578b3e417cb71ce646efd0819dd8c088de1bd"
    );

    // Standard SHA-512 for "hello world"
    let d512 = sha512(msg);
    let hex512 = d512.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        hex512,
        "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
    );

    // Test SRI parsing and verification
    let integrity = format!(
        "sha256-{} sha384-{}",
        crate::websocket::base64_encode(&d256),
        crate::websocket::base64_encode(&d384)
    );
    let parsed = parse_sri_metadata(&integrity);
    assert_eq!(parsed.len(), 2);

    // Valid integrity verification uses strongest algorithm (sha384)
    assert!(verify_subresource_integrity(msg, &integrity).unwrap());

    // Corrupted content triggers DigestMismatch
    let tampered = b"hello malicious world";
    let err = verify_subresource_integrity(tampered, &integrity).unwrap_err();
    assert!(matches!(err, SriError::DigestMismatch { .. }));
}

#[test]
fn test_certificate_pinning() {
    let mut pin_set = CertificatePinSet::new();
    let target_pin = "pin-base64-hash-example";
    pin_set.add_pin("api.mango.dev", target_pin, true);

    // Matching pin succeeds
    let presented = vec![target_pin.to_string()];
    assert!(pin_set.verify_pins("api.mango.dev", &presented).unwrap());

    // Subdomain succeeds because include_subdomains was true
    assert!(pin_set.verify_pins("v1.api.mango.dev", &presented).unwrap());

    // Mismatched pin fails
    let fake = vec!["attacker-pin-hash".to_string()];
    let err = pin_set.verify_pins("api.mango.dev", &fake).unwrap_err();
    assert!(matches!(err, PinError::PinMismatch { .. }));

    // Unpinned host allows any cert
    assert!(pin_set.verify_pins("unpinned.org", &fake).unwrap());
}

#[test]
fn test_proxy_configuration_and_bypass() {
    let mut proxy = ProxyConfig::default();
    proxy.http_proxy = Some("http://proxy.corp.com:8080".to_string());
    proxy.https_proxy = Some("http://secure-proxy.corp.com:8443".to_string());
    proxy.no_proxy = vec![
        "localhost".to_string(),
        ".internal.corp".to_string(),
        "127.0.0.1".to_string(),
    ];

    // Bypass rules
    assert!(proxy.should_bypass("localhost"));
    assert!(proxy.should_bypass("service.internal.corp"));
    assert!(!proxy.should_bypass("external.com"));

    // Proxy resolution
    let u1 = crate::url::Url::parse("http://external.com/api").unwrap();
    assert_eq!(
        proxy.resolve_proxy_for_url(&u1),
        Some("http://proxy.corp.com:8080".to_string())
    );

    let u2 = crate::url::Url::parse("https://external.com/secure").unwrap();
    assert_eq!(
        proxy.resolve_proxy_for_url(&u2),
        Some("http://secure-proxy.corp.com:8443".to_string())
    );

    let u3 = crate::url::Url::parse("https://service.internal.corp/secret").unwrap();
    assert_eq!(proxy.resolve_proxy_for_url(&u3), None);
}

#[test]
fn test_referrer_policy_parsing_and_computation() {
    let page_https =
        crate::url::Url::parse("https://user:pass@example.com:443/dir/page.html?token=123#frag")
            .unwrap();
    let same_dest = crate::url::Url::parse("https://example.com/assets/style.css").unwrap();
    let cross_https = crate::url::Url::parse("https://other.com/image.png").unwrap();
    let cross_http = crate::url::Url::parse("http://other.com/script.js").unwrap();

    // 1. NoReferrer
    assert_eq!(
        compute_referrer(&page_https, &same_dest, ReferrerPolicy::NoReferrer),
        None
    );
    assert_eq!(
        compute_referrer(&page_https, &cross_https, ReferrerPolicy::NoReferrer),
        None
    );

    // 2. StrictOriginWhenCrossOrigin (default)
    // Same-origin: stripped full URL (no userinfo, no frag)
    assert_eq!(
        compute_referrer(
            &page_https,
            &same_dest,
            ReferrerPolicy::StrictOriginWhenCrossOrigin
        ),
        Some("https://example.com/dir/page.html?token=123".to_string())
    );
    // Cross-origin HTTPS: origin only
    assert_eq!(
        compute_referrer(
            &page_https,
            &cross_https,
            ReferrerPolicy::StrictOriginWhenCrossOrigin
        ),
        Some("https://example.com/".to_string())
    );
    // Downgrade HTTPS -> HTTP: None
    assert_eq!(
        compute_referrer(
            &page_https,
            &cross_http,
            ReferrerPolicy::StrictOriginWhenCrossOrigin
        ),
        None
    );

    // 3. SameOrigin
    assert_eq!(
        compute_referrer(&page_https, &same_dest, ReferrerPolicy::SameOrigin),
        Some("https://example.com/dir/page.html?token=123".to_string())
    );
    assert_eq!(
        compute_referrer(&page_https, &cross_https, ReferrerPolicy::SameOrigin),
        None
    );

    // 4. Origin
    assert_eq!(
        compute_referrer(&page_https, &same_dest, ReferrerPolicy::Origin),
        Some("https://example.com/".to_string())
    );
    assert_eq!(
        compute_referrer(&page_https, &cross_http, ReferrerPolicy::Origin),
        Some("https://example.com/".to_string())
    );

    // 5. StrictOrigin
    assert_eq!(
        compute_referrer(&page_https, &cross_https, ReferrerPolicy::StrictOrigin),
        Some("https://example.com/".to_string())
    );
    assert_eq!(
        compute_referrer(&page_https, &cross_http, ReferrerPolicy::StrictOrigin),
        None
    );

    // 6. UnsafeUrl
    assert_eq!(
        compute_referrer(&page_https, &cross_http, ReferrerPolicy::UnsafeUrl),
        Some("https://example.com/dir/page.html?token=123".to_string())
    );

    // 7. Non-HTTP(S) scheme source (e.g. data:, blob:, file:)
    let page_data = crate::url::Url::parse("data:text/html,test").unwrap();
    assert_eq!(
        compute_referrer(&page_data, &same_dest, ReferrerPolicy::UnsafeUrl),
        None
    );

    // 8. Multiple tokens comma-separated (last valid wins)
    let parsed = ReferrerPolicy::parse("no-referrer, unsafe-url, strict-origin");
    assert_eq!(parsed, ReferrerPolicy::StrictOrigin);
}
