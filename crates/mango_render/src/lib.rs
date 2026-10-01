//! # mango_render
//!
//! Display list generation and painting pipeline. Converts a layout tree
//! into a flat list of draw commands, then rasterizes them to a pixel buffer.
//!
//! ## Phase 2b: Font & Image Rendering
//! - **`font`**: TrueType font loading and glyph rasterization via `fontdue`.
//! - **`image_decode`**: PNG/JPEG/GIF/WebP image decoding via the `image` crate.

pub mod display_list;
pub mod painter;
pub mod color;
pub mod text;
pub mod font;
pub mod image_decode;

pub mod svg;
pub mod canvas;
pub mod pdf;

pub use canvas::{
    clear_canvas_registry, get_canvas_dimensions, get_canvas_pixels, resize_canvas,
    with_canvas_mut, Canvas2D, CanvasState,
};
pub use display_list::{BorderWidths, DiffOp, DisplayCommand, DisplayList, DisplayListDiff};
pub use font::{
    decode_font_bytes, font_manager, FontFamily, FontManager, FontStyle, FontWeight, TextDecoration,
};
pub use image_decode::{cache_image, decode_data_uri, decode_image_bytes, get_cached_image, DecodedImage};
pub use painter::paint;
pub use pdf::{print_to_pdf, render_to_pdf, PageSize, PdfOptions};
pub use svg::{
    clear_global_svg_symbols, get_global_svg_symbol, get_svg_intrinsic_dimensions,
    register_global_svg_symbol, render_svg, SvgSymbol,
};

