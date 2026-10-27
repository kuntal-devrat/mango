use mango_css::values::BoxSizing;

use crate::box_tree::LayoutBox;

pub fn calculate_shrink_to_fit_width(box_node: &LayoutBox, max_width: f32) -> f32 {
    let (max_w_bound, min_w_bound) = if let Some(s) = box_node.style.as_ref() {
        let fs = s.font_size;
        let rfs = s.root_font_size;
        let max_w = if !matches!(
            s.max_width,
            mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
        ) {
            let px = s
                .max_width
                .to_px_with_viewport(fs, rfs, max_width, max_width);
            let non_content = if s.box_sizing == BoxSizing::BorderBox {
                let pl = s
                    .padding_left
                    .to_px_with_viewport(fs, rfs, max_width, max_width);
                let pr = s
                    .padding_right
                    .to_px_with_viewport(fs, rfs, max_width, max_width);
                pl + pr + s.border_left_width + s.border_right_width
            } else {
                0.0
            };
            Some((px - non_content).max(0.0))
        } else {
            None
        };
        let min_w = if !matches!(
            s.min_width,
            mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
        ) {
            let px = s
                .min_width
                .to_px_with_viewport(fs, rfs, max_width, max_width);
            let non_content = if s.box_sizing == BoxSizing::BorderBox {
                let pl = s
                    .padding_left
                    .to_px_with_viewport(fs, rfs, max_width, max_width);
                let pr = s
                    .padding_right
                    .to_px_with_viewport(fs, rfs, max_width, max_width);
                pl + pr + s.border_left_width + s.border_right_width
            } else {
                0.0
            };
            Some((px - non_content).max(0.0))
        } else {
            None
        };
        (max_w, min_w)
    } else {
        (None, None)
    };

    let effective_max_width = max_w_bound.map_or(max_width, |mw| max_width.min(mw));
    let measured = calculate_raw_shrink_to_fit_width(box_node, effective_max_width);
    let mut final_w = measured;
    if let Some(mw) = max_w_bound {
        final_w = final_w.min(mw);
    }
    if let Some(mw) = min_w_bound {
        final_w = final_w.max(mw);
    }
    final_w
}

fn calculate_raw_shrink_to_fit_width(box_node: &LayoutBox, max_width: f32) -> f32 {
    if let crate::box_model::BoxType::TextNode(text) = &box_node.box_type {
        let style = box_node.style.as_ref();
        let (font_size, is_bold, family_name) = if let Some(s) = style {
            let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
            (s.font_size, bold, s.font_family.clone())
        } else {
            (14.0, false, "sans-serif".to_string())
        };
        let weight = if is_bold {
            mango_render::FontWeight::Bold
        } else {
            mango_render::FontWeight::Regular
        };
        let family = mango_render::FontFamily::from_css_name(&family_name);
        let preserves_spaces = style
            .map(|s| {
                matches!(
                    s.white_space,
                    mango_css::values::WhiteSpace::Pre | mango_css::values::WhiteSpace::PreWrap
                )
            })
            .unwrap_or(false);
        let text_to_measure = if preserves_spaces {
            text.as_str()
        } else {
            text.trim()
        };
        let w = crate::inline_flow::measure_text_width_with_style(
            text_to_measure,
            font_size,
            weight,
            family,
        );
        let mut min_w = 0.0f32;
        for word in text_to_measure.split_whitespace() {
            let ww =
                crate::inline_flow::measure_text_width_with_style(word, font_size, weight, family);
            min_w = min_w.max(ww);
        }
        return w.min(max_width).max(min_w);
    }
    if let crate::box_model::BoxType::ReplacedElement {
        intrinsic_width, ..
    }
    | crate::box_model::BoxType::IFrame {
        intrinsic_width, ..
    }
    | crate::box_model::BoxType::Video {
        intrinsic_width, ..
    }
    | crate::box_model::BoxType::Audio {
        intrinsic_width, ..
    }
    | crate::box_model::BoxType::Canvas {
        intrinsic_width, ..
    } = &box_node.box_type
    {
        let w = if let Some(s) = box_node.style.as_ref() {
            if !matches!(
                s.width,
                mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
            ) {
                s.width
                    .to_px_with_viewport(s.font_size, s.root_font_size, max_width, max_width)
            } else {
                *intrinsic_width
            }
        } else {
            *intrinsic_width
        };
        return w.min(max_width);
    }

    if let Some(s) = box_node.style.as_ref() {
        if !matches!(
            s.width,
            mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
        ) {
            let font_size = s.font_size;
            let root_font_size = s.root_font_size;
            let raw_w =
                s.width
                    .to_px_with_viewport(font_size, root_font_size, max_width, max_width);
            let mut w = raw_w;
            if !matches!(
                s.max_width,
                mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
            ) {
                let max_w = s.max_width.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    max_width,
                    max_width,
                );
                w = w.min(max_w);
            }
            if !matches!(
                s.min_width,
                mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
            ) {
                let min_w = s.min_width.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    max_width,
                    max_width,
                );
                w = w.max(min_w);
            }
            return w.min(max_width);
        }

        if matches!(
            s.display,
            mango_css::values::Display::Flex | mango_css::values::Display::InlineFlex
        ) {
            let font_size = s.font_size;
            let root_font_size = s.root_font_size;
            let is_row = matches!(
                s.flex_direction,
                mango_css::values::FlexDirection::Row
                    | mango_css::values::FlexDirection::RowReverse
            );
            if is_row {
                let col_gap = s.column_gap.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    max_width,
                    max_width,
                );
                let mut sum_w = 0.0f32;
                let mut count = 0;
                for child in &box_node.children {
                    let pos = child
                        .style
                        .as_ref()
                        .map(|s| s.position)
                        .unwrap_or(mango_css::values::Position::Static);
                    if matches!(
                        pos,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    ) {
                        continue;
                    }
                    let child_w = calculate_shrink_to_fit_width(child, max_width);
                    let c_style = child.style.as_ref();
                    let (p_left, p_right, b_left, b_right, m_left, m_right) =
                        if let Some(cs) = c_style {
                            let c_fs = cs.font_size;
                            let c_root_fs = cs.root_font_size;
                            (
                                cs.padding_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.padding_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.border_left_width,
                                cs.border_right_width,
                                cs.margin_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.margin_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            )
                        } else {
                            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                        };
                    sum_w += child_w + p_left + p_right + b_left + b_right + m_left + m_right;
                    count += 1;
                }
                if count > 1 {
                    sum_w += (count - 1) as f32 * col_gap;
                }
                return sum_w.min(max_width);
            } else {
                let is_multiline = s.flex_wrap != mango_css::values::FlexWrap::NoWrap;
                let target_h = if is_multiline
                    && !matches!(
                        s.height,
                        mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
                    ) {
                    s.height.to_px_with_viewport(font_size, root_font_size, 0.0, 0.0)
                } else {
                    0.0
                };
                let target_content_h = if target_h > 0.0 {
                    let non_content_v = if s.box_sizing == BoxSizing::BorderBox {
                        let pt = s.padding_top.to_px_with_viewport(
                            font_size,
                            root_font_size,
                            max_width,
                            max_width,
                        );
                        let pb = s.padding_bottom.to_px_with_viewport(
                            font_size,
                            root_font_size,
                            max_width,
                            max_width,
                        );
                        pt + pb + s.border_top_width + s.border_bottom_width
                    } else {
                        0.0
                    };
                    (target_h - non_content_v).max(0.0)
                } else {
                    0.0
                };

                if is_multiline && target_content_h > 0.0 {
                    let row_gap = s.row_gap.to_px_with_viewport(
                        font_size,
                        root_font_size,
                        target_content_h,
                        target_content_h,
                    );
                    let col_gap = s.column_gap.to_px_with_viewport(
                        font_size,
                        root_font_size,
                        max_width,
                        max_width,
                    );
                    let mut num_cols = 0;
                    let mut sum_w = 0.0f32;
                    let mut curr_col_h = 0.0f32;
                    let mut curr_col_w = 0.0f32;

                    for child in &box_node.children {
                        let pos = child
                            .style
                            .as_ref()
                            .map(|s| s.position)
                            .unwrap_or(mango_css::values::Position::Static);
                        if matches!(
                            pos,
                            mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                        ) {
                            continue;
                        }
                        let child_w = calculate_shrink_to_fit_width(child, max_width);
                        let c_style = child.style.as_ref();
                        let (child_outer_w, child_outer_h) = if let Some(cs) = c_style {
                            let c_fs = cs.font_size;
                            let c_root_fs = cs.root_font_size;
                            let p_left = cs.padding_left.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let p_right = cs.padding_right.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let m_left = cs.margin_left.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let m_right = cs.margin_right.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let ow = child_w
                                + p_left
                                + p_right
                                + cs.border_left_width
                                + cs.border_right_width
                                + m_left
                                + m_right;

                            let raw_h = cs.height.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                target_content_h,
                                target_content_h,
                            );
                            let p_top = cs.padding_top.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let p_bottom = cs.padding_bottom.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let m_top = cs.margin_top.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let m_bottom = cs.margin_bottom.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                max_width,
                                max_width,
                            );
                            let oh = raw_h
                                + p_top
                                + p_bottom
                                + cs.border_top_width
                                + cs.border_bottom_width
                                + m_top
                                + m_bottom;
                            (ow, oh)
                        } else {
                            (child_w, 0.0)
                        };

                        if curr_col_h > 0.0
                            && curr_col_h + row_gap + child_outer_h > target_content_h
                        {
                            sum_w += curr_col_w;
                            num_cols += 1;
                            curr_col_h = child_outer_h;
                            curr_col_w = child_outer_w;
                        } else {
                            if curr_col_h > 0.0 {
                                curr_col_h += row_gap;
                            }
                            curr_col_h += child_outer_h;
                            curr_col_w = curr_col_w.max(child_outer_w);
                        }
                    }
                    if curr_col_h > 0.0 || curr_col_w > 0.0 {
                        sum_w += curr_col_w;
                        num_cols += 1;
                    }
                    if num_cols > 1 {
                        sum_w += (num_cols - 1) as f32 * col_gap;
                    }
                    return sum_w.min(max_width);
                }

                let mut max_w = 0.0f32;
                let mut current_line_w = 0.0f32;
                for child in &box_node.children {
                    let pos = child
                        .style
                        .as_ref()
                        .map(|s| s.position)
                        .unwrap_or(mango_css::values::Position::Static);
                    if matches!(
                        pos,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    ) {
                        continue;
                    }
                    let child_w = calculate_shrink_to_fit_width(child, max_width);
                    let c_style = child.style.as_ref();
                    let (p_left, p_right, b_left, b_right, m_left, m_right) =
                        if let Some(cs) = c_style {
                            let c_fs = cs.font_size;
                            let c_root_fs = cs.root_font_size;
                            (
                                cs.padding_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.padding_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.border_left_width,
                                cs.border_right_width,
                                cs.margin_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.margin_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            )
                        } else {
                            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                        };
                    let child_outer_w =
                        child_w + p_left + p_right + b_left + b_right + m_left + m_right;

                    let is_inline_or_float = child.is_inline()
                        || child
                            .style
                            .as_ref()
                            .map(|s| {
                                s.float != mango_css::values::Float::None
                                    || matches!(
                                        s.display,
                                        mango_css::values::Display::Inline
                                            | mango_css::values::Display::InlineBlock
                                            | mango_css::values::Display::InlineFlex
                                            | mango_css::values::Display::InlineGrid
                                    )
                            })
                            .unwrap_or(false);

                    if is_inline_or_float {
                        current_line_w += child_outer_w;
                    } else {
                        max_w = max_w.max(current_line_w);
                        current_line_w = 0.0;
                        max_w = max_w.max(child_outer_w);
                    }
                }
                max_w = max_w.max(current_line_w);
                return max_w.min(max_width);
            }
        }

        if matches!(
            s.display,
            mango_css::values::Display::Grid | mango_css::values::Display::InlineGrid
        ) {
            let font_size = s.font_size;
            let (_min_w, max_w) =
                crate::grid_flow::measure_grid_intrinsic_widths(box_node, font_size);
            let non_content = if s.box_sizing == BoxSizing::BorderBox {
                let pl = s.padding_left.to_px_with_viewport(
                    font_size,
                    s.root_font_size,
                    max_width,
                    max_width,
                );
                let pr = s.padding_right.to_px_with_viewport(
                    font_size,
                    s.root_font_size,
                    max_width,
                    max_width,
                );
                pl + pr + s.border_left_width + s.border_right_width
            } else {
                0.0
            };
            return (max_w + non_content).ceil().min(max_width);
        }
    }

    if let Some(tag) = box_node.tag_name.as_deref() {
        match tag {
            "input" => {
                let input_type = box_node.get_attribute("type").unwrap_or("text");
                match input_type {
                    "checkbox" | "radio" => return 16.0f32.min(max_width),
                    "submit" | "button" => {
                        let val = box_node.get_attribute("value").unwrap_or("Submit");
                        let style = box_node.style.as_ref();
                        let (font_size, is_bold, family_name) = if let Some(s) = style {
                            let bold =
                                crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                            (s.font_size, bold, s.font_family.clone())
                        } else {
                            (14.0, false, "sans-serif".to_string())
                        };
                        let weight = if is_bold {
                            mango_render::FontWeight::Bold
                        } else {
                            mango_render::FontWeight::Regular
                        };
                        let family = mango_render::FontFamily::from_css_name(&family_name);
                        let w = crate::inline_flow::measure_text_width_with_style(
                            val, font_size, weight, family,
                        );
                        return w.min(max_width);
                    }
                    _ => {
                        let size_attr = box_node
                            .get_attribute("size")
                            .and_then(|s| s.parse::<f32>().ok())
                            .unwrap_or(20.0);
                        return (size_attr * 8.0 + 16.0).min(max_width);
                    }
                }
            }
            "select" => {
                let text = box_node
                    .get_attribute("_mango_selected_text")
                    .or_else(|| box_node.get_attribute("value"))
                    .unwrap_or("Select...");
                let w = crate::inline_flow::measure_text_width(text, 13.0) + 32.0;
                return w.max(80.0).min(max_width);
            }
            "textarea" => return 220.0f32.min(max_width),
            "button" => {
                let mut content_w = 0.0f32;
                let style = box_node.style.as_ref();
                let (font_size, is_bold, family_name) = if let Some(s) = style {
                    let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                    (s.font_size, bold, s.font_family.clone())
                } else {
                    (14.0, false, "sans-serif".to_string())
                };
                let weight = if is_bold {
                    mango_render::FontWeight::Bold
                } else {
                    mango_render::FontWeight::Regular
                };
                let family = mango_render::FontFamily::from_css_name(&family_name);

                for c in &box_node.children {
                    match &c.box_type {
                        crate::box_model::BoxType::TextNode(t) => {
                            content_w += crate::inline_flow::measure_text_width_with_style(
                                t, font_size, weight, family,
                            );
                        }
                        crate::box_model::BoxType::ReplacedElement {
                            intrinsic_width, ..
                        }
                        | crate::box_model::BoxType::IFrame {
                            intrinsic_width, ..
                        }
                        | crate::box_model::BoxType::Video {
                            intrinsic_width, ..
                        }
                        | crate::box_model::BoxType::Audio {
                            intrinsic_width, ..
                        }
                        | crate::box_model::BoxType::Canvas {
                            intrinsic_width, ..
                        } => {
                            content_w += *intrinsic_width + 6.0;
                        }
                        _ => {
                            let w = calculate_shrink_to_fit_width(c, max_width);
                            content_w += w;
                        }
                    }
                }
                if content_w == 0.0 {
                    let val = box_node.get_attribute("value").unwrap_or("Button");
                    content_w = crate::inline_flow::measure_text_width_with_style(
                        val, font_size, weight, family,
                    );
                }
                return content_w.min(max_width);
            }
            _ => {}
        }
    }

    fn calculate_vertical_shrink_to_fit_width(box_node: &LayoutBox, max_width: f32) -> f32 {
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
                        let glyph_adv =
                            crate::inline_flow::measure_text_width_with_style_and_spacing(
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

    let has_inlines = box_node.children.iter().any(|c| {
        let pos = c
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(mango_css::values::Position::Static);
        !matches!(
            pos,
            mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
        ) && c.is_inline()
    });
    if has_inlines {
        let is_vertical = box_node
            .style
            .as_ref()
            .map(|s| s.writing_mode != mango_css::values::WritingMode::HorizontalTb)
            .unwrap_or(false);
        if is_vertical {
            return calculate_vertical_shrink_to_fit_width(box_node, max_width);
        }

        let mut max_line_w = 0.0f32;
        let mut curr_line_w = 0.0f32;
        let mut min_content_w = 0.0f32;

        fn accumulate_inline_width(
            node: &LayoutBox,
            parent_style: Option<&mango_css::computed::ComputedStyle>,
            curr_line: &mut f32,
            max_line: &mut f32,
            min_content: &mut f32,
            pending_space: &mut f32,
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
                *max_line = max_line.max(*curr_line);
                *curr_line = 0.0;
                *pending_space = 0.0;
                return;
            }
            match &node.box_type {
                crate::box_model::BoxType::InlineBlock => {
                    let w = calculate_shrink_to_fit_width(node, max_width);
                    *min_content = min_content.max(w);
                    let c_style = node.style.as_ref();
                    let (p_left, p_right, b_left, b_right, m_left, m_right) =
                        if let Some(cs) = c_style {
                            let c_fs = cs.font_size;
                            let c_root_fs = cs.root_font_size;
                            (
                                cs.padding_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.padding_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.border_left_width,
                                cs.border_right_width,
                                cs.margin_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.margin_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            )
                        } else {
                            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                        };
                    *curr_line += *pending_space + w + p_left + p_right + b_left + b_right + m_left + m_right;
                    *pending_space = 0.0;
                }
                crate::box_model::BoxType::TextNode(raw_text) => {
                    let style = node.style.as_deref().or(parent_style);
                    let preserves_spaces = style
                        .map(|s| {
                            matches!(
                                s.white_space,
                                mango_css::values::WhiteSpace::Pre
                                    | mango_css::values::WhiteSpace::PreWrap
                            )
                        })
                        .unwrap_or(false);
                    let (
                        font_size,
                        is_bold,
                        family_name,
                        transformed_text,
                        letter_spacing,
                        word_spacing,
                    ) = if let Some(s) = style {
                        let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                        let ls = s
                            .letter_spacing
                            .to_px(s.font_size, s.root_font_size, max_width);
                        let ws = s
                            .word_spacing
                            .to_px(s.font_size, s.root_font_size, max_width);
                        (
                            s.font_size,
                            bold,
                            s.font_family.clone(),
                            s.text_transform.apply(raw_text),
                            ls,
                            ws,
                        )
                    } else {
                        (
                            16.0,
                            false,
                            "sans-serif".to_string(),
                            raw_text.clone(),
                            0.0,
                            0.0,
                        )
                    };
                    let weight = if is_bold {
                        mango_render::FontWeight::Bold
                    } else {
                        mango_render::FontWeight::Regular
                    };
                    let family = mango_render::FontFamily::from_css_name(&family_name);

                    let space_w = (crate::inline_flow::measure_text_width_with_style_and_spacing(
                        " ",
                        font_size,
                        weight,
                        family,
                        letter_spacing,
                    ) + word_spacing)
                        .max(0.0);

                    // Tokenize into words and spaces matching collect_inline_atoms and CSS Text Level 3 § 4.1.1
                    let mut current_word = String::new();
                    for ch in transformed_text.chars() {
                        if ch.is_ascii_whitespace() {
                            if !current_word.is_empty() {
                                let w =
                                    crate::inline_flow::measure_text_width_with_style_and_spacing(
                                        &current_word,
                                        font_size,
                                        weight,
                                        family,
                                        letter_spacing,
                                    );
                                *curr_line += *pending_space + w;
                                *pending_space = 0.0;
                                *min_content = min_content.max(w);
                                current_word.clear();
                            }
                            if preserves_spaces {
                                let ch_w = if ch == '\t' { space_w * 4.0 } else { space_w };
                                *curr_line += ch_w;
                            } else if *curr_line > 0.0 {
                                *pending_space = space_w;
                            }
                        } else {
                            current_word.push(ch);
                        }
                    }
                    if !current_word.is_empty() {
                        let w = crate::inline_flow::measure_text_width_with_style_and_spacing(
                            &current_word,
                            font_size,
                            weight,
                            family,
                            letter_spacing,
                        );
                        *curr_line += *pending_space + w;
                        *pending_space = 0.0;
                        *min_content = min_content.max(w);
                    }
                }
                crate::box_model::BoxType::ReplacedElement {
                    intrinsic_width, ..
                }
                | crate::box_model::BoxType::IFrame {
                    intrinsic_width, ..
                }
                | crate::box_model::BoxType::Video {
                    intrinsic_width, ..
                }
                | crate::box_model::BoxType::Audio {
                    intrinsic_width, ..
                }
                | crate::box_model::BoxType::Canvas {
                    intrinsic_width, ..
                } => {
                    let w = if let Some(s) = node.style.as_ref() {
                        if !matches!(
                            s.width,
                            mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
                        ) {
                            s.width.to_px_with_viewport(
                                s.font_size,
                                s.root_font_size,
                                max_width,
                                max_width,
                            )
                        } else {
                            *intrinsic_width
                        }
                    } else {
                        *intrinsic_width
                    };
                    let (p_left, p_right, b_left, b_right, m_left, m_right) =
                        if let Some(cs) = node.style.as_ref() {
                            let c_fs = cs.font_size;
                            let c_root_fs = cs.root_font_size;
                            (
                                cs.padding_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.padding_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.border_left_width,
                                cs.border_right_width,
                                cs.margin_left
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                                cs.margin_right
                                    .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            )
                        } else {
                            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                        };
                    *curr_line += *pending_space + w + p_left + p_right + b_left + b_right + m_left + m_right;
                    *pending_space = 0.0;
                    *min_content =
                        min_content.max(w + p_left + p_right + b_left + b_right + m_left + m_right);
                }
                _ => {
                    let next_parent_style = node.style.as_deref().or(parent_style);
                    for c in &node.children {
                        accumulate_inline_width(
                            c,
                            next_parent_style,
                            curr_line,
                            max_line,
                            min_content,
                            pending_space,
                            max_width,
                        );
                    }
                }
            }
        }

        let p_style = box_node.style.as_deref();
        let mut pending_space = 0.0f32;
        for child in &box_node.children {
            accumulate_inline_width(
                child,
                p_style,
                &mut curr_line_w,
                &mut max_line_w,
                &mut min_content_w,
                &mut pending_space,
                max_width,
            );
        }
        max_line_w = max_line_w.max(curr_line_w);
        return max_line_w.min(max_width).max(min_content_w);
    }

    let mut measured = 0.0f32;
    for child in &box_node.children {
        let pos = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(mango_css::values::Position::Static);
        if matches!(
            pos,
            mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
        ) {
            continue;
        }
        match &child.box_type {
            crate::box_model::BoxType::TextNode(text) => {
                let style = child.style.as_ref().or(box_node.style.as_ref());
                let (font_size, is_bold, family_name, letter_spacing) = if let Some(s) = style {
                    let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                    let ls = s
                        .letter_spacing
                        .to_px(s.font_size, s.root_font_size, max_width);
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
                let w = crate::inline_flow::measure_text_width_with_style_and_spacing(
                    text,
                    font_size,
                    weight,
                    family,
                    letter_spacing,
                );
                measured = measured.max(w);
            }
            crate::box_model::BoxType::ReplacedElement {
                intrinsic_width, ..
            }
            | crate::box_model::BoxType::IFrame {
                intrinsic_width, ..
            }
            | crate::box_model::BoxType::Video {
                intrinsic_width, ..
            }
            | crate::box_model::BoxType::Audio {
                intrinsic_width, ..
            }
            | crate::box_model::BoxType::Canvas {
                intrinsic_width, ..
            } => {
                let w = if let Some(s) = child.style.as_ref() {
                    if !matches!(
                        s.width,
                        mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)
                    ) {
                        s.width.to_px_with_viewport(
                            s.font_size,
                            s.root_font_size,
                            max_width,
                            max_width,
                        )
                    } else {
                        *intrinsic_width
                    }
                } else {
                    *intrinsic_width
                };
                let (p_left, p_right, b_left, b_right, m_left, m_right) =
                    if let Some(cs) = child.style.as_ref() {
                        let c_fs = cs.font_size;
                        let c_root_fs = cs.root_font_size;
                        (
                            cs.padding_left
                                .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.padding_right
                                .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.border_left_width,
                            cs.border_right_width,
                            cs.margin_left
                                .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.margin_right
                                .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        )
                    } else {
                        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                    };
                measured = measured.max(w + p_left + p_right + b_left + b_right + m_left + m_right);
            }
            _ => {
                let child_w = calculate_shrink_to_fit_width(child, max_width);
                let c_style = child.style.as_ref();
                let (p_left, p_right, b_left, b_right, m_left, m_right) = if let Some(cs) = c_style
                {
                    let c_fs = cs.font_size;
                    let c_root_fs = cs.root_font_size;
                    (
                        cs.padding_left
                            .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        cs.padding_right
                            .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        cs.border_left_width,
                        cs.border_right_width,
                        cs.margin_left
                            .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        cs.margin_right
                            .to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                };
                measured =
                    measured.max(child_w + p_left + p_right + b_left + b_right + m_left + m_right);
            }
        }
    }
    measured.min(max_width)
}
