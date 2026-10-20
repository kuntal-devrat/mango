//! Painter: rasterizes display list commands into a pixel buffer.
//!
//! This is the bridge between the abstract display list and the raw pixel
//! buffer owned by `mango_platform`. The painter executes each command
//! in order, writing pixels into the buffer.
//!
//! - Scroll offset is handled by the layout and compositor systems.
//! - Text rendering delegates to `FontManager` for anti-aliased TrueType glyphs.
//! - Added `DrawImage` support for decoded image blitting.

pub(crate) mod affine;
pub(crate) mod blend;
pub(crate) mod borders;
pub(crate) mod core;
pub(crate) mod effects;
pub(crate) mod image;
pub(crate) mod shadows;
pub(crate) mod shapes;
pub(crate) mod text;

#[cfg(test)]
mod tests;

pub use self::blend::{blend_pixel, blend_pixel_mode};
pub use self::core::{paint, paint_damage, paint_with_clip};
