//! Phase 4 Integration Tests: Native HTTP/2 Multiplexing, HTTP/3 QUIC Engine,
//! CSP Level 3, CORS Preflight, and RFC 6265bis / CHIPS Partitioned Cookie Jar.

use std::collections::HashMap;
use std::time::Duration;

use mango_net::cookies::{CookieJar, CookiePriority};
use mango_net::hpack::{HeaderField, HpackDecoder, HpackEncoder};
use mango_net::http2::{FrameType, Http2Frame, Http2Session, StreamPriority, StreamState};
use mango_net::qpack::{QpackDecoder, QpackEncoder, QpackHeaderField};
use mango_net::quic::{
    ConnectionId, LongHeaderPacket, LongPacketType, QuicConnectionMigration, QuicFrame,
    ShortHeaderPacket,
};
use mango_net::security::{
    CorsError, CspPolicy, CspViolationReport, RequestCredentials, ResourceDirective, SriAlgorithm,
    sha256, validate_cors_preflight, validate_cors_response_with_credentials,
};
use mango_net::url::Url;

// ===========================================================================
// 1. HTTP/2 Multiplexing & HPACK Compression (RFC 7540 / RFC 7541)
// ===========================================================================

#[test]
fn test_hpack_static_and_dynamic_compression_roundtrip() {
    let mut encoder = HpackEncoder::new();
    let mut decoder = HpackDecoder::new();

    let request_headers = vec![
        HeaderField::new(":method", "GET"),
        HeaderField::new(":scheme", "https"),
        HeaderField::new(":path", "/index.html"),
        HeaderField::new(":authority", "example.com"),
        HeaderField::new("custom-header", "custom-value"),
    ];

    let encoded = encoder.encode(&request_headers);
    assert!(!encoded.is_empty());

    let decoded = decoder.decode(&encoded).expect("Decoding must succeed");
    assert_eq!(decoded.len(), 5);
    assert_eq!(decoded[0].name, ":method");
    assert_eq!(decoded[0].value, "GET");
    assert_eq!(decoded[1].name, ":scheme");
    assert_eq!(decoded[1].value, "https");
    assert_eq!(decoded[2].name, ":path");
    assert_eq!(decoded[2].value, "/index.html");
    assert_eq!(decoded[3].name, ":authority");
    assert_eq!(decoded[3].value, "example.com");
    assert_eq!(decoded[4].name, "custom-header");
    assert_eq!(decoded[4].value, "custom-value");

    // Dynamic table must retain the incrementally indexed authority and custom-header (2 entries)
    assert_eq!(encoder.dynamic_table_len(), 2);
    assert_eq!(decoder.dynamic_table_len(), 2);
}

#[test]
fn test_http2_session_multiplexing_and_headers_framing() {
    let mut session = Http2Session::new();
    let init_bytes = session.init_connection();
    assert!(init_bytes.starts_with(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"));

    let s1 = session.open_stream();
    let s2 = session.open_stream();
    let s3 = session.open_stream();

    assert_eq!(s1, 1);
    assert_eq!(s2, 3);
    assert_eq!(s3, 5);

    let stream = session.streams.get(&s1).unwrap();
    assert_eq!(stream.state, StreamState::Open);

    // Create a HEADERS frame for stream 1
    let frame = session.create_headers_frame(
        s1,
        &[
            (":method", "GET"),
            (":path", "/api/data"),
            (":scheme", "https"),
        ],
        true,
    );

    assert_eq!(frame.header.frame_type, FrameType::Headers);
    assert_eq!(frame.header.stream_id, s1);
    assert_eq!(frame.header.flags & 0x1, 0x1); // END_STREAM
    assert_eq!(frame.header.flags & 0x4, 0x4); // END_HEADERS

    // Handle incoming server response HEADERS frame on stream 1
    let mut server_encoder = HpackEncoder::new();
    let resp_headers = vec![
        HeaderField::new(":status", "200"),
        HeaderField::new("content-type", "application/json"),
        HeaderField::new("x-custom-res", "parity-test"),
    ];
    let resp_payload = server_encoder.encode(&resp_headers);

    let in_frame = Http2Frame::new(
        FrameType::Headers,
        0x5, // END_STREAM | END_HEADERS
        s1,
        resp_payload,
    );

    let result = session.handle_incoming_frame(&in_frame);
    assert!(result.is_ok());

    let s1_stream = session.streams.get(&s1).unwrap();
    assert_eq!(s1_stream.state, StreamState::HalfClosedRemote);
    assert_eq!(
        s1_stream
            .response_headers
            .get(":status")
            .map(|s| s.as_str()),
        Some("200")
    );
    assert_eq!(
        s1_stream
            .response_headers
            .get("content-type")
            .map(|s| s.as_str()),
        Some("application/json")
    );
    assert_eq!(
        s1_stream
            .response_headers
            .get("x-custom-res")
            .map(|s| s.as_str()),
        Some("parity-test")
    );
}

#[test]
fn test_http2_stream_priority_handling() {
    let mut session = Http2Session::new();
    let s1 = session.open_stream();

    let prio = StreamPriority {
        parent_stream_id: 0,
        weight: 64,
        exclusive: true,
    };
    let prio_frame = Http2Frame::priority(s1, prio);
    assert_eq!(prio_frame.header.frame_type, FrameType::Priority);

    let _ = session.handle_incoming_frame(&prio_frame);
    let s1_stream = session.streams.get(&s1).unwrap();
    assert_eq!(s1_stream.priority.parent_stream_id, 0);
    assert_eq!(s1_stream.priority.weight, 64);
    assert!(s1_stream.priority.exclusive);
}

// ===========================================================================
// 2. HTTP/3 & QUIC Transport Engine (RFC 9000 / RFC 9204)
// ===========================================================================

#[test]
fn test_quic_packet_framing_and_serialization() {
    let dcid = ConnectionId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let scid = ConnectionId::new([9, 10, 11, 12]);

    // Test Long Header Packet (Initial)
    let initial_pkt = LongHeaderPacket {
        packet_type: LongPacketType::Initial,
        version: 1,
        dcid: dcid.clone(),
        scid: scid.clone(),
        token: vec![0xAA, 0xBB],
        payload: vec![1, 2, 3, 4],
    };

    let serialized_long = initial_pkt.serialize();
    assert!(!serialized_long.is_empty());

    let parsed_long = LongHeaderPacket::parse(&serialized_long).expect("Parse long packet");
    assert_eq!(parsed_long.packet_type, LongPacketType::Initial);
    assert_eq!(parsed_long.version, 1);
    assert_eq!(parsed_long.dcid, dcid);
    assert_eq!(parsed_long.scid, scid);
    assert_eq!(parsed_long.token, vec![0xAA, 0xBB]);
    assert_eq!(parsed_long.payload, vec![1, 2, 3, 4]);

    // Test Short Header Packet (1-RTT)
    let short_pkt = ShortHeaderPacket {
        spin_bit: true,
        key_phase: false,
        dcid: dcid.clone(),
        packet_number: 100,
        payload: vec![5, 6, 7, 8],
    };

    let serialized_short = short_pkt.serialize();
    let parsed_short =
        ShortHeaderPacket::parse(&serialized_short, dcid.len()).expect("Parse short packet");
    assert!(parsed_short.spin_bit);
    assert!(!parsed_short.key_phase);
    assert_eq!(parsed_short.dcid, dcid);
    assert_eq!(parsed_short.packet_number, 100);
    assert_eq!(parsed_short.payload, vec![5, 6, 7, 8]);
}

#[test]
fn test_quic_frames_serialization_roundtrip() {
    // Ping frame
    let ping = QuicFrame::Ping;
    let serialized_ping = ping.serialize();
    assert_eq!(serialized_ping, vec![0x01]);

    // PathChallenge & PathResponse
    let challenge_data = [42u8; 8];
    let challenge = QuicFrame::PathChallenge {
        data: challenge_data,
    };
    let ser_challenge = challenge.serialize();
    assert_eq!(ser_challenge[0], 0x1a);
    assert_eq!(&ser_challenge[1..], &challenge_data);

    // Stream frame
    let stream_frame = QuicFrame::Stream {
        stream_id: 4,
        offset: 128,
        fin: true,
        data: vec![10, 20, 30, 40, 50],
    };
    let ser_stream = stream_frame.serialize();
    assert!(!ser_stream.is_empty());
}

#[test]
fn test_quic_connection_migration_path_validation() {
    let initial_cid = ConnectionId::new([1, 2, 3, 4]);
    let mut migration = QuicConnectionMigration::new(initial_cid.clone());
    assert!(migration.path_validated);
    assert_eq!(migration.active_cid, initial_cid);

    let new_cid = ConnectionId::new([5, 6, 7, 8]);
    migration.add_connection_id(1, new_cid.clone());

    let (challenge_frame, old_cid) = migration.initiate_migration().expect("Initiate migration");
    assert_eq!(old_cid, initial_cid);
    assert_eq!(migration.active_cid, new_cid);
    assert!(!migration.path_validated);

    if let QuicFrame::PathChallenge { data } = challenge_frame {
        let success = migration.validate_path_response(&data);
        assert!(success);
        assert!(migration.path_validated);
    } else {
        panic!("Expected PathChallenge frame");
    }
}

#[test]
fn test_qpack_static_and_dynamic_roundtrip() {
    let mut encoder = QpackEncoder::new();
    let mut decoder = QpackDecoder::new();

    let fields = vec![
        QpackHeaderField::new(":status", "200"),
        QpackHeaderField::new(":method", "POST"),
        QpackHeaderField::new(":path", "/submit"),
        QpackHeaderField::new("content-type", "application/json"),
        QpackHeaderField::new("x-custom-qpack", "h3-parity"),
    ];

    let encoded = encoder.encode(&fields);
    assert!(!encoded.is_empty());

    let decoded = decoder.decode(&encoded).expect("QPACK decoding");
    assert_eq!(decoded.len(), 5);
    assert_eq!(decoded[0].name, ":status");
    assert_eq!(decoded[0].value, "200");
    assert_eq!(decoded[1].name, ":method");
    assert_eq!(decoded[1].value, "POST");
    assert_eq!(decoded[2].name, ":path");
    assert_eq!(decoded[2].value, "/submit");
    assert_eq!(decoded[3].name, "content-type");
    assert_eq!(decoded[3].value, "application/json");
    assert_eq!(decoded[4].name, "x-custom-qpack");
    assert_eq!(decoded[4].value, "h3-parity");
}

// ===========================================================================
// 3. Content Security Policy (CSP Level 3)
// ===========================================================================

#[test]
fn test_csp_level3_directive_hierarchy_and_fallbacks() {
    // script-src-elem falls back to script-src, which falls back to default-src
    let policy_raw = "default-src 'self'; script-src https://scripts.example.com; worker-src https://workers.example.com; base-uri https://safe.example.com";
    let csp = CspPolicy::parse(policy_raw, false).expect("CSP parse");

    // script-src-elem falls back to script-src
    assert!(csp.allows(
        ResourceDirective::ScriptElem,
        "https://scripts.example.com/app.js",
        "https://myorigin.com"
    ));
    assert!(!csp.allows(
        ResourceDirective::ScriptElem,
        "https://evil.com/app.js",
        "https://myorigin.com"
    ));

    // worker-src has explicit directive
    assert!(csp.allows(
        ResourceDirective::Worker,
        "https://workers.example.com/worker.js",
        "https://myorigin.com"
    ));
    assert!(!csp.allows(
        ResourceDirective::Worker,
        "https://scripts.example.com/worker.js",
        "https://myorigin.com"
    ));

    // img-src falls back to default-src ('self')
    assert!(csp.allows(
        ResourceDirective::Image,
        "https://myorigin.com/logo.png",
        "https://myorigin.com"
    ));
    assert!(!csp.allows(
        ResourceDirective::Image,
        "https://other.com/logo.png",
        "https://myorigin.com"
    ));

    // base-uri does not fall back to default-src
    assert!(csp.allows(
        ResourceDirective::BaseUri,
        "https://safe.example.com/",
        "https://myorigin.com"
    ));
    assert!(!csp.allows(
        ResourceDirective::BaseUri,
        "https://myorigin.com/",
        "https://myorigin.com"
    ));
}

#[test]
fn test_csp_nonce_and_cryptographic_hash_validation() {
    let raw_script = b"alert('hello');";
    let digest = sha256(raw_script);
    let digest_b64 = mango_net::websocket::base64_encode(&digest);
    let policy_raw = format!("script-src 'nonce-rAnd0m123' 'sha256-{digest_b64}'");
    let csp = CspPolicy::parse(&policy_raw, false).expect("CSP parse");

    // Nonce validation
    assert!(csp.allows_nonce(ResourceDirective::Script, "rAnd0m123"));
    assert!(!csp.allows_nonce(ResourceDirective::Script, "wrong-nonce"));

    // Hash validation
    assert!(csp.allows_hash(ResourceDirective::Script, SriAlgorithm::Sha256, &digest));

    let wrong_script = b"alert('bad');";
    let wrong_digest = sha256(wrong_script);
    assert!(!csp.allows_hash(
        ResourceDirective::Script,
        SriAlgorithm::Sha256,
        &wrong_digest
    ));

    // Content hash helper
    assert!(csp.allows_content_hash(ResourceDirective::Script, raw_script));
    assert!(!csp.allows_content_hash(ResourceDirective::Script, wrong_script));
}

#[test]
fn test_csp_violation_reporting_payload_generation() {
    let report = CspViolationReport {
        document_uri: "https://example.com/page.html".to_string(),
        referrer: "https://google.com".to_string(),
        blocked_uri: "https://malicious.com/tracker.js".to_string(),
        effective_directive: "script-src-elem".to_string(),
        original_policy: "default-src 'self'".to_string(),
        disposition: "enforce".to_string(),
        status_code: 200,
        sample: Some("maliciousCode()".to_string()),
    };

    let json = report.to_json();
    assert!(json.starts_with("{\"csp-report\":{"));
    assert!(json.contains("\"document-uri\":\"https://example.com/page.html\""));
    assert!(json.contains("\"blocked-uri\":\"https://malicious.com/tracker.js\""));
    assert!(json.contains("\"effective-directive\":\"script-src-elem\""));
    assert!(json.contains("\"disposition\":\"enforce\""));
    assert!(json.contains("\"script-sample\":\"maliciousCode()\""));
}

// ===========================================================================
// 4. CORS & Preflight Validation Engine
// ===========================================================================

#[test]
fn test_cors_preflight_and_credential_modes() {
    let mut preflight_headers = HashMap::new();
    preflight_headers.insert(
        "access-control-allow-origin".to_string(),
        "https://app.example.com".to_string(),
    );
    preflight_headers.insert(
        "access-control-allow-methods".to_string(),
        "GET, POST, PUT, OPTIONS".to_string(),
    );
    preflight_headers.insert(
        "access-control-allow-headers".to_string(),
        "content-type, x-custom-token".to_string(),
    );
    preflight_headers.insert("access-control-max-age".to_string(), "600".to_string());

    // Valid preflight
    let max_age = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://app.example.com",
        "PUT",
        &["content-type", "x-custom-token"],
    )
    .expect("Preflight valid");
    assert_eq!(max_age, Duration::from_secs(600));

    // Disallowed method
    let bad_method = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://app.example.com",
        "DELETE",
        &["content-type"],
    );
    assert_eq!(
        bad_method,
        Err(CorsError::MethodNotAllowed("DELETE".to_string()))
    );

    // Disallowed header
    let bad_header = validate_cors_preflight(
        204,
        &preflight_headers,
        "https://app.example.com",
        "PUT",
        &["authorization"],
    );
    assert_eq!(
        bad_header,
        Err(CorsError::HeaderNotAllowed("authorization".to_string()))
    );

    // Credential modes: Wildcard with credentials must fail
    let mut resp_headers = HashMap::new();
    resp_headers.insert("access-control-allow-origin".to_string(), "*".to_string());

    let err = validate_cors_response_with_credentials(
        &resp_headers,
        "https://app.example.com",
        "https://api.example.com",
        RequestCredentials::Include,
    );
    assert_eq!(err, Err(CorsError::WildcardOriginWithCredentialsNotAllowed));

    // Credential mode: Omit allows wildcard
    let ok_omit = validate_cors_response_with_credentials(
        &resp_headers,
        "https://app.example.com",
        "https://api.example.com",
        RequestCredentials::Omit,
    );
    assert!(ok_omit.is_ok());

    // Credential mode: Include with explicit origin and credentials: true
    resp_headers.insert(
        "access-control-allow-origin".to_string(),
        "https://app.example.com".to_string(),
    );
    resp_headers.insert(
        "access-control-allow-credentials".to_string(),
        "true".to_string(),
    );
    let ok_include = validate_cors_response_with_credentials(
        &resp_headers,
        "https://app.example.com",
        "https://api.example.com",
        RequestCredentials::Include,
    );
    assert!(ok_include.is_ok());
}

// ===========================================================================
// 5. Cookie Jar RFC 6265bis & CHIPS (Partitioned Cookies)
// ===========================================================================

#[test]
fn test_chips_partitioned_cookie_lifecycle_and_isolation() {
    let mut jar = CookieJar::new();
    let url_api = Url::parse("https://api.embed.com/session").unwrap();

    // 1. Setting Partitioned cookie without Secure must fail (CHIPS requirement)
    let insecure_part = CookieJar::parse_set_cookie("user=alice; Partitioned", &url_api);
    assert!(insecure_part.is_none());

    // 2. Set Partitioned cookie on HTTPS with Secure and partition key for site-a
    let c1 = CookieJar::parse_set_cookie_with_partition(
        "embed_sid=secretA; Secure; Partitioned; SameSite=None",
        &url_api,
        Some("https://site-a.com"),
    )
    .expect("Set partitioned cookie for site A");
    assert_eq!(c1.partition_key.as_deref(), Some("https://site-a.com"));
    assert_eq!(c1.priority, CookiePriority::Medium);
    jar.store_cookie(c1);

    // 3. Set same-name Partitioned cookie for site-b
    let c2 = CookieJar::parse_set_cookie_with_partition(
        "embed_sid=secretB; Secure; Partitioned; SameSite=None",
        &url_api,
        Some("https://site-b.com"),
    )
    .expect("Set partitioned cookie for site B");
    assert_eq!(c2.partition_key.as_deref(), Some("https://site-b.com"));
    jar.store_cookie(c2);

    // Both partitioned cookies coexist under different partition keys
    assert_eq!(jar.len(), 2);

    // 4. Retrieve cookie in context of site-a
    let cookies_a = jar.get_cookies_with_context_and_partition(
        &url_api,
        None,
        false,
        true,
        Some("https://site-a.com"),
    );
    assert_eq!(cookies_a.len(), 1);
    assert_eq!(cookies_a[0].value, "secretA");

    // 5. Retrieve cookie in context of site-b
    let cookies_b = jar.get_cookies_with_context_and_partition(
        &url_api,
        None,
        false,
        true,
        Some("https://site-b.com"),
    );
    assert_eq!(cookies_b.len(), 1);
    assert_eq!(cookies_b[0].value, "secretB");

    // 6. Retrieve cookie in context of unpartitioned / third site
    let cookies_c = jar.get_cookies_with_context_and_partition(
        &url_api,
        None,
        false,
        true,
        Some("https://site-c.com"),
    );
    assert_eq!(cookies_c.len(), 0);
}

#[test]
fn test_cookie_priority_lru_eviction_chromium_parity() {
    let mut jar = CookieJar::new();
    let url = Url::parse("https://example.com/").unwrap();

    // Store cookies with different priorities
    // First: Low priority cookie
    let c_low = CookieJar::parse_set_cookie("c_low=1; Secure; Priority=Low", &url).unwrap();
    assert_eq!(c_low.priority, CookiePriority::Low);
    jar.store_cookie(c_low);

    // Second: High priority cookie
    let c_high = CookieJar::parse_set_cookie("c_high=1; Secure; Priority=High", &url).unwrap();
    assert_eq!(c_high.priority, CookiePriority::High);
    jar.store_cookie(c_high);

    // Fill up to 180 (MAX_COOKIES_PER_HOST) with Medium priority cookies
    for i in 0..178 {
        let cookie =
            CookieJar::parse_set_cookie(&format!("c_med_{i}={i}; Secure; Priority=Medium"), &url)
                .unwrap();
        jar.store_cookie(cookie);
    }
    assert_eq!(jar.len(), 180);

    // Now insert one more cookie (181st cookie) -> must trigger per-host eviction
    let c_extra = CookieJar::parse_set_cookie("c_extra=new; Secure; Priority=High", &url).unwrap();
    jar.store_cookie(c_extra);

    assert_eq!(jar.len(), 180);

    // Low priority cookie must be evicted first!
    let cookies = jar.get_cookies(&url);
    assert!(!cookies.iter().any(|c| c.name == "c_low"));
    // High priority cookies must still exist!
    assert!(cookies.iter().any(|c| c.name == "c_high"));
    assert!(cookies.iter().any(|c| c.name == "c_extra"));
}
