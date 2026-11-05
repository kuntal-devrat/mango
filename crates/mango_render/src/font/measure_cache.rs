//! In-memory LRU cache for text measurements powered by `quick_cache`.
//!
//! Avoids redundant per-character metric lookups and complex script shaping
//! across recurring words and phrases during layout passes.

use std::sync::OnceLock;

use quick_cache::sync::Cache;

use super::emoji::is_emoji_char;
use super::manager::FontManager;
use super::types::{FontFamily, FontWeight, web_font_epoch};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextMeasureKey {
    pub epoch: u64,
    pub family: FontFamily,
    pub weight: FontWeight,
    pub font_size_bits: u32,
    pub spacing_bits: u32,
    pub text: String,
}

static TEXT_MEASURE_CACHE: OnceLock<Cache<TextMeasureKey, (f32, f32)>> = OnceLock::new();

fn cache() -> &'static Cache<TextMeasureKey, (f32, f32)> {
    TEXT_MEASURE_CACHE.get_or_init(|| Cache::new(32_768))
}

/// Clears the text measurement cache.
pub fn clear_text_measure_cache() {
    if let Some(c) = TEXT_MEASURE_CACHE.get() {
        c.clear();
    }
}

/// Measures text width and height with LRU caching to eliminate repeated shaping.
pub fn measure_text_with_spacing_cached(
    fm: &FontManager,
    text: &str,
    font_size: f32,
    weight: FontWeight,
    family: FontFamily,
    letter_spacing: f32,
) -> (f32, f32) {
    if font_size <= 0.0 || text.is_empty() {
        return (0.0, 0.0);
    }
    let key = TextMeasureKey {
        epoch: web_font_epoch(),
        family,
        weight,
        font_size_bits: font_size.to_bits(),
        spacing_bits: letter_spacing.to_bits(),
        text: text.to_string(),
    };
    if let Some(cached) = cache().get(&key) {
        return cached;
    }

    let primary_font = fm.select_font(family, weight);
    let reg_font = fm.select_font(family, FontWeight::Regular);
    let is_synth_bold = weight == FontWeight::Bold;
    let px_size = font_size.max(1.0);

    if !is_synth_bold
        && !text.contains('\n')
        && !text.chars().any(is_emoji_char)
        && text.chars().any(super::shaping::is_complex_char)
    {
        let script_ch = text
            .chars()
            .find(|c| super::shaping::is_complex_char(*c))
            .unwrap_or(' ');
        let data = fm.select_font_data_for_char(family, weight, script_ch);
        let font = fm.select_font_for_char(family, weight, script_ch);
        if let Some(shaped) =
            super::shaping::shape_text_with_font_data(&data, 0, text, px_size, letter_spacing)
        {
            if !shaped.glyphs.is_empty() && shaped.glyphs.iter().any(|g| g.glyph_id != 0) {
                let line_metrics = font.horizontal_line_metrics(px_size);
                let height = line_metrics
                    .map(|lm| lm.new_line_size)
                    .unwrap_or(px_size * 1.2);
                let dims = (shaped.width, height);
                cache().insert(key, dims);
                return dims;
            }
        }
    }

    let shaped_text;
    let text_to_measure = if text.chars().any(super::shaping::is_complex_char) {
        shaped_text = super::shaping::shape_complex_script(text);
        &shaped_text
    } else {
        text
    };

    let mut total_width = 0.0f32;
    for ch in text_to_measure.chars() {
        if ch == '\n' || super::emoji::is_zero_width_or_format(ch) {
            continue;
        }
        if is_emoji_char(ch) {
            total_width += px_size + letter_spacing;
            continue;
        }
        let font = fm.select_font_for_char(family, weight, ch);
        let metrics = font.metrics(ch, px_size);
        let synth_extra = if is_synth_bold
            && std::ptr::eq(font.as_ref(), reg_font.as_ref())
            && !ch.is_whitespace()
        {
            if px_size >= 48.0 {
                3.0
            } else if px_size >= 28.0 {
                2.0
            } else {
                1.0
            }
        } else {
            0.0
        };
        total_width += metrics.advance_width + synth_extra + letter_spacing;
    }

    let line_metrics = primary_font.horizontal_line_metrics(px_size);
    let height = line_metrics
        .map(|lm| lm.new_line_size)
        .unwrap_or(px_size * 1.2);

    let dims = (total_width, height);
    cache().insert(key, dims);
    dims
}
