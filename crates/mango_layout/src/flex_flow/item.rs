use mango_css::values::Display;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

/// Helper function to lay out a flex item according to its formatting context.
pub fn layout_flex_item(
    child: &mut LayoutBox,
    item_cb: &Dimensions,
    float_ctx: &mut FloatContext,
    c_fs: f32,
) {
    if child.children.is_empty() && !matches!(child.box_type, BoxType::TextNode(_)) {
        return;
    }
    let c_disp = child.style.as_ref().map(|s| s.display);
    if matches!(c_disp, Some(Display::Flex) | Some(Display::InlineFlex)) {
        crate::flex_flow::layout_flex(child, item_cb, float_ctx);
    } else if matches!(c_disp, Some(Display::Grid) | Some(Display::InlineGrid)) {
        crate::grid_flow::layout_grid(child, item_cb, float_ctx);
    } else if matches!(c_disp, Some(Display::Table)) || child.tag_name.as_deref() == Some("table") {
        child.is_independent_formatting_context = true;
        crate::table_flow::layout_table_contents(child, item_cb, float_ctx);
    } else if let BoxType::TextNode(ref t) = child.box_type {
        let (c_weight, c_family, c_spacing) = if let Some(s) = &child.style {
            let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
            let weight = if bold {
                mango_render::FontWeight::Bold
            } else {
                mango_render::FontWeight::Regular
            };
            let family = mango_render::FontFamily::from_css_name(&s.font_family);
            let spacing = s.letter_spacing.to_px(s.font_size, s.root_font_size, 0.0);
            (weight, family, spacing)
        } else {
            (
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
                0.0,
            )
        };
        let text_w = crate::inline_flow::measure_text_width_with_style_and_spacing(
            t, c_fs, c_weight, c_family, c_spacing,
        );
        child.dimensions.content.size.width = text_w;
        child.dimensions.content.size.height = c_fs * 1.3;
    } else if child.box_type.is_replaced() {
        // Replaced element dimensions already set by flex_flow
    } else if child.box_type.is_inline_block() || child.is_block() {
        child.is_independent_formatting_context = true;
        if item_cb.content.height() <= 0.0 {
            child.dimensions.content.size.height = 0.0;
        } else if child
            .style
            .as_ref()
            .map(|s| s.height == mango_css::values::Length::Auto)
            .unwrap_or(true)
        {
            child.dimensions.content.size.height = item_cb.content.height();
        }
        let child_disp = child.style.as_ref().map(|s| s.display);
        if matches!(child_disp, Some(Display::Flex) | Some(Display::InlineFlex)) {
            crate::flex_flow::layout_flex(child, item_cb, float_ctx);
        } else if matches!(child_disp, Some(Display::Grid) | Some(Display::InlineGrid)) {
            crate::grid_flow::layout_grid(child, item_cb, float_ctx);
        } else if child_disp == Some(Display::Table) || child.tag_name.as_deref() == Some("table") {
            crate::table_flow::layout_table(child, item_cb, float_ctx);
        } else {
            crate::block_flow::layout_block_contents(child, item_cb, float_ctx);
        }
    } else if child.is_inline() {
        if child.dimensions.content.size.width <= 0.0 {
            child.dimensions.content.size.width = item_cb.content.width();
        }
        let h = crate::inline_flow::layout_inline_children(child, float_ctx);
        if child.dimensions.content.size.height <= 0.0 || child.dimensions.content.size.height < h {
            child.dimensions.content.size.height = h.max(c_fs * 1.3);
        }
    }
}

fn text_node_baseline(box_: &LayoutBox, default_fs: f32) -> f32 {
    let fs = box_
        .style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or(default_fs);
    let default_style = mango_css::computed::ComputedStyle::default();
    let s = box_.style.as_deref().unwrap_or(&default_style);
    let family = crate::inline_flow::shaping::font_family_for(s);
    let weight = crate::inline_flow::shaping::font_weight_for(s);
    let (ascent, descent, _) = mango_render::font::font_manager().font_metrics(family, weight, fs);
    let natural_h = ascent + descent;
    let lh = crate::inline_flow::shaping::used_line_height(s, family, weight);
    let half_leading = (lh - natural_h) / 2.0;
    ascent + half_leading
}

/// Computes the first baseline ascent of a flex item according to CSS Flexbox §8.5.
pub fn find_first_baseline_ascent(box_: &LayoutBox, default_fs: f32) -> Option<f32> {
    if let Some(s) = &box_.style
        && (s.overflow_x != mango_css::values::Overflow::Visible
            || s.overflow_y != mango_css::values::Overflow::Visible)
    {
        return None;
    }
    find_first_baseline_ascent_internal(box_, default_fs)
}

fn find_first_baseline_ascent_internal(box_: &LayoutBox, default_fs: f32) -> Option<f32> {
    if let Some(s) = &box_.style
        && s.display == Display::InlineBlock
        && (s.overflow_x != mango_css::values::Overflow::Visible
            || s.overflow_y != mango_css::values::Overflow::Visible)
    {
        return None;
    }

    if matches!(box_.box_type, BoxType::TextNode(_)) {
        return Some(text_node_baseline(box_, default_fs));
    }

    if matches!(
        box_.tag_name.as_deref(),
        Some("input" | "button" | "select" | "textarea")
    ) {
        let fs = box_
            .style
            .as_ref()
            .map(|s| s.font_size)
            .unwrap_or(default_fs);
        let natural_h = fs * 1.2;
        let content_h = box_.dimensions.content.height();
        let half_leading = if content_h > natural_h {
            (content_h - natural_h) / 2.0
        } else {
            0.0
        };
        return Some(half_leading + fs * 0.8);
    }

    let is_fieldset = box_.tag_name.as_deref() == Some("fieldset");
    let has_non_legend = is_fieldset
        && box_
            .children
            .iter()
            .any(|c| c.tag_name.as_deref() != Some("legend"));

    let is_flex = matches!(
        box_.style.as_ref().map(|s| s.display),
        Some(Display::Flex) | Some(Display::InlineFlex)
    );

    let mut indices: Vec<usize> = (0..box_.children.len()).collect();
    if is_flex {
        indices.sort_by_key(|&i| {
            let c = &box_.children[i];
            let is_oof = c
                .style
                .as_ref()
                .map(|s| {
                    matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
                })
                .unwrap_or(false);
            if is_oof {
                0
            } else {
                c.style.as_ref().map(|s| s.order).unwrap_or(0)
            }
        });
    }

    for &idx in &indices {
        let child = &box_.children[idx];
        if is_fieldset && has_non_legend && child.tag_name.as_deref() == Some("legend") {
            continue;
        }
        if child
            .style
            .as_ref()
            .map(|s| {
                s.display != Display::None
                    && !matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
            })
            .unwrap_or(true)
        {
            if let Some(child_ascent) = find_first_baseline_ascent_internal(child, default_fs) {
                let top_offset = child.dimensions.content.y() - box_.dimensions.content.y();
                let offset = if top_offset > 0.0 {
                    top_offset
                } else {
                    child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top
                };
                return Some(offset + child_ascent);
            }
        }
    }
    None
}

/// Computes the last baseline ascent of a flex item according to CSS Flexbox §8.5 and Box Alignment 3.
pub fn find_last_baseline_ascent(box_: &LayoutBox, default_fs: f32) -> Option<f32> {
    if let Some(s) = &box_.style
        && (s.overflow_x != mango_css::values::Overflow::Visible
            || s.overflow_y != mango_css::values::Overflow::Visible)
    {
        return None;
    }
    find_last_baseline_ascent_internal(box_, default_fs)
}

fn find_last_baseline_ascent_internal(box_: &LayoutBox, default_fs: f32) -> Option<f32> {
    if let Some(s) = &box_.style
        && s.display == Display::InlineBlock
        && (s.overflow_x != mango_css::values::Overflow::Visible
            || s.overflow_y != mango_css::values::Overflow::Visible)
    {
        return None;
    }

    if matches!(box_.box_type, BoxType::TextNode(_)) {
        return Some(text_node_baseline(box_, default_fs));
    }

    if matches!(
        box_.tag_name.as_deref(),
        Some("input" | "button" | "select" | "textarea")
    ) {
        let fs = box_
            .style
            .as_ref()
            .map(|s| s.font_size)
            .unwrap_or(default_fs);
        let natural_h = fs * 1.2;
        let content_h = box_.dimensions.content.height();
        let half_leading = if content_h > natural_h {
            (content_h - natural_h) / 2.0
        } else {
            0.0
        };
        return Some(half_leading + fs * 0.8);
    }

    let is_fieldset = box_.tag_name.as_deref() == Some("fieldset");
    let has_non_legend = is_fieldset
        && box_
            .children
            .iter()
            .any(|c| c.tag_name.as_deref() != Some("legend"));

    let is_flex = matches!(
        box_.style.as_ref().map(|s| s.display),
        Some(Display::Flex) | Some(Display::InlineFlex)
    );

    let mut indices: Vec<usize> = (0..box_.children.len()).collect();
    if is_flex {
        indices.sort_by_key(|&i| {
            let c = &box_.children[i];
            let is_oof = c
                .style
                .as_ref()
                .map(|s| {
                    matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
                })
                .unwrap_or(false);
            if is_oof {
                0
            } else {
                c.style.as_ref().map(|s| s.order).unwrap_or(0)
            }
        });
    }

    for &idx in indices.iter().rev() {
        let child = &box_.children[idx];
        if is_fieldset && has_non_legend && child.tag_name.as_deref() == Some("legend") {
            continue;
        }
        if child
            .style
            .as_ref()
            .map(|s| {
                s.display != Display::None
                    && !matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
            })
            .unwrap_or(true)
        {
            if let Some(child_ascent) = find_last_baseline_ascent_internal(child, default_fs) {
                let top_offset = child.dimensions.content.y() - box_.dimensions.content.y();
                let offset = if top_offset > 0.0 {
                    top_offset
                } else {
                    child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top
                };
                return Some(offset + child_ascent);
            }
        }
    }
    None
}
