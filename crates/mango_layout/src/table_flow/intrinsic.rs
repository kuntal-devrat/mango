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
            Length::Px(px) => px,
            Length::Rem(rem) => rem * root_fs,
            Length::Em(em) => em * fs,
            _ => 0.0,
        };
        let m_r = match s.margin_right {
            Length::Px(px) => px,
            Length::Rem(rem) => rem * root_fs,
            Length::Em(em) => em * fs,
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
    measure_box_intrinsic_widths_internal(
        node,
        default_font_size,
        default_weight,
        default_family,
        memo,
        None,
        false,
    )
}

/// Measures the intrinsic min-content and max-content width of a node based purely
/// on its contents, ignoring any explicit specified width on the node itself.
pub fn measure_box_content_intrinsic_widths(
    node: &LayoutBox,
    default_font_size: f32,
    default_weight: mango_render::FontWeight,
    default_family: mango_render::FontFamily,
    memo: &mut HashMap<usize, (f32, f32)>,
) -> (f32, f32) {
    measure_box_intrinsic_widths_internal(
        node,
        default_font_size,
        default_weight,
        default_family,
        memo,
        None,
        true,
    )
}

fn measure_box_intrinsic_widths_with_height(
    node: &LayoutBox,
    default_font_size: f32,
    default_weight: mango_render::FontWeight,
    default_family: mango_render::FontFamily,
    memo: &mut HashMap<usize, (f32, f32)>,
    containing_height: Option<f32>,
) -> (f32, f32) {
    measure_box_intrinsic_widths_internal(
        node,
        default_font_size,
        default_weight,
        default_family,
        memo,
        containing_height,
        false,
    )
}

fn measure_box_intrinsic_widths_internal(
    node: &LayoutBox,
    default_font_size: f32,
    default_weight: mango_render::FontWeight,
    default_family: mango_render::FontFamily,
    memo: &mut HashMap<usize, (f32, f32)>,
    containing_height: Option<f32>,
    ignore_own_explicit_width: bool,
) -> (f32, f32) {
    // A percentage-height flex subtree has different intrinsic width when its
    // containing block height changes. Include that resolution context in the
    // memo key so a pre-mutation measurement cannot be reused after relayout.
    let node_key = (node as *const LayoutBox as usize)
        ^ (if ignore_own_explicit_width {
            0x5555_5555
        } else {
            0
        });
    let height_key = containing_height.map(f32::to_bits).unwrap_or(0);
    let key = node_key.rotate_left(17) ^ height_key as usize;
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
        let is_vertical = node
            .style
            .as_ref()
            .map(|s| s.writing_mode.is_vertical())
            .unwrap_or(false);
        if is_vertical {
            let line_h = node
                .style
                .as_ref()
                .and_then(|s| s.line_height)
                .unwrap_or(font_size * 1.2);
            let res = (line_h, line_h);
            memo.insert(key, res);
            return res;
        }
        let preserves_spaces = node.style.as_ref().is_some_and(|s| {
            matches!(
                s.white_space,
                mango_css::values::WhiteSpace::Pre
                    | mango_css::values::WhiteSpace::PreWrap
                    | mango_css::values::WhiteSpace::BreakSpaces
            )
        });
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

        for raw_line in text.split('\n') {
            let line = if preserves_spaces { raw_line } else { raw_line.trim() };
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
        if !ignore_own_explicit_width
            && w.is_none()
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
        if !ignore_own_explicit_width
            && w.is_none()
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
                            mango_css::values::Position::Absolute
                                | mango_css::values::Position::Fixed
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
                if explicit_w.is_none()
                    && child_style.map(|s| s.width == Length::Auto).unwrap_or(true)
                {
                    if !is_row {
                        let height = match child_style.map(|s| &s.height) {
                            Some(Length::Px(px)) if *px > 0.0 => Some(*px),
                            _ if flex_height > 0.0 => Some(flex_height),
                            _ => None,
                        };
                        height.map(|h| h * r)
                    } else {
                        // Row flex container: if child stretches vertically and flex_height is definite
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
                            .map(|s| {
                                s.margin_top == Length::Auto || s.margin_bottom == Length::Auto
                            })
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
            let (c_min, c_max) = if let Some(w) = explicit_w {
                (w + child_margin_h, w + child_margin_h)
            } else if let Some(w) = aspect_ratio_width {
                (w + child_margin_h, w + child_margin_h)
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
                max_w = balance_intrinsic_max_content(&child_max_widths, k, col_gap);
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
        let non_content_h = get_node_horizontal_spacing(node, font_size);
        let res = (
            min_w.max(0.0) + non_content_h,
            max_w.max(0.0) + non_content_h,
        );
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
    if !ignore_own_explicit_width && let Some(style) = &node.style {
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
        let (c_min, c_max) = measure_box_intrinsic_widths_with_height(
            child,
            font_size,
            weight,
            family,
            memo,
            Some(child_style_height(node, containing_height)),
        );
        min_w = min_w.max(c_min);

        let is_child_inline = child.is_inline()
            || child
                .style
                .as_ref()
                .map(|s| {
                    s.float != mango_css::values::Float::None
                        || matches!(
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

fn child_style_height(node: &LayoutBox, containing_height: Option<f32>) -> f32 {
    let Some(style) = node.style.as_ref() else {
        return node.dimensions.content.size.height.max(0.0);
    };
    let base = containing_height
        .filter(|height| *height > 0.0)
        .unwrap_or(node.dimensions.content.size.height);
    let resolved =
        style
            .height
            .to_px_with_viewport(style.font_size, style.root_font_size, base, base);
    if resolved > 0.0 {
        resolved
    } else {
        node.dimensions.content.size.height.max(0.0)
    }
}

pub fn cell_intrinsic_metrics(
    cell: &LayoutBox,
    table_width: f32,
    memo: &mut HashMap<usize, (f32, f32)>,
    is_table_auto_width: bool,
) -> (f32, f32, Option<f32>) {
    let mut explicit_w = None;
    if let Some(style) = &cell.style {
        match style.width {
            Length::Px(px) if px > 0.0 => explicit_w = Some(px),
            Length::Percent(pct) if pct > 0.0 && !is_table_auto_width => {
                explicit_w = Some(table_width * (pct / 100.0));
            }
            _ => {}
        }
    }
    if explicit_w.is_none()
        && let Some(w_attr) = cell.get_attribute("width")
    {
        let w_str = w_attr.trim();
        if let Some(pct_str) = w_str.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>()
                && !is_table_auto_width
            {
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

    let min_content = content_min.max(1.0);
    let max_content = content_max.max(min_content);

    let (final_min, final_max) = if let Some(ew) = explicit_w {
        (min_content.max(ew), max_content.max(ew))
    } else {
        (min_content, max_content)
    };

    (final_min, final_max, explicit_w)
}

fn balance_intrinsic_max_content(sizes: &[f32], k: usize, gap: f32) -> f32 {
    let n = sizes.len();
    if n == 0 {
        return 0.0;
    }
    if k <= 1 || n <= 1 {
        let total: f32 = sizes.iter().sum();
        let gaps = if n > 1 { (n - 1) as f32 * gap } else { 0.0 };
        return total + gaps;
    }
    let k = k.min(n);

    let calc_len = |from: usize, to: usize| -> f32 {
        let items: f32 = sizes[from..to].iter().sum();
        let gaps = (to - from - 1) as f32 * gap;
        items + gaps
    };

    // dp[m][j] = (min_max_len: f32, counts: Vec<usize>)
    let mut dp: Vec<Vec<Option<(f32, Vec<usize>)>>> = vec![vec![None; n + 1]; k + 1];

    for j in 1..=(n - k + 1) {
        dp[1][j] = Some((calc_len(0, j), vec![j]));
    }

    for m in 2..=k {
        for j in m..=(n - k + m) {
            let mut best: Option<(f32, Vec<usize>)> = None;
            for p in (m - 1)..j {
                if let Some((prev_max, prev_counts)) = &dp[m - 1][p] {
                    let cur_len = calc_len(p, j);
                    let candidate_max = prev_max.max(cur_len);
                    let mut candidate_counts = prev_counts.clone();
                    candidate_counts.push(j - p);

                    let is_better = match &best {
                        None => true,
                        Some((best_max, best_counts)) => {
                            if candidate_max < *best_max - 1e-3 {
                                true
                            } else if (candidate_max - *best_max).abs() <= 1e-3 {
                                candidate_counts > *best_counts
                            } else {
                                false
                            }
                        }
                    };
                    if is_better {
                        best = Some((candidate_max, candidate_counts));
                    }
                }
            }
            dp[m][j] = best;
        }
    }

    dp[k][n]
        .as_ref()
        .map(|(val, _)| *val)
        .unwrap_or_else(|| sizes.iter().sum::<f32>() + (n - 1) as f32 * gap)
}
