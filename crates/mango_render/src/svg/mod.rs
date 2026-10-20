//! Pure-Rust SVG parser and vector rasterizer using `tiny-skia`.

pub mod ast;
pub mod cache;
pub mod path;
pub mod render;
pub mod xml;

#[cfg(test)]
mod tests;

pub use ast::{SvgDocument, SvgNode, SvgRenderState, SvgSymbol};
pub use cache::{
    clear_global_svg_symbols, get_global_svg_symbol, get_svg_intrinsic_dimensions,
    prune_svg_caches, register_global_svg_symbol, render_svg,
};
pub use path::parse_svg_path;
