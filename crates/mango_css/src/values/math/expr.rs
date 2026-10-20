//! Mathematical expression trees and rounding strategies.

use super::calc::CalcLength;
use crate::values::length::Length;

/// An identifier for an interned mathematical expression in CSS math functions (`calc()`, `min()`, `max()`, `clamp()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MathId(pub u32);

/// Rounding strategy for CSS `round()` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoundStrategy {
    #[default]
    Nearest,
    Up,
    Down,
    ToZero,
}

/// An algebraic or mathematical expression tree in CSS math functions.
#[derive(Debug, Clone, PartialEq)]
pub enum MathExpr {
    /// A leaf length (Px, Em, Rem, Percent, Vw, Vh, Vmin, Vmax, etc.).
    Length(Length),
    /// A unitless scalar number (e.g. `2` in `calc(2 * 10px)`).
    Number(f32),
    /// Binary addition: `a + b`.
    Add(Box<MathExpr>, Box<MathExpr>),
    /// Binary subtraction: `a - b`.
    Sub(Box<MathExpr>, Box<MathExpr>),
    /// Binary multiplication: `a * b`.
    Mul(Box<MathExpr>, Box<MathExpr>),
    /// Binary division: `a / b`.
    Div(Box<MathExpr>, Box<MathExpr>),
    /// Variadic minimum: `min(a, b, ...)`.
    Min(Vec<MathExpr>),
    /// Variadic maximum: `max(a, b, ...)`.
    Max(Vec<MathExpr>),
    /// Clamping function: `clamp(min, val, max)`.
    Clamp {
        min: Box<MathExpr>,
        val: Box<MathExpr>,
        max: Box<MathExpr>,
    },
    /// Rounding: `round(strategy?, val, step)`.
    Round {
        strategy: RoundStrategy,
        val: Box<MathExpr>,
        step: Box<MathExpr>,
    },
    /// Modulo: `mod(a, b)` (result has same sign as divisor).
    Mod(Box<MathExpr>, Box<MathExpr>),
    /// Remainder: `rem(a, b)` (result has same sign as dividend).
    Rem(Box<MathExpr>, Box<MathExpr>),
    /// Absolute value: `abs(a)`.
    Abs(Box<MathExpr>),
    /// Sign function: `sign(a)`.
    Sign(Box<MathExpr>),
    /// Sine: `sin(a)`.
    Sin(Box<MathExpr>),
    /// Cosine: `cos(a)`.
    Cos(Box<MathExpr>),
    /// Tangent: `tan(a)`.
    Tan(Box<MathExpr>),
    /// Inverse sine: `asin(a)`.
    Asin(Box<MathExpr>),
    /// Inverse cosine: `acos(a)`.
    Acos(Box<MathExpr>),
    /// Inverse tangent: `atan(a)`.
    Atan(Box<MathExpr>),
    /// Two-argument inverse tangent: `atan2(y, x)`.
    Atan2(Box<MathExpr>, Box<MathExpr>),
    /// Square root: `sqrt(a)`.
    Sqrt(Box<MathExpr>),
    /// Power: `pow(base, exp)`.
    Pow(Box<MathExpr>, Box<MathExpr>),
    /// Hypotenuse: `hypot(a, b, ...)`.
    Hypot(Vec<MathExpr>),
    /// A CSS variable reference inside math expressions: `var(--name, fallback)`.
    Var {
        name: String,
        fallback: Option<Box<MathExpr>>,
    },
}

impl Eq for MathExpr {}

impl std::hash::Hash for MathExpr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            MathExpr::Length(len) => {
                0u8.hash(state);
                len.hash(state);
            }
            MathExpr::Number(n) => {
                1u8.hash(state);
                n.to_bits().hash(state);
            }
            MathExpr::Add(a, b) => {
                2u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            MathExpr::Sub(a, b) => {
                3u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            MathExpr::Mul(a, b) => {
                4u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            MathExpr::Div(a, b) => {
                5u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            MathExpr::Min(list) => {
                6u8.hash(state);
                list.len().hash(state);
                for e in list {
                    e.hash(state);
                }
            }
            MathExpr::Max(list) => {
                7u8.hash(state);
                list.len().hash(state);
                for e in list {
                    e.hash(state);
                }
            }
            MathExpr::Clamp { min, val, max } => {
                8u8.hash(state);
                min.hash(state);
                val.hash(state);
                max.hash(state);
            }
            MathExpr::Round {
                strategy,
                val,
                step,
            } => {
                9u8.hash(state);
                strategy.hash(state);
                val.hash(state);
                step.hash(state);
            }
            MathExpr::Mod(a, b) => {
                10u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            MathExpr::Rem(a, b) => {
                11u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            MathExpr::Abs(a) => {
                12u8.hash(state);
                a.hash(state);
            }
            MathExpr::Sign(a) => {
                13u8.hash(state);
                a.hash(state);
            }
            MathExpr::Sin(a) => {
                14u8.hash(state);
                a.hash(state);
            }
            MathExpr::Cos(a) => {
                15u8.hash(state);
                a.hash(state);
            }
            MathExpr::Tan(a) => {
                16u8.hash(state);
                a.hash(state);
            }
            MathExpr::Asin(a) => {
                17u8.hash(state);
                a.hash(state);
            }
            MathExpr::Acos(a) => {
                18u8.hash(state);
                a.hash(state);
            }
            MathExpr::Atan(a) => {
                19u8.hash(state);
                a.hash(state);
            }
            MathExpr::Atan2(y, x) => {
                20u8.hash(state);
                y.hash(state);
                x.hash(state);
            }
            MathExpr::Sqrt(a) => {
                21u8.hash(state);
                a.hash(state);
            }
            MathExpr::Pow(base, exp) => {
                22u8.hash(state);
                base.hash(state);
                exp.hash(state);
            }
            MathExpr::Hypot(list) => {
                23u8.hash(state);
                list.len().hash(state);
                for e in list {
                    e.hash(state);
                }
            }
            MathExpr::Var { name, fallback } => {
                24u8.hash(state);
                name.hash(state);
                fallback.hash(state);
            }
        }
    }
}

impl MathExpr {
    /// Resolves this expression to concrete pixels given contextual metrics.
    pub fn to_px_with_viewports(
        &self,
        font_size: f32,
        root_font_size: f32,
        container_size: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> f32 {
        match self {
            MathExpr::Length(len) => len.to_px_with_viewports(
                font_size,
                root_font_size,
                container_size,
                viewport_width,
                viewport_height,
            ),
            MathExpr::Number(n) => *n,
            MathExpr::Add(a, b) => {
                a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                ) + b.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                )
            }
            MathExpr::Sub(a, b) => {
                a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                ) - b.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                )
            }
            MathExpr::Mul(a, b) => {
                a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                ) * b.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                )
            }
            MathExpr::Div(a, b) => {
                let divisor = b.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                if divisor.abs() > 1e-6 {
                    a.to_px_with_viewports(
                        font_size,
                        root_font_size,
                        container_size,
                        viewport_width,
                        viewport_height,
                    ) / divisor
                } else {
                    0.0
                }
            }
            MathExpr::Min(list) => {
                let mut min_val = f32::INFINITY;
                for item in list {
                    let v = item.to_px_with_viewports(
                        font_size,
                        root_font_size,
                        container_size,
                        viewport_width,
                        viewport_height,
                    );
                    if v < min_val {
                        min_val = v;
                    }
                }
                if min_val.is_infinite() { 0.0 } else { min_val }
            }
            MathExpr::Max(list) => {
                let mut max_val = f32::NEG_INFINITY;
                for item in list {
                    let v = item.to_px_with_viewports(
                        font_size,
                        root_font_size,
                        container_size,
                        viewport_width,
                        viewport_height,
                    );
                    if v > max_val {
                        max_val = v;
                    }
                }
                if max_val.is_infinite() { 0.0 } else { max_val }
            }
            MathExpr::Clamp { min, val, max } => {
                let min_v = min.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let v = val.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let max_v = max.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.max(min_v).min(max_v)
            }
            MathExpr::Round {
                strategy,
                val,
                step,
            } => {
                let v = val.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let s = step.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                if s.abs() < 1e-6 {
                    v
                } else {
                    match strategy {
                        RoundStrategy::Nearest => (v / s).round() * s,
                        RoundStrategy::Up => (v / s).ceil() * s,
                        RoundStrategy::Down => (v / s).floor() * s,
                        RoundStrategy::ToZero => (v / s).trunc() * s,
                    }
                }
            }
            MathExpr::Mod(a, b) => {
                let av = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let bv = b.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                if bv.abs() < 1e-6 {
                    0.0
                } else {
                    av - bv * (av / bv).floor()
                }
            }
            MathExpr::Rem(a, b) => {
                let av = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let bv = b.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                if bv.abs() < 1e-6 { 0.0 } else { av % bv }
            }
            MathExpr::Abs(a) => a
                .to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                )
                .abs(),
            MathExpr::Sign(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                if v > 0.0 {
                    1.0
                } else if v < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
            MathExpr::Sin(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.sin()
            }
            MathExpr::Cos(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.cos()
            }
            MathExpr::Tan(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.tan()
            }
            MathExpr::Asin(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.asin()
            }
            MathExpr::Acos(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.acos()
            }
            MathExpr::Atan(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                v.atan()
            }
            MathExpr::Atan2(y, x) => {
                let yv = y.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let xv = x.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                yv.atan2(xv)
            }
            MathExpr::Sqrt(a) => {
                let v = a.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                if v >= 0.0 { v.sqrt() } else { 0.0 }
            }
            MathExpr::Pow(base, exp) => {
                let b = base.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                let e = exp.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                );
                b.powf(e)
            }
            MathExpr::Hypot(list) => {
                let mut sum_sq = 0.0f32;
                for item in list {
                    let v = item.to_px_with_viewports(
                        font_size,
                        root_font_size,
                        container_size,
                        viewport_width,
                        viewport_height,
                    );
                    sum_sq += v * v;
                }
                sum_sq.sqrt()
            }
            MathExpr::Var { fallback, .. } => fallback.as_ref().map_or(0.0, |fb| {
                fb.to_px_with_viewports(
                    font_size,
                    root_font_size,
                    container_size,
                    viewport_width,
                    viewport_height,
                )
            }),
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
                        RoundStrategy::Nearest => (v / s).round() * s,
                        RoundStrategy::Up => (v / s).ceil() * s,
                        RoundStrategy::Down => (v / s).floor() * s,
                        RoundStrategy::ToZero => (v / s).trunc() * s,
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
            MathExpr::Asin(a) => Some(a.try_fold_px()?.asin()),
            MathExpr::Acos(a) => Some(a.try_fold_px()?.acos()),
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
                    true
                }
                Length::Em(em) => {
                    calc.em += em * sign;
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
                    calc.em += c.em * sign;
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
