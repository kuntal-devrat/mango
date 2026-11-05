use mango_css::values::Length;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

/// Intrinsic dimension context stored per leaf node in the Taffy tree.
#[derive(Clone, Copy, Debug, Default)]
pub struct LeafContext {
    pub intrinsic_width: f32,
    pub intrinsic_height: f32,
}

/// Computes the intrinsic content width and height for a flex/grid child item.
pub(crate) fn compute_child_intrinsic_size(
    child: &LayoutBox,
    cb_w: f32,
    cb_h: f32,
    container_fs: f32,
) -> (f32, f32) {
    let child_style = child.style.as_deref();
    let fs = child_style.map(|s| s.font_size).unwrap_or(container_fs);
    let root_fs = child_style
        .map(|s| s.root_font_size)
        .unwrap_or(container_fs);

    // 1. Replaced elements (img, svg, canvas, video, audio, iframe)
    match &child.box_type {
        BoxType::ReplacedElement {
            intrinsic_width,
            intrinsic_height,
            ..
        }
        | BoxType::Canvas {
            intrinsic_width,
            intrinsic_height,
            ..
        }
        | BoxType::IFrame {
            intrinsic_width,
            intrinsic_height,
            ..
        }
        | BoxType::Video {
            intrinsic_width,
            intrinsic_height,
            ..
        }
        | BoxType::Audio {
            intrinsic_width,
            intrinsic_height,
            ..
        } => {
            let mut w = *intrinsic_width;
            let mut h = *intrinsic_height;
            if let Some(s) = child_style {
                if let Length::Px(px) = s.width {
                    w = px;
                } else if let Length::Percent(p) = s.width {
                    if cb_w > 0.0 {
                        w = (p / 100.0) * cb_w;
                    }
                }
                if let Length::Px(px) = s.height {
                    h = px;
                } else if let Length::Percent(p) = s.height {
                    if cb_h > 0.0 {
                        h = (p / 100.0) * cb_h;
                    }
                }
                if let Length::Px(max_w) = s.max_width {
                    w = w.min(max_w);
                }
                if let Length::Px(max_h) = s.max_height {
                    h = h.min(max_h);
                }
                if let Some(ratio) = s.aspect_ratio.filter(|&r| r > 0.0) {
                    if w > 0.0 && h <= 0.0 {
                        h = w / ratio;
                    } else if h > 0.0 && w <= 0.0 {
                        w = h * ratio;
                    } else if w <= 0.0 && h <= 0.0 {
                        w = 300.0;
                        h = 300.0 / ratio;
                    }
                }
            }
            return (w.max(0.0), h.max(0.0));
        }
        _ => {}
    }

    // 2. Form controls intrinsic height
    let tag = child.tag_name.as_deref();
    let form_h = match tag {
        Some("select") => {
            let is_multiple = child.get_attribute("_mango_is_multiple") == Some("true")
                || child.get_attribute("multiple").is_some();
            let size_attr = child
                .get_attribute("size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(if is_multiple { 4 } else { 1 });
            if is_multiple || size_attr > 1 {
                Some(size_attr.max(3) as f32 * 18.0 + 8.0)
            } else {
                Some(24.0)
            }
        }
        Some("textarea") => Some(64.0),
        Some("input") => {
            let itype = child.get_attribute("type").unwrap_or("text");
            if itype == "checkbox" || itype == "radio" {
                Some(16.0)
            } else {
                Some(24.0)
            }
        }
        Some("button") => Some(24.0),
        _ => None,
    };

    // Padding and border sums for content-box conversion
    let pad_h = child_style
        .map(|s| {
            s.padding_left.to_px_with_viewport(fs, root_fs, cb_w, cb_h)
                + s.padding_right.to_px_with_viewport(fs, root_fs, cb_w, cb_h)
                + s.border_left_width
                + s.border_right_width
        })
        .unwrap_or(0.0);
    let pad_v = child_style
        .map(|s| {
            s.padding_top.to_px_with_viewport(fs, root_fs, cb_w, cb_h)
                + s.padding_bottom
                    .to_px_with_viewport(fs, root_fs, cb_w, cb_h)
                + s.border_top_width
                + s.border_bottom_width
        })
        .unwrap_or(0.0);

    let spacing_h = child_style
        .map(|s| {
            let m_l = match s.margin_left {
                Length::Px(px) => px,
                Length::Rem(rem) => rem * root_fs,
                Length::Em(em) => em * fs,
                _ => 0.0,
            };
            let m_r = match s.margin_right {
                Length::Px(px) => px,
                Length::Rem(rem) => rem * root_fs,
                Length::Em(em) => em * fs,
                _ => 0.0,
            };
            pad_h + m_l + m_r
        })
        .unwrap_or(pad_h);

    let is_border_box = child_style
        .map(|s| s.box_sizing == mango_css::values::BoxSizing::BorderBox)
        .unwrap_or(false);

    // Width measurement: fast-path definite width before heavy recursive subtree measuring
    let content_w = if let Some(s) = child_style {
        match s.width {
            Length::Px(px) => {
                if is_border_box {
                    (px - pad_h).max(0.0)
                } else {
                    px.max(0.0)
                }
            }
            _ => match s.flex_basis {
                Length::Px(px) => {
                    if is_border_box {
                        (px - pad_h).max(0.0)
                    } else {
                        px.max(0.0)
                    }
                }
                _ => {
                    if child.dimensions.content.size.width > 0.0 {
                        child.dimensions.content.size.width
                    } else {
                        let (_, max_w) = crate::scratchpad::with_intrinsic_width_cache(|memo| {
                            crate::table_flow::measure_box_intrinsic_widths(
                                child,
                                fs,
                                mango_render::FontWeight::Regular,
                                mango_render::FontFamily::SansSerif,
                                memo,
                            )
                        });
                        (max_w - spacing_h).max(0.0)
                    }
                }
            },
        }
    } else if child.dimensions.content.size.width > 0.0 {
        child.dimensions.content.size.width
    } else {
        let (_, max_w) = crate::scratchpad::with_intrinsic_width_cache(|memo| {
            crate::table_flow::measure_box_intrinsic_widths(
                child,
                fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                memo,
            )
        });
        (max_w - spacing_h).max(0.0)
    };

    let content_h = if let Some(s) = child_style {
        match s.height {
            Length::Px(px) => {
                if is_border_box {
                    (px - pad_v).max(0.0)
                } else {
                    px.max(0.0)
                }
            }
            _ => {
                if let Some(fh) = form_h {
                    fh.max(0.0)
                } else if child.dimensions.content.size.height > 0.0 {
                    child.dimensions.content.size.height
                } else if !child.children.is_empty() && child.children.len() <= 10 {
                    let measured = crate::flex_flow::sizing::measure_flex_child_intrinsic_height(
                        child, content_w, fs, root_fs, cb_h,
                    );
                    if measured > 0.0 {
                        measured
                    } else {
                        let (asc, desc, gap) = mango_render::font::font_manager().font_metrics(
                            mango_render::FontFamily::SansSerif,
                            mango_render::FontWeight::Regular,
                            fs,
                        );
                        s.line_height.unwrap_or(asc + desc + gap)
                    }
                } else if child.box_type.is_text()
                    || child.is_inline()
                    || matches!(child.box_type, BoxType::AnonymousBlock)
                    || !child.children.is_empty()
                {
                    let (asc, desc, gap) = mango_render::font::font_manager().font_metrics(
                        mango_render::FontFamily::SansSerif,
                        mango_render::FontWeight::Regular,
                        fs,
                    );
                    s.line_height.unwrap_or(asc + desc + gap)
                } else {
                    0.0
                }
            }
        }
    } else if let Some(fh) = form_h {
        (fh - pad_v).max(0.0)
    } else if child.dimensions.content.size.height > 0.0 {
        child.dimensions.content.size.height
    } else if child.box_type.is_text()
        || child.is_inline()
        || matches!(child.box_type, BoxType::AnonymousBlock)
        || !child.children.is_empty()
    {
        let (asc, desc, gap) = mango_render::font::font_manager().font_metrics(
            mango_render::FontFamily::SansSerif,
            mango_render::FontWeight::Regular,
            fs,
        );
        asc + desc + gap
    } else {
        child.dimensions.content.size.height
    };

    (content_w, content_h)
}
