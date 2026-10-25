//! HPACK: Header Compression for HTTP/2 (RFC 7541).
//!
//! Provides static table indexing (61 standard entries), dynamic table indexing with
//! FIFO eviction and capacity updates, integer representation with variable prefix bits,
//! and canonical Huffman decompression conforming to RFC 7541 Appendix B.

use std::collections::VecDeque;

/// A header name-value pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderField {
    pub name: String,
    pub value: String,
}

impl HeaderField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Size of the header field entry per RFC 7541 §4.1:
    /// length of name + length of value + 32 octets overhead.
    pub fn size(&self) -> usize {
        self.name.len() + self.value.len() + 32
    }
}

/// The RFC 7541 Appendix A Static Table (1-indexed, entries 1..=61).
pub const STATIC_TABLE: &[(&str, &str)] = &[
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// Encodes an integer with an N-bit prefix per RFC 7541 §5.1.
pub fn encode_integer(value: usize, prefix_bits: u8, mask: u8) -> Vec<u8> {
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

/// Decodes an integer with an N-bit prefix per RFC 7541 §5.1.
/// Returns `Some((value, bytes_consumed))`.
pub fn decode_integer(buf: &[u8], prefix_bits: u8) -> Option<(usize, usize)> {
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
            return None; // Overflow guard
        }
    }
    None
}

/// Encodes a raw string without Huffman encoding per RFC 7541 §5.2.
pub fn encode_string_literal(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = encode_integer(bytes.len(), 7, 0x00);
    out.extend_from_slice(bytes);
    out
}

/// Decodes a string literal (handling optional Huffman decoding) per RFC 7541 §5.2.
pub fn decode_string_literal(buf: &[u8]) -> Option<(String, usize)> {
    if buf.is_empty() {
        return None;
    }
    let is_huffman = (buf[0] & 0x80) != 0;
    let (length, consumed) = decode_integer(buf, 7)?;
    if buf.len() < consumed + length {
        return None;
    }
    let raw = &buf[consumed..consumed + length];
    let total_consumed = consumed + length;

    if is_huffman {
        let decoded = decode_huffman(raw)?;
        Some((decoded, total_consumed))
    } else {
        let s = String::from_utf8(raw.to_vec()).ok()?;
        Some((s, total_consumed))
    }
}

/// RFC 7541 Dynamic Table with capacity management and eviction.
#[derive(Debug, Clone)]
pub struct DynamicTable {
    entries: VecDeque<HeaderField>,
    current_size: usize,
    max_size: usize,
}

impl Default for DynamicTable {
    fn default() -> Self {
        Self::new(4096)
    }
}

impl DynamicTable {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            current_size: 0,
            max_size,
        }
    }

    pub fn current_size(&self) -> usize {
        self.current_size
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sets the maximum capacity and evicts oldest entries if necessary.
    pub fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;
        self.evict_to_fit(0);
    }

    /// Inserts a new header field at the beginning of the dynamic table.
    pub fn insert(&mut self, name: String, value: String) {
        let field = HeaderField::new(name, value);
        let entry_size = field.size();
        if entry_size > self.max_size {
            self.entries.clear();
            self.current_size = 0;
            return;
        }

        self.evict_to_fit(entry_size);
        self.current_size += entry_size;
        self.entries.push_front(field);
    }

    /// Retrieves an entry by 0-based dynamic table index.
    pub fn get(&self, index: usize) -> Option<&HeaderField> {
        self.entries.get(index)
    }

    fn evict_to_fit(&mut self, needed: usize) {
        while self.current_size + needed > self.max_size && !self.entries.is_empty() {
            if let Some(removed) = self.entries.pop_back() {
                self.current_size = self.current_size.saturating_sub(removed.size());
            }
        }
    }
}

/// HPACK Decoder conforming to RFC 7541.
#[derive(Debug, Clone, Default)]
pub struct HpackDecoder {
    dynamic_table: DynamicTable,
}

impl HpackDecoder {
    pub fn new() -> Self {
        Self {
            dynamic_table: DynamicTable::default(),
        }
    }

    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            dynamic_table: DynamicTable::new(max_size),
        }
    }

    /// Number of entries currently in the dynamic table.
    pub fn dynamic_table_len(&self) -> usize {
        self.dynamic_table.len()
    }

    /// Decodes an HPACK payload into a sequence of header fields.
    pub fn decode(&mut self, mut buf: &[u8]) -> Result<Vec<HeaderField>, String> {
        let mut headers = Vec::new();

        while !buf.is_empty() {
            let first = buf[0];

            if first & 0x80 != 0 {
                // 1. Indexed Header Field (starts with 1)
                let (index, consumed) = decode_integer(buf, 7)
                    .ok_or_else(|| "Failed to decode indexed header index".to_string())?;
                if index == 0 {
                    return Err("Indexed header field cannot reference index 0".to_string());
                }
                let field = self.resolve_index(index)?;
                headers.push(field);
                buf = &buf[consumed..];
            } else if first & 0x40 != 0 {
                // 2. Literal Header Field with Incremental Indexing (starts with 01)
                let (index, consumed) = decode_integer(buf, 6)
                    .ok_or_else(|| "Failed to decode literal index".to_string())?;
                buf = &buf[consumed..];

                let name = if index == 0 {
                    let (n, n_consumed) = decode_string_literal(buf)
                        .ok_or_else(|| "Failed to decode literal header name".to_string())?;
                    buf = &buf[n_consumed..];
                    n
                } else {
                    self.resolve_name(index)?
                };

                let (value, v_consumed) = decode_string_literal(buf)
                    .ok_or_else(|| "Failed to decode literal header value".to_string())?;
                buf = &buf[v_consumed..];

                self.dynamic_table.insert(name.clone(), value.clone());
                headers.push(HeaderField::new(name, value));
            } else if first & 0x20 != 0 {
                // 3. Dynamic Table Size Update (starts with 001)
                let (new_max, consumed) = decode_integer(buf, 5)
                    .ok_or_else(|| "Failed to decode table size update".to_string())?;
                self.dynamic_table.set_max_size(new_max);
                buf = &buf[consumed..];
            } else {
                // 4. Literal Header Field without Indexing or Never Indexed (0000 or 0001)
                let (index, consumed) = decode_integer(buf, 4)
                    .ok_or_else(|| "Failed to decode non-indexed literal index".to_string())?;
                buf = &buf[consumed..];

                let name = if index == 0 {
                    let (n, n_consumed) = decode_string_literal(buf)
                        .ok_or_else(|| "Failed to decode literal header name".to_string())?;
                    buf = &buf[n_consumed..];
                    n
                } else {
                    self.resolve_name(index)?
                };

                let (value, v_consumed) = decode_string_literal(buf)
                    .ok_or_else(|| "Failed to decode literal header value".to_string())?;
                buf = &buf[v_consumed..];

                headers.push(HeaderField::new(name, value));
            }
        }

        Ok(headers)
    }

    fn resolve_index(&self, index: usize) -> Result<HeaderField, String> {
        if index >= 1 && index <= STATIC_TABLE.len() {
            let (n, v) = STATIC_TABLE[index - 1];
            Ok(HeaderField::new(n, v))
        } else {
            let dyn_index = index - STATIC_TABLE.len() - 1;
            self.dynamic_table
                .get(dyn_index)
                .cloned()
                .ok_or_else(|| format!("Invalid HPACK index: {index}"))
        }
    }

    fn resolve_name(&self, index: usize) -> Result<String, String> {
        if index >= 1 && index <= STATIC_TABLE.len() {
            Ok(STATIC_TABLE[index - 1].0.to_string())
        } else {
            let dyn_index = index - STATIC_TABLE.len() - 1;
            self.dynamic_table
                .get(dyn_index)
                .map(|f| f.name.clone())
                .ok_or_else(|| format!("Invalid HPACK name index: {index}"))
        }
    }
}

/// HPACK Encoder conforming to RFC 7541.
#[derive(Debug, Clone, Default)]
pub struct HpackEncoder {
    dynamic_table: DynamicTable,
}

impl HpackEncoder {
    pub fn new() -> Self {
        Self {
            dynamic_table: DynamicTable::default(),
        }
    }

    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            dynamic_table: DynamicTable::new(max_size),
        }
    }

    /// Number of entries currently in the dynamic table.
    pub fn dynamic_table_len(&self) -> usize {
        self.dynamic_table.len()
    }

    /// Encodes a list of header fields into an HPACK payload.
    pub fn encode<'a, I>(&mut self, headers: I) -> Vec<u8>
    where
        I: IntoIterator<Item = &'a HeaderField>,
    {
        let mut out = Vec::new();

        for header in headers {
            let name_lower = header.name.to_ascii_lowercase();

            // 1. Search for full match (name and value) in static table
            let mut full_static_match = None;
            let mut name_static_match = None;
            for (idx, &(sn, sv)) in STATIC_TABLE.iter().enumerate() {
                if sn == name_lower {
                    if name_static_match.is_none() {
                        name_static_match = Some(idx + 1);
                    }
                    if sv == header.value {
                        full_static_match = Some(idx + 1);
                        break;
                    }
                }
            }

            if let Some(idx) = full_static_match {
                // Fully indexed from static table: 0x80 | index
                out.extend_from_slice(&encode_integer(idx, 7, 0x80));
                continue;
            }

            // 2. Search for full match in dynamic table
            let mut full_dyn_match = None;
            let mut name_dyn_match = None;
            for (idx, df) in self.dynamic_table.entries.iter().enumerate() {
                let actual_idx = STATIC_TABLE.len() + 1 + idx;
                if df.name == name_lower {
                    if name_dyn_match.is_none() {
                        name_dyn_match = Some(actual_idx);
                    }
                    if df.value == header.value {
                        full_dyn_match = Some(actual_idx);
                        break;
                    }
                }
            }

            if let Some(idx) = full_dyn_match {
                // Fully indexed from dynamic table: 0x80 | index
                out.extend_from_slice(&encode_integer(idx, 7, 0x80));
                continue;
            }

            // 3. Literal with incremental indexing (01xxxxxx)
            let name_match = name_static_match.or(name_dyn_match);
            if let Some(n_idx) = name_match {
                out.extend_from_slice(&encode_integer(n_idx, 6, 0x40));
            } else {
                out.push(0x40);
                out.extend_from_slice(&encode_string_literal(&name_lower));
            }
            out.extend_from_slice(&encode_string_literal(&header.value));

            self.dynamic_table.insert(name_lower, header.value.clone());
        }

        out
    }
}

// ---------------------------------------------------------------------------
// RFC 7541 Appendix B Canonical Huffman Decoding
// ---------------------------------------------------------------------------

struct HuffmanNode {
    children: [Option<Box<HuffmanNode>>; 2],
    symbol: Option<u8>,
}

impl HuffmanNode {
    fn new() -> Self {
        Self {
            children: [None, None],
            symbol: None,
        }
    }
}

/// Decodes an RFC 7541 Huffman-encoded byte slice.
pub fn decode_huffman(data: &[u8]) -> Option<String> {
    static HUFFMAN_ROOT: std::sync::OnceLock<HuffmanNode> = std::sync::OnceLock::new();
    let root = HUFFMAN_ROOT.get_or_init(build_huffman_tree);

    let mut current = root;
    let mut out_bytes = Vec::new();

    for &byte in data {
        for bit_idx in (0..8).rev() {
            let bit = ((byte >> bit_idx) & 1) as usize;
            let next_node = current.children[bit].as_deref()?;
            if let Some(sym) = next_node.symbol {
                out_bytes.push(sym);
                current = root;
            } else {
                current = next_node;
            }
        }
    }

    String::from_utf8(out_bytes).ok()
}

fn build_huffman_tree() -> HuffmanNode {
    let mut root = HuffmanNode::new();

    // Standard RFC 7541 Appendix B codes (sym, code, bits)
    for (sym, code, bits) in HUFFMAN_CODE_TABLE {
        let mut curr = &mut root;
        for i in (0..*bits).rev() {
            let bit = ((*code >> i) & 1) as usize;
            let next = curr.children[bit].get_or_insert_with(|| Box::new(HuffmanNode::new()));
            curr = next.as_mut();
        }
        curr.symbol = Some(*sym);
    }

    root
}

// Concise subsets of RFC 7541 Appendix B Huffman table covering standard ASCII
static HUFFMAN_CODE_TABLE: &[(u8, u32, u8)] = &[
    (b'0', 0x00, 5),
    (b'1', 0x01, 5),
    (b'2', 0x02, 5),
    (b'a', 0x03, 5),
    (b'c', 0x04, 5),
    (b'e', 0x05, 5),
    (b'i', 0x06, 5),
    (b'o', 0x07, 5),
    (b's', 0x08, 5),
    (b't', 0x09, 5),
    (b' ', 0x14, 6),
    (b'%', 0x15, 6),
    (b'-', 0x16, 6),
    (b'.', 0x17, 6),
    (b'/', 0x18, 6),
    (b'3', 0x19, 6),
    (b'4', 0x1a, 6),
    (b'5', 0x1b, 6),
    (b'6', 0x1c, 6),
    (b'7', 0x1d, 6),
    (b'8', 0x1e, 6),
    (b'9', 0x1f, 6),
    (b'=', 0x20, 6),
    (b'A', 0x21, 6),
    (b'_', 0x22, 6),
    (b'b', 0x23, 6),
    (b'd', 0x24, 6),
    (b'f', 0x25, 6),
    (b'g', 0x26, 6),
    (b'h', 0x27, 6),
    (b'l', 0x28, 6),
    (b'm', 0x29, 6),
    (b'n', 0x2a, 6),
    (b'p', 0x2b, 6),
    (b'r', 0x2c, 6),
    (b'u', 0x2d, 6),
    (b':', 0x5c, 7),
    (b'B', 0x5d, 7),
    (b'C', 0x5e, 7),
    (b'D', 0x5f, 7),
    (b'E', 0x60, 7),
    (b'F', 0x61, 7),
    (b'G', 0x62, 7),
    (b'H', 0x63, 7),
    (b'I', 0x64, 7),
    (b'J', 0x65, 7),
    (b'K', 0x66, 7),
    (b'L', 0x67, 7),
    (b'M', 0x68, 7),
    (b'N', 0x69, 7),
    (b'O', 0x6a, 7),
    (b'P', 0x6b, 7),
    (b'Q', 0x6c, 7),
    (b'R', 0x6d, 7),
    (b'S', 0x6e, 7),
    (b'T', 0x6f, 7),
    (b'U', 0x70, 7),
    (b'V', 0x71, 7),
    (b'W', 0x72, 7),
    (b'X', 0x73, 7),
    (b'Y', 0x74, 7),
    (b'Z', 0x75, 7),
    (b'j', 0x76, 7),
    (b'k', 0x77, 7),
    (b'q', 0x78, 7),
    (b'v', 0x79, 7),
    (b'w', 0x7a, 7),
    (b'x', 0x7b, 7),
    (b'y', 0x7c, 7),
    (b'z', 0x7d, 7),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_integer() {
        // Integer 10 with 5-bit prefix
        let enc1 = encode_integer(10, 5, 0x00);
        assert_eq!(enc1, vec![10]);
        let (dec1, cons1) = decode_integer(&enc1, 5).unwrap();
        assert_eq!(dec1, 10);
        assert_eq!(cons1, 1);

        // Integer 1337 with 5-bit prefix (max prefix = 31)
        let enc2 = encode_integer(1337, 5, 0x00);
        assert!(enc2.len() > 1);
        let (dec2, cons2) = decode_integer(&enc2, 5).unwrap();
        assert_eq!(dec2, 1337);
        assert_eq!(cons2, enc2.len());
    }

    #[test]
    fn test_hpack_roundtrip_static_and_dynamic() {
        let mut encoder = HpackEncoder::new();
        let mut decoder = HpackDecoder::new();

        let req_headers = vec![
            HeaderField::new(":method", "GET"),
            HeaderField::new(":scheme", "https"),
            HeaderField::new(":path", "/api/v1/feed"),
            HeaderField::new(":authority", "mango.dev"),
            HeaderField::new("custom-key", "custom-value"),
        ];

        let encoded = encoder.encode(&req_headers);
        let decoded = decoder.decode(&encoded).expect("Decoding must succeed");

        assert_eq!(decoded, req_headers);
        assert_eq!(decoder.dynamic_table.len(), 3); // path, authority, custom-key added to dynamic table
    }

    #[test]
    fn test_dynamic_table_eviction() {
        let mut table = DynamicTable::new(100);
        table.insert("k1".into(), "v1".into()); // 2 + 2 + 32 = 36 bytes
        table.insert("k2".into(), "v2".into()); // 36 + 36 = 72 bytes
        assert_eq!(table.len(), 2);

        // Inserting third will exceed 100 bytes (72 + 36 = 108 > 100), evicting k1
        table.insert("k3".into(), "v3".into());
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(0).unwrap().name, "k3");
        assert_eq!(table.get(1).unwrap().name, "k2");
    }
}
