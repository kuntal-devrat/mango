use mango_core::{EdgeSizes, Point};
use mango_css::values::{
    BoxSizing, Direction, Display, FlexDirection, Length, Position, WritingMode,
};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::alignment::{compute_align_content_spacing, compute_line_cross_sizes};
use super::container_helpers::{layout_empty_flex_container, resolve_container_cross_bounds};
use super::lines::{
    build_flex_lines, resolve_flex_cross_gap, resolve_flex_main_gap, resolve_flexible_lengths,
};
use super::metrics::measure_item_metrics;

/// Executes Flexbox layout on a flex container box according to CSS Flexible Box Layout Module Level 1.
pub fn layout_flex(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    if crate::taffy_adapter::compute_taffy_flex(container, containing_block, float_ctx) {
        return;
    }

    let container_width = containing_block.content.width();
    let container_height = if containing_block.content.height() > 0.0 {
        containing_block.content.height()
    } else if let Some(style) = &container.style {
        match style.height {
            Length::Px(px) => px,
            _ => 0.0,
        }
    } else {
        0.0
    };

    let style = container.style.clone().unwrap_or_default();
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;

    // 1. Resolve container padding, borders, and margins
    let pad_top = style.padding_top.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let pad_right = style.padding_right.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let pad_bottom = style.padding_bottom.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let pad_left = style.padding_left.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    container.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    container.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);
    container.dimensions.writing_mode = style.writing_mode;

    let margin_top = style.margin_top.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let margin_bottom = style.margin_bottom.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );

    let is_width_auto = style.width == Length::Auto;
    let is_margin_left_auto = style.margin_left == Length::Auto;
    let is_margin_right_auto = style.margin_right == Length::Auto;

    let mut margin_left = style.margin_left.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let mut margin_right = style.margin_right.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );

    let total_non_content_h = pad_left + pad_right + border_left + border_right;
    let total_non_content_v = pad_top + pad_bottom + border_top + border_bottom;

    let is_percent_indefinite =
        matches!(style.height, Length::Percent(_)) && containing_block.content.height() <= 0.0;
    let is_height_content = matches!(
        style.height,
        Length::Auto
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent
            | Length::Content
    );

    let is_vertical_wm = style.writing_mode.is_vertical();
    let is_flex_dir_row = matches!(
        style.flex_direction,
        FlexDirection::Row | FlexDirection::RowReverse
    );
    let is_row = if is_vertical_wm {
        !is_flex_dir_row
    } else {
        is_flex_dir_row
    };

    let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
    let is_pos_stretched_h = is_pos
        && style.top != Length::Auto
        && style.bottom != Length::Auto
        && container.dimensions.content.size.height > 0.0;
    let cb_h = containing_block.content.height();
    let c_h = container.dimensions.content.size.height;
    let is_pre_resolved_height = is_pos_stretched_h
        || (!is_height_content
            && c_h > 0.0
            && cb_h > 0.0
            && ((cb_h - total_non_content_v - c_h).abs() < 1.0 || (cb_h - c_h).abs() < 1.0));

    let (specified_content_height, content_height) = if is_pre_resolved_height {
        let ch = container.dimensions.content.size.height;
        (ch, ch)
    } else if !is_height_content && !is_percent_indefinite {
        let raw_h = style.height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let h = if style.box_sizing == BoxSizing::BorderBox {
            (raw_h - total_non_content_v).max(0.0)
        } else {
            raw_h
        };
        let avail_h = if style.overflow_x == mango_css::values::Overflow::Scroll {
            (h - 6.0).max(0.0)
        } else {
            h
        };
        (h, avail_h)
    } else if is_vertical_wm
        && containing_block.writing_mode.is_vertical()
        && containing_block.content.height() > 0.0
    {
        let h = (containing_block.content.height() - total_non_content_v).max(0.0);
        (h, h)
    } else {
        (0.0, 0.0)
    };

    let is_inline_flex = style.display == Display::InlineFlex;
    let is_width_content = matches!(
        style.width,
        Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
    );
    let (intrinsic_min, intrinsic_max) = if is_width_content
        || ((is_inline_flex || is_vertical_wm) && is_width_auto)
        || matches!(
            style.max_width,
            Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
        )
        || matches!(
            style.min_width,
            Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
        ) {
        let mut memo = std::collections::HashMap::new();
        crate::table_flow::measure_box_intrinsic_widths(
            container,
            font_size,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
            &mut memo,
        )
    } else {
        (0.0, 0.0)
    };

    let is_floating = style.float != mango_css::values::Float::None;
    let is_pos_stretched_w = is_pos
        && style.left != Length::Auto
        && style.right != Length::Auto
        && container.dimensions.content.size.width > 0.0;
    let is_pre_resolved_width = is_pos_stretched_w
        || (is_floating && container.dimensions.content.size.width > 0.0)
        || (!is_inline_flex
            && container.dimensions.content.size.width > 0.0
            && ((containing_block.content.width()
                - total_non_content_h
                - container.dimensions.content.size.width)
                .abs()
                < 1.0
                || (containing_block.content.width() - container.dimensions.content.size.width)
                    .abs()
                    < 1.0));

    let mut content_width = if is_pre_resolved_width {
        container.dimensions.content.size.width
    } else if is_width_auto {
        if is_margin_left_auto {
            margin_left = 0.0;
        }
        if is_margin_right_auto {
            margin_right = 0.0;
        }
        let avail = (container_width - total_non_content_h - margin_left - margin_right).max(0.0);
        if is_inline_flex || is_vertical_wm {
            let max_c = (intrinsic_max - total_non_content_h).max(0.0);
            let min_c = (intrinsic_min - total_non_content_h).max(0.0);
            avail.min(max_c).max(min_c)
        } else {
            avail
        }
    } else if is_width_content {
        match style.width {
            Length::MinContent => (intrinsic_min - total_non_content_h).max(0.0),
            Length::MaxContent | Length::Content => (intrinsic_max - total_non_content_h).max(0.0),
            Length::FitContent => {
                let avail =
                    (container_width - total_non_content_h - margin_left - margin_right).max(0.0);
                let max_c = (intrinsic_max - total_non_content_h).max(0.0);
                let min_c = (intrinsic_min - total_non_content_h).max(0.0);
                avail.min(max_c).max(min_c)
            }
            _ => (container_width - total_non_content_h).max(0.0),
        }
    } else {
        let raw_w = style.width.to_px_with_viewport(
            font_size,
            root_font_size,
            container_width,
            container_height,
        );
        let w = if style.box_sizing == BoxSizing::BorderBox {
            (raw_w - total_non_content_h).max(0.0)
        } else {
            raw_w
        };
        if style.overflow_y == mango_css::values::Overflow::Scroll {
            (w - 6.0).max(0.0)
        } else {
            w
        }
    };

    if !is_pre_resolved_width {
        if style.max_width != Length::Auto {
            let max_content_w = match style.max_width {
                Length::MinContent => (intrinsic_min - total_non_content_h).max(0.0),
                Length::MaxContent | Length::Content => {
                    (intrinsic_max - total_non_content_h).max(0.0)
                }
                Length::FitContent => {
                    let avail =
                        (container_width - total_non_content_h - margin_left - margin_right)
                            .max(0.0);
                    let max_c = (intrinsic_max - total_non_content_h).max(0.0);
                    let min_c = (intrinsic_min - total_non_content_h).max(0.0);
                    avail.min(max_c).max(min_c)
                }
                _ => {
                    let raw_max = style.max_width.to_px_with_viewport(
                        font_size,
                        root_font_size,
                        container_width,
                        container_height,
                    );
                    if style.box_sizing == BoxSizing::BorderBox {
                        (raw_max - total_non_content_h).max(0.0)
                    } else {
                        raw_max
                    }
                }
            };
            content_width = content_width.min(max_content_w);
        }
        if style.min_width != Length::Auto {
            let min_content_w = match style.min_width {
                Length::MinContent => (intrinsic_min - total_non_content_h).max(0.0),
                Length::MaxContent | Length::Content => {
                    (intrinsic_max - total_non_content_h).max(0.0)
                }
                Length::FitContent => {
                    let avail =
                        (container_width - total_non_content_h - margin_left - margin_right)
                            .max(0.0);
                    let max_c = (intrinsic_max - total_non_content_h).max(0.0);
                    let min_c = (intrinsic_min - total_non_content_h).max(0.0);
                    avail.min(max_c).max(min_c)
                }
                _ => {
                    let raw_min = style.min_width.to_px_with_viewport(
                        font_size,
                        root_font_size,
                        container_width,
                        container_height,
                    );
                    if style.box_sizing == BoxSizing::BorderBox {
                        (raw_min - total_non_content_h).max(0.0)
                    } else {
                        raw_min
                    }
                }
            };
            content_width = content_width.max(min_content_w);
        }

        let underflow =
            container_width - (content_width + total_non_content_h + margin_left + margin_right);

        let is_floating = style.float != mango_css::values::Float::None;
        let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
        if is_margin_left_auto && is_margin_right_auto {
            margin_left = (underflow / 2.0).max(0.0);
            margin_right = (underflow / 2.0).max(0.0);
        } else if is_margin_left_auto {
            margin_left = underflow.max(0.0);
        } else if is_margin_right_auto {
            margin_right = underflow.max(0.0);
        } else if !is_inline_flex && !is_width_auto && !is_floating && !is_pos {
            margin_right += underflow;
        }

        container.dimensions.margin =
            EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    }
    container.dimensions.content.size.width = content_width;

    let content_height = if is_height_content
        && let Some(ratio) = style.aspect_ratio
        && ratio > 0.0
        && content_width > 0.0
    {
        let h = if style.box_sizing == BoxSizing::BorderBox {
            ((content_width + total_non_content_h) / ratio - total_non_content_v).max(0.0)
        } else {
            (content_width / ratio).max(0.0)
        };
        container.dimensions.content.size.height = h;
        h
    } else {
        content_height
    };

    let mut eff_content_height = content_height;
    if style.min_height != Length::Auto {
        let min_h = style.min_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (min_h - total_non_content_v).max(0.0)
        } else {
            min_h
        };
        eff_content_height = eff_content_height.max(min_content_h);
    }
    if style.max_height != Length::Auto {
        let max_h = style.max_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (max_h - total_non_content_v).max(0.0)
        } else {
            max_h
        };
        if eff_content_height > 0.0 {
            eff_content_height = eff_content_height.min(max_content_h);
        }
    }

    let container_height = if eff_content_height > 0.0 {
        eff_content_height
    } else {
        container_height
    };

    // Position container in parent
    let is_pos = container
        .style
        .as_ref()
        .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
        .unwrap_or(false);
    let (container_x, container_y) = if is_pos || container.dimensions.content.origin != Point::ZERO
    {
        (
            container.dimensions.content.origin.x,
            container.dimensions.content.origin.y,
        )
    } else {
        (
            containing_block.content.x() + margin_left + border_left + pad_left,
            containing_block.content.y() + margin_top + border_top + pad_top,
        )
    };
    container.dimensions.content.origin = Point::new(container_x, container_y);

    if container.children.is_empty() {
        layout_empty_flex_container(
            container,
            &style,
            is_height_content,
            is_vertical_wm,
            content_height,
            container_height,
            font_size,
            root_font_size,
            pad_top,
            pad_bottom,
            border_top,
            border_bottom,
        );
        return;
    }

    // 2. Identify flex direction and axes
    let is_reverse = if is_row {
        if is_vertical_wm {
            if matches!(
                style.writing_mode,
                WritingMode::VerticalRl | WritingMode::SidewaysRl
            ) {
                style.flex_direction != FlexDirection::ColumnReverse
            } else {
                style.flex_direction == FlexDirection::ColumnReverse
            }
        } else {
            (style.flex_direction == FlexDirection::RowReverse)
                ^ (style.direction == Direction::Rtl)
        }
    } else if is_vertical_wm {
        if style.writing_mode == WritingMode::SidewaysLr {
            (style.flex_direction == FlexDirection::RowReverse)
                ^ (style.direction != Direction::Rtl)
        } else {
            (style.flex_direction == FlexDirection::RowReverse)
                ^ (style.direction == Direction::Rtl)
        }
    } else {
        style.flex_direction == FlexDirection::ColumnReverse
    };
    let is_wrap = style.flex_wrap.is_wrap();
    let is_wrap_reverse = style.flex_wrap.is_reverse();

    // 3. Sort children by `order` (stable sort)
    // CSS Flexible Box Module Level 1 § 4.1: The `order` property does not apply to
    // absolutely-positioned children of a flex container; they are treated as having order: 0.
    container.children.sort_by_key(|c| {
        let is_oof = c
            .style
            .as_ref()
            .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
            .unwrap_or(false);
        if is_oof {
            0
        } else {
            c.style.as_ref().map(|s| s.order).unwrap_or(0)
        }
    });

    // 4. Measure hypothetical main and cross sizes of each in-flow item
    let mut item_metrics =
        measure_item_metrics(container, is_row, content_width, container_height, &style);

    // 5. Construct flex lines (multi-line wrapping)
    let min_main_size = if is_row {
        if style.min_width != Length::Auto {
            let raw_min = style.min_width.to_px_with_viewport(
                font_size,
                root_font_size,
                container_width,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (raw_min - total_non_content_h).max(0.0)
            } else {
                raw_min
            }
        } else {
            0.0
        }
    } else if style.min_height != Length::Auto {
        let raw_min = style.min_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_min - total_non_content_v).max(0.0)
        } else {
            raw_min
        }
    } else {
        0.0
    };

    let max_main_size = if is_row {
        if style.max_width != Length::Auto {
            let raw_max = style.max_width.to_px_with_viewport(
                font_size,
                root_font_size,
                container_width,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (raw_max - total_non_content_h).max(0.0)
            } else {
                raw_max
            }
        } else {
            f32::INFINITY
        }
    } else if style.max_height != Length::Auto {
        let raw_max = style.max_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let mut max_h = if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - total_non_content_v).max(0.0)
        } else {
            raw_max
        };
        if style.overflow_x == mango_css::values::Overflow::Scroll {
            max_h = (max_h - 6.0).max(0.0);
        }
        max_h
    } else {
        f32::INFINITY
    };

    let sum_hypothetical: f32 = item_metrics
        .iter()
        .map(|m| m.base_main.clamp(m.min_main, m.max_main.max(m.min_main)))
        .sum();
    let is_multiline = style.flex_wrap != mango_css::values::FlexWrap::NoWrap;
    let is_clamped_by_max =
        max_main_size.is_finite() && !is_multiline && sum_hypothetical > max_main_size;
    let is_container_main_definite = if is_row {
        !(is_vertical_wm && is_width_auto)
            || (min_main_size > 0.0 && sum_hypothetical < min_main_size)
            || is_clamped_by_max
    } else {
        content_height > 0.0
            || (style.height != Length::Auto && !is_percent_indefinite)
            || (min_main_size > 0.0 && sum_hypothetical < min_main_size)
            || is_clamped_by_max
    };
    let container_main_size = if is_row {
        if is_vertical_wm && is_width_auto {
            if max_main_size.is_finite() {
                max_main_size
            } else {
                min_main_size
            }
        } else {
            content_width.max(min_main_size).min(max_main_size)
        }
    } else if content_height > 0.0 || is_clamped_by_max {
        (if content_height > 0.0 {
            content_height
        } else {
            max_main_size
        })
        .max(min_main_size)
        .min(max_main_size)
    } else if max_main_size.is_finite() {
        max_main_size
    } else {
        min_main_size
    };

    let main_gap = resolve_flex_main_gap(
        &style,
        is_row,
        container_main_size,
        container_height,
        is_container_main_definite,
    );

    let (wrap_main_size, can_wrap_main) = if is_container_main_definite {
        (container_main_size, true)
    } else if max_main_size.is_finite() {
        (max_main_size, true)
    } else {
        (f32::INFINITY, false)
    };

    let lines = build_flex_lines(
        &item_metrics,
        &style,
        can_wrap_main,
        wrap_main_size,
        main_gap,
    );

    // Resolve flexible main sizes and measure cross sizes for each line
    resolve_flexible_lengths(
        &lines,
        &mut item_metrics,
        container,
        is_row,
        is_container_main_definite,
        container_main_size,
        container_height,
        main_gap,
        &style,
        is_percent_indefinite,
    );

    // Pre-calculate line cross sizes for align-content distribution
    let container_cross_size = if is_row {
        if eff_content_height > 0.0 {
            eff_content_height
        } else {
            0.0
        }
    } else if is_vertical_wm && is_width_auto {
        0.0
    } else if style.overflow_y == mango_css::values::Overflow::Scroll {
        (content_width - 6.0).max(0.0)
    } else {
        content_width
    };

    let (min_cross_size, max_cross_size) = resolve_container_cross_bounds(
        &style,
        is_row,
        font_size,
        root_font_size,
        container_width,
        container_height,
        total_non_content_h,
        total_non_content_v,
    );

    let cross_gap = resolve_flex_cross_gap(&style, is_row, container_cross_size, container_height);

    let line_metrics = compute_line_cross_sizes(
        &lines,
        &item_metrics,
        container,
        &style,
        is_row,
        is_wrap,
        container_cross_size,
        min_cross_size,
        max_cross_size,
    );
    let mut line_cross_sizes = line_metrics.cross_sizes.clone();

    let (cross_cursor, line_spacing) = compute_align_content_spacing(
        &mut line_cross_sizes,
        &style,
        is_wrap,
        container_cross_size,
        cross_gap,
    );

    let cross_start_offset = cross_cursor;

    let effective_container_main = if is_container_main_definite {
        container_main_size
    } else {
        let max_line_main: f32 = lines
            .iter()
            .map(|l| {
                let total_item_main: f32 = l
                    .iter()
                    .map(|&idx| {
                        let m = &item_metrics[idx];
                        m.final_main + m.pad_main + m.border_main + m.margin_main
                    })
                    .sum();
                let gaps = if l.len() > 1 {
                    (l.len() - 1) as f32 * main_gap
                } else {
                    0.0
                };
                total_item_main + gaps
            })
            .fold(0.0f32, f32::max);
        max_line_main.max(min_main_size).min(max_main_size)
    };

    super::position_lines::position_lines_and_finalize(super::position_lines::PositionLinesParams {
        lines: &lines,
        item_metrics: &item_metrics,
        container,
        containing_block,
        float_ctx,
        style: &style,
        is_row,
        is_reverse,
        is_vertical_wm,
        is_width_auto,
        is_wrap_reverse,
        is_container_main_definite,
        effective_container_main,
        main_gap,
        line_cross_sizes: &line_cross_sizes,
        cross_start_offset,
        line_spacing,
        line_metrics: &line_metrics,
        container_x,
        container_y,
        container_height,
        content_width,
        specified_content_height,
        is_percent_indefinite,
        font_size,
        root_font_size,
        cross_gap,
        pad_top,
        pad_bottom,
        border_top,
        border_bottom,
    });
}
