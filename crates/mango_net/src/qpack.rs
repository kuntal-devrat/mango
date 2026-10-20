//! QPACK: Field Compression for HTTP/3 (RFC 9204).
//!
//! Implements the QPACK static table (99 standard entries per RFC 9204 Appendix A),
//! dynamic table insertions and references, prefix integer encoding/decoding,
//! and field line encoders/decoders for HTTP/3 frames.

use std::collections::VecDeque;

/// A header field name-value pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QpackHeaderField {
    pub name: String,
    pub value: String,
}

impl QpackHeaderField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Entry size per RFC 9204 §3.2.1: name.len() + value.len() + 32.
    pub fn size(&self) -> usize {
        self.name.len() + self.value.len() + 32
    }
}

/// The RFC 9204 Appendix A Static Table (0-indexed, entries 0..=98).
pub const QPACK_STATIC_TABLE: &[(&str, &str)] = &[
    (":authority", ""),
    (":path", "/"),
    (":age", "0"),
    ("content-disposition", ""),
    ("content-length", "0"),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("referer", ""),
    ("set-cookie", ""),
    (":method", "CONNECT"),
    (":method", "DELETE"),
    (":method", "GET"),
    (":method", "HEAD"),
    (":method", "OPTIONS"),
    (":method", "POST"),
    (":method", "PUT"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "103"),
    (":status", "200"),
    (":status", "304"),
    (":status", "404"),
    (":status", "503"),
    ("accept", "*/*"),
    ("accept", "application/dns-message"),
    ("accept-encoding", "gzip, deflate, br"),
    ("accept-ranges", "bytes"),
    ("access-control-allow-headers", "cache-control"),
    ("access-control-allow-headers", "content-type"),
    ("access-control-allow-origin", "*"),
    ("cache-control", "max-age=0"),
    ("cache-control", "max-age=2592000"),
    ("cache-control", "max-age=604800"),
    ("cache-control", "no-cache"),
    ("cache-control", "no-store"),
    ("cache-control", "public, max-age=31536000"),
    ("content-encoding", "br"),
    ("content-encoding", "gzip"),
    ("content-type", "application/dns-message"),
    ("content-type", "application/javascript"),
    ("content-type", "application/json"),
    ("content-type", "application/x-www-form-urlencoded"),
    ("content-type", "image/gif"),
    ("content-type", "image/jpeg"),
    ("content-type", "image/png"),
    ("content-type", "text/css"),
    ("content-type", "text/html; charset=utf-8"),
    ("content-type", "text/plain"),
    ("content-type", "text/plain;charset=utf-8"),
    ("range", "bytes=0-"),
    ("strict-transport-security", "max-age=31536000"),
    (
        "strict-transport-security",
        "max-age=31536000; includesubdomains",
    ),
    (
        "strict-transport-security",
        "max-age=31536000; includesubdomains; preload",
    ),
    ("vary", "accept-encoding"),
    ("vary", "origin"),
    ("x-content-type-options", "nosniff"),
    ("x-xss-protection", "1; mode=block"),
    (":status", "100"),
    (":status", "204"),
    (":status", "206"),
    (":status", "302"),
    (":status", "400"),
    (":status", "403"),
    (":status", "421"),
    (":status", "425"),
    (":status", "500"),
    ("accept-language", ""),
    ("access-control-allow-credentials", "FALSE"),
    ("access-control-allow-credentials", "TRUE"),
    ("access-control-allow-headers", "*"),
    ("access-control-allow-methods", "get"),
    ("access-control-allow-methods", "get, post, options"),
    ("access-control-allow-methods", "options"),
    ("access-control-expose-headers", "content-length"),
    ("access-control-request-headers", "content-type"),
    ("access-control-request-method", "get"),
    ("access-control-request-method", "post"),
    ("alt-svc", "clear"),
    ("authorization", ""),
    (
        "content-security-policy",
        "script-src 'none'; object-src 'none'; base-uri 'none'",
    ),
    ("early-data", "1"),
    ("expect-ct", ""),
    ("forwarded", ""),
    ("if-range", ""),
    ("origin", ""),
    ("purpose", "prefetch"),
    ("server", ""),
    ("timing-allow-origin", "*"),
    ("upgrade-insecure-requests", "1"),
    ("user-agent", ""),
    ("x-forwarded-for", ""),
    ("x-frame-options", "deny"),
    ("x-frame-options", "sameorigin"),
];

/// Encodes an integer with an N-bit prefix per RFC 7541 / RFC 9204.
pub fn qpack_encode_integer(value: usize, prefix_bits: u8, mask: u8) -> Vec<u8> {
    assert!((1..=8).contains(&prefix_bits));
    let max_prefix = (1 << prefix_bits) - 1;
    let mut out = Vec::new();

    if value < max_prefix {
        out.push(mask | (value as u8));
    } else {
        out.push(mask | (max_prefix as u8));
        let mut rem = value - max_prefix;
        while rem >= 128 {
            out.push(((rem & 0x7F) as u8) | 0x80);
            rem >>= 7;
        }
        out.push(rem as u8);
    }
    out
}

/// Decodes an integer with an N-bit prefix.
pub fn qpack_decode_integer(buf: &[u8], prefix_bits: u8) -> Option<(usize, usize)> {
    if buf.is_empty() || !(1..=8).contains(&prefix_bits) {
        return None;
    }
    let max_prefix = (1 << prefix_bits) - 1;
    let first = buf[0] & max_prefix as u8;
    if (first as usize) < max_prefix {
        return Some((first as usize, 1));
    }

    let mut value = max_prefix;
    let mut shift = 0;
    let mut idx = 1;

    while idx < buf.len() {
        let b = buf[idx];
        idx += 1;
        value += ((b & 0x7F) as usize) << shift;
        if b & 0x80 == 0 {
            return Some((value, idx));
        }
        shift += 7;
        if shift > 64 {
            return None;
        }
    }
    None
}

/// Encodes a raw string literal.
pub fn qpack_encode_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = qpack_encode_integer(bytes.len(), 7, 0x00);
    out.extend_from_slice(bytes);
    out
}

/// Decodes a string literal from buffer.
pub fn qpack_decode_string(buf: &[u8]) -> Option<(String, usize)> {
    if buf.is_empty() {
        return None;
    }
    let (length, consumed) = qpack_decode_integer(buf, 7)?;
    if buf.len() < consumed + length {
        return None;
    }
    let raw = &buf[consumed..consumed + length];
    let s = String::from_utf8(raw.to_vec()).ok()?;
    Some((s, consumed + length))
}

/// QPACK Dynamic Table with FIFO insertion and capacity limit.
#[derive(Debug, Clone)]
pub struct QpackDynamicTable {
    entries: VecDeque<QpackHeaderField>,
    current_size: usize,
    max_size: usize,
    total_inserts: usize,
}

impl Default for QpackDynamicTable {
    fn default() -> Self {
        Self::new(4096)
    }
}

impl QpackDynamicTable {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            current_size: 0,
            max_size,
            total_inserts: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn insert(&mut self, name: String, value: String) {
        let field = QpackHeaderField::new(name, value);
        let entry_size = field.size();
        if entry_size > self.max_size {
            self.entries.clear();
            self.current_size = 0;
            return;
        }

        while self.current_size + entry_size > self.max_size && !self.entries.is_empty() {
            if let Some(removed) = self.entries.pop_back() {
                self.current_size = self.current_size.saturating_sub(removed.size());
            }
        }

        self.current_size += entry_size;
        self.total_inserts += 1;
        self.entries.push_front(field);
    }

    pub fn get(&self, relative_index: usize) -> Option<&QpackHeaderField> {
        self.entries.get(relative_index)
    }
}

/// QPACK Decoder for HTTP/3 field sections.
#[derive(Debug, Clone, Default)]
pub struct QpackDecoder {
    pub dynamic_table: QpackDynamicTable,
}

impl QpackDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes an encoded field section payload.
    pub fn decode(&mut self, mut buf: &[u8]) -> Result<Vec<QpackHeaderField>, String> {
        if buf.len() < 2 {
            return Ok(Vec::new());
        }

        // 1. Required Insert Count (8-bit prefix) & Base (7-bit prefix with S-bit)
        let (_ric, ric_consumed) = qpack_decode_integer(buf, 8)
            .ok_or_else(|| "Failed to decode Required Insert Count".to_string())?;
        buf = &buf[ric_consumed..];

        let (_base, base_consumed) =
            qpack_decode_integer(buf, 7).ok_or_else(|| "Failed to decode Base".to_string())?;
        buf = &buf[base_consumed..];

        let mut headers = Vec::new();

        while !buf.is_empty() {
            let first = buf[0];

            if first & 0x80 != 0 {
                // 1. Indexed Field Line: 1Txxxxxx (T=1: Static table, T=0: Dynamic table)
                let is_static = (first & 0x40) != 0;
                let (index, consumed) = qpack_decode_integer(buf, 6)
                    .ok_or_else(|| "Failed to decode indexed field line".to_string())?;
                buf = &buf[consumed..];

                if is_static {
                    if index < QPACK_STATIC_TABLE.len() {
                        let (n, v) = QPACK_STATIC_TABLE[index];
                        headers.push(QpackHeaderField::new(n, v));
                    } else {
                        return Err(format!("QPACK invalid static index: {index}"));
                    }
                } else if let Some(entry) = self.dynamic_table.get(index) {
                    headers.push(entry.clone());
                } else {
                    return Err(format!("QPACK invalid dynamic index: {index}"));
                }
            } else if first & 0x40 != 0 {
                // 2. Literal Field Line With Name Reference: 01NTxxxx
                let is_static = (first & 0x10) != 0;
                let (index, consumed) = qpack_decode_integer(buf, 4)
                    .ok_or_else(|| "Failed to decode literal name reference".to_string())?;
                buf = &buf[consumed..];

                let name = if is_static {
                    if index < QPACK_STATIC_TABLE.len() {
                        QPACK_STATIC_TABLE[index].0.to_string()
                    } else {
                        return Err(format!("QPACK invalid static name index: {index}"));
                    }
                } else if let Some(entry) = self.dynamic_table.get(index) {
                    entry.name.clone()
                } else {
                    return Err(format!("QPACK invalid dynamic name index: {index}"));
                };

                let (value, v_consumed) = qpack_decode_string(buf)
                    .ok_or_else(|| "Failed to decode literal value".to_string())?;
                buf = &buf[v_consumed..];

                headers.push(QpackHeaderField::new(name, value));
            } else {
                // 3. Literal Field Line Without Name Reference: 001NTxxx or 000xxxxx
                let (n_len, n_consumed) = qpack_decode_integer(buf, 3)
                    .ok_or_else(|| "Failed to decode literal without name reference".to_string())?;
                buf = &buf[n_consumed..];

                if buf.len() < n_len {
                    return Err("QPACK literal name truncated".to_string());
                }
                let name = String::from_utf8(buf[..n_len].to_vec())
                    .map_err(|e| format!("Invalid UTF-8 in header name: {e}"))?;
                buf = &buf[n_len..];

                let (value, v_consumed) = qpack_decode_string(buf)
                    .ok_or_else(|| "Failed to decode literal value".to_string())?;
                buf = &buf[v_consumed..];

                headers.push(QpackHeaderField::new(name, value));
            }
        }

        Ok(headers)
    }
}

/// QPACK Encoder for HTTP/3 field sections.
#[derive(Debug, Clone, Default)]
pub struct QpackEncoder {
    pub dynamic_table: QpackDynamicTable,
}

impl QpackEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Encodes a sequence of header fields into a QPACK field section.
    pub fn encode<'a, I>(&mut self, headers: I) -> Vec<u8>
    where
        I: IntoIterator<Item = &'a QpackHeaderField>,
    {
        let mut out = Vec::new();
        // Prefixes: Required Insert Count = 0, Base = 0 (Sign=0)
        out.push(0x00);
        out.push(0x00);

        for header in headers {
            let name_lower = header.name.to_ascii_lowercase();

            // 1. Check for exact match in static table
            let mut full_static = None;
            let mut name_static = None;
            for (idx, &(sn, sv)) in QPACK_STATIC_TABLE.iter().enumerate() {
                if sn == name_lower {
                    if name_static.is_none() {
                        name_static = Some(idx);
                    }
                    if sv == header.value {
                        full_static = Some(idx);
                        break;
                    }
                }
            }

            if let Some(idx) = full_static {
                // Indexed static field: 11xxxxxx (T=1)
                out.extend_from_slice(&qpack_encode_integer(idx, 6, 0xC0));
                continue;
            }

            // 2. Literal with static name reference: 0101xxxx (T=1, N=0)
            if let Some(n_idx) = name_static {
                out.extend_from_slice(&qpack_encode_integer(n_idx, 4, 0x50));
                out.extend_from_slice(&qpack_encode_string(&header.value));
                continue;
            }

            // 3. Literal without name reference: 00100xxx (N=0, T=0)
            let name_bytes = name_lower.as_bytes();
            out.extend_from_slice(&qpack_encode_integer(name_bytes.len(), 3, 0x20));
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(&qpack_encode_string(&header.value));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qpack_static_indexing_roundtrip() {
        let mut encoder = QpackEncoder::new();
        let mut decoder = QpackDecoder::new();

        let headers = vec![
            QpackHeaderField::new(":method", "GET"),
            QpackHeaderField::new(":scheme", "https"),
            QpackHeaderField::new(":path", "/"),
            QpackHeaderField::new(":authority", "mango.dev"),
            QpackHeaderField::new("x-custom-qpack", "mango-v3"),
        ];

        let encoded = encoder.encode(&headers);
        let decoded = decoder
            .decode(&encoded)
            .expect("QPACK decoding must succeed");

        assert_eq!(decoded, headers);
    }
}
