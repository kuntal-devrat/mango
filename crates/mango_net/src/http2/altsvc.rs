//! HTTP/3 & Alt-Svc Support (RFC 7838 / RFC 9114).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// HTTP/3 & Alt-Svc Support (RFC 7838 / RFC 9114)
// ---------------------------------------------------------------------------

/// An Alternative Service record indicating available HTTP protocols (e.g., HTTP/3 over QUIC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltSvcRecord {
    /// ALPN token such as "h3", "h2", "h3-29".
    pub alpn: String,
    /// Host serving the alternative service (defaults to origin host if empty).
    pub host: Option<String>,
    /// Port serving the alternative service.
    pub port: u16,
    /// Maximum age in seconds for which the advertisement is valid.
    pub max_age: u64,
    /// Whether persist parameter was set.
    pub persist: bool,
}

/// Cache of Alternative Service advertisements (RFC 7838).
#[derive(Debug, Clone, Default)]
pub struct AltSvcCache {
    /// Key: origin (e.g. "https://example.com") -> list of AltSvc records.
    records: HashMap<String, Vec<AltSvcRecord>>,
}

impl AltSvcCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses an `Alt-Svc` header (e.g. `h3=":443"; ma=86400, h2="alt.example.com:8443"`).
    pub fn parse_alt_svc_header(header: &str) -> Vec<AltSvcRecord> {
        let mut results = Vec::new();
        if header.trim().eq_ignore_ascii_case("clear") {
            return results;
        }

        for entry in header.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let mut parts = entry.split(';');
            let Some(first) = parts.next() else { continue };
            let Some((alpn_raw, target_raw)) = first.split_once('=') else {
                continue;
            };

            let alpn = alpn_raw.trim().to_string();
            let target = target_raw.trim().trim_matches('"');

            let (host, port) = if let Some((h, p_str)) = target.rsplit_once(':') {
                let h_opt = if h.is_empty() {
                    None
                } else {
                    Some(h.to_string())
                };
                let p = p_str.parse::<u16>().unwrap_or(443);
                (h_opt, p)
            } else if let Ok(p) = target.parse::<u16>() {
                (None, p)
            } else {
                (None, 443)
            };

            let mut max_age = 86400; // default 24h
            let mut persist = false;

            for param in parts {
                let param = param.trim();
                if let Some((k, v)) = param.split_once('=') {
                    let k = k.trim().to_ascii_lowercase();
                    let v = v.trim().trim_matches('"');
                    if k == "ma" {
                        if let Ok(ma) = v.parse::<u64>() {
                            max_age = ma;
                        }
                    } else if k == "persist" && v == "1" {
                        persist = true;
                    }
                }
            }

            results.push(AltSvcRecord {
                alpn,
                host,
                port,
                max_age,
                persist,
            });
        }
        results
    }

    /// Registers alternative services for a given origin.
    pub fn add_records(&mut self, origin: &str, records: Vec<AltSvcRecord>) {
        let key = origin.trim().to_ascii_lowercase();
        self.records.insert(key, records);
    }

    /// Checks if HTTP/3 is advertised for this origin and returns `(target_host, port)`.
    pub fn get_h3_alternative(&self, origin: &str) -> Option<(Option<String>, u16)> {
        let key = origin.trim().to_ascii_lowercase();
        let records = self.records.get(&key)?;
        for record in records {
            if record.alpn == "h3" || record.alpn.starts_with("h3-") {
                return Some((record.host.clone(), record.port));
            }
        }
        None
    }

    /// Returns whether the origin supports HTTP/3.
    pub fn supports_h3(&self, origin: &str) -> bool {
        self.get_h3_alternative(origin).is_some()
    }
}

// ---------------------------------------------------------------------------
// HTTP/3 Frame Framing & QUIC Variable-Length Integer (RFC 9000 & RFC 9114)
// ---------------------------------------------------------------------------

/// QUIC variable-length integer encoding (RFC 9000 §16).
pub fn encode_quic_varint(val: u64) -> Vec<u8> {
    if val <= 63 {
        vec![val as u8]
    } else if val <= 16383 {
        let v = (val as u16) | 0x4000;
        v.to_be_bytes().to_vec()
    } else if val <= 1073741823 {
        let v = (val as u32) | 0x8000_0000;
        v.to_be_bytes().to_vec()
    } else {
        let v = val | 0xC000_0000_0000_0000;
        v.to_be_bytes().to_vec()
    }
}

/// QUIC variable-length integer decoding (RFC 9000 §16).
/// Returns `Some((value, bytes_consumed))`.
pub fn decode_quic_varint(buf: &[u8]) -> Option<(u64, usize)> {
    if buf.is_empty() {
        return None;
    }
    let first = buf[0];
    let prefix = first >> 6;
    match prefix {
        0 => Some((first as u64, 1)),
        1 => {
            if buf.len() < 2 {
                return None;
            }
            let val = (((first & 0x3F) as u64) << 8) | (buf[1] as u64);
            Some((val, 2))
        }
        2 => {
            if buf.len() < 4 {
                return None;
            }
            let val = (((first & 0x3F) as u64) << 24)
                | ((buf[1] as u64) << 16)
                | ((buf[2] as u64) << 8)
                | (buf[3] as u64);
            Some((val, 4))
        }
        3 => {
            if buf.len() < 8 {
                return None;
            }
            let val = (((first & 0x3F) as u64) << 56)
                | ((buf[1] as u64) << 48)
                | ((buf[2] as u64) << 40)
                | ((buf[3] as u64) << 32)
                | ((buf[4] as u64) << 24)
                | ((buf[5] as u64) << 16)
                | ((buf[6] as u64) << 8)
                | (buf[7] as u64);
            Some((val, 8))
        }
        _ => unreachable!(),
    }
}

/// HTTP/3 Binary Frame (RFC 9114 §7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http3Frame {
    pub frame_type: u64,
    pub payload: Vec<u8>,
}

impl Http3Frame {
    pub const DATA: u64 = 0x00;
    pub const HEADERS: u64 = 0x01;
    pub const CANCEL_PUSH: u64 = 0x03;
    pub const SETTINGS: u64 = 0x04;
    pub const PUSH_PROMISE: u64 = 0x05;
    pub const GOAWAY: u64 = 0x07;
    pub const MAX_PUSH_ID: u64 = 0x0D;

    pub fn new(frame_type: u64, payload: Vec<u8>) -> Self {
        Self {
            frame_type,
            payload,
        }
    }

    /// Serializes an HTTP/3 frame using QUIC varint for type and length.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&encode_quic_varint(self.frame_type));
        out.extend_from_slice(&encode_quic_varint(self.payload.len() as u64));
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parses an HTTP/3 frame from byte slice.
    pub fn parse(buf: &[u8]) -> Option<(Self, usize)> {
        let (frame_type, type_len) = decode_quic_varint(buf)?;
        let remainder = &buf[type_len..];
        let (length, len_len) = decode_quic_varint(remainder)?;
        let header_len = type_len + len_len;
        let payload_len = length as usize;
        if buf.len() < header_len + payload_len {
            return None;
        }
        let payload = buf[header_len..header_len + payload_len].to_vec();
        Some((
            Self {
                frame_type,
                payload,
            },
            header_len + payload_len,
        ))
    }
}
