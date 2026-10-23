use mango_core::Point;
use mango_css::values::{BoxSizing, Clear, Display, Float, Length, Position};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;
use crate::inline_flow::layout_inline_children;

use super::bfc::establishes_bfc;
use super::floats::{extract_floats_from_children, has_floats_recursive, layout_single_float};
use super::margins::{
    calculate_block_position, collapse_margins, collapsed_margin_top_for_box,
    is_empty_block_collapsing_through,
};
use super::multicol::layout_multicol_children;
use super::positioned::{
    apply_relative_offset, center_button_children, layout_positioned_children,
    layout_positioned_children_with_cb, shift_descendants,
};
use super::sizing::{calculate_block_height, calculate_block_width};

/// Recursively lays out a block container box in a block formatting context.
pub fn layout_block(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    let display = box_node.style.as_ref().map(|s| s.display);
    if matches!(display, Some(Display::Flex) | Some(Display::InlineFlex)) {
        crate::flex_flow::layout_flex(box_node, containing_block, float_ctx);
        return;
    }
    if display == Some(Display::Grid) || display == Some(Display::InlineGrid) {
        crate::grid_flow::layout_grid(box_node, containing_block, float_ctx);
        return;
    }

    // Table formatting contexts: route any table-internal display to the table
    // path so table-row / cell / row-group roots don't fall through to plain
    // block layout silently (CSS 2.1 §17).
    if matches!(
        display,
        Some(
            Display::Table
                | Display::TableRow
                | Display::TableCell
                | Display::TableRowGroup
                | Display::TableHeaderGroup
                | Display::TableFooterGroup
                | Display::TableColumn
                | Display::TableColumnGroup
                | Display::TableCaption
        )
    ) || matches!(
        box_node.tag_name.as_deref(),
        Some(
            "table"
                | "tr"
                | "td"
                | "th"
                | "tbody"
                | "thead"
                | "tfoot"
                | "caption"
                | "colgroup"
                | "col"
        )
    ) {
        crate::table_flow::layout_table(box_node, containing_block, float_ctx);
        return;
    }

    if box_node.style.as_ref().map(|s| s.display) == Some(Display::Table)
        || box_node.tag_name.as_deref() == Some("table")
    {
        crate::table_flow::layout_table(box_node, containing_block, float_ctx);
        return;
    }

    // 1. Calculate horizontal dimensions (width, padding, borders, margins)
    calculate_block_width(box_node, containing_block);

    // 2. Calculate vertical position (respecting clearance)
    let start_y = containing_block.content.y();
    calculate_block_position(box_node, containing_block, start_y, float_ctx);

    // If box_node has a specified height, resolve it tentatively
    // so children (like percent-height or max-height inlines) have a containing block height.
    if let Some(style) = &box_node.style
        && style.height != Length::Auto
    {
        let font_size = style.font_size;
        let root_font_size = style.root_font_size;
        let cb_h = containing_block.content.height();
        let (_, vp_h) = mango_css::get_current_viewport();
        let tentative_h = style
            .height
            .to_px_with_viewport(font_size, root_font_size, cb_h, vp_h);
        if tentative_h > 0.0 {
            let content_h = if style.box_sizing == BoxSizing::BorderBox {
                let non_content_v =
                    box_node.dimensions.padding.vertical() + box_node.dimensions.border.vertical();
                (tentative_h - non_content_v).max(0.0)
            } else {
                tentative_h
            };
            box_node.dimensions.content.size.height = content_h;
        }
    }

    // 3. Lay out child boxes
    let skip_children = box_node
        .style
        .as_ref()
        .map(|s| s.content_visibility == mango_css::values::ContentVisibility::Hidden)
        .unwrap_or(false);

    if !skip_children {
        if establishes_bfc(box_node) {
            let mut bfc_float_ctx = FloatContext::new();
            layout_block_children(box_node, &mut bfc_float_ctx);
        } else {
            layout_block_children(box_node, float_ctx);
        }
    }

    // 4. Calculate final height
    calculate_block_height(box_node, containing_block);
    center_button_children(box_node);

    // 5. Position out-of-flow positioned children against resolved container dimensions
    layout_positioned_children_with_cb(box_node, containing_block, float_ctx);

    // 6. If this box itself is a float, register its margin box in FloatContext
    if let Some(style) = &box_node.style {
        match style.float {
            Float::Left | Float::InlineStart => {
                float_ctx.add_left_float(box_node.dimensions.margin_box());
            }
            Float::Right | Float::InlineEnd => {
                // Adjust position to the right edge of container
                let margin_box_w = box_node.dimensions.margin_box().width();
                let new_x = containing_block.content.right() - margin_box_w;
                let dx = new_x - box_node.dimensions.margin_box().x();
                box_node.dimensions.content.origin.x += dx;
                // Shift the whole subtree so descendants follow the float
                // (mirrors layout_single_float's final-position adjustment).
                if dx.abs() > 0.001 {
                    shift_descendants(box_node, dx, 0.0);
                }
                float_ctx.add_right_float(box_node.dimensions.margin_box());
            }
            Float::None => {}
        }
    }
}

pub(crate) fn layout_block_children(box_node: &mut LayoutBox, float_ctx: &mut FloatContext) {
    if box_node.children.is_empty() {
        return;
    }

    let has_inlines = box_node.children.iter().any(|c| c.is_inline());

    if has_inlines {
        let has_any_floats = has_floats_recursive(box_node);
        if has_any_floats {
            let container_dims = box_node.dimensions;
            let parent_link = box_node.link_target.clone();
            let parent_frame = box_node.link_frame.clone();
            let mut floats = extract_floats_from_children(
                &mut box_node.children,
                parent_link.as_deref(),
                parent_frame.as_deref(),
            );

            let mut float_bottom = container_dims.content.y();
            for float_child in &mut floats {
                layout_single_float(
                    float_child,
                    &container_dims,
                    container_dims.content.y(),
                    float_ctx,
                );
                float_bottom = float_bottom.max(float_child.dimensions.margin_box().bottom());
            }

            let inline_height = layout_inline_children(box_node, float_ctx);
            box_node.children.extend(floats);
            box_node.dimensions.content.size.height =
                inline_height.max(float_bottom - container_dims.content.y());
            return;
        }

        // Container establishes an inline formatting context
        let inline_height = layout_inline_children(box_node, float_ctx);
        box_node.dimensions.content.size.height = inline_height;
    } else {
        // Check for CSS Multi-column layout (column-count / column-width)
        let col_count_opt = box_node.style.as_ref().and_then(|s| s.column_count);
        let col_width_opt = box_node
            .style
            .as_ref()
            .and_then(|s| s.column_width.as_ref());
        if col_count_opt.is_some() || col_width_opt.is_some() {
            let available_w = box_node.dimensions.content.width();
            let font_size = box_node.style.as_ref().map(|s| s.font_size).unwrap_or(16.0);
            let gap = box_node
                .style
                .as_ref()
                .map(|s| s.column_gap.to_px(font_size, 16.0, available_w))
                .unwrap_or(16.0);

            let resolved_cols = match (col_count_opt, col_width_opt) {
                (Some(c), Some(w_len)) => {
                    let w_px = w_len.to_px(font_size, 16.0, available_w);
                    if w_px > 0.0 {
                        let max_cols =
                            ((available_w + gap) / (w_px + gap)).floor().max(1.0) as usize;
                        c.min(max_cols).max(1)
                    } else {
                        c.max(1)
                    }
                }
                (Some(c), None) => c.max(1),
                (None, Some(w_len)) => {
                    let w_px = w_len.to_px(font_size, 16.0, available_w);
                    if w_px > 0.0 {
                        ((available_w + gap) / (w_px + gap)).floor().max(1.0) as usize
                    } else {
                        1
                    }
                }
                (None, None) => 1,
            };

            if resolved_cols > 1 {
                if box_node.children.len() == 1 {
                    if let Some(child_style_arc) = box_node.children[0].style.as_mut()
                        && child_style_arc.column_count.is_none()
                        && child_style_arc.column_width.is_none()
                    {
                        let child_style = std::sync::Arc::make_mut(child_style_arc);
                        child_style.column_count = Some(resolved_cols);
                        child_style.column_gap = mango_css::values::Length::Px(gap);
                    }
                } else {
                    layout_multicol_children(box_node, resolved_cols, gap, float_ctx);
                    return;
                }
            }
        }

        // Block formatting context: layout children vertically with margin collapsing
        let is_ol = box_node.tag_name.as_deref() == Some("ol");
        let mut ol_idx = 1;
        if is_ol {
            for child in &mut box_node.children {
                if child.tag_name.as_deref() == Some("li") {
                    child
                        .attributes
                        .push(("_mango_list_index".to_string(), ol_idx.to_string()));
                    ol_idx += 1;
                }
            }
        }

        let mut cursor_y = box_node.dimensions.content.y();
        let mut prev_margin_bottom = 0.0f32;
        let mut in_flow_count = 0;

        let mut container_dims = box_node.dimensions;
        if container_dims.content.size.height == 0.0
            && let Some(style) = &box_node.style
        {
            let fs = style.font_size;
            let rfs = style.root_font_size;
            let non_v =
                box_node.dimensions.padding.vertical() + box_node.dimensions.border.vertical();
            match style.height {
                Length::Px(px) if px >= 0.0 => {
                    container_dims.content.size.height = if style.box_sizing == BoxSizing::BorderBox
                    {
                        (px - non_v).max(0.0)
                    } else {
                        px
                    };
                }
                Length::Rem(rem) if rem >= 0.0 => {
                    let h = rem * rfs;
                    container_dims.content.size.height = if style.box_sizing == BoxSizing::BorderBox
                    {
                        (h - non_v).max(0.0)
                    } else {
                        h
                    };
                }
                Length::Em(em) if em >= 0.0 => {
                    let h = em * fs;
                    container_dims.content.size.height = if style.box_sizing == BoxSizing::BorderBox
                    {
                        (h - non_v).max(0.0)
                    } else {
                        h
                    };
                }
                _ => {
                    if let Some(ratio) = style.aspect_ratio
                        && ratio > 0.0
                        && container_dims.content.size.width > 0.0
                    {
                        container_dims.content.size.height =
                            (container_dims.content.size.width / ratio).max(0.0);
                    }
                }
            }
        }
        let box_establishes_bfc = establishes_bfc(box_node);

        for child in box_node.children.iter_mut() {
            let child_pos = child
                .style
                .as_ref()
                .map(|s| s.position)
                .unwrap_or(Position::Static);
            if matches!(child_pos, Position::Absolute | Position::Fixed) {
                // Absolutely/fixed positioned elements are out-of-flow; record static position before skipping
                child.dimensions.content.origin.x = container_dims.content.x();
                child.dimensions.content.origin.y = cursor_y;
                continue;
            }

            let child_float = child.style.as_ref().map(|s| s.float).unwrap_or(Float::None);
            if child_float != Float::None {
                layout_single_float(child, &container_dims, cursor_y, float_ctx);
                continue;
            }

            // Normal in-flow child: apply clearance from active floats
            let clear = child.style.as_ref().map(|s| s.clear).unwrap_or(Clear::None);
            cursor_y = float_ctx.apply_clearance(cursor_y, clear);
            if clear != Clear::None {
                prev_margin_bottom = 0.0;
            }

            // Calculate child horizontal dimensions
            calculate_block_width(child, &container_dims);

            // Margin collapsing with previous sibling (CSS 2.1 §8.3.1)
            let parent_has_top_strut = container_dims.border.top > 0.0
                || container_dims.padding.top > 0.0
                || box_establishes_bfc;
            let curr_margin_top = if in_flow_count == 0 && !parent_has_top_strut {
                let collapsed = collapsed_margin_top_for_box(child, &container_dims);
                child.dimensions.margin.top = collapsed;
                collapsed
            } else {
                child.dimensions.margin.top
            };

            if in_flow_count > 0 {
                let collapsed_margin = collapse_margins(prev_margin_bottom, curr_margin_top);
                cursor_y += collapsed_margin;
            } else if parent_has_top_strut {
                cursor_y += curr_margin_top;
            } else {
                let collapsed = collapse_margins(box_node.dimensions.margin.top, curr_margin_top);
                let diff = collapsed - box_node.dimensions.margin.top;
                if diff > 0.0 {
                    box_node.dimensions.margin.top = collapsed;
                    box_node.dimensions.content.origin.y += diff;
                }
                cursor_y = box_node.dimensions.content.origin.y;
            }
            in_flow_count += 1;

            // Position child
            let child_establishes_bfc = establishes_bfc(child);
            let mut child_x = container_dims.content.x()
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left;

            if child_establishes_bfc {
                let (min_x, max_x) = float_ctx.available_span(
                    cursor_y,
                    child.dimensions.margin_box().height().max(1.0),
                    container_dims.content.x(),
                    container_dims.content.width(),
                );
                let avail_w = max_x - min_x;
                let child_box_w =
                    child.dimensions.border_box().width() + child.dimensions.margin.left;
                if child_box_w > avail_w
                    && (min_x > container_dims.content.x()
                        || max_x < container_dims.content.right())
                {
                    cursor_y = float_ctx.apply_clearance(cursor_y, Clear::Both);
                } else if min_x > container_dims.content.x() {
                    child_x = min_x
                        + child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left;
                }
            }

            let child_y = cursor_y + child.dimensions.border.top + child.dimensions.padding.top;
            child.dimensions.content.origin = Point::new(child_x, child_y);

            // Pre-calculate definite/tentative height for child so descendants resolving
            // percentage heights have a definite containing block height.
            if let Some(child_style) = &child.style
                && child_style.height != Length::Auto
            {
                let font_size = child_style.font_size;
                let root_font_size = child_style.root_font_size;
                let cb_h = container_dims.content.height();
                let is_pct_indefinite = matches!(child_style.height, Length::Percent(_))
                    && cb_h <= 0.0
                    && !matches!(child_style.position, Position::Absolute | Position::Fixed);
                if !is_pct_indefinite {
                    let (_, vp_h) = mango_css::get_current_viewport();
                    let eff_cb_h = if cb_h > 0.0 { cb_h } else { 600.0 };
                    let tentative_h = child_style.height.to_px_with_viewport(
                        font_size,
                        root_font_size,
                        eff_cb_h,
                        vp_h,
                    );
                    let non_content_v =
                        child.dimensions.padding.vertical() + child.dimensions.border.vertical();
                    let content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                        (tentative_h - non_content_v).max(0.0)
                    } else {
                        tentative_h.max(0.0)
                    };
                    child.dimensions.content.size.height = content_h;
                }
            }

            // Lay out child recursively
            let child_disp = child.style.as_ref().map(|s| s.display);
            if child_disp == Some(Display::Table) || child.tag_name.as_deref() == Some("table") {
                let mut child_cb = container_dims;
                child_cb.content.origin.y = cursor_y - child.dimensions.margin.top;
                let mut table_float_ctx = FloatContext::new();
                crate::table_flow::layout_table(child, &child_cb, &mut table_float_ctx);
                layout_positioned_children(child, &mut table_float_ctx);
            } else if matches!(child_disp, Some(Display::Flex) | Some(Display::InlineFlex)) {
                let mut flex_float_ctx = FloatContext::new();
                crate::flex_flow::layout_flex(child, &container_dims, &mut flex_float_ctx);
            } else if child_disp == Some(Display::Grid) || child_disp == Some(Display::InlineGrid) {
                let mut grid_float_ctx = FloatContext::new();
                crate::grid_flow::layout_grid(child, &container_dims, &mut grid_float_ctx);
            } else if child_establishes_bfc {
                let mut inner_float_ctx = FloatContext::new();
                layout_block_children(child, &mut inner_float_ctx);
                calculate_block_height(child, &container_dims);
                center_button_children(child);
                layout_positioned_children(child, &mut inner_float_ctx);
            } else {
                layout_block_children(child, float_ctx);
                calculate_block_height(child, &container_dims);
                center_button_children(child);
                layout_positioned_children(child, float_ctx);
            }

            // In-flow border box bottom before relative offset (CSS 2.1 §9.4.3: relative offset does not affect following boxes)
            let in_flow_bottom = child.dimensions.border_box().bottom();

            // Relative positioning offset (CSS 2.1 §9.4.3)
            if child_pos == Position::Relative {
                apply_relative_offset(child, &container_dims);
            }

            // Advance cursor
            if is_empty_block_collapsing_through(child) {
                let collapsed =
                    collapse_margins(child.dimensions.margin.top, child.dimensions.margin.bottom);
                child.dimensions.margin.top = collapsed;
                child.dimensions.margin.bottom = 0.0;
                prev_margin_bottom = collapsed;
                cursor_y = in_flow_bottom;
            } else {
                cursor_y = in_flow_bottom;
                prev_margin_bottom = child.dimensions.margin.bottom;
            }
        }

        let parent_has_bottom_strut = container_dims.border.bottom > 0.0
            || container_dims.padding.bottom > 0.0
            || box_establishes_bfc;
        if !parent_has_bottom_strut && in_flow_count > 0 {
            box_node.dimensions.margin.bottom =
                collapse_margins(box_node.dimensions.margin.bottom, prev_margin_bottom);
        }
    }
}

pub fn layout_block_contents(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    layout_block_children(box_node, float_ctx);
    calculate_block_height(box_node, containing_block);
    center_button_children(box_node);
    layout_positioned_children_with_cb(box_node, containing_block, float_ctx);
}
