use std::collections::HashMap;

use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::{AlignItems, AlignSelf, BoxSizing, Length};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::layout_flex_item;

pub fn resolve_item_cross_size(
    child: &mut LayoutBox,
    is_row: bool,
    explicit_main: f32,
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
    container_style: &ComputedStyle,
) -> f32 {
    let child_style = child.style.clone().unwrap_or_default();
    let is_child_height_auto = matches!(
        child_style.height,
        Length::Auto
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent
            | Length::Content
    ) || (matches!(child_style.height, Length::Percent(_))
        && (container_height <= 0.0 || container_style.height == Length::Auto));

    let mut cross_size = if is_row {
        if !is_child_height_auto {
            let h = child_style.height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            );
            if child_style.box_sizing == BoxSizing::BorderBox {
                (h - pad_cross - border_cross).max(0.0)
            } else {
                h
            }
        } else if let Some(ratio) = child_style.aspect_ratio
            && ratio > 0.0
            && explicit_main > 0.0
        {
            (explicit_main / ratio).max(0.0)
        } else {
            match &child.box_type {
                BoxType::ReplacedElement {
                    intrinsic_height,
                    intrinsic_width,
                    ..
                }
                | BoxType::IFrame {
                    intrinsic_height,
                    intrinsic_width,
                    ..
                }
                | BoxType::Video {
                    intrinsic_height,
                    intrinsic_width,
                    ..
                }
                | BoxType::Audio {
                    intrinsic_height,
                    intrinsic_width,
                    ..
                }
                | BoxType::Canvas {
                    intrinsic_height,
                    intrinsic_width,
                    ..
                } => {
                    if explicit_main > 0.0 && *intrinsic_width > 0.0 && *intrinsic_height > 0.0 {
                        let ratio = if let Some(r) = child_style.aspect_ratio
                            && r > 0.0
                        {
                            r
                        } else {
                            *intrinsic_width / *intrinsic_height
                        };
                        (explicit_main / ratio).max(0.0)
                    } else {
                        *intrinsic_height
                    }
                }
                _ if child.tag_name.as_deref() == Some("input") => {
                    let itype = child.get_attribute("type").unwrap_or("text");
                    if itype == "checkbox" || itype == "radio" {
                        16.0
                    } else {
                        24.0
                    }
                }
                _ => {
                    let item_w = if explicit_main > 0.0 {
                        explicit_main
                    } else if child_style.width != Length::Auto {
                        child_style.width.to_px_with_viewport(
                            c_fs,
                            c_root_fs,
                            content_width,
                            container_height,
                        )
                    } else {
                        (content_width - pad_main - border_main - margin_main.max(0.0)).max(0.0)
                    };
                    child.dimensions.content.size.width = item_w;
                    let item_cb =
                        Dimensions::new(Rect::new(0.0, 0.0, item_w + pad_main + border_main, 0.0));
                    let mut item_float_ctx = FloatContext::new();
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    child.dimensions.content.size.height.max(c_fs * 1.3)
                }
            }
        }
    } else if !matches!(
        child_style.width,
        Length::Auto
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent
            | Length::Content
    ) {
        let w =
            child_style
                .width
                .to_px_with_viewport(c_fs, c_root_fs, content_width, container_height);
        if child_style.box_sizing == BoxSizing::BorderBox {
            (w - pad_cross - border_cross).max(0.0)
        } else {
            w
        }
    } else if let Some(ratio) = child_style.aspect_ratio
        && ratio > 0.0
        && explicit_main > 0.0
    {
        (explicit_main * ratio).max(0.0)
    } else if let Some(w) = match &child.box_type {
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
            if explicit_main > 0.0 && *intrinsic_width > 0.0 && *intrinsic_height > 0.0 {
                let ratio = if let Some(r) = child_style.aspect_ratio
                    && r > 0.0
                {
                    r
                } else {
                    *intrinsic_width / *intrinsic_height
                };
                Some((explicit_main * ratio).max(0.0))
            } else {
                Some(*intrinsic_width)
            }
        }
        _ => None,
    } {
        w
    } else {
        let item_align = match child_style.align_self {
            AlignSelf::Auto => container_style.align_items,
            AlignSelf::Stretch => AlignItems::Stretch,
            AlignSelf::FlexStart => AlignItems::FlexStart,
            AlignSelf::FlexEnd => AlignItems::FlexEnd,
            AlignSelf::Center => AlignItems::Center,
            AlignSelf::Baseline => AlignItems::Baseline,
        };
        if matches!(item_align, AlignItems::Normal | AlignItems::Stretch) {
            (content_width - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
        } else {
            let fit_w = crate::block_flow::calculate_shrink_to_fit_width(child, content_width);
            if fit_w > 0.0 {
                fit_w
            } else {
                let mut memo = HashMap::new();
                let (_, max_w) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                if max_w > 0.0 {
                    (max_w - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
                } else {
                    (content_width - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
                }
            }
        }
    };

    // Clamp cross size by min/max cross constraints
    if is_row {
        if child_style.max_height != Length::Auto {
            let max_h = child_style.max_height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            );
            let max_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                (max_h - pad_cross - border_cross).max(0.0)
            } else {
                max_h
            };
            cross_size = cross_size.min(max_content_h);
        }
        if child_style.min_height != Length::Auto {
            let min_h = child_style.min_height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            );
            let min_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                (min_h - pad_cross - border_cross).max(0.0)
            } else {
                min_h
            };
            cross_size = cross_size.max(min_content_h);
        }
    } else {
        if child_style.max_width != Length::Auto {
            let max_w = child_style.max_width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                container_height,
            );
            let max_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                (max_w - pad_cross - border_cross).max(0.0)
            } else {
                max_w
            };
            cross_size = cross_size.min(max_content_w);
        }
        if child_style.min_width != Length::Auto {
            let min_w = child_style.min_width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                container_height,
            );
            let min_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                (min_w - pad_cross - border_cross).max(0.0)
            } else {
                min_w
            };
            cross_size = cross_size.max(min_content_w);
        }
    }

    cross_size
}
