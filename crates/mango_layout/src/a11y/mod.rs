//! # Accessibility (A11y) Tree & ARIA Engine
//!
//! Converts DOM elements and layout boxes into an accessible tree conforming to
//! W3C WAI-ARIA 1.2 and the HTML Accessibility API Mappings (AAM).
//!
//! Provides screen-reader semantics, accessible name computation (AccName 1.2),
//! role resolution (implicit HTML + explicit ARIA), state extraction, and
//! spatial bounding boxes for assistive technologies.

pub mod name;
pub mod role;
pub mod state;
pub mod tree;

#[cfg(test)]
mod tests;

pub use name::{
    compute_accessible_name, compute_description, compute_input_name, compute_labelable_name,
    find_child_element_text, find_label_for_id, find_wrapping_label,
};
pub use role::{A11yRole, is_inside_sectioning, match_aria_role, resolve_role};
pub use state::{
    A11yState, CheckedState, PressedState, is_natively_focusable, resolve_state, split_ids,
};
pub use tree::{
    A11yNode, A11yTree, build_bounds_cache, build_node, collect_by_name, collect_by_role,
    dump_node, find_by_node_id_rec,
};
