//! WebRender hardware/software accelerated rendering backend for Mango.
//!
//! Translates high-level [`crate::display_list::DisplayList`] commands into
//! WebRender display items and manages a genuine WebRender [`webrender::Renderer`]
//! backed by `swgl` and OpenGL.

pub mod builder;
pub mod engine;
pub mod types;

#[cfg(test)]
mod tests;

pub use builder::{BuiltFrame, build_webrender_display_list};
pub use engine::WebRenderEngine;
pub use types::*;

use crate::display_list::DisplayList;

/// Renders a [`DisplayList`] directly to a 32-bit pixel buffer using WebRender.
///
/// Returns pixels in canonical `0xAARRGGBB` format, or `None` if WebRender is
/// unable to initialize or render the frame.
pub fn render_with_webrender(
    display_list: &DisplayList,
    width: u32,
    height: u32,
) -> Option<Vec<u32>> {
    WebRenderEngine::with_engine(width, height, |engine| {
        engine.render(display_list, width, height)
    })
}
