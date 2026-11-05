use mango_core::{EdgeSizes, Point, Size};
use mango_css::values::{AlignItems, AlignSelf, Display, FlexDirection, Length, Position};
use taffy::prelude::AvailableSpace;
use taffy::tree::TaffyTree;

use crate::block_flow::{
    apply_relative_offset, layout_block_contents, layout_positioned_children_with_cb,
};
use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::flex_flow::item::find_first_baseline_ascent;
use crate::flex_flow::layout_flex;
use crate::float::FloatContext;
use crate::grid_flow::layout_grid;
use crate::inline_flow::layout_inline_children;

use super::convert::to_taffy_style;
use super::intrinsic::{LeafContext, compute_child_intrinsic_size};

/// Executes Taffy-powered CSS Flexbox or CSS Grid layout on a container box.
/// Returns `true` if layout succeeded, or `false` if fallback is required.
pub fn execute_taffy_layout(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
    is_grid: bool,
) -> bool {
    let style = match &container.style {
        Some(s) => s.clone(),
        None => return false,
    };

    // Only handle horizontal-tb writing mode in Taffy; fall back to Mango for vertical modes
    if style.writing_mode != mango_css::values::WritingMode::HorizontalTb {
        return false;
    }

    // Subgrid and named grid template areas require Mango's internal grid engine
    if is_grid {
        if !style.grid_template_areas.is_empty()
            || !style.grid_column_lines.is_empty()
            || !style.grid_row_lines.is_empty()
        {
            return false;
        }
        let has_subgrid = style
            .grid_template_columns
            .iter()
            .chain(style.grid_template_rows.iter())
            .any(|t| matches!(t, mango_css::values::GridTrackSize::Subgrid));
        if has_subgrid {
            return false;
        }
        if container.children.iter().any(|c| {
            c.style
                .as_ref()
                .map(|s| {
                    s.grid_template_columns
                        .iter()
                        .chain(s.grid_template_rows.iter())
                        .any(|t| matches!(t, mango_css::values::GridTrackSize::Subgrid))
                })
                .unwrap_or(false)
        }) {
            return false;
        }
    }

    let cb_w = containing_block.content.width();
    let cb_h = containing_block.content.height();
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;

    // In-place sort children by CSS order property for flex containers
    let has_non_zero_order = container
        .children
        .iter()
        .any(|c| c.style.as_ref().map(|s| s.order != 0).unwrap_or(false));
    if !is_grid && has_non_zero_order {
        container
            .children
            .sort_by_key(|c| c.style.as_ref().map(|s| s.order).unwrap_or(0));
    }

    // 1. Resolve container box model properties
    let pad_top = style
        .padding_top
        .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);
    let pad_right = style
        .padding_right
        .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);
    let pad_bottom =
        style
            .padding_bottom
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);
    let pad_left = style
        .padding_left
        .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    container.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    container.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);
    container.dimensions.writing_mode = style.writing_mode;

    let margin_top = style
        .margin_top
        .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);
    let margin_bottom =
        style
            .margin_bottom
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);
    let margin_left = style
        .margin_left
        .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);
    let margin_right =
        style
            .margin_right
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h);

    container.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);

    // Compute initial container position in document coordinates
    let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
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

    // 2. Build Taffy Tree with LeafContext
    let mut tree: TaffyTree<LeafContext> = TaffyTree::new();
    tree.disable_rounding();

    let container_taffy_style = to_taffy_style(&style, cb_w, cb_h);
    let root_node = match tree.new_leaf_with_context(container_taffy_style, LeafContext::default())
    {
        Ok(node) => node,
        Err(_) => return false,
    };

    let mut in_flow_indices = Vec::new();
    let mut child_nodes = Vec::new();

    for (idx, child) in container.children.iter().enumerate() {
        let child_style = child.style.as_deref();
        let display = child_style.map(|s| s.display).unwrap_or(Display::Inline);
        let pos = child_style.map(|s| s.position).unwrap_or(Position::Static);

        if display == Display::None || matches!(pos, Position::Absolute | Position::Fixed) {
            continue;
        }

        let mut taffy_style = if let Some(cs) = child_style {
            to_taffy_style(cs, cb_w, cb_h)
        } else {
            taffy::style::Style::default()
        };

        let (int_w, int_h) = compute_child_intrinsic_size(child, cb_w, cb_h, font_size);

        // If replaced element with intrinsic sizing, ensure initial size is populated
        match &child.box_type {
            BoxType::ReplacedElement {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Canvas {
                intrinsic_width,
                intrinsic_height,
                ..
            } if *intrinsic_width > 0.0 && *intrinsic_height > 0.0 => {
                if taffy_style.size.width.is_auto() {
                    taffy_style.size.width = taffy::style_helpers::length(*intrinsic_width);
                }
                if taffy_style.size.height.is_auto() {
                    taffy_style.size.height = taffy::style_helpers::length(*intrinsic_height);
                }
            }
            _ => {}
        }

        // Automatic minimum size for flex items (CSS Flexbox spec Section 4.5):
        // In a row flex container, min-width: auto resolves to min-content width.
        // In a column flex container, min-height: auto resolves to min-content height.
        // If the item has non-visible overflow along that axis (e.g. overflow: hidden),
        // the automatic minimum size is 0.
        if !is_grid {
            let is_row = matches!(
                style.flex_direction,
                mango_css::values::FlexDirection::Row
                    | mango_css::values::FlexDirection::RowReverse
            );
            let s_opt = child.style.as_deref();
            let overflow_hidden = s_opt.is_some_and(|s| {
                if is_row {
                    s.overflow_x != mango_css::values::Overflow::Visible
                } else {
                    s.overflow_y != mango_css::values::Overflow::Visible
                }
            });
            if !overflow_hidden {
                if is_row && s_opt.map(|s| s.min_width.is_auto()).unwrap_or(true) && int_w > 0.0 {
                    taffy_style.min_size.width = taffy::style_helpers::length(int_w);
                } else if !is_row
                    && s_opt.map(|s| s.min_height.is_auto()).unwrap_or(true)
                    && int_h > 0.0
                {
                    taffy_style.min_size.height = taffy::style_helpers::length(int_h);
                }
            }
        }

        let child_node = match tree.new_leaf_with_context(
            taffy_style,
            LeafContext {
                intrinsic_width: int_w,
                intrinsic_height: int_h,
            },
        ) {
            Ok(n) => n,
            Err(_) => return false,
        };

        in_flow_indices.push(idx);
        child_nodes.push(child_node);
    }

    let taffy_children = if is_grid && has_non_zero_order {
        let mut ordered_pairs: Vec<(i32, taffy::NodeId)> = in_flow_indices
            .iter()
            .zip(child_nodes.iter())
            .map(|(&idx, &node)| {
                let order = container.children[idx]
                    .style
                    .as_ref()
                    .map(|s| s.order)
                    .unwrap_or(0);
                (order, node)
            })
            .collect();
        ordered_pairs.sort_by_key(|(order, _)| *order);
        ordered_pairs
            .into_iter()
            .map(|(_, node)| node)
            .collect::<Vec<_>>()
    } else {
        child_nodes.clone()
    };

    if tree.set_children(root_node, &taffy_children).is_err() {
        return false;
    }

    // 3. Compute layout with measure callback
    let avail_w = if cb_w > 0.0 {
        AvailableSpace::Definite(cb_w)
    } else {
        AvailableSpace::MinContent
    };
    let avail_h = if cb_h > 0.0 {
        AvailableSpace::Definite(cb_h)
    } else {
        let (_, vp_h) = mango_css::get_current_viewport();
        let min_h = style
            .min_height
            .to_px_with_viewport(font_size, root_font_size, 0.0, vp_h);
        let h = style
            .height
            .to_px_with_viewport(font_size, root_font_size, 0.0, vp_h);
        let eff_h = if h > 0.0 {
            h
        } else if min_h > 0.0 {
            min_h
        } else {
            0.0
        };
        if eff_h > 0.0 {
            AvailableSpace::Definite(eff_h)
        } else {
            AvailableSpace::MaxContent
        }
    };

    let available_space = taffy::geometry::Size {
        width: avail_w,
        height: avail_h,
    };

    if tree
        .compute_layout_with_measure(
            root_node,
            available_space,
            |inputs, _node_id, context, style| {
                let (int_w, int_h) = context
                    .map(|c| (c.intrinsic_width, c.intrinsic_height))
                    .unwrap_or((0.0, 0.0));
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known_dims, _avail| taffy::geometry::Size {
                        width: known_dims.width.unwrap_or(int_w),
                        height: known_dims.height.unwrap_or(int_h),
                    },
                )
            },
        )
        .is_err()
    {
        return false;
    }

    // 4. Propagate layout results back to container and children
    let root_layout = match tree.layout(root_node) {
        Ok(l) => l,
        Err(_) => return false,
    };

    let content_w = root_layout.content_box_width().max(0.0);
    let orig_content_h = root_layout.content_box_height().max(0.0);
    let mut content_h = orig_content_h;

    // Column-reverse height clamping: if auto height with max-height, wrap to content
    let is_column_reverse =
        !is_grid && matches!(style.flex_direction, FlexDirection::ColumnReverse);
    let mut col_reverse_shift_y = 0.0f32;
    if is_column_reverse && style.height == Length::Auto {
        let mut max_col_h = 0.0f32;
        for &node in &child_nodes {
            if let Ok(l) = tree.layout(node) {
                let item_total_h = l.size.height + l.margin.top + l.margin.bottom;
                if item_total_h > max_col_h {
                    max_col_h = item_total_h;
                }
            }
        }
        if max_col_h > 0.0 && max_col_h < content_h {
            col_reverse_shift_y = content_h - max_col_h;
            content_h = max_col_h;
        }
    }

    container.dimensions.content.size.width = content_w;
    container.dimensions.content.size.height = content_h;

    // Parent border box origin
    let container_border_x = container_x - pad_left - border_left;
    let container_border_y = container_y - pad_top - border_top;

    for (&child_idx, &node) in in_flow_indices.iter().zip(child_nodes.iter()) {
        let child_layout = match tree.layout(node) {
            Ok(l) => l,
            Err(_) => continue,
        };

        let child = &mut container.children[child_idx];
        let p_top = child_layout.padding.top;
        let p_right = child_layout.padding.right;
        let p_bottom = child_layout.padding.bottom;
        let p_left = child_layout.padding.left;
        child.dimensions.padding = EdgeSizes::new(p_top, p_right, p_bottom, p_left);

        let b_top = child_layout.border.top;
        let b_right = child_layout.border.right;
        let b_bottom = child_layout.border.bottom;
        let b_left = child_layout.border.left;
        child.dimensions.border = EdgeSizes::new(b_top, b_right, b_bottom, b_left);

        let m_top = child_layout.margin.top;
        let m_right = child_layout.margin.right;
        let m_bottom = child_layout.margin.bottom;
        let m_left = child_layout.margin.left;
        child.dimensions.margin = EdgeSizes::new(m_top, m_right, m_bottom, m_left);

        let local_x = child_layout.content_box_x();
        let local_y = child_layout.content_box_y() - col_reverse_shift_y;
        child.dimensions.content.origin =
            Point::new(container_border_x + local_x, container_border_y + local_y);

        let w = child_layout.content_box_width().max(0.0);
        let h = child_layout.content_box_height().max(0.0);
        child.dimensions.content.size = Size::new(w, h);

        // Lay out descendant subtree of child
        let child_dims = child.dimensions;
        let c_display = child
            .style
            .as_ref()
            .map(|s| s.display)
            .unwrap_or(Display::Inline);

        match c_display {
            Display::Flex | Display::InlineFlex => {
                layout_flex(child, &child_dims, float_ctx);
            }
            Display::Grid | Display::InlineGrid => {
                layout_grid(child, &child_dims, float_ctx);
            }
            Display::Block => {
                layout_block_contents(child, &child_dims, float_ctx);
            }
            _ => {
                if !child.children.is_empty() {
                    let _ = layout_inline_children(child, float_ctx);
                }
            }
        }

        // Restore child's used dimensions as computed by flex/grid container
        child.dimensions.content.size = Size::new(w, h);
    }

    // 5. Baseline alignment adjustment for flex rows
    if !is_grid {
        let is_row = matches!(
            style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let has_baseline = matches!(
            style.align_items,
            AlignItems::Baseline | AlignItems::LastBaseline
        ) || container.children.iter().any(|c| {
            c.style
                .as_ref()
                .map(|s| matches!(s.align_self, AlignSelf::Baseline | AlignSelf::LastBaseline))
                .unwrap_or(false)
        });

        if is_row && has_baseline {
            let mut max_ascent = 0.0f32;
            let mut item_ascents = Vec::new();

            for child in &container.children {
                let c_style = child.style.as_deref();
                let is_oof = c_style
                    .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                    .unwrap_or(false);
                if is_oof {
                    item_ascents.push(None);
                    continue;
                }

                let align = c_style
                    .and_then(|s| match s.align_self {
                        AlignSelf::Auto => None,
                        other => Some(other),
                    })
                    .unwrap_or_else(|| match style.align_items {
                        AlignItems::Baseline => AlignSelf::Baseline,
                        AlignItems::LastBaseline => AlignSelf::LastBaseline,
                        _ => AlignSelf::Auto,
                    });

                if matches!(align, AlignSelf::Baseline | AlignSelf::LastBaseline) {
                    let fs = c_style.map(|s| s.font_size).unwrap_or(font_size);
                    let ascent = find_first_baseline_ascent(child, fs)
                        .unwrap_or(child.dimensions.content.size.height);
                    if ascent > max_ascent {
                        max_ascent = ascent;
                    }
                    item_ascents.push(Some(ascent));
                } else {
                    item_ascents.push(None);
                }
            }

            for (child, ascent_opt) in container.children.iter_mut().zip(item_ascents) {
                if let Some(ascent) = ascent_opt {
                    let diff = (max_ascent - ascent).max(0.0);
                    child.dimensions.content.origin.y += diff;
                }
            }
        }
    }

    // 6. Auto-margin centering (e.g. `margin: 0 auto`) on container
    let is_margin_left_auto = style.margin_left == Length::Auto;
    let is_margin_right_auto = style.margin_right == Length::Auto;
    if !is_pos && (is_margin_left_auto || is_margin_right_auto) && cb_w > 0.0 {
        let total_non_content_h = pad_left + pad_right + border_left + border_right;
        let underflow = (cb_w - content_w - total_non_content_h).max(0.0);
        let new_margin_left = if is_margin_left_auto && is_margin_right_auto {
            underflow / 2.0
        } else if is_margin_left_auto {
            underflow
        } else {
            0.0
        };
        let new_margin_right = if is_margin_left_auto && is_margin_right_auto {
            underflow / 2.0
        } else if is_margin_right_auto {
            underflow
        } else {
            0.0
        };

        let shift_x = new_margin_left - margin_left;
        if shift_x.abs() > 0.001 {
            container.dimensions.content.origin.x += shift_x;
            container.dimensions.margin.left = new_margin_left;
            container.dimensions.margin.right = new_margin_right;
            for child in &mut container.children {
                let is_child_oof = child
                    .style
                    .as_ref()
                    .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                    .unwrap_or(false);
                if !is_child_oof {
                    child.dimensions.content.origin.x += shift_x;
                }
            }
        }
    }

    // 7. Apply relative positioning offsets
    let container_dims = container.dimensions;
    for child in &mut container.children {
        if child.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
            apply_relative_offset(child, &container_dims);
        }
    }

    // 8. Lay out out-of-flow positioned children
    if !is_grid {
        let is_flex_dir_row = matches!(
            style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let is_vertical_wm = matches!(
            style.writing_mode,
            mango_css::values::WritingMode::VerticalRl | mango_css::values::WritingMode::VerticalLr
        );
        let is_row = if is_vertical_wm {
            !is_flex_dir_row
        } else {
            is_flex_dir_row
        };
        let is_reverse = matches!(
            style.flex_direction,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        );
        crate::flex_flow::abspos::calculate_abspos_static_positions(
            container,
            &style,
            is_row,
            is_reverse,
            container_x,
            container_y,
            content_w,
            content_h,
        );
    }
    layout_positioned_children_with_cb(container, containing_block, float_ctx);

    true
}
