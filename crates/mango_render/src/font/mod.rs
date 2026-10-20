//! Font subsystem: TrueType font loading, text measurement, and glyph rasterization.

pub mod emoji;
pub mod loader;
pub mod manager;
pub mod rasterize;
pub mod types;
pub mod variable;

#[cfg(test)]
mod tests;

pub use emoji::{
    ColorEmojiBitmap, get_or_rasterize_color_emoji, is_emoji_char, rasterize_color_emoji,
};
pub use loader::decode_font_bytes;
pub use manager::{FontManager, font_manager, prune_web_fonts};
pub use types::{
    CachedGlyph, FontFamily, FontStyle, FontWeight, TextAntialias, TextDecoration,
    bump_web_font_epoch, web_font_epoch,
};
pub use variable::{VariableFontAxis, VariableFontMetadata, interpolate_axis, parse_fvar_table};
