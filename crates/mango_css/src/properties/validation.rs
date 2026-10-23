//! Validation of CSS property names and values according to W3C specifications.

use crate::values::{Length, Value};

/// Returns true if `val` is a valid CSS-wide keyword.
#[inline]
pub fn is_css_wide_keyword(val: &Value) -> bool {
    if let Value::Keyword(k) = val {
        matches!(
            k.to_ascii_lowercase().as_str(),
            "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "revert-rule"
        )
    } else {
        false
    }
}

fn is_valid_margin_val(v: &Value) -> bool {
    match v {
        Value::Length(len) => match len {
            Length::Px(_) | Length::Percent(_) | Length::Auto => true,
            Length::Calc(_)
            | Length::Em(_)
            | Length::Ex(_)
            | Length::Rem(_)
            | Length::Vw(_)
            | Length::Vh(_)
            | Length::Vmin(_)
            | Length::Vmax(_) => true,
            Length::Math(expr) => !expr.is_dimensionless_number(),
            _ => false,
        },
        Value::Number(n) if *n == 0.0 => true,
        _ => false,
    }
}

fn is_valid_padding_val(v: &Value) -> bool {
    match v {
        Value::Length(len) => match len {
            Length::Px(px) => *px >= 0.0,
            Length::Percent(pct) => *pct >= 0.0,
            Length::Calc(_)
            | Length::Em(_)
            | Length::Ex(_)
            | Length::Rem(_)
            | Length::Vw(_)
            | Length::Vh(_)
            | Length::Vmin(_)
            | Length::Vmax(_) => true,
            Length::Math(expr) => !expr.is_dimensionless_number(),
            _ => false,
        },
        Value::Number(n) if *n == 0.0 => true,
        _ => false,
    }
}

/// Returns true if `value` contains a `var()` reference (at any nesting depth).
///
/// Per CSS Variables Level 1 § 2.2, a declaration whose value contains `var()`
/// cannot be validated at parse time: it is only *invalid at computed-value time*
/// if the referenced custom property is missing and has no fallback. Therefore the
/// parse-time grammar check must accept any property value that mentions `var()`.
fn value_contains_var(value: &Value) -> bool {
    match value {
        Value::Var { .. } => true,
        Value::List(items) => items.iter().any(value_contains_var),
        Value::Gradient(g) => gradient_contains_var(g),
        _ => false,
    }
}

fn gradient_contains_var(g: &crate::values::Gradient) -> bool {
    // Gradients embed custom-property references inside their color stops.
    g.stops().iter().any(|s| s.var_name.is_some())
}

/// Returns true if `value` is valid for `property_name` under W3C CSS grammar.
pub fn is_valid_property_value(property_name: &str, value: &Value) -> bool {
    let prop = property_name.to_ascii_lowercase();

    // Custom properties accept any non-empty CSS value
    if prop.starts_with("--") {
        return true;
    }

    // CSS Variables Level 1 § 2.2: a declaration containing `var()` is always
    // syntactically valid; it is resolved (or invalidated) at computed-value time.
    if value_contains_var(value) {
        return true;
    }

    // NOTE: we deliberately do *not* reject declarations whose property name is
    // unknown to `CssPropertyId`. That registry is an optimisation, not the
    // engine's support matrix: the cascade/apply layer already ignores properties
    // it does not implement, so dropping them here would only hide them from the
    // computed style. Only the value grammar is checked here, and only for the
    // properties that have explicit rules below (everything else falls through to
    // `true`).

    // CSS-wide keywords are accepted on any valid standard property
    if is_css_wide_keyword(value) {
        return true;
    }

    match prop.as_str() {
        "all" => is_css_wide_keyword(value),

        "clear" => matches!(value, Value::Clear(_)),
        "float" => matches!(value, Value::Float(_)),
        "visibility" => matches!(value, Value::Visibility(_)),
        "display" => matches!(value, Value::Display(_)),
        "position" => matches!(value, Value::Position(_)),
        "box-sizing" => matches!(value, Value::BoxSizing(_)),
        "overflow" | "overflow-x" | "overflow-y" => matches!(value, Value::Overflow(_)),

        "width" | "min-width" | "height" | "min-height" => match value {
            Value::Length(len) => match len {
                Length::Px(px) => *px >= 0.0,
                Length::Percent(pct) => *pct >= 0.0,
                Length::Auto | Length::MinContent | Length::MaxContent | Length::FitContent => true,
                Length::Calc(_)
                | Length::Em(_)
                | Length::Ex(_)
                | Length::Rem(_)
                | Length::Vw(_)
                | Length::Vh(_)
                | Length::Vmin(_)
                | Length::Vmax(_) => true,
                Length::Math(expr) => !expr.is_dimensionless_number(),
                _ => false,
            },
            Value::Number(n) if *n == 0.0 => true,
            _ => false,
        },

        "max-width" | "max-height" => match value {
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => true,
            Value::Length(len) => match len {
                Length::Px(px) => *px >= 0.0,
                Length::Percent(pct) => *pct >= 0.0,
                Length::MinContent | Length::MaxContent | Length::FitContent => true,
                Length::Calc(_)
                | Length::Em(_)
                | Length::Ex(_)
                | Length::Rem(_)
                | Length::Vw(_)
                | Length::Vh(_)
                | Length::Vmin(_)
                | Length::Vmax(_) => true,
                Length::Math(expr) => !expr.is_dimensionless_number(),
                _ => false, // Auto is invalid for max-width/max-height!
            },
            Value::Number(n) if *n == 0.0 => true,
            _ => false,
        },

        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
            is_valid_margin_val(value)
        }

        "margin" => match value {
            Value::List(list) => {
                !list.is_empty() && list.len() <= 4 && list.iter().all(is_valid_margin_val)
            }
            single => is_valid_margin_val(single),
        },

        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
            is_valid_padding_val(value)
        }

        "padding" => match value {
            Value::List(list) => {
                !list.is_empty() && list.len() <= 4 && list.iter().all(is_valid_padding_val)
            }
            single => is_valid_padding_val(single),
        },

        "top" | "right" | "bottom" | "left" | "inset-block-start" | "inset-block-end"
        | "inset-inline-start" | "inset-inline-end" => is_valid_margin_val(value),

        "inset" => match value {
            Value::List(list) => {
                !list.is_empty() && list.len() <= 4 && list.iter().all(is_valid_margin_val)
            }
            single => is_valid_margin_val(single),
        },

        "inset-block" | "inset-inline" => match value {
            Value::List(list) => {
                !list.is_empty() && list.len() <= 2 && list.iter().all(is_valid_margin_val)
            }
            single => is_valid_margin_val(single),
        },

        "margin-trim" => match value {
            Value::Keyword(k) | Value::String(k) => matches!(
                k.to_ascii_lowercase().as_str(),
                "none" | "block" | "block-start" | "block-end"
            ),
            Value::List(list) if list.len() == 2 => {
                let w1 = match &list[0] {
                    Value::Keyword(k) | Value::String(k) => k.to_ascii_lowercase(),
                    _ => return false,
                };
                let w2 = match &list[1] {
                    Value::Keyword(k) | Value::String(k) => k.to_ascii_lowercase(),
                    _ => return false,
                };
                (w1 == "block-start" && w2 == "block-end")
                    || (w1 == "block-end" && w2 == "block-start")
            }
            _ => false,
        },

        "z-index" => match value {
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => true,
            Value::Length(Length::Auto) => true,
            Value::Number(n) => n.fract() == 0.0,
            Value::Length(Length::Calc(_) | Length::Math(_)) => true,
            _ => false,
        },

        "order" => match value {
            Value::Number(n) => n.fract() == 0.0,
            _ => false,
        },

        "flex-grow" | "flex-shrink" => match value {
            Value::Number(n) => *n >= 0.0,
            Value::Length(Length::Calc(_) | Length::Math(_)) => true,
            _ => false,
        },

        "flex-basis" => match value {
            Value::Length(len) => match len {
                Length::Px(px) => *px >= 0.0,
                Length::Percent(pct) => *pct >= 0.0,
                Length::Auto
                | Length::Content
                | Length::MinContent
                | Length::MaxContent
                | Length::FitContent => true,
                Length::Calc(_)
                | Length::Math(_)
                | Length::Em(_)
                | Length::Ex(_)
                | Length::Rem(_)
                | Length::Vw(_)
                | Length::Vh(_)
                | Length::Vmin(_)
                | Length::Vmax(_) => true,
                _ => false,
            },
            Value::Number(n) if *n == 0.0 => true,
            _ => false,
        },

        "overlay" => match value {
            Value::Keyword(k) => matches!(k.to_ascii_lowercase().as_str(), "none" | "auto"),
            Value::Length(Length::Auto) => true,
            _ => false,
        },

        "flex-direction" => match value {
            Value::FlexDirection(_) => true,
            Value::Keyword(k) => matches!(
                k.to_ascii_lowercase().as_str(),
                "row" | "row-reverse" | "column" | "column-reverse"
            ),
            _ => false,
        },

        "flex-wrap" => match value {
            Value::FlexWrap(_) => true,
            Value::Keyword(k) => matches!(
                k.to_ascii_lowercase().as_str(),
                "nowrap" | "wrap" | "wrap-reverse" | "balance" | "wrap-reverse balance"
            ),
            _ => false,
        },

        "flex-line-count" => match value {
            Value::Number(n) => *n >= 1.0 && n.fract() == 0.0,
            Value::Length(Length::Calc(_) | Length::Math(_)) => true,
            _ => false,
        },

        _ => true,
    }
}
