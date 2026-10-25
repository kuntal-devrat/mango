use mango_core::{EdgeSizes, Point, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::{
    AlignItems, AlignSelf, BoxSizing, Display, JustifyItems, JustifySelf, Length,
};

use crate::block_flow::{layout_block_contents, shift_descendants};
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::container::layout_grid_internal;
use super::types::PlacedItem;

/// Formats a single grid item within its assigned cell span width.
pub fn format_grid_item(
    child: &mut LayoutBox,
    span_w: f32,
    container_height: f32,
    parent_subgrid_cols: Option<&[f32]>,
) -> f32 {
    let child_style = child.style.clone().unwrap_or_default();
    let c_fs = child_style.font_size;

    // Child padding, border, margins
    let p_top = child_style
        .padding_top
        .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let p_right =
        child_style
            .padding_right
            .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let p_bottom =
        child_style
            .padding_bottom
            .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let p_left = child_style
        .padding_left
        .to_px_with_viewport(c_fs, 16.0, span_w, container_height);

    let b_top = child_style.border_top_width;
    let b_right = child_style.border_right_width;
    let b_bottom = child_style.border_bottom_width;
    let b_left = child_style.border_left_width;

    let m_top = child_style
        .margin_top
        .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let m_right =
        child_style
            .margin_right
            .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let m_bottom =
        child_style
            .margin_bottom
            .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let m_left = child_style
        .margin_left
        .to_px_with_viewport(c_fs, 16.0, span_w, container_height);

    child.dimensions.padding = EdgeSizes::new(p_top, p_right, p_bottom, p_left);
    child.dimensions.border = EdgeSizes::new(b_top, b_right, b_bottom, b_left);
    child.dimensions.margin = EdgeSizes::new(m_top, m_right, m_bottom, m_left);

    let non_content_w = p_left + p_right + b_left + b_right + m_left + m_right;
    let item_content_w = if child_style.width != Length::Auto {
        let explicit = child_style
            .width
            .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
        if child_style.box_sizing == BoxSizing::BorderBox {
            (explicit - p_left - p_right - b_left - b_right).max(0.0)
        } else {
            explicit
        }
    } else {
        (span_w - non_content_w).max(0.0)
    };
    let min_w = child_style
        .min_width
        .to_px_with_viewport(c_fs, 16.0, span_w, container_height);
    let max_w = if child_style.max_width != Length::Auto {
        child_style
            .max_width
            .to_px_with_viewport(c_fs, 16.0, span_w, container_height)
    } else {
        f32::INFINITY
    };
    let item_content_w = item_content_w.max(min_w).min(max_w);
    child.dimensions.content.size.width = item_content_w;
    child.is_independent_formatting_context = true;

    // Recursive layout for item contents
    let item_cb = Dimensions::new(Rect::new(0.0, 0.0, item_content_w, 0.0));
    let child_display = child.style.as_ref().map(|s| s.display);
    let mut item_float_ctx = FloatContext::new();
    if child_display == Some(Display::Grid) || child_display == Some(Display::InlineGrid) {
        layout_grid_internal(
            child,
            &item_cb,
            &mut item_float_ctx,
            parent_subgrid_cols,
            None,
        );
    } else if child_display == Some(Display::Flex) {
        crate::flex_flow::layout_flex(child, &item_cb, &mut item_float_ctx);
    } else if child.is_block() {
        layout_block_contents(child, &item_cb, &mut item_float_ctx);
    } else if child.is_inline() {
        crate::inline_flow::layout_inline_children(child, &mut item_float_ctx);
    }

    // Resolve height if specified or measured
    if child_style.height != Length::Auto {
        let raw_h =
            child_style
                .height
                .to_px_with_viewport(c_fs, 16.0, container_height, container_height);
        let content_h = if child_style.box_sizing == BoxSizing::BorderBox {
            (raw_h - p_top - p_bottom - b_top - b_bottom).max(0.0)
        } else {
            raw_h
        };
        let min_h = child_style.min_height.to_px_with_viewport(
            c_fs,
            16.0,
            container_height,
            container_height,
        );
        let max_h = if child_style.max_height != Length::Auto {
            child_style.max_height.to_px_with_viewport(
                c_fs,
                16.0,
                container_height,
                container_height,
            )
        } else {
            f32::INFINITY
        };
        let content_h = content_h.max(min_h).min(max_h);
        child.dimensions.content.size.height = content_h;
        child.dimensions.margin_box().height()
    } else {
        child.dimensions.margin_box().height()
    }
}

pub fn position_grid_items_in_cells(
    container: &mut LayoutBox,
    placed_items: &[PlacedItem],
    col_widths: &[f32],
    row_heights: &[f32],
    col_x_offsets: &[f32],
    row_y_offsets: &[f32],
    col_gap: f32,
    row_gap: f32,
    container_x: f32,
    container_y: f32,
    container_height: f32,
    style: &ComputedStyle,
) {
    for item in placed_items {
        let child = &mut container.children[item.child_idx];
        let child_style = child.style.clone().unwrap_or_default();

        let cell_x = col_x_offsets[item.col_start];
        let cell_y = row_y_offsets[item.row_start];

        let span_cell_w = (item.col_start..item.col_start + item.col_span)
            .map(|c| col_widths.get(c).copied().unwrap_or(0.0))
            .sum::<f32>()
            + (item.col_span.saturating_sub(1) as f32) * col_gap;

        let span_cell_h = (item.row_start..item.row_start + item.row_span)
            .map(|r| row_heights.get(r).copied().unwrap_or(0.0))
            .sum::<f32>()
            + (item.row_span.saturating_sub(1) as f32) * row_gap;

        let outer_w = child.dimensions.margin_box().width();
        let remaining_w = (span_cell_w - outer_w).max(0.0);

        let outer_h = child.dimensions.margin_box().height();
        let remaining_h = (span_cell_h - outer_h).max(0.0);

        // Auto margin resolution in cell (CSS Grid §10.1)
        let is_m_left_auto = child_style.margin_left == Length::Auto;
        let is_m_right_auto = child_style.margin_right == Length::Auto;
        let is_m_top_auto = child_style.margin_top == Length::Auto;
        let is_m_bottom_auto = child_style.margin_bottom == Length::Auto;

        let mut child_x = container_x
            + cell_x
            + child.dimensions.margin.left
            + child.dimensions.border.left
            + child.dimensions.padding.left;

        let item_justify = match child_style.justify_self {
            JustifySelf::Auto => style.justify_items,
            JustifySelf::Stretch => JustifyItems::Stretch,
            JustifySelf::FlexStart => JustifyItems::FlexStart,
            JustifySelf::FlexEnd => JustifyItems::FlexEnd,
            JustifySelf::Center => JustifyItems::Center,
            JustifySelf::Baseline => JustifyItems::Baseline,
        };

        if is_m_left_auto && is_m_right_auto {
            child_x += remaining_w / 2.0;
        } else if is_m_left_auto {
            child_x += remaining_w;
        } else {
            match item_justify {
                JustifyItems::Center => {
                    child_x += remaining_w / 2.0;
                }
                JustifyItems::FlexEnd => {
                    child_x += remaining_w;
                }
                JustifyItems::Stretch if child_style.width == Length::Auto => {
                    let p = child.dimensions.padding;
                    let b = child.dimensions.border;
                    let m = child.dimensions.margin;
                    let raw_stretch_w =
                        (span_cell_w - p.left - p.right - b.left - b.right - m.left - m.right)
                            .max(0.0);
                    child.dimensions.content.size.width = raw_stretch_w;
                }
                _ => {}
            }
        }

        let mut child_y = container_y
            + cell_y
            + child.dimensions.margin.top
            + child.dimensions.border.top
            + child.dimensions.padding.top;

        if is_m_top_auto && is_m_bottom_auto {
            child_y += remaining_h / 2.0;
        } else if is_m_top_auto {
            child_y += remaining_h;
        } else {
            // Alignment inside cell
            let item_align = match child_style.align_self {
                AlignSelf::Auto => style.align_items,
                AlignSelf::Stretch => AlignItems::Stretch,
                AlignSelf::FlexStart => AlignItems::FlexStart,
                AlignSelf::FlexEnd => AlignItems::FlexEnd,
                AlignSelf::Center => AlignItems::Center,
                AlignSelf::Baseline => AlignItems::Baseline,
                AlignSelf::SafeCenter => AlignItems::SafeCenter,
                AlignSelf::SafeFlexStart => AlignItems::SafeFlexStart,
                AlignSelf::SafeFlexEnd => AlignItems::SafeFlexEnd,
                AlignSelf::SafeStart => AlignItems::SafeStart,
                AlignSelf::SafeEnd => AlignItems::SafeEnd,
            };

            match item_align {
                AlignItems::Center => {
                    child_y += remaining_h / 2.0;
                }
                AlignItems::FlexEnd => {
                    child_y += remaining_h;
                }
                AlignItems::Normal | AlignItems::Stretch if child_style.height == Length::Auto => {
                    let p = child.dimensions.padding;
                    let b = child.dimensions.border;
                    let m = child.dimensions.margin;
                    let raw_stretch_h =
                        (span_cell_h - p.top - p.bottom - b.top - b.bottom - m.top - m.bottom)
                            .max(0.0);
                    let c_fs = child_style.font_size;
                    let min_h = child_style.min_height.to_px_with_viewport(
                        c_fs,
                        16.0,
                        container_height,
                        container_height,
                    );
                    let max_h = if child_style.max_height != Length::Auto {
                        child_style.max_height.to_px_with_viewport(
                            c_fs,
                            16.0,
                            container_height,
                            container_height,
                        )
                    } else {
                        f32::INFINITY
                    };
                    child.dimensions.content.size.height = raw_stretch_h.max(min_h).min(max_h);
                }
                _ => {}
            }
        }

        let dx = child_x - child.dimensions.content.x();
        let dy = child_y - child.dimensions.content.y();
        child.dimensions.content.origin = Point::new(child_x, child_y);
        shift_descendants(child, dx, dy);
    }
}
