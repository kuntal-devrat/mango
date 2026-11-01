//! Parsing of SVG leaf graphic elements (`path`, `rect`, `circle`, `ellipse`, `line`, `polygon`, `polyline`, `use`).

use super::ast::{SvgNode, SvgRenderState};
use super::path::parse_svg_path;
use super::xml_attrs::{
    apply_attributes_to_state, get_f32_attr_with_ref, parse_points, parse_transform,
};

pub(crate) fn parse_leaf_element(
    name: &str,
    attrs: &[(String, String)],
    cur_state: &SvgRenderState,
    ref_w: f32,
    ref_h: f32,
) -> Option<SvgNode> {
    let state = apply_attributes_to_state(cur_state.clone(), attrs);
    let tag_name = name.to_ascii_lowercase();
    match tag_name.as_str() {
        "path" => {
            if let Some((_, d)) = attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case("d")) {
                parse_svg_path(d).map(|path| SvgNode::Path { path, state })
            } else {
                None
            }
        }
        "rect" => {
            let x = get_f32_attr_with_ref(attrs, "x", ref_w).unwrap_or(0.0);
            let y = get_f32_attr_with_ref(attrs, "y", ref_h).unwrap_or(0.0);
            let w = get_f32_attr_with_ref(attrs, "width", ref_w).unwrap_or(0.0);
            let h = get_f32_attr_with_ref(attrs, "height", ref_h).unwrap_or(0.0);
            let rx = get_f32_attr_with_ref(attrs, "rx", ref_w).unwrap_or(0.0);
            let ry = get_f32_attr_with_ref(attrs, "ry", ref_h).unwrap_or(rx);
            if w > 0.0 && h > 0.0 {
                Some(SvgNode::Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                    rx,
                    ry,
                    state,
                })
            } else {
                None
            }
        }
        "circle" => {
            let cx = get_f32_attr_with_ref(attrs, "cx", ref_w).unwrap_or(0.0);
            let cy = get_f32_attr_with_ref(attrs, "cy", ref_h).unwrap_or(0.0);
            let ref_diag = ((ref_w * ref_w + ref_h * ref_h) / 2.0).sqrt();
            let r = get_f32_attr_with_ref(attrs, "r", ref_diag).unwrap_or(0.0);
            if r > 0.0 {
                Some(SvgNode::Circle { cx, cy, r, state })
            } else {
                None
            }
        }
        "ellipse" => {
            let cx = get_f32_attr_with_ref(attrs, "cx", ref_w).unwrap_or(0.0);
            let cy = get_f32_attr_with_ref(attrs, "cy", ref_h).unwrap_or(0.0);
            let rx = get_f32_attr_with_ref(attrs, "rx", ref_w).unwrap_or(0.0);
            let ry = get_f32_attr_with_ref(attrs, "ry", ref_h).unwrap_or(0.0);
            if rx > 0.0 && ry > 0.0 {
                Some(SvgNode::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    state,
                })
            } else {
                None
            }
        }
        "line" => {
            let x1 = get_f32_attr_with_ref(attrs, "x1", ref_w).unwrap_or(0.0);
            let y1 = get_f32_attr_with_ref(attrs, "y1", ref_h).unwrap_or(0.0);
            let x2 = get_f32_attr_with_ref(attrs, "x2", ref_w).unwrap_or(0.0);
            let y2 = get_f32_attr_with_ref(attrs, "y2", ref_h).unwrap_or(0.0);
            Some(SvgNode::Line {
                x1,
                y1,
                x2,
                y2,
                state,
            })
        }
        "polygon" => attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("points"))
            .map(|(_, pts)| SvgNode::Polygon {
                points: parse_points(pts),
                state,
            }),
        "polyline" => attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("points"))
            .map(|(_, pts)| SvgNode::Polyline {
                points: parse_points(pts),
                state,
            }),
        "use" => {
            let href = attrs
                .iter()
                .find(|(k, _)| {
                    k.eq_ignore_ascii_case("href") || k.eq_ignore_ascii_case("xlink:href")
                })
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let x = get_f32_attr_with_ref(attrs, "x", ref_w).unwrap_or(0.0);
            let y = get_f32_attr_with_ref(attrs, "y", ref_h).unwrap_or(0.0);
            let width = get_f32_attr_with_ref(attrs, "width", ref_w);
            let height = get_f32_attr_with_ref(attrs, "height", ref_h);
            let transform = attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("transform"))
                .and_then(|(_, v)| parse_transform(v));
            Some(SvgNode::Use {
                href,
                x,
                y,
                width,
                height,
                state,
                transform,
            })
        }
        _ => None,
    }
}
