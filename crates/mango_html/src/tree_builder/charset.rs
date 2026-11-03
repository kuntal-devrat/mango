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
    let normalized = label.trim().trim_matches(['"', '\'']);
    let name = normalized.split([';', ',']).next().unwrap_or("").trim();
    encoding_rs::Encoding::for_label(name.as_bytes()).map(|enc| enc.name())
}
