//! Data URI parsing and decoding (RFC 2397).

use crate::http::NetworkError;

/// Maximum allowed data: URI byte size (10MB) to prevent OOM denial of service.
pub const MAX_DATA_URI_BYTES: usize = 10 * 1024 * 1024;

/// Supports both base64-encoded and plain text / percent-encoded data URIs.
/// Format: `data:[<mediatype>][;base64],<data>`
pub fn decode_data_uri_bytes(data_uri: &str) -> Result<Vec<u8>, NetworkError> {
    if data_uri.len() > MAX_DATA_URI_BYTES {
        return Err(NetworkError::Other(
            "data: URI exceeds 10MB size limit".to_string(),
        ));
    }
    let rest = data_uri
        .strip_prefix("data:")
        .ok_or_else(|| NetworkError::InvalidUrl("Not a data: URI".to_string()))?;
    if let Some(comma_pos) = rest.find(',') {
        let metadata = &rest[..comma_pos];
        let payload = &rest[comma_pos + 1..];
        let is_base64 = metadata
            .split(';')
            .any(|part| part.trim().eq_ignore_ascii_case("base64"));
        if is_base64 {
            decode_base64_bytes(payload.trim())
                .map_err(|e| NetworkError::Other(format!("Base64 decode error: {e}")))
        } else {
            Ok(decode_percent_bytes(payload))
        }
    } else {
        Err(NetworkError::InvalidUrl(
            "Malformed data: URI — missing comma separator".to_string(),
        ))
    }
}

pub fn decode_percent_bytes(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(d1), Some(d2)) = (h1, h2) {
                out.push(((d1 << 4) | d2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

pub fn decode_base64_bytes(input: &str) -> Result<Vec<u8>, &'static str> {
    fn val(c: u8) -> Result<u8, &'static str> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' | b'-' => Ok(62),
            b'/' | b'_' => Ok(63),
            _ => Err("Invalid base64 character"),
        }
    }

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);

    let chunks = filtered.chunks(4);
    for chunk in chunks {
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push((b2 << 6) | b3);
        }
    }

    Ok(out)
}
