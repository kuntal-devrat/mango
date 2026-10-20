//! Shorthand expansion for `border`, `outline`, and `column-rule`.

use crate::properties::Declaration;
use crate::values::{BorderStyle, Display, Length, Value};

pub(crate) fn expand_border_sides(
    sides: &[&str],
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut width = None;
    let mut style = None;
    let mut color = None;

    for item in items {
        match item {
            Value::Length(len) => {
                if let Length::Px(px) = len
                    && *px == 0.0
                {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                }
                width = Some(item.clone());
            }
            Value::Number(n) => {
                if *n == 0.0 {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                }
                width = Some(Value::Length(Length::Px(*n)));
            }
            Value::BorderStyle(_) => style = Some(item.clone()),
            Value::Color(_) => color = Some(item.clone()),
            Value::Var { name, fallback } => {
                classify_and_assign_var(item, name, fallback, &mut width, &mut color);
            }
            Value::Display(Display::None) => {
                style = Some(Value::BorderStyle(BorderStyle::None));
                width = Some(Value::Length(Length::Px(0.0)));
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" | "hidden" => {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                    width = Some(Value::Length(Length::Px(0.0)));
                }
                "solid" | "groove" | "ridge" | "inset" | "outset" => {
                    style = Some(Value::BorderStyle(BorderStyle::Solid));
                }
                "dashed" => style = Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => style = Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => style = Some(Value::BorderStyle(BorderStyle::Double)),
                "thin" => width = Some(Value::Length(Length::Px(1.0))),
                "medium" => width = Some(Value::Length(Length::Px(3.0))),
                "thick" => width = Some(Value::Length(Length::Px(5.0))),
                "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                "currentcolor" => color = Some(Value::CurrentColor),
                _ => {
                    if let Some(c) = Value::parse_color(k) {
                        color = Some(Value::Color(c));
                    }
                }
            },
            _ => {}
        }
    }

    let is_none_style = style
        .as_ref()
        .map(|s| {
            matches!(
                s,
                Value::BorderStyle(BorderStyle::None) | Value::BorderStyle(BorderStyle::Hidden)
            )
        })
        .unwrap_or(false)
        || width
            .as_ref()
            .map(|w| matches!(w, Value::Length(Length::Px(p)) if *p == 0.0))
            .unwrap_or(false);
    let default_width = if is_none_style {
        Length::Px(0.0)
    } else {
        Length::Px(1.0)
    };
    let default_style = if is_none_style {
        BorderStyle::None
    } else {
        BorderStyle::Solid
    };
    let default_color = if is_none_style {
        Value::Color(mango_core::Color::TRANSPARENT)
    } else {
        Value::CurrentColor
    };

    let width_val = width.unwrap_or(Value::Length(default_width));
    let style_val = style.unwrap_or(Value::BorderStyle(default_style));
    let color_val = color.unwrap_or(default_color);

    let mut decls = Vec::with_capacity(sides.len() * 3);
    for side in sides {
        decls.push(Declaration::new(
            format!("border-{side}-width"),
            width_val.clone(),
            important,
        ));
        decls.push(Declaration::new(
            format!("border-{side}-style"),
            style_val.clone(),
            important,
        ));
        decls.push(Declaration::new(
            format!("border-{side}-color"),
            color_val.clone(),
            important,
        ));
    }
    decls
}

pub(crate) fn expand_column_rule_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut width = None;
    let mut style = None;
    let mut color = None;

    for item in items {
        match item {
            Value::Length(_) => width = Some(item.clone()),
            Value::Number(n) if *n == 0.0 => width = Some(Value::Length(Length::Px(0.0))),
            Value::BorderStyle(_) => style = Some(item.clone()),
            Value::Color(_) => color = Some(item.clone()),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" | "hidden" => style = Some(Value::BorderStyle(BorderStyle::None)),
                "solid" => style = Some(Value::BorderStyle(BorderStyle::Solid)),
                "dashed" => style = Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => style = Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => style = Some(Value::BorderStyle(BorderStyle::Double)),
                "groove" => style = Some(Value::BorderStyle(BorderStyle::Groove)),
                "ridge" => style = Some(Value::BorderStyle(BorderStyle::Ridge)),
                "inset" => style = Some(Value::BorderStyle(BorderStyle::Inset)),
                "outset" => style = Some(Value::BorderStyle(BorderStyle::Outset)),
                "thin" => width = Some(Value::Length(Length::Px(1.0))),
                "medium" => width = Some(Value::Length(Length::Px(3.0))),
                "thick" => width = Some(Value::Length(Length::Px(5.0))),
                _ => {
                    if let Some(c) = Value::parse_color(k) {
                        color = Some(Value::Color(c));
                    }
                }
            },
            _ => {}
        }
    }

    vec![
        Declaration::new(
            "column-rule-width",
            width.unwrap_or(Value::Length(Length::Px(1.0))),
            important,
        ),
        Declaration::new(
            "column-rule-style",
            style.unwrap_or(Value::BorderStyle(BorderStyle::Solid)),
            important,
        ),
        Declaration::new(
            "column-rule-color",
            color.unwrap_or(Value::Color(mango_core::Color::BLACK)),
            important,
        ),
    ]
}

/// Expands `outline: 1px solid red`, `outline: none`, `outline: 0`, etc.
pub(crate) fn expand_outline_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut width = None;
    let mut style = None;
    let mut color = None;

    for item in items {
        match item {
            Value::Length(len) => {
                if let Length::Px(px) = len
                    && *px == 0.0
                {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                }
                width = Some(item.clone());
            }
            Value::Number(n) if *n == 0.0 => {
                style = Some(Value::BorderStyle(BorderStyle::None));
                width = Some(Value::Length(Length::Px(0.0)));
            }
            Value::BorderStyle(_) => style = Some(item.clone()),
            Value::Color(_) => color = Some(item.clone()),
            Value::Var { name, fallback } => {
                classify_and_assign_var(item, name, fallback, &mut width, &mut color);
            }
            Value::Display(Display::None) => {
                style = Some(Value::BorderStyle(BorderStyle::None));
                width = Some(Value::Length(Length::Px(0.0)));
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" | "hidden" => {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                    width = Some(Value::Length(Length::Px(0.0)));
                }
                "solid" | "groove" | "ridge" | "inset" | "outset" => {
                    style = Some(Value::BorderStyle(BorderStyle::Solid));
                }
                "dashed" => style = Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => style = Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => style = Some(Value::BorderStyle(BorderStyle::Double)),
                "thin" => width = Some(Value::Length(Length::Px(1.0))),
                "medium" => width = Some(Value::Length(Length::Px(3.0))),
                "thick" => width = Some(Value::Length(Length::Px(5.0))),
                "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                "currentcolor" => color = Some(Value::CurrentColor),
                "invert" => color = Some(Value::Color(mango_core::Color::BLACK)),
                _ => {
                    if let Some(c) = Value::parse_color(k) {
                        color = Some(Value::Color(c));
                    }
                }
            },
            _ => {}
        }
    }

    let is_none_style = style
        .as_ref()
        .map(|s| {
            matches!(
                s,
                Value::BorderStyle(BorderStyle::None) | Value::BorderStyle(BorderStyle::Hidden)
            )
        })
        .unwrap_or(false)
        || width
            .as_ref()
            .map(|w| matches!(w, Value::Length(Length::Px(p)) if *p == 0.0))
            .unwrap_or(false);
    let default_width = if is_none_style {
        Length::Px(0.0)
    } else {
        Length::Px(1.0)
    };
    let default_style = if is_none_style {
        BorderStyle::None
    } else {
        BorderStyle::Solid
    };
    let default_color = if is_none_style {
        mango_core::Color::TRANSPARENT
    } else {
        mango_core::Color::BLACK
    };

    let width_val = width.unwrap_or(Value::Length(default_width));
    let style_val = style.unwrap_or(Value::BorderStyle(default_style));
    let color_val = color.unwrap_or(Value::Color(default_color));

    vec![
        Declaration::new("outline-width", width_val, important),
        Declaration::new("outline-style", style_val, important),
        Declaration::new("outline-color", color_val, important),
    ]
}

fn classify_and_assign_var(
    item: &Value,
    name: &str,
    fallback: &Option<Box<Value>>,
    width: &mut Option<Value>,
    color: &mut Option<Value>,
) {
    let lower = name.to_ascii_lowercase();
    let is_width = lower.contains("width")
        || lower.contains("size")
        || matches!(
            fallback.as_deref(),
            Some(Value::Length(_)) | Some(Value::Number(_))
        );
    let is_color = lower.contains("color")
        || matches!(
            fallback.as_deref(),
            Some(Value::Color(_)) | Some(Value::CurrentColor)
        );

    if is_width && width.is_none() {
        *width = Some(item.clone());
    } else if is_color && color.is_none() {
        *color = Some(item.clone());
    } else if width.is_none() && color.is_none() {
        if let Some(fb) = fallback {
            if matches!(**fb, Value::Length(_) | Value::Number(_)) {
                *width = Some(item.clone());
            } else {
                *color = Some(item.clone());
            }
        } else {
            *color = Some(item.clone());
        }
    } else if width.is_none() {
        *width = Some(item.clone());
    } else if color.is_none() {
        *color = Some(item.clone());
    }
}
