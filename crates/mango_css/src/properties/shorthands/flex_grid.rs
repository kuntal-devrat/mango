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

/// Expands flex shorthand into flex-grow, flex-shrink, and flex-basis.
pub(crate) fn expand_flex_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    if items.len() == 1 {
        match items[0] {
            Value::Keyword(ref k) if k == "none" => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(0.0), important),
                    Declaration::new("flex-shrink", Value::Number(0.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Auto), important),
                ];
            }
            Value::Keyword(ref k) if k == "auto" => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(1.0), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Auto), important),
                ];
            }
            Value::Number(n) => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(n), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Px(0.0)), important),
                ];
            }
            Value::Length(Length::Px(0.0)) => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(0.0), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Px(0.0)), important),
                ];
            }
            Value::Length(len) => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(1.0), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(len), important),
                ];
            }
            _ => {}
        }
    } else if items.len() == 2 {
        let grow = match items[0] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        let shrink = match items[1] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        return vec![
            Declaration::new("flex-grow", Value::Number(grow), important),
            Declaration::new("flex-shrink", Value::Number(shrink), important),
            Declaration::new("flex-basis", Value::Length(Length::Px(0.0)), important),
        ];
    } else if items.len() >= 3 {
        let grow = match items[0] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        let shrink = match items[1] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        let basis = match items[2] {
            Value::Length(len) => len,
            _ => Length::Auto,
        };
        return vec![
            Declaration::new("flex-grow", Value::Number(grow), important),
            Declaration::new("flex-shrink", Value::Number(shrink), important),
            Declaration::new("flex-basis", Value::Length(basis), important),
        ];
    }

    vec![Declaration::new("flex", value.clone(), important)]
}

/// Expands `flex-flow: <'flex-direction'> || <'flex-wrap'>`
pub(crate) fn expand_flex_flow_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut dir = None;
    let mut wrap = None;

    for item in items {
        match item {
            Value::FlexDirection(fd) => dir = Some(Value::FlexDirection(*fd)),
            Value::FlexWrap(fw) => wrap = Some(Value::FlexWrap(*fw)),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "row" => dir = Some(Value::FlexDirection(crate::values::FlexDirection::Row)),
                "row-reverse" => {
                    dir = Some(Value::FlexDirection(
                        crate::values::FlexDirection::RowReverse,
                    ))
                }
                "column" => dir = Some(Value::FlexDirection(crate::values::FlexDirection::Column)),
                "column-reverse" => {
                    dir = Some(Value::FlexDirection(
                        crate::values::FlexDirection::ColumnReverse,
                    ))
                }
                "nowrap" => wrap = Some(Value::FlexWrap(crate::values::FlexWrap::NoWrap)),
                "wrap" => wrap = Some(Value::FlexWrap(crate::values::FlexWrap::Wrap)),
                "wrap-reverse" => {
                    wrap = Some(Value::FlexWrap(crate::values::FlexWrap::WrapReverse))
                }
                _ => {}
            },
            _ => {}
        }
    }

    let dir_val = dir.unwrap_or(Value::FlexDirection(crate::values::FlexDirection::Row));
    let wrap_val = wrap.unwrap_or(Value::FlexWrap(crate::values::FlexWrap::NoWrap));

    vec![
        Declaration::new("flex-direction", dir_val, important),
        Declaration::new("flex-wrap", wrap_val, important),
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
