//! Flexbox and Grid property compute and application.

use crate::computed::style::ComputedStyle;
use crate::values::{
    AlignContent, AlignItems, AlignSelf, FlexDirection, FlexWrap, GridAutoFlow, GridPlacement,
    GridTrackSize, JustifyContent, JustifyItems, JustifySelf, Length, TextAlign, Value,
};

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

/// Applies flexbox and CSS grid properties to a ComputedStyle.
pub(crate) fn apply_grid_flex_property(style: &mut ComputedStyle, prop: &str, val: &Value) -> bool {
    match prop {
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
                "baseline" | "first baseline" => style.align_items = AlignItems::Baseline,
                "last baseline" => style.align_items = AlignItems::LastBaseline,
                "safe center" => style.align_items = AlignItems::SafeCenter,
                "safe flex-start" => style.align_items = AlignItems::SafeFlexStart,
                "safe flex-end" => style.align_items = AlignItems::SafeFlexEnd,
                "safe start" => style.align_items = AlignItems::SafeStart,
                "safe end" => style.align_items = AlignItems::SafeEnd,
                _ => {}
            },
            Value::List(list) => {
                let parts: Vec<String> = list
                    .iter()
                    .filter_map(|v| match v {
                        Value::Keyword(k) => Some(k.to_ascii_lowercase()),
                        Value::AlignItems(AlignItems::Baseline) => Some("baseline".to_string()),
                        Value::AlignItems(AlignItems::LastBaseline) => {
                            Some("last baseline".to_string())
                        }
                        _ => None,
                    })
                    .collect();
                let s = parts.join(" ");
                match s.as_str() {
                    "safe center" => style.align_items = AlignItems::SafeCenter,
                    "safe flex-start" => style.align_items = AlignItems::SafeFlexStart,
                    "safe flex-end" => style.align_items = AlignItems::SafeFlexEnd,
                    "safe start" => style.align_items = AlignItems::SafeStart,
                    "safe end" => style.align_items = AlignItems::SafeEnd,
                    "baseline" | "first baseline" => style.align_items = AlignItems::Baseline,
                    "last baseline" => style.align_items = AlignItems::LastBaseline,
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
                    AlignItems::LastBaseline => AlignSelf::LastBaseline,
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
                "baseline" | "first baseline" => style.align_self = AlignSelf::Baseline,
                "last baseline" => style.align_self = AlignSelf::LastBaseline,
                "safe center" => style.align_self = AlignSelf::SafeCenter,
                "safe flex-start" => style.align_self = AlignSelf::SafeFlexStart,
                "safe flex-end" => style.align_self = AlignSelf::SafeFlexEnd,
                "safe start" => style.align_self = AlignSelf::SafeStart,
                "safe end" => style.align_self = AlignSelf::SafeEnd,
                _ => {}
            },
            Value::List(list) => {
                let parts: Vec<String> = list
                    .iter()
                    .filter_map(|v| match v {
                        Value::Keyword(k) => Some(k.to_ascii_lowercase()),
                        Value::AlignItems(AlignItems::Baseline) => Some("baseline".to_string()),
                        Value::AlignItems(AlignItems::LastBaseline) => {
                            Some("last baseline".to_string())
                        }
                        Value::AlignSelf(AlignSelf::Baseline) => Some("baseline".to_string()),
                        Value::AlignSelf(AlignSelf::LastBaseline) => {
                            Some("last baseline".to_string())
                        }
                        _ => None,
                    })
                    .collect();
                let s = parts.join(" ");
                match s.as_str() {
                    "safe center" => style.align_self = AlignSelf::SafeCenter,
                    "safe flex-start" => style.align_self = AlignSelf::SafeFlexStart,
                    "safe flex-end" => style.align_self = AlignSelf::SafeFlexEnd,
                    "safe start" => style.align_self = AlignSelf::SafeStart,
                    "safe end" => style.align_self = AlignSelf::SafeEnd,
                    "baseline" | "first baseline" => style.align_self = AlignSelf::Baseline,
                    "last baseline" => style.align_self = AlignSelf::LastBaseline,
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
