use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::{BreakInside, ColumnSpan, Display, Position, WritingMode};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::container::layout_block_children;
use super::positioned::{
    center_button_children, layout_positioned_children, layout_positioned_children_with_cb,
    shift_descendants,
};
use super::sizing::{calculate_block_height, calculate_block_width};

/// Lays out block children in multiple balanced columns according to CSS Multi-column layout,
/// supporting column-span: all partitions, break-inside: avoid, writing-modes, and block fragmentation.
pub(crate) fn layout_multicol_children(
    box_node: &mut LayoutBox,
    num_cols: usize,
    gap: f32,
    float_ctx: &mut FloatContext,
) {
    let wm = box_node
        .style
        .as_ref()
        .map(|s| s.writing_mode)
        .unwrap_or(WritingMode::HorizontalTb);
    let is_vertical = wm.is_vertical();

    let (
        container_inline_origin,
        container_block_origin,
        container_inline_size,
        container_block_size,
    ) = if is_vertical {
        (
            box_node.dimensions.content.y(),
            box_node.dimensions.content.x(),
            box_node.dimensions.content.height(),
            box_node.dimensions.content.width(),
        )
    } else {
        (
            box_node.dimensions.content.x(),
            box_node.dimensions.content.y(),
            box_node.dimensions.content.width(),
            box_node.dimensions.content.height(),
        )
    };

    let col_inline_size =
        ((container_inline_size - (num_cols - 1) as f32 * gap) / num_cols as f32).max(1.0);
    let col_block_size = container_block_size;

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

    enum MulticolChunk {
        Columns(Vec<LayoutBox>),
        Spanning(LayoutBox),
    }

    let mut chunks: Vec<MulticolChunk> = Vec::new();
    let mut pending_normal: Vec<LayoutBox> = Vec::new();

    for child in box_node.children.drain(..) {
        let is_span_all = child
            .style
            .as_ref()
            .is_some_and(|s| s.column_span == ColumnSpan::All);
        if is_span_all {
            if !pending_normal.is_empty() {
                chunks.push(MulticolChunk::Columns(std::mem::take(&mut pending_normal)));
            }
            chunks.push(MulticolChunk::Spanning(child));
        } else {
            pending_normal.push(child);
        }
    }
    if !pending_normal.is_empty() {
        chunks.push(MulticolChunk::Columns(pending_normal));
    }

    let mut result_children = Vec::new();
    let mut current_offset = container_block_origin;

    for chunk in chunks {
        match chunk {
            MulticolChunk::Columns(mut items) => {
                let col_cb = if is_vertical {
                    Dimensions {
                        content: Rect::new(
                            current_offset,
                            container_inline_origin,
                            col_block_size,
                            col_inline_size,
                        ),
                        padding: EdgeSizes::ZERO,
                        border: EdgeSizes::ZERO,
                        margin: EdgeSizes::ZERO,
                        ..Default::default()
                    }
                } else {
                    Dimensions {
                        content: Rect::new(
                            container_inline_origin,
                            current_offset,
                            col_inline_size,
                            0.0,
                        ),
                        padding: EdgeSizes::ZERO,
                        border: EdgeSizes::ZERO,
                        margin: EdgeSizes::ZERO,
                        ..Default::default()
                    }
                };

                let mut unfragmented_total_h = 0.0f32;
                for it in &mut items {
                    let child_pos = it
                        .style
                        .as_ref()
                        .map(|s| s.position)
                        .unwrap_or(Position::Static);
                    if !matches!(child_pos, Position::Absolute | Position::Fixed) {
                        let child_display = it.style.as_ref().map(|s| s.display);
                        if matches!(
                            child_display,
                            Some(Display::Flex) | Some(Display::InlineFlex)
                        ) {
                            crate::flex_flow::layout_flex(it, &col_cb, float_ctx);
                        } else if child_display == Some(Display::Grid)
                            || child_display == Some(Display::InlineGrid)
                        {
                            crate::grid_flow::layout_grid(it, &col_cb, float_ctx);
                        } else if child_display == Some(Display::Table)
                            || it.tag_name.as_deref() == Some("table")
                        {
                            crate::table_flow::layout_table(it, &col_cb, float_ctx);
                        } else {
                            calculate_block_width(it, &col_cb);
                            layout_block_children(it, &col_cb, float_ctx);
                            calculate_block_height(it, &col_cb);
                            center_button_children(it);
                            layout_positioned_children_with_cb(it, &col_cb, float_ctx);
                        }
                        unfragmented_total_h += it.dimensions.margin_box().height();
                    }
                }

                let target_col_h = (unfragmented_total_h / num_cols as f32).max(20.0);

                // Fragment splittable blocks across column boundary
                let mut fragmented_items = Vec::with_capacity(items.len() * 2);
                for mut item in items.into_iter() {
                    let break_inside = item
                        .style
                        .as_ref()
                        .map_or(BreakInside::Auto, |s| s.break_inside);

                    if is_vertical {
                        let item_w = item.dimensions.border_box().width();
                        if item_w > col_block_size * 1.05 && col_block_size > 0.0 {
                            let num_frags = (item_w / col_block_size).ceil().max(1.0) as usize;
                            if item.children.is_empty() {
                                for _ in 0..num_frags {
                                    let mut frag = item.clone();
                                    frag.dimensions.content.size.width = col_block_size;
                                    frag.dimensions.content.size.height = col_inline_size;
                                    fragmented_items.push(frag);
                                }
                            } else {
                                let orig_content_x = item.dimensions.content.origin.x;
                                let is_rl =
                                    matches!(wm, WritingMode::VerticalRl | WritingMode::SidewaysRl);
                                for k in 0..num_frags {
                                    let frag_idx = if is_rl { num_frags - 1 - k } else { k };
                                    let min_x = frag_idx as f32 * col_block_size;
                                    let max_x = (frag_idx + 1) as f32 * col_block_size;
                                    let mut frag = item.clone();
                                    frag.dimensions.content.size.width = col_block_size;
                                    frag.dimensions.content.size.height = col_inline_size;
                                    let mut frag_children = Vec::new();
                                    for sc in &item.children {
                                        let sc_rel_x =
                                            sc.dimensions.content.origin.x - orig_content_x;
                                        let in_frag = if frag_idx == num_frags - 1 {
                                            sc_rel_x >= min_x - 0.1
                                        } else {
                                            sc_rel_x >= min_x - 0.1 && sc_rel_x < max_x - 0.1
                                        };
                                        if in_frag {
                                            let mut shifted_sc = sc.clone();
                                            let dx = -min_x;
                                            shifted_sc.dimensions.content.origin.x += dx;
                                            shift_descendants(&mut shifted_sc, dx, 0.0);
                                            frag_children.push(shifted_sc);
                                        }
                                    }
                                    frag.children = frag_children;
                                    fragmented_items.push(frag);
                                }
                            }
                        } else {
                            fragmented_items.push(item);
                        }
                    } else {
                        let can_fragment = break_inside == BreakInside::Auto
                            && item.children.len() > 1
                            && item.dimensions.margin_box().height() > target_col_h * 0.8;

                        if can_fragment {
                            let mut sub_children = std::mem::take(&mut item.children);
                            let mid = sub_children.len().div_ceil(2);
                            let second_half = sub_children.split_off(mid);

                            let mut frag1 = item.clone();
                            frag1.children = sub_children;
                            calculate_block_width(&mut frag1, &col_cb);
                            layout_block_children(&mut frag1, &col_cb, float_ctx);
                            calculate_block_height(&mut frag1, &col_cb);
                            fragmented_items.push(frag1);

                            let mut frag2 = item;
                            frag2.children = second_half;
                            calculate_block_width(&mut frag2, &col_cb);
                            layout_block_children(&mut frag2, &col_cb, float_ctx);
                            calculate_block_height(&mut frag2, &col_cb);
                            fragmented_items.push(frag2);
                        } else {
                            fragmented_items.push(item);
                        }
                    }
                }

                let chunk_size = layout_multicol_segment(
                    &mut fragmented_items,
                    wm,
                    container_inline_origin,
                    current_offset,
                    col_inline_size,
                    col_block_size,
                    gap,
                    num_cols,
                    target_col_h,
                    float_ctx,
                );

                result_children.extend(fragmented_items);
                current_offset += chunk_size;
            }
            MulticolChunk::Spanning(mut span_child) => {
                let span_cb = if is_vertical {
                    Dimensions {
                        content: Rect::new(
                            container_block_origin,
                            current_offset,
                            container_block_size,
                            container_inline_size,
                        ),
                        padding: EdgeSizes::ZERO,
                        border: EdgeSizes::ZERO,
                        margin: EdgeSizes::ZERO,
                        ..Default::default()
                    }
                } else {
                    Dimensions {
                        content: Rect::new(
                            container_inline_origin,
                            current_offset,
                            container_inline_size,
                            0.0,
                        ),
                        padding: EdgeSizes::ZERO,
                        border: EdgeSizes::ZERO,
                        margin: EdgeSizes::ZERO,
                        ..Default::default()
                    }
                };
                calculate_block_width(&mut span_child, &span_cb);
                layout_block_children(&mut span_child, &span_cb, float_ctx);
                calculate_block_height(&mut span_child, &span_cb);
                center_button_children(&mut span_child);
                layout_positioned_children(&mut span_child, float_ctx);

                let new_x = if is_vertical {
                    container_block_origin + span_child.dimensions.margin.left
                } else {
                    container_inline_origin + span_child.dimensions.margin.left
                };
                let new_y = if is_vertical {
                    current_offset + span_child.dimensions.margin.top
                } else {
                    current_offset + span_child.dimensions.margin.top
                };
                let dx = new_x - span_child.dimensions.content.x();
                let dy = new_y - span_child.dimensions.content.y();
                span_child.dimensions.content.origin = Point::new(new_x, new_y);
                shift_descendants(&mut span_child, dx, dy);

                current_offset += if is_vertical {
                    span_child.dimensions.margin_box().height()
                } else {
                    span_child.dimensions.margin_box().height()
                };
                result_children.push(span_child);
            }
        }
    }

    let total_extent = (current_offset - container_block_origin).max(0.0);
    box_node.children = result_children;
    let is_height_auto = box_node
        .style
        .as_ref()
        .map(|s| s.height == mango_css::values::Length::Auto)
        .unwrap_or(true);
    if is_vertical {
        if is_height_auto || box_node.dimensions.content.size.height <= 0.0 {
            box_node.dimensions.content.size.height = total_extent;
        }
        if box_node.dimensions.content.size.width <= 0.0 {
            box_node.dimensions.content.size.width = container_block_size;
        }
    } else {
        if is_height_auto || box_node.dimensions.content.size.height <= 0.0 {
            box_node.dimensions.content.size.height = total_extent;
        }
    }
}

fn layout_multicol_segment(
    children: &mut [LayoutBox],
    wm: WritingMode,
    container_inline_origin: f32,
    container_block_origin: f32,
    col_inline_size: f32,
    _col_block_size: f32,
    gap: f32,
    num_cols: usize,
    target_col_h: f32,
    float_ctx: &mut FloatContext,
) -> f32 {
    let is_vertical = wm.is_vertical();

    if is_vertical {
        let mut cur_col = 0;
        for child in children.iter_mut() {
            let child_pos = child
                .style
                .as_ref()
                .map(|s| s.position)
                .unwrap_or(Position::Static);
            if matches!(child_pos, Position::Absolute | Position::Fixed) {
                continue;
            }

            let col_y = container_inline_origin + cur_col as f32 * (col_inline_size + gap);
            let col_x = container_block_origin;

            let new_x = col_x + child.dimensions.margin.left;
            let new_y = col_y + child.dimensions.margin.top;

            let dx = new_x - child.dimensions.content.x();
            let dy = new_y - child.dimensions.content.y();
            child.dimensions.content.origin = Point::new(new_x, new_y);
            shift_descendants(child, dx, dy);

            cur_col += 1;
        }
        (cur_col as f32 * (col_inline_size + gap)).max(col_inline_size)
    } else {
        let col_cb = Dimensions {
            content: Rect::new(
                container_inline_origin,
                container_block_origin,
                col_inline_size,
                0.0,
            ),
            padding: EdgeSizes::ZERO,
            border: EdgeSizes::ZERO,
            margin: EdgeSizes::ZERO,
            ..Default::default()
        };

        let mut cur_col = 0;
        let mut cur_col_y = 0.0f32;
        let mut col_heights = vec![0.0f32; num_cols];

        for child in children.iter_mut() {
            let child_pos = child
                .style
                .as_ref()
                .map(|s| s.position)
                .unwrap_or(Position::Static);
            if matches!(child_pos, Position::Absolute | Position::Fixed) {
                continue;
            }

            let child_display = child.style.as_ref().map(|s| s.display);
            if matches!(
                child_display,
                Some(Display::Flex) | Some(Display::InlineFlex)
            ) {
                crate::flex_flow::layout_flex(child, &col_cb, float_ctx);
            } else if child_display == Some(Display::Grid)
                || child_display == Some(Display::InlineGrid)
            {
                crate::grid_flow::layout_grid(child, &col_cb, float_ctx);
            } else if child_display == Some(Display::Table)
                || child.tag_name.as_deref() == Some("table")
            {
                crate::table_flow::layout_table(child, &col_cb, float_ctx);
            } else {
                calculate_block_width(child, &col_cb);
                layout_block_children(child, &col_cb, float_ctx);
                calculate_block_height(child, &col_cb);
                center_button_children(child);
                layout_positioned_children_with_cb(child, &col_cb, float_ctx);
            }

            let h = child.dimensions.margin_box().height();
            let break_inside = child
                .style
                .as_ref()
                .map_or(BreakInside::Auto, |s| s.break_inside);
            let avoids_break = matches!(
                break_inside,
                BreakInside::Avoid | BreakInside::AvoidColumn | BreakInside::AvoidPage
            );

            if cur_col < num_cols - 1 && cur_col_y > 0.0 {
                if avoids_break {
                    if cur_col_y + h > target_col_h {
                        cur_col += 1;
                        cur_col_y = 0.0;
                    }
                } else if cur_col_y + h * 0.5 > target_col_h {
                    cur_col += 1;
                    cur_col_y = 0.0;
                }
            }

            let new_x = container_inline_origin
                + cur_col as f32 * (col_inline_size + gap)
                + child.dimensions.margin.left;
            let new_y = container_block_origin + cur_col_y + child.dimensions.margin.top;

            let dx = new_x - child.dimensions.content.x();
            let dy = new_y - child.dimensions.content.y();
            child.dimensions.content.origin = Point::new(new_x, new_y);
            shift_descendants(child, dx, dy);

            cur_col_y += h;
            col_heights[cur_col] = cur_col_y;
        }

        col_heights.into_iter().fold(0.0f32, f32::max)
    }
}
