//! CSS selectors: simple, compound, complex selectors, and DOM matching engine.

pub(crate) mod has;
pub mod nav;
pub mod nesting;
pub mod nth;
pub(crate) mod pseudo;
pub mod types;
pub mod validation;

#[cfg(test)]
mod tests;

pub use nav::{
    is_descendant_of, next_element_sibling, parent_element, prev_element_sibling,
    subsequent_element_siblings,
};
pub use nesting::expand_nested_selectors;
pub use nth::matches_an_plus_b;
pub use types::{
    AttributeOperator, Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector,
    current_host_element, current_scope_root, set_current_host_element, set_current_scope_root,
};
pub use validation::element_is_valid;

pub(crate) use has::get_or_parse_complex_selector;
