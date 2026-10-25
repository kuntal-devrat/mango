//! Inline formatting context: line breaking, word wrapping, text measurement, and text-align.
//!
//! Implements CSS 2.1 §9.4.2: Inline formatting contexts.

pub mod atom;
pub mod collector;
pub mod container;
pub mod containing_block;
pub mod line;
pub mod metrics;
pub mod shaping;
pub mod vertical;

#[cfg(test)]
mod tests;

pub use container::layout_inline_children;
pub use metrics::{
    measure_text_width, measure_text_width_with_style, measure_text_width_with_style_and_spacing,
};
pub use shaping::{
    is_bold_weight, is_cjk_char, is_cjk_closing_punct, is_cjk_opening_punct, is_combining_mark,
    is_rtl_char, shape_and_bidi_text, shape_and_bidi_text_with_dir,
};
