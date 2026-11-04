//! WHATWG HTML5 compliant tree builder powered by `html5ever`.

pub mod charset;

#[cfg(test)]
mod tests;

pub use charset::{canonical_charset, extract_charset_from_content};

/// Convenience function: parses raw HTML string into a [`crate::dom::Document`] using `html5ever`.
pub fn parse_html(html: &str) -> crate::dom::Document {
    crate::tree_sink::parse_html5ever(html)
}
