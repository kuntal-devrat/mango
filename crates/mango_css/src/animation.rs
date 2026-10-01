//! CSS transitions, `@keyframes` animations, and value interpolation.
//!
//! This module implements the runtime half of CSS animation:
//!
//! - [`interpolate_value`] / [`interpolate_style`] — numeric/colour interpolation
//!   between two CSS values or two computed styles.
//! - [`Keyframes`] — a parsed `@keyframes` rule with resolved offsets, sampled at
//!   an arbitrary progress value.
//! - [`AnimationEngine`] — tracks running `animation-*` declarations per DOM node,
//!   advances them over time, and produces the style overrides to apply.
//! - [`TransitionEngine`] — diffs consecutive computed styles and drives
//!   `transition-*` declarations, emitting `transitionend` events.
//!
//! Both engines are pure state machines: the browser chrome owns the clock and
//! calls [`AnimationEngine::advance`] / [`TransitionEngine::advance`] each frame.
//!
//! ## Example
//!
//! ```ignore
//! let mut engine = AnimationEngine::new();
//! engine.set_keyframes(collect_keyframes(&[&sheet]));
//! engine.start(node_id, &style.animations[0]);
//! engine.advance(16.0);              // ~60fps tick
//! engine.apply(node_id, &mut style); // style now reflects the animation
//! ```

use std::collections::HashMap;

use mango_core::Color;
use mango_html::dom::NodeId;

use crate::computed::{apply_cascaded_properties, ComputedStyle};
use crate::parser::{KeyframesRule, Stylesheet, Rule};
use crate::properties::Declaration;
use crate::values::{
    Animation, AnimationDirection, AnimationFillMode, AnimationIterationCount, AnimationPlayState,
    FilterFunction, Gradient, Length, TimingFunction, Transform, TransformFunction, Value,
};

/// Linearly interpolates two scalars.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Interpolates two colors in premultiplied sRGB space.
pub fn interpolate_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let blend = |x: u8, y: u8| -> u8 { lerp(x as f32, y as f32, t).round().clamp(0.0, 255.0) as u8 };
    Color::rgba(
        blend(a.r, b.r),
        blend(a.g, b.g),
        blend(a.b, b.b),
        blend(a.a, b.a),
    )
}

/// Interpolates two CSS lengths, falling back to a discrete switch for
/// incompatible units (e.g. `auto` ↔ `10px`).
pub fn interpolate_length(a: Length, b: Length, t: f32) -> Length {
    use Length::*;
    match (a, b) {
        (Px(x), Px(y)) => Px(lerp(x, y, t)),
        (Em(x), Em(y)) => Em(lerp(x, y, t)),
        (Rem(x), Rem(y)) => Rem(lerp(x, y, t)),
        (Percent(x), Percent(y)) => Percent(lerp(x, y, t)),
        (Vw(x), Vw(y)) => Vw(lerp(x, y, t)),
        (Vh(x), Vh(y)) => Vh(lerp(x, y, t)),
        (Calc(x), Calc(y)) => Calc(crate::values::CalcLength {
            px: lerp(x.px, y.px, t),
            percent: lerp(x.percent, y.percent, t),
            em: lerp(x.em, y.em, t),
            rem: lerp(x.rem, y.rem, t),
            vw: lerp(x.vw, y.vw, t),
            vh: lerp(x.vh, y.vh, t),
        }),
        // Mixed unit families: snap at the halfway point (CSS discrete interpolation).
        (x, y) => {
            if t < 0.5 {
                x
            } else {
                y
            }
        }
    }
}

/// Interpolates two transform lists function-by-function when their shapes match.
pub fn interpolate_transform(a: &Transform, b: &Transform, t: f32) -> Transform {
    if a.is_identity() && b.is_identity() {
        return Transform::default();
    }
    if a.0.len() != b.0.len() {
        return if t < 0.5 { a.clone() } else { b.clone() };
    }
    let mut out = Vec::with_capacity(a.0.len());
    for (fa, fb) in a.0.iter().zip(b.0.iter()) {
        let f = match (fa.clone(), fb.clone()) {
            (TransformFunction::Translate(x1, y1), TransformFunction::Translate(x2, y2)) => {
                TransformFunction::Translate(lerp(x1, x2, t), lerp(y1, y2, t))
            }
            (TransformFunction::TranslateX(x1), TransformFunction::TranslateX(x2)) => {
                TransformFunction::TranslateX(lerp(x1, x2, t))
            }
            (TransformFunction::TranslateY(y1), TransformFunction::TranslateY(y2)) => {
                TransformFunction::TranslateY(lerp(y1, y2, t))
            }
            (TransformFunction::Rotate(r1), TransformFunction::Rotate(r2)) => {
                TransformFunction::Rotate(lerp(r1, r2, t))
            }
            (TransformFunction::Scale(x1, y1), TransformFunction::Scale(x2, y2)) => {
                TransformFunction::Scale(lerp(x1, x2, t), lerp(y1, y2, t))
            }
            (TransformFunction::ScaleX(x1), TransformFunction::ScaleX(x2)) => {
                TransformFunction::ScaleX(lerp(x1, x2, t))
            }
            (TransformFunction::ScaleY(y1), TransformFunction::ScaleY(y2)) => {
                TransformFunction::ScaleY(lerp(y1, y2, t))
            }
            (TransformFunction::Skew(x1, y1), TransformFunction::Skew(x2, y2)) => {
                TransformFunction::Skew(lerp(x1, x2, t), lerp(y1, y2, t))
            }
            (
                TransformFunction::Matrix(a1, b1, c1, d1, e1, f1),
                TransformFunction::Matrix(a2, b2, c2, d2, e2, f2),
            ) => TransformFunction::Matrix(
                lerp(a1, a2, t),
                lerp(b1, b2, t),
                lerp(c1, c2, t),
                lerp(d1, d2, t),
                lerp(e1, e2, t),
                lerp(f1, f2, t),
            ),
            (TransformFunction::Perspective(p1), TransformFunction::Perspective(p2)) => {
                TransformFunction::Perspective(lerp(p1, p2, t))
            }
            (TransformFunction::TranslateZ(z1), TransformFunction::TranslateZ(z2)) => {
                TransformFunction::TranslateZ(lerp(z1, z2, t))
            }
            (TransformFunction::Translate3d(x1, y1, z1), TransformFunction::Translate3d(x2, y2, z2)) => {
                TransformFunction::Translate3d(lerp(x1, x2, t), lerp(y1, y2, t), lerp(z1, z2, t))
            }
            (TransformFunction::RotateX(r1), TransformFunction::RotateX(r2)) => {
                TransformFunction::RotateX(lerp(r1, r2, t))
            }
            (TransformFunction::RotateY(r1), TransformFunction::RotateY(r2)) => {
                TransformFunction::RotateY(lerp(r1, r2, t))
            }
            (TransformFunction::RotateZ(r1), TransformFunction::RotateZ(r2)) => {
                TransformFunction::RotateZ(lerp(r1, r2, t))
            }
            (
                TransformFunction::Rotate3d(x1, y1, z1, a1),
                TransformFunction::Rotate3d(x2, y2, z2, a2),
            ) => TransformFunction::Rotate3d(
                lerp(x1, x2, t),
                lerp(y1, y2, t),
                lerp(z1, z2, t),
                lerp(a1, a2, t),
            ),
            (TransformFunction::ScaleZ(z1), TransformFunction::ScaleZ(z2)) => {
                TransformFunction::ScaleZ(lerp(z1, z2, t))
            }
            (TransformFunction::Scale3d(x1, y1, z1), TransformFunction::Scale3d(x2, y2, z2)) => {
                TransformFunction::Scale3d(lerp(x1, x2, t), lerp(y1, y2, t), lerp(z1, z2, t))
            }
            (TransformFunction::Matrix3d(m1), TransformFunction::Matrix3d(m2)) => {
                let mut m = [0.0; 16];
                for i in 0..16 {
                    m[i] = lerp(m1[i], m2[i], t);
                }
                TransformFunction::Matrix3d(m)
            }
            (x, y) => {
                if t < 0.5 {
                    x
                } else {
                    y
                }
            }
        };
        out.push(f);
    }
    Transform(out)
}

fn interpolate_filter_list(a: &[FilterFunction], b: &[FilterFunction], t: f32) -> Vec<FilterFunction> {
    if a.len() != b.len() {
        return if t < 0.5 { a.to_vec() } else { b.to_vec() };
    }
    a.iter()
        .zip(b.iter())
        .map(|(fa, fb)| match (fa, fb) {
            (FilterFunction::Blur(x), FilterFunction::Blur(y)) => FilterFunction::Blur(lerp(*x, *y, t)),
            (FilterFunction::Brightness(x), FilterFunction::Brightness(y)) => {
                FilterFunction::Brightness(lerp(*x, *y, t))
            }
            (FilterFunction::Contrast(x), FilterFunction::Contrast(y)) => {
                FilterFunction::Contrast(lerp(*x, *y, t))
            }
            (FilterFunction::Grayscale(x), FilterFunction::Grayscale(y)) => {
                FilterFunction::Grayscale(lerp(*x, *y, t))
            }
            (FilterFunction::Opacity(x), FilterFunction::Opacity(y)) => {
                FilterFunction::Opacity(lerp(*x, *y, t))
            }
            (FilterFunction::Saturate(x), FilterFunction::Saturate(y)) => {
                FilterFunction::Saturate(lerp(*x, *y, t))
            }
            (FilterFunction::Sepia(x), FilterFunction::Sepia(y)) => FilterFunction::Sepia(lerp(*x, *y, t)),
            (FilterFunction::HueRotate(x), FilterFunction::HueRotate(y)) => {
                FilterFunction::HueRotate(lerp(*x, *y, t))
            }
            (FilterFunction::Invert(x), FilterFunction::Invert(y)) => FilterFunction::Invert(lerp(*x, *y, t)),
            (
                FilterFunction::DropShadow { offset_x: x1, offset_y: y1, blur: b1, color: c1 },
                FilterFunction::DropShadow { offset_x: x2, offset_y: y2, blur: b2, color: c2 },
            ) => FilterFunction::DropShadow {
                offset_x: lerp(*x1, *x2, t),
                offset_y: lerp(*y1, *y2, t),
                blur: lerp(*b1, *b2, t),
                color: interpolate_color(*c1, *c2, t),
            },
            (x, _) => x.clone(),
        })
        .collect()
}

fn interpolate_clip_path(a: &crate::values::ClipPath, b: &crate::values::ClipPath, t: f32) -> crate::values::ClipPath {
    use crate::values::ClipPath;
    match (a, b) {
        (
            ClipPath::Circle { radius: r1, center_x: cx1, center_y: cy1 },
            ClipPath::Circle { radius: r2, center_x: cx2, center_y: cy2 },
        ) => ClipPath::Circle {
            radius: interpolate_length(*r1, *r2, t),
            center_x: interpolate_length(*cx1, *cx2, t),
            center_y: interpolate_length(*cy1, *cy2, t),
        },
        (
            ClipPath::Ellipse { radius_x: rx1, radius_y: ry1, center_x: cx1, center_y: cy1 },
            ClipPath::Ellipse { radius_x: rx2, radius_y: ry2, center_x: cx2, center_y: cy2 },
        ) => ClipPath::Ellipse {
            radius_x: interpolate_length(*rx1, *rx2, t),
            radius_y: interpolate_length(*ry1, *ry2, t),
            center_x: interpolate_length(*cx1, *cx2, t),
            center_y: interpolate_length(*cy1, *cy2, t),
        },
        (
            ClipPath::Inset { top: t1, right: r1, bottom: b1, left: l1, round: rnd1 },
            ClipPath::Inset { top: t2, right: r2, bottom: b2, left: l2, round: rnd2 },
        ) => {
            let round = match (rnd1, rnd2) {
                (Some(a), Some(b)) => Some([
                    interpolate_length(a[0], b[0], t),
                    interpolate_length(a[1], b[1], t),
                    interpolate_length(a[2], b[2], t),
                    interpolate_length(a[3], b[3], t),
                ]),
                (Some(a), None) if t < 0.5 => Some(*a),
                (None, Some(b)) if t >= 0.5 => Some(*b),
                _ => None,
            };
            ClipPath::Inset {
                top: interpolate_length(*t1, *t2, t),
                right: interpolate_length(*r1, *r2, t),
                bottom: interpolate_length(*b1, *b2, t),
                left: interpolate_length(*l1, *l2, t),
                round,
            }
        }
        (ClipPath::Polygon(p1), ClipPath::Polygon(p2)) if p1.len() == p2.len() => {
            let pts = p1.iter().zip(p2.iter()).map(|((x1, y1), (x2, y2))| {
                (interpolate_length(*x1, *x2, t), interpolate_length(*y1, *y2, t))
            }).collect();
            ClipPath::Polygon(pts)
        }
        _ => if t < 0.5 { a.clone() } else { b.clone() },
    }
}

/// Interpolates two raw CSS values, returning `None` for non-interpolable pairs.
pub fn interpolate_value(a: &Value, b: &Value, t: f32) -> Option<Value> {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Value::Length(x), Value::Length(y)) => Some(Value::Length(interpolate_length(*x, *y, t))),
        (Value::Number(x), Value::Number(y)) => Some(Value::Number(lerp(*x, *y, t))),
        // Unitless numbers and bare `0` lengths (parsed as `0px`) describe the same
        // quantity for properties like `opacity`; normalize to a number.
        (Value::Length(Length::Px(x)), Value::Number(y)) => Some(Value::Number(lerp(*x, *y, t))),
        (Value::Number(x), Value::Length(Length::Px(y))) => Some(Value::Number(lerp(*x, *y, t))),
        (Value::Percentage(x), Value::Percentage(y)) => Some(Value::Percentage(lerp(*x, *y, t))),
        (Value::Angle(x), Value::Angle(y)) => Some(Value::Angle(lerp(*x, *y, t))),
        (Value::Time(x), Value::Time(y)) => Some(Value::Time(lerp(*x, *y, t))),
        (Value::Color(x), Value::Color(y)) => Some(Value::Color(interpolate_color(*x, *y, t))),
        (Value::CurrentColor, Value::Color(y)) => Some(Value::Color(*y)),
        (Value::Color(x), Value::CurrentColor) => Some(Value::Color(*x)),
        (Value::Transform(x), Value::Transform(y)) => {
            Some(Value::Transform(interpolate_transform(x, y, t)))
        }
        (Value::TextShadow(x), Value::TextShadow(y)) => Some(Value::TextShadow(crate::values::TextShadow {
            offset_x: lerp(x.offset_x, y.offset_x, t),
            offset_y: lerp(x.offset_y, y.offset_y, t),
            blur_radius: lerp(x.blur_radius, y.blur_radius, t),
            color: interpolate_color(x.color, y.color, t),
        })),
        (Value::BoxShadow(x), Value::BoxShadow(y)) => Some(Value::BoxShadow(crate::values::BoxShadow {
            offset_x: lerp(x.offset_x, y.offset_x, t),
            offset_y: lerp(x.offset_y, y.offset_y, t),
            blur_radius: lerp(x.blur_radius, y.blur_radius, t),
            spread_radius: lerp(x.spread_radius, y.spread_radius, t),
            color: interpolate_color(x.color, y.color, t),
            inset: x.inset,
        })),
        (Value::Filter(x), Value::Filter(y)) => Some(Value::Filter(interpolate_filter_list(x, y, t))),
        (Value::ClipPath(x), Value::ClipPath(y)) => Some(Value::ClipPath(interpolate_clip_path(x, y, t))),
        (Value::List(x), Value::List(y)) if x.len() == y.len() => {
            let mut out = Vec::with_capacity(x.len());
            for (xi, yi) in x.iter().zip(y.iter()) {
                out.push(interpolate_value(xi, yi, t)?);
            }
            Some(Value::List(out))
        }
        // Discrete properties switch at the midpoint.
        (x, y) => {
            if t < 0.5 {
                Some(x.clone())
            } else {
                Some(y.clone())
            }
        }
    }
}

/// Interpolates two computed styles, producing the blended style at `t`.
///
/// Only animatable properties are interpolated; everything else is taken from
/// `to` once the transition passes its midpoint.
pub fn interpolate_style(from: &ComputedStyle, to: &ComputedStyle, t: f32) -> ComputedStyle {
    let mut style = from.clone();

    style.opacity = lerp(from.opacity, to.opacity, t);
    style.color = interpolate_color(from.color, to.color, t);
    style.background_color = interpolate_color(from.background_color, to.background_color, t);
    style.border_top_color = interpolate_color(from.border_top_color, to.border_top_color, t);
    style.border_right_color = interpolate_color(from.border_right_color, to.border_right_color, t);
    style.border_bottom_color = interpolate_color(from.border_bottom_color, to.border_bottom_color, t);
    style.border_left_color = interpolate_color(from.border_left_color, to.border_left_color, t);
    style.outline_color = interpolate_color(from.outline_color, to.outline_color, t);

    style.font_size = lerp(from.font_size, to.font_size, t);
    style.letter_spacing = interpolate_length(from.letter_spacing, to.letter_spacing, t);
    style.word_spacing = interpolate_length(from.word_spacing, to.word_spacing, t);
    style.text_indent = interpolate_length(from.text_indent, to.text_indent, t);

    style.width = interpolate_length(from.width, to.width, t);
    style.height = interpolate_length(from.height, to.height, t);
    style.min_width = interpolate_length(from.min_width, to.min_width, t);
    style.max_width = interpolate_length(from.max_width, to.max_width, t);
    style.min_height = interpolate_length(from.min_height, to.min_height, t);
    style.max_height = interpolate_length(from.max_height, to.max_height, t);

    style.margin_top = interpolate_length(from.margin_top, to.margin_top, t);
    style.margin_right = interpolate_length(from.margin_right, to.margin_right, t);
    style.margin_bottom = interpolate_length(from.margin_bottom, to.margin_bottom, t);
    style.margin_left = interpolate_length(from.margin_left, to.margin_left, t);

    style.padding_top = interpolate_length(from.padding_top, to.padding_top, t);
    style.padding_right = interpolate_length(from.padding_right, to.padding_right, t);
    style.padding_bottom = interpolate_length(from.padding_bottom, to.padding_bottom, t);
    style.padding_left = interpolate_length(from.padding_left, to.padding_left, t);

    style.top = interpolate_length(from.top, to.top, t);
    style.right = interpolate_length(from.right, to.right, t);
    style.bottom = interpolate_length(from.bottom, to.bottom, t);
    style.left = interpolate_length(from.left, to.left, t);

    style.border_top_width = lerp(from.border_top_width, to.border_top_width, t);
    style.border_right_width = lerp(from.border_right_width, to.border_right_width, t);
    style.border_bottom_width = lerp(from.border_bottom_width, to.border_bottom_width, t);
    style.border_left_width = lerp(from.border_left_width, to.border_left_width, t);
    style.outline_width = lerp(from.outline_width, to.outline_width, t);
    style.outline_offset = lerp(from.outline_offset, to.outline_offset, t);

    style.border_top_left_radius = lerp(from.border_top_left_radius, to.border_top_left_radius, t);
    style.border_top_right_radius = lerp(from.border_top_right_radius, to.border_top_right_radius, t);
    style.border_bottom_right_radius =
        lerp(from.border_bottom_right_radius, to.border_bottom_right_radius, t);
    style.border_bottom_left_radius =
        lerp(from.border_bottom_left_radius, to.border_bottom_left_radius, t);

    style.row_gap = interpolate_length(from.row_gap, to.row_gap, t);
    style.column_gap = interpolate_length(from.column_gap, to.column_gap, t);
    style.flex_basis = interpolate_length(from.flex_basis, to.flex_basis, t);
    style.flex_grow = lerp(from.flex_grow, to.flex_grow, t);
    style.flex_shrink = lerp(from.flex_shrink, to.flex_shrink, t);

    style.transform = interpolate_transform(&from.transform, &to.transform, t);
    style.transform_origin_x = interpolate_length(from.transform_origin_x, to.transform_origin_x, t);
    style.transform_origin_y = interpolate_length(from.transform_origin_y, to.transform_origin_y, t);
    style.transform_origin_z = interpolate_length(from.transform_origin_z, to.transform_origin_z, t);
    style.filter = interpolate_filter_list(&from.filter, &to.filter, t);
    style.backdrop_filter = interpolate_filter_list(&from.backdrop_filter, &to.backdrop_filter, t);
    style.clip_path = interpolate_clip_path(&from.clip_path, &to.clip_path, t);

    if let Some(fs) = from.text_shadow
        && let Some(ts) = to.text_shadow
    {
        style.text_shadow = Some(crate::values::TextShadow {
            offset_x: lerp(fs.offset_x, ts.offset_x, t),
            offset_y: lerp(fs.offset_y, ts.offset_y, t),
            blur_radius: lerp(fs.blur_radius, ts.blur_radius, t),
            color: interpolate_color(fs.color, ts.color, t),
        });
    } else if t >= 0.5 {
        style.text_shadow = to.text_shadow;
    }

    if let (Some(fb), Some(tb)) = (from.box_shadow, to.box_shadow) {
        style.box_shadow = Some(crate::values::BoxShadow {
            offset_x: lerp(fb.offset_x, tb.offset_x, t),
            offset_y: lerp(fb.offset_y, tb.offset_y, t),
            blur_radius: lerp(fb.blur_radius, tb.blur_radius, t),
            spread_radius: lerp(fb.spread_radius, tb.spread_radius, t),
            color: interpolate_color(fb.color, tb.color, t),
            inset: fb.inset,
        });
    } else if t >= 0.5 {
        style.box_shadow = to.box_shadow;
    }

    if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
        (from.aspect_ratio, to.aspect_ratio, from.aspect_ratio, to.aspect_ratio)
    {
        style.aspect_ratio = Some(lerp(x1, y2, t));
        let _ = (y1, x2);
    }

    // Discrete properties flip at the midpoint.
    if t >= 0.5 {
        style.display = to.display;
        style.position = to.position;
        style.visibility = to.visibility;
        style.z_index = to.z_index;
        style.background_image = to.background_image.clone();
        style.background_gradient = to.background_gradient.clone();
        style.background_repeat = to.background_repeat;
        style.background_size = to.background_size;
        style.border_top_style = to.border_top_style;
        style.border_right_style = to.border_right_style;
        style.border_bottom_style = to.border_bottom_style;
        style.border_left_style = to.border_left_style;
        style.mix_blend_mode = to.mix_blend_mode;
        style.background_blend_mode = to.background_blend_mode;
        style.mask_image = to.mask_image.clone();
        style.mask_mode = to.mask_mode;
        style.will_change = to.will_change;
        style.will_change_properties = to.will_change_properties.clone();
    }

    style
}

/// A `@keyframes` rule with offsets normalized to the `0.0..=1.0` range.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Keyframes {
    pub name: String,
    /// Keyframes sorted by ascending offset; each holds property → declared value.
    pub frames: Vec<Keyframe>,
}

/// One stop within a [`Keyframes`] rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    /// Normalized offset in `0.0..=1.0`.
    pub offset: f32,
    /// Declared properties at this offset (longhands only, `!important` dropped).
    pub declarations: HashMap<String, Value>,
}

impl Keyframes {
    /// Builds a [`Keyframes`] value from a parsed `@keyframes` rule.
    pub fn from_rule(rule: &KeyframesRule) -> Self {
        let mut frames: Vec<Keyframe> = Vec::new();
        for kf in &rule.keyframes {
            let mut declarations: HashMap<String, Value> = HashMap::new();
            for decl in &kf.declarations {
                for expanded in Declaration::new(
                    decl.name.clone(),
                    decl.value.clone(),
                    decl.important,
                )
                .expand_shorthand()
                {
                    declarations.insert(expanded.name.clone(), expanded.value.clone());
                }
            }
            for offset_pct in &kf.offsets {
                frames.push(Keyframe {
                    offset: (offset_pct / 100.0).clamp(0.0, 1.0),
                    declarations: declarations.clone(),
                });
            }
        }
        frames.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap_or(std::cmp::Ordering::Equal));
        Self {
            name: rule.name.clone(),
            frames,
        }
    }

    /// Samples this rule at `progress` (0.0..=1.0), returning interpolated
    /// declared values keyed by property name.
    pub fn sample(&self, progress: f32) -> HashMap<String, Value> {
        let progress = progress.clamp(0.0, 1.0);
        if self.frames.is_empty() {
            return HashMap::new();
        }

        let before = self
            .frames
            .iter()
            .filter(|f| f.offset <= progress)
            .next_back()
            .unwrap_or(&self.frames[0]);
        let after = self
            .frames
            .iter()
            .find(|f| f.offset >= progress)
            .unwrap_or_else(|| self.frames.last().unwrap());

        if (after.offset - before.offset).abs() < f32::EPSILON {
            return before.declarations.clone();
        }
        let span = after.offset - before.offset;
        let t = ((progress - before.offset) / span).clamp(0.0, 1.0);

        let mut out: HashMap<String, Value> = HashMap::new();
        for (prop, a) in &before.declarations {
            match after.declarations.get(prop) {
                Some(b) => {
                    if let Some(v) = interpolate_value(a, b, t) {
                        out.insert(prop.clone(), v);
                    } else if t < 0.5 {
                        out.insert(prop.clone(), a.clone());
                    } else {
                        out.insert(prop.clone(), b.clone());
                    }
                }
                None => {
                    out.insert(prop.clone(), a.clone());
                }
            }
        }
        for (prop, b) in &after.declarations {
            if !out.contains_key(prop) {
                out.insert(prop.clone(), b.clone());
            }
        }
        out
    }
}

/// Collects every `@keyframes` rule from a set of stylesheets into a lookup map.
///
/// Later definitions win, matching the CSS cascade for `@keyframes`.
pub fn collect_keyframes<'a, I>(sheets: I) -> HashMap<String, Keyframes>
where
    I: IntoIterator<Item = &'a Stylesheet>,
{
    let mut map: HashMap<String, Keyframes> = HashMap::new();
    for sheet in sheets {
        collect_from_rules(&sheet.rules, &mut map);
    }
    map
}

fn collect_from_rules(rules: &[Rule], map: &mut HashMap<String, Keyframes>) {
    for rule in rules {
        match rule {
            Rule::Keyframes(kf) => {
                map.insert(kf.name.clone(), Keyframes::from_rule(kf));
            }
            // `@keyframes` nested inside `@media` blocks are collected by the
            // stylesheet parser when the media query matches at parse time.
            _ => {}
        }
    }
}

/// A lifecycle event produced by the animation subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationEventKind {
    Start,
    End,
    Iteration,
    Cancel,
}

/// An event emitted by [`AnimationEngine`] (maps to CSS `animationstart`/`animationend`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEvent {
    pub node: NodeId,
    pub name: String,
    pub kind: AnimationEventKind,
}

#[derive(Debug, Clone)]
struct ActiveAnimation {
    node: NodeId,
    spec: Animation,
    frames: Keyframes,
    /// Time in ms since this animation started (including any pending delay).
    elapsed_ms: f32,
    /// Completed iterations reported so far (for `animationiteration`).
    iterations_done: f32,
    started: bool,
    finished: bool,
}

/// Tracks and advances running CSS animations for the whole document.
#[derive(Debug, Default)]
pub struct AnimationEngine {
    active: Vec<ActiveAnimation>,
    keyframes: HashMap<String, Keyframes>,
    events: Vec<AnimationEvent>,
}

impl AnimationEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the known `@keyframes` definitions (call once per page load).
    pub fn set_keyframes(&mut self, keyframes: HashMap<String, Keyframes>) {
        self.keyframes = keyframes;
    }

    pub fn keyframes(&self) -> &HashMap<String, Keyframes> {
        &self.keyframes
    }

    /// Number of currently running animations.
    pub fn running_count(&self) -> usize {
        self.active.iter().filter(|a| !a.finished).count()
    }

    /// Returns true while at least one animation still needs frames.
    pub fn is_animating(&self) -> bool {
        self.active.iter().any(|a| !a.finished)
    }

    /// Starts (or restarts) the animation described by `spec` on `node`.
    ///
    /// Returns `false` when the animation has no duration or unknown name, which
    /// matches how browsers treat non-existent keyframes (nothing animates).
    pub fn start(&mut self, node: NodeId, spec: &Animation) -> bool {
        if spec.name.is_empty() || spec.name.eq_ignore_ascii_case("none") {
            return false;
        }
        let Some(frames) = self.keyframes.get(&spec.name).cloned() else {
            return false;
        };
        if frames.frames.is_empty() {
            return false;
        }
        let infinite = matches!(spec.iteration_count, AnimationIterationCount::Infinite);
        if !infinite && spec.duration_ms <= 0.0 {
            return false;
        }
        // Restart if an identical animation already exists for this node.
        self.active.retain(|a| !(a.node == node && a.spec.name == spec.name));
        self.active.push(ActiveAnimation {
            node,
            spec: spec.clone(),
            frames,
            elapsed_ms: 0.0,
            iterations_done: 0.0,
            started: false,
            finished: false,
        });
        true
    }

    /// Stops a named animation on a node, emitting `animationcancel`.
    pub fn cancel(&mut self, node: NodeId, name: &str) {
        let before = self.active.len();
        self.active.retain(|a| !(a.node == node && a.spec.name == name));
        if self.active.len() != before {
            self.events.push(AnimationEvent {
                node,
                name: name.to_string(),
                kind: AnimationEventKind::Cancel,
            });
        }
    }

    /// Drops all animation state (e.g. on navigation).
    pub fn clear(&mut self) {
        self.active.clear();
        self.events.clear();
    }

    /// Drops animation state for nodes that no longer exist / changed identity.
    pub fn retain_nodes(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        self.active.retain(|a| keep(a.node));
    }

    /// Advances every running animation by `dt_ms` milliseconds.
    pub fn advance(&mut self, dt_ms: f32) {
        if dt_ms <= 0.0 {
            return;
        }
        let mut new_events: Vec<AnimationEvent> = Vec::new();
        for anim in &mut self.active {
            if anim.finished {
                continue;
            }
            anim.elapsed_ms += dt_ms;

            // Delay phase: nothing has started yet.
            if anim.elapsed_ms < anim.spec.delay_ms {
                continue;
            }
            if !anim.started {
                anim.started = true;
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::Start,
                });
            }

            let duration = anim.spec.duration_ms.max(0.0);
            if duration <= 0.0 {
                anim.finished = true;
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::End,
                });
                continue;
            }

            let active_ms = anim.elapsed_ms - anim.spec.delay_ms;
            let total_iterations = active_ms / duration;
            if let AnimationIterationCount::Finite(count) = anim.spec.iteration_count
                && total_iterations >= count
            {
                anim.finished = true;
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::End,
                });
                continue;
            }
            // Emit an iteration event each time an iteration boundary is crossed.
            if total_iterations.floor() > anim.iterations_done {
                anim.iterations_done = total_iterations.floor();
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::Iteration,
                });
            }
        }
        self.events.extend(new_events);
    }

    /// Computes the animation progress for a single active animation, honoring
    /// direction, iteration count, and fill mode. Returns `None` when the
    /// animation contributes nothing at this moment.
    fn progress_for(anim: &ActiveAnimation) -> Option<f32> {
        let delay = anim.spec.delay_ms;
        let duration = anim.spec.duration_ms.max(0.0001);
        let elapsed = anim.elapsed_ms;

        // Before the delay elapses, only `backwards`/`both` fill modes apply.
        if elapsed < delay {
            return match anim.spec.fill_mode {
                AnimationFillMode::Backwards | AnimationFillMode::Both => Some(0.0),
                _ => None,
            };
        }
        let active_ms = elapsed - delay;
        let iterations = active_ms / duration;

        let finished = match anim.spec.iteration_count {
            AnimationIterationCount::Infinite => false,
            AnimationIterationCount::Finite(n) => iterations >= n,
        };
        if finished {
            return match anim.spec.fill_mode {
                AnimationFillMode::Forwards | AnimationFillMode::Both => {
                    // Freeze on the final iteration boundary.
                    let last = match anim.spec.iteration_count {
                        AnimationIterationCount::Finite(n) => n.max(0.0),
                        AnimationIterationCount::Infinite => iterations,
                    };
                    Some(Self::apply_direction(
                        last.fract() == 0.0 && last > 0.0,
                        last,
                        &anim.spec,
                    ))
                }
                _ => None,
            };
        }

        let complete_iterations = iterations.floor();
        let mut t = iterations.fract();
        match anim.spec.direction {
            AnimationDirection::Normal => {}
            AnimationDirection::Reverse => t = 1.0 - t,
            AnimationDirection::Alternate => {
                if (complete_iterations as i64) % 2 != 0 {
                    t = 1.0 - t;
                }
            }
            AnimationDirection::AlternateReverse => {
                if (complete_iterations as i64) % 2 == 0 {
                    t = 1.0 - t;
                }
            }
        }
        let _ = AnimationPlayState::Running;
        Some(t.clamp(0.0, 1.0))
    }

    /// Helper that resolves the final progress for fill-forwards at an exact
    /// iteration boundary (which direction-aware fraction would otherwise erase).
    fn apply_direction(_at_boundary: bool, iterations: f32, spec: &Animation) -> f32 {
        match spec.direction {
            AnimationDirection::Normal | AnimationDirection::Alternate => 1.0,
            AnimationDirection::Reverse => 0.0,
            AnimationDirection::AlternateReverse => {
                if (iterations.max(1.0) as i64) % 2 == 0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Applies all animations targeting `node` to `style`, in declaration order.
    pub fn apply(&self, node: NodeId, style: &mut ComputedStyle) {
        let ordered: Vec<&ActiveAnimation> =
            self.active.iter().filter(|a| a.node == node).collect();
        for anim in ordered {
            if anim.spec.play_state == AnimationPlayState::Paused {
                // A paused animation freezes at its current progress; the clock is
                // stopped by skipping `advance`, so just render the current frame.
            }
            let Some(progress) = Self::progress_for(anim) else {
                continue;
            };
            let eased = anim.spec.timing.sample(progress);
            let declarations = anim.frames.sample(eased);
            if declarations.is_empty() {
                continue;
            }
            apply_cascaded_properties(style, &declarations, None);
        }
    }

    /// Returns (and clears) the lifecycle events accumulated since the last call.
    pub fn drain_events(&mut self) -> Vec<AnimationEvent> {
        std::mem::take(&mut self.events)
    }
}

/// An event emitted by the transition engine (`transitionend` / `transitioncancel`).
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionEvent {
    pub node: NodeId,
    pub property: String,
    pub elapsed_ms: f32,
}

#[derive(Debug, Clone)]
struct RunningTransition {
    node: NodeId,
    property: String,
    from: ComputedStyle,
    to: ComputedStyle,
    start_ms: f32,
    duration_ms: f32,
    delay_ms: f32,
    timing: TimingFunction,
    finished: bool,
}

/// Drives `transition-*` declarations by diffing consecutive computed styles.
#[derive(Debug, Default)]
pub struct TransitionEngine {
    running: Vec<RunningTransition>,
    now_ms: f32,
    events: Vec<TransitionEvent>,
}

impl TransitionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_animating(&self) -> bool {
        self.running.iter().any(|t| !t.finished)
    }

    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Registers `new` as the target style for `node`, starting transitions for
    /// every property that changed and has a matching `transition-*` declaration.
    ///
    /// Returns `true` when at least one transition started.
    pub fn retarget(&mut self, node: NodeId, old: &ComputedStyle, new: &ComputedStyle) -> bool {
        // A transition that is itself replaced keeps its current blended value as
        // the new start point, matching browser "reversing transition" behavior.
        let current = self.current_style(node, new);
        for spec in &new.transitions {
            if spec.duration_ms <= 0.0 && spec.delay_ms <= 0.0 {
                continue;
            }
            let matches_property = |prop: &str| -> bool {
                spec.property == "all" || spec.property.eq_ignore_ascii_case(prop)
            };
            for prop in ANIMATABLE_PROPERTIES {
                if !matches_property(prop) || !property_differs(old, new, prop) {
                    continue;
                }
                let from = if self
                    .running
                    .iter()
                    .any(|t| t.node == node && t.property == *prop && !t.finished)
                {
                    current.clone()
                } else {
                    old.clone()
                };
                // Restart an in-flight transition for this property.
                self.running
                    .retain(|t| !(t.node == node && t.property == *prop && !t.finished));
                self.running.push(RunningTransition {
                    node,
                    property: (*prop).to_string(),
                    from,
                    to: new.clone(),
                    start_ms: self.now_ms,
                    duration_ms: spec.duration_ms,
                    delay_ms: spec.delay_ms,
                    timing: spec.timing,
                    finished: false,
                });
            }
        }
        self.running.iter().any(|t| t.node == node && !t.finished)
    }

    /// Advances the transition clock by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        if dt_ms <= 0.0 {
            return;
        }
        self.now_ms += dt_ms;
        let now = self.now_ms;
        let mut events: Vec<TransitionEvent> = Vec::new();
        for t in &mut self.running {
            if t.finished {
                continue;
            }
            let total = t.delay_ms + t.duration_ms;
            if now - t.start_ms >= total {
                t.finished = true;
                events.push(TransitionEvent {
                    node: t.node,
                    property: t.property.clone(),
                    elapsed_ms: t.duration_ms,
                });
            }
        }
        self.running.retain(|t| !t.finished || now - t.start_ms < t.delay_ms + t.duration_ms);
        self.events.extend(events);
    }

    fn blend_factor(&self, t: &RunningTransition, now: f32) -> f32 {
        let since = now - t.start_ms;
        if since <= t.delay_ms {
            return 0.0;
        }
        if t.duration_ms <= 0.0 {
            return 1.0;
        }
        let raw = ((since - t.delay_ms) / t.duration_ms).clamp(0.0, 1.0);
        t.timing.sample(raw)
    }

    /// Returns the interpolated style for `node` as of the current clock.
    ///
    /// `base` is the freshly computed target style; the returned style blends it
    /// with the recorded start styles of any in-flight transitions.
    pub fn current_style(&self, node: NodeId, base: &ComputedStyle) -> ComputedStyle {
        let active: Vec<&RunningTransition> = self
            .running
            .iter()
            .filter(|t| t.node == node && !t.finished)
            .collect();
        if active.is_empty() {
            return base.clone();
        }
        // Blend properties one at a time. `blend` starts from the transition's
        // "from" style so untouched properties keep their original values.
        let mut result = base.clone();
        for t in active {
            if t.to != *base {
                // The target changed since this transition started but the new
                // target has not been retargeted yet; keep animating toward `to`.
                result = interpolate_style(&t.from, &t.to, self.blend_factor(t, self.now_ms));
                let _ = &result;
            } else {
                result = interpolate_style(&t.from, &t.to, self.blend_factor(t, self.now_ms));
            }
        }
        result
    }

    /// Returns (and clears) `transitionend` events emitted since the last call.
    pub fn drain_events(&mut self) -> Vec<TransitionEvent> {
        std::mem::take(&mut self.events)
    }

    /// Drops all transition state.
    pub fn clear(&mut self) {
        self.running.clear();
        self.events.clear();
    }

    /// Drops transitions for nodes that no longer exist.
    pub fn retain_nodes(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        self.running.retain(|t| keep(t.node));
    }
}

/// Properties the transition engine is able to interpolate.
pub const ANIMATABLE_PROPERTIES: &[&str] = &[
    "opacity",
    "transform",
    "color",
    "background-color",
    "border-color",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "margin",
    "padding",
    "top",
    "right",
    "bottom",
    "left",
    "font-size",
    "letter-spacing",
    "word-spacing",
    "text-indent",
    "border-width",
    "border-radius",
    "box-shadow",
    "text-shadow",
    "filter",
    "flex-basis",
    "flex-grow",
    "flex-shrink",
    "gap",
    "outline-width",
    "outline-offset",
    "aspect-ratio",
];

/// Returns true when `prop`'s value differs between the two computed styles.
pub fn property_differs(a: &ComputedStyle, b: &ComputedStyle, prop: &str) -> bool {
    match prop {
        "opacity" => a.opacity != b.opacity,
        "transform" => a.transform != b.transform,
        "color" => a.color != b.color,
        "background-color" => a.background_color != b.background_color,
        "border-color" => {
            a.border_top_color != b.border_top_color
                || a.border_right_color != b.border_right_color
                || a.border_bottom_color != b.border_bottom_color
                || a.border_left_color != b.border_left_color
        }
        "width" => a.width != b.width,
        "height" => a.height != b.height,
        "min-width" => a.min_width != b.min_width,
        "min-height" => a.min_height != b.min_height,
        "max-width" => a.max_width != b.max_width,
        "max-height" => a.max_height != b.max_height,
        "margin" => {
            a.margin_top != b.margin_top
                || a.margin_right != b.margin_right
                || a.margin_bottom != b.margin_bottom
                || a.margin_left != b.margin_left
        }
        "padding" => {
            a.padding_top != b.padding_top
                || a.padding_right != b.padding_right
                || a.padding_bottom != b.padding_bottom
                || a.padding_left != b.padding_left
        }
        "top" => a.top != b.top,
        "right" => a.right != b.right,
        "bottom" => a.bottom != b.bottom,
        "left" => a.left != b.left,
        "font-size" => a.font_size != b.font_size,
        "letter-spacing" => a.letter_spacing != b.letter_spacing,
        "word-spacing" => a.word_spacing != b.word_spacing,
        "text-indent" => a.text_indent != b.text_indent,
        "border-width" => {
            a.border_top_width != b.border_top_width
                || a.border_right_width != b.border_right_width
                || a.border_bottom_width != b.border_bottom_width
                || a.border_left_width != b.border_left_width
        }
        "border-radius" => a.border_radius() != b.border_radius(),
        "box-shadow" => a.box_shadow != b.box_shadow,
        "text-shadow" => a.text_shadow != b.text_shadow,
        "filter" => a.filter != b.filter,
        "flex-basis" => a.flex_basis != b.flex_basis,
        "flex-grow" => a.flex_grow != b.flex_grow,
        "flex-shrink" => a.flex_shrink != b.flex_shrink,
        "gap" => a.row_gap != b.row_gap || a.column_gap != b.column_gap,
        "outline-width" => a.outline_width != b.outline_width,
        "outline-offset" => a.outline_offset != b.outline_offset,
        "aspect-ratio" => a.aspect_ratio != b.aspect_ratio,
        _ => false,
    }
}

/// Convenience: builds the display-time animation name list for a style.
pub fn animation_names(style: &ComputedStyle) -> Vec<String> {
    style.animations.iter().map(|a| a.name.clone()).collect()
}

/// Returns true when this style declares any transition that could animate.
pub fn has_transitions(style: &ComputedStyle) -> bool {
    style.transitions.iter().any(|t| t.duration_ms > 0.0 || t.delay_ms > 0.0)
}

/// Returns true when the style declares any runnable animation.
pub fn has_animations(style: &ComputedStyle) -> bool {
    style.animations.iter().any(|a| {
        !a.name.is_empty() && !a.name.eq_ignore_ascii_case("none")
    })
}

/// Interpolates two gradients (used when animating `background-image`).
pub fn interpolate_gradient(a: &Gradient, b: &Gradient, t: f32) -> Option<Gradient> {
    let (a_stops, b_stops) = (a.stops(), b.stops());
    if a_stops.len() != b_stops.len() {
        return None;
    }
    let stops = a_stops
        .iter()
        .zip(b_stops.iter())
        .map(|(sa, sb)| crate::values::ColorStop {
            color: interpolate_color(sa.color, sb.color, t),
            position: match (sa.position, sb.position) {
                (Some(x), Some(y)) => Some(lerp(x, y, t)),
                (x, y) => {
                    if t < 0.5 {
                        x
                    } else {
                        y
                    }
                }
            },
            end_position: None,
            is_current_color: sa.is_current_color || sb.is_current_color,
        })
        .collect();
    match (a, b) {
        (Gradient::Linear { angle_deg: a1, repeating, .. }, Gradient::Linear { angle_deg: a2, .. }) => {
            Some(Gradient::Linear {
                angle_deg: lerp(*a1, *a2, t),
                stops,
                repeating: *repeating,
            })
        }
        (Gradient::Radial { repeating, .. }, Gradient::Radial { .. }) => Some(Gradient::Radial {
            stops,
            repeating: *repeating,
        }),
        (Gradient::Conic { angle_deg: a1, repeating, .. }, Gradient::Conic { angle_deg: a2, .. }) => {
            Some(Gradient::Conic {
                angle_deg: lerp(*a1, *a2, t),
                stops,
                repeating: *repeating,
            })
        }
        _ => None,
    }
}

/// The set of `@keyframes` names plus the transition/animation engines, ready to
/// be driven by a render loop.
#[derive(Debug, Default)]
pub struct AnimationState {
    pub animations: AnimationEngine,
    pub transitions: TransitionEngine,
    /// Time of the last tick in milliseconds (monotonic within the browser run).
    pub last_tick_ms: f32,
}

impl AnimationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads `@keyframes` from author stylesheets.
    pub fn load_keyframes<'a, I>(&mut self, sheets: I)
    where
        I: IntoIterator<Item = &'a Stylesheet>,
    {
        self.animations.set_keyframes(collect_keyframes(sheets));
    }

    /// Starts any animations declared by `style` that are not already running.
    pub fn sync_animations(&mut self, node: NodeId, style: &ComputedStyle) -> bool {
        let mut started = false;
        for spec in &style.animations {
            if spec.name.is_empty() || spec.name.eq_ignore_ascii_case("none") {
                continue;
            }
            if self
                .animations
                .active
                .iter()
                .any(|a| a.node == node && a.spec.name == spec.name && !a.finished)
            {
                continue;
            }
            started |= self.animations.start(node, spec);
        }
        started
    }

    /// Advances both engines by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.last_tick_ms += dt_ms;
        self.animations.advance(dt_ms);
        self.transitions.advance(dt_ms);
    }

    /// Returns true when a new frame is required.
    pub fn is_animating(&self) -> bool {
        self.animations.is_animating() || self.transitions.is_animating()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_stylesheet;
    use crate::values::Transition;

    #[test]
    fn test_timing_function_sampling() {
        assert!((TimingFunction::Linear.sample(0.5) - 0.5).abs() < 1e-4);
        assert_eq!(TimingFunction::Linear.sample(0.0), 0.0);
        assert_eq!(TimingFunction::Linear.sample(1.0), 1.0);
        // ease-in starts slow: at t=0.5 it must be below the linear value
        assert!(TimingFunction::EaseIn.sample(0.5) < 0.5);
        assert!(TimingFunction::EaseOut.sample(0.5) > 0.5);
        // monotonic-ish endpoints
        assert!((TimingFunction::Ease.sample(1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_steps_timing_function() {
        let steps = TimingFunction::Steps(4, crate::values::StepPosition::JumpEnd);
        assert!((steps.sample(0.0) - 0.0).abs() < 1e-4);
        assert!((steps.sample(0.24) - 0.0).abs() < 1e-4);
        assert!((steps.sample(0.26) - 0.25).abs() < 1e-4);
        assert!((steps.sample(0.99) - 0.75).abs() < 1e-4);
    }

    #[test]
    fn test_interpolate_value_lengths_and_colors() {
        let a = Value::Length(Length::Px(0.0));
        let b = Value::Length(Length::Px(100.0));
        assert_eq!(interpolate_value(&a, &b, 0.5), Some(Value::Length(Length::Px(50.0))));

        let ca = Value::Color(Color::rgb(0, 0, 0));
        let cb = Value::Color(Color::rgb(255, 255, 255));
        let mid = interpolate_value(&ca, &cb, 0.5).unwrap();
        if let Value::Color(c) = mid {
            assert!((c.r as i32 - 128).abs() <= 1);
        } else {
            panic!("expected color");
        }
    }

    #[test]
    fn test_interpolate_transform() {
        let a = Transform(vec![TransformFunction::TranslateX(0.0)]);
        let b = Transform(vec![TransformFunction::TranslateX(100.0)]);
        let mid = interpolate_transform(&a, &b, 0.5);
        assert_eq!(mid.0[0], TransformFunction::TranslateX(50.0));
    }

    #[test]
    fn test_keyframes_sampling() {
        let sheet = parse_stylesheet(
            r#"
            @keyframes fade {
                from { opacity: 0; }
                to { opacity: 1; }
            }
            "#,
        );
        let map = collect_keyframes(std::iter::once(&sheet));
        let kf = map.get("fade").expect("keyframes parsed");
        let mid = kf.sample(0.5);
        assert_eq!(mid.get("opacity"), Some(&Value::Number(0.5)));
        let end = kf.sample(1.0);
        assert_eq!(end.get("opacity"), Some(&Value::Number(1.0)));

        // A three-stop rule interpolates between the bracketing stops.
        let sheet = parse_stylesheet(
            r#"
            @keyframes grow {
                0%   { width: 0px; }
                50%  { width: 100px; }
                100% { width: 200px; }
            }
            "#,
        );
        let map = collect_keyframes(std::iter::once(&sheet));
        let kf = map.get("grow").expect("keyframes parsed");
        assert_eq!(kf.frames.len(), 3);
        assert_eq!(
            kf.sample(0.25).get("width"),
            Some(&Value::Length(Length::Px(50.0)))
        );
        assert_eq!(
            kf.sample(0.75).get("width"),
            Some(&Value::Length(Length::Px(150.0)))
        );
    }

    #[test]
    fn test_animation_engine_advances_and_finishes() {
        let sheet = parse_stylesheet(
            r#"
            @keyframes slide {
                0% { transform: translateX(0px); }
                100% { transform: translateX(200px); }
            }
            "#,
        );
        let mut engine = AnimationEngine::new();
        engine.set_keyframes(collect_keyframes(std::iter::once(&sheet)));

        let node = NodeId::from_raw(1);
        let spec = Animation {
            name: "slide".to_string(),
            duration_ms: 100.0,
            timing: TimingFunction::Linear,
            iteration_count: AnimationIterationCount::Finite(1.0),
            fill_mode: AnimationFillMode::Both,
            ..Default::default()
        };
        assert!(engine.start(node, &spec));
        assert!(engine.is_animating());

        engine.advance(50.0);
        let mut style = ComputedStyle::default();
        engine.apply(node, &mut style);
        assert_eq!(style.transform.0[0], TransformFunction::TranslateX(100.0));

        engine.advance(60.0);
        let mut style = ComputedStyle::default();
        engine.apply(node, &mut style);
        assert_eq!(style.transform.0[0], TransformFunction::TranslateX(200.0));

        let events = engine.drain_events();
        assert!(events.iter().any(|e| e.kind == AnimationEventKind::Start));
        assert!(events.iter().any(|e| e.kind == AnimationEventKind::End));
    }

    #[test]
    fn test_transition_engine_interpolates_and_emits_end() {
        let node = NodeId::from_raw(2);
        let mut engine = TransitionEngine::new();

        let old = ComputedStyle::default();
        let mut new = ComputedStyle::default();
        new.opacity = 1.0;
        new.transitions = vec![Transition {
            property: "opacity".to_string(),
            duration_ms: 100.0,
            timing: TimingFunction::Linear,
            delay_ms: 0.0,
        }];
        let old = ComputedStyle { opacity: 0.0, ..old };

        assert!(engine.retarget(node, &old, &new));
        engine.advance(50.0);
        let blended = engine.current_style(node, &new);
        assert!((blended.opacity - 0.5).abs() < 1e-3);

        engine.advance(60.0);
        let events = engine.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].property, "opacity");
        let done = engine.current_style(node, &new);
        assert!((done.opacity - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_parse_transition_and_animation_shorthands() {
        let sheet = parse_stylesheet(
            r#"
            .btn {
                transition: opacity 0.3s ease-in 0.1s, transform 200ms linear;
                animation: spin 1s linear 0.2s infinite alternate both;
            }
            "#,
        );
        let rules = &sheet.rules;
        assert_eq!(rules.len(), 1);
        if let Rule::Style(rule) = &rules[0] {
            let mut names: Vec<String> = rule.declarations.iter().map(|d| d.name.clone()).collect();
            names.sort();
            assert!(names.contains(&"transition-property".to_string()));
            assert!(names.contains(&"transition-duration".to_string()));
            assert!(names.contains(&"transition-timing-function".to_string()));
            assert!(names.contains(&"transition-delay".to_string()));
            assert!(names.contains(&"animation-name".to_string()));
            assert!(names.contains(&"animation-iteration-count".to_string()));
            assert!(names.contains(&"animation-fill-mode".to_string()));
        } else {
            panic!("expected style rule");
        }
    }

    #[test]
    fn test_parses_gradient_variants() {
        let sheet = parse_stylesheet(
            r#"
            .a { background-image: linear-gradient(to right, red, blue); }
            .b { background-image: conic-gradient(from 45deg, red, blue); }
            .c { background-image: repeating-linear-gradient(90deg, red 0%, blue 10%); }
            "#,
        );
        let mut variants = Vec::new();
        for rule in &sheet.rules {
            if let Rule::Style(sr) = rule {
                for decl in &sr.declarations {
                    if let Value::Gradient(g) = &decl.value {
                        variants.push((**g).clone());
                    }
                }
            }
        }
        assert_eq!(variants.len(), 3);
        assert!(matches!(variants[0], Gradient::Linear { angle_deg, repeating: false, .. } if (angle_deg - 90.0).abs() < 0.01));
        assert!(matches!(variants[1], Gradient::Conic { angle_deg, repeating: false, .. } if (angle_deg - 45.0).abs() < 0.01));
        assert!(matches!(variants[2], Gradient::Linear { repeating: true, .. }));
    }
}
