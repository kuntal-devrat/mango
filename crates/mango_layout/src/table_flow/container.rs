use mango_core::Rect;
use mango_css::values::{
    BorderCollapse, BoxSizing, CaptionSide, Display, Length, Position, TableLayout,
};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::cells::{
    distribute_row_percentage_and_surplus, measure_and_layout_cells, position_rows_and_cells,
};
use super::collapse::resolve_collapsed_borders;
use super::columns::{compute_auto_column_widths, compute_fixed_column_widths};
use super::standalone::{
    calculate_table_position, calculate_table_width, extract_colgroup_specs, layout_standalone_row,
    layout_standalone_row_group,
};
use super::types::{
    CellRef, is_table_caption, is_table_cell, is_table_col, is_table_colgroup, is_table_row,
    is_table_row_group,
};

/// Lays out a table container box, captions, and its grid of rows and cells.
pub fn layout_table(
    table_box: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    if is_table_cell(table_box) {
        calculate_table_width(table_box, containing_block);
        let is_floated = table_box
            .style
            .as_ref()
            .map(|s| s.float != mango_css::values::Float::None)
            .unwrap_or(false);
        if !is_floated {
            let start_y = containing_block.content.y();
            calculate_table_position(table_box, containing_block, start_y, float_ctx);
        }
        crate::block_flow::layout_block_contents(table_box, containing_block, float_ctx);
        return;
    }
    if is_table_row(table_box) {
        calculate_table_width(table_box, containing_block);
        let is_floated = table_box
            .style
            .as_ref()
            .map(|s| s.float != mango_css::values::Float::None)
            .unwrap_or(false);
        if !is_floated {
            let start_y = containing_block.content.y();
            calculate_table_position(table_box, containing_block, start_y, float_ctx);
        }
        layout_standalone_row(table_box, containing_block, float_ctx);
        return;
    }
    if is_table_row_group(table_box) {
        calculate_table_width(table_box, containing_block);
        let is_floated = table_box
            .style
            .as_ref()
            .map(|s| s.float != mango_css::values::Float::None)
            .unwrap_or(false);
        if !is_floated {
            let start_y = containing_block.content.y();
            calculate_table_position(table_box, containing_block, start_y, float_ctx);
        }
        layout_standalone_row_group(table_box, containing_block, float_ctx);
        return;
    }
    if is_table_caption(table_box) || is_table_col(table_box) || is_table_colgroup(table_box) {
        calculate_table_width(table_box, containing_block);
        let is_floated = table_box
            .style
            .as_ref()
            .map(|s| s.float != mango_css::values::Float::None)
            .unwrap_or(false);
        if !is_floated {
            let start_y = containing_block.content.y();
            calculate_table_position(table_box, containing_block, start_y, float_ctx);
        }
        crate::block_flow::layout_block_contents(table_box, containing_block, float_ctx);
        return;
    }

    // 1. Calculate horizontal dimensions
    calculate_table_width(table_box, containing_block);

    // 2. Calculate vertical position (respecting clearance) if not already positioned by float or out-of-flow layout
    let is_floated = table_box
        .style
        .as_ref()
        .map(|s| s.float != mango_css::values::Float::None)
        .unwrap_or(false);
    let is_pos = table_box
        .style
        .as_ref()
        .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
        .unwrap_or(false);
    if !is_floated && !is_pos {
        let start_y = containing_block.content.y();
        calculate_table_position(table_box, containing_block, start_y, float_ctx);
    }

    // 3. Lay out table captions, rows and cells
    layout_table_contents(table_box, containing_block, float_ctx);
}

/// Main layout function orchestrating captions, rows, cells, and border collapse.
pub(crate) fn layout_table_contents(
    table_box: &mut LayoutBox,
    containing_block: &Dimensions,
    _float_ctx: &mut FloatContext,
) {
    let style = table_box.style.clone().unwrap_or_default();
    let is_collapse = style.border_collapse == BorderCollapse::Collapse;
    let is_fixed = style.table_layout == TableLayout::Fixed;

    let border_spacing = if is_collapse {
        0.0
    } else if let Some(cs) = table_box
        .get_attribute("cellspacing")
        .and_then(|s| s.trim().parse::<f32>().ok())
    {
        cs
    } else {
        style.border_spacing
    };

    let mut table_w = table_box.dimensions.content.width();
    let table_x = table_box.dimensions.content.x();
    let mut current_y = table_box.dimensions.content.y();

    // 1. Separate captions into top and bottom
    let mut top_caption_indices = Vec::new();
    let mut bottom_caption_indices = Vec::new();
    for (idx, child) in table_box.children.iter().enumerate() {
        if is_table_caption(child) {
            let side = child
                .style
                .as_ref()
                .map(|s| s.caption_side)
                .unwrap_or(style.caption_side);
            let is_bottom = side == CaptionSide::Bottom
                || child
                    .get_attribute("align")
                    .map(|a| a.trim().eq_ignore_ascii_case("bottom"))
                    .unwrap_or(false);
            if is_bottom {
                bottom_caption_indices.push(idx);
            } else {
                top_caption_indices.push(idx);
            }
        }
    }

    // Lay out top captions
    for &cap_idx in &top_caption_indices {
        let caption = &mut table_box.children[cap_idx];
        let mut caption_cb = Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
        caption_cb.content.size.width = table_w;
        let mut cap_float_ctx = FloatContext::new();
        crate::block_flow::layout_block(caption, &caption_cb, &mut cap_float_ctx);
        current_y += caption.dimensions.margin_box().height();
    }

    let grid_start_y = current_y;

    // 2. Identify `<colgroup>` and `<col>` column width specifications
    let col_specs = extract_colgroup_specs(table_box, table_w);

    // 3. Collect row references and count total columns and rows
    let has_row_groups = table_box.children.iter().any(is_table_row_group);

    // (group_idx, row_idx)
    let mut row_refs: Vec<(Option<usize>, usize)> = Vec::new();

    if has_row_groups {
        let mut thead_rows = Vec::new();
        let mut tbody_rows = Vec::new();
        let mut tfoot_rows = Vec::new();

        for (g_idx, group) in table_box.children.iter().enumerate() {
            if is_table_row_group(group) {
                let is_head = group.tag_name.as_deref() == Some("thead")
                    || group.style.as_ref().map(|s| s.display) == Some(Display::TableHeaderGroup);
                let is_foot = group.tag_name.as_deref() == Some("tfoot")
                    || group.style.as_ref().map(|s| s.display) == Some(Display::TableFooterGroup);

                for (r_idx, row) in group.children.iter().enumerate() {
                    if is_table_row(row) {
                        if is_head {
                            thead_rows.push((Some(g_idx), r_idx));
                        } else if is_foot {
                            tfoot_rows.push((Some(g_idx), r_idx));
                        } else {
                            tbody_rows.push((Some(g_idx), r_idx));
                        }
                    }
                }
            } else if is_table_row(group) {
                tbody_rows.push((None, g_idx));
            }
        }

        row_refs.extend(thead_rows);
        row_refs.extend(tbody_rows);
        row_refs.extend(tfoot_rows);
    } else {
        for (r_idx, row) in table_box.children.iter().enumerate() {
            if is_table_row(row) {
                row_refs.push((None, r_idx));
            }
        }
    }

    if row_refs.is_empty() {
        let mut max_child_w: f32 = 0.0;
        for (idx, child) in table_box.children.iter_mut().enumerate() {
            if top_caption_indices.contains(&idx) {
                continue;
            }
            if bottom_caption_indices.contains(&idx) {
                let mut caption_cb =
                    Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
                caption_cb.content.size.width = table_w;
                let mut cap_float_ctx = FloatContext::new();
                crate::block_flow::layout_block(child, &caption_cb, &mut cap_float_ctx);
                current_y += child.dimensions.margin_box().height();
                max_child_w = max_child_w.max(child.dimensions.margin_box().width());
            } else {
                let mut child_cb = Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
                child_cb.content.size.width = table_w;
                let mut child_float_ctx = FloatContext::new();
                crate::block_flow::layout_block(child, &child_cb, &mut child_float_ctx);
                current_y += child.dimensions.margin_box().height();
                let cw = child.dimensions.border_box().width();
                max_child_w = max_child_w.max(cw);
            }
        }
        let is_auto_table_width =
            style.width == Length::Auto && table_box.get_attribute("width").is_none();
        if is_auto_table_width && max_child_w > 0.0 {
            table_box.dimensions.content.size.width = max_child_w;
        }
        let font_size = style.font_size;
        let (_, vp_h) = mango_css::get_current_viewport();
        let explicit_h = if style.height != Length::Auto {
            let h_px = style
                .height
                .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
            if style.box_sizing == BoxSizing::BorderBox {
                let border_v = style.border_top_width + style.border_bottom_width;
                let pad_v = style
                    .padding_top
                    .to_px_with_viewport(font_size, 16.0, table_w, vp_h)
                    + style
                        .padding_bottom
                        .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
                (h_px - border_v - pad_v).max(0.0)
            } else {
                h_px
            }
        } else {
            0.0
        };
        let raw_h = (current_y - table_box.dimensions.content.y()).max(
            if max_child_w > 0.0 || explicit_h > 0.0 {
                explicit_h
            } else {
                10.0
            },
        );
        let min_h = style
            .min_height
            .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
        let mut final_h = raw_h.max(explicit_h).max(min_h);
        let max_h = style
            .max_height
            .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
        if max_h > 0.0 && style.max_height != Length::Auto {
            final_h = final_h.min(max_h);
        }
        table_box.dimensions.content.size.height = final_h;
        apply_table_relative_offsets(table_box);
        return;
    }

    // Determine number of columns and build initial 2D cell placements
    let mut num_cols = col_specs.len();
    let num_rows = row_refs.len();

    // Preliminary pass to find max columns
    {
        let mut active_rowspans: Vec<usize> = Vec::new();
        for &(g_idx, r_idx) in &row_refs {
            let row = if let Some(gi) = g_idx {
                &table_box.children[gi].children[r_idx]
            } else {
                &table_box.children[r_idx]
            };
            let mut col_idx = 0;
            for cell in &row.children {
                if is_table_cell(cell) {
                    while col_idx < active_rowspans.len() && active_rowspans[col_idx] > 0 {
                        col_idx += 1;
                    }
                    let c_span = cell
                        .get_attribute("colspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    let r_span = cell
                        .get_attribute("rowspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);

                    if r_span > 1 {
                        while active_rowspans.len() < col_idx + c_span {
                            active_rowspans.push(0);
                        }
                        for k in 0..c_span {
                            active_rowspans[col_idx + k] = r_span;
                        }
                    }
                    col_idx += c_span;
                }
            }
            num_cols = num_cols.max(col_idx);
            for rem in active_rowspans.iter_mut() {
                if *rem > 0 {
                    *rem -= 1;
                }
            }
        }
    }

    num_cols = num_cols.max(1);

    // Build complete 2D grid matrix of CellRefs
    let mut grid: Vec<Vec<Option<CellRef>>> = vec![vec![None; num_cols]; num_rows];
    let mut all_cells: Vec<CellRef> = Vec::new();

    for (grid_r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
        let row = if let Some(gi) = g_idx {
            &table_box.children[gi].children[r_idx]
        } else {
            &table_box.children[r_idx]
        };

        let mut grid_c = 0;
        for (cell_idx, cell) in row.children.iter().enumerate() {
            if is_table_cell(cell) {
                while grid_c < num_cols && grid[grid_r][grid_c].is_some() {
                    grid_c += 1;
                }
                if grid_c >= num_cols {
                    break;
                }

                let c_span = cell
                    .get_attribute("colspan")
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let r_span = cell
                    .get_attribute("rowspan")
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);

                let actual_c_span = c_span.min(num_cols - grid_c);
                let actual_r_span = r_span.min(num_rows - grid_r);

                let cell_ref = CellRef {
                    group_idx: g_idx,
                    row_idx: r_idx,
                    cell_idx,
                    start_row: grid_r,
                    start_col: grid_c,
                    row_span: actual_r_span,
                    col_span: actual_c_span,
                };
                all_cells.push(cell_ref);

                for ro in 0..actual_r_span {
                    for co in 0..actual_c_span {
                        if grid_r + ro < num_rows && grid_c + co < num_cols {
                            grid[grid_r + ro][grid_c + co] = Some(cell_ref);
                        }
                    }
                }

                grid_c += actual_c_span;
            }
        }
    }

    // 4. Column Width Calculation (Fixed vs Auto)
    let total_spacing = (num_cols + 1) as f32 * border_spacing;
    let available_w = (table_w - total_spacing).max(0.0);

    let col_widths = if is_fixed {
        compute_fixed_column_widths(num_cols, &col_specs, &row_refs, table_box, available_w)
    } else {
        compute_auto_column_widths(
            num_cols,
            &col_specs,
            &all_cells,
            table_box,
            table_w,
            available_w,
            border_spacing,
        )
    };

    let actual_table_w = col_widths.iter().sum::<f32>() + total_spacing;
    let is_auto_table_width =
        style.width == Length::Auto && table_box.get_attribute("width").is_none();
    let new_table_w = if is_auto_table_width {
        // Shrink-to-fit, but never below the table's own min-width or above
        // its max-width (CSS 2.1 §17.5.2).
        let fs = style.font_size;
        let rfs = style.root_font_size;
        let min_w = if style.min_width != Length::Auto {
            style.min_width.to_px_with_viewport(fs, rfs, table_w, 600.0)
        } else {
            0.0
        };
        let max_w = if style.max_width != Length::Auto {
            style.max_width.to_px_with_viewport(fs, rfs, table_w, 600.0)
        } else {
            f32::INFINITY
        };
        actual_table_w.max(min_w).min(max_w)
    } else {
        // A specified width may still be widened by columns whose min-content
        // widths cannot be satisfied (CSS 2.1 §17.5.2).
        table_w.max(actual_table_w)
    };
    if (new_table_w - table_w).abs() > 0.01 {
        table_w = new_table_w;
        table_box.dimensions.content.size.width = table_w;

        // Re-layout top captions with updated table width
        let mut re_y = table_box.dimensions.content.y();
        for &cap_idx in &top_caption_indices {
            let caption = &mut table_box.children[cap_idx];
            let mut caption_cb = Dimensions::new(Rect::new(table_x, re_y, table_w, 10000.0));
            caption_cb.content.size.width = table_w;
            let mut cap_float_ctx = FloatContext::new();
            crate::block_flow::layout_block(caption, &caption_cb, &mut cap_float_ctx);
            re_y += caption.dimensions.margin_box().height();
        }
    }

    // 5. Collapsed Border Resolution Model (CSS 2.1 §17.6.2)
    let collapsed_borders = resolve_collapsed_borders(
        is_collapse,
        table_box,
        num_rows,
        num_cols,
        &grid,
        &all_cells,
    );

    // 6. Measure cell heights and compute natural row heights
    let (mut row_natural_heights, cell_layouts) = measure_and_layout_cells(
        table_box,
        &row_refs,
        &all_cells,
        &col_widths,
        num_rows,
        border_spacing,
        &collapsed_borders,
    );

    // 7. Table row/cell percentage heights and vertical surplus distribution
    let top_captions_h = grid_start_y - table_box.dimensions.content.y();
    distribute_row_percentage_and_surplus(
        table_box,
        containing_block,
        &row_refs,
        &mut row_natural_heights,
        top_captions_h,
        border_spacing,
        num_rows,
    );

    // 8. Position rows and cells in 2D space and apply vertical alignment
    let row_y = position_rows_and_cells(
        table_box,
        &row_refs,
        &all_cells,
        &cell_layouts,
        &row_natural_heights,
        &col_widths,
        num_rows,
        table_x,
        table_w,
        grid_start_y,
        border_spacing,
        has_row_groups,
    );

    current_y = row_y;

    // 9. Lay out bottom captions
    for &cap_idx in &bottom_caption_indices {
        let caption = &mut table_box.children[cap_idx];
        let mut caption_cb = Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
        caption_cb.content.size.width = table_w;
        let mut cap_float_ctx = FloatContext::new();
        crate::block_flow::layout_block(caption, &caption_cb, &mut cap_float_ctx);
        current_y += caption.dimensions.margin_box().height();
    }

    let font_size = style.font_size;
    let (_, vp_h) = mango_css::get_current_viewport();
    let explicit_h = if style.height != Length::Auto {
        let h_px = style
            .height
            .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
        if style.box_sizing == BoxSizing::BorderBox {
            let border_v = style.border_top_width + style.border_bottom_width;
            let pad_v = style
                .padding_top
                .to_px_with_viewport(font_size, 16.0, table_w, vp_h)
                + style
                    .padding_bottom
                    .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
            (h_px - border_v - pad_v).max(0.0)
        } else {
            h_px
        }
    } else {
        0.0
    };
    let raw_h = (current_y - table_box.dimensions.content.y()).max(if explicit_h > 0.0 {
        explicit_h
    } else {
        10.0
    });
    let min_h = style
        .min_height
        .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
    let mut final_h = raw_h.max(explicit_h).max(min_h);
    let max_h = style
        .max_height
        .to_px_with_viewport(font_size, 16.0, table_w, vp_h);
    if max_h > 0.0 && style.max_height != Length::Auto {
        final_h = final_h.min(max_h);
    }
    table_box.dimensions.content.size.height = final_h;

    // 10. Apply relative positioning offsets to table children (CSS Position 3 § 5.1)
    apply_table_relative_offsets(table_box);
}

fn apply_table_relative_offsets(table_box: &mut LayoutBox) {
    let table_dims = table_box.dimensions;
    for child in &mut table_box.children {
        if is_table_caption(child) {
            if child.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
                crate::block_flow::apply_relative_offset(child, &table_dims);
            }
        } else if is_table_row_group(child) {
            if child.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
                crate::block_flow::apply_relative_offset(child, &table_dims);
            }
            let group_dims = child.dimensions;
            for row in &mut child.children {
                if is_table_row(row) {
                    if row.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
                        crate::block_flow::apply_relative_offset(row, &group_dims);
                    }
                    let row_dims = row.dimensions;
                    for cell in &mut row.children {
                        if is_table_cell(cell) {
                            if cell.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
                                crate::block_flow::apply_relative_offset(cell, &row_dims);
                            }
                        }
                    }
                }
            }
        } else if is_table_row(child) {
            if child.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
                crate::block_flow::apply_relative_offset(child, &table_dims);
            }
            let row_dims = child.dimensions;
            for cell in &mut child.children {
                if is_table_cell(cell) {
                    if cell.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
                        crate::block_flow::apply_relative_offset(cell, &row_dims);
                    }
                }
            }
        }
    }
}
