use super::*;

#[test]
fn test_sha1_and_base64() {
    let digest = sha1(b"The quick brown fox jumps over the lazy dog");
    let hex = digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    assert_eq!(hex, "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12");

    let empty = sha1(b"");
    let empty_hex = empty
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    assert_eq!(empty_hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");

    let b64 = base64_encode(b"Hello world");
    assert_eq!(b64, "SGVsbG8gd29ybGQ=");
}

#[test]
fn test_websocket_accept_key() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let accept = compute_accept_key(client_key);
    assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

#[test]
fn test_client_frame_roundtrip() {
    let mask = [0x12, 0x34, 0x56, 0x78];
    let payload = b"Hello RFC 6455!";
    let encoded = encode_client_frame(Opcode::Text, payload, mask);

    let mut cursor = std::io::Cursor::new(encoded);
    let (opcode, decoded_payload, fin) =
        decode_frame(&mut cursor).expect("Failed to decode valid client frame");

    assert_eq!(opcode, Opcode::Text);
    assert!(fin);
    assert_eq!(&decoded_payload[..], payload);
}

#[test]
fn test_decode_rejects_rsv_bits() {
    // 0x90 = RSV1 set on Text frame (unnegotiated extension).
    let frame = vec![0x91, 0x00];
    let mut cursor = std::io::Cursor::new(frame);
    assert!(decode_frame(&mut cursor).is_err());
}

#[test]
fn test_decode_rejects_nonminimal_length() {
    // 0x81 (FIN Text), 0x7E (16-bit len), 0x00, 0x7D (125 in 16-bit form - MUST be rejected).
    let mut frame = vec![0x81, 0x7E, 0x00, 0x7D];
    frame.extend_from_slice(&[b'a'; 125]);
    let mut cursor = std::io::Cursor::new(frame);
    assert!(decode_frame(&mut cursor).is_err());
}

#[test]
fn test_decode_rejects_fragmented_control() {
    // 0x08 = FIN=0 for Close frame (Control frames cannot be fragmented).
    let frame = vec![0x08, 0x00];
    let mut cursor = std::io::Cursor::new(frame);
    assert!(decode_frame(&mut cursor).is_err());
}

#[test]
fn test_decode_rejects_invalid_utf8_text() {
    // Text frame with invalid UTF-8 0xFF.
    let frame = vec![0x81u8, 0x01, 0xFF];
    let mut cursor = std::io::Cursor::new(frame);
    assert!(decode_frame(&mut cursor).is_err());
}

#[test]
fn test_random_nonce_and_mask_vary() {
    let a = random_bytes_16();
    let b = random_bytes_16();
    // Extremely unlikely to collide; ensures not a fixed constant.
    assert_ne!(a, b);
    let m1 = random_mask();
    let m2 = random_mask();
    // Allow rare collision but require function works; check not both zero.
    assert!(!(m1 == [0, 0, 0, 0] && m2 == [0, 0, 0, 0]));
}
