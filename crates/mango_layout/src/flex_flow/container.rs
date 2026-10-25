use mango_core::{EdgeSizes, Point};
use mango_css::values::{BoxSizing, Direction, Display, FlexDirection, FlexWrap, Length, Position};

use crate::block_flow::layout_positioned_children_with_cb;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::alignment::{
    compute_align_content_spacing, compute_justify_content_spacing, compute_line_cross_sizes,
};
use super::lines::{
    build_flex_lines, resolve_flex_cross_gap, resolve_flex_main_gap, resolve_flexible_lengths,
};
use super::metrics::measure_item_metrics;
use super::placement::{
    ContainerFinalizeParams, LinePlacementParams, calculate_abspos_static_positions,
    finalize_container_height, position_flex_items_on_line,
};

/// Executes Flexbox layout on a flex container box according to CSS Flexible Box Layout Module Level 1.
pub fn layout_flex(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
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

    let is_row = matches!(
        style.flex_direction,
        FlexDirection::Row | FlexDirection::RowReverse
    );

    let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
    let is_pos_stretched_h = is_pos
        && style.top != Length::Auto
        && style.bottom != Length::Auto
        && container.dimensions.content.size.height > 0.0;
    let is_pre_resolved_height = is_pos_stretched_h
        || (container.dimensions.content.size.height > 0.0
            && containing_block.content.height() > 0.0
            && ((containing_block.content.height()
                - total_non_content_v
                - container.dimensions.content.size.height)
                .abs()
                < 1.0
                || (containing_block.content.height() - container.dimensions.content.size.height)
                    .abs()
                    < 1.0));

    let content_height = if !is_height_content && !is_percent_indefinite {
        let raw_h = style.height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_h - total_non_content_v).max(0.0)
        } else {
            raw_h
        }
    } else if is_pre_resolved_height {
        container.dimensions.content.size.height
    } else {
        0.0
    };

    let is_inline_flex = style.display == Display::InlineFlex;
    let is_width_content = matches!(
        style.width,
        Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
    );
    let (intrinsic_min, intrinsic_max) = if is_width_content
        || (is_inline_flex && is_width_auto)
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

    let is_pos_stretched_w = is_pos
        && style.left != Length::Auto
        && style.right != Length::Auto
        && container.dimensions.content.size.width > 0.0;
    let is_pre_resolved_width = is_pos_stretched_w
        || (!is_inline_flex
            && is_width_auto
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
        if is_inline_flex {
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
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_w - total_non_content_h).max(0.0)
        } else {
            raw_w
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
        let mut final_h = if is_height_content {
            0.0
        } else {
            content_height
        };
        if style.min_height != Length::Auto {
            let min_h = style.min_height.to_px_with_viewport(
                font_size,
                root_font_size,
                container_height,
                container_height,
            );
            let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
                (min_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
            } else {
                min_h
            };
            final_h = final_h.max(min_content_h);
        }
        if style.max_height != Length::Auto {
            let max_h = style.max_height.to_px_with_viewport(
                font_size,
                root_font_size,
                container_height,
                container_height,
            );
            let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
                (max_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
            } else {
                max_h
            };
            final_h = final_h.min(max_content_h);
        }
        container.dimensions.content.size.height = final_h;
        return;
    }

    // 2. Identify flex direction and axes
    // In a horizontal-tb RTL flex container, the main-start edge is the
    // physical right edge.  This is equivalent to reversing the row's
    // placement direction, while preserving the DOM order of the items.
    let is_reverse = matches!(
        style.flex_direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    ) ^ (is_row && style.direction == Direction::Rtl);
    let is_wrap = style.flex_wrap != FlexWrap::NoWrap;
    let is_wrap_reverse = style.flex_wrap == FlexWrap::WrapReverse;

    // 3. Sort children by `order` (stable sort)
    container
        .children
        .sort_by_key(|c| c.style.as_ref().map(|s| s.order).unwrap_or(0));

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
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - total_non_content_v).max(0.0)
        } else {
            raw_max
        }
    } else {
        f32::INFINITY
    };

    let sum_hypothetical: f32 = item_metrics.iter().map(|m| m.base_main).sum();
    let is_container_main_definite = if is_row {
        true
    } else {
        content_height > 0.0
            || (style.height != Length::Auto && !is_percent_indefinite)
            || (min_main_size > 0.0 && sum_hypothetical < min_main_size)
            || (max_main_size.is_finite() && sum_hypothetical > max_main_size)
    };
    let container_main_size = if is_row {
        content_width.max(min_main_size).min(max_main_size)
    } else if content_height > 0.0 {
        content_height.max(min_main_size).min(max_main_size)
    } else if max_main_size.is_finite() && sum_hypothetical > max_main_size {
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

    let lines = build_flex_lines(
        &item_metrics,
        is_wrap,
        is_container_main_definite,
        container_main_size,
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
        if content_height > 0.0 {
            content_height
        } else {
            0.0
        }
    } else {
        content_width
    };

    let cross_gap = resolve_flex_cross_gap(&style, is_row, container_cross_size, container_height);

    let (mut line_cross_sizes, line_baseline_ascents) = compute_line_cross_sizes(
        &lines,
        &item_metrics,
        container,
        &style,
        is_row,
        is_wrap,
        container_cross_size,
    );

    let (cross_cursor, line_spacing) = compute_align_content_spacing(
        &mut line_cross_sizes,
        &style,
        is_wrap,
        container_cross_size,
        cross_gap,
    );

    let cross_start_offset = cross_cursor;
    let mut normal_cross_cursor = cross_start_offset;

    for (line_idx, line) in lines.iter().enumerate() {
        let (main_cursor, item_spacing) = compute_justify_content_spacing(
            line,
            &item_metrics,
            container,
            &style,
            is_row,
            is_container_main_definite,
            container_main_size,
            main_gap,
        );

        let line_cross_size = line_cross_sizes[line_idx];

        let line_cross_cursor = if is_wrap_reverse {
            let after_lines_size: f32 = line_cross_sizes[line_idx + 1..].iter().sum();
            let after_lines_count = line_cross_sizes.len() - 1 - line_idx;
            cross_start_offset + after_lines_size + after_lines_count as f32 * line_spacing
        } else {
            normal_cross_cursor
        };

        position_flex_items_on_line(LinePlacementParams {
            line,
            item_metrics: &item_metrics,
            container,
            style: &style,
            is_row,
            is_reverse,
            is_container_main_definite,
            container_x,
            container_y,
            container_main_size,
            container_height,
            content_width,
            line_cross_size,
            line_baseline_ascent: line_baseline_ascents[line_idx],
            cross_cursor: line_cross_cursor,
            main_cursor,
            item_spacing,
        });

        normal_cross_cursor += line_cross_size + line_spacing;
    }

    // 7. Calculate container final height
    finalize_container_height(ContainerFinalizeParams {
        container,
        style: &style,
        is_row,
        is_percent_indefinite,
        font_size,
        root_font_size,
        container_height,
        content_height,
        cross_cursor: normal_cross_cursor,
        cross_gap,
        container_y,
        pad_top,
        pad_bottom,
        border_top,
        border_bottom,
    });

    // 8. Calculate static positions for out-of-flow positioned children (CSS Flexbox §4.1)
    calculate_abspos_static_positions(
        container,
        &style,
        is_row,
        is_reverse,
        container_x,
        container_y,
        content_width,
        container.dimensions.content.size.height,
    );

    // 9. Lay out out-of-flow positioned children
    layout_positioned_children_with_cb(container, containing_block, float_ctx);
}
