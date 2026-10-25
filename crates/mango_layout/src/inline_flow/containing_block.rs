use mango_core::Rect;
use mango_css::values::{Direction, Position, WritingMode};
use mango_html::dom::NodeId;
use std::collections::HashMap;

use crate::{BoxType, FloatContext, LayoutBox};

pub(crate) fn compute_inline_containing_block_rect(
    first_box: Rect,
    last_box: Rect,
    direction: Direction,
    writing_mode: WritingMode,
) -> Rect {
    match writing_mode {
        WritingMode::VerticalLr | WritingMode::SidewaysLr => {
            let left = first_box.x();
            let right = last_box.right();
            let (top, bottom) = if (writing_mode == WritingMode::VerticalLr
                && direction == Direction::Rtl)
                || (writing_mode == WritingMode::SidewaysLr && direction == Direction::Ltr)
            {
                (last_box.y(), first_box.bottom())
            } else {
                (first_box.y(), last_box.bottom())
            };
            Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
        }
        WritingMode::VerticalRl | WritingMode::SidewaysRl => {
            let right = first_box.right();
            let left = last_box.x();
            let (top, bottom) = if (writing_mode == WritingMode::VerticalRl
                && direction == Direction::Rtl)
                || (writing_mode == WritingMode::SidewaysRl && direction == Direction::Ltr)
            {
                (last_box.y(), first_box.bottom())
            } else {
                (first_box.y(), last_box.bottom())
            };
            Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
        }
        WritingMode::HorizontalTb => {
            let top = first_box.y();
            let bottom = last_box.bottom();
            if direction == Direction::Rtl {
                let right = first_box.right();
                let left = last_box.x();
                Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
            } else {
                let left = first_box.x();
                let right = last_box.right();
                Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
            }
        }
    }
}

pub(crate) fn resolve_inline_containing_blocks(
    container: &LayoutBox,
    positioned_boxes: &mut Vec<LayoutBox>,
    _writing_mode: WritingMode,
    float_ctx: &mut FloatContext,
) {
    fn collect_oof_and_inlines(
        box_node: &LayoutBox,
        current_positioned_inline: Option<(NodeId, Direction, WritingMode)>,
        oof_by_inline: &mut HashMap<NodeId, (Vec<LayoutBox>, Direction, WritingMode)>,
        unassociated_oof: &mut Vec<LayoutBox>,
    ) {
        let child_pos = box_node
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        if matches!(child_pos, Position::Absolute | Position::Fixed) {
            if let Some((pid, dir, wm)) = current_positioned_inline {
                let entry = oof_by_inline
                    .entry(pid)
                    .or_insert_with(|| (Vec::new(), dir, wm));
                entry.0.push(box_node.clone());
            } else {
                unassociated_oof.push(box_node.clone());
            }
            return;
        }

        // Only inline boxes participate in the inline formatting context.
        // Atomic inlines (inline-block, etc.) and blocks establish their own formatting context.
        if box_node.box_type != BoxType::InlineNode {
            return;
        }

        let is_positioned_inline = box_node
            .style
            .as_ref()
            .map(|s| s.position != Position::Static)
            .unwrap_or(false);

        let next_pid = if is_positioned_inline && let Some(nid) = box_node.node_id {
            let dir = box_node
                .style
                .as_ref()
                .map(|s| s.direction)
                .unwrap_or(Direction::Ltr);
            let wm = box_node
                .style
                .as_ref()
                .map(|s| s.writing_mode)
                .unwrap_or(WritingMode::HorizontalTb);
            Some((nid, dir, wm))
        } else {
            current_positioned_inline
        };

        for child in &box_node.children {
            collect_oof_and_inlines(child, next_pid, oof_by_inline, unassociated_oof);
        }
    }

    let mut oof_by_inline: HashMap<NodeId, (Vec<LayoutBox>, Direction, WritingMode)> =
        HashMap::new();
    let mut unassociated_oof = Vec::new();

    if let Some(source_children) = container.raw_children.as_deref() {
        for sc in source_children {
            collect_oof_and_inlines(sc, None, &mut oof_by_inline, &mut unassociated_oof);
        }
    }

    for (inline_id, (oof_list, dir, wm)) in oof_by_inline {
        let matching: Vec<&LayoutBox> = positioned_boxes
            .iter()
            .filter(|b| b.node_id == Some(inline_id))
            .collect();
        let cb_rect = if !matching.is_empty() {
            match wm {
                WritingMode::VerticalRl
                | WritingMode::VerticalLr
                | WritingMode::SidewaysRl
                | WritingMode::SidewaysLr => {
                    let first_x = matching
                        .first()
                        .map(|b| b.dimensions.content.x())
                        .unwrap_or(0.0);
                    let last_x = matching
                        .last()
                        .map(|b| b.dimensions.content.x())
                        .unwrap_or(0.0);
                    let first_boxes: Vec<&LayoutBox> = matching
                        .iter()
                        .filter(|b| (b.dimensions.content.x() - first_x).abs() < 1.0)
                        .copied()
                        .collect();
                    let last_boxes: Vec<&LayoutBox> = matching
                        .iter()
                        .filter(|b| (b.dimensions.content.x() - last_x).abs() < 1.0)
                        .copied()
                        .collect();

                    let f_top = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.y())
                        .fold(f32::INFINITY, f32::min);
                    let f_bottom = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.bottom())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let f_left = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.x())
                        .fold(f32::INFINITY, f32::min);
                    let f_right = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.right())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let first_box = Rect::new(
                        f_left,
                        f_top,
                        (f_right - f_left).max(0.0),
                        (f_bottom - f_top).max(0.0),
                    );

                    let l_top = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.y())
                        .fold(f32::INFINITY, f32::min);
                    let l_bottom = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.bottom())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let l_left = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.x())
                        .fold(f32::INFINITY, f32::min);
                    let l_right = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.right())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let last_box = Rect::new(
                        l_left,
                        l_top,
                        (l_right - l_left).max(0.0),
                        (l_bottom - l_top).max(0.0),
                    );

                    let container_dir = container
                        .style
                        .as_ref()
                        .map(|s| s.direction)
                        .unwrap_or(Direction::Ltr);
                    let effective_dir = if container_dir == Direction::Rtl || dir == Direction::Rtl
                    {
                        Direction::Rtl
                    } else {
                        Direction::Ltr
                    };

                    compute_inline_containing_block_rect(first_box, last_box, effective_dir, wm)
                }
                WritingMode::HorizontalTb => {
                    let first_y = matching
                        .first()
                        .map(|b| b.dimensions.content.y())
                        .unwrap_or(0.0);
                    let last_y = matching
                        .last()
                        .map(|b| b.dimensions.content.y())
                        .unwrap_or(0.0);
                    let first_boxes: Vec<&LayoutBox> = matching
                        .iter()
                        .filter(|b| (b.dimensions.content.y() - first_y).abs() < 1.0)
                        .copied()
                        .collect();
                    let last_boxes: Vec<&LayoutBox> = matching
                        .iter()
                        .filter(|b| (b.dimensions.content.y() - last_y).abs() < 1.0)
                        .copied()
                        .collect();

                    let f_top = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.y())
                        .fold(f32::INFINITY, f32::min);
                    let f_bottom = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.bottom())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let f_left = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.x())
                        .fold(f32::INFINITY, f32::min);
                    let f_right = first_boxes
                        .iter()
                        .map(|b| b.dimensions.content.right())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let first_box = Rect::new(
                        f_left,
                        f_top,
                        (f_right - f_left).max(0.0),
                        (f_bottom - f_top).max(0.0),
                    );

                    let l_top = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.y())
                        .fold(f32::INFINITY, f32::min);
                    let l_bottom = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.bottom())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let l_left = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.x())
                        .fold(f32::INFINITY, f32::min);
                    let l_right = last_boxes
                        .iter()
                        .map(|b| b.dimensions.content.right())
                        .fold(f32::NEG_INFINITY, f32::max);
                    let last_box = Rect::new(
                        l_left,
                        l_top,
                        (l_right - l_left).max(0.0),
                        (l_bottom - l_top).max(0.0),
                    );

                    let container_dir = container
                        .style
                        .as_ref()
                        .map(|s| s.direction)
                        .unwrap_or(Direction::Ltr);
                    let effective_dir = if container_dir == Direction::Rtl || dir == Direction::Rtl
                    {
                        Direction::Rtl
                    } else {
                        Direction::Ltr
                    };

                    compute_inline_containing_block_rect(first_box, last_box, effective_dir, wm)
                }
            }
        } else {
            Rect::new(
                container.dimensions.content.x(),
                container.dimensions.content.y(),
                0.0,
                0.0,
            )
        };

        let mut custom_cb = crate::dimensions::Dimensions::new(cb_rect);
        custom_cb.direction = dir;
        custom_cb.writing_mode = wm;
        for mut oof in oof_list {
            oof.custom_containing_block = Some(custom_cb);
            oof.containing_inline_id = Some(inline_id);
            if let Some(b) = positioned_boxes.iter().find(|b| b.node_id == oof.node_id) {
                oof.dimensions.content.origin = b.dimensions.content.origin;
                oof.static_position = b.static_position;
                oof.is_static_bottom_anchored = b.is_static_bottom_anchored;
            }
            crate::block_flow::positioned::layout_positioned_element(
                &mut oof, &custom_cb, float_ctx,
            );
            if let Some(pos_box) = positioned_boxes
                .iter_mut()
                .find(|b| b.node_id == oof.node_id)
            {
                *pos_box = oof;
            } else {
                positioned_boxes.push(oof);
            }
        }
    }

    for mut oof in unassociated_oof {
        if let Some(b) = positioned_boxes.iter().find(|b| b.node_id == oof.node_id) {
            oof.dimensions.content.origin = b.dimensions.content.origin;
            oof.static_position = b.static_position;
            oof.is_static_bottom_anchored = b.is_static_bottom_anchored;
        }
        let container_cb = container.dimensions;
        crate::block_flow::positioned::layout_positioned_element(
            &mut oof,
            &container_cb,
            float_ctx,
        );
        if let Some(pos_box) = positioned_boxes
            .iter_mut()
            .find(|b| b.node_id == oof.node_id)
        {
            *pos_box = oof;
        } else {
            positioned_boxes.push(oof);
        }
    }
}
