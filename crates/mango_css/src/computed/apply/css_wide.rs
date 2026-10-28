//! CSS-wide keywords (`inherit`, `initial`, `unset`, `revert`, `revert-layer`) application.

use std::sync::Arc;

use crate::computed::style::ComputedStyle;
use crate::property_id::CssPropertyId;
use crate::values::Value;

/// Returns true if `val` represents one of the CSS-wide global keywords.
#[inline]
pub fn is_css_wide_keyword(val: &Value) -> Option<&'static str> {
    if let Value::Keyword(k) = val {
        let lower = k.to_ascii_lowercase();
        match lower.as_str() {
            "inherit" => Some("inherit"),
            "initial" => Some("initial"),
            "unset" => Some("unset"),
            "revert" => Some("revert"),
            "revert-layer" => Some("revert-layer"),
            "revert-rule" => Some("revert-rule"),
            _ => None,
        }
    } else {
        None
    }
}

/// Applies a CSS-wide keyword declaration to `style`. Returns `true` if the declaration
/// was recognized and applied as a CSS-wide keyword.
pub fn apply_css_wide_keyword(
    style: &mut ComputedStyle,
    prop: &str,
    val: &Value,
    parent_style: Option<&ComputedStyle>,
) -> bool {
    let Some(kw) = is_css_wide_keyword(val) else {
        return false;
    };

    let initial = ComputedStyle::default();
    let prop_lower = prop.to_ascii_lowercase();

    // Custom properties: --*
    if prop_lower.starts_with("--") {
        let cp = Arc::make_mut(&mut style.custom_properties);
        match kw {
            "inherit" | "unset" => {
                if let Some(parent) = parent_style {
                    if let Some(parent_val) = parent.custom_properties.get(&prop_lower) {
                        cp.insert(prop_lower, parent_val.clone());
                    } else {
                        cp.remove(&prop_lower);
                    }
                } else {
                    cp.remove(&prop_lower);
                }
            }
            "initial" | "revert" | "revert-layer" | "revert-rule" => {
                cp.remove(&prop_lower);
            }
            _ => {}
        }
        return true;
    }

    // CSS Cascading 4 § 7.3: 'all' shorthand resets all properties except direction and unicode-bidi.
    if prop_lower == "all" {
        let dir = style.direction;
        let bidi = style.unicode_bidi;
        match kw {
            "initial" => {
                *style = initial.clone();
            }
            "inherit" => {
                let source = parent_style.unwrap_or(&initial);
                *style = source.clone();
            }
            "unset" | "revert" | "revert-layer" | "revert-rule" => {
                let mut merged = initial.clone();
                if let Some(parent) = parent_style {
                    copy_all_inherited_properties(&mut merged, parent);
                }
                *style = merged;
            }
            _ => {}
        }
        style.direction = dir;
        style.unicode_bidi = bidi;
        return true;
    }

    let is_inherited = CssPropertyId::from_name(&prop_lower)
        .map(|id| id.is_inherited())
        .unwrap_or(false);

    let use_parent = match kw {
        "inherit" => true,
        "initial" => false,
        "unset" | "revert" | "revert-layer" | "revert-rule" => is_inherited,
        _ => false,
    };

    let source = if use_parent {
        parent_style.unwrap_or(&initial)
    } else {
        &initial
    };

    copy_single_property(style, source, &prop_lower)
}

/// Copies all inherited properties from `source` to `target`.
pub fn copy_all_inherited_properties(target: &mut ComputedStyle, source: &ComputedStyle) {
    target.root_font_size = source.root_font_size;
    target.custom_properties = Arc::clone(&source.custom_properties);
    target.color = source.color;
    target.font_size = source.font_size;
    target.font_weight = source.font_weight;
    target.font_style = source.font_style;
    target.font_family = source.font_family.clone();
    target.line_height = source.line_height;
    target.text_align = source.text_align;
    target.text_transform = source.text_transform;
    target.letter_spacing = source.letter_spacing;
    target.word_spacing = source.word_spacing;
    target.text_indent = source.text_indent;
    target.direction = source.direction;
    target.visibility = source.visibility;
    target.white_space = source.white_space;
    target.list_style_type = source.list_style_type;
    target.list_style_position = source.list_style_position;
    target.cursor = source.cursor;
    target.border_collapse = source.border_collapse;
    target.border_spacing = source.border_spacing;
    target.caption_side = source.caption_side;
    target.writing_mode = source.writing_mode;
    target.scrollbar_width = source.scrollbar_width;
    target.scrollbar_color = source.scrollbar_color;
    target.word_break = source.word_break;
    target.overflow_wrap = source.overflow_wrap;
    target.hyphens = source.hyphens;
    target.text_emphasis_style = source.text_emphasis_style.clone();
    target.text_emphasis_color = source.text_emphasis_color;
    target.font_feature_settings = source.font_feature_settings.clone();
    target.font_variation_settings = source.font_variation_settings.clone();
    target.accent_color = source.accent_color;
    target.quotes = source.quotes.clone();
    target.text_wrap = source.text_wrap;
}

fn copy_single_property(style: &mut ComputedStyle, source: &ComputedStyle, prop: &str) -> bool {
    match prop {
        "color" => style.color = source.color,
        "font-size" => style.font_size = source.font_size,
        "font-weight" => style.font_weight = source.font_weight,
        "font-style" => style.font_style = source.font_style,
        "font-family" => style.font_family = source.font_family.clone(),
        "line-height" => style.line_height = source.line_height,
        "text-align" => style.text_align = source.text_align,
        "text-decoration" | "text-decoration-line" => {
            style.text_decoration = source.text_decoration
        }
        "text-decoration-style" => {
            style.text_decoration_style = source.text_decoration_style;
        }
        "text-decoration-color" => {
            style.text_decoration_color = source.text_decoration_color;
        }
        "text-decoration-thickness" => {
            style.text_decoration_thickness = source.text_decoration_thickness.clone()
        }
        "text-underline-offset" => {
            style.text_underline_offset = source.text_underline_offset;
        }
        "text-transform" => style.text_transform = source.text_transform,
        "letter-spacing" => style.letter_spacing = source.letter_spacing,
        "word-spacing" => style.word_spacing = source.word_spacing,
        "text-indent" => style.text_indent = source.text_indent,
        "text-overflow" => style.text_overflow = source.text_overflow,
        "text-shadow" => {
            style.text_shadow = source.text_shadow;
            style.text_shadows = source.text_shadows.clone();
        }
        "text-wrap" => style.text_wrap = source.text_wrap,
        "white-space" => style.white_space = source.white_space,
        "word-break" => style.word_break = source.word_break,
        "overflow-wrap" | "word-wrap" => style.overflow_wrap = source.overflow_wrap,
        "hyphens" => style.hyphens = source.hyphens,
        "direction" => style.direction = source.direction,
        "unicode-bidi" => style.unicode_bidi = source.unicode_bidi,
        "writing-mode" => style.writing_mode = source.writing_mode,
        "cursor" => style.cursor = source.cursor,
        "list-style-type" => style.list_style_type = source.list_style_type,
        "list-style-position" => style.list_style_position = source.list_style_position,
        "visibility" => style.visibility = source.visibility,
        "opacity" => style.opacity = source.opacity,
        "display" => style.display = source.display,
        "position" => style.position = source.position,
        "float" => style.float = source.float,
        "clear" => style.clear = source.clear,
        "z-index" => style.z_index = source.z_index,
        "vertical-align" => style.vertical_align = source.vertical_align,
        "box-sizing" => style.box_sizing = source.box_sizing,
        "width" | "inline-size" => style.width = source.width,
        "height" | "block-size" => style.height = source.height,
        "min-width" | "min-inline-size" => style.min_width = source.min_width,
        "max-width" | "max-inline-size" => style.max_width = source.max_width,
        "min-height" | "min-block-size" => style.min_height = source.min_height,
        "max-height" | "max-block-size" => style.max_height = source.max_height,
        "top" => style.top = source.top,
        "right" => style.right = source.right,
        "bottom" => style.bottom = source.bottom,
        "left" => style.left = source.left,
        "margin-top" | "margin-block-start" => style.margin_top = source.margin_top,
        "margin-right" | "margin-inline-end" => style.margin_right = source.margin_right,
        "margin-bottom" | "margin-block-end" => style.margin_bottom = source.margin_bottom,
        "margin-left" | "margin-inline-start" => style.margin_left = source.margin_left,
        "margin-trim" => style.margin_trim = source.margin_trim.clone(),
        "padding-top" | "padding-block-start" => style.padding_top = source.padding_top,
        "padding-right" | "padding-inline-end" => style.padding_right = source.padding_right,
        "padding-bottom" | "padding-block-end" => style.padding_bottom = source.padding_bottom,
        "padding-left" | "padding-inline-start" => style.padding_left = source.padding_left,
        "border-top-width" | "border-block-start-width" => {
            style.border_top_width = source.border_top_width
        }
        "border-right-width" | "border-inline-end-width" => {
            style.border_right_width = source.border_right_width
        }
        "border-bottom-width" | "border-block-end-width" => {
            style.border_bottom_width = source.border_bottom_width
        }
        "border-left-width" | "border-inline-start-width" => {
            style.border_left_width = source.border_left_width
        }
        "border-top-style" => style.border_top_style = source.border_top_style,
        "border-right-style" => style.border_right_style = source.border_right_style,
        "border-bottom-style" => style.border_bottom_style = source.border_bottom_style,
        "border-left-style" => style.border_left_style = source.border_left_style,
        "border-top-color" => style.border_top_color = source.border_top_color,
        "border-right-color" => style.border_right_color = source.border_right_color,
        "border-bottom-color" => style.border_bottom_color = source.border_bottom_color,
        "border-left-color" => style.border_left_color = source.border_left_color,
        "border-top-left-radius" => {
            style.border_top_left_radius = source.border_top_left_radius;
            style.border_top_left_radius_len = source.border_top_left_radius_len;
        }
        "border-top-right-radius" => {
            style.border_top_right_radius = source.border_top_right_radius;
            style.border_top_right_radius_len = source.border_top_right_radius_len;
        }
        "border-bottom-right-radius" => {
            style.border_bottom_right_radius = source.border_bottom_right_radius;
            style.border_bottom_right_radius_len = source.border_bottom_right_radius_len;
        }
        "border-bottom-left-radius" => {
            style.border_bottom_left_radius = source.border_bottom_left_radius;
            style.border_bottom_left_radius_len = source.border_bottom_left_radius_len;
        }
        "overflow" => {
            style.overflow_x = source.overflow_x;
            style.overflow_y = source.overflow_y;
        }
        "overflow-x" => style.overflow_x = source.overflow_x,
        "overflow-y" => style.overflow_y = source.overflow_y,
        "background-color" => style.background_color = source.background_color,
        "background-image" => {
            style.background_image = source.background_image.clone();
            style.background_layers = source.background_layers.clone();
            style.background_gradient = source.background_gradient.clone();
        }
        "background-repeat" => style.background_repeat = source.background_repeat,
        "background-size" => style.background_size = source.background_size,
        "background-position" => style.background_position = source.background_position,
        "background-attachment" => style.background_attachment = source.background_attachment,
        "background-clip" => style.background_clip = source.background_clip,
        "box-shadow" => {
            style.box_shadow = source.box_shadow;
            style.box_shadows = source.box_shadows.clone();
        }
        "outline-width" => style.outline_width = source.outline_width,
        "outline-style" => style.outline_style = source.outline_style,
        "outline-color" => style.outline_color = source.outline_color,
        "outline-offset" => style.outline_offset = source.outline_offset,
        "flex-direction" => style.flex_direction = source.flex_direction,
        "flex-wrap" => style.flex_wrap = source.flex_wrap,
        "flex-line-count" => style.flex_line_count = source.flex_line_count,
        "flex-grow" => style.flex_grow = source.flex_grow,
        "flex-shrink" => style.flex_shrink = source.flex_shrink,
        "flex-basis" => style.flex_basis = source.flex_basis,
        "order" => style.order = source.order,
        "align-items" => style.align_items = source.align_items,
        "align-self" => style.align_self = source.align_self,
        "align-content" => style.align_content = source.align_content,
        "justify-content" => style.justify_content = source.justify_content,
        "justify-items" => style.justify_items = source.justify_items,
        "justify-self" => style.justify_self = source.justify_self,
        "gap" => {
            style.row_gap = source.row_gap;
            style.column_gap = source.column_gap;
        }
        "row-gap" => style.row_gap = source.row_gap,
        "column-gap" => style.column_gap = source.column_gap,
        "grid-template-columns" => {
            style.grid_template_columns = source.grid_template_columns.clone()
        }
        "grid-template-rows" => style.grid_template_rows = source.grid_template_rows.clone(),
        "grid-auto-flow" => style.grid_auto_flow = source.grid_auto_flow,
        "grid-column-start" => style.grid_column_start = source.grid_column_start.clone(),
        "grid-column-end" => style.grid_column_end = source.grid_column_end.clone(),
        "grid-row-start" => style.grid_row_start = source.grid_row_start.clone(),
        "grid-row-end" => style.grid_row_end = source.grid_row_end.clone(),
        "aspect-ratio" => style.aspect_ratio = source.aspect_ratio,
        "object-fit" => style.object_fit = source.object_fit,
        "object-position" => style.object_position = source.object_position,
        "will-change" => {
            style.will_change = source.will_change;
            style.will_change_properties = source.will_change_properties.clone();
        }
        "scrollbar-width" => style.scrollbar_width = source.scrollbar_width,
        "scrollbar-color" => style.scrollbar_color = source.scrollbar_color,
        "transform" => style.transform = source.transform.clone(),
        "filter" => style.filter = source.filter.clone(),
        "backdrop-filter" => style.backdrop_filter = source.backdrop_filter.clone(),
        "content" => {
            style.content = source.content.clone();
            style.content_items = source.content_items.clone();
        }
        "quotes" => style.quotes = source.quotes.clone(),
        "accent-color" => style.accent_color = source.accent_color,
        "appearance" => style.appearance = source.appearance,
        "border-collapse" => style.border_collapse = source.border_collapse,
        "border-spacing" => style.border_spacing = source.border_spacing,
        "table-layout" => style.table_layout = source.table_layout,
        "caption-side" => style.caption_side = source.caption_side,
        "resize" => style.resize = source.resize,
        _ => return false,
    }
    true
}
