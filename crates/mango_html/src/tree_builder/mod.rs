//! WHATWG HTML5 compliant tree builder.
//!
//! Consumes tokens from [`crate::tokenizer::Tokenizer`] and builds an arena-allocated [`crate::dom::Document`].
//! Implements insertion modes, open element stack, implied tag closing, and resilient error recovery.

pub mod charset;
pub mod formatting;
pub mod modes;
pub mod stack;
pub mod types;

#[cfg(test)]
mod tests;

pub use types::{FormattingEntry, InsertionMode, TreeBuilder};

/// Convenience function: parses raw HTML string into a [`crate::dom::Document`].
pub fn parse_html(html: &str) -> crate::dom::Document {
    TreeBuilder::parse(html)
}
