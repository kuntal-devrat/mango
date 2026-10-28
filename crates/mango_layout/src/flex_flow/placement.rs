use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::{
    AlignItems, AlignSelf, BoxSizing, JustifyContent, Length, Position, WritingMode,
};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::layout_flex_item;
use super::metrics::ItemMetric;

pub struct LinePlacementParams<'a> {
    pub line: &'a [usize],
    pub item_metrics: &'a [ItemMetric],
    pub container: &'a mut LayoutBox,
    pub style: &'a ComputedStyle,
    pub is_row: bool,
    pub is_reverse: bool,
    pub is_container_main_definite: bool,
    pub container_x: f32,
    pub container_y: f32,
    pub container_main_size: f32,
    pub container_height: f32,
    pub content_width: f32,
    pub line_cross_size: f32,
    pub line_baseline_ascent: f32,
    pub cross_cursor: f32,
    pub main_cursor: f32,
    pub item_spacing: f32,
}

pub fn position_flex_items_on_line(params: LinePlacementParams) {
    let LinePlacementParams {
        line,
        item_metrics,
        container,
        style,
        is_row,
        is_reverse,
        is_container_main_definite: _,
        container_x,
        container_y,
        container_main_size,
        container_height,
        content_width,
        line_cross_size,
        line_baseline_ascent,
        cross_cursor,
        mut main_cursor,
        item_spacing,
    } = params;

    let item_indices: Vec<usize> = line.to_vec();

    for &metric_idx in &item_indices {
        let m = &item_metrics[metric_idx];
        let child = &mut container.children[m.index];
        let child_style = child.style.clone().unwrap_or_default();

        let item_align = match child_style.align_self {
            AlignSelf::Auto => style.align_items,
            AlignSelf::Stretch => AlignItems::Stretch,
            AlignSelf::FlexStart => AlignItems::FlexStart,
            AlignSelf::FlexEnd => AlignItems::FlexEnd,
            AlignSelf::Center => AlignItems::Center,
            AlignSelf::Baseline => AlignItems::Baseline,
            AlignSelf::SafeCenter => AlignItems::SafeCenter,
            AlignSelf::SafeFlexStart => AlignItems::SafeFlexStart,
            AlignSelf::SafeFlexEnd => AlignItems::SafeFlexEnd,
            AlignSelf::SafeStart => AlignItems::SafeStart,
            AlignSelf::SafeEnd => AlignItems::SafeEnd,
        };

        let is_child_cross_auto = if is_row {
            child_style.height == Length::Auto
                || (matches!(child_style.height, Length::Percent(_))
                    && container
                        .style
                        .as_ref()
                        .map(|s| s.height == Length::Auto)
                        .unwrap_or(true))
        } else {
            child_style.width == Length::Auto
                || matches!(child_style.width, Length::Percent(p) if p >= 99.9)
        };

        let has_cross_auto_margin = if is_row {
            child_style.margin_top == Length::Auto || child_style.margin_bottom == Length::Auto
        } else {
            child_style.margin_left == Length::Auto || child_style.margin_right == Length::Auto
        };

        let c_fs = child_style.font_size;
        let c_root_fs = child_style.root_font_size;

        let mut item_cross = m.cross;
        if matches!(item_align, AlignItems::Normal | AlignItems::Stretch)
            && is_child_cross_auto
            && !has_cross_auto_margin
        {
            item_cross = (line_cross_size - m.pad_cross - m.border_cross - m.margin_cross).max(0.0);
        }

        let eff_cross_cb = if is_row {
            if container_height > 0.0 {
                container_height
            } else {
                line_cross_size
            }
        } else {
            if content_width > 0.0 {
                content_width
            } else {
                line_cross_size
            }
        };

        if is_row {
            let is_max_h_pct_indefinite =
                child_style.max_height.has_percent() && eff_cross_cb <= 0.0;
            if child_style.max_height != Length::Auto && !is_max_h_pct_indefinite {
                let max_h = child_style.max_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    eff_cross_cb,
                    container_height,
                );
                let max_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_h - m.pad_cross - m.border_cross).max(0.0)
                } else {
                    max_h
                };
                item_cross = item_cross.min(max_content_h);
            }
            if child_style.min_height != Length::Auto {
                let min_h = child_style.min_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    eff_cross_cb,
                    container_height,
                );
                let min_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_h - m.pad_cross - m.border_cross).max(0.0)
                } else {
                    min_h
                };
                item_cross = item_cross.max(min_content_h);
            }
        } else {
            if child_style.max_width != Length::Auto {
                let max_w = child_style.max_width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    eff_cross_cb,
                    container_height,
                );
                let max_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_w - m.pad_cross - m.border_cross).max(0.0)
                } else {
                    max_w
                };
                item_cross = item_cross.min(max_content_w);
            }
            if child_style.min_width != Length::Auto {
                let min_w = child_style.min_width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    eff_cross_cb,
                    container_height,
                );
                let min_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_w - m.pad_cross - m.border_cross).max(0.0)
                } else {
                    min_w
                };
                item_cross = item_cross.max(min_content_w);
            }
        }

        let mut cross_offset = 0.0;
        if has_cross_auto_margin {
            let free_cross = (line_cross_size
                - (item_cross + m.pad_cross + m.border_cross + m.margin_cross))
                .max(0.0);
            if is_row {
                let is_top_auto = child_style.margin_top == Length::Auto;
                let is_bottom_auto = child_style.margin_bottom == Length::Auto;
                if is_top_auto && is_bottom_auto {
                    child.dimensions.margin.top = free_cross / 2.0;
                    child.dimensions.margin.bottom = free_cross / 2.0;
                } else if is_top_auto {
                    child.dimensions.margin.top = free_cross;
                } else if is_bottom_auto {
                    child.dimensions.margin.bottom = free_cross;
                }
            } else {
                let is_left_auto = child_style.margin_left == Length::Auto;
                let is_right_auto = child_style.margin_right == Length::Auto;
                if is_left_auto && is_right_auto {
                    child.dimensions.margin.left = free_cross / 2.0;
                    child.dimensions.margin.right = free_cross / 2.0;
                } else if is_left_auto {
                    child.dimensions.margin.left = free_cross;
                } else if is_right_auto {
                    child.dimensions.margin.right = free_cross;
                }
            }
        } else {
            let is_cross_reversed = if is_row {
                if style.writing_mode.is_vertical() {
                    (style.direction == mango_css::values::Direction::Rtl)
                        ^ (style.flex_wrap == mango_css::values::FlexWrap::WrapReverse)
                } else {
                    style.flex_wrap == mango_css::values::FlexWrap::WrapReverse
                }
            } else if style.writing_mode.is_vertical() {
                matches!(
                    style.writing_mode,
                    WritingMode::VerticalRl | WritingMode::SidewaysRl
                ) ^ (style.flex_wrap == mango_css::values::FlexWrap::WrapReverse)
            } else {
                (style.direction == mango_css::values::Direction::Rtl)
                    ^ (style.flex_wrap == mango_css::values::FlexWrap::WrapReverse)
            };
            let free =
                line_cross_size - (item_cross + m.pad_cross + m.border_cross + m.margin_cross);
            cross_offset = match item_align {
                AlignItems::Normal
                | AlignItems::FlexStart
                | AlignItems::Stretch
                | AlignItems::SafeFlexStart
                | AlignItems::SafeStart => {
                    if is_cross_reversed {
                        free
                    } else {
                        0.0
                    }
                }
                AlignItems::Baseline => {
                    if is_row && line_baseline_ascent > 0.0 {
                        (line_baseline_ascent - m.baseline_ascent).max(0.0)
                    } else if is_cross_reversed {
                        free
                    } else {
                        0.0
                    }
                }
                AlignItems::FlexEnd | AlignItems::SafeFlexEnd | AlignItems::SafeEnd => {
                    let off = if is_cross_reversed { 0.0 } else { free };
                    if matches!(item_align, AlignItems::SafeFlexEnd | AlignItems::SafeEnd) {
                        off.max(0.0)
                    } else {
                        off
                    }
                }
                AlignItems::Center | AlignItems::SafeCenter => {
                    let off = free / 2.0;
                    if item_align == AlignItems::SafeCenter {
                        off.max(0.0)
                    } else {
                        off
                    }
                }
            };
        }

        let item_outer_margin = if is_row {
            child.dimensions.margin.horizontal()
        } else {
            child.dimensions.margin.vertical()
        };
        let item_outer_main = m.final_main + m.pad_main + m.border_main + item_outer_margin;
        let pre_x = child.dimensions.content.origin.x;
        let pre_y = child.dimensions.content.origin.y;
        let pre_w = child.dimensions.content.size.width;
        let pre_h = child.dimensions.content.size.height;

        let (target_w, target_h, new_x, new_y) = if is_row {
            let x = if is_reverse {
                container_x + container_main_size - main_cursor - item_outer_main
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
            } else {
                container_x
                    + main_cursor
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
            };
            let y = container_y
                + cross_cursor
                + cross_offset
                + child.dimensions.margin.top
                + child.dimensions.border.top
                + child.dimensions.padding.top;
            child.dimensions.content = Rect::new(x, y, m.final_main, item_cross);
            if !m.is_collapsed {
                main_cursor += item_outer_main + item_spacing;
            }
            (m.final_main, item_cross, x, y)
        } else {
            let x = container_x
                + cross_cursor
                + cross_offset
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left;
            let y = if is_reverse {
                container_y + container_main_size - main_cursor - item_outer_main
                    + child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
            } else {
                container_y
                    + main_cursor
                    + child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
            };
            child.dimensions.content = Rect::new(x, y, item_cross, m.final_main);
            if !m.is_collapsed {
                main_cursor += item_outer_main + item_spacing;
            }
            (item_cross, m.final_main, x, y)
        };

        let was_stretched = matches!(item_align, AlignItems::Normal | AlignItems::Stretch)
            && is_child_cross_auto
            && (item_cross - m.cross).abs() > 0.5;

        let size_unchanged = pre_w > 0.0
            && pre_h > 0.0
            && (pre_w - target_w).abs() < 0.5
            && (pre_h - target_h).abs() < 0.5;

        if size_unchanged && !was_stretched {
            let dx = new_x - pre_x;
            let dy = new_y - pre_y;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                crate::block_flow::shift_descendants(child, dx, dy);
            }
        } else {
            let is_child_positioned = child
                .style
                .as_ref()
                .map(|s| !matches!(s.position, Position::Static))
                .unwrap_or(false);
            if is_row {
                child.dimensions.content.size.width = m.final_main;
                child.dimensions.content.size.height = item_cross;
            } else {
                child.dimensions.content.size.height = m.final_main;
                child.dimensions.content.size.width = item_cross;
            }
            let mut item_cb_dims = child.dimensions;
            item_cb_dims.content.origin.x = 0.0;
            item_cb_dims.content.origin.y = 0.0;
            let is_item_basis_definite = child
                .style
                .as_ref()
                .map(|s| {
                    (s.height != Length::Auto && !matches!(s.height, Length::Percent(_)))
                        || (!s.flex_basis.is_auto()
                            && !s.flex_basis.is_content()
                            && !matches!(
                                s.flex_basis,
                                Length::Percent(_)
                                    | Length::MinContent
                                    | Length::MaxContent
                                    | Length::FitContent
                            ))
                })
                .unwrap_or(false);
            let is_container_strictly_definite = if is_row {
                true
            } else {
                style.height != Length::Auto && !matches!(style.height, Length::Percent(_))
                    || (matches!(style.height, Length::Percent(_)) && container_height > 0.0)
            };
            if !is_row && !is_container_strictly_definite && !is_item_basis_definite {
                item_cb_dims.content.size.height = 0.0;
                child.dimensions.content.size.height = 0.0;
            }
            let item_containing_block = if is_child_positioned {
                Dimensions::new(child.dimensions.border_box())
            } else {
                item_cb_dims
            };
            let saved_padding = child.dimensions.padding;
            let saved_border = child.dimensions.border;
            let saved_margin = child.dimensions.margin;
            let saved_origin = child.dimensions.content.origin;
            let mut item_float_ctx = FloatContext::new();
            let c_fs = child.style.as_ref().map(|s| s.font_size).unwrap_or(16.0);
            layout_flex_item(child, &item_containing_block, &mut item_float_ctx, c_fs);
            if is_row {
                child.dimensions.content.size.width = m.final_main;
                child.dimensions.content.size.height = item_cross;
            } else {
                child.dimensions.content.size.height = m.final_main;
                child.dimensions.content.size.width = item_cross;
            }
            child.dimensions.padding = saved_padding;
            child.dimensions.border = saved_border;
            child.dimensions.margin = saved_margin;
            child.dimensions.content.origin = saved_origin;
        }
    }
}

pub struct ContainerFinalizeParams<'a> {
    pub container: &'a mut LayoutBox,
    pub style: &'a ComputedStyle,
    pub is_row: bool,
    pub is_percent_indefinite: bool,
    pub font_size: f32,
    pub root_font_size: f32,
    pub container_height: f32,
    pub content_height: f32,
    pub cross_cursor: f32,
    pub cross_gap: f32,
    pub container_y: f32,
    pub pad_top: f32,
    pub pad_bottom: f32,
    pub border_top: f32,
    pub border_bottom: f32,
}

pub fn finalize_container_height(params: ContainerFinalizeParams) {
    let ContainerFinalizeParams {
        container,
        style,
        is_row,
        is_percent_indefinite,
        font_size,
        root_font_size,
        container_height,
        content_height,
        cross_cursor,
        cross_gap,
        container_y,
        pad_top,
        pad_bottom,
        border_top,
        border_bottom,
    } = params;

    let is_height_content = matches!(
        style.height,
        Length::Auto
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent
            | Length::Content
    );

    if !is_height_content && !is_percent_indefinite {
        let h = style.height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            container.dimensions.content.size.height =
                (h - pad_top - pad_bottom - border_top - border_bottom).max(0.0);
        } else {
            container.dimensions.content.size.height = h;
        }
    } else if content_height > 0.0 {
        container.dimensions.content.size.height = content_height;
    } else if is_row {
        container.dimensions.content.size.height = (cross_cursor - cross_gap).max(0.0);
    } else {
        let max_bottom = container
            .children
            .iter()
            .map(|c| c.dimensions.margin_box().bottom())
            .fold(container_y, f32::max);
        container.dimensions.content.size.height = (max_bottom - container_y).max(0.0);
    }

    // Clamp container height by min-height and max-height constraints (G1)
    if style.max_height != Length::Auto {
        let max_h = style.max_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (max_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        } else {
            max_h
        };
        container.dimensions.content.size.height =
            container.dimensions.content.size.height.min(max_content_h);
    }
    if style.min_height != Length::Auto {
        let min_h = style.min_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (min_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        } else {
            min_h
        };
        container.dimensions.content.size.height =
            container.dimensions.content.size.height.max(min_content_h);
    }
}

pub fn apply_wrap_reverse(
    container: &mut LayoutBox,
    is_row: bool,
    container_x: f32,
    container_y: f32,
) {
    if is_row {
        let total_h = container.dimensions.content.height();
        for child in &mut container.children {
            if child
                .style
                .as_ref()
                .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                .unwrap_or(false)
            {
                continue;
            }
            let orig_y = child.dimensions.content.y() - container_y;
            let new_y = container_y + total_h - orig_y - child.dimensions.border_box().height();
            let dy = new_y - child.dimensions.content.y();
            child.dimensions.content.origin.y = new_y;
            crate::block_flow::shift_descendants(child, 0.0, dy);
        }
    } else {
        let total_w = container.dimensions.content.width();
        for child in &mut container.children {
            if child
                .style
                .as_ref()
                .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                .unwrap_or(false)
            {
                continue;
            }
            let orig_x = child.dimensions.content.x() - container_x;
            let new_x = container_x + total_w - orig_x - child.dimensions.border_box().width();
            let dx = new_x - child.dimensions.content.x();
            child.dimensions.content.origin.x = new_x;
            crate::block_flow::shift_descendants(child, dx, 0.0);
        }
    }
}

/// Computes the spec-compliant static position for absolutely-positioned children
/// of a flex container (CSS Flexible Box Layout Module Level 1 § 4.1).
///
/// The static position of an abspos child is aligned within the flex container's
/// content box according to `justify-content` on the main axis and `align-self` / `align-items`
/// on the cross axis, taking into account writing modes and directional flow.
pub fn calculate_abspos_static_positions(
    container: &mut LayoutBox,
    style: &ComputedStyle,
    is_row: bool,
    is_reverse: bool,
    container_x: f32,
    container_y: f32,
    content_width: f32,
    content_height: f32,
) {
    let (main_size, cross_size) = if is_row {
        (content_width, content_height)
    } else {
        (content_height, content_width)
    };

    for child in &mut container.children {
        let child_style = child.style.clone().unwrap_or_default();
        if !matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        let c_fs = child_style.font_size;
        let c_root_fs = child_style.root_font_size;

        let border_l = child_style.border_left_width;
        let border_r = child_style.border_right_width;
        let border_t = child_style.border_top_width;
        let border_b = child_style.border_bottom_width;

        let pad_l = child_style.padding_left.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            content_height,
        );
        let pad_r = child_style.padding_right.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            content_height,
        );
        let pad_t = child_style.padding_top.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            content_height,
        );
        let pad_b = child_style.padding_bottom.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            content_height,
        );

        let item_w = if child_style.width != Length::Auto {
            let raw_w = child_style.width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                content_height,
            );
            if child_style.box_sizing == BoxSizing::BorderBox {
                (raw_w - pad_l - pad_r - border_l - border_r).max(0.0)
            } else {
                raw_w
            }
        } else {
            0.0
        };

        let item_h = if child_style.height != Length::Auto {
            let raw_h = child_style.height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_height,
                content_height,
            );
            if child_style.box_sizing == BoxSizing::BorderBox {
                (raw_h - pad_t - pad_b - border_t - border_b).max(0.0)
            } else {
                raw_h
            }
        } else {
            0.0
        };

        let border_box_w = item_w + pad_l + pad_r + border_l + border_r;
        let border_box_h = item_h + pad_t + pad_b + border_t + border_b;

        let margin_l = if child_style.margin_left == Length::Auto {
            0.0
        } else {
            child_style.margin_left.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                content_height,
            )
        };
        let margin_r = if child_style.margin_right == Length::Auto {
            0.0
        } else {
            child_style.margin_right.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                content_height,
            )
        };
        let margin_t = if child_style.margin_top == Length::Auto {
            0.0
        } else {
            child_style.margin_top.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                content_height,
            )
        };
        let margin_b = if child_style.margin_bottom == Length::Auto {
            0.0
        } else {
            child_style.margin_bottom.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                content_height,
            )
        };

        let margin_box_w = border_box_w + margin_l + margin_r;
        let margin_box_h = border_box_h + margin_t + margin_b;

        let (item_main_size, item_cross_size) = if is_row {
            (margin_box_w, margin_box_h)
        } else {
            (margin_box_h, margin_box_w)
        };

        // 1. Main axis alignment from justify-content
        let free_main = main_size - item_main_size;
        let main_offset = match style.justify_content {
            JustifyContent::Normal | JustifyContent::FlexStart => 0.0,
            JustifyContent::FlexEnd => free_main,
            JustifyContent::Center | JustifyContent::SpaceAround | JustifyContent::SpaceEvenly => {
                free_main / 2.0
            }
            JustifyContent::SpaceBetween => 0.0,
            JustifyContent::Start => {
                let is_dir_reverse = matches!(
                    style.flex_direction,
                    mango_css::values::FlexDirection::RowReverse
                        | mango_css::values::FlexDirection::ColumnReverse
                );
                if is_dir_reverse { free_main } else { 0.0 }
            }
            JustifyContent::End => {
                let is_dir_reverse = matches!(
                    style.flex_direction,
                    mango_css::values::FlexDirection::RowReverse
                        | mango_css::values::FlexDirection::ColumnReverse
                );
                if is_dir_reverse { 0.0 } else { free_main }
            }
            JustifyContent::Left => {
                if is_row {
                    if is_reverse { free_main } else { 0.0 }
                } else {
                    let is_dir_reverse = matches!(
                        style.flex_direction,
                        mango_css::values::FlexDirection::RowReverse
                            | mango_css::values::FlexDirection::ColumnReverse
                    );
                    if is_dir_reverse { free_main } else { 0.0 }
                }
            }
            JustifyContent::Right => {
                if is_row {
                    if is_reverse { 0.0 } else { free_main }
                } else {
                    let is_dir_reverse = matches!(
                        style.flex_direction,
                        mango_css::values::FlexDirection::RowReverse
                            | mango_css::values::FlexDirection::ColumnReverse
                    );
                    if is_dir_reverse { free_main } else { 0.0 }
                }
            }
        };

        let main_pos = if is_reverse {
            (main_size - item_main_size) - main_offset
        } else {
            main_offset
        };

        // 2. Cross axis alignment from align-self / align-items
        let align = match child_style.align_self {
            AlignSelf::Auto => style.align_items,
            AlignSelf::Stretch => AlignItems::Stretch,
            AlignSelf::FlexStart => AlignItems::FlexStart,
            AlignSelf::FlexEnd => AlignItems::FlexEnd,
            AlignSelf::Center => AlignItems::Center,
            AlignSelf::Baseline => AlignItems::Baseline,
            AlignSelf::SafeCenter => AlignItems::SafeCenter,
            AlignSelf::SafeFlexStart => AlignItems::SafeFlexStart,
            AlignSelf::SafeFlexEnd => AlignItems::SafeFlexEnd,
            AlignSelf::SafeStart => AlignItems::SafeStart,
            AlignSelf::SafeEnd => AlignItems::SafeEnd,
        };

        let is_cross_reversed = (style.flex_wrap == mango_css::values::FlexWrap::WrapReverse)
            ^ (!is_row && style.direction == mango_css::values::Direction::Rtl);
        let free_cross = cross_size - item_cross_size;
        let cross_pos = match align {
            AlignItems::Normal
            | AlignItems::FlexStart
            | AlignItems::Stretch
            | AlignItems::Baseline
            | AlignItems::SafeFlexStart
            | AlignItems::SafeStart => {
                if is_cross_reversed {
                    free_cross
                } else {
                    0.0
                }
            }
            AlignItems::FlexEnd => {
                if is_cross_reversed {
                    0.0
                } else {
                    free_cross
                }
            }
            AlignItems::SafeFlexEnd | AlignItems::SafeEnd => {
                if is_cross_reversed {
                    0.0
                } else {
                    free_cross.max(0.0)
                }
            }
            AlignItems::Center => free_cross / 2.0,
            AlignItems::SafeCenter => (free_cross / 2.0).max(0.0),
        };

        let (static_x, static_y) = if is_row {
            (
                container_x + main_pos + margin_l,
                container_y + cross_pos + margin_t,
            )
        } else {
            (
                container_x + cross_pos + margin_l,
                container_y + main_pos + margin_t,
            )
        };

        // Note: positioned.rs will add margin_l / margin_t to base_x / base_y when positioning
        child.dimensions.content.origin.x = static_x - margin_l;
        child.dimensions.content.origin.y = static_y - margin_t;
    }
}

/// Resolves the main-axis gap for flex item placement on a line.
///
/// Ensures percentage gaps resolve against the inner main-axis size of the flex container.
#[inline]
pub fn resolve_item_placement_gap(
    style: &ComputedStyle,
    is_row: bool,
    container_main_size: f32,
    container_height: f32,
    is_container_main_definite: bool,
) -> f32 {
    super::lines::resolve_flex_main_gap(
        style,
        is_row,
        container_main_size,
        container_height,
        is_container_main_definite,
    )
}
