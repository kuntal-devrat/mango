//! Text rendering API.
//!
//! Provides convenience wrappers around the [`crate::font::FontManager`] for
//! backward-compatible text measurement used by the layout engine.

use crate::font::{FontFamily, FontWeight, font_manager};

/// Measures the width and height of a text string at the given font size.
///
/// Uses the `fontdue`-backed [`FontManager`](crate::font::FontManager) with
/// proportional advance widths for accurate measurement.
pub fn measure_text(text: &str, font_size: f32) -> (f32, f32) {
    font_manager().measure_text(text, font_size, FontWeight::Regular, FontFamily::SansSerif)
}

/// Measures text with explicit bold weight.
pub fn measure_text_bold(text: &str, font_size: f32) -> (f32, f32) {
    font_manager().measure_text(text, font_size, FontWeight::Bold, FontFamily::SansSerif)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_text_returns_positive_dimensions() {
        let (w, h) = measure_text("Hello World", 16.0);
        assert!(w > 0.0, "width should be positive, got {w}");
        assert!(h > 0.0, "height should be positive, got {h}");
    }

    #[test]
    fn test_measure_empty_string() {
        let (w, _h) = measure_text("", 16.0);
        assert_eq!(w, 0.0, "empty string should have zero width");
    }
}
