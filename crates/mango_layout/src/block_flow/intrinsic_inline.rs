//! Inline formatting context shrink-to-fit width calculation.

use crate::box_tree::LayoutBox;

use super::intrinsic::calculate_shrink_to_fit_width;

/// Calculates the shrink-to-fit width of a block formatting context element containing inline children.
pub fn calculate_inline_shrink_to_fit_width(box_node: &LayoutBox, max_width: f32) -> f32 {
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
                *curr_line +=
                    *pending_space + w + p_left + p_right + b_left + b_right + m_left + m_right;
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
                *curr_line +=
                    *pending_space + w + p_left + p_right + b_left + b_right + m_left + m_right;
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
    max_line_w.min(max_width).max(min_content_w)
}
