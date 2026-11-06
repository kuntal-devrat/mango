//! Flex container intrinsic min/max content measurement.

use std::collections::HashMap;

use mango_css::values::{BoxSizing, Display, Length};
use mango_render::{FontFamily, FontWeight};

use crate::box_tree::LayoutBox;

use super::intrinsic::{
    child_style_height, get_node_horizontal_spacing, measure_box_intrinsic_widths_with_height,
};

/// Measures intrinsic min-content and max-content width for flex containers.
pub(crate) fn measure_flex_container_intrinsic_widths(
    node: &LayoutBox,
    font_size: f32,
    weight: FontWeight,
    family: FontFamily,
    memo: &mut HashMap<usize, (f32, f32)>,
    containing_height: Option<f32>,
) -> (f32, f32) {
    let is_vertical_wm = node
        .style
        .as_ref()
        .map(|s| s.writing_mode.is_vertical())
        .unwrap_or(false);
    let is_flex_dir_row = matches!(
        node.style.as_ref().map(|s| s.flex_direction),
        Some(mango_css::values::FlexDirection::Row)
            | Some(mango_css::values::FlexDirection::RowReverse)
            | None
    );
    let is_row = if is_vertical_wm {
        !is_flex_dir_row
    } else {
        is_flex_dir_row
    };
    let is_wrap = node
        .style
        .as_ref()
        .map(|s| s.flex_wrap.is_wrap())
        .unwrap_or(false);
    let is_balanced = node
        .style
        .as_ref()
        .map(|s| s.flex_wrap.is_balanced())
        .unwrap_or(false);
    let flex_line_count = node.style.as_ref().and_then(|s| s.flex_line_count);
    let col_gap = node
        .style
        .as_ref()
        .map(|s| {
            let gap_len = if is_flex_dir_row {
                &s.column_gap
            } else {
                &s.row_gap
            };
            gap_len.to_px(s.font_size, s.root_font_size, 0.0)
        })
        .unwrap_or(0.0);
    let mut min_w = 0.0f32;
    let mut max_w = 0.0f32;
    let mut child_count = 0;
    let mut child_max_widths = Vec::new();
    let flex_height = child_style_height(node, containing_height);
    for child in &node.children {
        let child_style = child.style.as_ref();
        if child_style
            .map(|s| {
                s.display == Display::None
                    || s.visibility == mango_css::values::Visibility::Collapse
                    || matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
            })
            .unwrap_or(false)
        {
            continue;
        }
        let child_margin_h = child_style
            .map(|s| {
                let fs = s.font_size;
                let rfs = s.root_font_size;
                let ml = match s.margin_left {
                    Length::Px(px) => px,
                    Length::Rem(rem) => rem * rfs,
                    Length::Em(em) => em * fs,
                    _ => 0.0,
                };
                let mr = match s.margin_right {
                    Length::Px(px) => px,
                    Length::Rem(rem) => rem * rfs,
                    Length::Em(em) => em * fs,
                    _ => 0.0,
                };
                ml + mr
            })
            .unwrap_or(0.0);
        let explicit_w = child_style.and_then(|s| {
            let to_px = |len: &Length| match len {
                Length::Px(px) if *px > 0.0 => Some(*px),
                Length::Rem(rem) if *rem > 0.0 => Some(*rem * s.root_font_size),
                Length::Em(em) if *em > 0.0 => Some(*em * s.font_size),
                _ => None,
            };
            to_px(&s.flex_basis).or_else(|| to_px(&s.width))
        });
        let ratio = child_style
            .and_then(|s| s.aspect_ratio.filter(|r| *r > 0.0))
            .or_else(|| match &child.box_type {
                crate::box_model::BoxType::ReplacedElement {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | crate::box_model::BoxType::IFrame {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | crate::box_model::BoxType::Video {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | crate::box_model::BoxType::Canvas {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                } if *intrinsic_height > 0.0 => Some(*intrinsic_width / *intrinsic_height),
                _ => None,
            });

        let aspect_ratio_width = if let Some(r) = ratio {
            if explicit_w.is_none() && child_style.map(|s| s.width == Length::Auto).unwrap_or(true)
            {
                if !is_row {
                    let height = match child_style.map(|s| &s.height) {
                        Some(Length::Px(px)) if *px > 0.0 => Some(*px),
                        _ if flex_height > 0.0 => Some(flex_height),
                        _ => None,
                    };
                    height.map(|h| h * r)
                } else {
                    let align = child_style
                        .map(|s| s.align_self)
                        .unwrap_or(mango_css::values::AlignSelf::Auto);
                    let is_stretch = match align {
                        mango_css::values::AlignSelf::Stretch => true,
                        mango_css::values::AlignSelf::Auto => matches!(
                            node.style
                                .as_ref()
                                .map(|s| s.align_items)
                                .unwrap_or(mango_css::values::AlignItems::Stretch),
                            mango_css::values::AlignItems::Normal
                                | mango_css::values::AlignItems::Stretch
                        ),
                        _ => false,
                    };
                    let has_auto_margin_v = child_style
                        .map(|s| s.margin_top == Length::Auto || s.margin_bottom == Length::Auto)
                        .unwrap_or(false);
                    if is_stretch && !has_auto_margin_v && flex_height > 0.0 {
                        Some(flex_height * r)
                    } else {
                        match child_style.map(|s| &s.height) {
                            Some(Length::Px(px)) if *px > 0.0 => Some(*px * r),
                            _ => None,
                        }
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let child_non_content_h = get_node_horizontal_spacing(child, font_size);
        let is_child_border_box = child_style
            .map(|s| s.box_sizing == BoxSizing::BorderBox)
            .unwrap_or(false);
        let child_extra_w = if is_child_border_box {
            child_margin_h
        } else {
            child_non_content_h
        };
        let (c_min, c_max) = if let Some(w) = explicit_w {
            (w + child_extra_w, w + child_extra_w)
        } else if let Some(w) = aspect_ratio_width {
            (w + child_extra_w, w + child_extra_w)
        } else {
            measure_box_intrinsic_widths_with_height(
                child,
                font_size,
                weight,
                family,
                memo,
                if !is_row { Some(flex_height) } else { None },
            )
        };
        child_max_widths.push(c_max);
        if is_row {
            if is_wrap {
                min_w = min_w.max(c_min);
            } else {
                min_w += c_min;
            }
            max_w += c_max;
            child_count += 1;
        } else {
            min_w = min_w.max(c_min);
            max_w = max_w.max(c_max);
            child_count += 1;
        }
    }
    if is_row {
        if is_balanced && flex_line_count.unwrap_or(0) > 1 {
            let k = flex_line_count.unwrap() as usize;
            max_w = super::balance::balance_intrinsic_max_content(&child_max_widths, k, col_gap);
        } else if child_count > 1 {
            let total_gap = (child_count - 1) as f32 * col_gap;
            max_w += total_gap;
            if !is_wrap {
                min_w += total_gap;
            }
        }
    } else if is_balanced && flex_line_count.unwrap_or(0) > 1 {
        let k = (flex_line_count.unwrap() as usize).min(child_count).max(1);
        if k > 1 {
            let col_total: f32 = max_w * k as f32 + (k - 1) as f32 * col_gap;
            max_w = col_total;
        }
    }
    let is_nowrap = node.style.as_ref().is_some_and(|s| {
        matches!(
            s.white_space,
            mango_css::values::WhiteSpace::Nowrap | mango_css::values::WhiteSpace::Pre
        )
    });
    if is_nowrap {
        min_w = min_w.max(max_w);
    }
    let is_editable = node
        .get_attribute("contenteditable")
        .is_some_and(|v| v != "false")
        || node.get_attribute("_mango_contenteditable") == Some("true");
    if is_editable && child_count == 0 && is_vertical_wm {
        let line_h = node
            .style
            .as_ref()
            .map(|s| {
                let family = crate::inline_flow::shaping::font_family_for(s);
                let weight = crate::inline_flow::shaping::font_weight_for(s);
                crate::inline_flow::shaping::used_line_height(s, family, weight)
            })
            .unwrap_or(font_size);
        min_w = min_w.max(line_h);
        max_w = max_w.max(line_h);
    }
    if node
        .style
        .as_ref()
        .is_some_and(|s| s.overflow_y == mango_css::values::Overflow::Scroll)
    {
        min_w += 6.0;
        max_w += 6.0;
    }
    super::balance::apply_min_max_width_bounds(node, font_size, &mut min_w, &mut max_w);
    let non_content_h = get_node_horizontal_spacing(node, font_size);
    (
        min_w.max(0.0) + non_content_h,
        max_w.max(0.0) + non_content_h,
    )
}
