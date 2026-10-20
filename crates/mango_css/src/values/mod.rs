pub mod animation;
pub mod color;
pub mod enums;
pub mod gradient;
pub mod length;
pub mod math;
pub mod transform;
pub mod value;

#[cfg(test)]
mod tests;

// Re-export all sub-modules for seamless drop-in compatibility
pub use animation::*;
pub use color::*;
pub use enums::*;
pub use gradient::*;
pub use length::*;
pub use math::*;
pub use transform::*;
pub use value::*;
