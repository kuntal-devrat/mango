//! CSS canonical serialization for MathExpr.

use super::expr::MathExpr;
use crate::values::length::Length;

fn format_number(n: f32) -> String {
    let rounded = (n * 1000.0).round() / 1000.0;
    if (rounded - rounded.round()).abs() < 1e-4 {
        format!("{:.0}", rounded.round())
    } else {
        format!("{rounded}")
    }
}

fn format_length_leaf(l: &Length) -> String {
    match l {
        Length::Px(px) => {
            let r = (*px * 1000.0).round() / 1000.0;
            if (r - r.round()).abs() < 1e-4 {
                format!("{:.0}px", r.round())
            } else {
                format!("{r}px")
            }
        }
        Length::Percent(pct) => {
            let r = (*pct * 1000.0).round() / 1000.0;
            if (r - r.round()).abs() < 1e-4 {
                format!("{:.0}%", r.round())
            } else {
                format!("{r}%")
            }
        }
        Length::Em(em) => format!("{}em", format_number(*em)),
        Length::Ex(ex) => format!("{}ex", format_number(*ex)),
        Length::Rem(rem) => format!("{}rem", format_number(*rem)),
        Length::Vw(vw) => format!("{}vw", format_number(*vw)),
        Length::Vh(vh) => format!("{}vh", format_number(*vh)),
        Length::Vmin(v) => format!("{}vmin", format_number(*v)),
        Length::Vmax(v) => format!("{}vmax", format_number(*v)),
        Length::Cqw(v) => format!("{}cqw", format_number(*v)),
        Length::Cqh(v) => format!("{}cqh", format_number(*v)),
        Length::Cqi(v) => format!("{}cqi", format_number(*v)),
        Length::Cqb(v) => format!("{}cqb", format_number(*v)),
        Length::Cqmin(v) => format!("{}cqmin", format_number(*v)),
        Length::Cqmax(v) => format!("{}cqmax", format_number(*v)),
        Length::Auto => "auto".to_string(),
        Length::Content => "content".to_string(),
        Length::MinContent => "min-content".to_string(),
        Length::MaxContent => "max-content".to_string(),
        Length::FitContent => "fit-content".to_string(),
        Length::Calc(calc) => {
            let mut parts = Vec::new();
            if calc.has_percent || calc.percent != 0.0 {
                parts.push(format!("{}%", format_number(calc.percent)));
            }
            if calc.em != 0.0 {
                parts.push(format!("{}em", format_number(calc.em)));
            }
            if calc.ex != 0.0 {
                parts.push(format!("{}ex", format_number(calc.ex)));
            }
            if calc.px != 0.0 {
                parts.push(format!("{}px", format_number(calc.px)));
            }
            if calc.rem != 0.0 {
                parts.push(format!("{}rem", format_number(calc.rem)));
            }
            if calc.vh != 0.0 {
                parts.push(format!("{}vh", format_number(calc.vh)));
            }
            if calc.vw != 0.0 {
                parts.push(format!("{}vw", format_number(calc.vw)));
            }
            if parts.is_empty() {
                "0px".to_string()
            } else if parts.len() == 1 {
                parts[0].clone()
            } else {
                parts.join(" + ")
            }
        }
        Length::Math(mid) => {
            if let Some(expr) = super::store::get_math(*mid) {
                format_math_inner(&expr)
            } else {
                "0px".to_string()
            }
        }
        Length::Anchor { .. } => "auto".to_string(),
    }
}

pub fn format_math_inner(expr: &MathExpr) -> String {
    match expr {
        MathExpr::Length(l) => format_length_leaf(l),
        MathExpr::Number(n) => format_number(*n),
        MathExpr::Add(a, b) => {
            let a_str = format_product_paren(a);
            let b_str = format_product_paren(b);
            format!("{a_str} + {b_str}")
        }
        MathExpr::Sub(a, b) => {
            let a_str = format_product_paren(a);
            let b_str = format_product_paren(b);
            format!("{a_str} - {b_str}")
        }
        MathExpr::Mul(a, b) => {
            let (first, second) = match (&**a, &**b) {
                (MathExpr::Number(_), _) => (a.as_ref(), b.as_ref()),
                (MathExpr::Length(Length::Px(_)), _) => (a.as_ref(), b.as_ref()),
                (_, MathExpr::Number(_)) => (b.as_ref(), a.as_ref()),
                (_, MathExpr::Length(Length::Px(_))) => (b.as_ref(), a.as_ref()),
                _ => (a.as_ref(), b.as_ref()),
            };
            format!(
                "{} * {}",
                format_math_inner(first),
                format_math_inner(second)
            )
        }
        MathExpr::Div(a, b) => {
            format!("{} / {}", format_math_inner(a), format_math_inner(b))
        }
        MathExpr::Sign(a) => {
            format!("sign({})", format_math_inner(a))
        }
        MathExpr::Abs(a) => {
            format!("abs({})", format_math_inner(a))
        }
        MathExpr::Min(args) => {
            let inner = args
                .iter()
                .map(format_math_inner)
                .collect::<Vec<_>>()
                .join(", ");
            format!("min({inner})")
        }
        MathExpr::Max(args) => {
            let inner = args
                .iter()
                .map(format_math_inner)
                .collect::<Vec<_>>()
                .join(", ");
            format!("max({inner})")
        }
        MathExpr::Clamp { min, val, max } => {
            format!(
                "clamp({}, {}, {})",
                format_math_inner(min),
                format_math_inner(val),
                format_math_inner(max)
            )
        }
        _ => "0px".to_string(),
    }
}

fn format_product_paren(expr: &MathExpr) -> String {
    match expr {
        MathExpr::Mul(_, _) => format!("({})", format_math_inner(expr)),
        _ => format_math_inner(expr),
    }
}

pub fn format_math_toplevel(expr: &MathExpr) -> String {
    let inner = format_math_inner(expr);
    if inner.starts_with("min(") || inner.starts_with("max(") || inner.starts_with("clamp(") {
        inner
    } else {
        format!("calc({inner})")
    }
}
