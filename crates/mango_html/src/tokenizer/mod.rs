//! WHATWG HTML5 compliant tokenizer state machine.
//!
//! Emits a stream of [`Token`]s by processing characters one-by-one according
//! to the HTML parsing specification.

pub mod machine;
pub mod token;

#[cfg(test)]
mod tests;

pub use machine::Tokenizer;
pub use token::{State, Token};
