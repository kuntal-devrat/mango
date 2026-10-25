pub mod animations;
pub mod box_model;
pub mod colors_bg;
pub mod css_wide;
pub mod layout;
pub mod typography;

use std::collections::HashMap;

use crate::computed::style::ComputedStyle;
use crate::values::{BorderStyle, Display, Float, Position, Value};

pub(crate) use animations::apply_animation_property;
pub(crate) use box_model::apply_box_model_property;
pub(crate) use colors_bg::apply_colors_bg_property;
pub use css_wide::apply_css_wide_keyword;
pub(crate) use layout::apply_layout_property;
pub(crate) use typography::apply_typography_property;

/// Applies parsed, variable-resolved declarations to a ComputedStyle.
pub fn apply_cascaded_properties(
    style: &mut ComputedStyle,
    resolved_cascaded: &HashMap<String, Value>,
    parent_style: Option<&ComputedStyle>,
) {
    // ── 1. Apply 'all' property if specified ──
    if let Some(all_val) = resolved_cascaded.get("all") {
        apply_css_wide_keyword(style, "all", all_val, parent_style);
    }

    // ── 2. Font Size (must be resolved first so em lengths can use it) ──
    typography::pre_apply_font_size(style, resolved_cascaded, parent_style);

    // ── 2b. Apply color early so currentColor resolves accurately for all other properties ──
    colors_bg::pre_apply_color(style, resolved_cascaded, parent_style);

    // ── 2c. Pre-apply background base defaults & background-image deterministically ──
    colors_bg::pre_apply_background_defaults(style, resolved_cascaded);

    // ── 2d. Pre-apply writing-mode & direction so logical properties resolve accurately ──
    if let Some(wm_val) = resolved_cascaded.get("writing-mode") {
        layout::apply_layout_property(style, "writing-mode", wm_val);
    }
    if let Some(dir_val) = resolved_cascaded.get("direction") {
        typography::apply_typography_property(style, "direction", dir_val);
    }

    // ── 3. Apply all remaining cascaded properties ──
    for (prop, val) in resolved_cascaded {
        if prop == "all" {
            continue;
        }
        if apply_css_wide_keyword(style, prop, val, parent_style) {
            continue;
        }
        if apply_box_model_property(style, prop, val) {
            continue;
        }
        if apply_colors_bg_property(style, prop, val, parent_style) {
            continue;
        }
        if apply_typography_property(style, prop, val) {
            continue;
        }
        if apply_layout_property(style, prop, val) {
            continue;
        }
        if apply_animation_property(style, prop, val) {
            continue;
        }
    }

    // CSS 2.1 § 9.7: If position is absolute/fixed, or float is not none, display is blockified.
    style.original_display = style.display;
    let is_out_of_flow = matches!(style.position, Position::Absolute | Position::Fixed);
    let is_floating = style.float != Float::None;
    if is_out_of_flow || is_floating {
        style.display = match style.display {
            Display::Inline | Display::InlineBlock => Display::Block,
            Display::InlineFlex => Display::Flex,
            Display::InlineGrid => Display::Grid,
            other => other,
        };
    }

    // CSS 2.1 § 8.5.1: If the border style is 'none' or 'hidden', the computed border width is zero.
    if matches!(
        style.border_top_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_top_width = 0.0;
    }
    if matches!(
        style.border_right_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_right_width = 0.0;
    }
    if matches!(
        style.border_bottom_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_bottom_width = 0.0;
    }
    if matches!(
        style.border_left_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_left_width = 0.0;
    }
}
