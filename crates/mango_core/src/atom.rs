//! Lightweight, deduplicated string atom backed by `string_cache`.
//!
//! Inline storage handles strings up to 7 bytes with zero heap allocation.
//! Longer strings are deduplicated in a thread-safe global pool.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

/// A lightweight, deduplicated string atom backed by `string_cache`.
///
/// Inline storage handles strings up to 7 bytes without any heap allocation.
/// Longer strings are interned in a shared global pool.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Atom(pub string_cache::DefaultAtom);

impl Atom {
    /// Creates a new `Atom` from any string-like reference.
    #[inline]
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(string_cache::DefaultAtom::from(s.as_ref()))
    }

    /// Returns a string slice view of this atom.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Atom {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for Atom {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Atom {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Atom {
    #[inline]
    fn from(s: &str) -> Self {
        Self(string_cache::DefaultAtom::from(s))
    }
}

impl From<String> for Atom {
    #[inline]
    fn from(s: String) -> Self {
        Self(string_cache::DefaultAtom::from(s))
    }
}

impl From<&String> for Atom {
    #[inline]
    fn from(s: &String) -> Self {
        Self(string_cache::DefaultAtom::from(s.as_str()))
    }
}

impl From<string_cache::DefaultAtom> for Atom {
    #[inline]
    fn from(atom: string_cache::DefaultAtom) -> Self {
        Self(atom)
    }
}

impl PartialEq<&str> for Atom {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for Atom {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<String> for Atom {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&String> for Atom {
    #[inline]
    fn eq(&self, other: &&String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<Atom> for &str {
    #[inline]
    fn eq(&self, other: &Atom) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Atom> for str {
    #[inline]
    fn eq(&self, other: &Atom) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Atom> for String {
    #[inline]
    fn eq(&self, other: &Atom) -> bool {
        self.as_str() == other.as_str()
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Atom({:?})", self.as_str())
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for Atom {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Atom {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Atom::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_basic_operations() {
        let a = Atom::from("div");
        assert_eq!(a, "div");
        assert_eq!(&*a, "div");
        assert_eq!(a.as_str(), "div");
        assert_eq!(a.as_ref(), "div");
        assert_eq!("div", a);
        assert_eq!(a, "div".to_string());
        assert_eq!(a, Atom::from("div"));
        assert_ne!(a, "span");

        // Inline <= 7 byte storage
        assert_eq!(std::mem::size_of::<Atom>(), 8);
    }
}
