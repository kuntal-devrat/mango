//! CSS property declarations and shorthand expansion logic.

pub mod declaration;
pub mod shorthands;

#[cfg(test)]
mod tests;

pub use declaration::Declaration;
