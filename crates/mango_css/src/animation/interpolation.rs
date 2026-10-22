//! Numeric, colour, transform, and style interpolation between CSS values.

use mango_core::Color;

use crate::computed::ComputedStyle;
use crate::values::{FilterFunction, Gradient, Length, Transform, TransformFunction, Value};

/// Linearly interpolates two scalars.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Interpolates two colors in premultiplied sRGB space.
pub fn interpolate_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let alpha_a = a.a as f32 / 255.0;
    let alpha_b = b.a as f32 / 255.0;
    let alpha_out = lerp(alpha_a, alpha_b, t);

    if alpha_out <= 0.0001 {
        return Color::TRANSPARENT;
    }

    let r_a = (a.r as f32 / 255.0) * alpha_a;
    let r_b = (b.r as f32 / 255.0) * alpha_b;
    let g_a = (a.g as f32 / 255.0) * alpha_a;
    let g_b = (b.g as f32 / 255.0) * alpha_b;
    let b_a = (a.b as f32 / 255.0) * alpha_a;
    let b_b = (b.b as f32 / 255.0) * alpha_b;

    let r_interp = lerp(r_a, r_b, t);
    let g_interp = lerp(g_a, g_b, t);
    let b_interp = lerp(b_a, b_b, t);

    let r_out = ((r_interp / alpha_out) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g_out = ((g_interp / alpha_out) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b_out = ((b_interp / alpha_out) * 255.0).round().clamp(0.0, 255.0) as u8;
    let a_out = (alpha_out * 255.0).round().clamp(0.0, 255.0) as u8;

    Color::rgba(r_out, g_out, b_out, a_out)
}

/// Interpolates two CSS lengths, falling back to a discrete switch for
/// incompatible units (e.g. `auto` ↔ `10px`).
pub fn interpolate_length(a: Length, b: Length, t: f32) -> Length {
    use Length::*;
    match (a, b) {
        (Px(x), Px(y)) => Px(lerp(x, y, t)),
        (Em(x), Em(y)) => Em(lerp(x, y, t)),
        (Ex(x), Ex(y)) => Ex(lerp(x, y, t)),
        (Rem(x), Rem(y)) => Rem(lerp(x, y, t)),
        (Percent(x), Percent(y)) => Percent(lerp(x, y, t)),
        (Vw(x), Vw(y)) => Vw(lerp(x, y, t)),
        (Vh(x), Vh(y)) => Vh(lerp(x, y, t)),
        (Calc(x), Calc(y)) => Calc(crate::values::CalcLength {
            px: lerp(x.px, y.px, t),
            percent: lerp(x.percent, y.percent, t),
            has_percent: x.has_percent || y.has_percent,
            em: lerp(x.em, y.em, t),
            ex: lerp(x.ex, y.ex, t),
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
            (
                TransformFunction::Translate3d(x1, y1, z1),
                TransformFunction::Translate3d(x2, y2, z2),
            ) => TransformFunction::Translate3d(lerp(x1, x2, t), lerp(y1, y2, t), lerp(z1, z2, t)),
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

fn interpolate_filter_list(
    a: &[FilterFunction],
    b: &[FilterFunction],
    t: f32,
) -> Vec<FilterFunction> {
    if a.len() != b.len() {
        return if t < 0.5 { a.to_vec() } else { b.to_vec() };
    }
    a.iter()
        .zip(b.iter())
        .map(|(fa, fb)| match (fa, fb) {
            (FilterFunction::Blur(x), FilterFunction::Blur(y)) => {
                FilterFunction::Blur(lerp(*x, *y, t))
            }
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
            (FilterFunction::Sepia(x), FilterFunction::Sepia(y)) => {
                FilterFunction::Sepia(lerp(*x, *y, t))
            }
            (FilterFunction::HueRotate(x), FilterFunction::HueRotate(y)) => {
                FilterFunction::HueRotate(lerp(*x, *y, t))
            }
            (FilterFunction::Invert(x), FilterFunction::Invert(y)) => {
                FilterFunction::Invert(lerp(*x, *y, t))
            }
            (
                FilterFunction::DropShadow {
                    offset_x: x1,
                    offset_y: y1,
                    blur: b1,
                    color: c1,
                },
                FilterFunction::DropShadow {
                    offset_x: x2,
                    offset_y: y2,
                    blur: b2,
                    color: c2,
                },
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

fn interpolate_clip_path(
    a: &crate::values::ClipPath,
    b: &crate::values::ClipPath,
    t: f32,
) -> crate::values::ClipPath {
    use crate::values::ClipPath;
    match (a, b) {
        (
            ClipPath::Circle {
                radius: r1,
                center_x: cx1,
                center_y: cy1,
            },
            ClipPath::Circle {
                radius: r2,
                center_x: cx2,
                center_y: cy2,
            },
        ) => ClipPath::Circle {
            radius: interpolate_length(*r1, *r2, t),
            center_x: interpolate_length(*cx1, *cx2, t),
            center_y: interpolate_length(*cy1, *cy2, t),
        },
        (
            ClipPath::Ellipse {
                radius_x: rx1,
                radius_y: ry1,
                center_x: cx1,
                center_y: cy1,
            },
            ClipPath::Ellipse {
                radius_x: rx2,
                radius_y: ry2,
                center_x: cx2,
                center_y: cy2,
            },
        ) => ClipPath::Ellipse {
            radius_x: interpolate_length(*rx1, *rx2, t),
            radius_y: interpolate_length(*ry1, *ry2, t),
            center_x: interpolate_length(*cx1, *cx2, t),
            center_y: interpolate_length(*cy1, *cy2, t),
        },
        (
            ClipPath::Inset {
                top: t1,
                right: r1,
                bottom: b1,
                left: l1,
                round: rnd1,
            },
            ClipPath::Inset {
                top: t2,
                right: r2,
                bottom: b2,
                left: l2,
                round: rnd2,
            },
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
            let pts = p1
                .iter()
                .zip(p2.iter())
                .map(|((x1, y1), (x2, y2))| {
                    (
                        interpolate_length(*x1, *x2, t),
                        interpolate_length(*y1, *y2, t),
                    )
                })
                .collect();
            ClipPath::Polygon(pts)
        }
        _ => {
            if t < 0.5 {
                a.clone()
            } else {
                b.clone()
            }
        }
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
        (Value::TextShadow(x), Value::TextShadow(y)) => {
            Some(Value::TextShadow(crate::values::TextShadow {
                offset_x: lerp(x.offset_x, y.offset_x, t),
                offset_y: lerp(x.offset_y, y.offset_y, t),
                blur_radius: lerp(x.blur_radius, y.blur_radius, t),
                color: interpolate_color(x.color, y.color, t),
            }))
        }
        (Value::BoxShadow(x), Value::BoxShadow(y)) => {
            Some(Value::BoxShadow(crate::values::BoxShadow {
                offset_x: lerp(x.offset_x, y.offset_x, t),
                offset_y: lerp(x.offset_y, y.offset_y, t),
                blur_radius: lerp(x.blur_radius, y.blur_radius, t),
                spread_radius: lerp(x.spread_radius, y.spread_radius, t),
                color: interpolate_color(x.color, y.color, t),
                inset: x.inset,
            }))
        }
        (Value::Filter(x), Value::Filter(y)) => {
            Some(Value::Filter(interpolate_filter_list(x, y, t)))
        }
        (Value::ClipPath(x), Value::ClipPath(y)) => {
            Some(Value::ClipPath(interpolate_clip_path(x, y, t)))
        }
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
    style.border_bottom_color =
        interpolate_color(from.border_bottom_color, to.border_bottom_color, t);
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
    style.border_top_right_radius =
        lerp(from.border_top_right_radius, to.border_top_right_radius, t);
    style.border_bottom_right_radius = lerp(
        from.border_bottom_right_radius,
        to.border_bottom_right_radius,
        t,
    );
    style.border_bottom_left_radius = lerp(
        from.border_bottom_left_radius,
        to.border_bottom_left_radius,
        t,
    );

    style.row_gap = interpolate_length(from.row_gap, to.row_gap, t);
    style.column_gap = interpolate_length(from.column_gap, to.column_gap, t);
    style.flex_basis = interpolate_length(from.flex_basis, to.flex_basis, t);
    style.flex_grow = lerp(from.flex_grow, to.flex_grow, t);
    style.flex_shrink = lerp(from.flex_shrink, to.flex_shrink, t);

    style.transform = interpolate_transform(&from.transform, &to.transform, t);
    style.transform_origin_x =
        interpolate_length(from.transform_origin_x, to.transform_origin_x, t);
    style.transform_origin_y =
        interpolate_length(from.transform_origin_y, to.transform_origin_y, t);
    style.transform_origin_z =
        interpolate_length(from.transform_origin_z, to.transform_origin_z, t);
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

    if let (Some(a), Some(b)) = (from.aspect_ratio, to.aspect_ratio) {
        style.aspect_ratio = Some(lerp(a, b, t));
    } else if t >= 0.5 {
        style.aspect_ratio = to.aspect_ratio;
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
            var_name: None,
        })
        .collect();
    match (a, b) {
        (
            Gradient::Linear {
                angle_deg: a1,
                repeating,
                ..
            },
            Gradient::Linear { angle_deg: a2, .. },
        ) => Some(Gradient::Linear {
            angle_deg: lerp(*a1, *a2, t),
            stops,
            repeating: *repeating,
        }),
        (
            Gradient::Radial {
                repeating,
                center_x: cx1,
                center_y: cy1,
                radius_x: rx1,
                radius_y: ry1,
                ..
            },
            Gradient::Radial {
                center_x: cx2,
                center_y: cy2,
                radius_x: rx2,
                radius_y: ry2,
                ..
            },
        ) => Some(Gradient::Radial {
            stops,
            repeating: *repeating,
            center_x: lerp(*cx1, *cx2, t),
            center_y: lerp(*cy1, *cy2, t),
            radius_x: match (rx1, rx2) {
                (Some(a), Some(b)) => Some(lerp(*a, *b, t)),
                (Some(a), None) => Some(*a),
                (None, Some(b)) => Some(*b),
                (None, None) => None,
            },
            radius_y: match (ry1, ry2) {
                (Some(a), Some(b)) => Some(lerp(*a, *b, t)),
                (Some(a), None) => Some(*a),
                (None, Some(b)) => Some(*b),
                (None, None) => None,
            },
        }),
        (
            Gradient::Conic {
                angle_deg: a1,
                repeating,
                ..
            },
            Gradient::Conic { angle_deg: a2, .. },
        ) => Some(Gradient::Conic {
            angle_deg: lerp(*a1, *a2, t),
            stops,
            repeating: *repeating,
        }),
        _ => None,
    }
}
