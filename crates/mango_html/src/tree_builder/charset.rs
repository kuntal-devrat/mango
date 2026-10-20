//! Character encoding detection from meta tags and attributes.

use super::types::TreeBuilder;

impl<'a> TreeBuilder<'a> {
    pub(crate) fn process_meta_attributes(&mut self, attributes: &[(String, String)]) {
        // 1. Check for charset="..."
        for (k, v) in attributes {
            if k.eq_ignore_ascii_case("charset")
                && let Some(canon) = canonical_charset(v)
            {
                self.doc.character_set = canon.to_string();
                return;
            }
        }

        // 2. Check for http-equiv="Content-Type" content="...charset=..."
        let is_http_equiv_ct = attributes.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("http-equiv") && v.trim().eq_ignore_ascii_case("content-type")
        });
        if is_http_equiv_ct
            && let Some((_, content)) = attributes
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content"))
            && let Some(cs) = extract_charset_from_content(content)
            && let Some(canon) = canonical_charset(&cs)
        {
            self.doc.character_set = canon.to_string();
        }
    }
}

pub(crate) fn extract_charset_from_content(content: &str) -> Option<String> {
    for part in content.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=')
            && k.trim().eq_ignore_ascii_case("charset")
        {
            let val = v.trim().trim_matches(['"', '\'']).trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Maps standard charset labels and aliases to their WHATWG canonical encoding name.
pub(crate) fn canonical_charset(label: &str) -> Option<&'static str> {
    let normalized = label.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
    let name = normalized.split([';', ',']).next().unwrap_or("").trim();
    match name {
        "utf-8" | "utf8" | "unicode-1-1-utf-8" | "unicode11utf8" | "unicode20utf8"
        | "x-unicode20utf8" | "csunicode11utf8" | "utf_8" => Some("UTF-8"),
        "utf-16" | "utf-16le" | "csunicode" | "iso-10646-ucs-2" | "ucs-2" | "unicode"
        | "unicodefeff" => Some("UTF-16LE"),
        "utf-16be" | "unicodefffe" => Some("UTF-16BE"),
        "windows-1252" | "cp1252" | "x-cp1252" | "iso-8859-1" | "iso8859-1" | "iso_8859-1"
        | "iso_8859-1:1987" | "iso-ir-100" | "latin1" | "latin-1" | "l1" | "csisolatin1"
        | "us-ascii" | "ascii" | "ansi_x3.4-1968" | "iso-ir-6" | "ansi_x3.4-1986"
        | "iso_646.irv:1991" | "iso646-us" | "us" | "ibm367" | "cp367" | "csascii" => {
            Some("windows-1252")
        }
        _ => None,
    }
}
