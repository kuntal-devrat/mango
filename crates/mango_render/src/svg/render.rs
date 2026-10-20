//! SVG vector rasterization, state composition, and path rendering.

use mango_core::Color;
use std::collections::HashMap;
use tiny_skia::{Color as SkiaColor, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

use super::ast::{SvgDocument, SvgNode, SvgPaint, SvgRenderState, SvgSymbol};
use super::cache::get_global_svg_symbol;

pub(crate) fn compose_state(
    node_state: &SvgRenderState,
    parent_state: &SvgRenderState,
) -> SvgRenderState {
    SvgRenderState {
        fill: node_state
            .fill
            .clone()
            .or_else(|| parent_state.fill.clone()),
        fill_rule: node_state.fill_rule,
        fill_opacity: node_state.fill_opacity * parent_state.fill_opacity,
        stroke: node_state
            .stroke
            .clone()
            .or_else(|| parent_state.stroke.clone()),
        stroke_width: if node_state.stroke.is_some() {
            node_state.stroke_width
        } else {
            parent_state.stroke_width
        },
        stroke_opacity: node_state.stroke_opacity * parent_state.stroke_opacity,
        stroke_linecap: node_state.stroke_linecap,
        stroke_linejoin: node_state.stroke_linejoin,
        opacity: node_state.opacity * parent_state.opacity,
    }
}

impl SvgDocument {
    pub(crate) fn render(
        &self,
        pixmap: &mut Pixmap,
        transform: Transform,
        parent_state: &SvgRenderState,
        current_color: Color,
    ) {
        for child in &self.children {
            child.render(
                pixmap,
                transform,
                parent_state,
                current_color,
                &self.symbols,
                0,
            );
        }
    }
}

impl SvgNode {
    pub(crate) fn render(
        &self,
        pixmap: &mut Pixmap,
        transform: Transform,
        parent_state: &SvgRenderState,
        current_color: Color,
        symbols: &HashMap<String, SvgSymbol>,
        depth: usize,
    ) {
        match self {
            SvgNode::Path { path, state } => {
                let effective = compose_state(state, parent_state);
                draw_path_with_state(pixmap, path, &effective, transform, current_color);
            }
            SvgNode::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
                state,
            } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                if *rx > 0.0 || *ry > 0.0 {
                    let r = (*rx).max(*ry).min(*width / 2.0).min(*height / 2.0);
                    pb.move_to(x + r, *y);
                    pb.line_to(x + width - r, *y);
                    pb.quad_to(x + width, *y, x + width, y + r);
                    pb.line_to(x + width, y + height - r);
                    pb.quad_to(x + width, y + height, x + width - r, y + height);
                    pb.line_to(x + r, y + height);
                    pb.quad_to(*x, y + height, *x, y + height - r);
                    pb.line_to(*x, y + r);
                    pb.quad_to(*x, *y, x + r, *y);
                    pb.close();
                } else {
                    pb.move_to(*x, *y);
                    pb.line_to(x + width, *y);
                    pb.line_to(x + width, y + height);
                    pb.line_to(*x, y + height);
                    pb.close();
                }
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Circle { cx, cy, r, state } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                add_ellipse_to_path(&mut pb, *cx, *cy, *r, *r);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Ellipse {
                cx,
                cy,
                rx,
                ry,
                state,
            } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                add_ellipse_to_path(&mut pb, *cx, *cy, *rx, *ry);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Line {
                x1,
                y1,
                x2,
                y2,
                state,
            } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                pb.move_to(*x1, *y1);
                pb.line_to(*x2, *y2);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Polygon { points, state } => {
                let effective = compose_state(state, parent_state);
                if points.len() >= 2 {
                    let mut pb = PathBuilder::new();
                    pb.move_to(points[0].0, points[0].1);
                    for pt in &points[1..] {
                        pb.line_to(pt.0, pt.1);
                    }
                    pb.close();
                    if let Some(path) = pb.finish() {
                        draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                    }
                }
            }
            SvgNode::Polyline { points, state } => {
                let effective = compose_state(state, parent_state);
                if points.len() >= 2 {
                    let mut pb = PathBuilder::new();
                    pb.move_to(points[0].0, points[0].1);
                    for pt in &points[1..] {
                        pb.line_to(pt.0, pt.1);
                    }
                    if let Some(path) = pb.finish() {
                        draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                    }
                }
            }
            SvgNode::Group {
                children,
                state,
                transform: grp_transform,
            } => {
                let effective = compose_state(state, parent_state);
                let mut cur_tf = transform;
                if let Some(gt) = grp_transform {
                    cur_tf = cur_tf.pre_concat(*gt);
                }
                for child in children {
                    child.render(pixmap, cur_tf, &effective, current_color, symbols, depth);
                }
            }
            SvgNode::Use {
                href,
                x,
                y,
                width,
                height,
                state,
                transform: use_transform,
            } => {
                if depth > 16 {
                    return;
                }
                let sym_id = href.split('#').next_back().unwrap_or(href).trim();
                if sym_id.is_empty() {
                    return;
                }
                let symbol = symbols
                    .get(sym_id)
                    .cloned()
                    .or_else(|| get_global_svg_symbol(sym_id));

                if let Some(sym) = symbol {
                    let mut cur_tf = transform;
                    if let Some(ut) = use_transform {
                        cur_tf = cur_tf.pre_concat(*ut);
                    }

                    if let Some((vb_x, vb_y, vb_w, vb_h)) = sym.view_box {
                        if vb_w > 0.0 && vb_h > 0.0 {
                            let target_w = width.unwrap_or(vb_w);
                            let target_h = height.unwrap_or(vb_h);
                            let scale_x = target_w / vb_w;
                            let scale_y = target_h / vb_h;
                            let scale = scale_x.min(scale_y);
                            let offset_x = *x + (target_w - vb_w * scale) / 2.0;
                            let offset_y = *y + (target_h - vb_h * scale) / 2.0;
                            cur_tf = cur_tf
                                .pre_translate(offset_x, offset_y)
                                .pre_scale(scale, scale)
                                .pre_translate(-vb_x, -vb_y);
                        }
                    } else {
                        cur_tf = cur_tf.pre_translate(*x, *y);
                    }

                    let effective_use_state = compose_state(state, parent_state);
                    for child in &sym.children {
                        child.render(
                            pixmap,
                            cur_tf,
                            &effective_use_state,
                            current_color,
                            symbols,
                            depth + 1,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn add_ellipse_to_path(pb: &mut PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32) {
    let k = 0.552_284_8;
    let ox = rx * k;
    let oy = ry * k;

    pb.move_to(cx - rx, cy);
    pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
    pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
    pb.close();
}

pub(crate) fn draw_path_with_state(
    pixmap: &mut Pixmap,
    path: &Path,
    state: &SvgRenderState,
    transform: Transform,
    current_color: Color,
) {
    // 1. Fill
    if let Some(ref fill_paint_type) = state.fill {
        let fill_color = match fill_paint_type {
            SvgPaint::None => None,
            SvgPaint::Color(c) => Some(*c),
            SvgPaint::CurrentColor => Some(current_color),
        };

        if let Some(fill_color) = fill_color {
            let alpha = (fill_color.a as f32 / 255.0) * state.fill_opacity * state.opacity;
            if alpha > 0.0 {
                let mut paint = Paint::default();
                let skia_color = SkiaColor::from_rgba(
                    fill_color.r as f32 / 255.0,
                    fill_color.g as f32 / 255.0,
                    fill_color.b as f32 / 255.0,
                    alpha,
                );
                if let Some(sc) = skia_color {
                    let b = path.bounds();
                    if b.width() > 0.0 && b.height() > 0.0 {
                        paint.set_color(sc);
                        paint.anti_alias = true;
                        pixmap.fill_path(path, &paint, state.fill_rule, transform, None);
                    }
                }
            }
        }
    }

    // 2. Stroke
    if let Some(ref stroke_paint_type) = state.stroke {
        let stroke_color = match stroke_paint_type {
            SvgPaint::None => None,
            SvgPaint::Color(c) => Some(*c),
            SvgPaint::CurrentColor => Some(current_color),
        };

        if let Some(stroke_color) = stroke_color {
            let alpha = (stroke_color.a as f32 / 255.0) * state.stroke_opacity * state.opacity;
            if alpha > 0.0 && state.stroke_width > 0.0 {
                let mut paint = Paint::default();
                let skia_color = SkiaColor::from_rgba(
                    stroke_color.r as f32 / 255.0,
                    stroke_color.g as f32 / 255.0,
                    stroke_color.b as f32 / 255.0,
                    alpha,
                );
                if let Some(sc) = skia_color {
                    paint.set_color(sc);
                    paint.anti_alias = true;
                    let stroke = Stroke {
                        width: state.stroke_width,
                        line_cap: state.stroke_linecap,
                        line_join: state.stroke_linejoin,
                        ..Default::default()
                    };
                    pixmap.stroke_path(path, &paint, &stroke, transform, None);
                }
            }
        }
    }
}
