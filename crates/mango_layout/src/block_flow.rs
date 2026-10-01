//! Block formatting context: normal flow, width calculation, margin collapsing, and positioning.
//!
//! Implements CSS 2.1 §9.4.1 (Block formatting contexts) and §10.3.3 (Block-level normal flow width).

use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::{BoxSizing, BreakInside, Clear, ColumnSpan, Display, Float, Length, Position};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;
use crate::inline_flow::layout_inline_children;

/// Recursively lays out a block container box in a block formatting context.
pub fn layout_block(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    let display = box_node.style.as_ref().map(|s| s.display);
    if matches!(display, Some(Display::Flex) | Some(Display::InlineFlex)) {
        crate::flex_flow::layout_flex(box_node, containing_block, float_ctx);
        return;
    }
    if display == Some(Display::Grid) || display == Some(Display::InlineGrid) {
        crate::grid_flow::layout_grid(box_node, containing_block, float_ctx);
        return;
    }

    if box_node.style.as_ref().map(|s| s.display) == Some(Display::Table)
        || box_node.tag_name.as_deref() == Some("table")
    {
        crate::table_flow::layout_table(box_node, containing_block, float_ctx);
        return;
    }

    // 1. Calculate horizontal dimensions (width, padding, borders, margins)
    calculate_block_width(box_node, containing_block);

    // 2. Calculate vertical position (respecting clearance)
    let start_y = containing_block.content.y();
    calculate_block_position(box_node, containing_block, start_y, float_ctx);

    // If box_node has a specified height, resolve it tentatively
    // so children (like percent-height or max-height inlines) have a containing block height.
    if let Some(style) = &box_node.style {
        if style.height != Length::Auto {
            let font_size = style.font_size;
            let root_font_size = style.root_font_size;
            let cb_h = containing_block.content.height();
            let tentative_h = style.height.to_px_with_viewport(font_size, root_font_size, cb_h, 600.0);
            if tentative_h > 0.0 {
                box_node.dimensions.content.size.height = tentative_h;
            }
        }
    }

    // 3. Lay out child boxes
    layout_block_children(box_node, float_ctx);

    // 4. Calculate final height
    calculate_block_height(box_node, containing_block);
    center_button_children(box_node);

    // 5. Position out-of-flow positioned children against resolved container dimensions
    layout_positioned_children(box_node, float_ctx);

    // 6. If this box itself is a float, register its margin box in FloatContext
    if let Some(style) = &box_node.style {
        match style.float {
            Float::Left => {
                float_ctx.add_left_float(box_node.dimensions.margin_box());
            }
            Float::Right => {
                // Adjust position to the right edge of container
                let margin_box_w = box_node.dimensions.margin_box().width();
                let new_x = containing_block.content.right() - margin_box_w;
                let dx = new_x - box_node.dimensions.margin_box().x();
                box_node.dimensions.content.origin.x += dx;
                float_ctx.add_right_float(box_node.dimensions.margin_box());
            }
            Float::None => {}
        }
    }
}

/// Computes the horizontal dimensions of a block box according to CSS 2.1 §10.3.3.
fn calculate_block_width(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    let style = box_node.style.clone().unwrap_or_default();
    let container_width = containing_block.content.width();
    let container_height = containing_block.content.height().max(0.0);

    // 1. Padding and borders
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    let pad_top = style.padding_top.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
    let pad_right = style.padding_right.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
    let pad_bottom = style.padding_bottom.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
    let pad_left = style.padding_left.to_px_with_viewport(font_size, root_font_size, container_width, container_height);

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    box_node.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    box_node.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);

    // 2. Margins and Width
    let margin_top = style.margin_top.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
    let margin_bottom = style.margin_bottom.to_px_with_viewport(font_size, root_font_size, container_width, container_height);

    let is_width_auto = style.width == Length::Auto;
    let is_margin_left_auto = style.margin_left == Length::Auto;
    let is_margin_right_auto = style.margin_right == Length::Auto;

    let mut margin_left = style.margin_left.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
    let mut margin_right = style.margin_right.to_px_with_viewport(font_size, root_font_size, container_width, container_height);

    let total_non_content_h = pad_left + pad_right + border_left + border_right;

    let is_positioned = matches!(
        style.position,
        Position::Absolute | Position::Fixed
    );
    let is_floating = style.float != Float::None;
    let is_inline_level = box_node.box_type != crate::box_model::BoxType::BlockNode
        && (style.display.is_inline_level()
            || matches!(
                box_node.box_type,
                crate::box_model::BoxType::InlineBlock
                    | crate::box_model::BoxType::InlineNode
                    | crate::box_model::BoxType::ReplacedElement { .. }
                    | crate::box_model::BoxType::IFrame { .. }
                    | crate::box_model::BoxType::Video { .. }
                    | crate::box_model::BoxType::Audio { .. }
                    | crate::box_model::BoxType::Canvas { .. }
            ));

    let tentative_width = if is_width_auto {
        if let crate::box_model::BoxType::ReplacedElement { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::IFrame { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::Video { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::Audio { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::Canvas { intrinsic_width, intrinsic_height, .. } = &box_node.box_type
        {
            if style.height != Length::Auto && *intrinsic_height > 0.0 {
                let ratio = if let Some(r) = style.aspect_ratio && r > 0.0 {
                    r
                } else {
                    *intrinsic_width / *intrinsic_height
                };
                let raw_h = style.height.to_px_with_viewport(font_size, root_font_size, container_height, container_height);
                (raw_h * ratio).max(1.0)
            } else {
                *intrinsic_width
            }
        } else if is_inline_level || is_floating {
            if let Some(ratio) = style.aspect_ratio && ratio > 0.0 && style.height != Length::Auto {
                let raw_h = style.height.to_px_with_viewport(font_size, root_font_size, container_height, container_height);
                (raw_h * ratio).max(1.0)
            } else {
                calculate_shrink_to_fit_width(box_node, (container_width - total_non_content_h).max(0.0))
            }
        } else if let Some(ratio) = style.aspect_ratio && ratio > 0.0 && style.height != Length::Auto {
            let raw_h = style.height.to_px_with_viewport(font_size, root_font_size, container_height, container_height);
            (raw_h * ratio).max(1.0)
        } else {
            (container_width - total_non_content_h - margin_left - margin_right).max(0.0)
        }
    } else {
        let raw_w = style.width.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_w - total_non_content_h).max(0.0)
        } else {
            raw_w.max(0.0)
        }
    };

    // Apply min-width and max-width clamping (CSS 2.1 § 10.4)
    let mut clamped_w = tentative_width;
    if style.max_width != Length::Auto {
        let raw_max = style.max_width.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
        let max_content_w = if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - total_non_content_h).max(0.0)
        } else {
            raw_max
        };
        clamped_w = clamped_w.min(max_content_w);
    }
    if style.min_width != Length::Auto {
        let raw_min = style.min_width.to_px_with_viewport(font_size, root_font_size, container_width, container_height);
        let min_content_w = if style.box_sizing == BoxSizing::BorderBox {
            (raw_min - total_non_content_h).max(0.0)
        } else {
            raw_min
        };
        clamped_w = clamped_w.max(min_content_w);
    }

    if is_inline_level || is_positioned || is_floating {
        if is_margin_left_auto {
            margin_left = 0.0;
        }
        if is_margin_right_auto {
            margin_right = 0.0;
        }
    } else {
        let underflow = container_width
            - (clamped_w + total_non_content_h + margin_left + margin_right);

        if is_margin_left_auto && is_margin_right_auto {
            // Margin auto centering (e.g. margin: 0 auto; or max-width: 1200px; margin: 0 auto;)
            margin_left = (underflow / 2.0).max(0.0);
            margin_right = (underflow / 2.0).max(0.0);
        } else if is_margin_left_auto {
            margin_left = underflow.max(0.0);
        } else if is_margin_right_auto {
            margin_right = underflow.max(0.0);
        } else if is_width_auto && clamped_w == tentative_width {
            // Normal in-flow block with auto width expands to fill container, margin-right absorbs 0
        } else {
            // Over-constrained: margin-right absorbs underflow
            margin_right += underflow;
        }
    }

    box_node.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    box_node.dimensions.content.size.width = clamped_w;
}

pub fn calculate_shrink_to_fit_width(box_node: &LayoutBox, max_width: f32) -> f32 {
    if let crate::box_model::BoxType::TextNode(text) = &box_node.box_type {
        let style = box_node.style.as_ref();
        let (font_size, is_bold, family_name) = if let Some(s) = style {
            let bold = match s.font_weight {
                mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
                mango_css::values::FontWeight::Numeric(w) => w >= 600,
                _ => false,
            };
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
        let w = crate::inline_flow::measure_text_width_with_style(text, font_size, weight, family);
        return w.min(max_width);
    }
    if let crate::box_model::BoxType::ReplacedElement { intrinsic_width, .. }
        | crate::box_model::BoxType::IFrame { intrinsic_width, .. }
        | crate::box_model::BoxType::Video { intrinsic_width, .. }
        | crate::box_model::BoxType::Audio { intrinsic_width, .. }
        | crate::box_model::BoxType::Canvas { intrinsic_width, .. } = &box_node.box_type
    {
        return (*intrinsic_width).min(max_width);
    }

    if let Some(s) = box_node.style.as_ref() {
        if !matches!(s.width, mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)) {
            let font_size = s.font_size;
            let root_font_size = s.root_font_size;
            let raw_w = s.width.to_px_with_viewport(font_size, root_font_size, max_width, max_width);
            let mut w = raw_w;
            if !matches!(s.max_width, mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)) {
                let max_w = s.max_width.to_px_with_viewport(font_size, root_font_size, max_width, max_width);
                w = w.min(max_w);
            }
            if !matches!(s.min_width, mango_css::values::Length::Auto | mango_css::values::Length::Percent(_)) {
                let min_w = s.min_width.to_px_with_viewport(font_size, root_font_size, max_width, max_width);
                w = w.max(min_w);
            }
            return w.min(max_width);
        }

        if matches!(s.display, mango_css::values::Display::Flex | mango_css::values::Display::InlineFlex) {
            let font_size = s.font_size;
            let root_font_size = s.root_font_size;
            let is_row = matches!(
                s.flex_direction,
                mango_css::values::FlexDirection::Row | mango_css::values::FlexDirection::RowReverse
            );
            if is_row {
                let col_gap = s.column_gap.to_px_with_viewport(font_size, root_font_size, max_width, max_width);
                let mut sum_w = 0.0f32;
                let mut count = 0;
                for child in &box_node.children {
                    let pos = child.style.as_ref().map(|s| s.position).unwrap_or(mango_css::values::Position::Static);
                    if matches!(pos, mango_css::values::Position::Absolute | mango_css::values::Position::Fixed) {
                        continue;
                    }
                    let child_w = calculate_shrink_to_fit_width(child, max_width);
                    let c_style = child.style.as_ref();
                    let (p_left, p_right, b_left, b_right, m_left, m_right) = if let Some(cs) = c_style {
                        let c_fs = cs.font_size;
                        let c_root_fs = cs.root_font_size;
                        (
                            cs.padding_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.padding_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.border_left_width,
                            cs.border_right_width,
                            cs.margin_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.margin_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
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
                return (sum_w.ceil() + 1.0).min(max_width);
            } else {
                let mut max_w = 0.0f32;
                for child in &box_node.children {
                    let pos = child.style.as_ref().map(|s| s.position).unwrap_or(mango_css::values::Position::Static);
                    if matches!(pos, mango_css::values::Position::Absolute | mango_css::values::Position::Fixed) {
                        continue;
                    }
                    let child_w = calculate_shrink_to_fit_width(child, max_width);
                    let c_style = child.style.as_ref();
                    let (p_left, p_right, b_left, b_right, m_left, m_right) = if let Some(cs) = c_style {
                        let c_fs = cs.font_size;
                        let c_root_fs = cs.root_font_size;
                        (
                            cs.padding_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.padding_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.border_left_width,
                            cs.border_right_width,
                            cs.margin_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.margin_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        )
                    } else {
                        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                    };
                    max_w = max_w.max(child_w + p_left + p_right + b_left + b_right + m_left + m_right);
                }
                return (max_w.ceil() + 1.0).min(max_width);
            }
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
                            let bold = match s.font_weight {
                                mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
                                mango_css::values::FontWeight::Numeric(w) => w >= 600,
                                _ => false,
                            };
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
                        let w = crate::inline_flow::measure_text_width_with_style(val, font_size, weight, family);
                        return w.min(max_width);
                    }
                    _ => {
                        let size_attr = box_node.get_attribute("size").and_then(|s| s.parse::<f32>().ok()).unwrap_or(20.0);
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
                    let bold = match s.font_weight {
                        mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
                        mango_css::values::FontWeight::Numeric(w) => w >= 600,
                        _ => false,
                    };
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
                            content_w += crate::inline_flow::measure_text_width_with_style(t, font_size, weight, family);
                        }
                        crate::box_model::BoxType::ReplacedElement { intrinsic_width, .. }
                        | crate::box_model::BoxType::IFrame { intrinsic_width, .. }
                        | crate::box_model::BoxType::Video { intrinsic_width, .. }
                        | crate::box_model::BoxType::Audio { intrinsic_width, .. }
                        | crate::box_model::BoxType::Canvas { intrinsic_width, .. } => {
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
                    content_w = crate::inline_flow::measure_text_width_with_style(val, font_size, weight, family);
                }
                return content_w.min(max_width);
            }
            _ => {}
        }
    }

    let has_inlines = box_node.children.iter().any(|c| {
        let pos = c.style.as_ref().map(|s| s.position).unwrap_or(mango_css::values::Position::Static);
        !matches!(pos, mango_css::values::Position::Absolute | mango_css::values::Position::Fixed) && c.is_inline()
    });
    if has_inlines {
        let mut max_line_w = 0.0f32;
        let mut curr_line_w = 0.0f32;

        fn accumulate_inline_width(
            node: &LayoutBox,
            parent_style: Option<&mango_css::computed::ComputedStyle>,
            curr_line: &mut f32,
            max_line: &mut f32,
            max_width: f32,
        ) {
            let pos = node.style.as_ref().map(|s| s.position).unwrap_or(mango_css::values::Position::Static);
            if matches!(pos, mango_css::values::Position::Absolute | mango_css::values::Position::Fixed) {
                return;
            }
            if node.tag_name.as_deref() == Some("br") {
                *max_line = max_line.max(*curr_line);
                *curr_line = 0.0;
                return;
            }
            match &node.box_type {
                crate::box_model::BoxType::InlineBlock => {
                    let w = calculate_shrink_to_fit_width(node, max_width);
                    let c_style = node.style.as_ref();
                    let (p_left, p_right, b_left, b_right, m_left, m_right) = if let Some(cs) = c_style {
                        let c_fs = cs.font_size;
                        let c_root_fs = cs.root_font_size;
                        (
                            cs.padding_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.padding_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.border_left_width,
                            cs.border_right_width,
                            cs.margin_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                            cs.margin_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        )
                    } else {
                        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                    };
                    *curr_line += w + p_left + p_right + b_left + b_right + m_left + m_right;
                }
                crate::box_model::BoxType::TextNode(raw_text) => {
                    let style = node.style.as_ref().or(parent_style);
                    let (font_size, is_bold, family_name, transformed_text) = if let Some(s) = style {
                        let bold = match s.font_weight {
                            mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
                            mango_css::values::FontWeight::Numeric(w) => w >= 600,
                            _ => false,
                        };
                        (s.font_size, bold, s.font_family.clone(), s.text_transform.apply(raw_text))
                    } else {
                        (16.0, false, "sans-serif".to_string(), raw_text.clone())
                    };
                    let weight = if is_bold {
                        mango_render::FontWeight::Bold
                    } else {
                        mango_render::FontWeight::Regular
                    };
                    let family = mango_render::FontFamily::from_css_name(&family_name);

                    // Tokenize into words and spaces matching collect_inline_atoms
                    let mut text_w = 0.0f32;
                    let mut current_word = String::new();
                    for ch in transformed_text.chars() {
                        if ch.is_ascii_whitespace() {
                            if !current_word.is_empty() {
                                text_w += crate::inline_flow::measure_text_width_with_style(
                                    &current_word,
                                    font_size,
                                    weight,
                                    family,
                                );
                                current_word.clear();
                            }
                            text_w += crate::inline_flow::measure_text_width_with_style(
                                " ",
                                font_size,
                                weight,
                                family,
                            );
                        } else {
                            current_word.push(ch);
                        }
                    }
                    if !current_word.is_empty() {
                        text_w += crate::inline_flow::measure_text_width_with_style(
                            &current_word,
                            font_size,
                            weight,
                            family,
                        );
                    }
                    *curr_line += text_w.ceil() + 2.0;
                }
                crate::box_model::BoxType::ReplacedElement { intrinsic_width, .. }
                | crate::box_model::BoxType::IFrame { intrinsic_width, .. }
                | crate::box_model::BoxType::Video { intrinsic_width, .. }
                | crate::box_model::BoxType::Audio { intrinsic_width, .. }
                | crate::box_model::BoxType::Canvas { intrinsic_width, .. } => {
                    *curr_line += *intrinsic_width;
                }
                _ => {
                    let next_parent_style = node.style.as_ref().or(parent_style);
                    for c in &node.children {
                        accumulate_inline_width(c, next_parent_style, curr_line, max_line, max_width);
                    }
                }
            }
        }

        let p_style = box_node.style.as_ref();
        for child in &box_node.children {
            accumulate_inline_width(child, p_style, &mut curr_line_w, &mut max_line_w, max_width);
        }
        max_line_w = max_line_w.max(curr_line_w);
        return max_line_w.min(max_width);
    }

    let mut measured = 0.0f32;
    for child in &box_node.children {
        let pos = child.style.as_ref().map(|s| s.position).unwrap_or(mango_css::values::Position::Static);
        if matches!(pos, mango_css::values::Position::Absolute | mango_css::values::Position::Fixed) {
            continue;
        }
        match &child.box_type {
            crate::box_model::BoxType::TextNode(text) => {
                let style = child.style.as_ref().or(box_node.style.as_ref());
                let (font_size, is_bold, family_name) = if let Some(s) = style {
                    let bold = match s.font_weight {
                        mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
                        mango_css::values::FontWeight::Numeric(w) => w >= 600,
                        _ => false,
                    };
                    (s.font_size, bold, s.font_family.clone())
                } else {
                    (16.0, false, "sans-serif".to_string())
                };
                let weight = if is_bold {
                    mango_render::FontWeight::Bold
                } else {
                    mango_render::FontWeight::Regular
                };
                let family = mango_render::FontFamily::from_css_name(&family_name);
                let w = crate::inline_flow::measure_text_width_with_style(text, font_size, weight, family);
                measured = measured.max(w);
            }
            crate::box_model::BoxType::ReplacedElement { intrinsic_width, .. }
            | crate::box_model::BoxType::IFrame { intrinsic_width, .. }
            | crate::box_model::BoxType::Video { intrinsic_width, .. }
            | crate::box_model::BoxType::Audio { intrinsic_width, .. }
            | crate::box_model::BoxType::Canvas { intrinsic_width, .. } => {
                measured = measured.max(*intrinsic_width);
            }
            _ => {
                let child_w = calculate_shrink_to_fit_width(child, max_width);
                let c_style = child.style.as_ref();
                let (p_left, p_right, b_left, b_right, m_left, m_right) = if let Some(cs) = c_style {
                    let c_fs = cs.font_size;
                    let c_root_fs = cs.root_font_size;
                    (
                        cs.padding_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        cs.padding_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        cs.border_left_width,
                        cs.border_right_width,
                        cs.margin_left.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                        cs.margin_right.to_px_with_viewport(c_fs, c_root_fs, max_width, max_width),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                };
                measured = measured.max(child_w + p_left + p_right + b_left + b_right + m_left + m_right);
            }
        }
    }
    measured.min(max_width)
}

/// Sets the initial position of the block box.
fn calculate_block_position(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    current_y: f32,
    float_ctx: &FloatContext,
) {
    let clear = box_node.style.as_ref().map(|s| s.clear).unwrap_or(Clear::None);
    let cleared_y = float_ctx.apply_clearance(current_y, clear);

    let x = containing_block.content.x()
        + box_node.dimensions.margin.left
        + box_node.dimensions.border.left
        + box_node.dimensions.padding.left;

    let y = cleared_y
        + box_node.dimensions.margin.top
        + box_node.dimensions.border.top
        + box_node.dimensions.padding.top;

    box_node.dimensions.content.origin = Point::new(x, y);
}

/// Collapses two adjoining vertical margins according to CSS 2.1 §8.3.1.
///
/// - If both are positive: max(m1, m2)
/// - If both are negative: min(m1, m2) (most negative)
/// - If one is positive and one is negative: m1 + m2 (deduct negative from positive)
#[inline]
pub fn collapse_margins(m1: f32, m2: f32) -> f32 {
    if m1 >= 0.0 && m2 >= 0.0 {
        m1.max(m2)
    } else if m1 <= 0.0 && m2 <= 0.0 {
        m1.min(m2)
    } else {
        m1 + m2
    }
}

/// Bottom edge (in page coordinates) of the lowest in-flow block child, if any.
///
/// Inline children are deliberately excluded: the inline formatting context has
/// already folded them into line boxes, and text that overflows its line box —
/// tight line-heights, tall glyphs, long floats — must not stretch the block.
/// Chromium lets such text overflow, so including it here made every block with
/// tight line-height a few pixels too tall and shifted the whole page below it.
fn in_flow_block_children_bottom(box_node: &LayoutBox) -> Option<f32> {
    let parent_has_bottom_strut = box_node.dimensions.border.bottom > 0.0
        || box_node.dimensions.padding.bottom > 0.0;

    let mut bottom: Option<f32> = None;
    for child in &box_node.children {
        if child.is_inline() {
            continue;
        }
        let position = child.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
        if matches!(position, Position::Absolute | Position::Fixed) {
            continue;
        }
        let child_bottom = if parent_has_bottom_strut {
            child.dimensions.margin_box().bottom()
        } else {
            child.dimensions.border_box().bottom()
        };
        bottom = Some(bottom.map_or(child_bottom, |current: f32| current.max(child_bottom)));
    }
    bottom
}

fn has_floats_recursive(box_node: &LayoutBox) -> bool {
    box_node.children.iter().any(|c| {
        c.style.as_ref().is_some_and(|s| s.float != Float::None)
            || (c.is_inline() && has_floats_recursive(c))
    })
}

fn extract_floats_from_children(children: &mut Vec<LayoutBox>, parent_link: Option<&str>) -> Vec<LayoutBox> {
    let mut extracted_floats = Vec::new();
    let mut retained_children = Vec::new();

    for mut child in children.drain(..) {
        let is_float = child.style.as_ref().is_some_and(|s| s.float != Float::None);
        if is_float {
            if child.link_target.is_none() {
                child.link_target = parent_link.map(|s| s.to_string());
            }
            extracted_floats.push(child);
        } else if child.is_inline() && has_floats_recursive(&child) {
            let child_link = child.link_target.as_deref().or(parent_link);
            let inner_floats = extract_floats_from_children(&mut child.children, child_link);
            extracted_floats.extend(inner_floats);
            retained_children.push(child);
        } else {
            retained_children.push(child);
        }
    }

    *children = retained_children;
    extracted_floats
}

/// Lays out the children of a block box, handling margin collapsing between siblings
/// and dispatching to inline formatting context when children are inline.
fn layout_block_children(box_node: &mut LayoutBox, float_ctx: &mut FloatContext) {
    if box_node.children.is_empty() {
        return;
    }

    let has_inlines = box_node.children.iter().any(|c| c.is_inline());

    if has_inlines {
        let has_any_floats = has_floats_recursive(box_node);
        if has_any_floats {
            let container_dims = box_node.dimensions;
            let parent_link = box_node.link_target.clone();
            let mut floats = extract_floats_from_children(&mut box_node.children, parent_link.as_deref());

            let mut float_bottom = container_dims.content.y();
            for float_child in &mut floats {
                layout_single_float(float_child, &container_dims, container_dims.content.y(), float_ctx);
                float_bottom = float_bottom.max(float_child.dimensions.margin_box().bottom());
            }

            let inline_height = layout_inline_children(box_node, float_ctx);
            box_node.children.extend(floats);
            box_node.dimensions.content.size.height = inline_height.max(float_bottom - container_dims.content.y());
            return;
        }

        // Container establishes an inline formatting context
        let inline_height = layout_inline_children(box_node, float_ctx);
        box_node.dimensions.content.size.height = inline_height;
    } else {
        // Check for CSS Multi-column layout (column-count / column-width)
        let col_count_opt = box_node.style.as_ref().and_then(|s| s.column_count);
        let col_width_opt = box_node.style.as_ref().and_then(|s| s.column_width.as_ref());
        if col_count_opt.is_some() || col_width_opt.is_some() {
            let available_w = box_node.dimensions.content.width();
            let font_size = box_node.style.as_ref().map(|s| s.font_size).unwrap_or(16.0);
            let gap = box_node
                .style
                .as_ref()
                .map(|s| s.column_gap.to_px(font_size, 16.0, available_w))
                .unwrap_or(16.0);

            let resolved_cols = match (col_count_opt, col_width_opt) {
                (Some(c), Some(w_len)) => {
                    let w_px = w_len.to_px(font_size, 16.0, available_w);
                    if w_px > 0.0 {
                        let max_cols = ((available_w + gap) / (w_px + gap)).floor().max(1.0) as usize;
                        c.min(max_cols).max(1)
                    } else {
                        c.max(1)
                    }
                }
                (Some(c), None) => c.max(1),
                (None, Some(w_len)) => {
                    let w_px = w_len.to_px(font_size, 16.0, available_w);
                    if w_px > 0.0 {
                        ((available_w + gap) / (w_px + gap)).floor().max(1.0) as usize
                    } else {
                        1
                    }
                }
                (None, None) => 1,
            };

            if resolved_cols > 1 {
                if box_node.children.len() == 1 {
                    if let Some(child_style) = box_node.children[0].style.as_mut() {
                        if child_style.column_count.is_none() && child_style.column_width.is_none() {
                            child_style.column_count = Some(resolved_cols);
                            child_style.column_gap = mango_css::values::Length::Px(gap);
                        }
                    }
                } else {
                    layout_multicol_children(box_node, resolved_cols, gap, float_ctx);
                    return;
                }
            }
        }

        // Block formatting context: layout children vertically with margin collapsing
        let is_ol = box_node.tag_name.as_deref() == Some("ol");
        let mut ol_idx = 1;
        if is_ol {
            for child in &mut box_node.children {
                if child.tag_name.as_deref() == Some("li") {
                    child.attributes.push(("_mango_list_index".to_string(), ol_idx.to_string()));
                    ol_idx += 1;
                }
            }
        }

        let mut cursor_y = box_node.dimensions.content.y();
        let mut prev_margin_bottom = 0.0f32;
        let mut in_flow_count = 0;

        let container_dims = box_node.dimensions;

        for child in box_node.children.iter_mut() {
            let child_pos = child.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
            if matches!(child_pos, Position::Absolute | Position::Fixed) {
                // Absolutely/fixed positioned elements are out-of-flow; placed in second pass
                continue;
            }

            let child_float = child.style.as_ref().map(|s| s.float).unwrap_or(Float::None);
            if child_float != Float::None {
                layout_single_float(child, &container_dims, cursor_y, float_ctx);
                continue;
            }

            // Normal in-flow child: apply clearance from active floats
            let clear = child.style.as_ref().map(|s| s.clear).unwrap_or(Clear::None);
            cursor_y = float_ctx.apply_clearance(cursor_y, clear);
            if clear != Clear::None {
                prev_margin_bottom = 0.0;
            }

            // Calculate child horizontal dimensions
            calculate_block_width(child, &container_dims);

            // Margin collapsing with previous sibling (CSS 2.1 §8.3.1)
            let curr_margin_top = child.dimensions.margin.top;
            let parent_has_top_strut = container_dims.border.top > 0.0 || container_dims.padding.top > 0.0;
            if in_flow_count > 0 {
                let collapsed_margin = collapse_margins(prev_margin_bottom, curr_margin_top);
                cursor_y += collapsed_margin;
            } else if parent_has_top_strut {
                cursor_y += curr_margin_top;
            } else {
                let diff = curr_margin_top - box_node.dimensions.margin.top;
                if diff > 0.0 {
                    box_node.dimensions.margin.top = curr_margin_top;
                    box_node.dimensions.content.origin.y += diff;
                }
                cursor_y = box_node.dimensions.content.origin.y;
            }
            in_flow_count += 1;

            // Position child
            let child_x = container_dims.content.x()
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left;
            let child_y = cursor_y
                + child.dimensions.border.top
                + child.dimensions.padding.top;

            child.dimensions.content.origin = Point::new(child_x, child_y);

            // Lay out child recursively
            let child_disp = child.style.as_ref().map(|s| s.display);
            if child_disp == Some(Display::Table) || child.tag_name.as_deref() == Some("table") {
                let mut child_cb = container_dims;
                child_cb.content.origin.y = cursor_y - child.dimensions.margin.top;
                crate::table_flow::layout_table(child, &child_cb, float_ctx);
                layout_positioned_children(child, float_ctx);
            } else if matches!(child_disp, Some(Display::Flex) | Some(Display::InlineFlex)) {
                crate::flex_flow::layout_flex(child, &container_dims, float_ctx);
            } else if child_disp == Some(Display::Grid) || child_disp == Some(Display::InlineGrid) {
                crate::grid_flow::layout_grid(child, &container_dims, float_ctx);
            } else {
                if child.box_type == crate::box_model::BoxType::AnonymousBlock {
                    let avail_h = if container_dims.content.height() > 0.0 {
                        container_dims.content.height()
                    } else if let Some(ps) = &box_node.style {
                        match ps.height {
                            Length::Px(px) => px,
                            _ => match ps.max_height {
                                Length::Px(px) => px,
                                _ => 0.0,
                            },
                        }
                    } else {
                        0.0
                    };
                    if avail_h > 0.0 {
                        child.dimensions.content.size.height = avail_h;
                    }
                }
                layout_block_children(child, float_ctx);
                calculate_block_height(child, &container_dims);
                center_button_children(child);
                layout_positioned_children(child, float_ctx);
            }

            // Relative positioning offset (CSS 2.1 §9.4.3)
            if child_pos == Position::Relative {
                apply_relative_offset(child, &container_dims);
            }

            // Advance cursor
            cursor_y = child.dimensions.border_box().bottom();
            prev_margin_bottom = child.dimensions.margin.bottom;
        }

        let parent_has_bottom_strut = container_dims.border.bottom > 0.0 || container_dims.padding.bottom > 0.0;
        if !parent_has_bottom_strut && in_flow_count > 0 {
            box_node.dimensions.margin.bottom = collapse_margins(box_node.dimensions.margin.bottom, prev_margin_bottom);
        }
    }
}

/// Lays out block children in multiple balanced columns according to CSS Multi-column layout,
/// supporting column-span: all partitions, break-inside: avoid, and block fragmentation.
fn layout_multicol_children(
    box_node: &mut LayoutBox,
    num_cols: usize,
    gap: f32,
    float_ctx: &mut FloatContext,
) {
    let container_x = box_node.dimensions.content.x();
    let container_y = box_node.dimensions.content.y();
    let container_w = box_node.dimensions.content.width();
    let col_w = ((container_w - (num_cols - 1) as f32 * gap) / num_cols as f32).max(10.0);

    let is_ol = box_node.tag_name.as_deref() == Some("ol");
    let mut ol_idx = 1;
    if is_ol {
        for child in &mut box_node.children {
            if child.tag_name.as_deref() == Some("li") {
                child.attributes.push(("_mango_list_index".to_string(), ol_idx.to_string()));
                ol_idx += 1;
            }
        }
    }

    enum MulticolChunk {
        Columns(Vec<LayoutBox>),
        Spanning(LayoutBox),
    }

    let mut chunks: Vec<MulticolChunk> = Vec::new();
    let mut pending_normal: Vec<LayoutBox> = Vec::new();

    for child in box_node.children.drain(..) {
        let is_span_all = child.style.as_ref().map_or(false, |s| s.column_span == ColumnSpan::All);
        if is_span_all {
            if !pending_normal.is_empty() {
                chunks.push(MulticolChunk::Columns(std::mem::take(&mut pending_normal)));
            }
            chunks.push(MulticolChunk::Spanning(child));
        } else {
            pending_normal.push(child);
        }
    }
    if !pending_normal.is_empty() {
        chunks.push(MulticolChunk::Columns(pending_normal));
    }

    let mut result_children = Vec::new();
    let mut current_y = container_y;

    for chunk in chunks {
        match chunk {
            MulticolChunk::Columns(mut items) => {
                let col_cb = Dimensions {
                    content: Rect::new(container_x, current_y, col_w, 0.0),
                    padding: EdgeSizes::ZERO,
                    border: EdgeSizes::ZERO,
                    margin: EdgeSizes::ZERO,
                };

                let mut unfragmented_total_h = 0.0f32;
                for it in &mut items {
                    let child_pos = it.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
                    if !matches!(child_pos, Position::Absolute | Position::Fixed) {
                        calculate_block_width(it, &col_cb);
                        layout_block_children(it, float_ctx);
                        calculate_block_height(it, &col_cb);
                        unfragmented_total_h += it.dimensions.margin_box().height();
                    }
                }

                let target_col_h = (unfragmented_total_h / num_cols as f32).max(20.0);

                // Fragment splittable blocks across column boundary
                let mut fragmented_items = Vec::with_capacity(items.len() * 2);
                for mut item in items.into_iter() {
                    let break_inside = item.style.as_ref().map_or(BreakInside::Auto, |s| s.break_inside);
                    let can_fragment = break_inside == BreakInside::Auto
                        && item.children.len() > 1
                        && item.dimensions.margin_box().height() > target_col_h * 0.8;

                    if can_fragment {
                        let mut sub_children = std::mem::take(&mut item.children);
                        let mid = (sub_children.len() + 1) / 2;
                        let second_half = sub_children.split_off(mid);

                        let mut frag1 = item.clone();
                        frag1.children = sub_children;
                        calculate_block_width(&mut frag1, &col_cb);
                        layout_block_children(&mut frag1, float_ctx);
                        calculate_block_height(&mut frag1, &col_cb);
                        fragmented_items.push(frag1);

                        let mut frag2 = item;
                        frag2.children = second_half;
                        calculate_block_width(&mut frag2, &col_cb);
                        layout_block_children(&mut frag2, float_ctx);
                        calculate_block_height(&mut frag2, &col_cb);
                        fragmented_items.push(frag2);
                    } else {
                        fragmented_items.push(item);
                    }
                }

                let chunk_h = layout_multicol_segment(
                    &mut fragmented_items,
                    container_x,
                    current_y,
                    col_w,
                    gap,
                    num_cols,
                    target_col_h,
                    float_ctx,
                );

                result_children.extend(fragmented_items);
                current_y += chunk_h;
            }
            MulticolChunk::Spanning(mut span_child) => {
                let span_cb = Dimensions {
                    content: Rect::new(container_x, current_y, container_w, 0.0),
                    padding: EdgeSizes::ZERO,
                    border: EdgeSizes::ZERO,
                    margin: EdgeSizes::ZERO,
                };
                calculate_block_width(&mut span_child, &span_cb);
                layout_block_children(&mut span_child, float_ctx);
                calculate_block_height(&mut span_child, &span_cb);
                center_button_children(&mut span_child);
                layout_positioned_children(&mut span_child, float_ctx);

                let new_x = container_x + span_child.dimensions.margin.left;
                let new_y = current_y + span_child.dimensions.margin.top;
                let dx = new_x - span_child.dimensions.content.x();
                let dy = new_y - span_child.dimensions.content.y();
                span_child.dimensions.content.origin = Point::new(new_x, new_y);
                shift_descendants(&mut span_child, dx, dy);

                current_y += span_child.dimensions.margin_box().height();
                result_children.push(span_child);
            }
        }
    }

    let total_h = (current_y - container_y).max(0.0);
    box_node.children = result_children;
    box_node.dimensions.content.size.height = total_h;
}

fn layout_multicol_segment(
    children: &mut [LayoutBox],
    container_x: f32,
    start_y: f32,
    col_w: f32,
    gap: f32,
    num_cols: usize,
    target_col_h: f32,
    float_ctx: &mut FloatContext,
) -> f32 {
    let col_cb = Dimensions {
        content: Rect::new(container_x, start_y, col_w, 0.0),
        padding: EdgeSizes::ZERO,
        border: EdgeSizes::ZERO,
        margin: EdgeSizes::ZERO,
    };

    let mut cur_col = 0;
    let mut cur_col_y = 0.0f32;
    let mut col_heights = vec![0.0f32; num_cols];

    for child in children.iter_mut() {
        let child_pos = child.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
        if matches!(child_pos, Position::Absolute | Position::Fixed) {
            continue;
        }

        let child_display = child.style.as_ref().map(|s| s.display);
        if matches!(child_display, Some(Display::Flex) | Some(Display::InlineFlex)) {
            crate::flex_flow::layout_flex(child, &col_cb, float_ctx);
        } else if child_display == Some(Display::Grid) || child_display == Some(Display::InlineGrid) {
            crate::grid_flow::layout_grid(child, &col_cb, float_ctx);
        } else if child_display == Some(Display::Table) || child.tag_name.as_deref() == Some("table") {
            crate::table_flow::layout_table(child, &col_cb, float_ctx);
        } else {
            calculate_block_width(child, &col_cb);
            layout_block_children(child, float_ctx);
            calculate_block_height(child, &col_cb);
            center_button_children(child);
            layout_positioned_children(child, float_ctx);
        }

        let h = child.dimensions.margin_box().height();
        let break_inside = child.style.as_ref().map_or(BreakInside::Auto, |s| s.break_inside);
        let avoids_break = matches!(
            break_inside,
            BreakInside::Avoid | BreakInside::AvoidColumn | BreakInside::AvoidPage
        );

        if cur_col < num_cols - 1 && cur_col_y > 0.0 {
            if avoids_break {
                if cur_col_y + h > target_col_h {
                    cur_col += 1;
                    cur_col_y = 0.0;
                }
            } else if cur_col_y + h * 0.5 > target_col_h {
                cur_col += 1;
                cur_col_y = 0.0;
            }
        }

        let new_x = container_x + cur_col as f32 * (col_w + gap) + child.dimensions.margin.left;
        let new_y = start_y + cur_col_y + child.dimensions.margin.top;

        let dx = new_x - child.dimensions.content.x();
        let dy = new_y - child.dimensions.content.y();
        child.dimensions.content.origin = Point::new(new_x, new_y);
        shift_descendants(child, dx, dy);

        cur_col_y += h;
        col_heights[cur_col] = cur_col_y;
    }

    col_heights.into_iter().fold(0.0f32, f32::max)
}

/// Recursively lays out the contents of a block box without re-calculating its outer dimensions or position.
pub fn layout_block_contents(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    layout_block_children(box_node, float_ctx);
    calculate_block_height(box_node, containing_block);
    center_button_children(box_node);
    layout_positioned_children(box_node, float_ctx);
}

/// Lays out a floated element, positions it against the active FloatContext,
/// shifts its descendants, and registers its margin box into the FloatContext.
fn layout_single_float(
    child: &mut LayoutBox,
    container_dims: &Dimensions,
    cursor_y: f32,
    float_ctx: &mut FloatContext,
) {
    let float_type = child.style.as_ref().map(|s| s.float).unwrap_or(Float::None);
    let clear = child.style.as_ref().map(|s| s.clear).unwrap_or(Clear::None);
    let float_y = float_ctx.apply_clearance(cursor_y, clear);

    // Calculate child horizontal dimensions (shrink-to-fit or explicit width)
    calculate_block_width(child, container_dims);

    let margin_box_w = child.dimensions.margin_box().width();
    let margin_box_h = child.dimensions.margin_box().height();

    let child_y = float_y
        + child.dimensions.margin.top
        + child.dimensions.border.top
        + child.dimensions.padding.top;

    let child_x = match float_type {
        Float::Left => {
            let (min_x, _max_x) = float_ctx.available_span(
                float_y,
                margin_box_h,
                container_dims.content.x(),
                container_dims.content.width(),
            );
            min_x
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
        Float::Right => {
            let (_min_x, max_x) = float_ctx.available_span(
                float_y,
                margin_box_h,
                container_dims.content.x(),
                container_dims.content.width(),
            );
            max_x - margin_box_w
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
        Float::None => {
            container_dims.content.x()
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
    };

    child.dimensions.content.origin = Point::new(child_x, child_y);

    // Floats establish an independent Block Formatting Context (BFC, CSS 2.1 §9.5).
    // Internal contents must not interact with or wrap around floats outside this BFC.
    let mut inner_float_ctx = FloatContext::new();

    // Lay out contents
    let child_disp = child.style.as_ref().map(|s| s.display);
    if child_disp == Some(Display::Table) || child.tag_name.as_deref() == Some("table") {
        crate::table_flow::layout_table(child, container_dims, &mut inner_float_ctx);
        layout_positioned_children(child, &mut inner_float_ctx);
    } else if matches!(child_disp, Some(Display::Flex) | Some(Display::InlineFlex)) {
        crate::flex_flow::layout_flex(child, container_dims, &mut inner_float_ctx);
    } else if child_disp == Some(Display::Grid) || child_disp == Some(Display::InlineGrid) {
        crate::grid_flow::layout_grid(child, container_dims, &mut inner_float_ctx);
    } else {
        layout_block_children(child, &mut inner_float_ctx);
        calculate_block_height(child, container_dims);
        center_button_children(child);
        layout_positioned_children(child, &mut inner_float_ctx);
    }

    // If final height changed the available horizontal span, adjust child_x
    let final_margin_box_h = child.dimensions.margin_box().height();
    let final_child_x = match float_type {
        Float::Left => {
            let (min_x, _max_x) = float_ctx.available_span(
                float_y,
                final_margin_box_h,
                container_dims.content.x(),
                container_dims.content.width(),
            );
            min_x
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
        Float::Right => {
            let (_min_x, max_x) = float_ctx.available_span(
                float_y,
                final_margin_box_h,
                container_dims.content.x(),
                container_dims.content.width(),
            );
            max_x - margin_box_w
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left
        }
        Float::None => child_x,
    };

    if (final_child_x - child_x).abs() > 0.001 {
        let dx = final_child_x - child_x;
        child.dimensions.content.origin.x = final_child_x;
        shift_descendants(child, dx, 0.0);
    }

    // Register into outer float_ctx
    match float_type {
        Float::Left => float_ctx.add_left_float(child.dimensions.margin_box()),
        Float::Right => float_ctx.add_right_float(child.dimensions.margin_box()),
        Float::None => {}
    }
}


/// Positions out-of-flow absolute and fixed children against the containing box's padding box.
pub fn layout_positioned_children(box_node: &mut LayoutBox, float_ctx: &mut FloatContext) {
    let container_dims = box_node.dimensions;
    for child in box_node.children.iter_mut() {
        let child_pos = child.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
        if matches!(child_pos, Position::Absolute | Position::Fixed) {
            layout_positioned_element(child, &container_dims, float_ctx);
        }
    }
}

/// Applies relative positioning offsets (`top`, `bottom`, `left`, `right`)
/// to a box without altering surrounding in-flow layout geometry.
fn apply_relative_offset(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    if let Some(style) = &box_node.style {
        let font_size = style.font_size;
        let cw = containing_block.content.width();
        let ch = if containing_block.content.height() > 0.0 {
            containing_block.content.height()
        } else {
            600.0
        };

        let dx = if style.left != Length::Auto {
            style.left.to_px_with_viewport(font_size, 16.0, cw, ch)
        } else if style.right != Length::Auto {
            -style.right.to_px_with_viewport(font_size, 16.0, cw, ch)
        } else {
            0.0
        };

        let dy = if style.top != Length::Auto {
            style.top.to_px_with_viewport(font_size, 16.0, ch, ch)
        } else if style.bottom != Length::Auto {
            -style.bottom.to_px_with_viewport(font_size, 16.0, ch, ch)
        } else {
            0.0
        };

        box_node.dimensions.content.origin.x += dx;
        box_node.dimensions.content.origin.y += dy;
        shift_descendants(box_node, dx, dy);
    }
}

/// Positions an out-of-flow absolute or fixed element against its containing block.
fn layout_positioned_element(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    let style = box_node.style.clone().unwrap_or_default();
    let font_size = style.font_size;
    let (vp_w, vp_h) = mango_css::values::get_current_viewport();
    let fixed_cb;
    let cb = if style.position == Position::Fixed {
        fixed_cb = Dimensions::new(mango_core::Rect::new(0.0, 0.0, vp_w, vp_h));
        &fixed_cb
    } else {
        containing_block
    };

    calculate_block_width(box_node, cb);
    if style.display == Display::Table || box_node.tag_name.as_deref() == Some("table") {
        crate::table_flow::layout_table(box_node, cb, float_ctx);
    } else if matches!(style.display, Display::Flex | Display::InlineFlex) {
        crate::flex_flow::layout_flex(box_node, cb, float_ctx);
    } else if style.display == Display::Grid || style.display == Display::InlineGrid {
        crate::grid_flow::layout_grid(box_node, cb, float_ctx);
    } else {
        layout_block_children(box_node, float_ctx);
        calculate_block_height(box_node, cb);
        center_button_children(box_node);
    }

    let pad_box = if style.position == Position::Fixed {
        mango_core::Rect::new(0.0, 0.0, vp_w, vp_h)
    } else {
        containing_block.padding_box()
    };

    let pad_w = pad_box.width();
    let pad_h = pad_box.height();

    // 1. Horizontal positioning (CSS 2.1 § 10.3.7)
    let is_left_auto = style.left == Length::Auto;
    let is_right_auto = style.right == Length::Auto;
    let is_margin_l_auto = style.margin_left == Length::Auto;
    let is_margin_r_auto = style.margin_right == Length::Auto;

    let margin_l = if is_margin_l_auto {
        0.0
    } else {
        style.margin_left.to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
    };
    let margin_r = if is_margin_r_auto {
        0.0
    } else {
        style.margin_right.to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
    };

    // If both left and right are specified and width is auto, width expands to fill the span
    if !is_left_auto && !is_right_auto && style.width == Length::Auto {
        let non_content_h = box_node.dimensions.padding.left
            + box_node.dimensions.padding.right
            + box_node.dimensions.border.left
            + box_node.dimensions.border.right;
        let left_px = style.left.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
        let right_px = style.right.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
        let tentative_w = (pad_w - left_px - right_px - margin_l - margin_r - non_content_h).max(0.0);
        let mut clamped_w = tentative_w;
        if style.max_width != Length::Auto {
            let max_w = style.max_width.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
            let max_content_w = if style.box_sizing == BoxSizing::BorderBox {
                (max_w - non_content_h).max(0.0)
            } else {
                max_w
            };
            clamped_w = clamped_w.min(max_content_w);
        }
        if style.min_width != Length::Auto {
            let min_w = style.min_width.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
            let min_content_w = if style.box_sizing == BoxSizing::BorderBox {
                (min_w - non_content_h).max(0.0)
            } else {
                min_w
            };
            clamped_w = clamped_w.max(min_content_w);
        }
        box_node.dimensions.content.size.width = clamped_w;
    }

    // 2. Vertical positioning (CSS 2.1 § 10.6.4)
    let is_top_auto = style.top == Length::Auto;
    let is_bottom_auto = style.bottom == Length::Auto;
    let is_margin_t_auto = style.margin_top == Length::Auto;
    let is_margin_b_auto = style.margin_bottom == Length::Auto;

    let margin_t = if is_margin_t_auto {
        0.0
    } else {
        style.margin_top.to_px_with_viewport(font_size, 16.0, pad_h, pad_h)
    };
    let margin_b = if is_margin_b_auto {
        0.0
    } else {
        style.margin_bottom.to_px_with_viewport(font_size, 16.0, pad_h, pad_h)
    };

    // If both top and bottom are specified and height is auto, height expands to fill the span
    if !is_top_auto && !is_bottom_auto && style.height == Length::Auto {
        let non_content_v = box_node.dimensions.padding.top
            + box_node.dimensions.padding.bottom
            + box_node.dimensions.border.top
            + box_node.dimensions.border.bottom;
        let top_px = style.top.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
        let bottom_px = style.bottom.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
        let tentative_h = (pad_h - top_px - bottom_px - margin_t - margin_b - non_content_v).max(0.0);
        let mut clamped_h = tentative_h;
        if style.max_height != Length::Auto {
            let max_h = style.max_height.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
            let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
                (max_h - non_content_v).max(0.0)
            } else {
                max_h
            };
            clamped_h = clamped_h.min(max_content_h);
        }
        if style.min_height != Length::Auto {
            let min_h = style.min_height.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
            let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
                (min_h - non_content_v).max(0.0)
            } else {
                min_h
            };
            clamped_h = clamped_h.max(min_content_h);
        }
        box_node.dimensions.content.size.height = clamped_h;
    }

    let border_box_w = box_node.dimensions.border_box().width();
    let border_box_h = box_node.dimensions.border_box().height();

    let border_box_x = if !is_left_auto && !is_right_auto {
        let left_px = style.left.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
        let right_px = style.right.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
        if is_margin_l_auto && is_margin_r_auto {
            let free_space = pad_w - left_px - right_px - border_box_w;
            let m = (free_space / 2.0).max(0.0);
            box_node.dimensions.margin.left = m;
            box_node.dimensions.margin.right = m;
            pad_box.x() + left_px + m
        } else if is_margin_l_auto {
            let m = (pad_w - left_px - right_px - border_box_w - margin_r).max(0.0);
            box_node.dimensions.margin.left = m;
            box_node.dimensions.margin.right = margin_r;
            pad_box.x() + left_px + m
        } else if is_margin_r_auto {
            box_node.dimensions.margin.left = margin_l;
            box_node.dimensions.margin.right = (pad_w - left_px - right_px - border_box_w - margin_l).max(0.0);
            pad_box.x() + left_px + margin_l
        } else {
            box_node.dimensions.margin.left = margin_l;
            box_node.dimensions.margin.right = margin_r;
            pad_box.x() + left_px + margin_l
        }
    } else if !is_left_auto {
        let left_px = style.left.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
        box_node.dimensions.margin.left = margin_l;
        box_node.dimensions.margin.right = margin_r;
        pad_box.x() + left_px + margin_l
    } else if !is_right_auto {
        let right_px = style.right.to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
        box_node.dimensions.margin.left = margin_l;
        box_node.dimensions.margin.right = margin_r;
        pad_box.right() - right_px - margin_r - border_box_w
    } else {
        box_node.dimensions.margin.left = margin_l;
        box_node.dimensions.margin.right = margin_r;
        // Static position: hypothetical in-flow position inside containing block's content box (CSS 2.1 § 10.3.7)
        containing_block.content.x() + margin_l
    };

    // 2. Vertical positioning (CSS 2.1 § 10.6.4)
    let is_top_auto = style.top == Length::Auto;
    let is_bottom_auto = style.bottom == Length::Auto;
    let is_margin_t_auto = style.margin_top == Length::Auto;
    let is_margin_b_auto = style.margin_bottom == Length::Auto;

    let margin_t = if is_margin_t_auto {
        0.0
    } else {
        style.margin_top.to_px_with_viewport(font_size, 16.0, pad_h, pad_h)
    };
    let margin_b = if is_margin_b_auto {
        0.0
    } else {
        style.margin_bottom.to_px_with_viewport(font_size, 16.0, pad_h, pad_h)
    };

    let border_box_y = if !is_top_auto && !is_bottom_auto {
        let top_px = style.top.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
        let bottom_px = style.bottom.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
        if is_margin_t_auto && is_margin_b_auto {
            let free_space = pad_h - top_px - bottom_px - border_box_h;
            let m = (free_space / 2.0).max(0.0);
            box_node.dimensions.margin.top = m;
            box_node.dimensions.margin.bottom = m;
            pad_box.y() + top_px + m
        } else if is_margin_t_auto {
            let m = (pad_h - top_px - bottom_px - border_box_h - margin_b).max(0.0);
            box_node.dimensions.margin.top = m;
            box_node.dimensions.margin.bottom = margin_b;
            pad_box.y() + top_px + m
        } else if is_margin_b_auto {
            box_node.dimensions.margin.top = margin_t;
            box_node.dimensions.margin.bottom = (pad_h - top_px - bottom_px - border_box_h - margin_t).max(0.0);
            pad_box.y() + top_px + margin_t
        } else {
            box_node.dimensions.margin.top = margin_t;
            box_node.dimensions.margin.bottom = margin_b;
            pad_box.y() + top_px + margin_t
        }
    } else if !is_top_auto {
        let top_px = style.top.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
        box_node.dimensions.margin.top = margin_t;
        box_node.dimensions.margin.bottom = margin_b;
        pad_box.y() + top_px + margin_t
    } else if !is_bottom_auto {
        let bottom_px = style.bottom.to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
        box_node.dimensions.margin.top = margin_t;
        box_node.dimensions.margin.bottom = margin_b;
        pad_box.bottom() - bottom_px - margin_b - border_box_h
    } else {
        box_node.dimensions.margin.top = margin_t;
        box_node.dimensions.margin.bottom = margin_b;
        // Static position: hypothetical in-flow position inside containing block's content box (CSS 2.1 § 10.6.4)
        containing_block.content.y() + margin_t
    };

    // Calculate content box origin from border box origin
    let content_x = border_box_x + box_node.dimensions.border.left + box_node.dimensions.padding.left;
    let content_y = border_box_y + box_node.dimensions.border.top + box_node.dimensions.padding.top;

    let dx = content_x - box_node.dimensions.content.x();
    let dy = content_y - box_node.dimensions.content.y();
    box_node.dimensions.content.origin = Point::new(content_x, content_y);
    shift_descendants(box_node, dx, dy);

    // Recursively position out-of-flow positioned children inside this positioned box
    layout_positioned_children(box_node, float_ctx);
}

pub(crate) fn shift_descendants(box_node: &mut LayoutBox, dx: f32, dy: f32) {
    for child in &mut box_node.children {
        let child_pos = child.style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
        if child_pos == Position::Fixed {
            continue;
        }
        child.dimensions.content.origin.x += dx;
        child.dimensions.content.origin.y += dy;
        shift_descendants(child, dx, dy);
    }
}

pub(crate) fn center_button_children(box_node: &mut LayoutBox) {
    if box_node.tag_name.as_deref() != Some("button") || box_node.children.is_empty() {
        return;
    }
    let button_h = box_node.dimensions.content.height();
    if button_h <= 0.0 {
        return;
    }
    let content_top = box_node.dimensions.content.y();
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for child in &box_node.children {
        let b = child.dimensions.margin_box();
        min_y = min_y.min(b.y());
        max_y = max_y.max(b.bottom());
    }
    if min_y < max_y && (max_y - min_y) < button_h {
        let children_h = max_y - min_y;
        let target_y = content_top + (button_h - children_h) / 2.0;
        let delta_y = target_y - min_y;
        if delta_y.abs() > 0.5 {
            for child in &mut box_node.children {
                child.dimensions.content.origin.y += delta_y;
                shift_descendants(child, 0.0, delta_y);
            }
        }
    }
}

/// Computes the final height of a block box.
fn calculate_block_height(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    let style = box_node.style.clone().unwrap_or_default();
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    let container_h = containing_block.content.height().max(0.0);
    let container_w = containing_block.content.width();
    let non_content_v = box_node.dimensions.padding.vertical() + box_node.dimensions.border.vertical();

    // 1. Determine tentative content height
    if let crate::box_model::BoxType::ReplacedElement {
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
    | crate::box_model::BoxType::Audio {
        intrinsic_width,
        intrinsic_height,
        ..
    }
    | crate::box_model::BoxType::Canvas {
        intrinsic_width,
        intrinsic_height,
        ..
    } = &box_node.box_type
    {
        if box_node.dimensions.content.size.width <= 0.0 {
            box_node.dimensions.content.size.width = *intrinsic_width;
        }
        if style.height != Length::Auto {
            let is_percent_indefinite = matches!(style.height, Length::Percent(_))
                && containing_block.content.height() <= 0.0;
            if !is_percent_indefinite {
                let h = style.height.to_px_with_viewport(font_size, root_font_size, container_h, container_h);
                box_node.dimensions.content.size.height = h;
            } else if *intrinsic_width > 0.0 && box_node.dimensions.content.size.width > 0.0 {
                let ratio = if let Some(r) = style.aspect_ratio && r > 0.0 {
                    r
                } else {
                    *intrinsic_width / *intrinsic_height
                };
                box_node.dimensions.content.size.height = (box_node.dimensions.content.size.width / ratio).max(1.0);
            } else {
                box_node.dimensions.content.size.height = *intrinsic_height;
            }
        } else if *intrinsic_width > 0.0 && box_node.dimensions.content.size.width > 0.0 {
            let ratio = if let Some(r) = style.aspect_ratio && r > 0.0 {
                r
            } else {
                *intrinsic_width / *intrinsic_height
            };
            box_node.dimensions.content.size.height = (box_node.dimensions.content.size.width / ratio).max(1.0);
        } else {
            box_node.dimensions.content.size.height = *intrinsic_height;
        }
    } else if style.height != Length::Auto {
        let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
        let eff_container_h = if containing_block.content.height() > 0.0 {
            containing_block.content.height()
        } else if is_pos {
            600.0
        } else {
            0.0
        };
        let h = style.height.to_px_with_viewport(font_size, root_font_size, eff_container_h, 600.0);
        if h > 0.0 || !matches!(style.height, Length::Percent(_)) {
            if style.box_sizing == BoxSizing::BorderBox {
                box_node.dimensions.content.size.height = (h - non_content_v).max(0.0);
            } else {
                box_node.dimensions.content.size.height = h.max(0.0);
            }
        }
    } else if let Some(tag) = box_node.tag_name.as_deref() {
        match tag {
            "input" => {
                let input_type = box_node.get_attribute("type").unwrap_or("text");
                if input_type == "checkbox" || input_type == "radio" {
                    box_node.dimensions.content.size.height = 16.0;
                } else {
                    box_node.dimensions.content.size.height = 24.0;
                }
            }
            "button" => {
                if box_node.dimensions.content.size.height < 26.0 {
                    box_node.dimensions.content.size.height = 26.0;
                }
            }
            "select" => {
                let is_multiple = box_node.get_attribute("_mango_is_multiple") == Some("true");
                let size_attr = box_node
                    .get_attribute("size")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(if is_multiple { 4 } else { 1 });
                if is_multiple || size_attr > 1 {
                    box_node.dimensions.content.size.height = size_attr.max(3) as f32 * 18.0 + 8.0;
                } else {
                    box_node.dimensions.content.size.height = 24.0;
                }
            }
            "textarea" => {
                box_node.dimensions.content.size.height = 64.0;
            }
            _ => {
                if let Some(bottom) = in_flow_block_children_bottom(box_node) {
                    box_node.dimensions.content.size.height =
                        box_node.dimensions.content.size.height.max(bottom - box_node.dimensions.content.y());
                }
            }
        }
    } else if let Some(bottom) = in_flow_block_children_bottom(box_node) {
        // Auto height from in-flow *block* children.
        box_node.dimensions.content.size.height =
            box_node.dimensions.content.size.height.max(bottom - box_node.dimensions.content.y());
    }

    // 1b. Apply aspect-ratio if height is auto and aspect_ratio is defined
    if style.height == Length::Auto {
        if let Some(ratio) = style.aspect_ratio {
            if ratio > 0.0 && box_node.dimensions.content.size.width > 0.0 {
                box_node.dimensions.content.size.height = (box_node.dimensions.content.size.width / ratio).max(1.0);
            }
        }
    }

    // 2. Apply max-height and min-height clamping (CSS 2.1 § 10.7)
    if style.max_height != Length::Auto {
        let raw_max = style.max_height.to_px_with_viewport(font_size, root_font_size, container_h, container_h);
        let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - non_content_v).max(0.0)
        } else {
            raw_max
        };
        box_node.dimensions.content.size.height =
            box_node.dimensions.content.size.height.min(max_content_h);
    }

    if style.min_height != Length::Auto {
        let raw_min = style.min_height.to_px_with_viewport(font_size, root_font_size, container_h, container_h);
        let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (raw_min - non_content_v).max(0.0)
        } else {
            raw_min
        };
        box_node.dimensions.content.size.height =
            box_node.dimensions.content.size.height.max(min_content_h);
    }

    // 3. CSS 2.1 §10.4: Constraint resolution for replaced elements with an
    //    intrinsic ratio and 'width' computed as 'auto'.
    //    After max-height / min-height clamping may have changed the height,
    //    re-derive width from the final height to preserve the aspect ratio.
    if style.width == Length::Auto {
        if let crate::box_model::BoxType::ReplacedElement { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::IFrame { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::Video { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::Audio { intrinsic_width, intrinsic_height, .. }
            | crate::box_model::BoxType::Canvas { intrinsic_width, intrinsic_height, .. } = &box_node.box_type
        {
            if *intrinsic_width > 0.0 && *intrinsic_height > 0.0 {
                let ratio = if let Some(r) = style.aspect_ratio && r > 0.0 {
                    r
                } else {
                    *intrinsic_width / *intrinsic_height
                };
                let ratio_w = box_node.dimensions.content.size.height * ratio;
                let mut final_w = ratio_w;
                if style.max_width != Length::Auto {
                    let raw_max = style.max_width.to_px_with_viewport(font_size, root_font_size, container_w, container_h);
                    final_w = final_w.min(raw_max);
                }
                if style.min_width != Length::Auto {
                    let raw_min = style.min_width.to_px_with_viewport(font_size, root_font_size, container_w, container_h);
                    final_w = final_w.max(raw_min);
                }
                box_node.dimensions.content.size.width = final_w;
            }
        }
    }
    box_node.dimensions.content.size.height = box_node.dimensions.content.size.height.max(0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Rect;
    use mango_css::computed::ComputedStyle;
    use mango_css::values::Length;

    #[test]
    fn test_block_width_calculation_auto() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

        let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        style.margin_left = Length::Px(10.0);
        style.margin_right = Length::Px(20.0);
        style.padding_left = Length::Px(5.0);
        style.padding_right = Length::Px(15.0);
        child.style = Some(style);
        root.children.push(child);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let child_box = &root.children[0];
        // 500 - (10 + 20 + 5 + 15) = 450
        assert_eq!(child_box.dimensions.content.width(), 450.0);
    }

    #[test]
    fn test_block_margin_auto_centering() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 600.0, 500.0);

        let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        style.width = Length::Px(200.0);
        style.margin_left = Length::Auto;
        style.margin_right = Length::Auto;
        child.style = Some(style);
        root.children.push(child);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let child_box = &root.children[0];
        // (600 - 200) / 2 = 200 margin each
        assert_eq!(child_box.dimensions.margin.left, 200.0);
        assert_eq!(child_box.dimensions.margin.right, 200.0);
        assert_eq!(child_box.dimensions.content.x(), 200.0);
    }

    #[test]
    fn test_block_max_width_margin_auto_centering() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 1000.0, 500.0);

        let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        // Modern web container pattern: width auto (or 100%), max-width 600px, margin: 0 auto
        style.width = Length::Auto;
        style.max_width = Length::Px(600.0);
        style.margin_left = Length::Auto;
        style.margin_right = Length::Auto;
        child.style = Some(style);
        root.children.push(child);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let child_box = &root.children[0];
        assert_eq!(child_box.dimensions.content.width(), 600.0);
        // (1000 - 600) / 2 = 200 margin each
        assert_eq!(child_box.dimensions.margin.left, 200.0);
        assert_eq!(child_box.dimensions.margin.right, 200.0);
        assert_eq!(child_box.dimensions.content.x(), 200.0);
    }

    #[test]
    fn test_block_min_max_height_clamping() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

        // Child 1: content height is 0, but min-height is 150px
        let mut child1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s1 = ComputedStyle::default();
        s1.min_height = Length::Px(150.0);
        child1.style = Some(s1);

        // Child 2: explicit height is 500px, but max-height is 200px
        let mut child2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s2 = ComputedStyle::default();
        s2.height = Length::Px(500.0);
        s2.max_height = Length::Px(200.0);
        child2.style = Some(s2);

        root.children.push(child1);
        root.children.push(child2);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        assert_eq!(root.children[0].dimensions.content.height(), 150.0);
        assert_eq!(root.children[1].dimensions.content.height(), 200.0);
    }

    #[test]
    fn test_margin_collapsing() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

        let mut b1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s1 = ComputedStyle::default();
        s1.height = Length::Px(50.0);
        s1.margin_bottom = Length::Px(30.0);
        b1.style = Some(s1);

        let mut b2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s2 = ComputedStyle::default();
        s2.height = Length::Px(50.0);
        s2.margin_top = Length::Px(20.0); // 30.max(20) = 30 collapsed gap
        b2.style = Some(s2);

        root.children.push(b1);
        root.children.push(b2);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let box1 = &root.children[0];
        let box2 = &root.children[1];

        // Box 1 bottom border box is at 0 + 50 = 50.
        // Box 2 top border box is at 50 + 30 = 80!
        assert_eq!(box1.dimensions.border_box().bottom(), 50.0);
        assert_eq!(box2.dimensions.border_box().y(), 80.0);
    }

    #[test]
    fn test_negative_and_mixed_margin_collapsing() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

        // Child 1: height 50, margin-bottom: -20
        let mut b1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s1 = ComputedStyle::default();
        s1.height = Length::Px(50.0);
        s1.margin_bottom = Length::Px(-20.0);
        b1.style = Some(s1);

        // Child 2: height 50, margin-top: 30
        // Mixed: 30 + (-20) = 10 collapsed margin
        let mut b2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s2 = ComputedStyle::default();
        s2.height = Length::Px(50.0);
        s2.margin_top = Length::Px(30.0);
        s2.margin_bottom = Length::Px(-15.0);
        b2.style = Some(s2);

        // Child 3: height 50, margin-top: -25
        // Both negative: min(-15, -25) = -25 collapsed margin
        let mut b3 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut s3 = ComputedStyle::default();
        s3.height = Length::Px(50.0);
        s3.margin_top = Length::Px(-25.0);
        b3.style = Some(s3);

        root.children.push(b1);
        root.children.push(b2);
        root.children.push(b3);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let box1 = &root.children[0];
        let box2 = &root.children[1];
        let box3 = &root.children[2];

        // Box 1: top = 0, bottom = 50
        assert_eq!(box1.dimensions.border_box().bottom(), 50.0);
        // Box 2: y = 50 + 10 = 60, bottom = 60 + 50 = 110
        assert_eq!(box2.dimensions.border_box().y(), 60.0);
        assert_eq!(box2.dimensions.border_box().bottom(), 110.0);
        // Box 3: y = 110 + (-25) = 85
        assert_eq!(box3.dimensions.border_box().y(), 85.0);
    }

    #[test]
    fn test_parent_child_margin_collapsing() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);

        // Parent has no border and no padding
        let mut parent = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut p_style = ComputedStyle::default();
        p_style.height = Length::Auto;
        p_style.margin_top = Length::Px(10.0);
        p_style.margin_bottom = Length::Px(10.0);
        parent.style = Some(p_style);

        // Child has height 100, margin-top: 40, margin-bottom: 50
        let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut c_style = ComputedStyle::default();
        c_style.height = Length::Px(100.0);
        c_style.margin_top = Length::Px(40.0);
        c_style.margin_bottom = Length::Px(50.0);
        child.style = Some(c_style);

        parent.children.push(child);
        root.children.push(parent);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let parent_box = &root.children[0];
        // Parent's top margin collapsed with child's: max(10, 40) = 40
        assert_eq!(parent_box.dimensions.margin.top, 40.0);
        // Parent's content height should be 100.0 (child's border box, not stretched by child's margin)
        assert_eq!(parent_box.dimensions.content.height(), 100.0);
        // Parent's bottom margin collapsed with child's: max(10, 50) = 50
        assert_eq!(parent_box.dimensions.margin.bottom, 50.0);
    }

    #[test]
    fn test_float_right_contour_text_wrapping() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 600.0);

        // Floated right box (e.g. Wikipedia infobox / thumbnail)
        let mut float_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut float_style = ComputedStyle::default();
        float_style.float = mango_css::values::Float::Right;
        float_style.width = Length::Px(200.0);
        float_style.height = Length::Px(80.0);
        float_box.style = Some(float_style);

        // Paragraph containing text
        let mut p_box = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let p_style = ComputedStyle::default();
        p_box.style = Some(p_style);

        let text_child = LayoutBox::new(
            crate::box_model::BoxType::TextNode(
                "Wikipedia is a free online encyclopedia written and maintained by a community of volunteers."
                    .to_string(),
            ),
            Some(ComputedStyle::default()),
        );
        p_box.children.push(text_child);

        root.children.push(float_box);
        root.children.push(p_box);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        let floated = &root.children[0];
        let p = &root.children[1];
        assert_eq!(floated.dimensions.content.x(), 300.0);
        assert_eq!(floated.dimensions.content.y(), 0.0);
        assert_eq!(floated.dimensions.content.width(), 200.0);

        // The paragraph starts at y = 0 (beside the float, not below it!)
        assert_eq!(p.dimensions.content.y(), 0.0);

        // None of the text lines inside the paragraph exceed the left edge of the float (300.0)
        for line_box in &p.children {
            if line_box.dimensions.content.y() < 80.0 {
                assert!(
                    line_box.dimensions.content.right() <= 300.5,
                    "Line box at y={} right={} exceeded float left boundary 300.0",
                    line_box.dimensions.content.y(),
                    line_box.dimensions.content.right()
                );
            }
        }
    }

    #[test]
    fn test_multicolumn_layout_balances_columns() {
        let mut root = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 500.0, 600.0);

        let mut multi_style = ComputedStyle::default();
        multi_style.column_count = Some(2);
        multi_style.column_gap = Length::Px(20.0);
        root.style = Some(multi_style);

        // 4 child items of height 40px each
        for i in 0..4 {
            let mut child = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
            let mut child_style = ComputedStyle::default();
            child_style.height = Length::Px(40.0);
            child.style = Some(child_style);
            child.tag_name = Some(format!("item-{}", i));
            root.children.push(child);
        }

        let mut float_ctx = FloatContext::new();
        layout_block(&mut root, &containing_block, &mut float_ctx);

        assert_eq!(root.children.len(), 4);
        // Column 0 width = (500 - 20) / 2 = 240
        // Children 0 and 1 are in col 0 (x = 0)
        assert_eq!(root.children[0].dimensions.content.x(), 0.0);
        assert_eq!(root.children[0].dimensions.content.width(), 240.0);
        assert_eq!(root.children[1].dimensions.content.x(), 0.0);

        // Children 2 and 3 are in col 1 (x = 240 + 20 = 260)
        assert_eq!(root.children[2].dimensions.content.x(), 260.0);
        assert_eq!(root.children[2].dimensions.content.width(), 240.0);
        assert_eq!(root.children[3].dimensions.content.x(), 260.0);
    }

    #[test]
    fn test_replaced_element_aspect_ratio_auto_height_and_width() {
        let mut containing_block = Dimensions::default();
        containing_block.content = Rect::new(0.0, 0.0, 800.0, 600.0);

        // Case 1: Image has intrinsic 400x200 (aspect ratio 2:1). CSS sets width: 200px, height: auto.
        // Height must resolve to 100px.
        let mut img_box = LayoutBox::new(
            crate::box_model::BoxType::ReplacedElement {
                intrinsic_width: 400.0,
                intrinsic_height: 200.0,
                pixels: vec![],
            },
            None,
        );
        let mut style = ComputedStyle::default();
        style.width = Length::Px(200.0);
        style.height = Length::Auto;
        img_box.style = Some(style);

        let mut float_ctx = FloatContext::new();
        layout_block(&mut img_box, &containing_block, &mut float_ctx);

        assert_eq!(img_box.dimensions.content.width(), 200.0);
        assert_eq!(img_box.dimensions.content.height(), 100.0, "Auto height should preserve 2:1 aspect ratio");

        // Case 2: Image has intrinsic 400x200 (aspect ratio 2:1). CSS sets width: auto, height: 50px.
        // Width must resolve to 100px.
        let mut img_box2 = LayoutBox::new(
            crate::box_model::BoxType::ReplacedElement {
                intrinsic_width: 400.0,
                intrinsic_height: 200.0,
                pixels: vec![],
            },
            None,
        );
        let mut style2 = ComputedStyle::default();
        style2.width = Length::Auto;
        style2.height = Length::Px(50.0);
        img_box2.style = Some(style2);

        let mut float_ctx2 = FloatContext::new();
        layout_block(&mut img_box2, &containing_block, &mut float_ctx2);

        assert_eq!(img_box2.dimensions.content.width(), 100.0, "Auto width should preserve 2:1 aspect ratio");
        assert_eq!(img_box2.dimensions.content.height(), 50.0);

        // Case 3: Image has intrinsic 400x200 (2:1) but CSS aspect-ratio is 16/9.
        // width: 320px, height: auto -> height should be 320 / (16/9) = 180px.
        let mut img_box3 = LayoutBox::new(
            crate::box_model::BoxType::ReplacedElement {
                intrinsic_width: 400.0,
                intrinsic_height: 200.0,
                pixels: vec![],
            },
            None,
        );
        let mut style3 = ComputedStyle::default();
        style3.width = Length::Px(320.0);
        style3.height = Length::Auto;
        style3.aspect_ratio = Some(16.0 / 9.0);
        img_box3.style = Some(style3);

        let mut float_ctx3 = FloatContext::new();
        layout_block(&mut img_box3, &containing_block, &mut float_ctx3);

        assert_eq!(img_box3.dimensions.content.width(), 320.0);
        assert!((img_box3.dimensions.content.height() - 180.0).abs() < 1.0, "CSS aspect-ratio should override intrinsic aspect ratio");

        // Case 4: Image has intrinsic 400x200 (2:1) but CSS aspect-ratio is 16/9.
        // width: auto, height: 180px -> width should be 180 * (16/9) = 320px.
        let mut img_box4 = LayoutBox::new(
            crate::box_model::BoxType::ReplacedElement {
                intrinsic_width: 400.0,
                intrinsic_height: 200.0,
                pixels: vec![],
            },
            None,
        );
        let mut style4 = ComputedStyle::default();
        style4.width = Length::Auto;
        style4.height = Length::Px(180.0);
        style4.aspect_ratio = Some(16.0 / 9.0);
        img_box4.style = Some(style4);

        let mut float_ctx4 = FloatContext::new();
        layout_block(&mut img_box4, &containing_block, &mut float_ctx4);

        assert!((img_box4.dimensions.content.width() - 320.0).abs() < 1.0, "CSS aspect-ratio should override intrinsic aspect ratio for auto width");
        assert_eq!(img_box4.dimensions.content.height(), 180.0);
    }

    #[test]
    fn test_main_page_shrink_to_fit() {
        let html = r#"<style>
.vector-menu-tabs .vector-menu-content-list { display: flex; }
.vector-menu-tabs li { display: block; float: left; margin: 0; padding: 0; }
.vector-menu-tabs li a { display: inline-flex; align-items: center; }
</style>
<nav class="vector-menu vector-menu-tabs">
<div class="vector-menu-content">
<ul class="vector-menu-content-list">
<li id="ca-nstab-main" class="selected vector-tab-noicon mw-list-item"><a href="/wiki/Main_Page"><span>Main Page</span></a></li>
</ul>
</div>
</nav>"#;
        let doc = mango_html::parse_html(html);
        let st = crate::style_tree::build_style_tree(&doc, &[]).unwrap();
        let mut bt = crate::box_tree::build_box_tree(&st);
        let cb = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 0.0));
        let mut float_ctx = FloatContext::new();
        layout_block(&mut bt, &cb, &mut float_ctx);
        fn print_tree(b: &LayoutBox, depth: usize) {
            println!("{}Box {:?}: tag={:?} w={} h={}", " ".repeat(depth), b.box_type, b.tag_name, b.dimensions.content.width(), b.dimensions.content.height());
            for c in &b.children {
                print_tree(c, depth + 2);
            }
        }
        print_tree(&bt, 0);
    }
}
