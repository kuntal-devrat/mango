use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::{BoxSizing, Length};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::types::{is_table_cell, is_table_col, is_table_colgroup, is_table_row};

pub fn calculate_table_width(table_box: &mut LayoutBox, containing_block: &Dimensions) {
    let style = table_box.style.clone().unwrap_or_default();
    let container_width = containing_block.content.width();
    let container_height = if containing_block.content.height() > 0.0 {
        containing_block.content.height()
    } else {
        mango_css::get_current_viewport().1
    };
    let font_size = style.font_size;
    let pad_top =
        style
            .padding_top
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_right =
        style
            .padding_right
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_bottom = style.padding_bottom.to_px_with_viewport(
        font_size,
        16.0,
        container_width,
        container_height,
    );
    let pad_left =
        style
            .padding_left
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    table_box.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    table_box.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);

    let margin_top =
        style
            .margin_top
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let margin_bottom =
        style
            .margin_bottom
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let mut margin_left =
        style
            .margin_left
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let mut margin_right =
        style
            .margin_right
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let total_non_content_h = pad_left + pad_right + border_left + border_right;

    let mut explicit_width = None;
    if let Some(w_attr) = table_box.get_attribute("width") {
        let w_str = w_attr.trim();
        if let Some(pct_str) = w_str.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>() {
                explicit_width = Some(container_width * (pct / 100.0));
            }
        } else {
            let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
            if let Ok(px) = num_str.parse::<f32>()
                && px > 0.0
            {
                explicit_width = Some(px);
            }
        }
    }

    let mut content_width = match style.width {
        Length::Auto => {
            if let Some(w) = explicit_width {
                match style.box_sizing {
                    BoxSizing::ContentBox => w,
                    BoxSizing::BorderBox => (w - total_non_content_h).max(0.0),
                }
            } else {
                (container_width - total_non_content_h - margin_left - margin_right).max(0.0)
            }
        }
        ref len => {
            let px = len.to_px_with_viewport(font_size, 16.0, container_width, container_height);
            match style.box_sizing {
                BoxSizing::ContentBox => px,
                BoxSizing::BorderBox => (px - total_non_content_h).max(0.0),
            }
        }
    };

    let min_w =
        style
            .min_width
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    if min_w > 0.0 {
        let min_content_w = match style.box_sizing {
            BoxSizing::ContentBox => min_w,
            BoxSizing::BorderBox => (min_w - total_non_content_h).max(0.0),
        };
        content_width = content_width.max(min_content_w);
    }
    let max_w =
        style
            .max_width
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    if max_w > 0.0 && style.max_width != Length::Auto {
        let max_content_w = match style.box_sizing {
            BoxSizing::ContentBox => max_w,
            BoxSizing::BorderBox => (max_w - total_non_content_h).max(0.0),
        };
        content_width = content_width.min(max_content_w);
    }

    if style.margin_left == Length::Auto && style.margin_right == Length::Auto {
        let remaining = (container_width - content_width - total_non_content_h).max(0.0);
        margin_left = remaining / 2.0;
        margin_right = remaining / 2.0;
    } else if style.margin_left == Length::Auto {
        let remaining =
            (container_width - content_width - total_non_content_h - margin_right).max(0.0);
        margin_left = remaining;
    } else if style.margin_right == Length::Auto {
        let remaining =
            (container_width - content_width - total_non_content_h - margin_left).max(0.0);
        margin_right = remaining;
    }

    table_box.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    table_box.dimensions.content.size.width = content_width;
}

pub fn calculate_table_position(
    table_box: &mut LayoutBox,
    containing_block: &Dimensions,
    start_y: f32,
    _float_ctx: &mut FloatContext,
) {
    let x = containing_block.content.x()
        + table_box.dimensions.margin.left
        + table_box.dimensions.border.left
        + table_box.dimensions.padding.left;

    let y = start_y
        + table_box.dimensions.margin.top
        + table_box.dimensions.border.top
        + table_box.dimensions.padding.top;

    table_box.dimensions.content.origin = Point::new(x, y);
}

/// Lays out a standalone `table-row` root: cells share the row width equally
/// (respecting explicit widths), are positioned side-by-side, and the row
/// height encloses the tallest cell (CSS 2.1 §17.5).
pub fn layout_standalone_row(
    row_box: &mut LayoutBox,
    containing_block: &Dimensions,
    _float_ctx: &mut FloatContext,
) {
    let row_w = row_box.dimensions.content.width().max(1.0);
    let row_x = row_box.dimensions.content.x();
    let row_y = row_box.dimensions.content.y();
    let n = row_box
        .children
        .iter()
        .filter(|c| is_table_cell(c))
        .count()
        .max(1);
    let cell_w = (row_w / n as f32).max(1.0);
    let mut max_h = 0.0f32;
    let mut cur_x = row_x;
    for cell in row_box.children.iter_mut() {
        if !is_table_cell(cell) {
            // Non-cell children (e.g. stray text) are laid out as blocks.
            let mut cb = Dimensions::new(Rect::new(cur_x, row_y, cell_w, 0.0));
            cb.content.size.width = cell_w;
            let mut fctx = FloatContext::new();
            crate::block_flow::layout_block_contents(cell, &cb, &mut fctx);
            cell.dimensions.content.origin.x =
                cur_x + cell.dimensions.padding.left + cell.dimensions.border.left;
            cell.dimensions.content.origin.y =
                row_y + cell.dimensions.padding.top + cell.dimensions.border.top;
            max_h = max_h.max(cell.dimensions.margin_box().height());
            cur_x += cell_w;
            continue;
        }
        let mut cb = Dimensions::new(Rect::new(0.0, 0.0, cell_w, 0.0));
        cb.content.size.width = cell_w;
        let mut fctx = FloatContext::new();
        // Use block-contents directly to avoid re-entering table dispatch.
        // First resolve the cell's own box, then its contents.
        let cs = cell.style.clone().unwrap_or_default();
        let fs = cs.font_size;
        let rfs = cs.root_font_size;
        cell.dimensions.padding = EdgeSizes::new(
            cs.padding_top.to_px_with_viewport(fs, rfs, cell_w, 0.0),
            cs.padding_right.to_px_with_viewport(fs, rfs, cell_w, 0.0),
            cs.padding_bottom.to_px_with_viewport(fs, rfs, cell_w, 0.0),
            cs.padding_left.to_px_with_viewport(fs, rfs, cell_w, 0.0),
        );
        cell.dimensions.border = EdgeSizes::new(
            cs.border_top_width,
            cs.border_right_width,
            cs.border_bottom_width,
            cs.border_left_width,
        );
        cell.dimensions.margin = EdgeSizes::new(
            cs.margin_top.to_px_with_viewport(fs, rfs, cell_w, 0.0),
            cs.margin_right.to_px_with_viewport(fs, rfs, cell_w, 0.0),
            cs.margin_bottom.to_px_with_viewport(fs, rfs, cell_w, 0.0),
            cs.margin_left.to_px_with_viewport(fs, rfs, cell_w, 0.0),
        );
        let non_content = cell.dimensions.padding.left
            + cell.dimensions.padding.right
            + cell.dimensions.border.left
            + cell.dimensions.border.right;
        cell.dimensions.content.size.width = (cell_w - non_content).max(1.0);
        cell.dimensions.content.origin = Point::new(
            cur_x
                + cell.dimensions.margin.left
                + cell.dimensions.border.left
                + cell.dimensions.padding.left,
            row_y
                + cell.dimensions.margin.top
                + cell.dimensions.border.top
                + cell.dimensions.padding.top,
        );
        crate::block_flow::layout_block_contents(cell, &cb, &mut fctx);
        // Re-anchor after contents changed height.
        cell.dimensions.content.origin.x = cur_x
            + cell.dimensions.margin.left
            + cell.dimensions.border.left
            + cell.dimensions.padding.left;
        max_h = max_h.max(cell.dimensions.margin_box().height());
        cur_x += cell_w;
    }
    // Stretch cells to the row height.
    for cell in row_box.children.iter_mut() {
        let non_v = cell.dimensions.padding.top
            + cell.dimensions.padding.bottom
            + cell.dimensions.border.top
            + cell.dimensions.border.bottom;
        let target = (max_h - cell.dimensions.margin.top - cell.dimensions.margin.bottom - non_v)
            .max(cell.dimensions.content.height())
            .max(0.0);
        cell.dimensions.content.size.height = target;
    }
    row_box.dimensions.content.size.width = row_w;
    row_box.dimensions.content.size.height = max_h.max(1.0);
    let _ = containing_block;
}

/// Lays out a standalone row-group root by stacking its rows vertically.
pub fn layout_standalone_row_group(
    group_box: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    let group_x = group_box.dimensions.content.x();
    let mut cur_y = group_box.dimensions.content.y();
    let group_w = group_box.dimensions.content.width().max(1.0);
    let mut max_w = group_w;
    for row in group_box.children.iter_mut() {
        if !is_table_row(row) {
            let mut cb = Dimensions::new(Rect::new(group_x, cur_y, group_w, 0.0));
            cb.content.size.width = group_w;
            crate::block_flow::layout_block_contents(row, &cb, float_ctx);
            row.dimensions.content.origin.x = group_x;
            cur_y += row.dimensions.margin_box().height();
            max_w = max_w.max(row.dimensions.margin_box().width());
            continue;
        }
        // Resolve row box then delegate to standalone row layout.
        let cs = row.style.clone().unwrap_or_default();
        row.dimensions.content.origin = Point::new(group_x, cur_y);
        row.dimensions.content.size.width = group_w;
        let mut row_cb = Dimensions::new(Rect::new(group_x, cur_y, group_w, 0.0));
        row_cb.content.size.width = group_w;
        layout_standalone_row(row, &row_cb, float_ctx);
        // Re-anchor row at the stacked position.
        let dx = group_x - row.dimensions.content.x();
        let dy = cur_y - row.dimensions.content.y();
        row.dimensions.content.origin.x += dx;
        row.dimensions.content.origin.y += dy;
        crate::block_flow::shift_descendants(row, dx, dy);
        let _ = cs;
        cur_y += row.dimensions.margin_box().height();
    }
    group_box.dimensions.content.size.height = (cur_y - group_box.dimensions.content.y()).max(1.0);
    group_box.dimensions.content.size.width = max_w;
    let _ = containing_block;
}

/// Extracts `<colgroup>` and `<col>` column width specifications.
pub fn extract_colgroup_specs(table_box: &LayoutBox, table_width: f32) -> Vec<Option<f32>> {
    let mut specs = Vec::new();

    let parse_w = |node: &LayoutBox| -> Option<f32> {
        if let Some(style) = &node.style {
            match style.width {
                Length::Px(px) if px > 0.0 => return Some(px),
                Length::Percent(pct) if pct > 0.0 => return Some(table_width * (pct / 100.0)),
                _ => {}
            }
        }
        if let Some(w_attr) = node.get_attribute("width") {
            let w_str = w_attr.trim();
            if let Some(pct_str) = w_str.strip_suffix('%') {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    return Some(table_width * (pct / 100.0));
                }
            } else {
                let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
                if let Ok(px) = num_str.parse::<f32>()
                    && px > 0.0
                {
                    return Some(px);
                }
            }
        }
        None
    };

    let parse_span = |node: &LayoutBox| -> usize {
        node.get_attribute("span")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .max(1)
    };

    for child in &table_box.children {
        if is_table_col(child) {
            let span = parse_span(child);
            let w = parse_w(child);
            for _ in 0..span {
                specs.push(w);
            }
        } else if is_table_colgroup(child) {
            let col_children: Vec<&LayoutBox> =
                child.children.iter().filter(|c| is_table_col(c)).collect();
            if !col_children.is_empty() {
                for col in col_children {
                    let span = parse_span(col);
                    let w = parse_w(col).or_else(|| parse_w(child));
                    for _ in 0..span {
                        specs.push(w);
                    }
                }
            } else {
                let span = parse_span(child);
                let w = parse_w(child);
                for _ in 0..span {
                    specs.push(w);
                }
            }
        }
    }

    specs
}
