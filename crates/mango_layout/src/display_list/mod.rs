//! Display list generation: flattens the laid-out box tree into render commands.
//!
//! Generates background fills, borders, and text drawing commands respecting
//! the CSS stacking and paint order.

pub mod background;
pub mod borders;
pub mod builder;
pub mod forms;
pub mod media;
pub mod paint_layer;
pub mod progress;
pub mod stacking;
pub mod text;
pub mod types;

#[cfg(test)]
mod tests;

pub use builder::{
    build_display_list, build_display_list_with_scroll, build_display_list_with_scroll_xy,
};
pub use types::{COLOR_SWATCHES, DisplayListCache};
