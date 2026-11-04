//! Character encoding detection from meta tags and attributes.

pub fn extract_charset_from_content(content: &str) -> Option<String> {
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
pub fn canonical_charset(label: &str) -> Option<&'static str> {
    let normalized = label.trim().trim_matches(['"', '\'']);
    let name = normalized.split([';', ',']).next().unwrap_or("").trim();
    encoding_rs::Encoding::for_label(name.as_bytes()).map(|enc| enc.name())
}
