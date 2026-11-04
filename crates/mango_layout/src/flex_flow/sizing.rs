use std::collections::HashMap;

use mango_core::Rect;
use mango_css::values::Length;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::layout_flex_item;

pub(crate) fn form_control_intrinsic_height(child: &LayoutBox) -> Option<f32> {
    match child.tag_name.as_deref() {
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
    }
}

pub fn resolve_item_main_size(
    child: &mut LayoutBox,
    is_row: bool,
    content_width: f32,
    container_height: f32,
    c_fs: f32,
    c_root_fs: f32,
    pad_main: f32,
    border_main: f32,
    margin_main: f32,
    pad_cross: f32,
    border_cross: f32,
    margin_cross: f32,
) -> (f32, bool) {
    let child_style = child.style.clone().unwrap_or_default();
    let basis = &child_style.flex_basis;
    let is_basis_auto = basis.is_auto();
    let is_basis_content = matches!(basis, Length::Content)
        || basis.is_content()
        || (is_basis_auto
            && (if is_row {
                child_style.width == Length::Auto
            } else {
                child_style.height == Length::Auto
            }));
    let is_basis_min_content = matches!(basis, Length::MinContent);
    let is_basis_max_content = matches!(basis, Length::MaxContent);
    let is_basis_fit_content = matches!(basis, Length::FitContent);

    let mut is_main_explicit = false;
    let aspect_ratio = if child.get_attribute("_mango_no_aspect_ratio").is_some() {
        None
    } else {
        child_style
            .aspect_ratio
            .filter(|&r| r > 0.0)
            .or_else(|| match &child.box_type {
                BoxType::ReplacedElement {
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
                }
                | BoxType::Canvas {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                } if *intrinsic_height > 0.0 => Some(*intrinsic_width / *intrinsic_height),
                _ => None,
            })
    };

    let explicit_main = if is_row {
        let aspect_w = if let Some(ratio) = aspect_ratio {
            if child_style.width == Length::Auto || is_basis_content {
                if child_style.height != Length::Auto
                    && !matches!(child_style.height, Length::Percent(_))
                    || (matches!(child_style.height, Length::Percent(_)) && container_height > 0.0)
                {
                    let h = child_style.height.to_px_with_viewport(
                        c_fs,
                        c_root_fs,
                        container_height,
                        container_height,
                    );
                    Some((h * ratio).max(0.0))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(w) = aspect_w {
            w
        } else if is_basis_min_content {
            let mut memo = HashMap::new();
            let (c_min, _) = crate::table_flow::measure_box_intrinsic_widths(
                child,
                c_fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                &mut memo,
            );
            (c_min - pad_main - border_main - margin_main.max(0.0)).max(0.0)
        } else if is_basis_max_content || is_basis_content {
            let mut memo = HashMap::new();
            let (_, c_max) = if is_basis_content {
                crate::table_flow::measure_box_content_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                )
            } else {
                crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                )
            };
            (c_max - pad_main - border_main - margin_main.max(0.0)).max(0.0)
        } else if is_basis_fit_content {
            let mut memo = HashMap::new();
            let (c_min, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                child,
                c_fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                &mut memo,
            );
            let fit = content_width.min(c_max).max(c_min);
            (fit - pad_main - border_main - margin_main.max(0.0)).max(0.0)
        } else if !is_basis_auto {
            is_main_explicit = true;
            basis.to_px_with_viewport(c_fs, c_root_fs, content_width, container_height)
        } else if matches!(child_style.width, Length::MinContent) {
            let mut memo = HashMap::new();
            let (c_min, _) = crate::table_flow::measure_box_intrinsic_widths(
                child,
                c_fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                &mut memo,
            );
            (c_min - pad_main - border_main - margin_main.max(0.0)).max(0.0)
        } else if matches!(child_style.width, Length::MaxContent | Length::Content) {
            let mut memo = HashMap::new();
            let (_, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                child,
                c_fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                &mut memo,
            );
            (c_max - pad_main - border_main - margin_main.max(0.0)).max(0.0)
        } else if matches!(child_style.width, Length::FitContent) {
            let mut memo = HashMap::new();
            let (c_min, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                child,
                c_fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                &mut memo,
            );
            let fit = content_width.min(c_max).max(c_min);
            (fit - pad_main - border_main - margin_main.max(0.0)).max(0.0)
        } else if child_style.width != Length::Auto {
            is_main_explicit = true;
            child_style
                .width
                .to_px_with_viewport(c_fs, c_root_fs, content_width, container_height)
        } else if let Some(ratio) = child_style.aspect_ratio
            && ratio > 0.0
            && child_style.height != Length::Auto
        {
            let h = child_style.height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            );
            (h * ratio).max(0.0)
        } else {
            match &child.box_type {
                BoxType::ReplacedElement {
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
                }
                | BoxType::Canvas {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                } => {
                    if !is_basis_content
                        && child_style.height != Length::Auto
                        && *intrinsic_height > 0.0
                    {
                        let ratio = if let Some(r) = child_style.aspect_ratio
                            && r > 0.0
                        {
                            r
                        } else {
                            *intrinsic_width / *intrinsic_height
                        };
                        let h = child_style.height.to_px_with_viewport(
                            c_fs,
                            c_root_fs,
                            container_height,
                            container_height,
                        );
                        (h * ratio).max(0.0)
                    } else {
                        *intrinsic_width
                    }
                }
                _ => crate::block_flow::calculate_shrink_to_fit_width(child, content_width),
            }
        }
    } else if let Some(h) = if let Some(ratio) = aspect_ratio {
        if child_style.height == Length::Auto || is_basis_content {
            if child_style.width != Length::Auto && !matches!(child_style.width, Length::Percent(_))
                || (matches!(child_style.width, Length::Percent(_)) && content_width > 0.0)
            {
                let w = child_style.width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    content_width,
                    container_height,
                );
                Some((w / ratio).max(0.0))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    } {
        h
    } else if is_basis_min_content || is_basis_max_content || is_basis_fit_content {
        0.0
    } else if !is_basis_auto && !is_basis_content {
        is_main_explicit = true;
        basis.to_px_with_viewport(c_fs, c_root_fs, container_height, container_height)
    } else if !is_basis_content
        && child_style.height != Length::Auto
        && !matches!(
            child_style.height,
            Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
        )
    {
        is_main_explicit = true;
        child_style
            .height
            .to_px_with_viewport(c_fs, c_root_fs, container_height, container_height)
    } else if let Some(ratio) = child_style.aspect_ratio
        && ratio > 0.0
        && child_style.width != Length::Auto
    {
        let w =
            child_style
                .width
                .to_px_with_viewport(c_fs, c_root_fs, content_width, container_height);
        (w / ratio).max(0.0)
    } else {
        match &child.box_type {
            BoxType::ReplacedElement {
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
            }
            | BoxType::Canvas {
                intrinsic_width,
                intrinsic_height,
                ..
            } => {
                if child_style.width != Length::Auto && *intrinsic_width > 0.0 {
                    let ratio = if let Some(r) = child_style.aspect_ratio
                        && r > 0.0
                    {
                        r
                    } else {
                        *intrinsic_width / *intrinsic_height
                    };
                    let w = child_style.width.to_px_with_viewport(
                        c_fs,
                        c_root_fs,
                        content_width,
                        container_height,
                    );
                    (w / ratio).max(0.0)
                } else {
                    *intrinsic_height
                }
            }
            _ => {
                if let Some(fc_h) = form_control_intrinsic_height(child) {
                    fc_h
                } else {
                    let item_w = if child_style.width != Length::Auto {
                        child_style.width.to_px_with_viewport(
                            c_fs,
                            c_root_fs,
                            content_width,
                            container_height,
                        )
                    } else {
                        (content_width - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
                    };
                    let orig_style = if is_basis_content {
                        let orig = child.style.clone();
                        if let Some(s) = child.style.as_ref() {
                            let mut new_style = (**s).clone();
                            new_style.height = Length::Auto;
                            child.style = Some(std::sync::Arc::new(new_style));
                        }
                        orig
                    } else {
                        None
                    };
                    child.dimensions.content.size.width = item_w;
                    let item_cb = Dimensions::new(Rect::new(
                        0.0,
                        0.0,
                        item_w + pad_cross + border_cross,
                        0.0,
                    ));
                    let mut item_float_ctx = FloatContext::new();
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    let measured_h = child.dimensions.content.size.height;
                    if let Some(orig) = orig_style {
                        child.style = Some(orig);
                    }
                    if child.is_inline() || child.box_type.is_text() {
                        measured_h.max(c_fs * 1.3)
                    } else {
                        measured_h
                    }
                }
            }
        }
    };

    (explicit_main, is_main_explicit)
}

pub(crate) fn measure_flex_child_intrinsic_height(
    child: &LayoutBox,
    cross_size: f32,
    c_fs: f32,
    c_root_fs: f32,
    container_height: f32,
) -> f32 {
    let mut temp_child = child.clone();
    let item_w = if let Some(s) = child.style.as_ref()
        && s.width != Length::Auto
    {
        s.width
            .to_px_with_viewport(c_fs, c_root_fs, cross_size, container_height)
    } else {
        cross_size
    };
    temp_child.dimensions.content.size.width = item_w;
    let item_cb = Dimensions::new(Rect::new(0.0, 0.0, item_w, 0.0));
    let mut item_float_ctx = FloatContext::new();
    layout_flex_item(&mut temp_child, &item_cb, &mut item_float_ctx, c_fs);
    temp_child.dimensions.content.size.height
}

pub use super::limits::resolve_item_base_and_limits;
