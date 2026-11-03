//! Character encoding detection and decoding for HTML documents (PRD Section 5.4).
//!
//! Implements the full [WHATWG Encoding](https://encoding.spec.whatwg.org/) and
//! HTML5 § 13.2.3.2 detection order used by browsers when turning a byte stream into text:
//!
//! 1. **BOM sniffing** (UTF-8, UTF-16LE, UTF-16BE) — overrides all headers and meta tags
//! 2. The `charset` parameter of the `Content-Type` HTTP header
//! 3. A `<meta charset>` / `<meta http-equiv="Content-Type">` prescan of the first 1024 bytes,
//!    skipping HTML comments and respecting attribute order
//! 4. A default encoding (UTF-8, per WHATWG)
//!
//! Decoders are provided for UTF-8, UTF-16LE/BE, and the single-byte
//! ISO-8859-1 / Windows-1252 family that still dominates older pages.
//!
//! ## Example
//!
//! ```
//! use mango_net::encoding::{decode_html_bytes, detect_encoding, Encoding};
//!
//! // UTF-8 BOM wins over conflicting meta charset
//! let bytes = b"\xEF\xBB\xBF<meta charset=\"windows-1252\">caf\xc3\xa9";
//! assert!(decode_html_bytes(bytes, None).contains("café"));
//!
//! // Content-Type charset is honoured for non-UTF-8 payloads
//! let latin1 = b"caf\xe9";
//! assert_eq!(
//!     decode_html_bytes(latin1, Some("text/html; charset=iso-8859-1")),
//!     "café"
//! );
//! ```

/// A character encoding supported by Mango Browser (PRD 5.4).
///
/// Backed by the official WHATWG-standard [`encoding_rs`] engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding(pub &'static encoding_rs::Encoding);

impl Default for Encoding {
    fn default() -> Self {
        Self(encoding_rs::UTF_8)
    }
}

#[allow(non_upper_case_globals)]
impl Encoding {
    pub const Utf8: Self = Self(encoding_rs::UTF_8);
    pub const Utf16Le: Self = Self(encoding_rs::UTF_16LE);
    pub const Utf16Be: Self = Self(encoding_rs::UTF_16BE);
    pub const Windows1252: Self = Self(encoding_rs::WINDOWS_1252);

    /// Maps a charset label (from an HTTP header, `<meta charset>`, or alias) to an [`Encoding`].
    ///
    /// Follows the WHATWG Encoding Standard lookup table for aliases via [`encoding_rs`].
    pub fn from_label(label: &str) -> Option<Encoding> {
        let normalized = label.trim().trim_matches(['"', '\'']);
        let name = normalized.split([';', ',']).next().unwrap_or("").trim();
        encoding_rs::Encoding::for_label(name.as_bytes()).map(Encoding)
    }

    /// The canonical name reported to scripts (`document.characterSet`).
    pub fn name(&self) -> &'static str {
        self.0.name()
    }

    /// Returns the underlying [`encoding_rs::Encoding`].
    pub fn raw(&self) -> &'static encoding_rs::Encoding {
        self.0
    }
}

/// Decodes HTML bytes into a `String` using browser-style encoding sniffing.
///
/// `content_type` is the raw `Content-Type` HTTP header value, if any.
pub fn decode_html_bytes(bytes: &[u8], content_type: Option<&str>) -> String {
    let encoding = detect_encoding(bytes, content_type);
    decode_with(bytes, encoding)
}

/// Decodes HTML bytes into a `String` and returns the detected [`Encoding`].
pub fn decode_html_bytes_with_encoding(
    bytes: &[u8],
    content_type: Option<&str>,
) -> (String, Encoding) {
    let encoding = detect_encoding(bytes, content_type);
    (decode_with(bytes, encoding), encoding)
}

/// Determines the encoding of a document, following the WHATWG sniffing order (PRD 5.4).
///
/// 1. Byte-Order Mark (BOM) sniffing
/// 2. `Content-Type` charset parameter
/// 3. `<meta charset>` / `<meta http-equiv="Content-Type">` prescan (first 1024 bytes)
/// 4. Default to UTF-8
pub fn detect_encoding(bytes: &[u8], content_type: Option<&str>) -> Encoding {
    // 1. Byte-order mark (BOM) sniffing (overrides HTTP headers & meta tags per WHATWG)
    if let Some(enc) = sniff_bom(bytes) {
        return enc;
    }

    // 2. Content-Type charset parameter
    if let Some(label) = content_type.and_then(charset_from_content_type)
        && let Some(enc) = Encoding::from_label(&label)
    {
        return enc;
    }

    // 3. <meta charset> and <meta http-equiv="Content-Type"> prescan of the first 1024 bytes
    if let Some(label) = sniff_meta_charset(bytes)
        && let Some(enc) = Encoding::from_label(&label)
    {
        return enc;
    }

    // 4. Default
    Encoding::Utf8
}

/// Sniffs a leading Byte Order Mark (BOM) from the byte stream.
///
/// Returns `Some(Encoding)` if a UTF-8, UTF-16LE, or UTF-16BE BOM is present.
pub fn sniff_bom(bytes: &[u8]) -> Option<Encoding> {
    encoding_rs::Encoding::for_bom(bytes).map(|(enc, _)| Encoding(enc))
}

/// Extracts the `charset` parameter from a `Content-Type` HTTP header value (PRD 5.4).
///
/// Correctly handles parameters regardless of casing (`charset=`, `CharSet=`),
/// whitespace around `=`, and optional quotation marks (`"..."` or `'...'`).
pub fn charset_from_content_type(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=')
            && key.trim().eq_ignore_ascii_case("charset")
        {
            let val = val.trim().trim_matches(['"', '\'']).trim();
            let candidate: String = val
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
                .collect();
            if !candidate.is_empty() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Prescans the first 1024 bytes for a `<meta charset>` or
/// `<meta http-equiv="Content-Type" content="...charset=...">` declaration,
/// following the WHATWG HTML5 prescan specification (§ 13.2.3.2) (PRD 5.4).
///
/// Features:
/// - Skips HTML comments (`<!-- ... -->`)
/// - Parses `<meta>` attributes in any order (`charset`, `http-equiv`, `content`)
/// - Handles single/double-quoted and unquoted attribute values
/// - Ignores non-Content-Type `<meta http-equiv>` tags (e.g. `refresh`, `keywords`)
/// - Stops scanning strictly after 1024 bytes
pub fn sniff_meta_charset(bytes: &[u8]) -> Option<String> {
    let limit = bytes.len().min(1024);
    let data = &bytes[..limit];
    let mut pos = 0;

    while pos < data.len() {
        // 1. Skip comments: <!-- ... -->
        if data[pos..].starts_with(b"<!--") {
            pos += 4;
            if let Some(end) = data[pos..].windows(3).position(|w| w == b"-->") {
                pos += end + 3;
            } else {
                break; // Unclosed comment extending past 1024 bytes
            }
            continue;
        }

        // 2. Check for <meta ...>
        if data[pos..].len() >= 5
            && data[pos] == b'<'
            && (data[pos + 1..].starts_with(b"meta")
                || data[pos + 1..pos + 5].eq_ignore_ascii_case(b"meta"))
        {
            let next_byte = data.get(pos + 5).copied().unwrap_or(b'\0');
            if is_html_space(next_byte) || next_byte == b'/' || next_byte == b'>' {
                pos += 5;

                let mut charset_attr: Option<String> = None;
                let mut http_equiv_attr: Option<String> = None;
                let mut content_attr: Option<String> = None;

                // Parse attributes until '>'
                while pos < data.len() {
                    // Skip whitespace
                    while pos < data.len() && is_html_space(data[pos]) {
                        pos += 1;
                    }
                    if pos >= data.len() {
                        break;
                    }
                    if data[pos] == b'>' {
                        pos += 1;
                        break;
                    }
                    if data[pos] == b'/' {
                        pos += 1;
                        continue;
                    }

                    // Attribute name
                    let name_start = pos;
                    while pos < data.len()
                        && !is_html_space(data[pos])
                        && data[pos] != b'='
                        && data[pos] != b'/'
                        && data[pos] != b'>'
                    {
                        pos += 1;
                    }
                    let name = String::from_utf8_lossy(&data[name_start..pos]).to_ascii_lowercase();

                    // Skip whitespace before '='
                    while pos < data.len() && is_html_space(data[pos]) {
                        pos += 1;
                    }

                    let value = if pos < data.len() && data[pos] == b'=' {
                        pos += 1; // skip '='
                        // Skip whitespace after '='
                        while pos < data.len() && is_html_space(data[pos]) {
                            pos += 1;
                        }
                        if pos < data.len() && (data[pos] == b'"' || data[pos] == b'\'') {
                            let quote = data[pos];
                            pos += 1;
                            let val_start = pos;
                            while pos < data.len() && data[pos] != quote {
                                pos += 1;
                            }
                            let val = String::from_utf8_lossy(&data[val_start..pos]).into_owned();
                            if pos < data.len() && data[pos] == quote {
                                pos += 1;
                            }
                            val
                        } else {
                            let val_start = pos;
                            while pos < data.len() && !is_html_space(data[pos]) && data[pos] != b'>'
                            {
                                pos += 1;
                            }
                            String::from_utf8_lossy(&data[val_start..pos]).into_owned()
                        }
                    } else {
                        String::new()
                    };

                    match name.as_str() {
                        "charset" => {
                            if charset_attr.is_none() {
                                charset_attr = Some(value);
                            }
                        }
                        "http-equiv" => {
                            if http_equiv_attr.is_none() {
                                http_equiv_attr = Some(value);
                            }
                        }
                        "content" if content_attr.is_none() => {
                            content_attr = Some(value);
                        }
                        _ => {}
                    }
                }

                // 1. <meta charset="...">
                if let Some(c) = charset_attr {
                    let trimmed = c.trim().trim_matches(['"', '\'']).trim();
                    if !trimmed.is_empty() && Encoding::from_label(trimmed).is_some() {
                        return Some(trimmed.to_string());
                    }
                }

                // 2. <meta http-equiv="Content-Type" content="..."> fallback
                if let Some(he) = http_equiv_attr
                    && he.trim().eq_ignore_ascii_case("content-type")
                    && let Some(c) = content_attr
                    && let Some(extracted) = extract_charset_from_meta_content(&c)
                    && Encoding::from_label(&extracted).is_some()
                {
                    return Some(extracted);
                }

                continue;
            }
        }

        // 3. Skip other tags like </...>, <!...>, <?...>, or other elements
        if data[pos] == b'<' {
            pos += 1;
            // Advance to closing '>' respecting quotes
            let mut in_quote: Option<u8> = None;
            while pos < data.len() {
                let b = data[pos];
                if let Some(q) = in_quote {
                    if b == q {
                        in_quote = None;
                    }
                } else if b == b'"' || b == b'\'' {
                    in_quote = Some(b);
                } else if b == b'>' {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
            continue;
        }

        pos += 1;
    }

    None
}

/// Extracts the `charset` parameter from a `<meta content="...">` attribute value.
pub fn extract_charset_from_meta_content(content: &str) -> Option<String> {
    for part in content.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=')
            && k.trim().eq_ignore_ascii_case("charset")
        {
            let val = v.trim().trim_matches(['"', '\'']).trim();
            let candidate: String = val
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
                .collect();
            if !candidate.is_empty() {
                return Some(candidate);
            }
        }
    }
    None
}

#[inline]
fn is_html_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0C)
}

/// Decodes bytes using an explicit encoding (PRD 5.4).
///
/// Strips leading BOM if present and safely replaces invalid byte sequences
/// with the Unicode replacement character U+FFFD per WHATWG Encoding standard.
pub fn decode_with(bytes: &[u8], encoding: Encoding) -> String {
    let (cow, _) = encoding.0.decode_with_bom_removal(bytes);
    cow.into_owned()
}

/// Maps a Windows-1252 byte to its Unicode scalar value.
pub fn windows_1252_char(byte: u8) -> char {
    let raw = [byte];
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(&raw);
    cow.chars().next().unwrap_or('\u{FFFD}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bom_detection_wins() {
        let utf8_bom = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(detect_encoding(&utf8_bom, None), Encoding::Utf8);
        assert_eq!(decode_html_bytes(&utf8_bom, None), "hi");

        let utf16le = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00];
        assert_eq!(detect_encoding(&utf16le, None), Encoding::Utf16Le);
        assert_eq!(decode_html_bytes(&utf16le, None), "hi");

        let utf16be = [0xFE, 0xFF, 0x00, b'h', 0x00, b'i'];
        assert_eq!(detect_encoding(&utf16be, None), Encoding::Utf16Be);
        assert_eq!(decode_html_bytes(&utf16be, None), "hi");

        // BOM overrides conflicting Content-Type header
        let bom_override = [0xEF, 0xBB, 0xBF, 0xC3, 0xA9]; // UTF-8 BOM + "é"
        assert_eq!(
            detect_encoding(&bom_override, Some("text/html; charset=iso-8859-1")),
            Encoding::Utf8
        );
        assert_eq!(
            decode_html_bytes(&bom_override, Some("text/html; charset=iso-8859-1")),
            "é"
        );
    }

    #[test]
    fn test_content_type_charset() {
        assert_eq!(
            charset_from_content_type("text/html; charset=UTF-8").as_deref(),
            Some("UTF-8")
        );
        assert_eq!(
            charset_from_content_type("text/html; charset=\"windows-1252\"").as_deref(),
            Some("windows-1252")
        );
        assert_eq!(
            charset_from_content_type("text/html; charset = 'iso-8859-1'").as_deref(),
            Some("iso-8859-1")
        );
        assert_eq!(
            charset_from_content_type("text/html; CharSet=utf-8").as_deref(),
            Some("utf-8")
        );
        assert_eq!(
            charset_from_content_type("charset=windows-1252").as_deref(),
            Some("windows-1252")
        );
        assert_eq!(charset_from_content_type("text/html"), None);

        assert_eq!(
            detect_encoding(b"caf\xe9", Some("text/html; charset=iso-8859-1")),
            Encoding::Windows1252
        );
        assert_eq!(
            decode_html_bytes(b"caf\xe9", Some("text/html; charset=iso-8859-1")),
            "café"
        );
    }

    #[test]
    fn test_meta_charset_sniffing() {
        let html = br#"<html><head><meta charset="windows-1252"><title>caf</title></head></html>"#;
        assert_eq!(detect_encoding(html, None), Encoding::Windows1252);

        let single_quote = br#"<meta charset='windows-1252'>"#;
        assert_eq!(detect_encoding(single_quote, None), Encoding::Windows1252);

        let unquoted = br#"<meta charset=windows-1252>"#;
        assert_eq!(detect_encoding(unquoted, None), Encoding::Windows1252);

        let spaces = br#"<meta charset = "windows-1252">"#;
        assert_eq!(detect_encoding(spaces, None), Encoding::Windows1252);

        let utf8_page = br#"<meta charset="utf-8">"#;
        assert_eq!(detect_encoding(utf8_page, None), Encoding::Utf8);
    }

    #[test]
    fn test_meta_http_equiv_content_type() {
        let legacy = br#"<meta http-equiv="Content-Type" content="text/html; charset=latin1">"#;
        assert_eq!(detect_encoding(legacy, None), Encoding::Windows1252);

        // Reverse attribute order: content first, then http-equiv
        let reverse_order =
            br#"<meta content="text/html; charset=iso-8859-1" http-equiv="Content-Type">"#;
        assert_eq!(detect_encoding(reverse_order, None), Encoding::Windows1252);

        // Case insensitivity in attribute values
        let upper =
            br#"<META HTTP-EQUIV="CONTENT-TYPE" CONTENT="text/html; CHARSET=WINDOWS-1252">"#;
        assert_eq!(detect_encoding(upper, None), Encoding::Windows1252);
    }

    #[test]
    fn test_meta_http_equiv_refresh_or_keywords_ignored() {
        // http-equiv="refresh" with a charset in URL must NOT be treated as document charset
        let refresh = br#"<meta http-equiv="refresh" content="5; url=http://example.com/?charset=iso-8859-1">"#;
        assert_eq!(detect_encoding(refresh, None), Encoding::Utf8);

        // meta keywords containing the word charset must NOT trigger false positive
        let keywords = br#"<meta name="keywords" content="character sets and utf8">"#;
        assert_eq!(detect_encoding(keywords, None), Encoding::Utf8);
    }

    #[test]
    fn test_meta_in_comment_is_skipped() {
        let commented = br#"<!-- <meta charset="windows-1252"> --> <meta charset="utf-8">"#;
        assert_eq!(detect_encoding(commented, None), Encoding::Utf8);

        let commented2 = br#"<!-- <meta http-equiv="Content-Type" content="text/html; charset=latin1"> --> <meta charset="utf-8">"#;
        assert_eq!(detect_encoding(commented2, None), Encoding::Utf8);
    }

    #[test]
    fn test_meta_charset_only_scans_first_1024_bytes() {
        let mut html = vec![b'a'; 2000];
        let meta = br#"<meta charset="windows-1252">"#;
        html.extend_from_slice(meta);
        // The meta tag sits past the prescan window, so the default applies.
        assert_eq!(detect_encoding(&html, None), Encoding::Utf8);
    }

    #[test]
    fn test_windows_1252_smart_quotes_and_symbols() {
        // 0x93/0x94 are the Windows-1252 left/right double quotes.
        let decoded = decode_with(&[0x93, b'q', 0x94], Encoding::Windows1252);
        assert_eq!(decoded, "\u{201C}q\u{201D}");
        assert_eq!(decode_with(b"caf\xe9", Encoding::Windows1252), "café");

        // 0x80 is Euro (€), 0x99 is Trademark (™)
        let symbols = decode_with(&[0x80, b' ', 0x99], Encoding::Windows1252);
        assert_eq!(symbols, "€ ™");
    }

    #[test]
    fn test_encoding_labels_comprehensive() {
        assert_eq!(Encoding::from_label("UTF-8"), Some(Encoding::Utf8));
        assert_eq!(Encoding::from_label("utf8"), Some(Encoding::Utf8));
        assert_eq!(
            Encoding::from_label("unicode-1-1-utf-8"),
            Some(Encoding::Utf8)
        );
        assert_eq!(
            Encoding::from_label(" latin1 "),
            Some(Encoding::Windows1252)
        );
        assert_eq!(
            Encoding::from_label("iso-8859-1"),
            Some(Encoding::Windows1252)
        );
        assert_eq!(
            Encoding::from_label("windows-1252"),
            Some(Encoding::Windows1252)
        );
        assert_eq!(Encoding::from_label("cp1252"), Some(Encoding::Windows1252));
        assert_eq!(
            Encoding::from_label("us-ascii"),
            Some(Encoding::Windows1252)
        );
        assert_eq!(Encoding::from_label("ascii"), Some(Encoding::Windows1252));
        assert_eq!(Encoding::from_label("utf-16"), Some(Encoding::Utf16Le));
        assert_eq!(Encoding::from_label("utf-16le"), Some(Encoding::Utf16Le));
        assert_eq!(Encoding::from_label("utf-16be"), Some(Encoding::Utf16Be));
        assert_eq!(Encoding::from_label("unknown-encoding"), None);
        assert_eq!(Encoding::Utf8.name(), "UTF-8");
        assert_eq!(Encoding::Windows1252.name(), "windows-1252");
    }

    #[test]
    fn test_invalid_utf8_is_replaced_not_lost() {
        let decoded = decode_html_bytes(b"ok\xff\xfe!", None);
        assert!(decoded.starts_with("ok"));
        assert!(decoded.ends_with('!'));
    }

    #[test]
    fn test_utf16_odd_length_replacement() {
        // UTF-16 with an odd trailing byte (3 bytes)
        let le = [b'A', 0x00, 0xFF];
        let decoded = decode_with(&le, Encoding::Utf16Le);
        assert!(decoded.starts_with('A'));
        assert!(decoded.contains('\u{FFFD}'));
    }

    #[test]
    fn test_decode_html_bytes_with_encoding() {
        let (html, enc) =
            decode_html_bytes_with_encoding(b"caf\xe9", Some("text/html; charset=latin1"));
        assert_eq!(html, "café");
        assert_eq!(enc, Encoding::Windows1252);
    }

    #[test]
    fn test_whatwg_legacy_encodings_support() {
        // Shift_JIS test
        let sjis_enc = Encoding::from_label("shift_jis").expect("shift_jis supported");
        assert_eq!(sjis_enc.name(), "Shift_JIS");

        // GBK / GB18030 test
        let gbk_enc = Encoding::from_label("gbk").expect("gbk supported");
        assert_eq!(gbk_enc.name(), "GBK");

        // Windows-1251 (Cyrillic) test
        let w1251_enc = Encoding::from_label("windows-1251").expect("windows-1251 supported");
        assert_eq!(w1251_enc.name(), "windows-1251");

        // EUC-JP test
        let euc_jp = Encoding::from_label("euc-jp").expect("euc-jp supported");
        assert_eq!(euc_jp.name(), "EUC-JP");
    }
}
