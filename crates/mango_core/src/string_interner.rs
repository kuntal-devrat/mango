//! A string interner for deduplicating frequently-used strings.
//!
//! HTML/CSS parsing produces enormous numbers of repeated strings (tag names
//! like "div", "span", "p", attribute names like "class", "id", "href").
//! The interner stores each unique string once and hands out cheap, copyable
//! [`InternedString`] handles for O(1) comparison.

use std::collections::HashMap;

/// A handle to an interned string. Cheap to copy and compare (just a `u32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedString {
    index: u32,
}

impl InternedString {
    /// Returns the raw index of this interned string.
    #[inline]
    pub fn raw(self) -> u32 {
        self.index
    }
}

/// Stores unique strings and hands out [`InternedString`] handles.
///
/// # Example
/// ```
/// use mango_core::string_interner::StringInterner;
///
/// let mut interner = StringInterner::new();
/// let a = interner.intern("div");
/// let b = interner.intern("div");
/// let c = interner.intern("span");
///
/// assert_eq!(a, b);      // Same string → same handle
/// assert_ne!(a, c);      // Different string → different handle
/// assert_eq!(interner.resolve(a), "div");
/// ```
#[derive(Debug, Clone)]
pub struct StringInterner {
    /// Map from string content to its index in `strings`.
    map: HashMap<String, InternedString>,
    /// The actual stored strings, indexed by `InternedString::index`.
    strings: Vec<String>,
}

impl StringInterner {
    /// Creates a new, empty string interner.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    /// Creates a new interner pre-loaded with common HTML/CSS strings.
    pub fn with_common_strings() -> Self {
        let mut interner = Self::new();

        // Pre-intern the most common HTML tag names
        let common = [
            "html", "head", "body", "div", "span", "p", "a", "img", "ul", "ol", "li", "h1",
            "h2", "h3", "h4", "h5", "h6", "table", "tbody", "thead", "tfoot", "tr", "td", "th",
            "caption", "colgroup", "col", "form", "input", "button", "label", "select", "optgroup",
            "option", "textarea", "dialog", "script", "style", "link", "meta", "title", "br",
            "hr", "pre", "code", "em", "strong", "b", "i", "u", "ruby", "slot", "frameset", "nav",
            "header", "footer", "main", "section", "article", "aside", "figure", "figcaption",
            "blockquote", "dl", "dt", "dd", "video", "audio", "source", "canvas", "svg",
            // Common attribute names
            "id", "class", "href", "src", "alt", "title", "type", "name", "value", "width",
            "height", "rel", "charset", "content", "http-equiv", "lang", "style", "colspan",
            "rowspan", "checked", "disabled", "selected", "for", "action", "method", "placeholder",
            "viewBox",
        ];

        for s in &common {
            interner.intern(s);
        }

        interner
    }

    /// Interns a string, returning its handle. If the string was already
    /// interned, returns the existing handle without allocating.
    pub fn intern(&mut self, s: &str) -> InternedString {
        if let Some(&existing) = self.map.get(s) {
            return existing;
        }

        let index = u32::try_from(self.strings.len())
            .expect("StringInterner overflow: exceeded u32::MAX unique strings");
        let handle = InternedString { index };
        let owned = s.to_string();
        self.map.insert(owned.clone(), handle);
        self.strings.push(owned);
        handle
    }

    /// Normalizes an ASCII string to lowercase and interns it.
    pub fn intern_ascii_lowercase(&mut self, s: &str) -> InternedString {
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            let lower = s.to_ascii_lowercase();
            self.intern(&lower)
        } else {
            self.intern(s)
        }
    }

    /// Resolves an [`InternedString`] back to its string content.
    ///
    /// # Panics
    /// Panics if the handle doesn't belong to this interner.
    #[inline]
    pub fn resolve(&self, handle: InternedString) -> &str {
        &self.strings[handle.index as usize]
    }

    /// Safely resolves an [`InternedString`] back to its string content, returning `None`
    /// if the handle is invalid or from a different interner instance.
    #[inline]
    pub fn try_resolve(&self, handle: InternedString) -> Option<&str> {
        self.strings.get(handle.index as usize).map(|s| s.as_str())
    }

    /// Returns the number of unique strings interned.
    #[inline]
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns `true` if no strings have been interned.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_returns_same_handle() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        assert_eq!(a, b);
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn test_different_strings_different_handles() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("world");
        assert_ne!(a, b);
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_resolve() {
        let mut interner = StringInterner::new();
        let handle = interner.intern("mango");
        assert_eq!(interner.resolve(handle), "mango");
    }

    #[test]
    fn test_with_common_strings() {
        let interner = StringInterner::with_common_strings();
        // Should have pre-interned a bunch of strings
        assert!(interner.len() > 50);
    }
}
