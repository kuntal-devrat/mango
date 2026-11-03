//! Flexbox, Grid, Table, Multi-column, containment, and miscellaneous layout properties.

use super::box_model::parse_border_style_value;
use crate::computed::custom_props::is_current_color;
use crate::computed::style::ComputedStyle;
use crate::values::{
    Appearance, BorderCollapse, CaptionSide, Cursor, Length, ListStylePosition, ListStyleType,
    Overflow, Position, TableLayout, Value, VerticalAlign, Visibility,
};

pub(crate) fn parse_intrinsic_length(val: &Value) -> Option<Length> {
    match val {
        Value::Length(l) => Some(*l),
        Value::Keyword(k) | Value::String(k) => {
            let s = k.trim();
            if s.eq_ignore_ascii_case("none") {
                None
            } else {
                let stripped = s.strip_prefix("auto").unwrap_or(s).trim();
                if stripped.is_empty() || stripped.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Length::parse(stripped)
                }
            }
        }
        Value::List(items) => {
            for item in items.iter().rev() {
                if let Some(l) = parse_intrinsic_length(item) {
                    return Some(l);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) use super::grid_flex::apply_grid_flex_property;

/// Applies flexbox, grid, table, multi-column, and containment properties.
pub(crate) fn apply_layout_property(style: &mut ComputedStyle, prop: &str, val: &Value) -> bool {
    if apply_grid_flex_property(style, prop, val) {
        return true;
    }

    match prop {
        "appearance" | "-webkit-appearance" | "-moz-appearance" | "-ms-appearance"
        | "-o-appearance" => match val {
            Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                "none" => style.appearance = Appearance::None,
                "auto" => style.appearance = Appearance::Auto,
                _ => {}
            },
            _ => {}
        },

        "z-index" => match val {
            Value::Number(n) => style.z_index = Some(*n as i32),
            Value::Length(Length::Auto) => style.z_index = None,
            Value::Length(len) => {
                let px = len.to_px(style.font_size, style.root_font_size, 0.0);
                style.z_index = Some(px.round() as i32);
            }
            Value::Keyword(k) if k == "auto" => style.z_index = None,
            _ => {}
        },

        "list-style-type" => match val {
            Value::ListStyleType(lst) => style.list_style_type = *lst,
            Value::Keyword(k) => match k.as_str() {
                "disc" => style.list_style_type = ListStyleType::Disc,
                "circle" => style.list_style_type = ListStyleType::Circle,
                "square" => style.list_style_type = ListStyleType::Square,
                "decimal" => style.list_style_type = ListStyleType::Decimal,
                "lower-alpha" => style.list_style_type = ListStyleType::LowerAlpha,
                "upper-alpha" => style.list_style_type = ListStyleType::UpperAlpha,
                "lower-roman" => style.list_style_type = ListStyleType::LowerRoman,
                "upper-roman" => style.list_style_type = ListStyleType::UpperRoman,
                "none" => style.list_style_type = ListStyleType::None,
                _ => {}
            },
            _ => {}
        },
        "list-style-position" => match val {
            Value::ListStylePosition(lsp) => style.list_style_position = *lsp,
            Value::Keyword(k) => match k.as_str() {
                "outside" => style.list_style_position = ListStylePosition::Outside,
                "inside" => style.list_style_position = ListStylePosition::Inside,
                _ => {}
            },
            _ => {}
        },
        "cursor" => match val {
            Value::Cursor(c) => style.cursor = *c,
            Value::Keyword(k) => {
                if let Some(c) = Cursor::from_keyword(k.as_str()) {
                    style.cursor = c;
                }
            }
            _ => {}
        },
        "vertical-align" => match val {
            Value::VerticalAlign(va) => style.vertical_align = *va,
            Value::Keyword(k) => match k.as_str() {
                "baseline" => style.vertical_align = VerticalAlign::Baseline,
                "top" => style.vertical_align = VerticalAlign::Top,
                "middle" => style.vertical_align = VerticalAlign::Middle,
                "bottom" => style.vertical_align = VerticalAlign::Bottom,
                "text-top" => style.vertical_align = VerticalAlign::TextTop,
                "text-bottom" => style.vertical_align = VerticalAlign::TextBottom,
                "sub" => style.vertical_align = VerticalAlign::Sub,
                "super" => style.vertical_align = VerticalAlign::Super,
                _ => {}
            },
            _ => {}
        },
        "border-collapse" => match val {
            Value::BorderCollapse(bc) => style.border_collapse = *bc,
            Value::Keyword(k) => match k.as_str() {
                "collapse" => style.border_collapse = BorderCollapse::Collapse,
                "separate" => style.border_collapse = BorderCollapse::Separate,
                _ => {}
            },
            _ => {}
        },
        "border-spacing" => match val {
            Value::Length(Length::Px(px)) if *px >= 0.0 => style.border_spacing = *px,
            Value::Number(n) if *n >= 0.0 => style.border_spacing = *n,
            _ => {}
        },
        "table-layout" => match val {
            Value::TableLayout(tl) => style.table_layout = *tl,
            Value::Position(Position::Fixed) => style.table_layout = TableLayout::Fixed,
            Value::Keyword(k) => match k.as_str() {
                "auto" => style.table_layout = TableLayout::Auto,
                "fixed" => style.table_layout = TableLayout::Fixed,
                _ => {}
            },
            _ => {}
        },
        "caption-side" => match val {
            Value::CaptionSide(cs) => style.caption_side = *cs,
            Value::VerticalAlign(VerticalAlign::Top) => style.caption_side = CaptionSide::Top,
            Value::VerticalAlign(VerticalAlign::Bottom) => style.caption_side = CaptionSide::Bottom,
            Value::Keyword(k) => match k.as_str() {
                "top" => style.caption_side = CaptionSide::Top,
                "bottom" => style.caption_side = CaptionSide::Bottom,
                _ => {}
            },
            _ => {}
        },
        "pointer-events" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(pe) = crate::values::PointerEvents::parse(k) {
                    style.pointer_events = pe;
                }
            }
            _ => {}
        },
        "user-select" | "-webkit-user-select" | "-moz-user-select" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(us) = crate::values::UserSelect::parse(k) {
                    style.user_select = us;
                }
            }
            _ => {}
        },
        "scroll-behavior" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(sb) = crate::values::ScrollBehavior::parse(k) {
                    style.scroll_behavior = sb;
                }
            }
            _ => {}
        },
        "scroll-snap-type" => match val {
            Value::Keyword(k) | Value::String(k) => {
                style.scroll_snap_type = Some(k.clone());
            }
            Value::List(items) => {
                let mut joined = String::new();
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        joined.push(' ');
                    }
                    match it {
                        Value::Keyword(k) | Value::String(k) => joined.push_str(k),
                        _ => {}
                    }
                }
                if !joined.is_empty() {
                    style.scroll_snap_type = Some(joined);
                }
            }
            _ => {}
        },

        "column-count" => match val {
            Value::Number(n) => {
                if *n >= 1.0 {
                    style.column_count = Some(*n as usize);
                } else {
                    style.column_count = None;
                }
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                style.column_count = None;
            }
            _ => {}
        },
        "column-width" => match val {
            Value::Length(len) => {
                style.column_width = Some(*len);
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                style.column_width = None;
            }
            _ => {}
        },
        "column-rule-width" => match val {
            Value::Length(len) => style.column_rule_width = *len,
            Value::Number(n) => style.column_rule_width = Length::Px(*n),
            Value::Keyword(k) => match k.as_str() {
                "thin" => style.column_rule_width = Length::Px(1.0),
                "medium" => style.column_rule_width = Length::Px(3.0),
                "thick" => style.column_rule_width = Length::Px(5.0),
                _ => {}
            },
            _ => {}
        },
        "column-rule-style" => {
            if let Some(bs) = parse_border_style_value(val) {
                style.column_rule_style = bs;
            }
        }
        "column-rule-color" => {
            if is_current_color(val) {
                style.column_rule_color = style.color;
            } else if let Value::Color(c) = val {
                style.column_rule_color = *c;
            } else if let Value::Keyword(k) = val
                && let Some(c) = Value::parse_color(k)
            {
                style.column_rule_color = c;
            }
        }
        "column-span" => match val {
            Value::ColumnSpan(cs) => style.column_span = *cs,
            Value::Keyword(k) if k.eq_ignore_ascii_case("all") => {
                style.column_span = crate::values::ColumnSpan::All;
            }
            _ => style.column_span = crate::values::ColumnSpan::None,
        },
        "break-inside" | "page-break-inside" => match val {
            Value::BreakInside(bi) => style.break_inside = *bi,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "avoid" => style.break_inside = crate::values::BreakInside::Avoid,
                "avoid-column" => style.break_inside = crate::values::BreakInside::AvoidColumn,
                "avoid-page" => style.break_inside = crate::values::BreakInside::AvoidPage,
                _ => style.break_inside = crate::values::BreakInside::Auto,
            },
            _ => {}
        },
        "contain" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(c) = crate::values::Contain::parse(k) {
                    style.contain = c;
                }
            }
            Value::List(items) => {
                let combined = items
                    .iter()
                    .filter_map(|item| match item {
                        Value::Keyword(s) | Value::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(c) = crate::values::Contain::parse(&combined) {
                    style.contain = c;
                }
            }
            _ => {}
        },
        "contain-intrinsic-width" => {
            style.contain_intrinsic_width = parse_intrinsic_length(val);
        }
        "contain-intrinsic-height" => {
            style.contain_intrinsic_height = parse_intrinsic_length(val);
        }
        "contain-intrinsic-size" => match val {
            Value::List(items) if items.len() >= 2 => {
                style.contain_intrinsic_width = parse_intrinsic_length(&items[0]);
                style.contain_intrinsic_height = parse_intrinsic_length(&items[1]);
            }
            _ => {
                let len = parse_intrinsic_length(val);
                style.contain_intrinsic_width = len;
                style.contain_intrinsic_height = len;
            }
        },
        "content-visibility" => match val {
            Value::Overflow(Overflow::Hidden) => {
                style.content_visibility = crate::values::ContentVisibility::Hidden;
            }
            Value::Overflow(Overflow::Auto) => {
                style.content_visibility = crate::values::ContentVisibility::Auto;
            }
            Value::Overflow(Overflow::Visible) => {
                style.content_visibility = crate::values::ContentVisibility::Visible;
            }
            Value::Visibility(Visibility::Hidden) => {
                style.content_visibility = crate::values::ContentVisibility::Hidden;
            }
            Value::Visibility(Visibility::Visible) => {
                style.content_visibility = crate::values::ContentVisibility::Visible;
            }
            Value::Keyword(k) | Value::String(k) => {
                style.content_visibility = match k.to_ascii_lowercase().as_str() {
                    "auto" => crate::values::ContentVisibility::Auto,
                    "hidden" => crate::values::ContentVisibility::Hidden,
                    _ => crate::values::ContentVisibility::Visible,
                };
            }
            _ => {}
        },
        "writing-mode" => {
            if let Value::Keyword(k) = val {
                style.writing_mode = match k.to_ascii_lowercase().as_str() {
                    "vertical-rl" => crate::values::WritingMode::VerticalRl,
                    "vertical-lr" => crate::values::WritingMode::VerticalLr,
                    "sideways-rl" => crate::values::WritingMode::SidewaysRl,
                    "sideways-lr" => crate::values::WritingMode::SidewaysLr,
                    _ => crate::values::WritingMode::HorizontalTb,
                };
            }
        }
        "resize" => {
            if let Value::Keyword(k) = val {
                style.resize = match k.to_ascii_lowercase().as_str() {
                    "both" => crate::values::Resize::Both,
                    "horizontal" => crate::values::Resize::Horizontal,
                    "vertical" => crate::values::Resize::Vertical,
                    _ => crate::values::Resize::None,
                };
            }
        }
        "scrollbar-width" => {
            if let Value::Keyword(k) = val {
                style.scrollbar_width = match k.to_ascii_lowercase().as_str() {
                    "thin" => crate::values::ScrollbarWidth::Thin,
                    "none" => crate::values::ScrollbarWidth::None,
                    _ => crate::values::ScrollbarWidth::Auto,
                };
            }
        }
        "scrollbar-color" => match val {
            Value::List(list) if list.len() >= 2 => {
                let c1 = match &list[0] {
                    Value::Color(c) => Some(*c),
                    Value::CurrentColor => Some(style.color),
                    Value::Keyword(k) if k.eq_ignore_ascii_case("currentcolor") => {
                        Some(style.color)
                    }
                    Value::Keyword(k) => Value::parse_color(k),
                    _ => None,
                };
                let c2 = match &list[1] {
                    Value::Color(c) => Some(*c),
                    Value::CurrentColor => Some(style.color),
                    Value::Keyword(k) if k.eq_ignore_ascii_case("currentcolor") => {
                        Some(style.color)
                    }
                    Value::Keyword(k) => Value::parse_color(k),
                    _ => None,
                };
                if let (Some(thumb), Some(track)) = (c1, c2) {
                    style.scrollbar_color = Some((thumb, track));
                }
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                style.scrollbar_color = None;
            }
            _ => {}
        },
        _ => return false,
    }
    true
}
