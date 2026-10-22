//! CSS property declarations and shorthand expansion logic.

pub mod declaration;
pub mod shorthands;
pub mod supported;
pub mod validation;

#[cfg(test)]
mod tests;

pub use declaration::Declaration;
pub use supported::is_supported_property;
pub use validation::*;
