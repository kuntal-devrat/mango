//! Computed style resolution and CSS property inheritance.
//!
//! Converts cascaded property values into concrete, typed [`ComputedStyle`] structs
//! ready for the layout engine.

pub mod apply;
pub mod compute;
pub mod custom_props;
pub mod style;

#[cfg(test)]
mod tests;

pub use apply::apply_cascaded_properties;
pub use compute::{
    compute_pseudo_style, compute_pseudo_style_with_index, compute_pseudo_style_with_shadow_index,
    compute_style, compute_style_with_index, compute_style_with_shadow_index,
};
pub use style::ComputedStyle;
