//! Resolution of CSS custom properties (variables) and currentcolor.

use std::collections::{HashMap, HashSet};

use crate::values::{Length, MathExpr, MathId, Value, get_math, make_math_length};

#[inline]
pub(crate) fn is_current_color(val: &Value) -> bool {
    match val {
        Value::CurrentColor => true,
        Value::Keyword(k) => k.eq_ignore_ascii_case("currentcolor"),
        _ => false,
    }
}

#[inline]
pub(crate) fn value_has_vars(val: &Value) -> bool {
    match val {
        Value::Var { .. } | Value::Gradient(_) => true,
        Value::Length(Length::Math(id)) => math_has_vars(*id),
        Value::List(items) => items.iter().any(value_has_vars),
        _ => false,
    }
}

pub(crate) fn math_has_vars(id: MathId) -> bool {
    get_math(id).is_some_and(|m| expr_has_vars(&m))
}

pub(crate) fn expr_has_vars(expr: &MathExpr) -> bool {
    match expr {
        MathExpr::Var { .. } => true,
        MathExpr::Add(a, b)
        | MathExpr::Sub(a, b)
        | MathExpr::Mul(a, b)
        | MathExpr::Div(a, b)
        | MathExpr::Mod(a, b)
        | MathExpr::Rem(a, b)
        | MathExpr::Atan2(a, b)
        | MathExpr::Pow(a, b) => expr_has_vars(a) || expr_has_vars(b),
        MathExpr::Min(list) | MathExpr::Max(list) | MathExpr::Hypot(list) => {
            list.iter().any(expr_has_vars)
        }
        MathExpr::Clamp { min, val, max } => {
            expr_has_vars(min) || expr_has_vars(val) || expr_has_vars(max)
        }
        MathExpr::Round { val, step, .. } => expr_has_vars(val) || expr_has_vars(step),
        MathExpr::Abs(a)
        | MathExpr::Sign(a)
        | MathExpr::Sin(a)
        | MathExpr::Cos(a)
        | MathExpr::Tan(a)
        | MathExpr::Asin(a)
        | MathExpr::Acos(a)
        | MathExpr::Atan(a)
        | MathExpr::Sqrt(a) => expr_has_vars(a),
        _ => false,
    }
}

/// Recursively resolves `var(--name, fallback)` references using the element's custom properties.
pub(crate) fn resolve_value_vars(
    val: &Value,
    custom_props: &HashMap<String, Value>,
    visited: &mut HashSet<String>,
) -> Option<Value> {
    match val {
        Value::Var { name, fallback } => {
            if visited.contains(name) {
                // Cycle detected: fallback or invalid
                return fallback
                    .as_ref()
                    .and_then(|fb| resolve_value_vars(fb, custom_props, visited));
            }
            visited.insert(name.clone());
            let target_val_opt = custom_props.get(name);
            let res = if let Some(target_val) = target_val_opt {
                resolve_value_vars(target_val, custom_props, visited).or_else(|| {
                    fallback
                        .as_ref()
                        .and_then(|fb| resolve_value_vars(fb, custom_props, visited))
                })
            } else {
                fallback
                    .as_ref()
                    .and_then(|fb| resolve_value_vars(fb, custom_props, visited))
            };
            visited.remove(name);
            res
        }
        Value::Gradient(g) => {
            let mut grad = (**g).clone();
            grad.resolve_var_colors(custom_props);
            Some(Value::Gradient(Box::new(grad)))
        }
        Value::Length(Length::Math(id)) => {
            if let Some(expr) = get_math(*id) {
                let resolved = resolve_math_expr_vars(&expr, custom_props, visited);
                Some(Value::Length(make_math_length(resolved)))
            } else {
                Some(val.clone())
            }
        }
        Value::List(items) => {
            let mut has_var = false;
            for item in items {
                if value_has_vars(item) {
                    has_var = true;
                    break;
                }
            }
            if has_var {
                let mut new_items = Vec::with_capacity(items.len());
                for item in items {
                    let resolved_item = resolve_value_vars(item, custom_props, visited)
                        .unwrap_or_else(|| item.clone());
                    new_items.push(resolved_item);
                }
                Some(Value::List(new_items))
            } else {
                Some(val.clone())
            }
        }
        _ => Some(val.clone()),
    }
}

pub(crate) fn resolve_math_expr_vars(
    expr: &MathExpr,
    custom_props: &HashMap<String, Value>,
    visited: &mut HashSet<String>,
) -> MathExpr {
    match expr {
        MathExpr::Var { name, fallback } => {
            if visited.contains(name) {
                return fallback
                    .as_ref()
                    .map(|fb| resolve_math_expr_vars(fb, custom_props, visited))
                    .unwrap_or(MathExpr::Number(0.0));
            }
            visited.insert(name.clone());
            let res = if let Some(target_val) = custom_props.get(name) {
                match target_val {
                    Value::Length(l) => match l {
                        Length::Px(px) => MathExpr::Length(Length::Px(*px)),
                        Length::Em(em) => MathExpr::Length(Length::Em(*em)),
                        Length::Rem(rem) => MathExpr::Length(Length::Rem(*rem)),
                        Length::Percent(p) => MathExpr::Length(Length::Percent(*p)),
                        Length::Vw(vw) => MathExpr::Length(Length::Vw(*vw)),
                        Length::Vh(vh) => MathExpr::Length(Length::Vh(*vh)),
                        Length::Math(mid) => {
                            if let Some(m) = get_math(*mid) {
                                resolve_math_expr_vars(&m, custom_props, visited)
                            } else {
                                MathExpr::Number(0.0)
                            }
                        }
                        Length::Calc(calc) => MathExpr::Length(Length::Calc(*calc)),
                        _ => MathExpr::Length(*l),
                    },
                    Value::Number(n) => MathExpr::Number(*n),
                    Value::Percentage(p) => MathExpr::Length(Length::Percent(*p)),
                    Value::Var {
                        name: n2,
                        fallback: fb2,
                    } => {
                        let inner_var = MathExpr::Var {
                            name: n2.clone(),
                            fallback: fb2.as_ref().and_then(|f| match **f {
                                Value::Length(l) => Some(Box::new(MathExpr::Length(l))),
                                Value::Number(n) => Some(Box::new(MathExpr::Number(n))),
                                _ => None,
                            }),
                        };
                        resolve_math_expr_vars(&inner_var, custom_props, visited)
                    }
                    Value::Keyword(k) => {
                        if let Some(l) = Length::parse(k) {
                            MathExpr::Length(l)
                        } else if let Ok(n) = k.parse::<f32>() {
                            MathExpr::Number(n)
                        } else {
                            fallback
                                .as_ref()
                                .map(|fb| resolve_math_expr_vars(fb, custom_props, visited))
                                .unwrap_or(MathExpr::Number(0.0))
                        }
                    }
                    _ => fallback
                        .as_ref()
                        .map(|fb| resolve_math_expr_vars(fb, custom_props, visited))
                        .unwrap_or(MathExpr::Number(0.0)),
                }
            } else {
                fallback
                    .as_ref()
                    .map(|fb| resolve_math_expr_vars(fb, custom_props, visited))
                    .unwrap_or(MathExpr::Number(0.0))
            };
            visited.remove(name);
            res
        }
        MathExpr::Add(a, b) => MathExpr::Add(
            Box::new(resolve_math_expr_vars(a, custom_props, visited)),
            Box::new(resolve_math_expr_vars(b, custom_props, visited)),
        ),
        MathExpr::Sub(a, b) => MathExpr::Sub(
            Box::new(resolve_math_expr_vars(a, custom_props, visited)),
            Box::new(resolve_math_expr_vars(b, custom_props, visited)),
        ),
        MathExpr::Mul(a, b) => MathExpr::Mul(
            Box::new(resolve_math_expr_vars(a, custom_props, visited)),
            Box::new(resolve_math_expr_vars(b, custom_props, visited)),
        ),
        MathExpr::Div(a, b) => MathExpr::Div(
            Box::new(resolve_math_expr_vars(a, custom_props, visited)),
            Box::new(resolve_math_expr_vars(b, custom_props, visited)),
        ),
        MathExpr::Min(list) => MathExpr::Min(
            list.iter()
                .map(|e| resolve_math_expr_vars(e, custom_props, visited))
                .collect(),
        ),
        MathExpr::Max(list) => MathExpr::Max(
            list.iter()
                .map(|e| resolve_math_expr_vars(e, custom_props, visited))
                .collect(),
        ),
        MathExpr::Clamp { min, val, max } => MathExpr::Clamp {
            min: Box::new(resolve_math_expr_vars(min, custom_props, visited)),
            val: Box::new(resolve_math_expr_vars(val, custom_props, visited)),
            max: Box::new(resolve_math_expr_vars(max, custom_props, visited)),
        },
        MathExpr::Round {
            strategy,
            val,
            step,
        } => MathExpr::Round {
            strategy: *strategy,
            val: Box::new(resolve_math_expr_vars(val, custom_props, visited)),
            step: Box::new(resolve_math_expr_vars(step, custom_props, visited)),
        },
        MathExpr::Mod(a, b) => MathExpr::Mod(
            Box::new(resolve_math_expr_vars(a, custom_props, visited)),
            Box::new(resolve_math_expr_vars(b, custom_props, visited)),
        ),
        MathExpr::Rem(a, b) => MathExpr::Rem(
            Box::new(resolve_math_expr_vars(a, custom_props, visited)),
            Box::new(resolve_math_expr_vars(b, custom_props, visited)),
        ),
        MathExpr::Abs(a) => {
            MathExpr::Abs(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Sign(a) => {
            MathExpr::Sign(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Sin(a) => {
            MathExpr::Sin(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Cos(a) => {
            MathExpr::Cos(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Tan(a) => {
            MathExpr::Tan(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Asin(a) => {
            MathExpr::Asin(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Acos(a) => {
            MathExpr::Acos(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Atan(a) => {
            MathExpr::Atan(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Atan2(y, x) => MathExpr::Atan2(
            Box::new(resolve_math_expr_vars(y, custom_props, visited)),
            Box::new(resolve_math_expr_vars(x, custom_props, visited)),
        ),
        MathExpr::Sqrt(a) => {
            MathExpr::Sqrt(Box::new(resolve_math_expr_vars(a, custom_props, visited)))
        }
        MathExpr::Pow(base, exp) => MathExpr::Pow(
            Box::new(resolve_math_expr_vars(base, custom_props, visited)),
            Box::new(resolve_math_expr_vars(exp, custom_props, visited)),
        ),
        MathExpr::Hypot(list) => MathExpr::Hypot(
            list.iter()
                .map(|e| resolve_math_expr_vars(e, custom_props, visited))
                .collect(),
        ),
        other => other.clone(),
    }
}
