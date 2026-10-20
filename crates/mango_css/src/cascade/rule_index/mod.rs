//! O(1) Rule Index bucketing and candidate matching.

pub mod index;
pub mod keys;
pub(crate) mod matching;

pub use index::RuleIndex;
pub use keys::extract_subject_keys_from_is_where;
