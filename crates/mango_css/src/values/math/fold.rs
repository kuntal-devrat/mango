//! Constant folding, algebraic simplification, and type classification for MathExpr.

use super::calc::CalcLength;
use super::expr::MathExpr;
use crate::values::length::Length;

impl MathExpr {
    /// Returns `true` if this expression resolves to a dimensionless `<number>` rather than a `<length-percentage>`.
    pub fn is_dimensionless_number(&self) -> bool {
        match self {
            MathExpr::Number(_) => true,
            MathExpr::Length(_) => false,
            MathExpr::Sign(_)
            | MathExpr::Sin(_)
            | MathExpr::Cos(_)
            | MathExpr::Tan(_)
            | MathExpr::Asin(_)
            | MathExpr::Acos(_)
            | MathExpr::Atan(_)
            | MathExpr::Atan2(_, _) => true,
            MathExpr::Sqrt(a) => a.is_dimensionless_number(),
            MathExpr::Pow(base, _) => base.is_dimensionless_number(),
            MathExpr::Add(a, b) | MathExpr::Sub(a, b) | MathExpr::Mul(a, b) => {
                a.is_dimensionless_number() && b.is_dimensionless_number()
            }
            MathExpr::Div(a, b) => {
                if b.is_dimensionless_number() {
                    a.is_dimensionless_number()
                } else {
                    !a.is_dimensionless_number()
                }
            }
            MathExpr::Abs(a) => a.is_dimensionless_number(),
            MathExpr::Round { val, .. } => val.is_dimensionless_number(),
            MathExpr::Mod(a, _) | MathExpr::Rem(a, _) => a.is_dimensionless_number(),
            MathExpr::Min(list) | MathExpr::Max(list) => list
                .first()
                .is_some_and(|item| item.is_dimensionless_number()),
            MathExpr::Clamp { val, .. } => val.is_dimensionless_number(),
            MathExpr::Hypot(list) => list
                .first()
                .is_none_or(|item| item.is_dimensionless_number()),
            MathExpr::Var { fallback, .. } => fallback
                .as_ref()
                .is_some_and(|fb| fb.is_dimensionless_number()),
        }
    }

    /// Recursively simplifies constant numeric arithmetic (e.g. `2 + 1` -> `3`).
    pub fn simplify(&self) -> MathExpr {
        match self {
            MathExpr::Add(a, b) => {
                let a_s = a.simplify();
                let b_s = b.simplify();
                if let (MathExpr::Number(an), MathExpr::Number(bn)) = (&a_s, &b_s) {
                    MathExpr::Number(an + bn)
                } else {
                    MathExpr::Add(Box::new(a_s), Box::new(b_s))
                }
            }
            MathExpr::Sub(a, b) => {
                let a_s = a.simplify();
                let b_s = b.simplify();
                if let (MathExpr::Number(an), MathExpr::Number(bn)) = (&a_s, &b_s) {
                    MathExpr::Number(an - bn)
                } else {
                    MathExpr::Sub(Box::new(a_s), Box::new(b_s))
                }
            }
            MathExpr::Mul(a, b) => {
                let a_s = a.simplify();
                let b_s = b.simplify();
                if let (MathExpr::Number(an), MathExpr::Number(bn)) = (&a_s, &b_s) {
                    MathExpr::Number(an * bn)
                } else {
                    MathExpr::Mul(Box::new(a_s), Box::new(b_s))
                }
            }
            MathExpr::Div(a, b) => {
                let a_s = a.simplify();
                let b_s = b.simplify();
                if let (MathExpr::Number(an), MathExpr::Number(bn)) = (&a_s, &b_s) {
                    if bn.abs() > 1e-6 {
                        MathExpr::Number(an / bn)
                    } else {
                        MathExpr::Div(Box::new(a_s), Box::new(b_s))
                    }
                } else {
                    MathExpr::Div(Box::new(a_s), Box::new(b_s))
                }
            }
            MathExpr::Min(list) => {
                let simplified: Vec<MathExpr> = list.iter().map(|e| e.simplify()).collect();
                if simplified.iter().all(|e| matches!(e, MathExpr::Number(_))) {
                    let mut min_val = f32::INFINITY;
                    for e in &simplified {
                        if let MathExpr::Number(n) = e {
                            min_val = min_val.min(*n);
                        }
                    }
                    if !min_val.is_infinite() {
                        return MathExpr::Number(min_val);
                    }
                }
                MathExpr::Min(simplified)
            }
            MathExpr::Max(list) => {
                let simplified: Vec<MathExpr> = list.iter().map(|e| e.simplify()).collect();
                if simplified.iter().all(|e| matches!(e, MathExpr::Number(_))) {
                    let mut max_val = f32::NEG_INFINITY;
                    for e in &simplified {
                        if let MathExpr::Number(n) = e {
                            max_val = max_val.max(*n);
                        }
                    }
                    if !max_val.is_infinite() {
                        return MathExpr::Number(max_val);
                    }
                }
                MathExpr::Max(simplified)
            }
            MathExpr::Abs(a) => {
                let a_s = a.simplify();
                if let MathExpr::Number(an) = a_s {
                    MathExpr::Number(an.abs())
                } else {
                    MathExpr::Abs(Box::new(a_s))
                }
            }
            _ => self.clone(),
        }
    }

    /// Attempts to fold constant pixel lengths at parse time.
    pub fn try_fold_px(&self) -> Option<f32> {
        match self {
            MathExpr::Var { .. } => None,
            MathExpr::Length(Length::Px(px)) => Some(*px),
            MathExpr::Length(_) => None,
            MathExpr::Number(n) => Some(*n),
            MathExpr::Add(a, b) => Some(a.try_fold_px()? + b.try_fold_px()?),
            MathExpr::Sub(a, b) => Some(a.try_fold_px()? - b.try_fold_px()?),
            MathExpr::Mul(a, b) => Some(a.try_fold_px()? * b.try_fold_px()?),
            MathExpr::Div(a, b) => {
                let div = b.try_fold_px()?;
                if div.abs() > 1e-6 {
                    Some(a.try_fold_px()? / div)
                } else {
                    Some(0.0)
                }
            }
            MathExpr::Min(list) => {
                let mut m = f32::INFINITY;
                for item in list {
                    m = m.min(item.try_fold_px()?);
                }
                Some(if m.is_infinite() { 0.0 } else { m })
            }
            MathExpr::Max(list) => {
                let mut m = f32::NEG_INFINITY;
                for item in list {
                    m = m.max(item.try_fold_px()?);
                }
                Some(if m.is_infinite() { 0.0 } else { m })
            }
            MathExpr::Clamp { min, val, max } => {
                let min_v = min.try_fold_px()?;
                let v = val.try_fold_px()?;
                let max_v = max.try_fold_px()?;
                Some(v.max(min_v).min(max_v))
            }
            MathExpr::Round {
                strategy,
                val,
                step,
            } => {
                let v = val.try_fold_px()?;
                let s = step.try_fold_px()?;
                if s.abs() < 1e-6 {
                    Some(v)
                } else {
                    Some(match strategy {
                        super::expr::RoundStrategy::Nearest => (v / s).round() * s,
                        super::expr::RoundStrategy::Up => (v / s).ceil() * s,
                        super::expr::RoundStrategy::Down => (v / s).floor() * s,
                        super::expr::RoundStrategy::ToZero => (v / s).trunc() * s,
                    })
                }
            }
            MathExpr::Mod(a, b) => {
                let av = a.try_fold_px()?;
                let bv = b.try_fold_px()?;
                if bv.abs() < 1e-6 {
                    Some(0.0)
                } else {
                    Some(av - bv * (av / bv).floor())
                }
            }
            MathExpr::Rem(a, b) => {
                let av = a.try_fold_px()?;
                let bv = b.try_fold_px()?;
                if bv.abs() < 1e-6 {
                    Some(0.0)
                } else {
                    Some(av % bv)
                }
            }
            MathExpr::Abs(a) => Some(a.try_fold_px()?.abs()),
            MathExpr::Sign(a) => {
                let v = a.try_fold_px()?;
                Some(if v > 0.0 {
                    1.0
                } else if v < 0.0 {
                    -1.0
                } else {
                    0.0
                })
            }
            MathExpr::Sin(a) => Some(a.try_fold_px()?.sin()),
            MathExpr::Cos(a) => Some(a.try_fold_px()?.cos()),
            MathExpr::Tan(a) => Some(a.try_fold_px()?.tan()),
            MathExpr::Asin(a) => {
                let v = a.try_fold_px()?;
                Some(if (-1.0..=1.0).contains(&v) {
                    v.asin()
                } else {
                    0.0
                })
            }
            MathExpr::Acos(a) => {
                let v = a.try_fold_px()?;
                Some(if (-1.0..=1.0).contains(&v) {
                    v.acos()
                } else {
                    0.0
                })
            }
            MathExpr::Atan(a) => Some(a.try_fold_px()?.atan()),
            MathExpr::Atan2(y, x) => Some(y.try_fold_px()?.atan2(x.try_fold_px()?)),
            MathExpr::Sqrt(a) => {
                let v = a.try_fold_px()?;
                Some(if v >= 0.0 { v.sqrt() } else { 0.0 })
            }
            MathExpr::Pow(base, exp) => Some(base.try_fold_px()?.powf(exp.try_fold_px()?)),
            MathExpr::Hypot(list) => {
                let mut sum_sq = 0.0f32;
                for item in list {
                    let v = item.try_fold_px()?;
                    sum_sq += v * v;
                }
                Some(sum_sq.sqrt())
            }
        }
    }

    /// Converts this expression into a linear CalcLength if it only contains linear additions/subtractions.
    pub fn try_into_calc_length(&self) -> Option<CalcLength> {
        let mut calc = CalcLength::default();
        if self.accumulate_calc(&mut calc, 1.0) {
            Some(calc)
        } else {
            None
        }
    }

    fn accumulate_calc(&self, calc: &mut CalcLength, sign: f32) -> bool {
        match self {
            MathExpr::Length(len) => match len {
                Length::Px(px) => {
                    calc.px += px * sign;
                    true
                }
                Length::Percent(pct) => {
                    calc.percent += pct * sign;
                    calc.has_percent = true;
                    true
                }
                Length::Em(em) => {
                    calc.em += em * sign;
                    true
                }
                Length::Ex(ex) => {
                    calc.ex += ex * sign;
                    true
                }
                Length::Rem(rem) => {
                    calc.rem += rem * sign;
                    true
                }
                Length::Vw(vw) => {
                    calc.vw += vw * sign;
                    true
                }
                Length::Vh(vh) => {
                    calc.vh += vh * sign;
                    true
                }
                Length::Calc(c) => {
                    calc.px += c.px * sign;
                    calc.percent += c.percent * sign;
                    calc.has_percent |= c.has_percent;
                    calc.em += c.em * sign;
                    calc.ex += c.ex * sign;
                    calc.rem += c.rem * sign;
                    calc.vw += c.vw * sign;
                    calc.vh += c.vh * sign;
                    true
                }
                _ => false,
            },
            MathExpr::Number(n) if *n == 0.0 => true,
            MathExpr::Add(a, b) => a.accumulate_calc(calc, sign) && b.accumulate_calc(calc, sign),
            MathExpr::Sub(a, b) => a.accumulate_calc(calc, sign) && b.accumulate_calc(calc, -sign),
            MathExpr::Mul(a, b) => {
                if let MathExpr::Number(n) = **b {
                    a.accumulate_calc(calc, sign * n)
                } else if let MathExpr::Number(n) = **a {
                    b.accumulate_calc(calc, sign * n)
                } else {
                    false
                }
            }
            MathExpr::Div(a, b) => {
                if let MathExpr::Number(n) = **b {
                    if n.abs() > 1e-6 {
                        a.accumulate_calc(calc, sign / n)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
