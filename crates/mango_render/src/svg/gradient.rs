//! SVG linear and radial gradient shader construction.

use mango_core::Color;
use tiny_skia::{
    Color as SkiaColor, GradientStop, LinearGradient, Point, RadialGradient, Rect, Shader,
    SpreadMode, Transform,
};

use super::ast::{SvgGradient, SvgSpreadMethod};

/// Builds a `tiny_skia::Shader` for the given SVG gradient and target bounding box.
pub(crate) fn build_svg_shader(
    gradient: &SvgGradient,
    bounds: Rect,
    shape_transform: Transform,
    global_opacity: f32,
    current_color: Color,
) -> Option<Shader<'_>> {
    match gradient {
        SvgGradient::Linear {
            x1,
            y1,
            x2,
            y2,
            stops,
            transform: grad_tf,
            spread_method,
            user_space_on_use,
            ..
        } => {
            let (p1, p2) = if *user_space_on_use {
                (Point::from_xy(*x1, *y1), Point::from_xy(*x2, *y2))
            } else {
                let bx = bounds.x();
                let by = bounds.y();
                let bw = bounds.width();
                let bh = bounds.height();
                (
                    Point::from_xy(bx + *x1 * bw, by + *y1 * bh),
                    Point::from_xy(bx + *x2 * bw, by + *y2 * bh),
                )
            };

            let skia_stops = convert_stops(stops, global_opacity, current_color);
            if skia_stops.is_empty() {
                return None;
            }

            let mode = convert_spread_mode(*spread_method);
            let mut final_tf = shape_transform;
            if let Some(gtf) = grad_tf {
                final_tf = final_tf.pre_concat(*gtf);
            }

            LinearGradient::new(p1, p2, skia_stops, mode, final_tf)
        }
        SvgGradient::Radial {
            cx,
            cy,
            r,
            fx,
            fy,
            stops,
            transform: grad_tf,
            spread_method,
            user_space_on_use,
            ..
        } => {
            let (center, focal, radius) = if *user_space_on_use {
                (Point::from_xy(*cx, *cy), Point::from_xy(*fx, *fy), *r)
            } else {
                let bx = bounds.x();
                let by = bounds.y();
                let bw = bounds.width();
                let bh = bounds.height();
                let radius_scale = (bw * bw + bh * bh).sqrt() / std::f32::consts::SQRT_2;
                (
                    Point::from_xy(bx + *cx * bw, by + *cy * bh),
                    Point::from_xy(bx + *fx * bw, by + *fy * bh),
                    *r * radius_scale.max(1.0),
                )
            };

            if radius <= 0.0 {
                return None;
            }

            let skia_stops = convert_stops(stops, global_opacity, current_color);
            if skia_stops.is_empty() {
                return None;
            }

            let mode = convert_spread_mode(*spread_method);
            let mut final_tf = shape_transform;
            if let Some(gtf) = grad_tf {
                final_tf = final_tf.pre_concat(*gtf);
            }

            RadialGradient::new(focal, center, radius, skia_stops, mode, final_tf)
        }
    }
}

fn convert_spread_mode(method: SvgSpreadMethod) -> SpreadMode {
    match method {
        SvgSpreadMethod::Pad => SpreadMode::Pad,
        SvgSpreadMethod::Reflect => SpreadMode::Reflect,
        SvgSpreadMethod::Repeat => SpreadMode::Repeat,
    }
}

fn convert_stops(
    stops: &[super::ast::SvgStop],
    global_opacity: f32,
    _current_color: Color,
) -> Vec<GradientStop> {
    if stops.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(stops.len());
    for s in stops {
        let alpha = (s.color.a as f32 / 255.0) * s.opacity * global_opacity;
        let c = SkiaColor::from_rgba(
            s.color.r as f32 / 255.0,
            s.color.g as f32 / 255.0,
            s.color.b as f32 / 255.0,
            alpha.clamp(0.0, 1.0),
        )
        .unwrap_or(SkiaColor::BLACK);
        out.push(GradientStop::new(s.offset.clamp(0.0, 1.0), c));
    }
    out
}
