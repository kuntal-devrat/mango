//! SVG vector rasterization, state composition, and path rendering.

use mango_core::Color;
use std::collections::HashMap;
use tiny_skia::{
    Color as SkiaColor, Mask, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

use super::ast::{
    SvgClipPath, SvgDocument, SvgGradient, SvgNode, SvgPaint, SvgRenderState, SvgSymbol,
    SvgTextAnchor,
};
use super::cache::get_global_svg_symbol;
use super::clip::build_clip_mask;
use super::gradient::build_svg_shader;
use super::text::render_svg_text;

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
        stroke_dasharray: node_state
            .stroke_dasharray
            .clone()
            .or_else(|| parent_state.stroke_dasharray.clone()),
        stroke_dashoffset: if node_state.stroke_dashoffset != 0.0 {
            node_state.stroke_dashoffset
        } else {
            parent_state.stroke_dashoffset
        },
        clip_path: node_state
            .clip_path
            .clone()
            .or_else(|| parent_state.clip_path.clone()),
        opacity: node_state.opacity * parent_state.opacity,
        font_size: if node_state.font_size > 0.0 {
            node_state.font_size
        } else {
            parent_state.font_size
        },
        font_family: if !node_state.font_family.is_empty() {
            node_state.font_family.clone()
        } else {
            parent_state.font_family.clone()
        },
        font_weight: if !node_state.font_weight.is_empty() {
            node_state.font_weight.clone()
        } else {
            parent_state.font_weight.clone()
        },
        font_style: if !node_state.font_style.is_empty() {
            node_state.font_style.clone()
        } else {
            parent_state.font_style.clone()
        },
        text_anchor: if node_state.text_anchor != SvgTextAnchor::Start {
            node_state.text_anchor
        } else {
            parent_state.text_anchor
        },
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
                &self.gradients,
                &self.clip_paths,
                None,
                0,
            );
        }
    }
}

impl SvgNode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &self,
        pixmap: &mut Pixmap,
        transform: Transform,
        parent_state: &SvgRenderState,
        current_color: Color,
        symbols: &HashMap<String, SvgSymbol>,
        gradients: &HashMap<String, SvgGradient>,
        clip_paths: &HashMap<String, SvgClipPath>,
        inherited_clip: Option<&Mask>,
        depth: usize,
    ) {
        let node_clip_mask = if let Some(clip_id) = self.get_clip_id() {
            let clean_id = clip_id.trim_start_matches('#');
            if let Some(cp) = clip_paths.get(clean_id) {
                build_clip_mask(cp, pixmap.width(), pixmap.height(), transform, symbols)
            } else {
                None
            }
        } else {
            None
        };

        let effective_clip = node_clip_mask.as_ref().or(inherited_clip);

        match self {
            SvgNode::Path { path, state } => {
                let effective = compose_state(state, parent_state);
                draw_path_with_state(
                    pixmap,
                    path,
                    &effective,
                    transform,
                    current_color,
                    gradients,
                    effective_clip,
                );
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
                    draw_path_with_state(
                        pixmap,
                        &path,
                        &effective,
                        transform,
                        current_color,
                        gradients,
                        effective_clip,
                    );
                }
            }
            SvgNode::Circle { cx, cy, r, state } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                add_ellipse_to_path(&mut pb, *cx, *cy, *r, *r);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(
                        pixmap,
                        &path,
                        &effective,
                        transform,
                        current_color,
                        gradients,
                        effective_clip,
                    );
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
                    draw_path_with_state(
                        pixmap,
                        &path,
                        &effective,
                        transform,
                        current_color,
                        gradients,
                        effective_clip,
                    );
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
                    draw_path_with_state(
                        pixmap,
                        &path,
                        &effective,
                        transform,
                        current_color,
                        gradients,
                        effective_clip,
                    );
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
                        draw_path_with_state(
                            pixmap,
                            &path,
                            &effective,
                            transform,
                            current_color,
                            gradients,
                            effective_clip,
                        );
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
                        draw_path_with_state(
                            pixmap,
                            &path,
                            &effective,
                            transform,
                            current_color,
                            gradients,
                            effective_clip,
                        );
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
                    child.render(
                        pixmap,
                        cur_tf,
                        &effective,
                        current_color,
                        symbols,
                        gradients,
                        clip_paths,
                        effective_clip,
                        depth,
                    );
                }
            }
            SvgNode::Text {
                x,
                y,
                dx,
                dy,
                text,
                font_size,
                font_family,
                font_weight,
                font_style: _,
                text_anchor,
                tspans,
                state,
                transform: text_transform,
            } => {
                let effective = compose_state(state, parent_state);
                let mut cur_tf = transform;
                if let Some(tt) = text_transform {
                    cur_tf = cur_tf.pre_concat(*tt);
                }
                render_svg_text(
                    pixmap,
                    *x,
                    *y,
                    *dx,
                    *dy,
                    text,
                    *font_size,
                    font_family,
                    font_weight,
                    *text_anchor,
                    tspans,
                    &effective,
                    cur_tf,
                    current_color,
                    effective_clip,
                );
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
                            gradients,
                            clip_paths,
                            effective_clip,
                            depth + 1,
                        );
                    }
                }
            }
        }
    }

    fn get_clip_id(&self) -> Option<&str> {
        let state = match self {
            SvgNode::Path { state, .. } => state,
            SvgNode::Rect { state, .. } => state,
            SvgNode::Circle { state, .. } => state,
            SvgNode::Ellipse { state, .. } => state,
            SvgNode::Line { state, .. } => state,
            SvgNode::Polygon { state, .. } => state,
            SvgNode::Polyline { state, .. } => state,
            SvgNode::Group { state, .. } => state,
            SvgNode::Use { state, .. } => state,
            SvgNode::Text { state, .. } => state,
        };
        state.clip_path.as_deref()
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
    gradients: &HashMap<String, SvgGradient>,
    clip_mask: Option<&Mask>,
) {
    let b = path.bounds();
    if b.width() <= 0.0 && b.height() <= 0.0 {
        return;
    }

    // 1. Fill (only if path has non-zero fill area)
    if b.width() > 0.0
        && b.height() > 0.0
        && let Some(ref fill_paint_type) = state.fill
    {
        match fill_paint_type {
            SvgPaint::None => {}
            SvgPaint::Url(url) => {
                let clean_id = url.trim_start_matches('#');
                if let Some(grad) = gradients.get(clean_id) {
                    let total_opacity = state.fill_opacity * state.opacity;
                    if total_opacity > 0.0
                        && let Some(shader) =
                            build_svg_shader(grad, b, transform, total_opacity, current_color)
                    {
                        let mut paint = Paint::default();
                        paint.shader = shader;
                        paint.anti_alias = true;
                        pixmap.fill_path(path, &paint, state.fill_rule, transform, clip_mask);
                    }
                }
            }
            SvgPaint::Color(c) => {
                draw_solid_fill(pixmap, path, *c, state, transform, clip_mask);
            }
            SvgPaint::CurrentColor => {
                draw_solid_fill(pixmap, path, current_color, state, transform, clip_mask);
            }
        }
    }

    // 2. Stroke
    if let Some(ref stroke_paint_type) = state.stroke {
        if state.stroke_width <= 0.0 {
            return;
        }
        let stroke_dash = state
            .stroke_dasharray
            .as_ref()
            .and_then(|dashes| StrokeDash::new(dashes.clone(), state.stroke_dashoffset));

        let stroke = Stroke {
            width: state.stroke_width,
            line_cap: state.stroke_linecap,
            line_join: state.stroke_linejoin,
            dash: stroke_dash,
            ..Default::default()
        };

        match stroke_paint_type {
            SvgPaint::None => {}
            SvgPaint::Url(url) => {
                let clean_id = url.trim_start_matches('#');
                if let Some(grad) = gradients.get(clean_id) {
                    let total_opacity = state.stroke_opacity * state.opacity;
                    if total_opacity > 0.0
                        && let Some(shader) =
                            build_svg_shader(grad, b, transform, total_opacity, current_color)
                    {
                        let mut paint = Paint::default();
                        paint.shader = shader;
                        paint.anti_alias = true;
                        pixmap.stroke_path(path, &paint, &stroke, transform, clip_mask);
                    }
                }
            }
            SvgPaint::Color(c) => {
                draw_solid_stroke(pixmap, path, *c, state, &stroke, transform, clip_mask);
            }
            SvgPaint::CurrentColor => {
                draw_solid_stroke(
                    pixmap,
                    path,
                    current_color,
                    state,
                    &stroke,
                    transform,
                    clip_mask,
                );
            }
        }
    }
}

fn draw_solid_fill(
    pixmap: &mut Pixmap,
    path: &Path,
    color: Color,
    state: &SvgRenderState,
    transform: Transform,
    clip_mask: Option<&Mask>,
) {
    let alpha = (color.a as f32 / 255.0) * state.fill_opacity * state.opacity;
    if alpha <= 0.0 {
        return;
    }
    let mut paint = Paint::default();
    let skia_color = SkiaColor::from_rgba(
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        alpha,
    );
    if let Some(sc) = skia_color {
        paint.set_color(sc);
        paint.anti_alias = true;
        pixmap.fill_path(path, &paint, state.fill_rule, transform, clip_mask);
    }
}

fn draw_solid_stroke(
    pixmap: &mut Pixmap,
    path: &Path,
    color: Color,
    state: &SvgRenderState,
    stroke: &Stroke,
    transform: Transform,
    clip_mask: Option<&Mask>,
) {
    let alpha = (color.a as f32 / 255.0) * state.stroke_opacity * state.opacity;
    if alpha <= 0.0 {
        return;
    }
    let mut paint = Paint::default();
    let skia_color = SkiaColor::from_rgba(
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        alpha,
    );
    if let Some(sc) = skia_color {
        paint.set_color(sc);
        paint.anti_alias = true;
        pixmap.stroke_path(path, &paint, stroke, transform, clip_mask);
    }
}
