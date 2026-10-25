use std::collections::HashMap;

use mango_core::{Point, Rect};
use mango_css::values::{Length, VerticalAlign};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::collapse::CollapsedBorderMap;
use super::types::{CellRef, is_table_row, is_table_row_group};

pub fn measure_and_layout_cells(
    table_box: &mut LayoutBox,
    row_refs: &[(Option<usize>, usize)],
    all_cells: &[CellRef],
    col_widths: &[f32],
    num_rows: usize,
    border_spacing: f32,
    collapsed_borders: &CollapsedBorderMap,
) -> (Vec<f32>, HashMap<(usize, usize, usize), (f32, f32)>) {
    let mut row_natural_heights = vec![0.0f32; num_rows];
    let mut cell_layouts: HashMap<(usize, usize, usize), (f32, f32)> = HashMap::new();

    // Check specified row heights (from row style or row height attribute)
    for (r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
        let row = if let Some(gi) = g_idx {
            &table_box.children[gi].children[r_idx]
        } else {
            &table_box.children[r_idx]
        };
        let mut spec_h = None;
        if let Some(s) = &row.style {
            match s.height {
                Length::Px(px) if px >= 0.0 => spec_h = Some(px),
                _ => {}
            }
        }
        if spec_h.is_none()
            && let Some(h_attr) = row.get_attribute("height")
        {
            let h_str = h_attr.trim();
            let num_str = h_str.strip_suffix("px").unwrap_or(h_str).trim();
            if let Ok(px) = num_str.parse::<f32>()
                && px >= 0.0
            {
                spec_h = Some(px);
            }
        }
        if let Some(sh) = spec_h {
            row_natural_heights[r] = row_natural_heights[r].max(sh);
        }
    }

    // Layout single-row cells first to determine row heights.
    for cell_ref in all_cells {
        let col_start = cell_ref.start_col;
        let c_span = cell_ref.col_span;
        let spanned_w: f32 = (0..c_span).map(|k| col_widths[col_start + k]).sum();
        let intervening_spacing = (c_span.saturating_sub(1)) as f32 * border_spacing;
        let cell_w = spanned_w + intervening_spacing;

        let definite_row_h = row_natural_heights
            .get(cell_ref.start_row)
            .copied()
            .unwrap_or(0.0);
        let cb_h = if definite_row_h > 0.0 {
            definite_row_h
        } else {
            0.0
        };

        let cell = if let Some(gi) = cell_ref.group_idx {
            &mut table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
        } else {
            &mut table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
        };

        let mut cell_cb = Dimensions::new(Rect::new(0.0, 0.0, cell_w, cb_h));
        cell_cb.content.size.width = cell_w;
        cell_cb.content.size.height = cb_h;
        let mut cell_float_ctx = FloatContext::new();
        crate::block_flow::layout_block(cell, &cell_cb, &mut cell_float_ctx);

        let cell_key = (
            cell_ref.group_idx.unwrap_or(usize::MAX),
            cell_ref.row_idx,
            cell_ref.cell_idx,
        );
        if let Some((top, right, bottom, left)) = collapsed_borders.get(&cell_key) {
            cell.dimensions.border.top = top.width;
            cell.dimensions.border.right = right.width;
            cell.dimensions.border.bottom = bottom.width;
            cell.dimensions.border.left = left.width;
        }

        let cell_total_h = cell.dimensions.margin_box().height();
        cell_layouts.insert(cell_key, (cell_w, cell_total_h));

        if cell_ref.row_span == 1 {
            row_natural_heights[cell_ref.start_row] =
                row_natural_heights[cell_ref.start_row].max(cell_total_h);
        }
    }

    // Distribute shortfall for multi-row cells (rowspan > 1)
    for cell_ref in all_cells {
        if cell_ref.row_span > 1 {
            let cell_key = (
                cell_ref.group_idx.unwrap_or(usize::MAX),
                cell_ref.row_idx,
                cell_ref.cell_idx,
            );
            let Some(&(_, cell_total_h)) = cell_layouts.get(&cell_key) else {
                continue;
            };

            let r_start = cell_ref.start_row;
            let r_span = cell_ref.row_span;
            let current_spanned_h: f32 = (0..r_span)
                .map(|k| row_natural_heights[r_start + k])
                .sum::<f32>()
                + (r_span - 1) as f32 * border_spacing;

            if cell_total_h > current_spanned_h {
                let diff = (cell_total_h - current_spanned_h) / r_span as f32;
                for k in 0..r_span {
                    row_natural_heights[r_start + k] += diff;
                }
            }
        }
    }

    (row_natural_heights, cell_layouts)
}

pub fn distribute_row_percentage_and_surplus(
    table_box: &LayoutBox,
    containing_block: &Dimensions,
    row_refs: &[(Option<usize>, usize)],
    row_natural_heights: &mut [f32],
    top_captions_h: f32,
    border_spacing: f32,
    num_rows: usize,
) {
    let mut explicit_table_h = None;
    if let Some(s) = &table_box.style {
        match s.height {
            Length::Px(px) if px > 0.0 => explicit_table_h = Some(px),
            Length::Percent(pct) if pct > 0.0 => {
                let cb_h = containing_block.content.height();
                if cb_h > 0.0 {
                    explicit_table_h = Some(cb_h * (pct / 100.0));
                }
            }
            _ => {}
        }
    }
    if explicit_table_h.is_none()
        && let Some(h_attr) = table_box.get_attribute("height")
    {
        let h_str = h_attr.trim();
        if let Some(pct_str) = h_str.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>() {
                let cb_h = containing_block.content.height();
                if cb_h > 0.0 {
                    explicit_table_h = Some(cb_h * (pct / 100.0));
                }
            }
        } else {
            let num_str = h_str.strip_suffix("px").unwrap_or(h_str).trim();
            if let Ok(px) = num_str.parse::<f32>()
                && px > 0.0
            {
                explicit_table_h = Some(px);
            }
        }
    }

    let total_v_spacing = (num_rows + 1) as f32 * border_spacing;

    if let Some(target_table_h) = explicit_table_h {
        let available_grid_h = (target_table_h - top_captions_h - total_v_spacing).max(0.0);

        // Resolve row percentage heights against available_grid_h
        for (r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
            let row = if let Some(gi) = g_idx {
                &table_box.children[gi].children[r_idx]
            } else {
                &table_box.children[r_idx]
            };
            if let Some(s) = &row.style
                && let Length::Percent(pct) = s.height
            {
                let row_pct_h = available_grid_h * (pct / 100.0);
                row_natural_heights[r] = row_natural_heights[r].max(row_pct_h);
            }
        }

        let sum_rows: f32 = row_natural_heights.iter().sum();
        if sum_rows < available_grid_h && !row_natural_heights.is_empty() {
            let surplus = available_grid_h - sum_rows;
            if sum_rows > 0.0 {
                for r in 0..num_rows {
                    row_natural_heights[r] += surplus * (row_natural_heights[r] / sum_rows);
                }
            } else {
                let eq = surplus / num_rows as f32;
                for r in 0..num_rows {
                    row_natural_heights[r] += eq;
                }
            }
        }
    }
}

pub fn position_rows_and_cells(
    table_box: &mut LayoutBox,
    row_refs: &[(Option<usize>, usize)],
    all_cells: &[CellRef],
    cell_layouts: &HashMap<(usize, usize, usize), (f32, f32)>,
    row_natural_heights: &[f32],
    col_widths: &[f32],
    num_rows: usize,
    table_x: f32,
    table_w: f32,
    grid_start_y: f32,
    border_spacing: f32,
    has_row_groups: bool,
) -> f32 {
    let mut row_y = grid_start_y + border_spacing;
    let mut row_positions_y = Vec::with_capacity(num_rows);

    for r in 0..num_rows {
        row_positions_y.push(row_y);
        row_y += row_natural_heights[r] + border_spacing;
    }

    // Set row dimensions
    for (r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
        let row = if let Some(gi) = g_idx {
            &mut table_box.children[gi].children[r_idx]
        } else {
            &mut table_box.children[r_idx]
        };
        row.dimensions.content.origin = Point::new(table_x, row_positions_y[r]);
        row.dimensions.content.size.width = table_w;
        row.dimensions.content.size.height = row_natural_heights[r];
    }

    // Set rowgroup dimensions
    if has_row_groups {
        for group in &mut table_box.children {
            if is_table_row_group(group) {
                let first_row_y = group
                    .children
                    .iter()
                    .filter(|c| is_table_row(c))
                    .map(|c| c.dimensions.content.origin.y)
                    .next()
                    .unwrap_or(grid_start_y);
                let last_row_bottom = group
                    .children
                    .iter()
                    .filter(|c| is_table_row(c))
                    .map(|c| c.dimensions.content.origin.y + c.dimensions.content.size.height)
                    .next_back()
                    .unwrap_or(first_row_y);

                group.dimensions.content.origin = Point::new(table_x, first_row_y);
                group.dimensions.content.size.width = table_w;
                group.dimensions.content.size.height = last_row_bottom - first_row_y;
            }
        }
    }

    // Position and stretch every cell
    for cell_ref in all_cells {
        let cell_key = (
            cell_ref.group_idx.unwrap_or(usize::MAX),
            cell_ref.row_idx,
            cell_ref.cell_idx,
        );
        let Some(&(cell_w, _)) = cell_layouts.get(&cell_key) else {
            continue;
        };

        let c_start = cell_ref.start_col;
        let r_start = cell_ref.start_row;
        let r_span = cell_ref.row_span;

        let cell_x = table_x
            + border_spacing
            + (0..c_start)
                .map(|k| col_widths[k] + border_spacing)
                .sum::<f32>();
        let cell_y = row_positions_y[r_start];

        let spanned_h: f32 = (0..r_span)
            .map(|k| row_natural_heights[r_start + k])
            .sum::<f32>()
            + (r_span.saturating_sub(1)) as f32 * border_spacing;

        let cell = if let Some(gi) = cell_ref.group_idx {
            &mut table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
        } else {
            &mut table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
        };

        let old_x = cell.dimensions.content.origin.x;
        let old_y = cell.dimensions.content.origin.y;

        let new_x = cell_x + cell.dimensions.padding.left + cell.dimensions.border.left;
        let new_y = cell_y + cell.dimensions.padding.top + cell.dimensions.border.top;

        let dx = new_x - old_x;
        let dy = new_y - old_y;

        cell.dimensions.content.origin.x = new_x;
        cell.dimensions.content.origin.y = new_y;
        cell.dimensions.content.size.width = (cell_w
            - cell.dimensions.padding.left
            - cell.dimensions.padding.right
            - cell.dimensions.border.left
            - cell.dimensions.border.right)
            .max(1.0);

        let non_content_v = cell.dimensions.padding.top
            + cell.dimensions.padding.bottom
            + cell.dimensions.border.top
            + cell.dimensions.border.bottom;
        let allocated_h = (spanned_h - non_content_v).max(0.0);
        let natural_h = cell.dimensions.content.size.height;
        let extra_h = (allocated_h - natural_h).max(0.0);

        cell.dimensions.content.size.height = allocated_h.max(1.0);

        // Vertical alignment inside table cell
        let mut va_dy = 0.0;
        if extra_h > 0.5 {
            let va = cell
                .style
                .as_ref()
                .map(|s| s.vertical_align)
                .unwrap_or(VerticalAlign::Middle);

            va_dy = match va {
                VerticalAlign::Top => 0.0,
                VerticalAlign::Middle => extra_h / 2.0,
                VerticalAlign::Bottom => extra_h,
                VerticalAlign::Baseline => 0.0,
                _ => 0.0,
            };
        }

        if dx.abs() > 0.001 || (dy + va_dy).abs() > 0.001 {
            crate::block_flow::shift_descendants(cell, dx, dy + va_dy);
        }
    }

    row_y
}
