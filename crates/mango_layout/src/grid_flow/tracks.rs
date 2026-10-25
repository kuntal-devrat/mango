use mango_css::values::{AlignContent, GridTrackSize, JustifyContent, Length};

use crate::box_tree::LayoutBox;

use super::sizing::{
    measure_col_content_max_width, measure_col_content_min_width, track_min_px, track_min_size,
};
use super::types::PlacedItem;

pub fn expand_column_templates(
    template_columns: &[GridTrackSize],
    parent_subgrid_cols: Option<&[f32]>,
    content_width: f32,
    container_height: f32,
    font_size: f32,
    col_gap: f32,
    in_flow_count: usize,
    area_cols: usize,
) -> Vec<GridTrackSize> {
    let is_subgrid_cols = parent_subgrid_cols.is_some()
        && template_columns
            .iter()
            .any(|t| matches!(t, GridTrackSize::Subgrid));

    if is_subgrid_cols {
        if let Some(cols) = parent_subgrid_cols {
            cols.iter()
                .map(|&w| GridTrackSize::Length(Length::Px(w)))
                .collect()
        } else {
            Vec::new()
        }
    } else if !template_columns.is_empty() {
        let mut expanded = Vec::new();
        let mut non_repeat_fixed = 0.0f32;
        let mut non_repeat_count = 0usize;
        for t in template_columns {
            match t {
                GridTrackSize::RepeatAutoFill(_) | GridTrackSize::RepeatAutoFit(_) => {}
                GridTrackSize::Length(l) => {
                    non_repeat_fixed +=
                        l.to_px_with_viewport(font_size, 16.0, content_width, container_height);
                    non_repeat_count += 1;
                }
                _ => {
                    non_repeat_count += 1;
                }
            }
        }
        let repeat_space =
            (content_width - non_repeat_fixed - (non_repeat_count as f32) * col_gap).max(0.0);

        for t in template_columns {
            match t {
                GridTrackSize::RepeatAutoFill(inner) => {
                    let min_sz = track_min_size(inner, font_size, content_width, container_height);
                    let step = (min_sz + col_gap).max(1.0);
                    let count = ((repeat_space + col_gap) / step).floor() as usize;
                    let count = count.max(1);
                    for _ in 0..count {
                        expanded.push((**inner).clone());
                    }
                }
                GridTrackSize::RepeatAutoFit(inner) => {
                    let min_sz = track_min_size(inner, font_size, content_width, container_height);
                    let step = (min_sz + col_gap).max(1.0);
                    let count = ((repeat_space + col_gap) / step).floor() as usize;
                    let count = count.min(in_flow_count).max(1);
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
    }
}

pub fn resolve_column_widths_and_offsets(
    template_cols: &[GridTrackSize],
    placed_items: &[PlacedItem],
    container: &LayoutBox,
    num_cols: usize,
    content_width: f32,
    container_height: f32,
    font_size: f32,
    col_gap: f32,
    justify_content: JustifyContent,
) -> (Vec<f32>, Vec<f32>) {
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
            GridTrackSize::MinContent => {
                let resolved = measure_col_content_min_width(c, placed_items, container, font_size)
                    .min(available_width_for_tracks);
                col_widths[c] = resolved;
                fixed_used += resolved;
            }
            GridTrackSize::Auto | GridTrackSize::MaxContent => {
                let resolved = measure_col_content_max_width(c, placed_items, container, font_size)
                    .min(available_width_for_tracks);
                col_widths[c] = resolved;
                fixed_used += resolved;
            }
            GridTrackSize::MinMax(min_t, max_t) => {
                let min_px = track_min_px(
                    min_t,
                    font_size,
                    content_width,
                    container_height,
                    c,
                    placed_items,
                    container,
                );
                col_widths[c] = min_px;
                fixed_used += min_px;
                if let GridTrackSize::Fr(fr) = &**max_t {
                    total_fr += *fr;
                }
            }
            GridTrackSize::RepeatAutoFill(_) | GridTrackSize::RepeatAutoFit(_) => {
                let resolved = measure_col_content_max_width(c, placed_items, container, font_size)
                    .min(available_width_for_tracks);
                col_widths[c] = resolved;
                fixed_used += resolved;
            }
        }
    }

    let mut remaining_for_minmax = (available_width_for_tracks - fixed_used).max(0.0);
    let minmax_growable_count = template_cols
        .iter()
        .enumerate()
        .filter(|(c, track)| {
            if let GridTrackSize::MinMax(min_t, max_t) = track {
                if let GridTrackSize::Fr(_) = &**max_t {
                    false
                } else {
                    let min_px = track_min_px(
                        min_t,
                        font_size,
                        content_width,
                        container_height,
                        *c,
                        placed_items,
                        container,
                    );
                    let max_px = match &**max_t {
                        GridTrackSize::Length(l) => {
                            l.to_px_with_viewport(font_size, 16.0, content_width, container_height)
                        }
                        GridTrackSize::Auto | GridTrackSize::MaxContent => {
                            measure_col_content_max_width(*c, placed_items, container, font_size)
                        }
                        GridTrackSize::MinContent => {
                            measure_col_content_min_width(*c, placed_items, container, font_size)
                        }
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
        for (c, track) in template_cols.iter().enumerate() {
            if let GridTrackSize::MinMax(min_t, max_t) = track {
                if let GridTrackSize::Fr(_) = &**max_t {
                    continue;
                }
                let min_px = track_min_px(
                    min_t,
                    font_size,
                    content_width,
                    container_height,
                    c,
                    placed_items,
                    container,
                );
                let max_px = match &**max_t {
                    GridTrackSize::Length(l) => {
                        l.to_px_with_viewport(font_size, 16.0, content_width, container_height)
                    }
                    GridTrackSize::Auto | GridTrackSize::MaxContent => {
                        measure_col_content_max_width(c, placed_items, container, font_size)
                    }
                    GridTrackSize::MinContent => {
                        measure_col_content_min_width(c, placed_items, container, font_size)
                    }
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

    let total_col_tracks_w: f32 = col_widths.iter().sum::<f32>() + total_col_gaps;
    let free_col_space = (content_width - total_col_tracks_w).max(0.0);

    let (col_start_x, extra_col_gap) = match justify_content {
        JustifyContent::Center => (free_col_space / 2.0, 0.0),
        JustifyContent::FlexEnd => (free_col_space, 0.0),
        JustifyContent::SpaceBetween => {
            if num_cols > 1 {
                (0.0, free_col_space / (num_cols - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            let gap_share = free_col_space / num_cols as f32;
            (gap_share / 2.0, gap_share)
        }
        JustifyContent::SpaceEvenly => {
            let gap_share = free_col_space / (num_cols + 1) as f32;
            (gap_share, gap_share)
        }
        _ => (0.0, 0.0),
    };

    let mut col_x_offsets = vec![0.0f32; num_cols];
    if num_cols > 0 {
        col_x_offsets[0] = col_start_x;
        for c in 1..num_cols {
            col_x_offsets[c] = col_x_offsets[c - 1] + col_widths[c - 1] + col_gap + extra_col_gap;
        }
    }

    (col_widths, col_x_offsets)
}

pub fn resolve_row_heights_and_offsets(
    grid_template_rows: &[GridTrackSize],
    placed_items: &[PlacedItem],
    container: &LayoutBox,
    num_rows: usize,
    container_height: f32,
    font_size: f32,
    row_gap: f32,
    align_content: AlignContent,
) -> (Vec<f32>, Vec<f32>) {
    let mut row_heights = vec![0.0f32; num_rows];

    for (r, r_track) in grid_template_rows.iter().enumerate() {
        if r < num_rows
            && let GridTrackSize::Length(l) = r_track
        {
            row_heights[r] =
                l.to_px_with_viewport(font_size, 16.0, container_height, container_height);
        }
    }

    let total_row_gaps = (num_rows.saturating_sub(1) as f32) * row_gap;
    let fixed_row_total: f32 = row_heights.iter().sum();
    let total_row_fr: f32 = grid_template_rows
        .iter()
        .enumerate()
        .filter_map(|(r, t)| {
            if r < num_rows {
                match t {
                    GridTrackSize::Fr(fr) => Some(*fr),
                    GridTrackSize::Subgrid => Some(1.0),
                    _ => None,
                }
            } else {
                None
            }
        })
        .sum();

    if total_row_fr > 0.0 && container_height > 0.0 {
        let remaining_h = (container_height - fixed_row_total - total_row_gaps).max(0.0);
        for (r, r_track) in grid_template_rows.iter().enumerate() {
            if r < num_rows {
                match r_track {
                    GridTrackSize::Fr(fr) => {
                        row_heights[r] = (fr / total_row_fr) * remaining_h;
                    }
                    GridTrackSize::Subgrid => {
                        row_heights[r] = (1.0 / total_row_fr) * remaining_h;
                    }
                    _ => {}
                }
            }
        }
    }

    for item in placed_items {
        let child = &container.children[item.child_idx];
        let item_outer_h = child.dimensions.margin_box().height();
        if item.row_span == 1 && item.row_start < num_rows {
            row_heights[item.row_start] = row_heights[item.row_start].max(item_outer_h);
        }
    }

    for item in placed_items {
        if item.row_span > 1 {
            let child = &container.children[item.child_idx];
            let item_outer_h = child.dimensions.margin_box().height();
            let current_span_h: f32 = (item.row_start..item.row_start + item.row_span)
                .map(|r| row_heights.get(r).copied().unwrap_or(0.0))
                .sum::<f32>()
                + (item.row_span.saturating_sub(1) as f32) * row_gap;
            if item_outer_h > current_span_h {
                let excess_per_row = (item_outer_h - current_span_h) / (item.row_span as f32);
                for row_h in row_heights
                    .iter_mut()
                    .skip(item.row_start)
                    .take(item.row_span)
                {
                    *row_h += excess_per_row;
                }
            }
        }
    }

    let total_row_tracks_h: f32 =
        row_heights.iter().sum::<f32>() + (num_rows.saturating_sub(1) as f32) * row_gap;
    let free_row_space = if container_height > 0.0 {
        (container_height - total_row_tracks_h).max(0.0)
    } else {
        0.0
    };

    let (row_start_y, extra_row_gap) = match align_content {
        AlignContent::Center => (free_row_space / 2.0, 0.0),
        AlignContent::FlexEnd => (free_row_space, 0.0),
        AlignContent::SpaceBetween => {
            if num_rows > 1 {
                (0.0, free_row_space / (num_rows - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        AlignContent::SpaceAround => {
            let gap_share = free_row_space / num_rows as f32;
            (gap_share / 2.0, gap_share)
        }
        AlignContent::SpaceEvenly => {
            let gap_share = free_row_space / (num_rows + 1) as f32;
            (gap_share, gap_share)
        }
        _ => (0.0, 0.0),
    };

    let mut row_y_offsets = vec![0.0f32; num_rows];
    if num_rows > 0 {
        row_y_offsets[0] = row_start_y;
        for r in 1..num_rows {
            row_y_offsets[r] = row_y_offsets[r - 1] + row_heights[r - 1] + row_gap + extra_row_gap;
        }
    }

    (row_heights, row_y_offsets)
}
