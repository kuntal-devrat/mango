//! Fixed-size enum-indexed property map for O(1) property storage.

use super::id::{CSS_PROPERTY_COUNT, CssPropertyId};
use crate::values::Value;
use std::collections::HashMap;

/// An enum-indexed map of CSS properties.
///
/// Backed by a fixed-size `[Option<Value>; CSS_PROPERTY_COUNT]` array indexed by
/// `CssPropertyId` discriminant, providing true O(1) access with zero hashing
/// overhead and no heap allocation per entry.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyMap {
    entries: Box<[Option<Value>; CSS_PROPERTY_COUNT]>,
    len: usize,
}

impl Default for PropertyMap {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertyMap {
    /// Creates an empty `PropertyMap`.
    pub fn new() -> Self {
        Self {
            entries: Box::new(std::array::from_fn(|_| None)),
            len: 0,
        }
    }

    /// Creates an empty `PropertyMap` with pre-allocated capacity (no-op for array-backed).
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    /// Inserts a property and its value into the map.
    pub fn insert(&mut self, property: CssPropertyId, value: Value) -> Option<Value> {
        let idx = property as usize;
        let old = self.entries[idx].take();
        self.entries[idx] = Some(value);
        if old.is_none() {
            self.len += 1;
        }
        old
    }

    /// Gets a reference to the value for a property, if present.
    pub fn get(&self, property: CssPropertyId) -> Option<&Value> {
        self.entries[property as usize].as_ref()
    }

    /// Gets a mutable reference to the value for a property, if present.
    pub fn get_mut(&mut self, property: CssPropertyId) -> Option<&mut Value> {
        self.entries[property as usize].as_mut()
    }

    /// Removes a property from the map and returns its value.
    pub fn remove(&mut self, property: CssPropertyId) -> Option<Value> {
        let old = self.entries[property as usize].take();
        if old.is_some() {
            self.len -= 1;
        }
        old
    }

    /// Returns `true` if the map contains the specified property.
    pub fn contains(&self, property: CssPropertyId) -> bool {
        self.entries[property as usize].is_some()
    }

    /// Returns the number of properties in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the map contains no properties.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clears all properties from the map.
    pub fn clear(&mut self) {
        for slot in self.entries.iter_mut() {
            *slot = None;
        }
        self.len = 0;
    }

    /// Returns an iterator over property-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&CssPropertyId, &Value)> {
        CssPropertyId::all()
            .iter()
            .filter_map(move |id| self.entries[*id as usize].as_ref().map(|v| (id, v)))
    }

    /// Returns a mutable iterator over property-value pairs.
    pub fn iter_mut(&mut self) -> PropertyMapIterMut<'_> {
        PropertyMapIterMut {
            iter: self.entries.iter_mut().enumerate(),
        }
    }

    /// Converts a string-based declaration map into a strongly typed `PropertyMap`.
    /// Properties that are not recognized by `CssPropertyId` are omitted.
    pub fn from_string_map(map: &HashMap<String, Value>) -> Self {
        let mut prop_map = Self::new();
        for (k, v) in map {
            if let Some(prop_id) = CssPropertyId::from_name(k) {
                prop_map.insert(prop_id, v.clone());
            }
        }
        prop_map
    }

    /// Converts this `PropertyMap` into a standard string-keyed map for backward compatibility.
    pub fn to_string_map(&self) -> HashMap<String, Value> {
        let mut map = HashMap::with_capacity(self.len);
        for (id, val) in self.iter() {
            map.insert(id.name().to_string(), val.clone());
        }
        map
    }
}

/// Mutable iterator for `PropertyMap`.
pub struct PropertyMapIterMut<'a> {
    iter: std::iter::Enumerate<std::slice::IterMut<'a, Option<Value>>>,
}

impl<'a> Iterator for PropertyMapIterMut<'a> {
    type Item = (CssPropertyId, &'a mut Value);

    fn next(&mut self) -> Option<Self::Item> {
        for (idx, slot) in self.iter.by_ref() {
            if let Some(val) = slot {
                let id = CssPropertyId::all()[idx];
                return Some((id, val));
            }
        }
        None
    }
}
