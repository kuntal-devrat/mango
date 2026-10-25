use super::*;
use crate::ResourceLoader;
use std::sync::Arc;

#[test]
fn test_frame_header_serialization() {
    let header = FrameHeader {
        length: 12,
        frame_type: FrameType::Settings,
        flags: 0x1,
        stream_id: 0,
    };

    let raw = header.serialize();
    assert_eq!(raw.len(), 9);

    let parsed = FrameHeader::parse(&raw).expect("Must parse valid 9-byte header");
    assert_eq!(parsed, header);
    assert_eq!(parsed.frame_type, FrameType::Settings);
    assert_eq!(parsed.length, 12);
    assert_eq!(parsed.flags, 0x1);
    assert_eq!(parsed.stream_id, 0);
}

#[test]
fn test_http2_session_initialization_and_multiplexing() {
    let mut session = Http2Session::new();
    let init_bytes = session.init_connection();
    assert!(init_bytes.starts_with(HTTP2_CLIENT_PREFACE));

    let stream1 = session.open_stream();
    let stream2 = session.open_stream();
    assert_eq!(stream1, 1);
    assert_eq!(stream2, 3);
    assert_eq!(session.streams.len(), 2);

    // Server sends SETTINGS frame
    let server_settings = Http2Frame::settings(&[(0x3, 200)]);
    let ack_reply = session.handle_incoming_frame(&server_settings).unwrap();
    assert!(ack_reply.is_some());
    let ack = ack_reply.unwrap();
    assert_eq!(ack.header.frame_type, FrameType::Settings);
    assert_eq!(ack.header.flags, 0x1); // ACK flag
    assert_eq!(session.peer_settings.get(&0x3), Some(&200));

    // Server sends DATA to stream 1 with END_STREAM
    let data_frame = Http2Frame::new(FrameType::Data, 0x1, 1, b"Hello HTTP/2!".to_vec());
    let _ = session.handle_incoming_frame(&data_frame);
    let s1 = session.streams.get(&1).unwrap();
    assert_eq!(s1.response_body, b"Hello HTTP/2!");
    assert_eq!(s1.state, StreamState::HalfClosedRemote);
}

#[test]
fn test_http2_connection_pool() {
    let mut pool = Http2ConnectionPool::new();
    assert_eq!(pool.active_connections(), 0);

    let origin = "https://mango.dev:443";
    let session_arc = pool.get_or_create(origin);
    assert_eq!(pool.active_connections(), 1);

    // Fetching same origin returns identical instance
    let s2 = pool.get_session(origin).unwrap();
    assert!(Arc::ptr_eq(&session_arc, &s2));

    pool.remove(origin);
    assert_eq!(pool.active_connections(), 0);
}

#[test]
fn test_alt_svc_parsing_and_cache() {
    let header = r#"h3=":443"; ma=86400, h2="alt.example.com:8443"; ma=3600; persist=1"#;
    let records = AltSvcCache::parse_alt_svc_header(header);
    assert_eq!(records.len(), 2);

    assert_eq!(records[0].alpn, "h3");
    assert_eq!(records[0].host, None);
    assert_eq!(records[0].port, 443);
    assert_eq!(records[0].max_age, 86400);
    assert!(!records[0].persist);

    assert_eq!(records[1].alpn, "h2");
    assert_eq!(records[1].host, Some("alt.example.com".to_string()));
    assert_eq!(records[1].port, 8443);
    assert_eq!(records[1].max_age, 3600);
    assert!(records[1].persist);

    let mut cache = AltSvcCache::new();
    cache.add_records("https://example.com", records);
    assert!(cache.supports_h3("https://example.com"));
    assert_eq!(
        cache.get_h3_alternative("https://example.com"),
        Some((None, 443))
    );
    assert!(!cache.supports_h3("https://other.com"));
}

#[test]
fn test_quic_varint_and_http3_framing() {
    // Test varint 1, 2, 4, 8 byte boundaries
    let cases = [
        0u64, 25, 63, 64, 15000, 16383, 16384, 1073741823, 1073741824,
    ];
    for val in cases {
        let enc = encode_quic_varint(val);
        let (dec, consumed) = decode_quic_varint(&enc).expect("decode varint");
        assert_eq!(dec, val);
        assert_eq!(consumed, enc.len());
    }

    // Test HTTP/3 frame
    let frame = Http3Frame::new(Http3Frame::DATA, b"quic and http3 payload".to_vec());
    let serialized = frame.serialize();
    let (parsed, read_bytes) = Http3Frame::parse(&serialized).expect("parse frame");
    assert_eq!(read_bytes, serialized.len());
    assert_eq!(parsed.frame_type, Http3Frame::DATA);
    assert_eq!(parsed.payload, b"quic and http3 payload");
}

#[test]
fn test_http2_priority_stream_scheduling() {
    let mut session = Http2Session::new();
    let _ = session.init_connection();

    // 1. Open streams with different priorities in REVERSE priority order:
    // Image (weight 99) -> Font (weight 179) -> Script (weight 219) -> Style (weight 255)
    let s_image = session.send_request_with_priority(
        &[(":method", "GET"), (":path", "/logo.png")],
        false,
        StreamPriority::IMAGE,
    );
    let s_font = session.send_request_with_priority(
        &[(":method", "GET"), (":path", "/inter.woff2")],
        false,
        StreamPriority::FONT,
    );
    let s_script = session.send_request_with_priority(
        &[(":method", "GET"), (":path", "/app.js")],
        false,
        StreamPriority::SCRIPT,
    );
    let s_css = session.send_request_with_priority(
        &[(":method", "GET"), (":path", "/style.css")],
        false,
        StreamPriority::STYLE,
    );

    assert_eq!(s_image, 1);
    assert_eq!(s_font, 3);
    assert_eq!(s_script, 5);
    assert_eq!(s_css, 7);

    // Queue data frames on each stream in arbitrary order
    session.send_data(s_image, b"PNG bytes".to_vec(), true);
    session.send_data(s_font, b"WOFF2 bytes".to_vec(), true);
    session.send_data(s_script, b"JS bytes".to_vec(), true);
    session.send_data(s_css, b"CSS bytes".to_vec(), true);

    // Also queue a control frame (PING on stream 0) after all stream frames
    session.enqueue_frame(Http2Frame::ping([1, 2, 3, 4, 5, 6, 7, 8], false));

    // 2. Schedule all outbound frames
    let scheduled = session.schedule_outbound_frames();
    assert_eq!(scheduled.len(), 9); // 1 ping + 4 headers + 4 data

    // PING control frame (stream 0) must be scheduled FIRST!
    assert_eq!(scheduled[0].header.stream_id, 0);
    assert_eq!(scheduled[0].header.frame_type, FrameType::Ping);

    // Style stream (7) must be scheduled before Script (5), Font (3), and Image (1)
    let stream_ids: Vec<u32> = scheduled[1..].iter().map(|f| f.header.stream_id).collect();
    // Headers and Data for CSS (7) should be at the head
    assert_eq!(stream_ids[0], s_css);
    assert_eq!(stream_ids[1], s_css);

    // Headers and Data for Script (5) next
    assert_eq!(stream_ids[2], s_script);
    assert_eq!(stream_ids[3], s_script);

    // Headers and Data for Font (3) next
    assert_eq!(stream_ids[4], s_font);
    assert_eq!(stream_ids[5], s_font);

    // Headers and Data for Image (1) last
    assert_eq!(stream_ids[6], s_image);
    assert_eq!(stream_ids[7], s_image);
}

#[test]
fn test_http2_headers_frame_with_priority_payload() {
    let mut session = Http2Session::new();
    let stream_id = session.open_stream_with_priority(StreamPriority::STYLE);
    let frame = session.create_headers_frame_with_priority(
        stream_id,
        &[(":method", "GET"), (":path", "/reset.css")],
        true,
        StreamPriority::STYLE,
    );

    // Must have END_HEADERS (0x4), PRIORITY (0x20), and END_STREAM (0x1) flags
    assert_eq!(frame.header.flags & 0x20, 0x20);
    assert_eq!(frame.header.flags & 0x4, 0x4);
    assert_eq!(frame.header.flags & 0x1, 0x1);
    assert!(frame.payload.len() >= 5);

    // First 5 bytes: priority payload
    let dep_raw = u32::from_be_bytes([
        frame.payload[0],
        frame.payload[1],
        frame.payload[2],
        frame.payload[3],
    ]);
    let exclusive = (dep_raw & 0x8000_0000) != 0;
    let parent = dep_raw & 0x7FFF_FFFF;
    let weight = frame.payload[4];

    assert!(!exclusive);
    assert_eq!(parent, 0);
    assert_eq!(weight, 255);
}

#[test]
fn test_resource_loader_priority_and_http2_registration() {
    let loader = ResourceLoader::new();
    let (stream_id, prio) = loader.register_http2_request(
        "https://example.com:443",
        "/main.css",
        SubresourcePriority::Style,
    );
    assert_eq!(stream_id, 1);
    assert_eq!(prio.weight, 255);

    let (stream_id2, prio2) = loader.register_http2_request(
        "https://example.com:443",
        "/avatar.jpg",
        SubresourcePriority::Image,
    );
    assert_eq!(stream_id2, 3);
    assert_eq!(prio2.weight, 99);

    assert_eq!(
        ResourceLoader::subresource_priority(crate::security::ResourceDirective::Style),
        SubresourcePriority::Style
    );
    assert_eq!(
        ResourceLoader::subresource_priority(crate::security::ResourceDirective::Script),
        SubresourcePriority::Script
    );
    assert_eq!(
        ResourceLoader::subresource_priority(crate::security::ResourceDirective::Font),
        SubresourcePriority::Font
    );
    assert_eq!(
        ResourceLoader::subresource_priority(crate::security::ResourceDirective::Image),
        SubresourcePriority::Image
    );
}
