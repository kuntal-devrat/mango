use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::{BreakInside, ColumnSpan, Display, Position};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::container::layout_block_children;
use super::positioned::{center_button_children, layout_positioned_children, shift_descendants};
use super::sizing::{calculate_block_height, calculate_block_width};

/// Lays out block children in multiple balanced columns according to CSS Multi-column layout,
/// supporting column-span: all partitions, break-inside: avoid, and block fragmentation.
pub(crate) fn layout_multicol_children(
    box_node: &mut LayoutBox,
    num_cols: usize,
    gap: f32,
    float_ctx: &mut FloatContext,
) {
    let container_x = box_node.dimensions.content.x();
    let container_y = box_node.dimensions.content.y();
    let container_w = box_node.dimensions.content.width();
    let col_w = ((container_w - (num_cols - 1) as f32 * gap) / num_cols as f32).max(10.0);

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
    let mut current_y = container_y;

    for chunk in chunks {
        match chunk {
            MulticolChunk::Columns(mut items) => {
                let col_cb = Dimensions {
                    content: Rect::new(container_x, current_y, col_w, 0.0),
                    padding: EdgeSizes::ZERO,
                    border: EdgeSizes::ZERO,
                    margin: EdgeSizes::ZERO,
                };

                let mut unfragmented_total_h = 0.0f32;
                for it in &mut items {
                    let child_pos = it
                        .style
                        .as_ref()
                        .map(|s| s.position)
                        .unwrap_or(Position::Static);
                    if !matches!(child_pos, Position::Absolute | Position::Fixed) {
                        calculate_block_width(it, &col_cb);
                        layout_block_children(it, float_ctx);
                        calculate_block_height(it, &col_cb);
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
                        layout_block_children(&mut frag1, float_ctx);
                        calculate_block_height(&mut frag1, &col_cb);
                        fragmented_items.push(frag1);

                        let mut frag2 = item;
                        frag2.children = second_half;
                        calculate_block_width(&mut frag2, &col_cb);
                        layout_block_children(&mut frag2, float_ctx);
                        calculate_block_height(&mut frag2, &col_cb);
                        fragmented_items.push(frag2);
                    } else {
                        fragmented_items.push(item);
                    }
                }

                let chunk_h = layout_multicol_segment(
                    &mut fragmented_items,
                    container_x,
                    current_y,
                    col_w,
                    gap,
                    num_cols,
                    target_col_h,
                    float_ctx,
                );

                result_children.extend(fragmented_items);
                current_y += chunk_h;
            }
            MulticolChunk::Spanning(mut span_child) => {
                let span_cb = Dimensions {
                    content: Rect::new(container_x, current_y, container_w, 0.0),
                    padding: EdgeSizes::ZERO,
                    border: EdgeSizes::ZERO,
                    margin: EdgeSizes::ZERO,
                };
                calculate_block_width(&mut span_child, &span_cb);
                layout_block_children(&mut span_child, float_ctx);
                calculate_block_height(&mut span_child, &span_cb);
                center_button_children(&mut span_child);
                layout_positioned_children(&mut span_child, float_ctx);

                let new_x = container_x + span_child.dimensions.margin.left;
                let new_y = current_y + span_child.dimensions.margin.top;
                let dx = new_x - span_child.dimensions.content.x();
                let dy = new_y - span_child.dimensions.content.y();
                span_child.dimensions.content.origin = Point::new(new_x, new_y);
                shift_descendants(&mut span_child, dx, dy);

                current_y += span_child.dimensions.margin_box().height();
                result_children.push(span_child);
            }
        }
    }

    let total_h = (current_y - container_y).max(0.0);
    box_node.children = result_children;
    box_node.dimensions.content.size.height = total_h;
}

fn layout_multicol_segment(
    children: &mut [LayoutBox],
    container_x: f32,
    start_y: f32,
    col_w: f32,
    gap: f32,
    num_cols: usize,
    target_col_h: f32,
    float_ctx: &mut FloatContext,
) -> f32 {
    let col_cb = Dimensions {
        content: Rect::new(container_x, start_y, col_w, 0.0),
        padding: EdgeSizes::ZERO,
        border: EdgeSizes::ZERO,
        margin: EdgeSizes::ZERO,
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
        } else if child_display == Some(Display::Grid) || child_display == Some(Display::InlineGrid)
        {
            crate::grid_flow::layout_grid(child, &col_cb, float_ctx);
        } else if child_display == Some(Display::Table)
            || child.tag_name.as_deref() == Some("table")
        {
            crate::table_flow::layout_table(child, &col_cb, float_ctx);
        } else {
            calculate_block_width(child, &col_cb);
            layout_block_children(child, float_ctx);
            calculate_block_height(child, &col_cb);
            center_button_children(child);
            layout_positioned_children(child, float_ctx);
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

        let new_x = container_x + cur_col as f32 * (col_w + gap) + child.dimensions.margin.left;
        let new_y = start_y + cur_col_y + child.dimensions.margin.top;

        let dx = new_x - child.dimensions.content.x();
        let dy = new_y - child.dimensions.content.y();
        child.dimensions.content.origin = Point::new(new_x, new_y);
        shift_descendants(child, dx, dy);

        cur_col_y += h;
        col_heights[cur_col] = cur_col_y;
    }

    col_heights.into_iter().fold(0.0f32, f32::max)
}
