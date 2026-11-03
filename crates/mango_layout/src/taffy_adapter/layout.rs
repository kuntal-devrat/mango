use mango_core::{EdgeSizes, Point, Size};
use mango_css::values::{Display, Length, Position};
use taffy::prelude::AvailableSpace;
use taffy::tree::TaffyTree;

use crate::block_flow::{
    apply_relative_offset, layout_block_contents, layout_positioned_children_with_cb,
};
use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::flex_flow::layout_flex;
use crate::float::FloatContext;
use crate::grid_flow::layout_grid;
use crate::inline_flow::layout_inline_children;

use super::convert::to_taffy_style;

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

    // Subgrid requires parent track inheritance which is handled by Mango's internal grid engine
    if is_grid {
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
        if !style.grid_template_areas.is_empty()
            || !style.grid_column_lines.is_empty()
            || !style.grid_row_lines.is_empty()
        {
            return false;
        }
        if style.order != 0
            || container
                .children
                .iter()
                .any(|c| c.style.as_ref().map(|s| s.order != 0).unwrap_or(false))
        {
            return false;
        }
        if style
            .grid_template_columns
            .iter()
            .any(|t| matches!(t, mango_css::values::GridTrackSize::RepeatAutoFill(_)))
        {
            return false;
        }
        if style
            .grid_template_columns
            .iter()
            .chain(style.grid_template_rows.iter())
            .any(|t| matches!(t, mango_css::values::GridTrackSize::MinMax(_, _)))
        {
            return false;
        }
        if style.width == Length::Auto && containing_block.content.width() <= 0.0 {
            return false;
        }
    } else {
        // Fall back to legacy flex layout for multi-line wrapping and specific legacy test behaviors
        if style.flex_wrap.is_wrap() {
            return false;
        }
        if container.children.iter().any(|c| {
            c.style
                .as_ref()
                .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                .unwrap_or(false)
        }) {
            return false;
        }
        if container.children.iter().any(|c| {
            matches!(
                c.tag_name.as_deref(),
                Some("input") | Some("button") | Some("select") | Some("textarea")
            )
        }) {
            return false;
        }
        if matches!(
            style.align_items,
            mango_css::values::AlignItems::Baseline | mango_css::values::AlignItems::LastBaseline
        ) {
            return false;
        }
        if style.order != 0
            || container
                .children
                .iter()
                .any(|c| c.style.as_ref().map(|s| s.order != 0).unwrap_or(false))
        {
            return false;
        }
        if container.children.iter().any(|c| {
            c.style
                .as_ref()
                .map(|s| s.visibility == mango_css::values::Visibility::Collapse)
                .unwrap_or(false)
        }) {
            return false;
        }
        if style.width == Length::Auto && containing_block.content.width() <= 0.0 {
            return false;
        }
        if !matches!(
            style.align_content,
            mango_css::values::AlignContent::Normal | mango_css::values::AlignContent::Stretch
        ) {
            return false;
        }
        if container.children.iter().any(|c| {
            c.style
                .as_ref()
                .map(|s| {
                    matches!(
                        s.flex_basis,
                        Length::Content
                            | Length::MinContent
                            | Length::MaxContent
                            | Length::FitContent
                    ) || (s.flex_grow == 0.0
                        && s.flex_shrink > 0.0
                        && s.flex_basis == Length::Px(0.0)
                        && s.width == Length::Auto
                        && !c.children.is_empty())
                })
                .unwrap_or(false)
        }) {
            return false;
        }
        if container.children.iter().any(|c| {
            c.style
                .as_ref()
                .map(|s| {
                    s.width == Length::Auto
                        && s.flex_basis == Length::Auto
                        && !c.children.is_empty()
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

    // Compute container position in document coordinates
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

    // 2. Build Taffy Tree
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let container_taffy_style = to_taffy_style(&style, cb_w, cb_h);
    let root_node = match tree.new_leaf(container_taffy_style) {
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

        let child_node = match tree.new_leaf(taffy_style) {
            Ok(n) => n,
            Err(_) => return false,
        };

        in_flow_indices.push(idx);
        child_nodes.push(child_node);
    }

    if tree.set_children(root_node, &child_nodes).is_err() {
        return false;
    }

    // 3. Compute layout
    let avail_w = if cb_w > 0.0 {
        AvailableSpace::Definite(cb_w)
    } else {
        AvailableSpace::MinContent
    };
    let avail_h = if cb_h > 0.0 {
        AvailableSpace::Definite(cb_h)
    } else {
        AvailableSpace::MaxContent
    };

    let available_space = taffy::geometry::Size {
        width: avail_w,
        height: avail_h,
    };

    if tree.compute_layout(root_node, available_space).is_err() {
        return false;
    }

    // 4. Propagate layout results back to container and children
    let root_layout = match tree.layout(root_node) {
        Ok(l) => l,
        Err(_) => return false,
    };

    let content_w = root_layout.content_box_width().max(0.0);
    let content_h = root_layout.content_box_height().max(0.0);
    container.dimensions.content.size.width = content_w;
    container.dimensions.content.size.height = content_h;

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
        let local_y = child_layout.content_box_y();
        child.dimensions.content.origin = Point::new(container_x + local_x, container_y + local_y);

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
    }

    // 5. Apply relative positioning offsets
    let container_dims = container.dimensions;
    for child in &mut container.children {
        if child.style.as_ref().map(|s| s.position) == Some(Position::Relative) {
            apply_relative_offset(child, &container_dims);
        }
    }

    // 6. Lay out out-of-flow positioned children
    layout_positioned_children_with_cb(container, containing_block, float_ctx);

    true
}
