//! Vertical writing mode shrink-to-fit width calculation.

use crate::box_tree::LayoutBox;

use super::intrinsic::calculate_shrink_to_fit_width;

/// Calculates the shrink-to-fit width of a block formatting context element in vertical writing modes.
pub fn calculate_vertical_shrink_to_fit_width(box_node: &LayoutBox, max_width: f32) -> f32 {
    let (family, weight) = if let Some(s) = box_node.style.as_ref() {
        (
            crate::inline_flow::shaping::font_family_for(s),
            crate::inline_flow::shaping::font_weight_for(s),
        )
    } else {
        (
            mango_render::FontFamily::Serif,
            mango_render::FontWeight::Regular,
        )
    };
    let base_col_w = box_node.style.as_ref().map_or(16.0, |s| {
        crate::inline_flow::shaping::used_line_height(s, family, weight)
    });

    let max_col_h = if box_node.dimensions.content.height() > 0.0 {
        box_node.dimensions.content.height()
    } else if let Some(s) = box_node.style.as_ref() {
        let fs = s.font_size;
        let rfs = s.root_font_size;
        match s.height {
            mango_css::values::Length::Px(px) if px > 0.0 => px,
            mango_css::values::Length::Em(em) if em > 0.0 => em * fs,
            mango_css::values::Length::Rem(rem) if rem > 0.0 => rem * rfs,
            _ => 400.0,
        }
    } else {
        400.0
    };

    let mut col_widths: Vec<f32> = Vec::new();
    let mut cur_col_w = 0.0f32;
    let mut cur_col_has_text = false;
    let mut cur_y = 0.0f32;

    fn traverse_vertical_inlines(
        node: &LayoutBox,
        max_col_h: f32,
        base_col_w: f32,
        col_widths: &mut Vec<f32>,
        cur_col_w: &mut f32,
        cur_col_has_text: &mut bool,
        cur_y: &mut f32,
        max_width: f32,
    ) {
        let pos = node
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(mango_css::values::Position::Static);
        if matches!(
            pos,
            mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
        ) {
            return;
        }
        if node.tag_name.as_deref() == Some("br") {
            col_widths.push(if *cur_col_has_text || *cur_col_w <= 0.0 {
                cur_col_w.max(base_col_w)
            } else {
                *cur_col_w
            });
            *cur_col_w = 0.0;
            *cur_col_has_text = false;
            *cur_y = 0.0;
            return;
        }
        match &node.box_type {
            crate::box_model::BoxType::TextNode(raw_text) => {
                let style = node.style.as_ref();
                let (font_size, is_bold, family_name, letter_spacing) = if let Some(s) = style {
                    let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                    let ls = s.letter_spacing.to_px(s.font_size, s.root_font_size, 400.0);
                    (s.font_size, bold, s.font_family.clone(), ls)
                } else {
                    (16.0, false, "sans-serif".to_string(), 0.0)
                };
                let weight = if is_bold {
                    mango_render::FontWeight::Bold
                } else {
                    mango_render::FontWeight::Regular
                };
                let family = mango_render::FontFamily::from_css_name(&family_name);

                for ch in raw_text.chars() {
                    let ch_str = ch.to_string();
                    let glyph_adv = crate::inline_flow::measure_text_width_with_style_and_spacing(
                        &ch_str,
                        font_size,
                        weight,
                        family,
                        letter_spacing,
                    )
                    .max(1.0);

                    if *cur_y + glyph_adv > max_col_h && *cur_y > 0.0 {
                        col_widths.push(if *cur_col_has_text || *cur_col_w <= 0.0 {
                            cur_col_w.max(base_col_w)
                        } else {
                            *cur_col_w
                        });
                        *cur_col_w = 0.0;
                        *cur_col_has_text = true;
                        *cur_y = glyph_adv;
                    } else {
                        *cur_col_has_text = true;
                        *cur_y += glyph_adv;
                    }
                }
            }
            crate::box_model::BoxType::InlineBlock => {
                let adv = node.dimensions.margin_box().height().max(16.0);
                let item_w = if node.dimensions.margin_box().width() > 0.0 {
                    node.dimensions.margin_box().width()
                } else {
                    calculate_shrink_to_fit_width(node, max_width)
                };
                if *cur_y + adv > max_col_h && *cur_y > 0.0 {
                    col_widths.push(if *cur_col_has_text || *cur_col_w <= 0.0 {
                        cur_col_w.max(base_col_w)
                    } else {
                        *cur_col_w
                    });
                    *cur_y = adv;
                    *cur_col_w = item_w;
                    *cur_col_has_text = false;
                } else {
                    *cur_y += adv;
                    *cur_col_w = cur_col_w.max(item_w);
                }
            }
            _ => {
                for child in &node.children {
                    traverse_vertical_inlines(
                        child,
                        max_col_h,
                        base_col_w,
                        col_widths,
                        cur_col_w,
                        cur_col_has_text,
                        cur_y,
                        max_width,
                    );
                }
            }
        }
    }

    for child in &box_node.children {
        traverse_vertical_inlines(
            child,
            max_col_h,
            base_col_w,
            &mut col_widths,
            &mut cur_col_w,
            &mut cur_col_has_text,
            &mut cur_y,
            max_width,
        );
    }

    if cur_y > 0.0 || col_widths.is_empty() {
        col_widths.push(if cur_col_has_text || cur_col_w <= 0.0 {
            cur_col_w.max(base_col_w)
        } else {
            cur_col_w
        });
    }

    let total_col_w: f32 = col_widths.iter().sum();
    total_col_w.min(max_width)
}
