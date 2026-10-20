//! CSS Box Model types: box categories, dimensions, margin collapsing,
//! iframe sandboxing, flow-relative logical geometries, and replaced sizing.
//!
//! Conforms to:
//! - CSS 2.1 Chapter 8 (Box model, margin collapsing, padding, border)
//! - CSS 2.1 §10.3.2 & §10.6.2 (Replaced element sizing)
//! - CSS Box Model Module Level 3
//! - CSS Sizing Module Level 3
//! - CSS Logical Properties and Values Level 1
//! - CSS Overflow Module Level 3
//! - WHATWG HTML Living Standard §4.8.5.1 (`iframe` sandbox attributes)

pub mod box_type;
pub mod iframe;
pub mod logical;
pub mod overflow;
pub mod sizing;
pub mod strut;

#[cfg(test)]
mod tests;

pub use box_type::BoxType;
pub use iframe::IFrameSandbox;
pub use logical::{Direction, LogicalEdgeSizes, LogicalRect, WritingMode};
pub use overflow::OverflowModel;
pub use sizing::{adjust_for_box_sizing, resolve_object_fit_rect, resolve_replaced_size};
pub use strut::{BoxArea, MarginStrut};

pub use crate::dimensions::Dimensions;
