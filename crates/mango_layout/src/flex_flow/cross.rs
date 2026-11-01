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
    let item_align = match child_style.align_self {
        AlignSelf::Auto => container_style.align_items,
        AlignSelf::Stretch => AlignItems::Stretch,
        AlignSelf::FlexStart => AlignItems::FlexStart,
        AlignSelf::FlexEnd => AlignItems::FlexEnd,
        AlignSelf::Center => AlignItems::Center,
        AlignSelf::Baseline => AlignItems::Baseline,
        AlignSelf::SafeCenter => AlignItems::SafeCenter,
        AlignSelf::SafeFlexStart => AlignItems::SafeFlexStart,
        AlignSelf::SafeFlexEnd => AlignItems::SafeFlexEnd,
        AlignSelf::SafeStart => AlignItems::SafeStart,
        AlignSelf::SafeEnd => AlignItems::SafeEnd,
        AlignSelf::LastBaseline => AlignItems::LastBaseline,
    };

    let has_cross_auto_margin = if is_row {
        child_style.margin_top == Length::Auto || child_style.margin_bottom == Length::Auto
    } else {
        child_style.margin_left == Length::Auto || child_style.margin_right == Length::Auto
    };

    let is_wrap = container_style.flex_wrap != mango_css::values::FlexWrap::NoWrap;
    let has_explicit_main_prop = if is_row {
        child_style.width != Length::Auto
            && !matches!(
                child_style.width,
                Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
            )
    } else {
        child_style.height != Length::Auto
            && !matches!(
                child_style.height,
                Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
            )
    };
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
        } else if !is_wrap
            && matches!(item_align, AlignItems::Normal | AlignItems::Stretch)
            && !has_cross_auto_margin
            && container_height > 0.0
        {
            let mut stretched =
                (container_height - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0);
            if child_style.max_height != Length::Auto {
                let max_h = child_style.max_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                let max_content = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_h - pad_cross - border_cross).max(0.0)
                } else {
                    max_h
                };
                stretched = stretched.min(max_content);
            }
            if child_style.min_height != Length::Auto {
                let min_h = child_style.min_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                let min_content = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_h - pad_cross - border_cross).max(0.0)
                } else {
                    min_h
                };
                stretched = stretched.max(min_content);
            }
            stretched
        } else if let Some(ratio) = child_style.aspect_ratio
            && ratio > 0.0
            && has_explicit_main_prop
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
                    if has_explicit_main_prop
                        && explicit_main > 0.0
                        && *intrinsic_width > 0.0
                        && *intrinsic_height > 0.0
                    {
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
                _ if super::sizing::form_control_intrinsic_height(child).is_some() => {
                    super::sizing::form_control_intrinsic_height(child).unwrap()
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
                    let eff_cb_h = if container_style.flex_wrap.is_balanced() {
                        if let Some(min_lines) = container_style.flex_line_count {
                            if min_lines > 1 {
                                let cross_gap = super::lines::resolve_flex_cross_gap(
                                    container_style,
                                    is_row,
                                    content_width,
                                    container_height,
                                );
                                if container_height > 0.0 {
                                    (container_height - (min_lines - 1) as f32 * cross_gap).max(0.0)
                                        / min_lines as f32
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        }
                    } else if container_height > 0.0 {
                        container_height
                    } else {
                        0.0
                    };
                    let item_cb = Dimensions::new(Rect::new(
                        0.0,
                        0.0,
                        item_w + pad_main + border_main,
                        eff_cb_h,
                    ));
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
    } else if !is_wrap
        && matches!(item_align, AlignItems::Normal | AlignItems::Stretch)
        && child_style.width == Length::Auto
        && !has_cross_auto_margin
        && !(container_style.writing_mode.is_vertical() && container_style.width == Length::Auto)
    {
        // An auto cross size on a single-line column flex item is stretched to the flex
        // line's cross size, including replaced elements.
        (content_width - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
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
            if has_explicit_main_prop
                && explicit_main > 0.0
                && *intrinsic_width > 0.0
                && *intrinsic_height > 0.0
            {
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
        #[allow(clippy::nonminimal_bool)]
        if !is_wrap
            && matches!(item_align, AlignItems::Normal | AlignItems::Stretch)
            && !has_cross_auto_margin
            && !(container_style.writing_mode.is_vertical()
                && container_style.width == Length::Auto)
        {
            (content_width - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
        } else {
            let available_cross_for_shrink = if container_style.flex_wrap.is_balanced() {
                if let Some(min_lines) = container_style.flex_line_count {
                    if min_lines > 1 {
                        let cross_gap = super::lines::resolve_flex_cross_gap(
                            container_style,
                            is_row,
                            content_width,
                            container_height,
                        );
                        let container_cross = if is_row {
                            container_height
                        } else {
                            content_width
                        };
                        if container_cross > 0.0 {
                            (container_cross - (min_lines - 1) as f32 * cross_gap).max(0.0)
                                / min_lines as f32
                        } else {
                            content_width
                        }
                    } else {
                        content_width
                    }
                } else {
                    content_width
                }
            } else {
                content_width
            };
            let fit_w =
                crate::block_flow::calculate_shrink_to_fit_width(child, available_cross_for_shrink);
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
                } else if !is_wrap {
                    (content_width - pad_cross - border_cross - margin_cross.max(0.0)).max(0.0)
                } else {
                    0.0
                }
            }
        }
    };

    // Clamp cross size by min/max cross constraints
    if is_row {
        let is_max_h_pct_indefinite =
            child_style.max_height.has_percent() && container_height <= 0.0;
        if child_style.max_height != Length::Auto && !is_max_h_pct_indefinite {
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
