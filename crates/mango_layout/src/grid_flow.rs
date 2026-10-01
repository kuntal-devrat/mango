//! CSS Grid Layout Module Level 1 implementation.
//!
//! Handles 1-track, 2-track (Wikipedia sidebar TOC + main content columns), and multi-track
//! grid containers: track sizing (fixed lengths, percentages, `fr` fractional distribution,
//! `auto`, `minmax`), grid item placement (auto-placement and explicit `grid-column` / `grid-row`
//! line / span placements), gap distribution (`column-gap` / `row-gap`), and recursive cell content layout.

use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::{
    AlignItems, AlignSelf, BoxSizing, Display, GridPlacement, GridTrackSize, Length, Position,
};

use crate::block_flow::{layout_block_contents, layout_positioned_children, shift_descendants};
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

struct PlacedItem {
    child_idx: usize,
    col_start: usize,
    col_span: usize,
    row_start: usize,
    row_span: usize,
}

/// Executes CSS Grid layout on a grid container box.
pub fn layout_grid(
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

    // 1. Resolve container padding, borders, and margins
    let pad_top = style.padding_top.to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_right = style.padding_right.to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_bottom = style.padding_bottom.to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_left = style.padding_left.to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    container.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    container.dimensions.border = EdgeSizes::new(border_top, border_right, border_bottom, border_left);

    let margin_top = style.margin_top.to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let margin_bottom = style.margin_bottom.to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let is_width_auto = style.width == Length::Auto;
    let is_margin_left_auto = style.margin_left == Length::Auto;
    let is_margin_right_auto = style.margin_right == Length::Auto;

    let mut margin_left = style.margin_left.to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let mut margin_right = style.margin_right.to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let total_non_content_h = pad_left + pad_right + border_left + border_right;

    let content_width = if is_width_auto {
        if is_margin_left_auto {
            margin_left = 0.0;
        }
        if is_margin_right_auto {
            margin_right = 0.0;
        }
        (container_width - total_non_content_h - margin_left - margin_right).max(0.0)
    } else {
        let raw_w = style.width.to_px_with_viewport(font_size, 16.0, container_width, container_height);
        let resolved_w = if style.box_sizing == BoxSizing::BorderBox {
            (raw_w - total_non_content_h).max(0.0)
        } else {
            raw_w
        };

        let underflow = container_width - (resolved_w + total_non_content_h + margin_left + margin_right);

        if is_margin_left_auto && is_margin_right_auto {
            margin_left = (underflow / 2.0).max(0.0);
            margin_right = (underflow / 2.0).max(0.0);
        } else if is_margin_left_auto {
            margin_left = underflow.max(0.0);
        } else if is_margin_right_auto {
            margin_right = underflow.max(0.0);
        } else {
            margin_right += underflow;
        }

        resolved_w
    };

    container.dimensions.margin = EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    container.dimensions.content.size.width = content_width;

    // Position container in parent if not already positioned
    let (container_x, container_y) = if container.dimensions.content.origin != Point::ZERO {
        (container.dimensions.content.origin.x, container.dimensions.content.origin.y)
    } else {
        (
            containing_block.content.x() + margin_left + border_left + pad_left,
            containing_block.content.y() + margin_top + border_top + pad_top,
        )
    };
    container.dimensions.content.origin = Point::new(container_x, container_y);

    if container.children.is_empty() {
        container.dimensions.content.size.height = 0.0;
        return;
    }

    // 2. Resolve gaps
    let col_gap = style.column_gap.to_px_with_viewport(font_size, 16.0, content_width, container_height);
    let row_gap = style.row_gap.to_px_with_viewport(font_size, 16.0, content_width, container_height);

    // 3. Filter in-flow children
    let mut in_flow_indices = Vec::new();
    for (idx, child) in container.children.iter().enumerate() {
        let disp = child.style.as_ref().map(|s| s.display).unwrap_or(Display::Block);
        let pos = child.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
        if disp != Display::None && !matches!(pos, Position::Absolute | Position::Fixed) {
            in_flow_indices.push(idx);
        }
    }

    // 4. Resolve column track templates (including auto-fill / auto-fit)
    let area_cols = style.grid_template_areas.iter().map(|row| row.len()).max().unwrap_or(0);
    let mut template_cols = if !style.grid_template_columns.is_empty() {
        let mut expanded = Vec::new();
        // Calculate non-repeat space
        let mut non_repeat_fixed = 0.0f32;
        let mut non_repeat_count = 0usize;
        for t in &style.grid_template_columns {
            match t {
                GridTrackSize::RepeatAutoFill(_) | GridTrackSize::RepeatAutoFit(_) => {}
                GridTrackSize::Length(l) => {
                    non_repeat_fixed += l.to_px_with_viewport(font_size, 16.0, content_width, container_height);
                    non_repeat_count += 1;
                }
                _ => {
                    non_repeat_count += 1;
                }
            }
        }
        let repeat_space = (content_width - non_repeat_fixed - (non_repeat_count as f32) * col_gap).max(0.0);

        for t in &style.grid_template_columns {
            match t {
                GridTrackSize::RepeatAutoFill(inner) => {
                    let min_sz = track_min_size(inner, font_size, content_width, container_height);
                    let count = ((repeat_space + col_gap) / (min_sz + col_gap)).floor() as usize;
                    let count = count.max(1);
                    for _ in 0..count {
                        expanded.push((**inner).clone());
                    }
                }
                GridTrackSize::RepeatAutoFit(inner) => {
                    let min_sz = track_min_size(inner, font_size, content_width, container_height);
                    let count = ((repeat_space + col_gap) / (min_sz + col_gap)).floor() as usize;
                    let count = count.min(in_flow_indices.len()).max(1);
                    for _ in 0..count {
                        expanded.push((**inner).clone());
                    }
                }
                other => expanded.push(other.clone()),
            }
        }
        expanded
    } else if area_cols > 0 {
        vec![GridTrackSize::Fr(1.0); area_cols]
    } else {
        vec![GridTrackSize::Fr(1.0)]
    };
    let mut num_cols = template_cols.len().max(area_cols).max(1);
    let explicit_row_count = style.grid_template_rows.len().max(style.grid_template_areas.len()).max(1);
    let auto_flow = style.grid_auto_flow;
    let is_col_flow = auto_flow.is_column();
    let is_dense = auto_flow.is_dense();

    // 5. Place items into grid cells
    let mut placed_items = Vec::new();
    let mut occupied = std::collections::HashSet::<(usize, usize)>::new();
    let mut auto_row = 0usize;
    let mut auto_col = 0usize;

    for &child_idx in &in_flow_indices {
        let child = &container.children[child_idx];
        let child_style = child.style.clone().unwrap_or_default();

        let (col_explicit, col_auto_span) = match (&child_style.grid_column_start, &child_style.grid_column_end) {
            (GridPlacement::Line(start_line), GridPlacement::Line(end_line)) => {
                let s = resolve_line(*start_line, num_cols, false);
                let e = resolve_line(*end_line, num_cols, true);
                let span = if e > s { e - s } else { 1 };
                (Some((s, span)), 1)
            }
            (GridPlacement::Line(start_line), GridPlacement::Span(span_val)) => {
                let s = resolve_line(*start_line, num_cols, false);
                (Some((s, (*span_val as usize).max(1))), 1)
            }
            (GridPlacement::Line(start_line), GridPlacement::Auto) => {
                let s = resolve_line(*start_line, num_cols, false);
                (Some((s, 1)), 1)
            }
            (GridPlacement::Area(name), GridPlacement::Area(end_name)) => {
                let s = resolve_grid_line_name(name, &style.grid_column_lines, &style.grid_template_areas, true, false);
                let e = resolve_grid_line_name(end_name, &style.grid_column_lines, &style.grid_template_areas, true, true);
                if let (Some(s_idx), Some(e_idx)) = (s, e) {
                    let span = if e_idx > s_idx { e_idx - s_idx } else { 1 };
                    (Some((s_idx, span)), 1)
                } else if let Some(s_idx) = s {
                    (Some((s_idx, 1)), 1)
                } else {
                    (None, 1)
                }
            }
            (GridPlacement::Area(name), GridPlacement::Span(span_val)) => {
                let s = resolve_grid_line_name(name, &style.grid_column_lines, &style.grid_template_areas, true, false);
                if let Some(s_idx) = s {
                    (Some((s_idx, (*span_val as usize).max(1))), 1)
                } else {
                    (None, (*span_val as usize).max(1))
                }
            }
            (GridPlacement::Area(name), GridPlacement::Auto) => {
                let s = resolve_grid_line_name(name, &style.grid_column_lines, &style.grid_template_areas, true, false);
                let e = resolve_grid_line_name(name, &style.grid_column_lines, &style.grid_template_areas, true, true);
                if let (Some(s_idx), Some(e_idx)) = (s, e) {
                    let span = if e_idx > s_idx { e_idx - s_idx } else { 1 };
                    (Some((s_idx, span)), 1)
                } else if let Some(s_idx) = s {
                    (Some((s_idx, 1)), 1)
                } else {
                    (None, 1)
                }
            }
            (GridPlacement::Span(span_val), _) => (None, (*span_val as usize).max(1)),
            _ => (None, 1),
        };

        let (row_explicit, row_auto_span) = match (&child_style.grid_row_start, &child_style.grid_row_end) {
            (GridPlacement::Line(start_line), GridPlacement::Line(end_line)) => {
                let s = resolve_line(*start_line, explicit_row_count, false);
                let e = resolve_line(*end_line, explicit_row_count, true);
                let span = if e > s { e - s } else { 1 };
                (Some((s, span)), 1)
            }
            (GridPlacement::Line(start_line), GridPlacement::Span(span_val)) => {
                let s = resolve_line(*start_line, explicit_row_count, false);
                (Some((s, (*span_val as usize).max(1))), 1)
            }
            (GridPlacement::Line(start_line), GridPlacement::Auto) => {
                let s = resolve_line(*start_line, explicit_row_count, false);
                (Some((s, 1)), 1)
            }
            (GridPlacement::Area(name), GridPlacement::Area(end_name)) => {
                let s = resolve_grid_line_name(name, &style.grid_row_lines, &style.grid_template_areas, false, false);
                let e = resolve_grid_line_name(end_name, &style.grid_row_lines, &style.grid_template_areas, false, true);
                if let (Some(s_idx), Some(e_idx)) = (s, e) {
                    let span = if e_idx > s_idx { e_idx - s_idx } else { 1 };
                    (Some((s_idx, span)), 1)
                } else if let Some(s_idx) = s {
                    (Some((s_idx, 1)), 1)
                } else {
                    (None, 1)
                }
            }
            (GridPlacement::Area(name), GridPlacement::Span(span_val)) => {
                let s = resolve_grid_line_name(name, &style.grid_row_lines, &style.grid_template_areas, false, false);
                if let Some(s_idx) = s {
                    (Some((s_idx, (*span_val as usize).max(1))), 1)
                } else {
                    (None, (*span_val as usize).max(1))
                }
            }
            (GridPlacement::Area(name), GridPlacement::Auto) => {
                let s = resolve_grid_line_name(name, &style.grid_row_lines, &style.grid_template_areas, false, false);
                let e = resolve_grid_line_name(name, &style.grid_row_lines, &style.grid_template_areas, false, true);
                if let (Some(s_idx), Some(e_idx)) = (s, e) {
                    let span = if e_idx > s_idx { e_idx - s_idx } else { 1 };
                    (Some((s_idx, span)), 1)
                } else if let Some(s_idx) = s {
                    (Some((s_idx, 1)), 1)
                } else {
                    (None, 1)
                }
            }
            (GridPlacement::Span(span_val), _) => (None, (*span_val as usize).max(1)),
            _ => (None, 1),
        };

        let (col_start, col_span, row_start, row_span) = match (col_explicit, row_explicit) {
            (Some((cs, cspan)), Some((rs, rspan))) => {
                (cs, cspan, rs, rspan)
            }
            (Some((cs, cspan)), None) => {
                let rspan = row_auto_span;
                let mut r = if is_dense { 0 } else { auto_row };
                loop {
                    let can_fit = (r..r + rspan).all(|ri| {
                        (cs..cs + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                    });
                    if can_fit {
                        break;
                    }
                    r += 1;
                }
                (cs, cspan, r, rspan)
            }
            (None, Some((rs, rspan))) => {
                let cspan = col_auto_span;
                let mut c = if is_dense { 0 } else { auto_col };
                loop {
                    let can_fit = (rs..rs + rspan).all(|ri| {
                        (c..c + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                    });
                    if can_fit {
                        break;
                    }
                    c += 1;
                }
                (c, cspan, rs, rspan)
            }
            (None, None) => {
                let cspan = col_auto_span;
                let rspan = row_auto_span;
                if is_dense {
                    if is_col_flow {
                        let mut found = (0, 0);
                        'dense_col: for c in 0.. {
                            for r in 0..explicit_row_count.max(100) {
                                let can_fit = (r..r + rspan).all(|ri| {
                                    (c..c + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                                });
                                if can_fit {
                                    found = (c, r);
                                    break 'dense_col;
                                }
                            }
                        }
                        (found.0, cspan, found.1, rspan)
                    } else {
                        let mut found = (0, 0);
                        'dense_row: for r in 0.. {
                            for c in 0..num_cols {
                                if c + cspan > num_cols {
                                    continue;
                                }
                                let can_fit = (r..r + rspan).all(|ri| {
                                    (c..c + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                                });
                                if can_fit {
                                    found = (c, r);
                                    break 'dense_row;
                                }
                            }
                        }
                        (found.0, cspan, found.1, rspan)
                    }
                } else if is_col_flow {
                    let target_rows = explicit_row_count.max(1);
                    loop {
                        if auto_row + rspan > target_rows && auto_row > 0 {
                            auto_col += 1;
                            auto_row = 0;
                        }
                        let can_fit = (auto_row..auto_row + rspan).all(|ri| {
                            (auto_col..auto_col + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                        });
                        if can_fit {
                            let cs = auto_col;
                            let rs = auto_row;
                            auto_row += rspan;
                            break (cs, cspan, rs, rspan);
                        }
                        auto_row += 1;
                    }
                } else {
                    loop {
                        if auto_col + cspan > num_cols {
                            auto_row += 1;
                            auto_col = 0;
                        }
                        let can_fit = (auto_row..auto_row + rspan).all(|ri| {
                            (auto_col..auto_col + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                        });
                        if can_fit {
                            let cs = auto_col;
                            let rs = auto_row;
                            auto_col += cspan;
                            break (cs, cspan, rs, rspan);
                        }
                        auto_col += 1;
                    }
                }
            }
        };

        for r in row_start..row_start + row_span {
            for c in col_start..col_start + col_span {
                occupied.insert((r, c));
            }
        }
        num_cols = num_cols.max(col_start + col_span);

        if !is_dense {
            if is_col_flow {
                auto_col = col_start;
                auto_row = row_start + row_span;
            } else {
                auto_col = col_start + col_span;
                if auto_col >= num_cols {
                    auto_row = row_start + row_span;
                    auto_col = 0;
                } else {
                    auto_row = row_start;
                }
            }
        }

        placed_items.push(PlacedItem {
            child_idx,
            col_start,
            col_span,
            row_start,
            row_span,
        });
    }

    // Expand implicit column tracks if needed
    while template_cols.len() < num_cols {
        template_cols.push(GridTrackSize::Auto);
    }

    let area_rows = style.grid_template_areas.len();
    let num_rows = placed_items
        .iter()
        .map(|item| item.row_start + item.row_span)
        .max()
        .unwrap_or(1)
        .max(area_rows)
        .max(1);

    // 6. Track Sizing for Columns
    let total_col_gaps = (num_cols.saturating_sub(1) as f32) * col_gap;
    let available_width_for_tracks = (content_width - total_col_gaps).max(0.0);

    let mut col_widths = vec![0.0f32; num_cols];
    let mut total_fr = 0.0f32;
    let mut fixed_used = 0.0f32;

    for (c, track) in template_cols.iter().enumerate() {
        match track {
            GridTrackSize::Length(len) => {
                let px = len.to_px_with_viewport(font_size, 16.0, content_width, container_height);
                col_widths[c] = px;
                fixed_used += px;
            }
            GridTrackSize::Fr(fr) => {
                total_fr += *fr;
            }
            GridTrackSize::Subgrid => {
                total_fr += 1.0;
            }
            GridTrackSize::Auto | GridTrackSize::MinContent | GridTrackSize::MaxContent => {
                // Auto/content track: base width on max child intrinsic width in this column
                let mut max_child_w = 0.0f32;
                for item in &placed_items {
                    if item.col_start <= c && c < item.col_start + item.col_span {
                        let child = &container.children[item.child_idx];
                        let w = if let Some(cs) = &child.style && cs.width != Length::Auto {
                            cs.width.to_px_with_viewport(font_size, 16.0, content_width, container_height)
                        } else {
                            match &child.box_type {
                                crate::box_model::BoxType::TextNode(t) => {
                                    crate::inline_flow::measure_text_width(t, font_size)
                                }
                                crate::box_model::BoxType::ReplacedElement { intrinsic_width, .. }
                                | crate::box_model::BoxType::IFrame { intrinsic_width, .. }
                                | crate::box_model::BoxType::Video { intrinsic_width, .. }
                                | crate::box_model::BoxType::Audio { intrinsic_width, .. }
                                | crate::box_model::BoxType::Canvas { intrinsic_width, .. } => *intrinsic_width,
                                _ => 120.0,
                            }
                        };
                        max_child_w = max_child_w.max(w);
                    }
                }
                let resolved = max_child_w.min(available_width_for_tracks);
                col_widths[c] = resolved;
                fixed_used += resolved;
            }
            GridTrackSize::MinMax(min_t, max_t) => {
                let min_px = track_min_px(min_t, font_size, content_width, container_height, c, &placed_items, container);
                col_widths[c] = min_px;
                fixed_used += min_px;
                if let GridTrackSize::Fr(fr) = &**max_t {
                    total_fr += *fr;
                }
            }
            GridTrackSize::RepeatAutoFill(_) | GridTrackSize::RepeatAutoFit(_) => {
                // These should have been expanded in the template expansion pass above.
                // If we encounter them here, treat as auto.
                let resolved = measure_col_content_max_width(c, &placed_items, container, font_size)
                    .min(available_width_for_tracks);
                col_widths[c] = resolved;
                fixed_used += resolved;
            }
        }
    }

    // Distribute available space to non-fr MinMax tracks that can grow
    let mut remaining_for_minmax = (available_width_for_tracks - fixed_used).max(0.0);
    let minmax_growable_count = template_cols
        .iter()
        .enumerate()
        .filter(|(c, track)| {
            if let GridTrackSize::MinMax(min_t, max_t) = track {
                if let GridTrackSize::Fr(_) = &**max_t {
                    false
                } else {
                    let min_px = track_min_px(min_t, font_size, content_width, container_height, *c, &placed_items, container);
                    let max_px = match &**max_t {
                        GridTrackSize::Length(l) => l.to_px_with_viewport(font_size, 16.0, content_width, container_height),
                        GridTrackSize::MaxContent => measure_col_content_max_width(*c, &placed_items, container, font_size),
                        GridTrackSize::MinContent => measure_col_content_min_width(*c, &placed_items, container, font_size),
                        _ => min_px,
                    };
                    max_px > min_px
                }
            } else {
                false
            }
        })
        .count();

    if minmax_growable_count > 0 && remaining_for_minmax > 0.0 {
        // Distribute remaining space among growable minmax tracks up to their max_px
        for (c, track) in template_cols.iter().enumerate() {
            if let GridTrackSize::MinMax(min_t, max_t) = track {
                if let GridTrackSize::Fr(_) = &**max_t {
                    continue;
                }
                let min_px = track_min_px(min_t, font_size, content_width, container_height, c, &placed_items, container);
                let max_px = match &**max_t {
                    GridTrackSize::Length(l) => l.to_px_with_viewport(font_size, 16.0, content_width, container_height),
                    GridTrackSize::MaxContent => measure_col_content_max_width(c, &placed_items, container, font_size),
                    GridTrackSize::MinContent => measure_col_content_min_width(c, &placed_items, container, font_size),
                    _ => min_px,
                };
                let growth_capacity = (max_px - min_px).max(0.0);
                if growth_capacity > 0.0 {
                    let share = remaining_for_minmax / (minmax_growable_count as f32);
                    let add = share.min(growth_capacity);
                    col_widths[c] += add;
                    fixed_used += add;
                    remaining_for_minmax = (remaining_for_minmax - add).max(0.0);
                }
            }
        }
    }

    // Distribute remaining space to fr tracks
    if total_fr > 0.0 {
        let remaining = (available_width_for_tracks - fixed_used).max(0.0);
        for (c, track) in template_cols.iter().enumerate() {
            match track {
                GridTrackSize::Fr(fr) => {
                    col_widths[c] = (fr / total_fr) * remaining;
                }
                GridTrackSize::Subgrid => {
                    col_widths[c] = (1.0 / total_fr) * remaining;
                }
                GridTrackSize::MinMax(_, max_t) => {
                    if let GridTrackSize::Fr(fr) = &**max_t {
                        col_widths[c] += (fr / total_fr) * remaining;
                    }
                }
                _ => {}
            }
        }
    }

    // Compute column X offsets relative to container content
    let mut col_x_offsets = vec![0.0f32; num_cols];
    for c in 1..num_cols {
        col_x_offsets[c] = col_x_offsets[c - 1] + col_widths[c - 1] + col_gap;
    }

    // 7. Measure children and resolve row heights
    let mut row_heights = vec![0.0f32; num_rows];

    // Read any explicit grid-template-rows
    for (r, r_track) in style.grid_template_rows.iter().enumerate() {
        if r < num_rows
            && let GridTrackSize::Length(l) = r_track
        {
            row_heights[r] = l.to_px_with_viewport(font_size, 16.0, container_height, container_height);
        }
    }

    // Lay out each item into its cell and update row heights
    for item in &placed_items {
        let child = &mut container.children[item.child_idx];
        let child_style = child.style.clone().unwrap_or_default();
        let c_fs = child_style.font_size;

        // Compute available span width for item
        let span_w = (item.col_start..item.col_start + item.col_span)
            .map(|c| col_widths.get(c).copied().unwrap_or(0.0))
            .sum::<f32>()
            + (item.col_span.saturating_sub(1) as f32) * col_gap;

        // Child padding, border, margins
        let p_top = child_style.padding_top.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        let p_right = child_style.padding_right.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        let p_bottom = child_style.padding_bottom.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        let p_left = child_style.padding_left.to_px_with_viewport(c_fs, 16.0, span_w, container_height);

        let b_top = child_style.border_top_width;
        let b_right = child_style.border_right_width;
        let b_bottom = child_style.border_bottom_width;
        let b_left = child_style.border_left_width;

        let m_top = child_style.margin_top.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        let m_right = child_style.margin_right.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        let m_bottom = child_style.margin_bottom.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        let m_left = child_style.margin_left.to_px_with_viewport(c_fs, 16.0, span_w, container_height);

        child.dimensions.padding = EdgeSizes::new(p_top, p_right, p_bottom, p_left);
        child.dimensions.border = EdgeSizes::new(b_top, b_right, b_bottom, b_left);
        child.dimensions.margin = EdgeSizes::new(m_top, m_right, m_bottom, m_left);

        let non_content_w = p_left + p_right + b_left + b_right + m_left + m_right;
        let item_content_w = if child_style.width != Length::Auto {
            let explicit = child_style.width.to_px_with_viewport(c_fs, 16.0, span_w, container_height);
            if child_style.box_sizing == BoxSizing::BorderBox {
                (explicit - p_left - p_right - b_left - b_right).max(0.0)
            } else {
                explicit
            }
        } else {
            (span_w - non_content_w).max(0.0)
        };
        child.dimensions.content.size.width = item_content_w;

        // Recursive layout for item contents
        let item_cb = Dimensions::new(Rect::new(0.0, 0.0, item_content_w, 0.0));
        let child_display = child.style.as_ref().map(|s| s.display);
        let mut item_float_ctx = FloatContext::new();
        if child_display == Some(Display::Grid) || child_display == Some(Display::InlineGrid) {
            layout_grid(child, &item_cb, &mut item_float_ctx);
        } else if child_display == Some(Display::Flex) {
            crate::flex_flow::layout_flex(child, &item_cb, &mut item_float_ctx);
        } else if child.is_block() {
            layout_block_contents(child, &item_cb, &mut item_float_ctx);
        } else if child.is_inline() {
            crate::inline_flow::layout_inline_children(child, &mut item_float_ctx);
        }

        // Resolve height if specified or measured
        let item_outer_h = if child_style.height != Length::Auto {
            let raw_h = child_style.height.to_px_with_viewport(c_fs, 16.0, container_height, container_height);
            let content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                (raw_h - p_top - p_bottom - b_top - b_bottom).max(0.0)
            } else {
                raw_h
            };
            child.dimensions.content.size.height = content_h;
            child.dimensions.margin_box().height()
        } else {
            child.dimensions.margin_box().height()
        };

        if item.row_span == 1 && item.row_start < num_rows {
            row_heights[item.row_start] = row_heights[item.row_start].max(item_outer_h);
        }
    }

    // Second pass for multi-row items: distribute excess height if span is not already large enough
    for item in &placed_items {
        if item.row_span > 1 {
            let child = &container.children[item.child_idx];
            let item_outer_h = child.dimensions.margin_box().height();
            let current_span_h: f32 = (item.row_start..item.row_start + item.row_span)
                .map(|r| row_heights.get(r).copied().unwrap_or(0.0))
                .sum::<f32>()
                + (item.row_span.saturating_sub(1) as f32) * row_gap;
            if item_outer_h > current_span_h {
                let excess_per_row = (item_outer_h - current_span_h) / (item.row_span as f32);
                for row_h in row_heights.iter_mut().skip(item.row_start).take(item.row_span) {
                    *row_h += excess_per_row;
                }
            }
        }
    }

    // Compute row Y offsets relative to container content
    let mut row_y_offsets = vec![0.0f32; num_rows];
    for r in 1..num_rows {
        row_y_offsets[r] = row_y_offsets[r - 1] + row_heights[r - 1] + row_gap;
    }

    // 8. Position children at resolved cell origins
    for item in &placed_items {
        let child = &mut container.children[item.child_idx];
        let child_style = child.style.clone().unwrap_or_default();

        let cell_x = col_x_offsets[item.col_start];
        let cell_y = row_y_offsets[item.row_start];

        let child_x = container_x
            + cell_x
            + child.dimensions.margin.left
            + child.dimensions.border.left
            + child.dimensions.padding.left;

        let mut child_y = container_y
            + cell_y
            + child.dimensions.margin.top
            + child.dimensions.border.top
            + child.dimensions.padding.top;

        // Alignment inside cell
        let item_align = match child_style.align_self {
            AlignSelf::Auto => style.align_items,
            AlignSelf::Stretch => AlignItems::Stretch,
            AlignSelf::FlexStart => AlignItems::FlexStart,
            AlignSelf::FlexEnd => AlignItems::FlexEnd,
            AlignSelf::Center => AlignItems::Center,
            AlignSelf::Baseline => AlignItems::Baseline,
        };

        let span_cell_h = (item.row_start..item.row_start + item.row_span)
            .map(|r| row_heights.get(r).copied().unwrap_or(0.0))
            .sum::<f32>()
            + (item.row_span.saturating_sub(1) as f32) * row_gap;

        let outer_h = child.dimensions.margin_box().height();
        let remaining_h = (span_cell_h - outer_h).max(0.0);

        match item_align {
            AlignItems::Center => {
                child_y += remaining_h / 2.0;
            }
            AlignItems::FlexEnd => {
                child_y += remaining_h;
            }
            AlignItems::Stretch if child_style.height == Length::Auto => {
                let p = child.dimensions.padding;
                let b = child.dimensions.border;
                let m = child.dimensions.margin;
                child.dimensions.content.size.height = (span_cell_h - p.top - p.bottom - b.top - b.bottom - m.top - m.bottom).max(0.0);
            }
            _ => {}
        }

        let dx = child_x - child.dimensions.content.x();
        let dy = child_y - child.dimensions.content.y();
        child.dimensions.content.origin = Point::new(child_x, child_y);
        shift_descendants(child, dx, dy);
    }

    // 9. Compute container final height
    let is_percent_indefinite = matches!(style.height, Length::Percent(_))
        && containing_block.content.height() <= 0.0;
    if style.height != Length::Auto && !is_percent_indefinite {
        let raw_h = style.height.to_px_with_viewport(font_size, 16.0, container_height, container_height);
        if style.box_sizing == BoxSizing::BorderBox {
            container.dimensions.content.size.height = (raw_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0);
        } else {
            container.dimensions.content.size.height = raw_h;
        }
    } else {
        let total_grid_h = row_heights.iter().sum::<f32>() + (num_rows.saturating_sub(1) as f32) * row_gap;
        container.dimensions.content.size.height = total_grid_h.max(0.0);
    }

    // 10. Position out-of-flow children
    layout_positioned_children(container, float_ctx);
}

fn find_area_bounds(areas: &[Vec<String>], name: &str) -> Option<(usize, usize, usize, usize)> {
    let lower = name.to_ascii_lowercase();
    let mut min_r = None;
    let mut max_r = 0;
    let mut min_c = None;
    let mut max_c = 0;

    for (r, row) in areas.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if cell == &lower {
                min_r = Some(min_r.map_or(r, |mr: usize| mr.min(r)));
                max_r = max_r.max(r);
                min_c = Some(min_c.map_or(c, |mc: usize| mc.min(c)));
                max_c = max_c.max(c);
            }
        }
    }

    match (min_r, min_c) {
        (Some(r1), Some(c1)) => Some((r1, c1, max_r + 1, max_c + 1)),
        _ => None,
    }
}

/// Resolves a 1-based CSS grid line index to a 0-based track index.
/// `is_end` is accepted for call-site clarity but does not change the calculation:
/// CSS grid lines are 1-based for both start and end placements.
fn resolve_line(line: i32, num_cols: usize, _is_end: bool) -> usize {
    if line > 0 {
        ((line - 1) as usize).min(num_cols)
    } else if line < 0 {
        let idx = (num_cols as i32 + 1 + line).max(0);
        (idx as usize).min(num_cols)
    } else {
        0
    }
}

/// Resolves a named grid line or area name to a 0-based track index.
///
/// First checks explicit named lines (`grid_column_lines` / `grid_row_lines`),
/// then falls back to `grid-template-areas` to derive implicit `-start` / `-end` lines.
///
/// `is_column` selects the axis, `is_end` selects the end boundary of an area.
fn resolve_grid_line_name(
    name: &str,
    named_lines: &[(String, usize)],
    areas: &[Vec<String>],
    is_column: bool,
    is_end: bool,
) -> Option<usize> {
    let lower = name.to_ascii_lowercase();

    // 1. Check explicit named lines (e.g. [main-start] 1fr [main-end])
    for (line_name, idx) in named_lines {
        if line_name.eq_ignore_ascii_case(&lower) {
            return Some(*idx);
        }
    }

    // 2. Check for implicit lines from named areas.
    //    A named area "foo" creates implicit lines "foo-start" and "foo-end".
    //    Also, placing `grid-area: foo` uses the area bounds directly.
    let area_name = lower
        .strip_suffix("-start")
        .or_else(|| lower.strip_suffix("-end"))
        .unwrap_or(&lower);

    if let Some(bounds) = find_area_bounds(areas, area_name) {
        let (row_start, col_start, row_end, col_end) = bounds;
        if lower.ends_with("-start") {
            return Some(if is_column { col_start } else { row_start });
        } else if lower.ends_with("-end") {
            return Some(if is_column { col_end } else { row_end });
        } else {
            // Direct area name: start gets start, end gets end
            return Some(if is_end {
                if is_column { col_end } else { row_end }
            } else {
                if is_column { col_start } else { row_start }
            });
        }
    }

    None
}

/// Returns the minimum size (in px) of a track specification, used for
/// calculating how many repetitions fit in `repeat(auto-fill/auto-fit, ...)`.
fn track_min_size(
    track: &GridTrackSize,
    font_size: f32,
    container_width: f32,
    container_height: f32,
) -> f32 {
    match track {
        GridTrackSize::Length(l) => l.to_px_with_viewport(font_size, 16.0, container_width, container_height),
        GridTrackSize::MinMax(min_t, _) => track_min_size(min_t, font_size, container_width, container_height),
        GridTrackSize::Fr(_) => 0.0,
        GridTrackSize::Auto | GridTrackSize::MinContent | GridTrackSize::MaxContent => 0.0,
        GridTrackSize::Subgrid => 0.0,
        GridTrackSize::RepeatAutoFill(inner) => track_min_size(inner, font_size, container_width, container_height),
        GridTrackSize::RepeatAutoFit(inner) => track_min_size(inner, font_size, container_width, container_height),
    }
}

/// Returns the resolved minimum pixel size for a `minmax()` track's minimum component.
/// For `min-content` / `max-content`, measures the column's child content.
fn track_min_px(
    min_track: &GridTrackSize,
    font_size: f32,
    container_width: f32,
    container_height: f32,
    col: usize,
    placed_items: &[PlacedItem],
    container: &LayoutBox,
) -> f32 {
    match min_track {
        GridTrackSize::Length(l) => l.to_px_with_viewport(font_size, 16.0, container_width, container_height),
        GridTrackSize::MinContent => measure_col_content_min_width(col, placed_items, container, font_size),
        GridTrackSize::MaxContent => measure_col_content_max_width(col, placed_items, container, font_size),
        GridTrackSize::Auto => measure_col_content_min_width(col, placed_items, container, font_size),
        _ => 0.0,
    }
}

/// Measures the maximum intrinsic content width of all items placed in the given column.
fn measure_col_content_max_width(
    col: usize,
    placed_items: &[PlacedItem],
    container: &LayoutBox,
    font_size: f32,
) -> f32 {
    let mut max_w = 0.0f32;
    for item in placed_items {
        if item.col_start <= col && col < item.col_start + item.col_span {
            let child = &container.children[item.child_idx];
            let w = child_intrinsic_width(child, font_size, false);
            // For multi-column-span items, attribute a proportional share
            let share = if item.col_span > 1 { w / item.col_span as f32 } else { w };
            max_w = max_w.max(share);
        }
    }
    max_w
}

/// Measures the minimum intrinsic content width of all items placed in the given column.
fn measure_col_content_min_width(
    col: usize,
    placed_items: &[PlacedItem],
    container: &LayoutBox,
    font_size: f32,
) -> f32 {
    let mut max_w = 0.0f32;
    for item in placed_items {
        if item.col_start <= col && col < item.col_start + item.col_span {
            let child = &container.children[item.child_idx];
            let w = child_intrinsic_width(child, font_size, true);
            let share = if item.col_span > 1 { w / item.col_span as f32 } else { w };
            max_w = max_w.max(share);
        }
    }
    max_w
}

/// Returns the intrinsic width of a child layout box.
/// When `is_min` is true, returns the min-content width (longest unbreakable word);
/// when false, returns the max-content width (unconstrained line).
fn child_intrinsic_width(child: &LayoutBox, font_size: f32, is_min: bool) -> f32 {
    // If the child has an explicit width, use that
    if let Some(cs) = &child.style {
        if cs.width != Length::Auto {
            return cs.width.to_px_with_viewport(
                cs.font_size,
                16.0,
                0.0, // no percentage basis for intrinsic measurement
                0.0,
            );
        }
    }
    let fs = child.style.as_ref().map(|s| s.font_size).unwrap_or(font_size);
    match &child.box_type {
        crate::box_model::BoxType::TextNode(t) => {
            if is_min {
                // Min-content: longest single word
                t.split_whitespace()
                    .map(|word| crate::inline_flow::measure_text_width(word, fs))
                    .fold(0.0f32, f32::max)
            } else {
                crate::inline_flow::measure_text_width(t, fs)
            }
        }
        crate::box_model::BoxType::ReplacedElement { intrinsic_width, .. }
        | crate::box_model::BoxType::IFrame { intrinsic_width, .. }
        | crate::box_model::BoxType::Video { intrinsic_width, .. }
        | crate::box_model::BoxType::Audio { intrinsic_width, .. }
        | crate::box_model::BoxType::Canvas { intrinsic_width, .. } => *intrinsic_width,
        _ => {
            // For block/inline containers, recurse into children for a rough estimate
            let mut total = 0.0f32;
            for c in &child.children {
                let w = child_intrinsic_width(c, fs, is_min);
                if is_min {
                    total = total.max(w);
                } else {
                    total += w;
                }
            }
            if total > 0.0 { total } else { 120.0 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_css::ComputedStyle;

    #[test]
    fn test_grid_two_track_wikipedia_columns() {
        // Simulates Wikipedia 2-column layout: 200px sidebar TOC + 1fr main content, 20px gap
        let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut c_style = ComputedStyle::default();
        c_style.display = Display::Grid;
        c_style.width = Length::Px(600.0);
        c_style.column_gap = Length::Px(20.0);
        c_style.grid_template_columns = vec![
            GridTrackSize::Length(Length::Px(200.0)),
            GridTrackSize::Fr(1.0),
        ];
        container.style = Some(c_style);

        // Child 1: Sidebar (height: 300px)
        let mut sidebar = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s_style = ComputedStyle::default();
        s_style.height = Length::Px(300.0);
        sidebar.style = Some(s_style);

        // Child 2: Main article content (height: 500px)
        let mut main_content = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut m_style = ComputedStyle::default();
        m_style.height = Length::Px(500.0);
        main_content.style = Some(m_style);

        container.children.push(sidebar);
        container.children.push(main_content);

        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 600.0, 800.0);
        let mut float_ctx = FloatContext::new();

        layout_grid(&mut container, &containing_block, &mut float_ctx);

        let side = &container.children[0];
        let main = &container.children[1];

        // Sidebar at (0, 0), width 200, height 300
        assert_eq!(side.dimensions.content.x(), 0.0);
        assert_eq!(side.dimensions.content.y(), 0.0);
        assert_eq!(side.dimensions.content.width(), 200.0);
        assert_eq!(side.dimensions.content.height(), 300.0);

        // Main content at (200 + 20 = 220, 0), width 600 - 200 - 20 = 380, height 500
        assert_eq!(main.dimensions.content.x(), 220.0);
        assert_eq!(main.dimensions.content.y(), 0.0);
        assert_eq!(main.dimensions.content.width(), 380.0);
        assert_eq!(main.dimensions.content.height(), 500.0);

        // Container height is max(300, 500) = 500
        assert_eq!(container.dimensions.content.height(), 500.0);
    }

    #[test]
    fn test_grid_item_span_and_auto_wrapping() {
        // Grid with 2 columns (1fr 1fr), container width 400px
        let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut c_style = ComputedStyle::default();
        c_style.display = Display::Grid;
        c_style.width = Length::Px(400.0);
        c_style.row_gap = Length::Px(10.0);
        c_style.grid_template_columns = vec![
            GridTrackSize::Fr(1.0),
            GridTrackSize::Fr(1.0),
        ];
        container.style = Some(c_style);

        // Item 1: Full-width Header spanning both columns (grid-column: 1 / 3)
        let mut header = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut h_style = ComputedStyle::default();
        h_style.height = Length::Px(60.0);
        h_style.grid_column_start = GridPlacement::Line(1);
        h_style.grid_column_end = GridPlacement::Line(3);
        header.style = Some(h_style);

        // Item 2: Col 0 on Row 1
        let mut item2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut i2_style = ComputedStyle::default();
        i2_style.height = Length::Px(100.0);
        item2.style = Some(i2_style);

        // Item 3: Col 1 on Row 1
        let mut item3 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut i3_style = ComputedStyle::default();
        i3_style.height = Length::Px(120.0);
        item3.style = Some(i3_style);

        container.children.push(header);
        container.children.push(item2);
        container.children.push(item3);

        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 400.0, 600.0);
        let mut float_ctx = FloatContext::new();

        layout_grid(&mut container, &containing_block, &mut float_ctx);

        let h = &container.children[0];
        let i2 = &container.children[1];
        let i3 = &container.children[2];

        // Header spans columns 0 and 1 -> width = 400
        assert_eq!(h.dimensions.content.x(), 0.0);
        assert_eq!(h.dimensions.content.y(), 0.0);
        assert_eq!(h.dimensions.content.width(), 400.0);
        assert_eq!(h.dimensions.content.height(), 60.0);

        // Item 2 and 3 placed on row 1 (y = 60 + 10 = 70)
        assert_eq!(i2.dimensions.content.x(), 0.0);
        assert_eq!(i2.dimensions.content.y(), 70.0);
        assert_eq!(i2.dimensions.content.width(), 200.0);

        assert_eq!(i3.dimensions.content.x(), 200.0);
        assert_eq!(i3.dimensions.content.y(), 70.0);
        assert_eq!(i3.dimensions.content.width(), 200.0);

        // Total grid height = 60 (header) + 10 (gap) + 120 (max(100, 120)) = 190
        assert_eq!(container.dimensions.content.height(), 190.0);
    }

    #[test]
    fn test_grid_template_areas_named_placement() {
        let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut c_style = ComputedStyle::default();
        c_style.display = Display::Grid;
        c_style.width = Length::Px(1000.0);
        c_style.column_gap = Length::Px(20.0);
        c_style.grid_template_columns = vec![
            GridTrackSize::Fr(1.0),
            GridTrackSize::Length(Length::Px(200.0)),
        ];
        c_style.grid_template_areas = vec![
            vec!["titlebar".to_string(), "columnend".to_string()],
            vec!["content".to_string(), "columnend".to_string()],
        ];
        container.style = Some(c_style);

        // Sidebar placed in columnend
        let mut sidebar = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s_style = ComputedStyle::default();
        s_style.grid_column_start = GridPlacement::Area("columnend".to_string());
        s_style.grid_row_start = GridPlacement::Area("columnend".to_string());
        s_style.height = Length::Px(300.0);
        sidebar.style = Some(s_style);

        // Titlebar placed in titlebar
        let mut titlebar = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut t_style = ComputedStyle::default();
        t_style.grid_column_start = GridPlacement::Area("titlebar".to_string());
        t_style.grid_row_start = GridPlacement::Area("titlebar".to_string());
        t_style.height = Length::Px(50.0);
        titlebar.style = Some(t_style);

        // Main content placed in content
        let mut content = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut m_style = ComputedStyle::default();
        m_style.grid_column_start = GridPlacement::Area("content".to_string());
        m_style.grid_row_start = GridPlacement::Area("content".to_string());
        m_style.height = Length::Px(400.0);
        content.style = Some(m_style);

        // Note: added in non-DOM order to verify areas place them correctly!
        container.children.push(sidebar);
        container.children.push(titlebar);
        container.children.push(content);

        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let mut float_ctx = FloatContext::new();

        layout_grid(&mut container, &containing_block, &mut float_ctx);

        let side = &container.children[0];
        let title = &container.children[1];
        let main = &container.children[2];

        // Column widths: Col 1 is 200px, Col 0 is 1000 - 200 - 20 = 780px
        // Titlebar at (0, 0), width 780, height 50
        assert_eq!(title.dimensions.content.x(), 0.0);
        assert_eq!(title.dimensions.content.y(), 0.0);
        assert_eq!(title.dimensions.content.width(), 780.0);
        assert_eq!(title.dimensions.content.height(), 50.0);

        // Main content at (0, 50), width 780, height 400
        assert_eq!(main.dimensions.content.x(), 0.0);
        assert_eq!(main.dimensions.content.y(), 50.0);
        assert_eq!(main.dimensions.content.width(), 780.0);
        assert_eq!(main.dimensions.content.height(), 400.0);

        // Sidebar at col 1 (x = 780 + 20 = 800), y = 0, width 200, height 300
        assert_eq!(side.dimensions.content.x(), 800.0);
        assert_eq!(side.dimensions.content.y(), 0.0);
        assert_eq!(side.dimensions.content.width(), 200.0);
        assert_eq!(side.dimensions.content.height(), 300.0);
    }

    #[test]
    fn test_grid_minmax_constrained_by_content_track() {
        // Simulates Wikipedia Vector 2022 main grid:
        // grid-template-columns: minmax(0, 948px) min-content
        // Container content width: 972px, col-gap: 24px
        // Child 0 in col 0: large content
        // Child 1 in col 1: min-content sidebar (width 196px)
        let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut c_style = ComputedStyle::default();
        c_style.display = Display::Grid;
        c_style.width = Length::Px(972.0);
        c_style.column_gap = Length::Px(24.0);
        c_style.grid_template_columns = vec![
            GridTrackSize::MinMax(
                Box::new(GridTrackSize::Length(Length::Px(0.0))),
                Box::new(GridTrackSize::Length(Length::Px(948.0))),
            ),
            GridTrackSize::MinContent,
        ];
        container.style = Some(c_style);

        let mut article = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut a_style = ComputedStyle::default();
        a_style.height = Length::Px(500.0);
        article.style = Some(a_style);

        let mut appearance = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut app_style = ComputedStyle::default();
        app_style.width = Length::Px(196.0);
        app_style.height = Length::Px(300.0);
        appearance.style = Some(app_style);

        container.children.push(article);
        container.children.push(appearance);

        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 972.0, 800.0);
        let mut float_ctx = FloatContext::new();

        layout_grid(&mut container, &containing_block, &mut float_ctx);

        let art = &container.children[0];
        let app = &container.children[1];

        // Available for tracks: 972 - 24 = 948.
        // Col 1 takes 196px. Col 0 gets 948 - 196 = 752px!
        assert_eq!(art.dimensions.content.width(), 752.0);
        assert_eq!(app.dimensions.content.width(), 196.0);
        assert_eq!(app.dimensions.content.x(), 752.0 + 24.0); // 776.0
    }
}
