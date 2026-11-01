//! SVG `<clipPath>` mask generation and clipping rasterization.

use std::collections::HashMap;
use tiny_skia::{Mask, PathBuilder, Transform};

use super::ast::{SvgClipPath, SvgNode, SvgSymbol};
use super::cache::get_global_svg_symbol;
use super::render::add_ellipse_to_path;

/// Builds a 1-channel raster clip mask for the given `SvgClipPath`.
pub(crate) fn build_clip_mask(
    clip_path: &SvgClipPath,
    pixmap_w: u32,
    pixmap_h: u32,
    transform: Transform,
    symbols: &HashMap<String, SvgSymbol>,
) -> Option<Mask> {
    let mut mask = Mask::new(pixmap_w, pixmap_h)?;
    let mut final_tf = transform;
    if let Some(ctf) = clip_path.transform {
        final_tf = final_tf.pre_concat(ctf);
    }

    for child in &clip_path.children {
        render_clip_node_to_mask(&mut mask, child, final_tf, symbols, 0);
    }

    Some(mask)
}

fn render_clip_node_to_mask(
    mask: &mut Mask,
    node: &SvgNode,
    transform: Transform,
    symbols: &HashMap<String, SvgSymbol>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    match node {
        SvgNode::Path { path, state } => {
            mask.fill_path(path, state.fill_rule, true, transform);
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
                mask.fill_path(&path, state.fill_rule, true, transform);
            }
        }
        SvgNode::Circle { cx, cy, r, state } => {
            let mut pb = PathBuilder::new();
            add_ellipse_to_path(&mut pb, *cx, *cy, *r, *r);
            if let Some(path) = pb.finish() {
                mask.fill_path(&path, state.fill_rule, true, transform);
            }
        }
        SvgNode::Ellipse {
            cx,
            cy,
            rx,
            ry,
            state,
        } => {
            let mut pb = PathBuilder::new();
            add_ellipse_to_path(&mut pb, *cx, *cy, *rx, *ry);
            if let Some(path) = pb.finish() {
                mask.fill_path(&path, state.fill_rule, true, transform);
            }
        }
        SvgNode::Line { .. } => {}
        SvgNode::Polygon { points, state } => {
            if points.len() >= 2 {
                let mut pb = PathBuilder::new();
                pb.move_to(points[0].0, points[0].1);
                for pt in &points[1..] {
                    pb.line_to(pt.0, pt.1);
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    mask.fill_path(&path, state.fill_rule, true, transform);
                }
            }
        }
        SvgNode::Polyline { points, state } => {
            if points.len() >= 2 {
                let mut pb = PathBuilder::new();
                pb.move_to(points[0].0, points[0].1);
                for pt in &points[1..] {
                    pb.line_to(pt.0, pt.1);
                }
                if let Some(path) = pb.finish() {
                    mask.fill_path(&path, state.fill_rule, true, transform);
                }
            }
        }
        SvgNode::Group {
            children,
            transform: grp_tf,
            ..
        } => {
            let mut cur_tf = transform;
            if let Some(gt) = grp_tf {
                cur_tf = cur_tf.pre_concat(*gt);
            }
            for child in children {
                render_clip_node_to_mask(mask, child, cur_tf, symbols, depth + 1);
            }
        }
        SvgNode::Use {
            href,
            x,
            y,
            width,
            height,
            transform: use_tf,
            ..
        } => {
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
                if let Some(ut) = use_tf {
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
                for child in &sym.children {
                    render_clip_node_to_mask(mask, child, cur_tf, symbols, depth + 1);
                }
            }
        }
        SvgNode::Text { .. } => {}
    }
}
