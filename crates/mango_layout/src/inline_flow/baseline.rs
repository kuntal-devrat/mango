use mango_css::computed::ComputedStyle;
use mango_css::values::{
    AlignItems, AlignSelf, Display, FlexDirection, Float, Overflow, Position, VerticalAlign,
};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::shaping::{font_family_for, font_weight_for, used_line_height};

/// Computes the distance from the top margin edge of an inline-block to the baseline
/// of its last in-flow line box, adhering to CSS 2.1 §10.8.1 and CSS Flexbox §8.5.
pub fn find_inline_block_last_baseline_from_top(box_: &LayoutBox) -> Option<f32> {
    if let Some(s) = &box_.style
        && (s.overflow_x != Overflow::Visible || s.overflow_y != Overflow::Visible)
    {
        return None;
    }

    if matches!(box_.box_type, BoxType::TextNode(_)) {
        let fs = box_.style.as_ref().map(|s| s.font_size).unwrap_or(16.0);
        let default_style = ComputedStyle::default();
        let s = box_.style.as_deref().unwrap_or(&default_style);
        let family = font_family_for(s);
        let weight = font_weight_for(s);
        let (ascent, descent, _) =
            mango_render::font::font_manager().font_metrics(family, weight, fs);
        let natural_h = ascent + descent;
        let lh = used_line_height(s, family, weight);
        let half_leading = (lh - natural_h) / 2.0;
        return Some(ascent + half_leading);
    }

    if matches!(
        box_.tag_name.as_deref(),
        Some("input" | "button" | "select" | "textarea")
    ) {
        let fs = box_.style.as_ref().map(|s| s.font_size).unwrap_or(13.0);
        let default_style = ComputedStyle::default();
        let s = box_.style.as_deref().unwrap_or(&default_style);
        let family = font_family_for(s);
        let weight = font_weight_for(s);
        let (ascent, descent, _) =
            mango_render::font::font_manager().font_metrics(family, weight, fs);
        let natural_h = ascent + descent;
        let content_h = box_.dimensions.content.height();
        let half_leading = if content_h > natural_h {
            (content_h - natural_h) / 2.0
        } else {
            0.0
        };
        let margin_top =
            box_.dimensions.margin.top + box_.dimensions.border.top + box_.dimensions.padding.top;
        return Some(margin_top + half_leading + ascent);
    }

    let is_flex = matches!(
        box_.style.as_ref().map(|s| s.display),
        Some(Display::Flex) | Some(Display::InlineFlex)
    );

    let check_child = |child: &LayoutBox| -> Option<f32> {
        let is_in_flow = child
            .style
            .as_ref()
            .map(|s| {
                matches!(s.position, Position::Static | Position::Relative)
                    && s.float == Float::None
            })
            .unwrap_or(true);
        let is_visible_disp = child
            .style
            .as_ref()
            .map(|s| s.display != Display::None)
            .unwrap_or(true);

        let is_top_or_bottom_valign = child
            .style
            .as_ref()
            .map(|s| {
                matches!(
                    s.vertical_align,
                    VerticalAlign::Top
                        | VerticalAlign::TextTop
                        | VerticalAlign::Bottom
                        | VerticalAlign::TextBottom
                )
            })
            .unwrap_or(false);

        if is_in_flow
            && is_visible_disp
            && !is_top_or_bottom_valign
            && let Some(child_baseline) = find_inline_block_last_baseline_from_top(child)
        {
            let top_offset = child.dimensions.content.y() - box_.dimensions.content.y();
            let margin_top = box_.dimensions.margin.top
                + box_.dimensions.border.top
                + box_.dimensions.padding.top;
            return Some(margin_top + top_offset + child_baseline);
        }
        None
    };

    if is_flex {
        if let Some(b_str) = box_.get_attribute("_mango_flex_first_baseline")
            && let Ok(b_val) = b_str.parse::<f32>()
        {
            let margin_top = box_.dimensions.margin.top
                + box_.dimensions.border.top
                + box_.dimensions.padding.top;
            return Some(margin_top + b_val);
        }

        let is_row = box_
            .style
            .as_ref()
            .map(|s| {
                matches!(
                    s.flex_direction,
                    FlexDirection::Row | FlexDirection::RowReverse
                )
            })
            .unwrap_or(true);

        let in_flow_children: Vec<&LayoutBox> = box_
            .children
            .iter()
            .filter(|c| {
                c.style
                    .as_ref()
                    .map(|s| {
                        s.display != Display::None
                            && s.visibility == mango_css::values::Visibility::Visible
                            && !matches!(s.position, Position::Absolute | Position::Fixed)
                    })
                    .unwrap_or(true)
            })
            .collect();

        if in_flow_children.is_empty() {
            return None;
        }

        let is_wrap = box_
            .style
            .as_ref()
            .map(|s| s.flex_wrap.is_wrap())
            .unwrap_or(false);

        let first_line_children: Vec<&LayoutBox> = if !is_wrap {
            in_flow_children
        } else if is_row {
            let first_y = in_flow_children[0].dimensions.content.y();
            in_flow_children
                .into_iter()
                .filter(|c| (c.dimensions.content.y() - first_y).abs() < 2.0)
                .collect()
        } else {
            let first_x = in_flow_children[0].dimensions.content.x();
            in_flow_children
                .into_iter()
                .filter(|c| (c.dimensions.content.x() - first_x).abs() < 2.0)
                .collect()
        };

        let is_baseline_aligned = |c: &&LayoutBox| -> bool {
            let align = match c
                .style
                .as_ref()
                .map(|s| s.align_self)
                .unwrap_or(AlignSelf::Auto)
            {
                AlignSelf::Auto => box_
                    .style
                    .as_ref()
                    .map(|s| s.align_items)
                    .unwrap_or(AlignItems::Stretch),
                AlignSelf::Baseline => AlignItems::Baseline,
                AlignSelf::LastBaseline => AlignItems::LastBaseline,
                _ => AlignItems::Stretch,
            };
            matches!(align, AlignItems::Baseline | AlignItems::LastBaseline)
        };

        let candidates: Vec<&LayoutBox> =
            if is_row && first_line_children.iter().any(is_baseline_aligned) {
                first_line_children
                    .into_iter()
                    .filter(is_baseline_aligned)
                    .collect()
            } else {
                first_line_children
            };

        for child in candidates {
            if let Some(b) = check_child(child) {
                return Some(b);
            }
        }
    } else {
        for child in box_.children.iter().rev() {
            if let Some(b) = check_child(child) {
                return Some(b);
            }
        }
    }
    None
}
