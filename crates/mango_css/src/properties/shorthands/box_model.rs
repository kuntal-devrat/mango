//! Shorthand expansion for box model: margins, padding, insets, border-radius, overflow.

use crate::properties::Declaration;
use crate::values::Value;

pub(crate) fn expand_border_radius_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (tl, tr, br, bl) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => return vec![Declaration::new("border-radius", value.clone(), important)],
    };

    vec![
        Declaration::new("border-top-left-radius", tl, important),
        Declaration::new("border-top-right-radius", tr, important),
        Declaration::new("border-bottom-right-radius", br, important),
        Declaration::new("border-bottom-left-radius", bl, important),
    ]
}

/// Expands overflow shorthand into overflow-x and overflow-y.
pub(crate) fn expand_overflow_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (ox, oy) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return vec![Declaration::new("overflow", value.clone(), important)],
    };

    vec![
        Declaration::new("overflow-x", ox, important),
        Declaration::new("overflow-y", oy, important),
    ]
}

/// Expands list-style shorthand into list-style-type and list-style-position.
pub(crate) fn expand_list_style_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let mut decls = Vec::new();
    for v in values {
        match &v {
            Value::ListStyleType(_) => {
                decls.push(Declaration::new("list-style-type", v, important));
            }
            Value::ListStylePosition(_) => {
                decls.push(Declaration::new("list-style-position", v, important));
            }
            _ => {}
        }
    }
    if decls.is_empty() {
        vec![Declaration::new(
            "list-style-type",
            value.clone(),
            important,
        )]
    } else {
        decls
    }
}

/// Expands 1, 2, 3, or 4-value box shorthands (margin / padding).
pub(crate) fn expand_four_sides(prefix: &str, value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    for v in &values {
        let is_valid = match prefix {
            "padding" => match v {
                Value::Length(len) => match len {
                    crate::values::Length::Px(px) => *px >= 0.0,
                    crate::values::Length::Percent(pct) => *pct >= 0.0,
                    crate::values::Length::Calc(_)
                    | crate::values::Length::Math(_)
                    | crate::values::Length::Em(_)
                    | crate::values::Length::Rem(_)
                    | crate::values::Length::Vw(_)
                    | crate::values::Length::Vh(_)
                    | crate::values::Length::Vmin(_)
                    | crate::values::Length::Vmax(_) => true,
                    _ => false,
                },
                _ => false,
            },
            "margin" => match v {
                Value::Length(len) => matches!(
                    len,
                    crate::values::Length::Px(_)
                        | crate::values::Length::Percent(_)
                        | crate::values::Length::Auto
                        | crate::values::Length::Calc(_)
                        | crate::values::Length::Math(_)
                        | crate::values::Length::Em(_)
                        | crate::values::Length::Rem(_)
                        | crate::values::Length::Vw(_)
                        | crate::values::Length::Vh(_)
                        | crate::values::Length::Vmin(_)
                        | crate::values::Length::Vmax(_)
                ),
                _ => false,
            },
            _ => true,
        };
        if !is_valid {
            return Vec::new();
        }
    }

    let (top, right, bottom, left) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => return Vec::new(),
    };

    vec![
        Declaration::new(format!("{prefix}-top"), top, important),
        Declaration::new(format!("{prefix}-right"), right, important),
        Declaration::new(format!("{prefix}-bottom"), bottom, important),
        Declaration::new(format!("{prefix}-left"), left, important),
    ]
}

/// Expands `inset: <top> <right> <bottom> <left>` box offsets shorthand.
pub(crate) fn expand_inset_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (top, right, bottom, left) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => return Vec::new(),
    };

    vec![
        Declaration::new("top", top, important),
        Declaration::new("right", right, important),
        Declaration::new("bottom", bottom, important),
        Declaration::new("left", left, important),
    ]
}

/// Expands `inset-inline: <start> <end>` logical shorthand.
pub(crate) fn expand_inset_inline_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    let (start, end) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return Vec::new(),
    };
    vec![
        Declaration::new("inset-inline-start", start, important),
        Declaration::new("inset-inline-end", end, important),
    ]
}

/// Expands `inset-block: <start> <end>` logical shorthand.
pub(crate) fn expand_inset_block_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    let (start, end) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return Vec::new(),
    };
    vec![
        Declaration::new("inset-block-start", start, important),
        Declaration::new("inset-block-end", end, important),
    ]
}

/// Expands border scalar properties (e.g. `border-width: 1px 2px`).
pub(crate) fn expand_four_sides_scalar(
    prefix: &str,
    suffix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (top, right, bottom, left) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => return Vec::new(),
    };

    vec![
        Declaration::new(format!("{prefix}-top-{suffix}"), top, important),
        Declaration::new(format!("{prefix}-right-{suffix}"), right, important),
        Declaration::new(format!("{prefix}-bottom-{suffix}"), bottom, important),
        Declaration::new(format!("{prefix}-left-{suffix}"), left, important),
    ]
}

/// Expands 1-2 value logical inline shorthands (e.g. `padding-inline`, `margin-inline`).
pub(crate) fn expand_inline_shorthand(
    prefix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    let (start, end) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return Vec::new(),
    };
    vec![
        Declaration::new(format!("{prefix}-left"), start, important),
        Declaration::new(format!("{prefix}-right"), end, important),
    ]
}

/// Expands 1-2 value logical block shorthands (e.g. `padding-block`, `margin-block`).
pub(crate) fn expand_block_shorthand(
    prefix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    let (start, end) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return Vec::new(),
    };
    vec![
        Declaration::new(format!("{prefix}-top"), start, important),
        Declaration::new(format!("{prefix}-bottom"), end, important),
    ]
}

/// Expands 1-2 value scalar shorthands for two sides (e.g. `border-inline-width: 1px 2px`).
pub(crate) fn expand_two_sides_scalar(
    prefix: &str,
    side1: &str,
    side2: &str,
    suffix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    let (val1, val2) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return Vec::new(),
    };
    vec![
        Declaration::new(format!("{prefix}-{side1}-{suffix}"), val1, important),
        Declaration::new(format!("{prefix}-{side2}-{suffix}"), val2, important),
    ]
}
