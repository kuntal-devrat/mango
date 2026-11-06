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
fn child_padding_and_border(
    s: Option<&mango_css::ComputedStyle>,
    fs: f32,
    rfs: f32,
    w: f32,
    h: f32,
) -> (f32, f32) {
    let Some(s) = s else {
        return (0.0, 0.0);
    };
    let px = |l: &mango_css::values::Length| l.to_px_with_viewport(fs, rfs, w, h);
    let pad_h =
        px(&s.padding_left) + px(&s.padding_right) + s.border_left_width + s.border_right_width;
    let pad_v =
        px(&s.padding_top) + px(&s.padding_bottom) + s.border_top_width + s.border_bottom_width;
    (pad_h, pad_v)
}

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
        if style
            .grid_template_columns
            .iter()
            .chain(style.grid_template_rows.iter())
            .any(|t| matches!(t, mango_css::values::GridTrackSize::Subgrid))
        {
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
    let p_px = |p: &mango_css::values::Length| {
        p.to_px_with_viewport(font_size, root_font_size, cb_w, cb_h)
    };
    let pad_top = p_px(&style.padding_top);
    let pad_right = p_px(&style.padding_right);
    let pad_bottom = p_px(&style.padding_bottom);
    let pad_left = p_px(&style.padding_left);

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    container.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    container.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);
    container.dimensions.writing_mode = style.writing_mode;

    let is_ifc = container.is_independent_formatting_context;
    let margin_top = if is_ifc && container.dimensions.margin.top > 0.0 {
        container.dimensions.margin.top
    } else {
        p_px(&style.margin_top)
    };
    let margin_bottom = if is_ifc && container.dimensions.margin.bottom > 0.0 {
        container.dimensions.margin.bottom
    } else {
        p_px(&style.margin_bottom)
    };
    let margin_left = if (style.margin_left == Length::Auto || is_ifc)
        && container.dimensions.margin.left > 0.0
    {
        container.dimensions.margin.left
    } else {
        style
            .margin_left
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h)
    };
    let margin_right = if (style.margin_right == Length::Auto || is_ifc)
        && container.dimensions.margin.right > 0.0
    {
        container.dimensions.margin.right
    } else {
        style
            .margin_right
            .to_px_with_viewport(font_size, root_font_size, cb_w, cb_h)
    };

    container.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);

    // Compute initial container position in document coordinates
    let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
    let (container_x, container_y) =
        if is_pos || is_ifc || container.dimensions.content.origin != Point::ZERO {
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

    let total_h_p_b = pad_left + pad_right + border_left + border_right;
    let total_v_p_b = pad_top + pad_bottom + border_top + border_bottom;

    let mut container_taffy_style = to_taffy_style(&style, cb_w, cb_h);
    let is_block_level = matches!(style.display, Display::Flex | Display::Grid);
    if is_block_level
        && style.width == Length::Auto
        && !is_pos
        && !is_ifc
        && style.float == mango_css::values::Float::None
        && cb_w > 0.0
    {
        let net_w = if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
            cb_w
        } else {
            (cb_w - total_h_p_b).max(0.0)
        };
        container_taffy_style.size.width = taffy::style_helpers::length(net_w);
    }
    let is_definite_container = is_pos || (is_ifc && container.dimensions.content.size.width > 0.0);
    if is_definite_container {
        if container.dimensions.content.size.width > 0.0 {
            let target_w = if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                container.dimensions.content.size.width + total_h_p_b
            } else {
                container.dimensions.content.size.width
            };
            container_taffy_style.size.width = taffy::style_helpers::length(target_w);
        }
        if container.dimensions.content.size.height > 0.0 {
            let target_h = if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                container.dimensions.content.size.height + total_v_p_b
            } else {
                container.dimensions.content.size.height
            };
            container_taffy_style.size.height = taffy::style_helpers::length(target_h);
        }
    }
    let root_node = match tree.new_leaf_with_context(container_taffy_style, LeafContext::default())
    {
        Ok(node) => node,
        Err(_) => return false,
    };

    let (_, vp_h) = mango_css::get_current_viewport();
    let container_content_w =
        if is_definite_container && container.dimensions.content.size.width > 0.0 {
            container.dimensions.content.size.width
        } else {
            match style.width {
                Length::Auto => {
                    if is_block_level
                        && !is_pos
                        && style.float == mango_css::values::Float::None
                        && cb_w > 0.0
                    {
                        (cb_w
                            - margin_left
                            - margin_right
                            - pad_left
                            - pad_right
                            - border_left
                            - border_right)
                            .max(0.0)
                    } else {
                        0.0
                    }
                }
                Length::Percent(p) => {
                    if cb_w > 0.0 {
                        let px = (p / 100.0) * cb_w;
                        if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                            (px - total_h_p_b).max(0.0)
                        } else {
                            px
                        }
                    } else {
                        0.0
                    }
                }
                ref len => {
                    let px = len.to_px_with_viewport(font_size, root_font_size, cb_w, vp_h);
                    if px > 0.0 {
                        if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                            (px - total_h_p_b).max(0.0)
                        } else {
                            px
                        }
                    } else {
                        0.0
                    }
                }
            }
        };
    let container_content_h =
        if is_definite_container && container.dimensions.content.size.height > 0.0 {
            container.dimensions.content.size.height
        } else {
            match style.height {
                Length::Auto => 0.0,
                Length::Percent(p) => {
                    if cb_h > 0.0 {
                        let px = (p / 100.0) * cb_h;
                        if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                            (px - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
                        } else {
                            px
                        }
                    } else {
                        0.0
                    }
                }
                ref len => {
                    let px = len.to_px_with_viewport(font_size, root_font_size, cb_h, vp_h);
                    if px > 0.0 {
                        if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                            (px - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
                        } else {
                            px
                        }
                    } else {
                        0.0
                    }
                }
            }
        };
    let child_cb_w = if container_content_w > 0.0 {
        container_content_w
    } else {
        cb_w
    };
    let child_cb_h = if container_content_h > 0.0 {
        container_content_h
    } else {
        cb_h
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
            let mut ts = to_taffy_style(cs, child_cb_w, child_cb_h);
            if cs.visibility == mango_css::values::Visibility::Collapse {
                ts.display = taffy::style::Display::None;
            }
            ts
        } else {
            taffy::style::Style::default()
        };

        let is_row = !is_grid
            && matches!(
                style.flex_direction,
                mango_css::values::FlexDirection::Row
                    | mango_css::values::FlexDirection::RowReverse
            );
        let s_opt = child.style.as_deref();
        let overflow_hidden = !is_grid
            && s_opt.is_some_and(|s| {
                if is_row {
                    s.overflow_x != mango_css::values::Overflow::Visible
                } else {
                    s.overflow_y != mango_css::values::Overflow::Visible
                }
            });
        let (int_w, int_h) = compute_child_intrinsic_size(child, child_cb_w, child_cb_h, font_size);

        // If replaced element with intrinsic sizing, ensure initial size is populated (unless aspect_ratio is defined)
        if taffy_style.aspect_ratio.is_none() {
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
        }

        // Automatic minimum size for flex items (CSS Flexbox spec Section 4.5):
        // In a row flex container, min-width: auto resolves to min-content width.
        // In a column flex container, min-height: auto resolves to min-content height.
        // If the item has non-visible overflow along that axis (e.g. overflow: hidden),
        // the automatic minimum size is 0.
        if !is_grid {
            let is_bb = taffy_style.box_sizing == taffy::style::BoxSizing::BorderBox;
            let (pad_h, pad_v) = if is_bb {
                child_padding_and_border(
                    child_style,
                    font_size,
                    root_font_size,
                    child_cb_w,
                    child_cb_h,
                )
            } else {
                (0.0, 0.0)
            };
            let eff_min_w = int_w + pad_h;
            let eff_min_h = int_h + pad_v;
            if !overflow_hidden {
                if is_row && s_opt.map(|s| s.min_width.is_auto()).unwrap_or(true) && int_w > 0.0 {
                    taffy_style.min_size.width = taffy::style_helpers::length(eff_min_w);
                } else if !is_row
                    && s_opt.map(|s| s.min_height.is_auto()).unwrap_or(true)
                    && int_h > 0.0
                {
                    taffy_style.min_size.height = taffy::style_helpers::length(eff_min_h);
                }
            } else if is_row && s_opt.map(|s| s.min_width.is_auto()).unwrap_or(true) {
                taffy_style.min_size.width = taffy::style_helpers::zero();
            } else if !is_row && s_opt.map(|s| s.min_height.is_auto()).unwrap_or(true) {
                taffy_style.min_size.height = taffy::style_helpers::zero();
            }
        }

        let eff_int_w = int_w;
        let eff_int_h = int_h;

        let child_node = match tree.new_leaf_with_context(
            taffy_style,
            LeafContext {
                intrinsic_width: eff_int_w,
                intrinsic_height: eff_int_h,
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
    let avail_w = if is_definite_container && container.dimensions.content.size.width > 0.0 {
        AvailableSpace::Definite(container.dimensions.content.size.width)
    } else if cb_w > 0.0 {
        AvailableSpace::Definite(cb_w)
    } else {
        AvailableSpace::MinContent
    };
    let avail_h = if is_definite_container && container.dimensions.content.size.height > 0.0 {
        AvailableSpace::Definite(container.dimensions.content.size.height)
    } else if cb_h > 0.0 {
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
                    |known_dims, _avail| {
                        let eff_w = known_dims.width.unwrap_or(int_w);
                        let h = match (known_dims.height, style.aspect_ratio) {
                            (Some(h), _) => h,
                            (None, Some(r)) if r > 0.0 && known_dims.width.is_some() => eff_w / r,
                            (None, _) => int_h,
                        };
                        let w = match (known_dims.width, style.aspect_ratio) {
                            (Some(w), _) => w,
                            (None, Some(r)) if r > 0.0 && known_dims.height.is_some() => h * r,
                            _ => eff_w,
                        };
                        taffy::geometry::Size {
                            width: w,
                            height: h,
                        }
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

    let mut content_w = root_layout.content_box_width().max(0.0);
    let content_h = root_layout.content_box_height().max(0.0);

    let is_flex_row = matches!(
        style.flex_direction,
        mango_css::values::FlexDirection::Row | mango_css::values::FlexDirection::RowReverse
    );
    // If auto-width flex row container without wrap, correct for Taffy clamping negative margins
    if !is_grid
        && is_flex_row
        && style.width == mango_css::values::Length::Auto
        && style.flex_wrap == mango_css::values::FlexWrap::NoWrap
        && !child_nodes.is_empty()
    {
        let mut max_right = 0.0f32;
        for &node in &child_nodes {
            if let Ok(l) = tree.layout(node) {
                max_right = max_right.max(l.location.x + l.size.width);
            }
        }
        if max_right > 0.0 && max_right < content_w {
            content_w = max_right;
        }
    }

    container.dimensions.content.size.width = content_w;
    container.dimensions.content.size.height = content_h;

    // Parent border box origin
    let container_border_x = container_x - pad_left - border_left;
    let container_border_y = container_y - pad_top - border_top;
    let box_traps_fixed = !style.transform.is_identity()
        || !style.filter.is_empty()
        || !style.backdrop_filter.is_empty()
        || matches!(
            style.contain,
            mango_css::values::Contain::Paint
                | mango_css::values::Contain::Strict
                | mango_css::values::Contain::Layout
                | mango_css::values::Contain::Content
        )
        || style.will_change;

    for (&child_idx, &node) in in_flow_indices.iter().zip(child_nodes.iter()) {
        let child_layout = match tree.layout(node) {
            Ok(l) => l,
            Err(_) => continue,
        };

        let child = &mut container.children[child_idx];
        let to_edge = |r: taffy::Rect<f32>| EdgeSizes::new(r.top, r.right, r.bottom, r.left);
        child.dimensions.padding = to_edge(child_layout.padding);
        child.dimensions.border = to_edge(child_layout.border);
        child.dimensions.margin = to_edge(child_layout.margin);

        let local_x = child_layout.content_box_x();
        let local_y = child_layout.content_box_y();
        child.dimensions.content.origin =
            Point::new(container_border_x + local_x, container_border_y + local_y);

        let w = child_layout.content_box_width().max(0.0);
        let h = child_layout.content_box_height().max(0.0);
        child.dimensions.content.size = Size::new(w, h);
        if box_traps_fixed {
            child.fixed_containing_block = Some(container.dimensions);
        } else if child.fixed_containing_block.is_none() {
            child.fixed_containing_block = container.fixed_containing_block;
        }

        // Lay out descendant subtree of child
        let mut child_dims = child.dimensions;
        if !is_grid
            && matches!(
                style.flex_direction,
                FlexDirection::Column | FlexDirection::ColumnReverse
            )
        {
            let container_h_definite = match style.height {
                Length::Auto => false,
                Length::Percent(_) => cb_h > 0.0,
                ref len => len.to_px_with_viewport(font_size, root_font_size, cb_h, vp_h) > 0.0,
            };
            let child_h_definite = child
                .style
                .as_ref()
                .map(|s| match s.height {
                    Length::Auto | Length::Percent(_) => false,
                    ref len => len.to_px_with_viewport(font_size, root_font_size, cb_h, vp_h) > 0.0,
                })
                .unwrap_or(false);
            if !container_h_definite && !child_h_definite {
                child_dims.height_indefinite = true;
                child.dimensions.height_indefinite = true;
            }
        }
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
    if !is_pos && !is_ifc && (is_margin_left_auto || is_margin_right_auto) && cb_w > 0.0 {
        let underflow = (cb_w - content_w - total_h_p_b).max(0.0);
        let half_or_full = |is_auto, other_auto| {
            if is_auto && other_auto {
                underflow / 2.0
            } else if is_auto {
                underflow
            } else {
                0.0
            }
        };
        let new_margin_left = half_or_full(is_margin_left_auto, is_margin_right_auto);
        let new_margin_right = half_or_full(is_margin_right_auto, is_margin_left_auto);

        let shift_x = new_margin_left - container.dimensions.margin.left;
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
            container.dimensions.content.origin.x,
            container.dimensions.content.origin.y,
            content_w,
            content_h,
        );
    }
    layout_positioned_children_with_cb(container, containing_block, float_ctx);

    true
}
