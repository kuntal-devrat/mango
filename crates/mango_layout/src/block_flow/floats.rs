use mango_core::Point;
use mango_css::values::{Clear, Display, Float};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::container::layout_block_children;
use super::positioned::{
    center_button_children, layout_positioned_children, layout_positioned_children_with_cb,
    shift_descendants,
};
use super::sizing::{calculate_block_height, calculate_block_width};

pub(crate) fn has_floats_recursive(box_node: &LayoutBox) -> bool {
    box_node.children.iter().any(|c| {
        c.style.as_ref().is_some_and(|s| s.float != Float::None)
            || (c.is_inline() && has_floats_recursive(c))
    })
}

pub(crate) fn extract_floats_from_children(
    children: &mut Vec<LayoutBox>,
    parent_link: Option<&str>,
    parent_frame: Option<&str>,
) -> Vec<LayoutBox> {
    let mut extracted_floats = Vec::new();
    let mut retained_children = Vec::new();

    for mut child in children.drain(..) {
        let is_float =
            !child.is_text() && child.style.as_ref().is_some_and(|s| s.float != Float::None);
        if is_float {
            if child.link_target.is_none() {
                child.link_target = parent_link.map(|s| s.to_string());
                child.link_frame = parent_frame.map(|s| s.to_string());
            }
            extracted_floats.push(child);
        } else if child.is_inline() && has_floats_recursive(&child) {
            let child_link = child.link_target.as_deref().or(parent_link);
            let child_frame = child.link_frame.as_deref().or(parent_frame);
            let inner_floats =
                extract_floats_from_children(&mut child.children, child_link, child_frame);
            extracted_floats.extend(inner_floats);
            retained_children.push(child);
        } else {
            retained_children.push(child);
        }
    }

    *children = retained_children;
    extracted_floats
}

/// Lays out a floated element, positions it against the active FloatContext,
/// shifts its descendants, and registers its margin box into the FloatContext.
pub(crate) fn layout_single_float(
    child: &mut LayoutBox,
    container_dims: &Dimensions,
    cursor_y: f32,
    float_ctx: &mut FloatContext,
) {
    let float_type = child.style.as_ref().map(|s| s.float).unwrap_or(Float::None);
    let clear = child.style.as_ref().map(|s| s.clear).unwrap_or(Clear::None);
    let mut float_y = float_ctx.apply_clearance(cursor_y, clear);

    // Calculate child horizontal and vertical dimensions (shrink-to-fit or explicit)
    calculate_block_width(child, container_dims);
    calculate_block_height(child, container_dims);

    let margin_box_w = child.dimensions.margin_box().width();
    let margin_box_h = child.dimensions.margin_box().height().max(1.0);

    // Step down until the float fits horizontally or clears all active floats (CSS 2.1 §9.5.1 Rules 1-9)
    let is_right = matches!(float_type, Float::Right | Float::InlineEnd);
    let (margin_box_x, margin_box_y) = float_ctx.find_float_position(
        is_right,
        float_y,
        margin_box_w,
        margin_box_h,
        container_dims.content.x(),
        container_dims.content.width(),
    );
    float_y = margin_box_y;

    let child_x = margin_box_x
        + child.dimensions.margin.left
        + child.dimensions.border.left
        + child.dimensions.padding.left;
    let child_y = margin_box_y
        + child.dimensions.margin.top
        + child.dimensions.border.top
        + child.dimensions.padding.top;

    child.dimensions.content.origin = Point::new(child_x, child_y);

    // Floats establish an independent Block Formatting Context (BFC, CSS 2.1 §9.5).
    // Internal contents must not interact with or wrap around floats outside this BFC.
    let mut inner_float_ctx = FloatContext::new();

    // Lay out contents
    let child_disp = child.style.as_ref().map(|s| s.display);
    if child_disp == Some(Display::Table) || child.tag_name.as_deref() == Some("table") {
        crate::table_flow::layout_table(child, container_dims, &mut inner_float_ctx);
        layout_positioned_children(child, &mut inner_float_ctx);
    } else if matches!(child_disp, Some(Display::Flex) | Some(Display::InlineFlex)) {
        crate::flex_flow::layout_flex(child, container_dims, &mut inner_float_ctx);
    } else if child_disp == Some(Display::Grid) || child_disp == Some(Display::InlineGrid) {
        crate::grid_flow::layout_grid(child, container_dims, &mut inner_float_ctx);
    } else {
        layout_block_children(child, container_dims, &mut inner_float_ctx);
        calculate_block_height(child, container_dims);
        center_button_children(child);
        layout_positioned_children_with_cb(child, container_dims, &mut inner_float_ctx);
    }

    // If final height changed the available horizontal span, adjust child_x
    let final_margin_box_h = child.dimensions.margin_box().height();
    let final_child_x = match float_type {
        Float::Left | Float::InlineStart => {
            let (min_x, _max_x) = float_ctx.available_span(
                float_y,
                final_margin_box_h,
                container_dims.content.x(),
                container_dims.content.width(),
            );
            min_x
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
        Float::Right | Float::InlineEnd => {
            let (_min_x, max_x) = float_ctx.available_span(
                float_y,
                final_margin_box_h,
                container_dims.content.x(),
                container_dims.content.width(),
            );
            max_x - margin_box_w
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
        Float::None => child_x,
    };

    if (final_child_x - child_x).abs() > 0.001 {
        let dx = final_child_x - child_x;
        child.dimensions.content.origin.x = final_child_x;
        shift_descendants(child, dx, 0.0);
    }

    // Register into outer float_ctx
    match float_type {
        Float::Left | Float::InlineStart => float_ctx.add_left_float(child.dimensions.margin_box()),
        Float::Right | Float::InlineEnd => float_ctx.add_right_float(child.dimensions.margin_box()),
        Float::None => {}
    }
}
