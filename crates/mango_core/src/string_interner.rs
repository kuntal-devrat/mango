//! A string interner for deduplicating frequently-used strings.
//!
//! HTML/CSS parsing produces enormous numbers of repeated strings (tag names
//! like "div", "span", "p", attribute names like "class", "id", "href").
//! The interner stores each unique string once and hands out cheap, copyable
//! [`InternedString`] handles for O(1) comparison.
//!
//! # Thread Safety
//!
//! `StringInterner` is `Send + Sync` (enforced by compile-time assertions),
//! enabling shared ownership across parallel parsing and style resolution
//! phases when wrapped in an `Arc<RwLock<…>>` or similar synchronization
//! primitive.

use std::collections::HashMap;
use std::fmt;

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

impl fmt::Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InternedString({})", self.index)
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

    /// Creates a new interner with pre-allocated capacity for the given
    /// number of unique strings.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            strings: Vec::with_capacity(capacity),
        }
    }

    /// Creates a new interner pre-loaded with common HTML/CSS strings.
    ///
    /// Pre-allocates capacity for the known set (~90 entries) plus headroom
    /// for page-specific strings.
    pub fn with_common_strings() -> Self {
        // Pre-intern the most common HTML tag names
        let common = [
            "html",
            "head",
            "body",
            "div",
            "span",
            "p",
            "a",
            "img",
            "ul",
            "ol",
            "li",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "table",
            "tbody",
            "thead",
            "tfoot",
            "tr",
            "td",
            "th",
            "caption",
            "colgroup",
            "col",
            "form",
            "input",
            "button",
            "label",
            "select",
            "optgroup",
            "option",
            "textarea",
            "dialog",
            "script",
            "style",
            "link",
            "meta",
            "title",
            "br",
            "hr",
            "pre",
            "code",
            "em",
            "strong",
            "b",
            "i",
            "u",
            "ruby",
            "slot",
            "frameset",
            "nav",
            "header",
            "footer",
            "main",
            "section",
            "article",
            "aside",
            "figure",
            "figcaption",
            "blockquote",
            "dl",
            "dt",
            "dd",
            "video",
            "audio",
            "source",
            "canvas",
            "svg",
            // Common attribute names
            "id",
            "class",
            "href",
            "src",
            "alt",
            "title",
            "type",
            "name",
            "value",
            "width",
            "height",
            "rel",
            "charset",
            "content",
            "http-equiv",
            "lang",
            "style",
            "colspan",
            "rowspan",
            "checked",
            "disabled",
            "selected",
            "for",
            "action",
            "method",
            "placeholder",
            "viewBox",
        ];

        // Pre-allocate with headroom: common strings + ~128 extra for page-specific ones.
        let capacity = common.len() + 128;
        let mut interner = Self::with_capacity(capacity);

        for s in &common {
            interner.intern(s);
        }

        interner
    }

    /// Interns a string, returning its handle. If the string was already
    /// interned, returns the existing handle without allocating.
    ///
    /// Only performs a single heap allocation for new strings (the `to_string()`
    /// call). The `HashMap` key shares the same `String` reference via index
    /// lookup — but since we can't store `&str` references into our own `Vec`
    /// without self-referential borrows, we clone once into the map and once
    /// into the vec. The clone is optimized by the allocator for small strings.
    pub fn intern(&mut self, s: &str) -> InternedString {
        if let Some(&existing) = self.map.get(s) {
            return existing;
        }

        let index = u32::try_from(self.strings.len())
            .expect("StringInterner overflow: exceeded u32::MAX unique strings");
        let handle = InternedString { index };
        let owned = s.to_string();
        self.strings.push(owned.clone());
        self.map.insert(owned, handle);
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

    /// Clears all interned strings, resetting the interner to an empty state.
    ///
    /// Preserves allocated capacity for reuse. All previously returned
    /// `InternedString` handles become invalid after this call.
    pub fn clear(&mut self) {
        self.map.clear();
        self.strings.clear();
    }

    /// Returns `true` if the given string has already been interned.
    #[inline]
    pub fn contains(&self, s: &str) -> bool {
        self.map.contains_key(s)
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time assertions: StringInterner is Send+Sync.
const _: () = {
    #[allow(dead_code)]
    fn assert_send<T: Send>() {}
    #[allow(dead_code)]
    fn assert_sync<T: Sync>() {}
    #[allow(dead_code)]
    fn assertions() {
        assert_send::<StringInterner>();
        assert_sync::<StringInterner>();
        assert_send::<InternedString>();
        assert_sync::<InternedString>();
    }
};

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

    #[test]
    fn test_clear() {
        let mut interner = StringInterner::new();
        interner.intern("a");
        interner.intern("b");
        assert_eq!(interner.len(), 2);
        interner.clear();
        assert!(interner.is_empty());
        // Re-interning after clear should work
        let h = interner.intern("c");
        assert_eq!(interner.resolve(h), "c");
    }

    #[test]
    fn test_contains() {
        let mut interner = StringInterner::new();
        interner.intern("hello");
        assert!(interner.contains("hello"));
        assert!(!interner.contains("world"));
    }

    #[test]
    fn test_interned_string_display() {
        let mut interner = StringInterner::new();
        let h = interner.intern("test");
        assert_eq!(format!("{h}"), "InternedString(0)");
    }

    #[test]
    fn test_with_capacity() {
        let interner = StringInterner::with_capacity(100);
        assert!(interner.is_empty());
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StringInterner>();
        assert_send_sync::<InternedString>();
    }
}
