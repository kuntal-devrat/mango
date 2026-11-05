use mango_css::values::{Length, Position};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;

/// Applies relative positioning offsets (`top`, `bottom`, `left`, `right`)
/// to a box without altering surrounding in-flow layout geometry.
pub(crate) fn apply_relative_offset(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    if let Some(style) = &box_node.style {
        let font_size = style.font_size;
        let root_font_size = style.root_font_size;
        let cw = containing_block.content.width();
        let ch = if containing_block.content.height() > 0.0 {
            containing_block.content.height()
        } else {
            0.0
        };

        let dx = if style.left != Length::Auto {
            style
                .left
                .to_px_with_viewport(font_size, root_font_size, cw, ch)
        } else if style.right != Length::Auto {
            -style
                .right
                .to_px_with_viewport(font_size, root_font_size, cw, ch)
        } else {
            0.0
        };

        let dy = if style.top != Length::Auto {
            style
                .top
                .to_px_with_viewport(font_size, root_font_size, ch, ch)
        } else if style.bottom != Length::Auto {
            -style
                .bottom
                .to_px_with_viewport(font_size, root_font_size, ch, ch)
        } else {
            0.0
        };

        box_node.dimensions.content.origin.x += dx;
        box_node.dimensions.content.origin.y += dy;
        shift_descendants(box_node, dx, dy);
    }
}

pub(crate) fn shift_descendants(box_node: &mut LayoutBox, dx: f32, dy: f32) {
    for child in &mut box_node.children {
        let child_pos = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        if child_pos == Position::Fixed {
            let (shift_x, shift_y) = if let Some(style) = &child.style {
                let sx = if style.left == Length::Auto && style.right == Length::Auto {
                    dx
                } else {
                    0.0
                };
                let sy = if style.top == Length::Auto && style.bottom == Length::Auto {
                    dy
                } else {
                    0.0
                };
                (sx, sy)
            } else {
                (dx, dy)
            };
            child.dimensions.content.origin.x += shift_x;
            child.dimensions.content.origin.y += shift_y;
            if let Some(ref mut pt) = child.static_position {
                pt.x += shift_x;
                pt.y += shift_y;
            }
            shift_descendants(child, shift_x, shift_y);
            continue;
        }
        child.dimensions.content.origin.x += dx;
        child.dimensions.content.origin.y += dy;
        if let Some(ref mut pt) = child.static_position {
            pt.x += dx;
            pt.y += dy;
        }
        shift_descendants(child, dx, dy);
    }
}

pub(crate) fn center_button_children(box_node: &mut LayoutBox) {
    if box_node.tag_name.as_deref() != Some("button") || box_node.children.is_empty() {
        return;
    }
    let button_h = box_node.dimensions.content.height();
    if button_h <= 0.0 {
        return;
    }
    let content_top = box_node.dimensions.content.y();
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for child in &box_node.children {
        let b = child.dimensions.margin_box();
        min_y = min_y.min(b.y());
        max_y = max_y.max(b.bottom());
    }
    if min_y < max_y && (max_y - min_y) < button_h {
        let children_h = max_y - min_y;
        let target_y = content_top + (button_h - children_h) / 2.0;
        let delta_y = target_y - min_y;
        if delta_y.abs() > 0.5 {
            for child in &mut box_node.children {
                child.dimensions.content.origin.y += delta_y;
                shift_descendants(child, 0.0, delta_y);
            }
        }
    }
}
