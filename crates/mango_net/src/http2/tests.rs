use super::*;
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
