//! Shorthand expansion for flexbox and grid layouts.

use crate::properties::Declaration;
use crate::values::{Length, Value};

pub(crate) fn expand_grid_placement_shorthand(
    prefix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let start_name = format!("{prefix}-start");
    let end_name = format!("{prefix}-end");

    match value {
        Value::List(items) if items.len() >= 2 => {
            vec![
                Declaration::new(start_name, items[0].clone(), important),
                Declaration::new(end_name, items[1].clone(), important),
            ]
        }
        single => {
            vec![Declaration::new(start_name, single.clone(), important)]
        }
    }
}

/// Expands `grid-area` into `grid-row-start`, `grid-column-start`, `grid-row-end`, `grid-column-end`.
pub(crate) fn expand_grid_area_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    match value {
        Value::List(items) => match items.len() {
            1 => {
                let v = items[0].clone();
                vec![
                    Declaration::new("grid-row-start", v.clone(), important),
                    Declaration::new("grid-column-start", v.clone(), important),
                    Declaration::new("grid-row-end", v.clone(), important),
                    Declaration::new("grid-column-end", v, important),
                ]
            }
            2 => vec![
                Declaration::new("grid-row-start", items[0].clone(), important),
                Declaration::new("grid-column-start", items[1].clone(), important),
                Declaration::new(
                    "grid-row-end",
                    Value::GridPlacement(crate::values::GridPlacement::Auto),
                    important,
                ),
                Declaration::new(
                    "grid-column-end",
                    Value::GridPlacement(crate::values::GridPlacement::Auto),
                    important,
                ),
            ],
            3 => vec![
                Declaration::new("grid-row-start", items[0].clone(), important),
                Declaration::new("grid-column-start", items[1].clone(), important),
                Declaration::new("grid-row-end", items[2].clone(), important),
                Declaration::new(
                    "grid-column-end",
                    Value::GridPlacement(crate::values::GridPlacement::Auto),
                    important,
                ),
            ],
            _ => vec![
                Declaration::new("grid-row-start", items[0].clone(), important),
                Declaration::new("grid-column-start", items[1].clone(), important),
                Declaration::new("grid-row-end", items[2].clone(), important),
                Declaration::new("grid-column-end", items[3].clone(), important),
            ],
        },
        single => vec![
            Declaration::new("grid-row-start", single.clone(), important),
            Declaration::new("grid-column-start", single.clone(), important),
            Declaration::new("grid-row-end", single.clone(), important),
            Declaration::new("grid-column-end", single.clone(), important),
        ],
    }
}

/// Expands `grid-template: <rows> / <columns>` into `grid-template-rows` and `grid-template-columns`.
pub(crate) fn expand_grid_template_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    match value {
        Value::List(items) if items.len() >= 2 => vec![
            Declaration::new("grid-template-rows", items[0].clone(), important),
            Declaration::new("grid-template-columns", items[1].clone(), important),
        ],
        single => vec![Declaration::new(
            "grid-template-rows",
            single.clone(),
            important,
        )],
    }
}

fn is_flex_basis_value(val: &Value) -> bool {
    match val {
        Value::Length(len) => match len {
            Length::Percent(_)
            | Length::Em(_)
            | Length::Ex(_)
            | Length::Rem(_)
            | Length::Vw(_)
            | Length::Vh(_)
            | Length::Vmin(_)
            | Length::Vmax(_)
            | Length::Cqw(_)
            | Length::Cqh(_)
            | Length::Cqi(_)
            | Length::Cqb(_)
            | Length::Cqmin(_)
            | Length::Cqmax(_)
            | Length::Auto
            | Length::Content
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent => true,
            Length::Px(_) => true,
            Length::Calc(calc) => {
                calc.has_percent
                    || calc.px != 0.0
                    || calc.em != 0.0
                    || calc.ex != 0.0
                    || calc.rem != 0.0
                    || calc.vw != 0.0
                    || calc.vh != 0.0
            }
            Length::Math(id) => {
                !crate::values::get_math(*id).map_or(false, |m| m.is_dimensionless_number())
            }
            Length::Anchor { .. } => false,
        },
        Value::Keyword(k) => matches!(
            k.to_ascii_lowercase().as_str(),
            "auto" | "content" | "fit-content" | "min-content" | "max-content"
        ),
        _ => false,
    }
}

/// Expands flex shorthand into flex-grow, flex-shrink, and flex-basis according to CSS Flexbox Level 1 § 7.1.
pub(crate) fn expand_flex_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    if items.is_empty() || items.len() > 3 {
        return Vec::new();
    }

    if items.len() == 1 {
        if let Value::Keyword(ref k) = items[0] {
            if k.eq_ignore_ascii_case("none") {
                return vec![
                    Declaration::new("flex-grow", Value::Number(0.0), important),
                    Declaration::new("flex-shrink", Value::Number(0.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Auto), important),
                ];
            }
        }
    }

    // "none" is invalid if combined with any other token
    for it in items {
        if let Value::Keyword(k) = it {
            if k.eq_ignore_ascii_case("none") {
                return Vec::new();
            }
        }
    }

    let mut grow: Option<Value> = None;
    let mut shrink: Option<Value> = None;
    let mut basis: Option<Value> = None;

    for it in items {
        if is_flex_basis_value(it) {
            if basis.is_some() {
                return Vec::new(); // Duplicate basis
            }
            let b = match it {
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => Value::Length(Length::Auto),
                    "content" => Value::Length(Length::Content),
                    "fit-content" => Value::Length(Length::FitContent),
                    "min-content" => Value::Length(Length::MinContent),
                    "max-content" => Value::Length(Length::MaxContent),
                    _ => it.clone(),
                },
                _ => it.clone(),
            };
            basis = Some(b);
        } else if let Value::Number(n) = it {
            if grow.is_none() {
                grow = Some(Value::Number(*n));
            } else if shrink.is_none() {
                shrink = Some(Value::Number(*n));
            } else if basis.is_none() && *n == 0.0 {
                // Unitless zero preceded by two flex factors is interpreted as flex-basis: 0px
                basis = Some(Value::Length(Length::Px(0.0)));
            } else {
                return Vec::new(); // Too many numbers
            }
        } else if let Value::Length(Length::Calc(_) | Length::Math(_)) = it {
            // Math expression without length units: treated as a flex factor
            if grow.is_none() {
                grow = Some(it.clone());
            } else if shrink.is_none() {
                shrink = Some(it.clone());
            } else {
                return Vec::new(); // Too many numbers
            }
        } else {
            return Vec::new(); // Invalid token in flex shorthand
        }
    }

    let final_grow = grow.unwrap_or(Value::Number(1.0));
    let final_shrink = shrink.unwrap_or(Value::Number(1.0));
    let final_basis = basis.unwrap_or(Value::Length(Length::Percent(0.0)));

    vec![
        Declaration::new("flex-grow", final_grow, important),
        Declaration::new("flex-shrink", final_shrink, important),
        Declaration::new("flex-basis", final_basis, important),
    ]
}

/// Expands `flex-flow: <'flex-direction'> || <'flex-wrap'>`
pub(crate) fn expand_flex_flow_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let words: Vec<String> = items
        .iter()
        .filter_map(|it| match it {
            Value::Keyword(k) => Some(k.to_ascii_lowercase()),
            Value::FlexDirection(fd) => Some(match fd {
                crate::values::FlexDirection::Row => "row".to_string(),
                crate::values::FlexDirection::RowReverse => "row-reverse".to_string(),
                crate::values::FlexDirection::Column => "column".to_string(),
                crate::values::FlexDirection::ColumnReverse => "column-reverse".to_string(),
            }),
            Value::FlexWrap(fw) => Some(match fw {
                crate::values::FlexWrap::NoWrap => "nowrap".to_string(),
                crate::values::FlexWrap::Wrap => "wrap".to_string(),
                crate::values::FlexWrap::WrapReverse => "wrap-reverse".to_string(),
            }),
            _ => None,
        })
        .collect();

    if words.len() != items.len() || words.is_empty() || words.len() > 3 {
        return Vec::new();
    }

    let is_dir = |w: &str| matches!(w, "row" | "row-reverse" | "column" | "column-reverse");
    let mut dir: Option<&str> = None;
    let mut wrap_indices = Vec::new();

    for (i, w) in words.iter().enumerate() {
        if is_dir(w) {
            if dir.is_some() {
                return Vec::new(); // Duplicate direction
            }
            dir = Some(w.as_str());
        } else {
            wrap_indices.push(i);
        }
    }

    let wrap_val: String = match wrap_indices.len() {
        0 => "nowrap".to_string(),
        1 => {
            let w = words[wrap_indices[0]].as_str();
            match w {
                "nowrap" | "wrap" | "wrap-reverse" | "balance" => w.to_string(),
                _ => return Vec::new(),
            }
        }
        2 => {
            // Two-keyword wrap must be contiguous!
            if wrap_indices[1] != wrap_indices[0] + 1 {
                return Vec::new();
            }
            let w1 = words[wrap_indices[0]].as_str();
            let w2 = words[wrap_indices[1]].as_str();
            match (w1, w2) {
                ("wrap", "balance") | ("balance", "wrap") => "balance".to_string(),
                ("wrap-reverse", "balance") | ("balance", "wrap-reverse") => {
                    "wrap-reverse balance".to_string()
                }
                _ => return Vec::new(),
            }
        }
        _ => return Vec::new(),
    };

    let dir_val = dir.unwrap_or("row");
    let dir_value = match dir_val {
        "row" => Value::FlexDirection(crate::values::FlexDirection::Row),
        "row-reverse" => Value::FlexDirection(crate::values::FlexDirection::RowReverse),
        "column" => Value::FlexDirection(crate::values::FlexDirection::Column),
        "column-reverse" => Value::FlexDirection(crate::values::FlexDirection::ColumnReverse),
        other => Value::Keyword(other.to_string()),
    };
    let wrap_value = match wrap_val.as_str() {
        "nowrap" => Value::FlexWrap(crate::values::FlexWrap::NoWrap),
        "wrap" => Value::FlexWrap(crate::values::FlexWrap::Wrap),
        "wrap-reverse" => Value::FlexWrap(crate::values::FlexWrap::WrapReverse),
        other => Value::Keyword(other.to_string()),
    };
    vec![
        Declaration::new("flex-direction", dir_value, important),
        Declaration::new("flex-wrap", wrap_value, important),
    ]
}

/// Expands `gap: <row-gap> <column-gap>`.
pub(crate) fn expand_gap_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let (row, col) = match items.len() {
        1 => (items[0].clone(), items[0].clone()),
        2.. => (items[0].clone(), items[1].clone()),
        _ => return vec![Declaration::new("gap", value.clone(), important)],
    };

    vec![
        Declaration::new("row-gap", row, important),
        Declaration::new("column-gap", col, important),
    ]
}

/// Expands place-items, place-content, or place-self shorthand.
pub(crate) fn expand_place_shorthand(
    align_prop: &str,
    justify_prop: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let (align, justify) = match items.len() {
        1 => (items[0].clone(), items[0].clone()),
        2.. => (items[0].clone(), items[1].clone()),
        _ => return vec![Declaration::new(align_prop, value.clone(), important)],
    };

    vec![
        Declaration::new(align_prop, align, important),
        Declaration::new(justify_prop, justify, important),
    ]
}
