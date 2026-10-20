use mango_core::{EdgeSizes, Point};
use mango_css::values::{BoxSizing, Display, GridTrackSize, Length, Position};

use crate::block_flow::layout_positioned_children;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::{format_grid_item, position_grid_items_in_cells};
use super::placement::place_grid_items;
use super::tracks::{
    expand_column_templates, resolve_column_widths_and_offsets, resolve_row_heights_and_offsets,
};

/// Executes CSS Grid layout on a grid container box.
pub fn layout_grid(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    layout_grid_internal(container, containing_block, float_ctx, None, None);
}

/// Internal CSS Grid layout implementation with subgrid parent track inheritance.
pub fn layout_grid_internal(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
    parent_subgrid_cols: Option<&[f32]>,
    _parent_subgrid_rows: Option<&[f32]>,
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

    container.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    container.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);

    let margin_top =
        style
            .margin_top
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let margin_bottom =
        style
            .margin_bottom
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let is_width_auto = style.width == Length::Auto;
    let is_margin_left_auto = style.margin_left == Length::Auto;
    let is_margin_right_auto = style.margin_right == Length::Auto;

    let mut margin_left =
        style
            .margin_left
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let mut margin_right =
        style
            .margin_right
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);

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
        let raw_w =
            style
                .width
                .to_px_with_viewport(font_size, 16.0, container_width, container_height);
        let resolved_w = if style.box_sizing == BoxSizing::BorderBox {
            (raw_w - total_non_content_h).max(0.0)
        } else {
            raw_w
        };

        let underflow =
            container_width - (resolved_w + total_non_content_h + margin_left + margin_right);

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

    let raw_min_w =
        style
            .min_width
            .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let min_w = if style.box_sizing == BoxSizing::BorderBox {
        (raw_min_w - total_non_content_h).max(0.0)
    } else {
        raw_min_w
    };
    let max_w = if style.max_width != Length::Auto {
        let raw_max =
            style
                .max_width
                .to_px_with_viewport(font_size, 16.0, container_width, container_height);
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - total_non_content_h).max(0.0)
        } else {
            raw_max
        }
    } else {
        f32::INFINITY
    };
    let content_width = content_width.max(min_w).min(max_w);

    container.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    container.dimensions.content.size.width = content_width;

    // Position container in parent if not already positioned
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
        let h = if !is_height_content && !is_percent_indefinite {
            let raw_h = style.height.to_px_with_viewport(
                font_size,
                16.0,
                container_height,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (raw_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
            } else {
                raw_h
            }
        } else {
            0.0
        };
        let raw_min_h = style.min_height.to_px_with_viewport(
            font_size,
            16.0,
            container_height,
            container_height,
        );
        let min_h = if style.box_sizing == BoxSizing::BorderBox {
            (raw_min_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        } else {
            raw_min_h
        };
        let max_h = if style.max_height != Length::Auto {
            let raw_max = style.max_height.to_px_with_viewport(
                font_size,
                16.0,
                container_height,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (raw_max - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
            } else {
                raw_max
            }
        } else {
            f32::INFINITY
        };
        container.dimensions.content.size.height = h.max(min_h).min(max_h);
        return;
    }

    // 2. Resolve gaps
    let col_gap =
        style
            .column_gap
            .to_px_with_viewport(font_size, 16.0, content_width, container_height);
    let row_gap =
        style
            .row_gap
            .to_px_with_viewport(font_size, 16.0, content_width, container_height);

    // 3. Filter in-flow children and sort by order (CSS Grid §4.1)
    let mut in_flow_indices = Vec::new();
    for (idx, child) in container.children.iter().enumerate() {
        let disp = child
            .style
            .as_ref()
            .map(|s| s.display)
            .unwrap_or(Display::Block);
        let pos = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        if disp != Display::None && !matches!(pos, Position::Absolute | Position::Fixed) {
            in_flow_indices.push(idx);
        }
    }
    in_flow_indices.sort_by_key(|&idx| {
        container.children[idx]
            .style
            .as_ref()
            .map(|s| s.order)
            .unwrap_or(0)
    });

    // 4. Resolve column track templates
    let area_cols = style
        .grid_template_areas
        .iter()
        .map(|row| row.len())
        .max()
        .unwrap_or(0);

    let mut template_cols = expand_column_templates(
        &style.grid_template_columns,
        parent_subgrid_cols,
        content_width,
        container_height,
        font_size,
        col_gap,
        in_flow_indices.len(),
        area_cols,
    );

    let mut num_cols = template_cols.len().max(area_cols).max(1);
    let explicit_row_count = style
        .grid_template_rows
        .len()
        .max(style.grid_template_areas.len())
        .max(1);

    // 5. Place items into grid cells
    let (placed_items, resolved_num_cols) = place_grid_items(
        &in_flow_indices,
        container,
        &style,
        num_cols,
        explicit_row_count,
    );
    num_cols = resolved_num_cols;

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
        .max(style.grid_template_rows.len())
        .max(1);

    // 6. Track Sizing for Columns
    let (col_widths, col_x_offsets) = resolve_column_widths_and_offsets(
        &template_cols,
        &placed_items,
        container,
        num_cols,
        content_width,
        container_height,
        font_size,
        col_gap,
        style.justify_content,
    );

    // Compute span widths and subgrid-inherited tracks for each placed item
    let item_spans: Vec<(usize, f32, Vec<f32>)> = placed_items
        .iter()
        .map(|item| {
            let tracks: Vec<f32> = (item.col_start..item.col_start + item.col_span)
                .map(|c| col_widths.get(c).copied().unwrap_or(0.0))
                .collect();
            let span_w =
                tracks.iter().sum::<f32>() + (item.col_span.saturating_sub(1) as f32) * col_gap;
            (item.child_idx, span_w, tracks)
        })
        .collect();

    let item_spans_map: std::collections::HashMap<usize, (f32, Vec<f32>)> = item_spans
        .into_iter()
        .map(|(idx, w, tr)| (idx, (w, tr)))
        .collect();

    // Parallel formatting pass for independent grid cells using Rayon
    if placed_items.len() >= 2 {
        use rayon::prelude::*;
        container
            .children
            .par_iter_mut()
            .enumerate()
            .for_each(|(idx, child)| {
                if let Some((span_w, tracks)) = item_spans_map.get(&idx) {
                    format_grid_item(child, *span_w, container_height, Some(tracks));
                }
            });
    } else {
        for (idx, (span_w, tracks)) in &item_spans_map {
            if let Some(child) = container.children.get_mut(*idx) {
                format_grid_item(child, *span_w, container_height, Some(tracks));
            }
        }
    }

    // 7. Resolve row heights and offsets
    let (row_heights, row_y_offsets) = resolve_row_heights_and_offsets(
        &style.grid_template_rows,
        &placed_items,
        container,
        num_rows,
        container_height,
        font_size,
        row_gap,
        style.align_content,
    );

    // 8. Position children at resolved cell origins
    position_grid_items_in_cells(
        container,
        &placed_items,
        &col_widths,
        &row_heights,
        &col_x_offsets,
        &row_y_offsets,
        col_gap,
        row_gap,
        container_x,
        container_y,
        container_height,
        &style,
    );

    // 9. Compute container final height
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
    if !is_height_content && !is_percent_indefinite {
        let raw_h =
            style
                .height
                .to_px_with_viewport(font_size, 16.0, container_height, container_height);
        if style.box_sizing == BoxSizing::BorderBox {
            container.dimensions.content.size.height =
                (raw_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0);
        } else {
            container.dimensions.content.size.height = raw_h;
        }
    } else {
        let total_grid_h =
            row_heights.iter().sum::<f32>() + (num_rows.saturating_sub(1) as f32) * row_gap;
        container.dimensions.content.size.height = total_grid_h.max(0.0);
    }

    let raw_min_h =
        style
            .min_height
            .to_px_with_viewport(font_size, 16.0, container_height, container_height);
    let min_h = if style.box_sizing == BoxSizing::BorderBox {
        (raw_min_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
    } else {
        raw_min_h
    };
    let max_h = if style.max_height != Length::Auto {
        let raw_max = style.max_height.to_px_with_viewport(
            font_size,
            16.0,
            container_height,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        } else {
            raw_max
        }
    } else {
        f32::INFINITY
    };
    container.dimensions.content.size.height = container
        .dimensions
        .content
        .size
        .height
        .max(min_h)
        .min(max_h);

    // 10. Position out-of-flow children
    layout_positioned_children(container, float_ctx);
}
