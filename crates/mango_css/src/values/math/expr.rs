//! Mathematical expression trees and rounding strategies.

use crate::values::length::Length;

/// An identifier for an interned mathematical expression in CSS math functions (`calc()`, `min()`, `max()`, `clamp()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MathId(pub u32);

impl MathId {
    /// Returns `true` if the interned math expression resolves to a dimensionless `<number>` rather than a length.
    pub fn is_dimensionless_number(&self) -> bool {
        crate::values::math::get_math(*self).is_some_and(|e| e.is_dimensionless_number())
    }
}

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
}
