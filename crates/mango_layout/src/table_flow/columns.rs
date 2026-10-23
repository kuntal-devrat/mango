use mango_css::values::Length;

use crate::box_tree::LayoutBox;

use super::intrinsic::cell_intrinsic_metrics;
use super::types::{CellRef, is_table_cell};

pub fn compute_fixed_column_widths(
    num_cols: usize,
    col_specs: &[Option<f32>],
    row_refs: &[(Option<usize>, usize)],
    table_box: &LayoutBox,
    available_w: f32,
) -> Vec<f32> {
    let mut fixed_widths = vec![None::<f32>; num_cols];

    for i in 0..num_cols {
        if let Some(w) = col_specs.get(i).copied().flatten() {
            fixed_widths[i] = Some(w);
        }
    }

    // Check first row's cells for remaining unassigned columns
    if !row_refs.is_empty() {
        let &(g0, r0) = &row_refs[0];
        let first_row = if let Some(gi) = g0 {
            &table_box.children[gi].children[r0]
        } else {
            &table_box.children[r0]
        };
        let mut c_idx = 0;
        for cell in &first_row.children {
            if is_table_cell(cell) && c_idx < num_cols {
                let span = cell
                    .get_attribute("colspan")
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let actual_span = span.min(num_cols - c_idx);

                let mut explicit_w = None;
                if let Some(s) = &cell.style {
                    match s.width {
                        Length::Px(px) if px > 0.0 => explicit_w = Some(px),
                        Length::Percent(pct) if pct > 0.0 => {
                            explicit_w = Some(available_w * (pct / 100.0))
                        }
                        _ => {}
                    }
                }
                if explicit_w.is_none()
                    && let Some(w_attr) = cell.get_attribute("width")
                {
                    let w_str = w_attr.trim();
                    if let Some(pct_str) = w_str.strip_suffix('%') {
                        if let Ok(pct) = pct_str.trim().parse::<f32>() {
                            explicit_w = Some(available_w * (pct / 100.0));
                        }
                    } else {
                        let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
                        if let Ok(px) = num_str.parse::<f32>()
                            && px > 0.0
                        {
                            explicit_w = Some(px);
                        }
                    }
                }

                if let Some(ew) = explicit_w {
                    let per_col_w = ew / actual_span as f32;
                    for k in 0..actual_span {
                        if fixed_widths[c_idx + k].is_none() {
                            fixed_widths[c_idx + k] = Some(per_col_w);
                        }
                    }
                }
                c_idx += actual_span;
            }
        }
    }

    let assigned_sum: f32 = fixed_widths.iter().filter_map(|&w| w).sum();
    let unassigned_count = fixed_widths.iter().filter(|w| w.is_none()).count();
    let remaining_w = (available_w - assigned_sum).max(0.0);
    let equal_share = if unassigned_count > 0 {
        remaining_w / unassigned_count as f32
    } else {
        0.0
    };

    let mut col_widths = vec![0.0f32; num_cols];
    for i in 0..num_cols {
        col_widths[i] = fixed_widths[i].unwrap_or(equal_share).max(1.0);
    }
    col_widths
}

pub fn compute_auto_column_widths(
    num_cols: usize,
    col_specs: &[Option<f32>],
    all_cells: &[CellRef],
    table_box: &LayoutBox,
    table_w: f32,
    available_w: f32,
    border_spacing: f32,
) -> Vec<f32> {
    let mut col_min_widths = vec![4.0f32; num_cols];
    let mut col_max_widths = vec![4.0f32; num_cols];
    let mut col_explicit_widths = vec![None::<f32>; num_cols];
    let mut intrinsic_memo = std::collections::HashMap::new();

    // Seed with <col> specs
    for i in 0..num_cols {
        if let Some(w) = col_specs.get(i).copied().flatten() {
            col_min_widths[i] = col_min_widths[i].max(w);
            col_max_widths[i] = col_max_widths[i].max(w);
            col_explicit_widths[i] = Some(w);
        }
    }

    // Pass 1: single-column cells
    for cell_ref in all_cells {
        if cell_ref.col_span == 1 {
            let cell = if let Some(gi) = cell_ref.group_idx {
                &table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
            } else {
                &table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
            };
            let (c_min, c_max, c_exp) = cell_intrinsic_metrics(cell, table_w, &mut intrinsic_memo);
            let c_idx = cell_ref.start_col;
            col_min_widths[c_idx] = col_min_widths[c_idx].max(c_min);
            col_max_widths[c_idx] = col_max_widths[c_idx].max(c_max);
            if let Some(ew) = c_exp {
                col_explicit_widths[c_idx] =
                    Some(col_explicit_widths[c_idx].unwrap_or(0.0).max(ew));
            }
        }
    }

    // Pass 2: multi-column cells
    for cell_ref in all_cells {
        if cell_ref.col_span > 1 {
            let cell = if let Some(gi) = cell_ref.group_idx {
                &table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
            } else {
                &table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
            };
            let (c_min, c_max, _) = cell_intrinsic_metrics(cell, table_w, &mut intrinsic_memo);
            let c_idx = cell_ref.start_col;
            let actual_span = cell_ref.col_span;
            let internal_spacing = (actual_span - 1) as f32 * border_spacing;

            let span_min_sum: f32 = (0..actual_span)
                .map(|k| col_min_widths[c_idx + k])
                .sum::<f32>()
                + internal_spacing;
            if c_min > span_min_sum {
                let diff = (c_min - span_min_sum) / actual_span as f32;
                for k in 0..actual_span {
                    col_min_widths[c_idx + k] += diff;
                }
            }

            let span_max_sum: f32 = (0..actual_span)
                .map(|k| col_max_widths[c_idx + k])
                .sum::<f32>()
                + internal_spacing;
            if c_max > span_max_sum {
                let diff = (c_max - span_max_sum) / actual_span as f32;
                for k in 0..actual_span {
                    col_max_widths[c_idx + k] += diff;
                }
            }
        }
    }

    for i in 0..num_cols {
        col_min_widths[i] = col_min_widths[i].max(4.0);
        col_max_widths[i] = col_max_widths[i].max(col_min_widths[i]);
    }

    let sum_min: f32 = col_min_widths.iter().sum();
    let sum_max: f32 = col_max_widths.iter().sum();
    let mut col_widths = vec![0.0f32; num_cols];

    if available_w <= sum_min {
        for i in 0..num_cols {
            col_widths[i] = col_min_widths[i];
        }
    } else if available_w <= sum_max {
        let extra = available_w - sum_min;
        let diff_sum = (sum_max - sum_min).max(1.0);
        for i in 0..num_cols {
            let col_diff = (col_max_widths[i] - col_min_widths[i]).max(0.0);
            col_widths[i] = (col_min_widths[i] + extra * (col_diff / diff_sum)).max(1.0);
        }
    } else {
        // CSS 2.1 §17.5.2: If width: auto and no explicit HTML width, table does not expand beyond max-width
        let is_auto_table_width = table_box
            .style
            .as_ref()
            .map(|s| s.width == mango_css::values::Length::Auto)
            .unwrap_or(true)
            && table_box.get_attribute("width").is_none();

        if is_auto_table_width {
            for i in 0..num_cols {
                col_widths[i] = col_max_widths[i];
            }
        } else {
            let surplus = available_w - sum_max;
            let num_auto = col_explicit_widths.iter().filter(|w| w.is_none()).count();
            if num_auto > 0 {
                let auto_max_sum: f32 = col_max_widths
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| col_explicit_widths[*i].is_none())
                    .map(|(_, m)| *m)
                    .sum();
                for i in 0..num_cols {
                    if col_explicit_widths[i].is_none() {
                        let share = if auto_max_sum > 0.0 {
                            col_max_widths[i] / auto_max_sum
                        } else {
                            1.0 / num_auto as f32
                        };
                        col_widths[i] = (col_max_widths[i] + surplus * share).max(1.0);
                    } else {
                        col_widths[i] = col_max_widths[i].max(1.0);
                    }
                }
            } else if sum_max > 0.0 {
                for i in 0..num_cols {
                    col_widths[i] =
                        (col_max_widths[i] + surplus * (col_max_widths[i] / sum_max)).max(1.0);
                }
            } else {
                let eq = (available_w / num_cols as f32).max(1.0);
                for i in 0..num_cols {
                    col_widths[i] = eq;
                }
            }
        }
    }
    col_widths
}
