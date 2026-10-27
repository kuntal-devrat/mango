//! Flexbox, Grid, Table, Multi-column, containment, and miscellaneous layout properties.

use super::box_model::parse_border_style_value;
use crate::computed::custom_props::is_current_color;
use crate::computed::style::ComputedStyle;
use crate::values::{
    AlignContent, AlignItems, AlignSelf, Appearance, BorderCollapse, CaptionSide, Cursor,
    FlexDirection, FlexWrap, GridAutoFlow, GridPlacement, GridTrackSize, JustifyContent,
    JustifyItems, JustifySelf, Length, ListStylePosition, ListStyleType, Overflow, Position,
    TableLayout, TextAlign, Value, VerticalAlign, Visibility,
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

pub(crate) fn value_to_track_size(v: &Value) -> Option<GridTrackSize> {
    match v {
        Value::Length(l) => Some(GridTrackSize::Length(*l)),
        Value::Fr(f) => Some(GridTrackSize::Fr(*f)),
        Value::GridTrackList(list) if list.len() == 1 => Some(list[0].clone()),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "auto" => Some(GridTrackSize::Auto),
            "min-content" => Some(GridTrackSize::MinContent),
            "max-content" => Some(GridTrackSize::MaxContent),
            "subgrid" => Some(GridTrackSize::Subgrid),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn value_to_track_list(v: &Value) -> Vec<GridTrackSize> {
    match v {
        Value::GridTrackList(list) => list.clone(),
        Value::GridTrackListWithLines { tracks, .. } => tracks.clone(),
        Value::List(items) => items.iter().filter_map(value_to_track_size).collect(),
        single => value_to_track_size(single).into_iter().collect(),
    }
}

pub(crate) fn value_to_template_areas(v: &Value) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    match v {
        Value::List(items) => {
            for item in items {
                if let Value::String(s) = item {
                    let cells: Vec<String> = s
                        .split_whitespace()
                        .map(|w| w.to_ascii_lowercase())
                        .collect();
                    if !cells.is_empty() {
                        rows.push(cells);
                    }
                }
            }
        }
        Value::String(s) => {
            for line in s.lines() {
                let cells: Vec<String> = line
                    .split_whitespace()
                    .map(|w| w.to_ascii_lowercase())
                    .collect();
                if !cells.is_empty() {
                    rows.push(cells);
                }
            }
        }
        _ => {}
    }
    rows
}

pub(crate) fn value_to_placement(v: &Value) -> GridPlacement {
    match v {
        Value::GridPlacement(gp) => gp.clone(),
        Value::Number(n) => GridPlacement::Line(*n as i32),
        Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => GridPlacement::Auto,
        Value::Keyword(k) => GridPlacement::Area(k.to_ascii_lowercase()),
        Value::String(s) => GridPlacement::Area(s.to_ascii_lowercase()),
        _ => GridPlacement::Auto,
    }
}

/// Applies flexbox, grid, table, multi-column, and containment properties.
pub(crate) fn apply_layout_property(style: &mut ComputedStyle, prop: &str, val: &Value) -> bool {
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

        // Flexbox & positioning
        "flex-direction" => match val {
            Value::FlexDirection(fd) => style.flex_direction = *fd,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "row" => style.flex_direction = FlexDirection::Row,
                "row-reverse" => style.flex_direction = FlexDirection::RowReverse,
                "column" => style.flex_direction = FlexDirection::Column,
                "column-reverse" => style.flex_direction = FlexDirection::ColumnReverse,
                _ => {}
            },
            _ => {}
        },
        "flex-wrap" => match val {
            Value::FlexWrap(fw) => style.flex_wrap = *fw,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "nowrap" => style.flex_wrap = FlexWrap::NoWrap,
                "wrap" => style.flex_wrap = FlexWrap::Wrap,
                "wrap-reverse" => style.flex_wrap = FlexWrap::WrapReverse,
                "balance" => style.flex_wrap = FlexWrap::Balance,
                "wrap-reverse balance" => style.flex_wrap = FlexWrap::WrapReverseBalance,
                _ => {}
            },
            _ => {}
        },
        "flex-line-count" => match val {
            Value::Number(n) => {
                let count = n.round() as i64;
                if count >= 1 {
                    style.flex_line_count = Some(count as u32);
                }
            }
            Value::Length(len) => {
                let px = len.to_px(style.font_size, style.root_font_size, 0.0);
                if px.is_nan() {
                    style.flex_line_count = Some(1);
                } else {
                    let count = px.round() as i64;
                    if count >= 1 {
                        style.flex_line_count = Some(count as u32);
                    }
                }
            }
            _ => {}
        },
        "justify-content" => match val {
            Value::JustifyContent(jc) => style.justify_content = *jc,
            Value::TextAlign(TextAlign::Center) => style.justify_content = JustifyContent::Center,
            Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                "normal" => style.justify_content = JustifyContent::Normal,
                "center" => style.justify_content = JustifyContent::Center,
                "flex-start" => style.justify_content = JustifyContent::FlexStart,
                "flex-end" => style.justify_content = JustifyContent::FlexEnd,
                "start" => style.justify_content = JustifyContent::Start,
                "end" => style.justify_content = JustifyContent::End,
                "left" => style.justify_content = JustifyContent::Left,
                "right" => style.justify_content = JustifyContent::Right,
                "space-between" => style.justify_content = JustifyContent::SpaceBetween,
                "space-around" => style.justify_content = JustifyContent::SpaceAround,
                "space-evenly" => style.justify_content = JustifyContent::SpaceEvenly,
                _ => {}
            },
            _ => {}
        },
        "justify-items" => match val {
            Value::JustifyItems(ji) => style.justify_items = *ji,
            Value::TextAlign(TextAlign::Center) => style.justify_items = JustifyItems::Center,
            Value::JustifyContent(JustifyContent::Normal | JustifyContent::FlexStart) => {
                style.justify_items = JustifyItems::FlexStart
            }
            Value::JustifyContent(JustifyContent::FlexEnd) => {
                style.justify_items = JustifyItems::FlexEnd
            }
            Value::JustifyContent(JustifyContent::Center) => {
                style.justify_items = JustifyItems::Center
            }
            Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                "center" => style.justify_items = JustifyItems::Center,
                "start" | "flex-start" => style.justify_items = JustifyItems::FlexStart,
                "end" | "flex-end" => style.justify_items = JustifyItems::FlexEnd,
                "stretch" => style.justify_items = JustifyItems::Stretch,
                "baseline" => style.justify_items = JustifyItems::Baseline,
                _ => {}
            },
            _ => {}
        },
        "justify-self" => match val {
            Value::JustifySelf(js) => style.justify_self = *js,
            Value::JustifyItems(ji) => {
                style.justify_self = match ji {
                    JustifyItems::Stretch => JustifySelf::Stretch,
                    JustifyItems::FlexStart => JustifySelf::FlexStart,
                    JustifyItems::FlexEnd => JustifySelf::FlexEnd,
                    JustifyItems::Center => JustifySelf::Center,
                    JustifyItems::Baseline => JustifySelf::Baseline,
                };
            }
            Value::TextAlign(TextAlign::Center) => style.justify_self = JustifySelf::Center,
            Value::JustifyContent(JustifyContent::Normal | JustifyContent::FlexStart) => {
                style.justify_self = JustifySelf::FlexStart
            }
            Value::JustifyContent(JustifyContent::FlexEnd) => {
                style.justify_self = JustifySelf::FlexEnd
            }
            Value::JustifyContent(JustifyContent::Center) => {
                style.justify_self = JustifySelf::Center
            }
            Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                "auto" => style.justify_self = JustifySelf::Auto,
                "center" => style.justify_self = JustifySelf::Center,
                "start" | "flex-start" => style.justify_self = JustifySelf::FlexStart,
                "end" | "flex-end" => style.justify_self = JustifySelf::FlexEnd,
                "stretch" => style.justify_self = JustifySelf::Stretch,
                "baseline" => style.justify_self = JustifySelf::Baseline,
                _ => {}
            },
            _ => {}
        },
        "align-items" => match val {
            Value::AlignItems(ai) => style.align_items = *ai,
            Value::TextAlign(TextAlign::Center) => style.align_items = AlignItems::Center,
            Value::JustifyContent(JustifyContent::Normal | JustifyContent::FlexStart) => {
                style.align_items = AlignItems::FlexStart
            }
            Value::JustifyContent(JustifyContent::FlexEnd) => {
                style.align_items = AlignItems::FlexEnd
            }
            Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                "normal" => style.align_items = AlignItems::Normal,
                "center" => style.align_items = AlignItems::Center,
                "flex-start" | "start" => style.align_items = AlignItems::FlexStart,
                "flex-end" | "end" => style.align_items = AlignItems::FlexEnd,
                "stretch" => style.align_items = AlignItems::Stretch,
                "baseline" => style.align_items = AlignItems::Baseline,
                "safe center" => style.align_items = AlignItems::SafeCenter,
                "safe flex-start" => style.align_items = AlignItems::SafeFlexStart,
                "safe flex-end" => style.align_items = AlignItems::SafeFlexEnd,
                "safe start" => style.align_items = AlignItems::SafeStart,
                "safe end" => style.align_items = AlignItems::SafeEnd,
                _ => {}
            },
            Value::List(list) => {
                let s = list
                    .iter()
                    .filter_map(|v| match v {
                        Value::Keyword(k) => Some(k.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_ascii_lowercase();
                match s.as_str() {
                    "safe center" => style.align_items = AlignItems::SafeCenter,
                    "safe flex-start" => style.align_items = AlignItems::SafeFlexStart,
                    "safe flex-end" => style.align_items = AlignItems::SafeFlexEnd,
                    "safe start" => style.align_items = AlignItems::SafeStart,
                    "safe end" => style.align_items = AlignItems::SafeEnd,
                    _ => {}
                }
            }
            _ => {}
        },
        "align-self" => match val {
            Value::AlignSelf(as_val) => style.align_self = *as_val,
            Value::AlignItems(ai) => {
                style.align_self = match ai {
                    AlignItems::Normal | AlignItems::Stretch => AlignSelf::Stretch,
                    AlignItems::FlexStart => AlignSelf::FlexStart,
                    AlignItems::FlexEnd => AlignSelf::FlexEnd,
                    AlignItems::Center => AlignSelf::Center,
                    AlignItems::Baseline => AlignSelf::Baseline,
                    AlignItems::SafeCenter => AlignSelf::SafeCenter,
                    AlignItems::SafeFlexStart => AlignSelf::SafeFlexStart,
                    AlignItems::SafeFlexEnd => AlignSelf::SafeFlexEnd,
                    AlignItems::SafeStart => AlignSelf::SafeStart,
                    AlignItems::SafeEnd => AlignSelf::SafeEnd,
                };
            }
            Value::TextAlign(TextAlign::Center) => style.align_self = AlignSelf::Center,
            Value::JustifyContent(JustifyContent::Normal | JustifyContent::FlexStart) => {
                style.align_self = AlignSelf::FlexStart
            }
            Value::JustifyContent(JustifyContent::FlexEnd) => style.align_self = AlignSelf::FlexEnd,
            Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                "auto" => style.align_self = AlignSelf::Auto,
                "center" => style.align_self = AlignSelf::Center,
                "flex-start" | "start" => style.align_self = AlignSelf::FlexStart,
                "flex-end" | "end" => style.align_self = AlignSelf::FlexEnd,
                "stretch" => style.align_self = AlignSelf::Stretch,
                "baseline" => style.align_self = AlignSelf::Baseline,
                "safe center" => style.align_self = AlignSelf::SafeCenter,
                "safe flex-start" => style.align_self = AlignSelf::SafeFlexStart,
                "safe flex-end" => style.align_self = AlignSelf::SafeFlexEnd,
                "safe start" => style.align_self = AlignSelf::SafeStart,
                "safe end" => style.align_self = AlignSelf::SafeEnd,
                _ => {}
            },
            Value::List(list) => {
                let s = list
                    .iter()
                    .filter_map(|v| match v {
                        Value::Keyword(k) => Some(k.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_ascii_lowercase();
                match s.as_str() {
                    "safe center" => style.align_self = AlignSelf::SafeCenter,
                    "safe flex-start" => style.align_self = AlignSelf::SafeFlexStart,
                    "safe flex-end" => style.align_self = AlignSelf::SafeFlexEnd,
                    "safe start" => style.align_self = AlignSelf::SafeStart,
                    "safe end" => style.align_self = AlignSelf::SafeEnd,
                    _ => {}
                }
            }
            _ => {}
        },
        "align-content" => match val {
            Value::AlignContent(ac) => style.align_content = *ac,
            Value::JustifyContent(jc) => {
                style.align_content = match jc {
                    JustifyContent::Normal => AlignContent::Normal,
                    JustifyContent::FlexStart => AlignContent::FlexStart,
                    JustifyContent::FlexEnd => AlignContent::FlexEnd,
                    JustifyContent::Center => AlignContent::Center,
                    JustifyContent::SpaceBetween => AlignContent::SpaceBetween,
                    JustifyContent::SpaceAround => AlignContent::SpaceAround,
                    JustifyContent::SpaceEvenly => AlignContent::SpaceEvenly,
                    JustifyContent::Start | JustifyContent::Left => AlignContent::FlexStart,
                    JustifyContent::End | JustifyContent::Right => AlignContent::FlexEnd,
                };
            }
            Value::AlignItems(ai) => {
                style.align_content = match ai {
                    AlignItems::Normal => AlignContent::Normal,
                    AlignItems::Stretch => AlignContent::Stretch,
                    AlignItems::FlexStart => AlignContent::FlexStart,
                    AlignItems::FlexEnd => AlignContent::FlexEnd,
                    AlignItems::Center => AlignContent::Center,
                    _ => AlignContent::Stretch,
                };
            }
            Value::TextAlign(TextAlign::Center) => style.align_content = AlignContent::Center,
            Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                "normal" => style.align_content = AlignContent::Normal,
                "stretch" => style.align_content = AlignContent::Stretch,
                "flex-start" | "start" => style.align_content = AlignContent::FlexStart,
                "flex-end" | "end" => style.align_content = AlignContent::FlexEnd,
                "center" => style.align_content = AlignContent::Center,
                "space-between" => style.align_content = AlignContent::SpaceBetween,
                "space-around" => style.align_content = AlignContent::SpaceAround,
                "space-evenly" => style.align_content = AlignContent::SpaceEvenly,
                _ => {}
            },
            _ => {}
        },
        "flex-grow" => {
            if let Value::Number(n) = val {
                style.flex_grow = (*n).max(0.0);
            } else if let Value::Length(l) = val {
                let num = l.to_px(style.font_size, style.root_font_size, 0.0);
                style.flex_grow = num.max(0.0);
            }
        }
        "flex-shrink" => {
            if let Value::Number(n) = val {
                style.flex_shrink = (*n).max(0.0);
            } else if let Value::Length(l) = val {
                let num = l.to_px(style.font_size, style.root_font_size, 0.0);
                style.flex_shrink = num.max(0.0);
            }
        }
        "flex-basis" => match val {
            Value::Length(l) => style.flex_basis = *l,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "content" => style.flex_basis = Length::Content,
                "min-content" => style.flex_basis = Length::MinContent,
                "max-content" => style.flex_basis = Length::MaxContent,
                "fit-content" => style.flex_basis = Length::FitContent,
                "auto" => style.flex_basis = Length::Auto,
                _ => {}
            },
            _ => {}
        },
        "row-gap" => {
            if let Value::Length(l) = val {
                style.row_gap = *l;
            } else if let Value::Number(n) = val {
                style.row_gap = Length::Px(*n);
            } else if let Value::Keyword(k) = val
                && k.eq_ignore_ascii_case("normal")
            {
                style.row_gap = Length::Px(0.0);
            }
        }
        "column-gap" => {
            if let Value::Length(l) = val {
                style.column_gap = *l;
            } else if let Value::Number(n) = val {
                style.column_gap = Length::Px(*n);
            } else if let Value::Keyword(k) = val
                && k.eq_ignore_ascii_case("normal")
            {
                style.column_gap = Length::Px(0.0);
            }
        }
        "order" => {
            if let Value::Number(n) = val {
                style.order = *n as i32;
            } else if let Value::Length(Length::Px(px)) = val {
                style.order = *px as i32;
            }
        }
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
            Value::Keyword(k) => match k.as_str() {
                "default" => style.cursor = Cursor::Default,
                "pointer" => style.cursor = Cursor::Pointer,
                "text" => style.cursor = Cursor::Text,
                "not-allowed" => style.cursor = Cursor::NotAllowed,
                "move" => style.cursor = Cursor::Move,
                "wait" => style.cursor = Cursor::Wait,
                "help" => style.cursor = Cursor::Help,
                _ => {}
            },
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

        // CSS Grid properties
        "grid-template-columns" => {
            if let Value::Keyword(k) = val
                && k.eq_ignore_ascii_case("none")
            {
                style.grid_template_columns.clear();
                style.grid_column_lines.clear();
            } else if let Value::GridTrackListWithLines { tracks, lines } = val {
                style.grid_template_columns = tracks.clone();
                style.grid_column_lines = lines.clone();
            } else {
                style.grid_template_columns = value_to_track_list(val);
            }
        }
        "grid-template-rows" => {
            if let Value::Keyword(k) = val
                && k.eq_ignore_ascii_case("none")
            {
                style.grid_template_rows.clear();
                style.grid_row_lines.clear();
            } else if let Value::GridTrackListWithLines { tracks, lines } = val {
                style.grid_template_rows = tracks.clone();
                style.grid_row_lines = lines.clone();
            } else {
                style.grid_template_rows = value_to_track_list(val);
            }
        }
        "grid-auto-flow" => match val {
            Value::GridAutoFlow(flow) => style.grid_auto_flow = *flow,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "column" => style.grid_auto_flow = GridAutoFlow::Column,
                "column dense" | "dense column" => style.grid_auto_flow = GridAutoFlow::ColumnDense,
                "row dense" | "dense row" | "dense" => {
                    style.grid_auto_flow = GridAutoFlow::RowDense
                }
                _ => style.grid_auto_flow = GridAutoFlow::Row,
            },
            _ => {}
        },
        "grid-template-areas" => {
            if let Value::Keyword(k) = val
                && k.eq_ignore_ascii_case("none")
            {
                style.grid_template_areas.clear();
            } else {
                style.grid_template_areas = value_to_template_areas(val);
            }
        }
        "grid-column-start" => {
            style.grid_column_start = value_to_placement(val);
        }
        "grid-column-end" => {
            style.grid_column_end = value_to_placement(val);
        }
        "grid-row-start" => {
            style.grid_row_start = value_to_placement(val);
        }
        "grid-row-end" => {
            style.grid_row_end = value_to_placement(val);
        }
        _ => return false,
    }
    true
}
