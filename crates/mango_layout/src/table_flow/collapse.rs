use std::collections::HashMap;

use mango_core::Color;
use mango_css::values::BorderStyle;

use crate::box_tree::LayoutBox;

use super::types::{BorderSide, CellRef, resolve_border_conflict};

pub type CollapsedBorderMap =
    HashMap<(usize, usize, usize), (BorderSide, BorderSide, BorderSide, BorderSide)>;

pub fn resolve_collapsed_borders(
    is_collapse: bool,
    table_box: &mut LayoutBox,
    num_rows: usize,
    num_cols: usize,
    grid: &[Vec<Option<CellRef>>],
    all_cells: &[CellRef],
) -> CollapsedBorderMap {
    if !is_collapse {
        return HashMap::new();
    }

    let extract_cell_border = |cell: &LayoutBox, side: &str| -> BorderSide {
        let s = cell.style.as_ref();
        let (w, st, c) = match side {
            "top" => (
                s.map_or(0.0, |st| st.border_top_width),
                s.map_or(BorderStyle::None, |st| st.border_top_style),
                s.map_or(Color::BLACK, |st| st.border_top_color),
            ),
            "bottom" => (
                s.map_or(0.0, |st| st.border_bottom_width),
                s.map_or(BorderStyle::None, |st| st.border_bottom_style),
                s.map_or(Color::BLACK, |st| st.border_bottom_color),
            ),
            "left" => (
                s.map_or(0.0, |st| st.border_left_width),
                s.map_or(BorderStyle::None, |st| st.border_left_style),
                s.map_or(Color::BLACK, |st| st.border_left_color),
            ),
            "right" => (
                s.map_or(0.0, |st| st.border_right_width),
                s.map_or(BorderStyle::None, |st| st.border_right_style),
                s.map_or(Color::BLACK, |st| st.border_right_color),
            ),
            _ => (0.0, BorderStyle::None, Color::BLACK),
        };
        BorderSide {
            width: w,
            style: st,
            color: c,
            precedence: 4,
        }
    };

    let style = table_box.style.clone().unwrap_or_default();
    let table_border_top = BorderSide {
        width: table_box.dimensions.border.top,
        style: style.border_top_style,
        color: style.border_top_color,
        precedence: 1,
    };
    let table_border_bottom = BorderSide {
        width: table_box.dimensions.border.bottom,
        style: style.border_bottom_style,
        color: style.border_bottom_color,
        precedence: 1,
    };
    let table_border_left = BorderSide {
        width: table_box.dimensions.border.left,
        style: style.border_left_style,
        color: style.border_left_color,
        precedence: 1,
    };
    let table_border_right = BorderSide {
        width: table_box.dimensions.border.right,
        style: style.border_right_style,
        color: style.border_right_color,
        precedence: 1,
    };

    // Horizontal edges: between row r and row r+1 (size: (num_rows - 1) x num_cols)
    let mut horiz_edges: Vec<Vec<BorderSide>> =
        vec![vec![BorderSide::default(); num_cols]; num_rows.saturating_sub(1)];
    for r in 0..num_rows.saturating_sub(1) {
        for c in 0..num_cols {
            if let (Some(top_cell_ref), Some(bot_cell_ref)) = (grid[r][c], grid[r + 1][c])
                && (top_cell_ref.start_row != bot_cell_ref.start_row
                    || top_cell_ref.start_col != bot_cell_ref.start_col)
            {
                let top_cell = if let Some(gi) = top_cell_ref.group_idx {
                    &table_box.children[gi].children[top_cell_ref.row_idx].children
                        [top_cell_ref.cell_idx]
                } else {
                    &table_box.children[top_cell_ref.row_idx].children[top_cell_ref.cell_idx]
                };
                let bot_cell = if let Some(gi) = bot_cell_ref.group_idx {
                    &table_box.children[gi].children[bot_cell_ref.row_idx].children
                        [bot_cell_ref.cell_idx]
                } else {
                    &table_box.children[bot_cell_ref.row_idx].children[bot_cell_ref.cell_idx]
                };
                let side_top = extract_cell_border(top_cell, "bottom");
                let side_bot = extract_cell_border(bot_cell, "top");
                horiz_edges[r][c] = resolve_border_conflict(side_top, side_bot);
            }
        }
    }

    // Vertical edges: between col c and col c+1 (size: num_rows x (num_cols - 1))
    let mut vert_edges: Vec<Vec<BorderSide>> =
        vec![vec![BorderSide::default(); num_cols.saturating_sub(1)]; num_rows];
    for r in 0..num_rows {
        for c in 0..num_cols.saturating_sub(1) {
            if let (Some(left_cell_ref), Some(right_cell_ref)) = (grid[r][c], grid[r][c + 1])
                && (left_cell_ref.start_row != right_cell_ref.start_row
                    || left_cell_ref.start_col != right_cell_ref.start_col)
            {
                let left_cell = if let Some(gi) = left_cell_ref.group_idx {
                    &table_box.children[gi].children[left_cell_ref.row_idx].children
                        [left_cell_ref.cell_idx]
                } else {
                    &table_box.children[left_cell_ref.row_idx].children[left_cell_ref.cell_idx]
                };
                let right_cell = if let Some(gi) = right_cell_ref.group_idx {
                    &table_box.children[gi].children[right_cell_ref.row_idx].children
                        [right_cell_ref.cell_idx]
                } else {
                    &table_box.children[right_cell_ref.row_idx].children[right_cell_ref.cell_idx]
                };
                let side_left = extract_cell_border(left_cell, "right");
                let side_right = extract_cell_border(right_cell, "left");
                vert_edges[r][c] = resolve_border_conflict(side_left, side_right);
            }
        }
    }

    let mut map = HashMap::new();
    for r in 0..num_rows {
        for c in 0..num_cols {
            if let Some(cell_ref) = grid[r][c]
                && r == cell_ref.start_row
                && c == cell_ref.start_col
            {
                let cell = if let Some(gi) = cell_ref.group_idx {
                    &table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
                } else {
                    &table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
                };
                let bot_boundary_r = cell_ref.start_row + cell_ref.row_span - 1;
                let bottom = if bot_boundary_r < num_rows.saturating_sub(1) {
                    horiz_edges[bot_boundary_r][cell_ref.start_col]
                } else if table_border_bottom.width > 0.0 {
                    let current = extract_cell_border(cell, "bottom");
                    resolve_border_conflict(current, table_border_bottom)
                } else {
                    extract_cell_border(cell, "bottom")
                };
                let top = if cell_ref.start_row > 0 {
                    BorderSide::default()
                } else if table_border_top.width > 0.0 {
                    let current = extract_cell_border(cell, "top");
                    resolve_border_conflict(current, table_border_top)
                } else {
                    extract_cell_border(cell, "top")
                };
                let right_boundary_c = cell_ref.start_col + cell_ref.col_span - 1;
                let right = if right_boundary_c < num_cols.saturating_sub(1) {
                    vert_edges[cell_ref.start_row][right_boundary_c]
                } else if table_border_right.width > 0.0 {
                    let current = extract_cell_border(cell, "right");
                    resolve_border_conflict(current, table_border_right)
                } else {
                    extract_cell_border(cell, "right")
                };
                let left = if cell_ref.start_col > 0 {
                    BorderSide::default()
                } else if table_border_left.width > 0.0 {
                    let current = extract_cell_border(cell, "left");
                    resolve_border_conflict(current, table_border_left)
                } else {
                    extract_cell_border(cell, "left")
                };
                map.insert(
                    (
                        cell_ref.group_idx.unwrap_or(usize::MAX),
                        cell_ref.row_idx,
                        cell_ref.cell_idx,
                    ),
                    (top, right, bottom, left),
                );
            }
        }
    }
    // Seed dimensions with collapsed widths.
    for cell_ref in all_cells {
        let key = (
            cell_ref.group_idx.unwrap_or(usize::MAX),
            cell_ref.row_idx,
            cell_ref.cell_idx,
        );
        if let Some((top, right, bottom, left)) = map.get(&key) {
            let cell = if let Some(gi) = cell_ref.group_idx {
                &mut table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
            } else {
                &mut table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
            };
            cell.dimensions.border.top = top.width;
            cell.dimensions.border.right = right.width;
            cell.dimensions.border.bottom = bottom.width;
            cell.dimensions.border.left = left.width;
        }
    }
    map
}
