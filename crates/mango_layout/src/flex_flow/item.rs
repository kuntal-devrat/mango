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
        crate::block_flow::layout_block_contents(child, item_cb, float_ctx);
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

/// Computes the first baseline ascent of a flex item according to CSS Flexbox §8.5.
pub fn find_first_baseline_ascent(box_: &LayoutBox, default_fs: f32) -> f32 {
    if matches!(box_.box_type, BoxType::TextNode(_)) {
        let fs = box_
            .style
            .as_ref()
            .map(|s| s.font_size)
            .unwrap_or(default_fs);
        return fs * 0.8;
    }

    let is_fieldset = box_.tag_name.as_deref() == Some("fieldset");
    let has_non_legend = is_fieldset
        && box_
            .children
            .iter()
            .any(|c| c.tag_name.as_deref() != Some("legend"));

    for child in &box_.children {
        if is_fieldset && has_non_legend && child.tag_name.as_deref() == Some("legend") {
            continue;
        }
        if child
            .style
            .as_ref()
            .map(|s| s.display != Display::None)
            .unwrap_or(true)
        {
            let child_ascent = find_first_baseline_ascent(child, default_fs);
            if child_ascent > 0.0 {
                return child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
                    + child_ascent;
            }
        }
    }
    let fs = box_
        .style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or(default_fs);
    fs * 0.8
}
