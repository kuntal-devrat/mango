//! Typed CSS property identifier system and enum-indexed map.

pub mod id;
pub mod map;
pub mod metadata;

#[cfg(test)]
mod tests;

pub use id::{CSS_PROPERTY_COUNT, CssPropertyId};
pub use map::{PropertyMap, PropertyMapIterMut};
