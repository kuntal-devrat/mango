use mango_css::computed::ComputedStyle;
use mango_css::values::{Direction, Position, WritingMode};

use crate::block_flow::{apply_relative_offset, layout_positioned_children_with_cb};
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::alignment::{compute_justify_content_spacing, LineCrossMetrics};
use super::metrics::ItemMetric;
use super::placement::{
    calculate_abspos_static_positions, finalize_container_height, position_flex_items_on_line,
    ContainerFinalizeParams, LinePlacementParams,
};

pub struct PositionLinesParams<'a> {
    pub lines: &'a [Vec<usize>],
    pub item_metrics: &'a [ItemMetric],
    pub container: &'a mut LayoutBox,
    pub containing_block: &'a Dimensions,
    pub float_ctx: &'a mut FloatContext,
    pub style: &'a ComputedStyle,
    pub is_row: bool,
    pub is_reverse: bool,
    pub is_vertical_wm: bool,
    pub is_width_auto: bool,
    pub is_wrap_reverse: bool,
    pub is_container_main_definite: bool,
    pub effective_container_main: f32,
    pub main_gap: f32,
    pub line_cross_sizes: &'a [f32],
    pub cross_start_offset: f32,
    pub line_spacing: f32,
    pub line_metrics: &'a LineCrossMetrics,
    pub container_x: f32,
    pub container_y: f32,
    pub container_height: f32,
    pub content_width: f32,
    pub specified_content_height: f32,
    pub is_percent_indefinite: bool,
    pub font_size: f32,
    pub root_font_size: f32,
    pub cross_gap: f32,
    pub pad_top: f32,
    pub pad_bottom: f32,
    pub border_top: f32,
    pub border_bottom: f32,
}

pub fn position_lines_and_finalize(params: PositionLinesParams) {
    let PositionLinesParams {
        lines,
        item_metrics,
        container,
        containing_block,
        float_ctx,
        style,
        is_row,
        is_reverse,
        is_vertical_wm,
        is_width_auto,
        is_wrap_reverse,
        is_container_main_definite,
        effective_container_main,
        main_gap,
        line_cross_sizes,
        cross_start_offset,
        line_spacing,
        line_metrics,
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
    } = params;

    let mut normal_cross_cursor = cross_start_offset;

    for (line_idx, line) in lines.iter().enumerate() {
        let (main_cursor, item_spacing) = compute_justify_content_spacing(
            line,
            item_metrics,
            container,
            style,
            is_row,
            is_reverse,
            is_container_main_definite,
            effective_container_main,
            main_gap,
        );

        let line_cross_size = line_cross_sizes[line_idx];

        let cross_is_reversed_progression = if is_row {
            if style.writing_mode == WritingMode::SidewaysLr {
                style.direction != Direction::Rtl
            } else {
                is_vertical_wm && style.direction == Direction::Rtl
            }
        } else {
            matches!(
                style.writing_mode,
                WritingMode::VerticalRl | WritingMode::SidewaysRl
            ) || (style.writing_mode == WritingMode::HorizontalTb
                && style.direction == Direction::Rtl)
        };

        let is_cross_reverse_lines = if cross_is_reversed_progression {
            !is_wrap_reverse
        } else {
            is_wrap_reverse
        };

        let line_cross_cursor = if is_cross_reverse_lines {
            let after_lines_size: f32 = line_cross_sizes[line_idx + 1..].iter().sum();
            let after_lines_count = line_cross_sizes.len() - 1 - line_idx;
            cross_start_offset + after_lines_size + after_lines_count as f32 * line_spacing
        } else {
            normal_cross_cursor
        };

        position_flex_items_on_line(LinePlacementParams {
            line,
            item_metrics,
            container,
            style,
            is_row,
            is_reverse,
            is_container_main_definite,
            container_x,
            container_y,
            container_main_size: effective_container_main,
            container_height,
            content_width,
            line_cross_size,
            line_baseline_ascent: line_metrics.first_ascents[line_idx],
            line_last_baseline_ascent: line_metrics.last_ascents[line_idx],
            line_baseline_cross: line_metrics.first_group_crosses[line_idx],
            line_last_baseline_cross: line_metrics.last_group_crosses[line_idx],
            cross_cursor: line_cross_cursor,
            main_cursor,
            item_spacing,
        });

        normal_cross_cursor += line_cross_size + line_spacing;
    }

    if is_vertical_wm && is_width_auto {
        let final_w = if is_row {
            effective_container_main
        } else {
            (normal_cross_cursor - cross_gap).max(0.0)
        };
        container.dimensions.content.size.width = final_w;
    }

    // 7. Calculate container final height
    finalize_container_height(ContainerFinalizeParams {
        container,
        style,
        is_row,
        is_percent_indefinite,
        font_size,
        root_font_size,
        container_height,
        content_height: specified_content_height,
        cross_cursor: normal_cross_cursor,
        cross_gap,
        container_y,
        pad_top,
        pad_bottom,
        border_top,
        border_bottom,
    });

    // Apply relative positioning offsets for flex items (CSS Flexbox §4.1)
    let container_dims = container.dimensions;
    for child in &mut container.children {
        if child.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
            apply_relative_offset(child, &container_dims);
        }
    }

    // 8. Calculate static positions for out-of-flow positioned children (CSS Flexbox §4.1)
    calculate_abspos_static_positions(
        container,
        style,
        is_row,
        is_reverse,
        container_x,
        container_y,
        content_width,
        container.dimensions.content.size.height,
    );

    // 9. Lay out out-of-flow positioned children
    layout_positioned_children_with_cb(container, containing_block, float_ctx);

    // CSS Flexbox §8.5: Record flex container first-line baseline
    super::metrics::record_flex_container_baseline(
        container,
        lines,
        &line_metrics.first_ascents,
        is_row,
        is_wrap_reverse,
        style.font_size,
    );

    // Initial scroll position for reverse flow directions (CSS Flexbox §5.1 & CSS Overflow 3 §2.2)
    super::metrics::init_reverse_scroll_position(container, style);
}
