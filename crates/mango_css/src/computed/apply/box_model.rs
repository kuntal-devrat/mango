//! Box model, sizing, borders, margins, padding, and display properties.

use crate::computed::style::ComputedStyle;
use crate::values::{BorderStyle, Clear, Display, Float, Length, Overflow, Value, Visibility};

/// Parses a border-style keyword or value.
pub(crate) fn parse_border_style_value(val: &Value) -> Option<BorderStyle> {
    match val {
        Value::BorderStyle(s) => Some(*s),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "none" => Some(BorderStyle::None),
            "hidden" => Some(BorderStyle::Hidden),
            "solid" | "groove" | "ridge" | "inset" | "outset" => Some(BorderStyle::Solid),
            "dashed" => Some(BorderStyle::Dashed),
            "dotted" => Some(BorderStyle::Dotted),
            "double" => Some(BorderStyle::Double),
            _ => None,
        },
        Value::Display(Display::None) => Some(BorderStyle::None),
        _ => None,
    }
}

/// Applies box-model, dimension, margin, padding, border geometry, and display properties.
pub(crate) fn apply_box_model_property(style: &mut ComputedStyle, prop: &str, val: &Value) -> bool {
    match prop {
        "display" => match val {
            Value::Display(d) => style.display = *d,
            Value::Keyword(k) => match k.as_str() {
                "block" => style.display = Display::Block,
                "inline" => style.display = Display::Inline,
                "inline-block" => style.display = Display::InlineBlock,
                "flex" => style.display = Display::Flex,
                "inline-flex" => style.display = Display::InlineFlex,
                "grid" => style.display = Display::Grid,
                "inline-grid" => style.display = Display::InlineGrid,
                "flow-root" => style.display = Display::FlowRoot,
                "contents" => style.display = Display::Contents,
                "table" => style.display = Display::Table,
                "table-row" => style.display = Display::TableRow,
                "table-cell" => style.display = Display::TableCell,
                "table-caption" => style.display = Display::TableCaption,
                "list-item" => style.display = Display::ListItem,
                "ruby" => style.display = Display::Ruby,
                "ruby-base" => style.display = Display::RubyBase,
                "ruby-text" => style.display = Display::RubyText,
                "none" => style.display = Display::None,
                _ => {}
            },
            _ => {}
        },
        "position" => {
            if let Value::Position(p) = val {
                style.position = *p;
            }
        }
        "float" => {
            if let Value::Float(f) = val {
                style.float = *f;
            }
        }
        "clear" => match val {
            Value::Clear(c) => style.clear = *c,
            Value::Float(Float::Left) => style.clear = Clear::Left,
            Value::Float(Float::Right) => style.clear = Clear::Right,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "left" => style.clear = Clear::Left,
                "right" => style.clear = Clear::Right,
                "both" => style.clear = Clear::Both,
                "none" => style.clear = Clear::None,
                _ => {}
            },
            _ => {}
        },

        // Dimensions
        "width" => match val {
            Value::Length(l) => style.width = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.width = Length::Auto,
                "content" => style.width = Length::Content,
                "min-content" => style.width = Length::MinContent,
                "max-content" => style.width = Length::MaxContent,
                "fit-content" => style.width = Length::FitContent,
                _ => {}
            },
            _ => {}
        },
        "height" => match val {
            Value::Length(l) => style.height = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.height = Length::Auto,
                "content" => style.height = Length::Content,
                "min-content" => style.height = Length::MinContent,
                "max-content" => style.height = Length::MaxContent,
                "fit-content" => style.height = Length::FitContent,
                _ => {}
            },
            _ => {}
        },
        "min-width" => match val {
            Value::Length(l) => style.min_width = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.min_width = Length::Auto,
                "content" => style.min_width = Length::Content,
                "min-content" => style.min_width = Length::MinContent,
                "max-content" => style.min_width = Length::MaxContent,
                "fit-content" => style.min_width = Length::FitContent,
                _ => {}
            },
            _ => {}
        },
        "max-width" => match val {
            Value::Length(l) => style.max_width = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.max_width = Length::Auto,
                "content" => style.max_width = Length::Content,
                "min-content" => style.max_width = Length::MinContent,
                "max-content" => style.max_width = Length::MaxContent,
                "fit-content" => style.max_width = Length::FitContent,
                _ => {}
            },
            _ => {}
        },
        "min-height" => match val {
            Value::Length(l) => style.min_height = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.min_height = Length::Auto,
                "content" => style.min_height = Length::Content,
                "min-content" => style.min_height = Length::MinContent,
                "max-content" => style.min_height = Length::MaxContent,
                "fit-content" => style.min_height = Length::FitContent,
                _ => {}
            },
            _ => {}
        },
        "max-height" => match val {
            Value::Length(l) => style.max_height = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.max_height = Length::Auto,
                "content" => style.max_height = Length::Content,
                "min-content" => style.max_height = Length::MinContent,
                "max-content" => style.max_height = Length::MaxContent,
                "fit-content" => style.max_height = Length::FitContent,
                _ => {}
            },
            _ => {}
        },
        "aspect-ratio" => match val {
            Value::Number(n) if *n > 0.0 => style.aspect_ratio = Some(*n),
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => style.aspect_ratio = None,
            _ => {}
        },

        // Margins
        "margin-top" => {
            if let Value::Length(l) = val {
                style.margin_top = *l;
            }
        }
        "margin-right" => {
            if let Value::Length(l) = val {
                style.margin_right = *l;
            }
        }
        "margin-bottom" => {
            if let Value::Length(l) = val {
                style.margin_bottom = *l;
            }
        }
        "margin-left" => {
            if let Value::Length(l) = val {
                style.margin_left = *l;
            }
        }

        // Padding
        "padding-top" => {
            if let Value::Length(l) = val {
                style.padding_top = *l;
            }
        }
        "padding-right" => {
            if let Value::Length(l) = val {
                style.padding_right = *l;
            }
        }
        "padding-bottom" => {
            if let Value::Length(l) = val {
                style.padding_bottom = *l;
            }
        }
        "padding-left" => {
            if let Value::Length(l) = val {
                style.padding_left = *l;
            }
        }

        // Border widths
        "border-top-width" => {
            if let Value::Length(l) = val {
                style.border_top_width = l.to_px(style.font_size, 16.0, 100.0);
            } else if let Value::Number(n) = val {
                style.border_top_width = (*n).max(0.0);
            }
        }
        "border-right-width" => {
            if let Value::Length(l) = val {
                style.border_right_width = l.to_px(style.font_size, 16.0, 100.0);
            } else if let Value::Number(n) = val {
                style.border_right_width = (*n).max(0.0);
            }
        }
        "border-bottom-width" => {
            if let Value::Length(l) = val {
                style.border_bottom_width = l.to_px(style.font_size, 16.0, 100.0);
            } else if let Value::Number(n) = val {
                style.border_bottom_width = (*n).max(0.0);
            }
        }
        "border-left-width" => {
            if let Value::Length(l) = val {
                style.border_left_width = l.to_px(style.font_size, 16.0, 100.0);
            } else if let Value::Number(n) = val {
                style.border_left_width = (*n).max(0.0);
            }
        }

        // Border styles
        "border-top-style" => {
            if let Some(s) = parse_border_style_value(val) {
                style.border_top_style = s;
            }
        }
        "border-right-style" => {
            if let Some(s) = parse_border_style_value(val) {
                style.border_right_style = s;
            }
        }
        "border-bottom-style" => {
            if let Some(s) = parse_border_style_value(val) {
                style.border_bottom_style = s;
            }
        }
        "border-left-style" => {
            if let Some(s) = parse_border_style_value(val) {
                style.border_left_style = s;
            }
        }

        // Border radius
        "border-top-left-radius" => match val {
            Value::Length(len) => {
                style.border_top_left_radius_len = *len;
                style.border_top_left_radius = len
                    .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                    .max(0.0);
            }
            Value::Percentage(pct) => {
                style.border_top_left_radius_len = Length::Percent((*pct).max(0.0));
                style.border_top_left_radius = 0.0;
            }
            Value::Number(n) => {
                style.border_top_left_radius_len = Length::Px((*n).max(0.0));
                style.border_top_left_radius = (*n).max(0.0);
            }
            _ => {}
        },
        "border-top-right-radius" => match val {
            Value::Length(len) => {
                style.border_top_right_radius_len = *len;
                style.border_top_right_radius = len
                    .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                    .max(0.0);
            }
            Value::Percentage(pct) => {
                style.border_top_right_radius_len = Length::Percent((*pct).max(0.0));
                style.border_top_right_radius = 0.0;
            }
            Value::Number(n) => {
                style.border_top_right_radius_len = Length::Px((*n).max(0.0));
                style.border_top_right_radius = (*n).max(0.0);
            }
            _ => {}
        },
        "border-bottom-right-radius" => match val {
            Value::Length(len) => {
                style.border_bottom_right_radius_len = *len;
                style.border_bottom_right_radius = len
                    .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                    .max(0.0);
            }
            Value::Percentage(pct) => {
                style.border_bottom_right_radius_len = Length::Percent((*pct).max(0.0));
                style.border_bottom_right_radius = 0.0;
            }
            Value::Number(n) => {
                style.border_bottom_right_radius_len = Length::Px((*n).max(0.0));
                style.border_bottom_right_radius = (*n).max(0.0);
            }
            _ => {}
        },
        "border-bottom-left-radius" => match val {
            Value::Length(len) => {
                style.border_bottom_left_radius_len = *len;
                style.border_bottom_left_radius = len
                    .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                    .max(0.0);
            }
            Value::Percentage(pct) => {
                style.border_bottom_left_radius_len = Length::Percent((*pct).max(0.0));
                style.border_bottom_left_radius = 0.0;
            }
            Value::Number(n) => {
                style.border_bottom_left_radius_len = Length::Px((*n).max(0.0));
                style.border_bottom_left_radius = (*n).max(0.0);
            }
            _ => {}
        },

        // Positioning offsets
        "top" => {
            if let Value::Length(l) = val {
                style.top = *l;
            }
        }
        "right" => {
            if let Value::Length(l) = val {
                style.right = *l;
            }
        }
        "bottom" => {
            if let Value::Length(l) = val {
                style.bottom = *l;
            }
        }
        "left" => {
            if let Value::Length(l) = val {
                style.left = *l;
            }
        }

        "box-sizing" => {
            if let Value::BoxSizing(bs) = val {
                style.box_sizing = *bs;
            }
        }

        // Visual formatting properties
        "overflow" => match val {
            Value::Overflow(o) => {
                style.overflow_x = *o;
                style.overflow_y = *o;
            }
            Value::Keyword(k) => match k.as_str() {
                "visible" => {
                    style.overflow_x = Overflow::Visible;
                    style.overflow_y = Overflow::Visible;
                }
                "hidden" => {
                    style.overflow_x = Overflow::Hidden;
                    style.overflow_y = Overflow::Hidden;
                }
                "scroll" => {
                    style.overflow_x = Overflow::Scroll;
                    style.overflow_y = Overflow::Scroll;
                }
                "auto" => {
                    style.overflow_x = Overflow::Auto;
                    style.overflow_y = Overflow::Auto;
                }
                _ => {}
            },
            _ => {}
        },
        "overflow-x" => match val {
            Value::Overflow(o) => style.overflow_x = *o,
            Value::Length(Length::Auto) => style.overflow_x = Overflow::Auto,
            Value::Visibility(Visibility::Hidden) => style.overflow_x = Overflow::Hidden,
            Value::Visibility(Visibility::Visible) => style.overflow_x = Overflow::Visible,
            Value::Keyword(k) => match k.as_str() {
                "visible" => style.overflow_x = Overflow::Visible,
                "hidden" => style.overflow_x = Overflow::Hidden,
                "scroll" => style.overflow_x = Overflow::Scroll,
                "auto" => style.overflow_x = Overflow::Auto,
                _ => {}
            },
            _ => {}
        },
        "overflow-y" => match val {
            Value::Overflow(o) => style.overflow_y = *o,
            Value::Length(Length::Auto) => style.overflow_y = Overflow::Auto,
            Value::Visibility(Visibility::Hidden) => style.overflow_y = Overflow::Hidden,
            Value::Visibility(Visibility::Visible) => style.overflow_y = Overflow::Visible,
            Value::Keyword(k) => match k.as_str() {
                "visible" => style.overflow_y = Overflow::Visible,
                "hidden" => style.overflow_y = Overflow::Hidden,
                "scroll" => style.overflow_y = Overflow::Scroll,
                "auto" => style.overflow_y = Overflow::Auto,
                _ => {}
            },
            _ => {}
        },
        "visibility" => match val {
            Value::Visibility(v) => style.visibility = *v,
            Value::Overflow(Overflow::Hidden) => style.visibility = Visibility::Hidden,
            Value::Overflow(Overflow::Visible) => style.visibility = Visibility::Visible,
            Value::Keyword(k) => match k.as_str() {
                "visible" => style.visibility = Visibility::Visible,
                "hidden" => style.visibility = Visibility::Hidden,
                "collapse" => style.visibility = Visibility::Collapse,
                _ => {}
            },
            _ => {}
        },
        "opacity" => match val {
            Value::Number(n) => style.opacity = (*n).clamp(0.0, 1.0),
            Value::Percentage(pct) => style.opacity = (*pct / 100.0).clamp(0.0, 1.0),
            Value::Length(Length::Px(px)) => style.opacity = (*px).clamp(0.0, 1.0),
            Value::Length(Length::Percent(pct)) => style.opacity = (*pct / 100.0).clamp(0.0, 1.0),
            _ => {}
        },

        "outline-width" => {
            let px = match val {
                Value::Length(len) => len.to_px(style.font_size, 16.0, 0.0),
                Value::Number(n) => *n,
                Value::Keyword(k) => match k.as_str() {
                    "thin" => 1.0,
                    "medium" => 3.0,
                    "thick" => 5.0,
                    _ => 0.0,
                },
                _ => 0.0,
            };
            style.outline_width = px.max(0.0);
        }
        "outline-style" => {
            if let Some(bs) = parse_border_style_value(val) {
                style.outline_style = bs;
            }
        }
        "outline-offset" => {
            if let Value::Length(len) = val {
                style.outline_offset = len.to_px(style.font_size, 16.0, 0.0);
            } else if let Value::Number(n) = val {
                style.outline_offset = *n;
            }
        }
        _ => return false,
    }
    true
}
