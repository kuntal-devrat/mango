use std::collections::HashMap;

use mango_core::Rect;
use mango_css::values::{BoxSizing, Length, Overflow};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::layout_flex_item;

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
    let is_basis_content = matches!(basis, Length::Content) || basis.is_content();
    let is_basis_min_content = matches!(basis, Length::MinContent);
    let is_basis_max_content = matches!(basis, Length::MaxContent);
    let is_basis_fit_content = matches!(basis, Length::FitContent);
    let is_basis_auto = basis.is_auto();

    let mut is_main_explicit = false;
    let explicit_main = if is_row {
        if is_basis_min_content {
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
            let (_, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                child,
                c_fs,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                &mut memo,
            );
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
                    if child_style.height != Length::Auto && *intrinsic_height > 0.0 {
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
                _ => {
                    let mut memo = HashMap::new();
                    let (_, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                        child,
                        c_fs,
                        mango_render::FontWeight::Regular,
                        mango_render::FontFamily::SansSerif,
                        &mut memo,
                    );
                    (c_max - pad_main - border_main - margin_main.max(0.0)).max(0.0)
                }
            }
        }
    } else if is_basis_min_content
        || is_basis_max_content
        || is_basis_content
        || is_basis_fit_content
    {
        0.0
    } else if !is_basis_auto {
        is_main_explicit = true;
        basis.to_px_with_viewport(c_fs, c_root_fs, container_height, container_height)
    } else if child_style.height != Length::Auto
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
                child.dimensions.content.size.width = item_w;
                let item_cb =
                    Dimensions::new(Rect::new(0.0, 0.0, item_w + pad_cross + border_cross, 0.0));
                let mut item_float_ctx = FloatContext::new();
                layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                let measured_h = child.dimensions.content.size.height;
                if child.is_inline() || child.box_type.is_text() {
                    measured_h.max(c_fs * 1.3)
                } else {
                    measured_h
                }
            }
        }
    };

    (explicit_main, is_main_explicit)
}

pub fn resolve_item_base_and_limits(
    child: &mut LayoutBox,
    is_row: bool,
    explicit_main: f32,
    is_main_explicit: bool,
    cross_size: f32,
    content_width: f32,
    container_height: f32,
    c_fs: f32,
    c_root_fs: f32,
    pad_main: f32,
    border_main: f32,
    margin_main: f32,
) -> (f32, f32, f32) {
    let child_style = child.style.clone().unwrap_or_default();

    let mut base_main = if is_main_explicit {
        if child_style.box_sizing == BoxSizing::BorderBox {
            (explicit_main - pad_main - border_main).max(0.0)
        } else {
            explicit_main.max(0.0)
        }
    } else if explicit_main > 0.0 {
        explicit_main
    } else {
        match &child.box_type {
            BoxType::TextNode(t) => {
                let bold = crate::inline_flow::is_bold_weight(
                    child_style.font_weight,
                    child_style.font_size,
                );
                let weight = if bold {
                    mango_render::FontWeight::Bold
                } else {
                    mango_render::FontWeight::Regular
                };
                let family = mango_render::FontFamily::from_css_name(&child_style.font_family);
                let spacing = child_style.letter_spacing.to_px(
                    child_style.font_size,
                    child_style.root_font_size,
                    0.0,
                );
                let w = crate::inline_flow::measure_text_width_with_style_and_spacing(
                    t, c_fs, weight, family, spacing,
                );
                if is_row { w } else { c_fs * 1.3 }
            }
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
                if is_row {
                    *intrinsic_width
                } else {
                    *intrinsic_height
                }
            }
            _ => {
                if is_row {
                    let fit_w =
                        crate::block_flow::calculate_shrink_to_fit_width(child, content_width);
                    if fit_w > 0.0 { fit_w } else { 0.0 }
                } else {
                    let pad_cross = child.dimensions.padding.horizontal();
                    let border_cross = child.dimensions.border.horizontal();
                    child.dimensions.content.size.width = cross_size;
                    let item_cb = Dimensions::new(Rect::new(
                        0.0,
                        0.0,
                        cross_size + pad_cross + border_cross,
                        0.0,
                    ));
                    let mut item_float_ctx = FloatContext::new();
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    child.dimensions.content.size.height
                }
            }
        }
    };

    let mut min_main = 0.0f32;
    let mut max_main = f32::INFINITY;

    if is_row {
        if child_style.max_width != Length::Auto {
            let max_w = child_style.max_width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                container_height,
            );
            let max_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                (max_w - pad_main - border_main).max(0.0)
            } else {
                max_w
            };
            max_main = max_content_w;
        }
        if child_style.min_width != Length::Auto {
            let min_w = child_style.min_width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                container_height,
            );
            let min_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                (min_w - pad_main - border_main).max(0.0)
            } else {
                min_w
            };
            min_main = min_content_w;
        } else if child_style.overflow_x == Overflow::Visible {
            let auto_min = match &child.box_type {
                BoxType::ReplacedElement {
                    intrinsic_width, ..
                }
                | BoxType::IFrame {
                    intrinsic_width, ..
                }
                | BoxType::Video {
                    intrinsic_width, ..
                }
                | BoxType::Audio {
                    intrinsic_width, ..
                }
                | BoxType::Canvas {
                    intrinsic_width, ..
                } => *intrinsic_width,
                _ => {
                    if child.children.is_empty() && child.text().is_none() {
                        0.0
                    } else {
                        let mut memo = HashMap::new();
                        let (child_min, _) = crate::table_flow::measure_box_intrinsic_widths(
                            child,
                            c_fs,
                            mango_render::FontWeight::Regular,
                            mango_render::FontFamily::SansSerif,
                            &mut memo,
                        );
                        let content_min =
                            (child_min - pad_main - border_main - margin_main.max(0.0)).max(0.0);
                        if is_main_explicit && explicit_main > 0.0 {
                            content_min.min(explicit_main)
                        } else {
                            content_min
                        }
                    }
                }
            };
            min_main = auto_min.min(max_main);
        }
    } else {
        if child_style.max_height != Length::Auto {
            let max_h = child_style.max_height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            );
            let max_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                (max_h - pad_main - border_main).max(0.0)
            } else {
                max_h
            };
            max_main = max_content_h;
        }
        if child_style.min_height != Length::Auto {
            let min_h = child_style.min_height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            );
            let min_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                (min_h - pad_main - border_main).max(0.0)
            } else {
                min_h
            };
            min_main = min_content_h;
        } else if child_style.overflow_y == Overflow::Visible {
            let auto_min = match &child.box_type {
                BoxType::ReplacedElement {
                    intrinsic_height, ..
                }
                | BoxType::IFrame {
                    intrinsic_height, ..
                }
                | BoxType::Video {
                    intrinsic_height, ..
                }
                | BoxType::Audio {
                    intrinsic_height, ..
                }
                | BoxType::Canvas {
                    intrinsic_height, ..
                } => *intrinsic_height,
                _ => {
                    if child.dimensions.content.size.height > 0.0 {
                        child.dimensions.content.size.height
                    } else if child.children.is_empty() && child.text().is_none() {
                        0.0
                    } else {
                        child.dimensions.content.size.width = cross_size;
                        let item_cb = Dimensions::new(Rect::new(0.0, 0.0, cross_size, 0.0));
                        let mut item_float_ctx = FloatContext::new();
                        layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                        child.dimensions.content.size.height
                    }
                }
            };
            min_main = auto_min.min(max_main);
        }
    }

    base_main = base_main.clamp(min_main, max_main);
    (base_main, min_main, max_main)
}
