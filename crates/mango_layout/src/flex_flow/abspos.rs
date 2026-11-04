use mango_css::computed::ComputedStyle;
use mango_css::values::{AlignItems, AlignSelf, BoxSizing, JustifyContent, Length, Position};

use crate::box_tree::LayoutBox;

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
            AlignSelf::LastBaseline => AlignItems::LastBaseline,
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
            | AlignItems::LastBaseline
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
        child.static_position = Some(mango_core::Point::new(
            static_x - margin_l,
            static_y - margin_t,
        ));
    }
}
