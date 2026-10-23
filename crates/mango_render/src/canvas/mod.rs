//! HTML5 Canvas 2D backing store and vector rasterization engine.

pub mod context;
pub mod drawing;
pub mod pixel;
pub mod registry;
pub mod state;
pub mod text;

#[cfg(test)]
mod tests;

pub use context::Canvas2D;
pub use registry::{
    clear_canvas_registry, get_canvas_dimensions, get_canvas_pixels, remove_canvas,
    remove_canvases_for_document, resize_canvas, with_canvas_mut,
};
pub use state::CanvasState;
