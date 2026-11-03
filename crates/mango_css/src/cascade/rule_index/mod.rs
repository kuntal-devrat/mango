//! O(1) Rule Index bucketing and candidate matching.

pub(crate) mod add_rule;
pub(crate) mod container;
pub mod index;
pub mod keys;
pub(crate) mod matching;
pub(crate) mod scope;

pub use index::RuleIndex;
pub use keys::extract_subject_keys_from_is_where;
