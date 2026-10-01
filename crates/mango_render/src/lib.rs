//! # mango_render
//!
//! Display list generation and painting pipeline. Converts a layout tree
//! into a flat list of draw commands, then rasterizes them to a pixel buffer.
//!
//! ## Phase 2b: Font & Image Rendering
//! - **`font`**: TrueType font loading and glyph rasterization via `fontdue`.
//! - **`image_decode`**: PNG/JPEG/GIF/WebP image decoding via the `image` crate.

#![allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_checked_ops,
    clippy::manual_clamp,
    clippy::needless_range_loop,
    clippy::field_reassign_with_default
)]

pub mod color;
pub mod display_list;
pub mod font;
pub mod image_decode;
pub mod painter;
pub mod text;

pub mod canvas;
pub mod pdf;
pub mod svg;

pub use canvas::{
    Canvas2D, CanvasState, clear_canvas_registry, get_canvas_dimensions, get_canvas_pixels,
    resize_canvas, with_canvas_mut,
};
pub use display_list::{BorderWidths, DiffOp, DisplayCommand, DisplayList, DisplayListDiff};
pub use font::{
    FontFamily, FontManager, FontStyle, FontWeight, TextDecoration, decode_font_bytes, font_manager,
};
pub use image_decode::{
    DecodedImage, cache_image, decode_data_uri, decode_image_bytes, get_cached_image,
};
pub use painter::paint;
pub use pdf::{PageSize, PdfOptions, print_to_pdf, render_to_pdf};
pub use svg::{
    SvgSymbol, clear_global_svg_symbols, get_global_svg_symbol, get_svg_intrinsic_dimensions,
    register_global_svg_symbol, render_svg,
};
