use std::collections::HashMap;

use mango_css::values::{BoxSizing, Display, Length};

use crate::box_tree::LayoutBox;

use super::types::{is_table_cell, is_table_row, is_table_row_group};

pub fn get_node_horizontal_spacing(node: &LayoutBox, _font_size: f32) -> f32 {
    if let Some(s) = &node.style {
        let root_fs = s.root_font_size;
        let fs = s.font_size;
        let p_l = s.padding_left.to_px(fs, root_fs, 0.0);
        let p_r = s.padding_right.to_px(fs, root_fs, 0.0);
        let b_l = s.border_left_width;
        let b_r = s.border_right_width;
        let m_l = match s.margin_left {
            Length::Px(px) if px > 0.0 => px,
            Length::Rem(rem) if rem > 0.0 => rem * root_fs,
            Length::Em(em) if em > 0.0 => em * fs,
            _ => 0.0,
        };
        let m_r = match s.margin_right {
            Length::Px(px) if px > 0.0 => px,
            Length::Rem(rem) if rem > 0.0 => rem * root_fs,
            Length::Em(em) if em > 0.0 => em * fs,
            _ => 0.0,
        };
        p_l + p_r + b_l + b_r + m_l + m_r
    } else {
        node.dimensions.padding.left
            + node.dimensions.padding.right
            + node.dimensions.border.left
            + node.dimensions.border.right
            + node.dimensions.margin.left
            + node.dimensions.margin.right
    }
}

/// Measures the intrinsic min-content and max-content width of a node.
/// Recognizes nested tables and computes their min/max content recursively.
pub fn measure_box_intrinsic_widths(
    node: &LayoutBox,
    default_font_size: f32,
    default_weight: mango_render::FontWeight,
    default_family: mango_render::FontFamily,
    memo: &mut HashMap<usize, (f32, f32)>,
) -> (f32, f32) {
    let key = node as *const LayoutBox as usize;
    if let Some(&res) = memo.get(&key) {
        return res;
    }

    if node.style.as_ref().is_some_and(|style| {
        style.display == Display::None
            || matches!(
                style.position,
                mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
            )
    }) {
        let res = (0.0, 0.0);
        memo.insert(key, res);
        return res;
    }

    let font_size = node
        .style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or(default_font_size);
    let is_bold = node
        .style
        .as_ref()
        .map(|s| crate::inline_flow::is_bold_weight(s.font_weight, s.font_size))
        .unwrap_or(false);
    let weight = if is_bold {
        mango_render::FontWeight::Bold
    } else {
        default_weight
    };
    let family = node
        .style
        .as_ref()
        .map(|s| mango_render::FontFamily::from_css_name(&s.font_family))
        .unwrap_or(default_family);

    // 1. Text node measurement
    if let Some(text) = node.text() {
        let is_nowrap = node.style.as_ref().is_some_and(|s| {
            matches!(
                s.white_space,
                mango_css::values::WhiteSpace::Nowrap | mango_css::values::WhiteSpace::Pre
            )
        });
        let letter_spacing = node
            .style
            .as_ref()
            .map(|s| s.letter_spacing.to_px(s.font_size, s.root_font_size, 0.0))
            .unwrap_or(0.0);
        let mut max_word_w = 0.0f32;
        let mut max_line_w = 0.0f32;

        for line in text.split('\n') {
            let line_w = crate::inline_flow::measure_text_width_with_style_and_spacing(
                line,
                font_size,
                weight,
                family,
                letter_spacing,
            );
            if line_w > max_line_w {
                max_line_w = line_w;
            }
            if !is_nowrap {
                for word in line.split_whitespace() {
                    let word_w = crate::inline_flow::measure_text_width_with_style_and_spacing(
                        word,
                        font_size,
                        weight,
                        family,
                        letter_spacing,
                    );
                    if word_w > max_word_w {
                        max_word_w = word_w;
                    }
                }
            }
        }
        let min_w = if is_nowrap { max_line_w } else { max_word_w };
        let res = (min_w, max_line_w);
        memo.insert(key, res);
        return res;
    }

    // 2. Form controls & Replaced elements
    if node.tag_name.as_deref() == Some("select") {
        let mut w = None;
        if let Some(style) = &node.style {
            match style.width {
                Length::Px(px) if px > 0.0 => w = Some(px),
                _ => {}
            }
        }
        let fixed_w = w.unwrap_or_else(|| {
            let text = node
                .get_attribute("_mango_selected_text")
                .or_else(|| node.get_attribute("value"))
                .unwrap_or("Select...");
            let pad_h = node.dimensions.padding.left
                + node.dimensions.padding.right
                + node.dimensions.border.left
                + node.dimensions.border.right;
            let text_w =
                crate::inline_flow::measure_text_width_with_style(text, font_size, weight, family);
            let arrow_space = if node
                .style
                .as_ref()
                .map(|s| s.appearance == mango_css::values::Appearance::None)
                .unwrap_or(false)
            {
                20.0
            } else {
                28.0
            };
            (text_w + pad_h + arrow_space).max(60.0)
        });
        let res = (fixed_w, fixed_w);
        memo.insert(key, res);
        return res;
    }

    if node.tag_name.as_deref() == Some("input") {
        let mut w = None;
        if let Some(w_attr) = node.get_attribute("width") {
            let trimmed = w_attr.trim().trim_end_matches("px");
            if let Ok(val) = trimmed.parse::<f32>()
                && val > 0.0
            {
                w = Some(val);
            }
        }
        if w.is_none()
            && let Some(style) = &node.style
        {
            match style.width {
                Length::Px(px) if px > 0.0 => w = Some(px),
                _ => {}
            }
        }
        let fixed_w = w.unwrap_or_else(|| {
            let itype = node.get_attribute("type").unwrap_or("text");
            if itype == "checkbox" || itype == "radio" {
                return 16.0;
            }
            let pad_h = node.dimensions.padding.left
                + node.dimensions.padding.right
                + node.dimensions.border.left
                + node.dimensions.border.right;
            if itype == "submit" || itype == "button" || itype == "reset" {
                let val = node.get_attribute("value").unwrap_or("Submit");
                let text_w = crate::inline_flow::measure_text_width_with_style(
                    val, font_size, weight, family,
                );
                return (text_w + pad_h + 16.0).max(40.0);
            }
            let size = node
                .get_attribute("size")
                .and_then(|s| s.trim().parse::<f32>().ok())
                .unwrap_or(20.0);
            (size * (font_size * 0.55) + pad_h + 8.0).max(60.0)
        });
        let res = (fixed_w, fixed_w);
        memo.insert(key, res);
        return res;
    }

    if node.tag_name.as_deref() == Some("button")
        && !matches!(
            node.style.as_ref().map(|s| s.display),
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
        )
    {
        let mut w = None;
        if let Some(style) = &node.style {
            match style.width {
                Length::Px(px) if px > 0.0 => w = Some(px),
                _ => {}
            }
        }
        if let Some(fixed_w) = w {
            let res = (fixed_w, fixed_w);
            memo.insert(key, res);
            return res;
        }
        // If button has children (text/elements), measure child content
        let mut sum_min = 0.0f32;
        let mut sum_max = 0.0f32;
        for child in &node.children {
            let (c_min, c_max) =
                measure_box_intrinsic_widths(child, font_size, weight, family, memo);
            sum_min += c_min;
            sum_max += c_max;
        }
        let pad_h = get_node_horizontal_spacing(node, font_size);
        let default_pad = if node.style.as_ref().is_none_or(|s| {
            s.padding_left == Length::Auto || matches!(s.padding_left, Length::Px(px) if px == 0.0)
        }) {
            12.0
        } else {
            0.0
        };
        let btn_min = (sum_min + pad_h + default_pad).max(32.0);
        let btn_max = (sum_max + pad_h + default_pad).max(32.0);
        let is_nowrap = node.style.as_ref().is_some_and(|s| {
            matches!(
                s.white_space,
                mango_css::values::WhiteSpace::Nowrap | mango_css::values::WhiteSpace::Pre
            )
        });
        let btn_min = if is_nowrap {
            btn_min.max(btn_max)
        } else {
            btn_min
        };
        let res = (btn_min, btn_max);
        memo.insert(key, res);
        return res;
    }

    if matches!(
        node.tag_name.as_deref(),
        Some("img") | Some("svg") | Some("iframe") | Some("canvas") | Some("video")
    ) || node.box_type.is_replaced()
    {
        let mut w = None;
        if let Some(w_attr) = node.get_attribute("width") {
            let trimmed = w_attr.trim().trim_end_matches("px");
            if let Ok(val) = trimmed.parse::<f32>()
                && val > 0.0
            {
                w = Some(val);
            }
        }
        if w.is_none()
            && let Some(style) = &node.style
        {
            match style.width {
                Length::Px(px) if px > 0.0 => w = Some(px),
                _ => {}
            }
        }
        let fixed_w = w.unwrap_or(if node.dimensions.content.width() > 0.0 {
            node.dimensions.content.width()
        } else {
            match &node.box_type {
                crate::box_model::BoxType::ReplacedElement {
                    intrinsic_width, ..
                } if *intrinsic_width > 0.0 => *intrinsic_width,
                _ => 16.0,
            }
        });
        let is_border_box = node
            .style
            .as_ref()
            .map(|s| s.box_sizing == BoxSizing::BorderBox)
            .unwrap_or(false);
        let spacing = get_node_horizontal_spacing(node, font_size);
        let total_w = if is_border_box {
            let m_h = if let Some(s) = &node.style {
                let fs = s.font_size;
                let rfs = s.root_font_size;
                let ml = s.margin_left.to_px(fs, rfs, 0.0);
                let mr = s.margin_right.to_px(fs, rfs, 0.0);
                ml + mr
            } else {
                node.dimensions.margin.left + node.dimensions.margin.right
            };
            fixed_w + m_h
        } else {
            fixed_w + spacing
        };
        let res = (total_w, total_w);
        memo.insert(key, res);
        return res;
    }

    // Flex container intrinsic measurement
    if matches!(
        node.style.as_ref().map(|s| s.display),
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) {
        let is_row = matches!(
            node.style.as_ref().map(|s| s.flex_direction),
            Some(mango_css::values::FlexDirection::Row)
                | Some(mango_css::values::FlexDirection::RowReverse)
                | None
        );
        let is_wrap = matches!(
            node.style.as_ref().map(|s| s.flex_wrap),
            Some(mango_css::values::FlexWrap::Wrap)
                | Some(mango_css::values::FlexWrap::WrapReverse)
        );
        let col_gap = node
            .style
            .as_ref()
            .map(|s| s.column_gap.to_px(s.font_size, s.root_font_size, 0.0))
            .unwrap_or(0.0);
        let mut min_w = 0.0f32;
        let mut max_w = 0.0f32;
        let mut child_count = 0;
        for child in &node.children {
            let child_style = child.style.as_ref();
            if child_style
                .map(|s| {
                    matches!(
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
                        Length::Px(px) if px > 0.0 => px,
                        Length::Rem(rem) if rem > 0.0 => rem * rfs,
                        Length::Em(em) if em > 0.0 => em * fs,
                        _ => 0.0,
                    };
                    let mr = match s.margin_right {
                        Length::Px(px) if px > 0.0 => px,
                        Length::Rem(rem) if rem > 0.0 => rem * rfs,
                        Length::Em(em) if em > 0.0 => em * fs,
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
            let (c_min, c_max) = if let Some(w) = explicit_w {
                (w + child_margin_h, w + child_margin_h)
            } else {
                measure_box_intrinsic_widths(child, font_size, weight, family, memo)
            };
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
            }
        }
        if is_row && child_count > 1 {
            let total_gap = (child_count - 1) as f32 * col_gap;
            max_w += total_gap;
            if !is_wrap {
                min_w += total_gap;
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
        let non_content_h = get_node_horizontal_spacing(node, font_size);
        let res = (min_w + non_content_h, max_w + non_content_h);
        memo.insert(key, res);
        return res;
    }

    // Grid container intrinsic measurement
    if matches!(
        node.style.as_ref().map(|s| s.display),
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        let (grid_min_w, grid_max_w) =
            crate::grid_flow::measure_grid_intrinsic_widths(node, font_size);
        let non_content_h = get_node_horizontal_spacing(node, font_size);
        let res = (grid_min_w + non_content_h, grid_max_w + non_content_h);
        memo.insert(key, res);
        return res;
    }

    // 3. Nested table intrinsic measurement
    if node.tag_name.as_deref() == Some("table")
        || node.style.as_ref().map(|s| s.display) == Some(Display::Table)
    {
        let mut explicit_table_w = None;
        if let Some(w_attr) = node.get_attribute("width") {
            let trimmed = w_attr.trim().trim_end_matches("px");
            if let Ok(val) = trimmed.parse::<f32>()
                && val > 0.0
            {
                explicit_table_w = Some(val);
            }
        }
        if explicit_table_w.is_none()
            && let Some(style) = &node.style
        {
            match style.width {
                Length::Px(px) if px > 0.0 => explicit_table_w = Some(px),
                _ => {}
            }
        }

        // Measure columns in the nested table
        let mut n_cols_min: Vec<f32> = Vec::new();
        let mut n_cols_max: Vec<f32> = Vec::new();

        let mut visit_nested_row = |row: &LayoutBox| {
            let mut c_idx = 0;
            for cell in &row.children {
                if is_table_cell(cell) {
                    let span = cell
                        .get_attribute("colspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    while n_cols_min.len() < c_idx + span {
                        n_cols_min.push(0.0);
                        n_cols_max.push(0.0);
                    }
                    if span == 1 {
                        let (c_min, c_max) = measure_box_intrinsic_widths(
                            cell,
                            default_font_size,
                            default_weight,
                            default_family,
                            memo,
                        );
                        n_cols_min[c_idx] = n_cols_min[c_idx].max(c_min);
                        n_cols_max[c_idx] = n_cols_max[c_idx].max(c_max);
                    }
                    c_idx += span;
                }
            }
        };

        for child in &node.children {
            if is_table_row_group(child) {
                for r in &child.children {
                    if is_table_row(r) {
                        visit_nested_row(r);
                    }
                }
            } else if is_table_row(child) {
                visit_nested_row(child);
            }
        }

        let non_content_h = get_node_horizontal_spacing(node, font_size);

        let table_min = n_cols_min.iter().sum::<f32>() + non_content_h;
        let table_max = explicit_table_w.unwrap_or(n_cols_max.iter().sum::<f32>() + non_content_h);

        let res = (table_min.max(1.0), table_max.max(table_min));
        memo.insert(key, res);
        return res;
    }

    // 4. General container: check explicit width first, otherwise max of child intrinsic widths
    if let Some(style) = &node.style {
        match style.width {
            Length::Px(px) if px >= 0.0 => {
                let non_content_h = get_node_horizontal_spacing(node, font_size);
                let w = if style.box_sizing == BoxSizing::BorderBox {
                    (px - non_content_h).max(0.0)
                } else {
                    px
                };
                let res = (w + non_content_h, w + non_content_h);
                memo.insert(key, res);
                return res;
            }
            _ => {}
        }
    }

    let mut min_w = 0.0f32;
    let mut max_w = 0.0f32;
    let mut current_line_max = 0.0f32;
    for child in &node.children {
        if child.style.as_ref().is_some_and(|child_style| {
            child_style.display == Display::None
                || matches!(
                    child_style.position,
                    mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                )
        }) {
            continue;
        }
        let (c_min, c_max) = measure_box_intrinsic_widths(child, font_size, weight, family, memo);
        min_w = min_w.max(c_min);

        let is_child_inline = child.is_inline()
            || child
                .style
                .as_ref()
                .map(|s| {
                    matches!(
                        s.display,
                        Display::Inline
                            | Display::InlineBlock
                            | Display::InlineFlex
                            | Display::InlineGrid
                    )
                })
                .unwrap_or(false);

        if is_child_inline {
            current_line_max += c_max;
        } else {
            max_w = max_w.max(current_line_max);
            current_line_max = 0.0;
            max_w = max_w.max(c_max);
        }
    }
    max_w = max_w.max(current_line_max);
    let is_nowrap = node.style.as_ref().is_some_and(|s| {
        matches!(
            s.white_space,
            mango_css::values::WhiteSpace::Nowrap | mango_css::values::WhiteSpace::Pre
        )
    });
    if is_nowrap {
        min_w = min_w.max(max_w);
    }
    let non_content_h = get_node_horizontal_spacing(node, font_size);
    let res = (min_w + non_content_h, max_w + non_content_h);
    memo.insert(key, res);
    res
}

pub fn cell_intrinsic_metrics(
    cell: &LayoutBox,
    table_width: f32,
    memo: &mut HashMap<usize, (f32, f32)>,
) -> (f32, f32, Option<f32>) {
    let mut explicit_w = None;
    if let Some(style) = &cell.style {
        match style.width {
            Length::Px(px) if px > 0.0 => explicit_w = Some(px),
            Length::Percent(pct) if pct > 0.0 => explicit_w = Some(table_width * (pct / 100.0)),
            _ => {}
        }
    }
    if explicit_w.is_none()
        && let Some(w_attr) = cell.get_attribute("width")
    {
        let w_str = w_attr.trim();
        if let Some(pct_str) = w_str.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>() {
                explicit_w = Some(table_width * (pct / 100.0));
            }
        } else {
            let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
            if let Ok(px) = num_str.parse::<f32>()
                && px > 0.0
            {
                explicit_w = Some(px);
            }
        }
    }

    let h_padding = cell.dimensions.padding.left
        + cell.dimensions.padding.right
        + cell.dimensions.border.left
        + cell.dimensions.border.right;

    let font_size = cell.style.as_ref().map(|s| s.font_size).unwrap_or(14.0);
    let is_bold = cell
        .style
        .as_ref()
        .map(|s| crate::inline_flow::is_bold_weight(s.font_weight, s.font_size))
        .unwrap_or(false);
    let weight = if is_bold {
        mango_render::FontWeight::Bold
    } else {
        mango_render::FontWeight::Regular
    };
    let family = cell
        .style
        .as_ref()
        .map(|s| mango_render::FontFamily::from_css_name(&s.font_family))
        .unwrap_or(mango_render::FontFamily::SansSerif);

    let (content_min, content_max) =
        measure_box_intrinsic_widths(cell, font_size, weight, family, memo);

    let min_content = (content_min + h_padding).max(1.0);
    let max_content = (content_max + h_padding).max(min_content);

    let (final_min, final_max) = if let Some(ew) = explicit_w {
        (min_content.max(ew), max_content.max(ew))
    } else {
        (min_content, max_content)
    };

    (final_min, final_max, explicit_w)
}
