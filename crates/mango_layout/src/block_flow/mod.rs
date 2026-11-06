//! Block formatting context: normal flow, width calculation, margin collapsing, and positioning.
//!
//! Implements CSS 2.1 §9.4.1 (Block formatting contexts) and §10.3.3 (Block-level normal flow width).

pub mod anchor;
pub mod bfc;
pub mod container;
pub mod floats;
pub mod intrinsic;
pub mod intrinsic_inline;
pub mod intrinsic_vertical;
pub mod margins;
pub mod multicol;
pub mod positioned;
pub mod relative;
pub mod sizing;

#[cfg(test)]
mod tests;

pub use anchor::{clear_anchors, collect_anchors, get_anchor, register_anchor};
pub use bfc::establishes_bfc;
pub use container::{layout_block, layout_block_contents};
pub(crate) use floats::layout_single_float;
pub use intrinsic::calculate_shrink_to_fit_width;
pub use margins::{collapse_margins, is_empty_block_collapsing_through};
pub use positioned::{layout_positioned_children, layout_positioned_children_with_cb};
pub(crate) use relative::{apply_relative_offset, shift_descendants};
