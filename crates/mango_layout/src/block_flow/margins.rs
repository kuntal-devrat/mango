use mango_core::Point;
use mango_css::values::{Clear, Length, Position};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::bfc::establishes_bfc;

/// Sets the initial position of the block box.
pub(crate) fn calculate_block_position(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    current_y: f32,
    float_ctx: &FloatContext,
) {
    let clear = box_node
        .style
        .as_ref()
        .map(|s| s.clear)
        .unwrap_or(Clear::None);
    let cleared_y = float_ctx.apply_clearance(current_y, clear);

    let x = containing_block.content.x()
        + box_node.dimensions.margin.left
        + box_node.dimensions.border.left
        + box_node.dimensions.padding.left;

    let y = cleared_y
        + box_node.dimensions.margin.top
        + box_node.dimensions.border.top
        + box_node.dimensions.padding.top;

    box_node.dimensions.content.origin = Point::new(x, y);
}

/// Collapses two adjoining vertical margins according to CSS 2.1 §8.3.1.
///
/// - If both are positive: max(m1, m2)
/// - If both are negative: min(m1, m2) (most negative)
/// - If one is positive and one is negative: m1 + m2 (deduct negative from positive)
#[inline]
pub fn collapse_margins(m1: f32, m2: f32) -> f32 {
    if m1 >= 0.0 && m2 >= 0.0 {
        m1.max(m2)
    } else if m1 <= 0.0 && m2 <= 0.0 {
        m1.min(m2)
    } else {
        m1 + m2
    }
}

/// Returns true if an element's top and bottom margins are adjoining according to CSS 2.1 §8.3.1,
/// meaning the element has no border, padding, in-flow content, or height separating its margins.
pub fn is_empty_block_collapsing_through(box_node: &LayoutBox) -> bool {
    if box_node.is_inline() {
        return false;
    }
    let pos = box_node
        .style
        .as_ref()
        .map(|s| s.position)
        .unwrap_or(Position::Static);
    if matches!(pos, Position::Absolute | Position::Fixed) {
        return false;
    }
    if box_node.dimensions.border.vertical() > 0.0
        || box_node.dimensions.padding.vertical() > 0.0
        || box_node.dimensions.content.size.height > 0.0
    {
        return false;
    }
    if let Some(style) = &box_node.style {
        if style.height != Length::Auto && style.height != Length::Px(0.0) {
            return false;
        }
        if style.min_height != Length::Auto && style.min_height != Length::Px(0.0) {
            return false;
        }
        if establishes_bfc(box_node) {
            return false;
        }
    }
    box_node.children.iter().all(|c| {
        let c_pos = c
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        matches!(c_pos, Position::Absolute | Position::Fixed)
            || is_empty_block_collapsing_through(c)
    })
}

/// Recursively determines the collapsed top margin for a box and its first in-flow descendants
/// when no top border, padding, or BFC strut intervenes (CSS 2.1 § 8.3.1).
pub(crate) fn collapsed_margin_top_for_box(
    layout_box: &LayoutBox,
    container_dims: &Dimensions,
) -> f32 {
    let style = match &layout_box.style {
        Some(s) => s,
        None => return layout_box.dimensions.margin.top,
    };
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    let cb_w = container_dims.content.width();
    let cb_h = container_dims.content.height().max(0.0);

    let my_margin_top = if layout_box.dimensions.margin.top != 0.0 {
        layout_box.dimensions.margin.top
    } else {
        style
            .margin_top
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h)
    };

    let pad_top = if layout_box.dimensions.padding.top != 0.0 {
        layout_box.dimensions.padding.top
    } else {
        style
            .padding_top
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h)
    };
    let border_top = if layout_box.dimensions.border.top != 0.0 {
        layout_box.dimensions.border.top
    } else {
        style.border_top_width
    };

    if pad_top > 0.0 || border_top > 0.0 || establishes_bfc(layout_box) {
        return my_margin_top;
    }

    // If the box establishes an IFC or has an inline-level child, line boxes intervene (CSS 2.1 § 8.3.1)
    if layout_box.children.iter().any(|c| c.is_inline()) {
        return my_margin_top;
    }

    for child in &layout_box.children {
        if !child.is_in_flow_block() {
            continue;
        }
        let child_collapsed = collapsed_margin_top_for_box(child, container_dims);
        return collapse_margins(my_margin_top, child_collapsed);
    }

    my_margin_top
}
