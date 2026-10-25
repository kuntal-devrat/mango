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
use std::ops::Index;
use std::sync::Arc;

/// A handle to an interned string. Cheap to copy, compare, and order (just a `u32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InternedString {
    index: u32,
}

impl InternedString {
    /// The canonical empty interned string handle (index 0).
    pub const EMPTY: InternedString = InternedString { index: 0 };

    /// Creates a new `InternedString` handle from a raw index.
    #[inline]
    pub const fn from_raw(index: u32) -> Self {
        Self { index }
    }

    /// Creates a new `InternedString` handle from a raw index. Alias for [`from_raw`](Self::from_raw).
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self { index }
    }

    /// Returns the raw index of this interned string.
    #[inline]
    pub fn raw(self) -> u32 {
        self.index
    }

    /// Returns the raw index of this interned string. Alias for [`raw`](Self::raw).
    #[inline]
    pub fn index(self) -> u32 {
        self.index
    }

    /// Returns `true` if this handle refers to the empty string `""` at index 0.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.index == 0
    }
}

impl fmt::Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InternedString({})", self.index)
    }
}

/// Stores unique strings and hands out [`InternedString`] handles.
///
/// Uses `Arc<str>` internally to ensure exactly one heap allocation per unique
/// string, eliminating duplicate string storage across hash map keys and index lists.
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
/// assert_eq!(&interner[a], "div");
/// ```
#[derive(Debug, Clone)]
pub struct StringInterner {
    /// Map from string content to its index in `strings`.
    map: HashMap<Arc<str>, InternedString>,
    /// The actual stored strings, indexed by `InternedString::index`.
    strings: Vec<Arc<str>>,
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
    /// for page-specific strings. The empty string `""` is always pre-interned at index 0.
    pub fn with_common_strings() -> Self {
        // Pre-intern the empty string followed by common HTML tag and attribute names
        let common = [
            "",
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
    /// Performs exactly a single heap allocation (`Arc::from(s)`) for new strings.
    /// The `Arc` is shared between the lookup map and index vector.
    pub fn intern(&mut self, s: &str) -> InternedString {
        if let Some(&existing) = self.map.get(s) {
            return existing;
        }

        let index = u32::try_from(self.strings.len())
            .expect("StringInterner overflow: exceeded u32::MAX unique strings");
        let handle = InternedString { index };
        let arc: Arc<str> = Arc::from(s);
        self.strings.push(Arc::clone(&arc));
        self.map.insert(arc, handle);
        handle
    }

    /// Normalizes an ASCII string to lowercase and interns it.
    ///
    /// For inputs up to 64 bytes, uses a stack buffer to convert to lowercase and check
    /// whether the normalized string is already interned, avoiding all heap allocations on cache hits.
    pub fn intern_ascii_lowercase(&mut self, s: &str) -> InternedString {
        if !s.bytes().any(|b| b.is_ascii_uppercase()) {
            return self.intern(s);
        }

        // Fast path for tag and attribute names up to 64 bytes: check map without heap allocation
        if s.len() <= 64 {
            let mut buf = [0u8; 64];
            let slice = &mut buf[..s.len()];
            slice.copy_from_slice(s.as_bytes());
            slice.make_ascii_lowercase();
            if let Ok(lower_str) = std::str::from_utf8(slice) {
                if let Some(&existing) = self.map.get(lower_str) {
                    return existing;
                }
                return self.intern(lower_str);
            }
        }

        let lower = s.to_ascii_lowercase();
        self.intern(&lower)
    }

    /// Looks up an already-interned string without mutating the interner.
    ///
    /// Returns `Some(handle)` if present, or `None` if the string has not been interned.
    #[inline]
    pub fn get(&self, s: &str) -> Option<InternedString> {
        self.map.get(s).copied()
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
        self.strings.get(handle.index as usize).map(|s| s.as_ref())
    }

    /// Returns an iterator over all interned strings and their handles in index order.
    pub fn iter(&self) -> impl Iterator<Item = (InternedString, &str)> {
        self.strings
            .iter()
            .enumerate()
            .map(|(i, s)| (InternedString::from_raw(i as u32), s.as_ref()))
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

    /// Returns the allocated capacity of the internal string storage.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.strings.capacity()
    }

    /// Reserves capacity for at least `additional` more unique strings.
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
        self.strings.reserve(additional);
    }

    /// Shrinks the capacity of internal collections to fit the number of elements.
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
        self.strings.shrink_to_fit();
    }

    /// Clears all interned strings, resetting the interner while preserving
    /// the canonical empty string `""` at index 0 ([`InternedString::EMPTY`]).
    ///
    /// Preserves allocated capacity for reuse. All previously returned
    /// `InternedString` handles (except `InternedString::EMPTY`) become invalid after this call.
    pub fn clear(&mut self) {
        self.map.clear();
        self.strings.clear();
        self.intern("");
    }

    /// Clears all interned strings and re-interns the canonical empty string `""` at index 0 ([`InternedString::EMPTY`]).
    ///
    /// Alias for [`clear`](Self::clear).
    pub fn clear_retaining_empty(&mut self) {
        self.clear();
    }

    /// Resets the interner, clearing all strings and restoring the canonical set of common HTML/CSS strings.
    pub fn reset_with_common_strings(&mut self) {
        self.map.clear();
        self.strings.clear();
        *self = Self::with_common_strings();
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

impl Index<InternedString> for StringInterner {
    type Output = str;

    #[inline]
    fn index(&self, handle: InternedString) -> &str {
        self.resolve(handle)
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
    use std::collections::BTreeSet;

    #[test]
    fn test_intern_returns_same_handle() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        assert_eq!(a, b);
        assert_eq!(interner.len(), 1);
        assert_eq!(&interner[a], "hello");
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
    fn test_resolve_and_try_resolve() {
        let mut interner = StringInterner::new();
        let handle = interner.intern("mango");
        assert_eq!(interner.resolve(handle), "mango");
        assert_eq!(interner.try_resolve(handle), Some("mango"));
        let bad = InternedString::from_raw(9999);
        assert_eq!(interner.try_resolve(bad), None);
    }

    #[test]
    fn test_with_common_strings() {
        let interner = StringInterner::with_common_strings();
        // Empty string is at index 0
        assert_eq!(interner.resolve(InternedString::EMPTY), "");
        assert_eq!(interner.get(""), Some(InternedString::EMPTY));
        assert!(interner.len() > 50);
        assert!(interner.contains("div"));
        assert!(interner.contains("span"));
    }

    #[test]
    fn test_read_only_get() {
        let mut interner = StringInterner::new();
        let handle = interner.intern("foo");
        assert_eq!(interner.get("foo"), Some(handle));
        assert_eq!(interner.get("bar"), None);
    }

    #[test]
    fn test_intern_ascii_lowercase() {
        let mut interner = StringInterner::new();
        let h1 = interner.intern("div");
        let h2 = interner.intern_ascii_lowercase("DIV");
        let h3 = interner.intern_ascii_lowercase("DiV");
        assert_eq!(h1, h2);
        assert_eq!(h1, h3);
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut interner = StringInterner::new();
        interner.intern("a");
        interner.intern("b");
        assert_eq!(interner.len(), 2);
        interner.clear();
        assert_eq!(interner.len(), 1);
        assert_eq!(interner.resolve(InternedString::EMPTY), "");
        let h = interner.intern("c");
        assert_eq!(interner.resolve(h), "c");
        assert_eq!(h.raw(), 1);
    }

    #[test]
    fn test_clear_retaining_empty() {
        let mut interner = StringInterner::with_common_strings();
        assert_eq!(interner.resolve(InternedString::EMPTY), "");
        interner.intern("custom_var");
        interner.clear_retaining_empty();
        assert_eq!(interner.len(), 1);
        assert_eq!(interner.resolve(InternedString::EMPTY), "");
        assert_eq!(interner.get(""), Some(InternedString::EMPTY));
        let h = interner.intern("first_after_clear");
        assert_ne!(h, InternedString::EMPTY);
        assert_eq!(h.raw(), 1);
    }

    #[test]
    fn test_reset_with_common_strings() {
        let mut interner = StringInterner::new();
        interner.intern("random");
        interner.reset_with_common_strings();
        assert_eq!(interner.resolve(InternedString::EMPTY), "");
        assert!(interner.contains("div"));
        assert!(interner.contains("span"));
    }

    #[test]
    fn test_contains() {
        let mut interner = StringInterner::new();
        interner.intern("hello");
        assert!(interner.contains("hello"));
        assert!(!interner.contains("world"));
    }

    #[test]
    fn test_iter() {
        let mut interner = StringInterner::new();
        interner.intern("alpha");
        interner.intern("beta");
        let items: Vec<(InternedString, &str)> = interner.iter().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].1, "alpha");
        assert_eq!(items[1].1, "beta");
    }

    #[test]
    fn test_interned_string_ordering() {
        let a = InternedString::from_raw(1);
        let b = InternedString::from_raw(2);
        assert!(a < b);
        let mut set = BTreeSet::new();
        set.insert(b);
        set.insert(a);
        let sorted: Vec<_> = set.into_iter().collect();
        assert_eq!(sorted, vec![a, b]);
    }

    #[test]
    fn test_capacity_and_shrink() {
        let mut interner = StringInterner::with_capacity(100);
        assert!(interner.capacity() >= 100);
        interner.intern("item");
        interner.shrink_to_fit();
        assert!(interner.capacity() >= 1);
    }

    #[test]
    fn test_interned_string_display() {
        let mut interner = StringInterner::new();
        let h = interner.intern("test");
        assert_eq!(format!("{h}"), "InternedString(0)");
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StringInterner>();
        assert_send_sync::<InternedString>();
    }
}
