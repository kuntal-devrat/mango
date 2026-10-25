use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use mango_core::EdgeSizes;
use mango_css::values::{BoxSizing, Direction, Float, Length, Position, WritingMode};

use super::bfc::establishes_bfc;
use super::intrinsic::calculate_shrink_to_fit_width;

pub(crate) fn calculate_block_width(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    let style = box_node.style.clone().unwrap_or_default();
    let container_width = containing_block.content.width();
    let container_height = containing_block.content.height().max(0.0);

    // 1. Padding and borders
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    let pad_top = style.padding_top.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let pad_right = style.padding_right.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let pad_bottom = style.padding_bottom.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let pad_left = style.padding_left.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    box_node.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    box_node.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);
    box_node.dimensions.writing_mode = style.writing_mode;
    box_node.dimensions.direction = style.direction;

    // 2. Margins and Width
    let margin_top = style.margin_top.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let margin_bottom = style.margin_bottom.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );

    let is_width_auto = style.width == Length::Auto;
    let is_margin_left_auto = style.margin_left == Length::Auto;
    let is_margin_right_auto = style.margin_right == Length::Auto;

    let mut margin_left = style.margin_left.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );
    let mut margin_right = style.margin_right.to_px_with_viewport(
        font_size,
        root_font_size,
        container_width,
        container_height,
    );

    let total_non_content_h = pad_left + pad_right + border_left + border_right;

    let is_positioned = matches!(style.position, Position::Absolute | Position::Fixed);
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
        if (style.contain.contains_size() || style.contain.contains_inline_size())
            && let Some(contain_iw) = style.contain_intrinsic_width
        {
            let raw_iw = contain_iw.to_px_with_viewport(
                font_size,
                root_font_size,
                container_width,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (raw_iw - total_non_content_h).max(0.0)
            } else {
                raw_iw.max(0.0)
            }
        } else if let crate::box_model::BoxType::ReplacedElement {
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
            if style.height != Length::Auto && *intrinsic_height > 0.0 {
                let ratio = if let Some(r) = style.aspect_ratio
                    && r > 0.0
                {
                    r
                } else {
                    *intrinsic_width / *intrinsic_height
                };
                let raw_h = style.height.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    container_height,
                    container_height,
                );
                (raw_h * ratio).max(1.0)
            } else {
                *intrinsic_width
            }
        } else if is_inline_level
            || is_floating
            || is_positioned
            || (style.writing_mode != WritingMode::HorizontalTb
                && box_node.tag_name.as_deref() != Some("html")
                && box_node.tag_name.as_deref() != Some("body"))
        {
            if let Some(ratio) = style.aspect_ratio
                && ratio > 0.0
                && style.height != Length::Auto
            {
                let raw_h = style.height.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    container_height,
                    container_height,
                );
                (raw_h * ratio).max(1.0)
            } else {
                calculate_shrink_to_fit_width(
                    box_node,
                    (container_width - total_non_content_h).max(0.0),
                )
            }
        } else if let Some(ratio) = style.aspect_ratio
            && ratio > 0.0
            && style.height != Length::Auto
        {
            let raw_h = style.height.to_px_with_viewport(
                font_size,
                root_font_size,
                container_height,
                container_height,
            );
            (raw_h * ratio).max(1.0)
        } else {
            (container_width - total_non_content_h - margin_left - margin_right).max(0.0)
        }
    } else {
        let raw_w = style.width.to_px_with_viewport(
            font_size,
            root_font_size,
            container_width,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_w - total_non_content_h).max(0.0)
        } else {
            raw_w.max(0.0)
        }
    };

    // Apply min-width and max-width clamping (CSS 2.1 § 10.4)
    let mut clamped_w = tentative_width;
    if style.max_width != Length::Auto {
        let raw_max = style.max_width.to_px_with_viewport(
            font_size,
            root_font_size,
            container_width,
            container_height,
        );
        let max_content_w = if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - total_non_content_h).max(0.0)
        } else {
            raw_max
        };
        clamped_w = clamped_w.min(max_content_w);
    }
    if style.min_width != Length::Auto {
        let raw_min = style.min_width.to_px_with_viewport(
            font_size,
            root_font_size,
            container_width,
            container_height,
        );
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
        let underflow =
            container_width - (clamped_w + total_non_content_h + margin_left + margin_right);

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
            let cb_wm = containing_block.writing_mode;
            let cb_dir = containing_block.direction;
            let is_rtl = style.direction == Direction::Rtl || cb_dir == Direction::Rtl;
            let is_left_end = matches!(cb_wm, WritingMode::VerticalRl | WritingMode::SidewaysRl)
                || (cb_wm == WritingMode::HorizontalTb && is_rtl);

            if is_left_end {
                // Over-constrained where start is right and end is left: margin-left absorbs underflow (CSS 2.1 §10.3.3 & CSS Writing Modes 3 §7.1)
                margin_left = (margin_left + underflow).max(0.0);
            } else {
                // Over-constrained where start is left and end is right: margin-right absorbs underflow (CSS 2.1 §10.3.3 & CSS Writing Modes 3 §7.1)
                margin_right = (margin_right + underflow).max(0.0);
            }
        }
    }

    box_node.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    box_node.dimensions.content.size.width = clamped_w;
}

fn in_flow_block_children_bottom(box_node: &LayoutBox) -> Option<f32> {
    let parent_has_bottom_strut = box_node.dimensions.border.bottom > 0.0
        || box_node.dimensions.padding.bottom > 0.0
        || establishes_bfc(box_node);

    let mut bottom: Option<f32> = None;
    for child in &box_node.children {
        if child.is_inline() {
            continue;
        }
        let position = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
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

pub(crate) fn calculate_block_height(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    let style = box_node.style.clone().unwrap_or_default();
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    let container_h = containing_block.content.height().max(0.0);
    let container_w = containing_block.content.width();
    let non_content_v =
        box_node.dimensions.padding.vertical() + box_node.dimensions.border.vertical();
    let is_percent_indefinite = matches!(style.height, Length::Percent(_))
        && containing_block.content.height() <= 0.0
        && !matches!(style.position, Position::Absolute | Position::Fixed);

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
                let h = style.height.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    container_h,
                    container_h,
                );
                box_node.dimensions.content.size.height = h;
            } else if *intrinsic_width > 0.0 && box_node.dimensions.content.size.width > 0.0 {
                let ratio = if let Some(r) = style.aspect_ratio
                    && r > 0.0
                {
                    r
                } else {
                    *intrinsic_width / *intrinsic_height
                };
                box_node.dimensions.content.size.height =
                    (box_node.dimensions.content.size.width / ratio).max(1.0);
            } else {
                box_node.dimensions.content.size.height = *intrinsic_height;
            }
        } else if *intrinsic_width > 0.0 && box_node.dimensions.content.size.width > 0.0 {
            let ratio = if let Some(r) = style.aspect_ratio
                && r > 0.0
            {
                r
            } else {
                *intrinsic_width / *intrinsic_height
            };
            box_node.dimensions.content.size.height =
                (box_node.dimensions.content.size.width / ratio).max(1.0);
        } else {
            box_node.dimensions.content.size.height = *intrinsic_height;
        }
    } else if !is_percent_indefinite
        && !matches!(
            style.height,
            Length::Auto
                | Length::MinContent
                | Length::MaxContent
                | Length::FitContent
                | Length::Content
        )
    {
        let is_pos = matches!(style.position, Position::Absolute | Position::Fixed);
        let eff_container_h = if containing_block.content.height() > 0.0 {
            containing_block.content.height()
        } else if is_pos {
            600.0
        } else {
            0.0
        };
        let h = style
            .height
            .to_px_with_viewport(font_size, root_font_size, eff_container_h, 600.0);
        if style.box_sizing == BoxSizing::BorderBox {
            box_node.dimensions.content.size.height = (h - non_content_v).max(0.0);
        } else {
            box_node.dimensions.content.size.height = h.max(0.0);
        }
    } else if style.contain.contains_size()
        || style.content_visibility == mango_css::values::ContentVisibility::Hidden
    {
        if let Some(intrinsic_h) = style.contain_intrinsic_height {
            let raw_ih =
                intrinsic_h.to_px_with_viewport(font_size, root_font_size, container_h, 600.0);
            if style.box_sizing == BoxSizing::BorderBox {
                box_node.dimensions.content.size.height = (raw_ih - non_content_v).max(0.0);
            } else {
                box_node.dimensions.content.size.height = raw_ih.max(0.0);
            }
        } else {
            box_node.dimensions.content.size.height = 0.0;
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
                        (bottom - box_node.dimensions.content.y()).max(0.0);
                }
            }
        }
    } else if let Some(bottom) = in_flow_block_children_bottom(box_node) {
        // Auto height from in-flow *block* children.
        box_node.dimensions.content.size.height =
            (bottom - box_node.dimensions.content.y()).max(0.0);
    }

    // In CSS 2.1 §10.6.7: Auto heights for BFC roots must enclose all floating descendants
    if establishes_bfc(box_node) && style.height == Length::Auto {
        fn find_deep_floats_bottom(node: &LayoutBox, current_max: &mut f32) {
            for child in &node.children {
                if child.style.as_ref().is_some_and(|s| s.float != Float::None) {
                    *current_max = current_max.max(child.dimensions.margin_box().bottom());
                } else if !establishes_bfc(child) {
                    find_deep_floats_bottom(child, current_max);
                }
            }
        }
        let mut max_float_bottom = box_node.dimensions.content.y();
        find_deep_floats_bottom(box_node, &mut max_float_bottom);
        let needed_h = max_float_bottom - box_node.dimensions.content.y();
        if needed_h > box_node.dimensions.content.size.height {
            box_node.dimensions.content.size.height = needed_h;
        }
    }

    // 1b. Apply aspect-ratio if height is auto and aspect_ratio is defined
    if style.height == Length::Auto
        && let Some(ratio) = style.aspect_ratio
        && ratio > 0.0
        && box_node.dimensions.content.size.width > 0.0
    {
        box_node.dimensions.content.size.height =
            (box_node.dimensions.content.size.width / ratio).max(1.0);
    }

    // 2. Apply max-height and min-height clamping (CSS 2.1 § 10.7)
    if style.max_height != Length::Auto {
        let raw_max = style.max_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_h,
            container_h,
        );
        let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (raw_max - non_content_v).max(0.0)
        } else {
            raw_max
        };
        box_node.dimensions.content.size.height =
            box_node.dimensions.content.size.height.min(max_content_h);
    }

    if style.min_height != Length::Auto {
        let raw_min = style.min_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_h,
            container_h,
        );
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
    if style.width == Length::Auto
        && let crate::box_model::BoxType::ReplacedElement {
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
        && *intrinsic_width > 0.0
        && *intrinsic_height > 0.0
    {
        let ratio = if let Some(r) = style.aspect_ratio
            && r > 0.0
        {
            r
        } else {
            *intrinsic_width / *intrinsic_height
        };
        let ratio_w = box_node.dimensions.content.size.height * ratio;
        let mut final_w = ratio_w;
        if style.max_width != Length::Auto {
            let raw_max = style.max_width.to_px_with_viewport(
                font_size,
                root_font_size,
                container_w,
                container_h,
            );
            final_w = final_w.min(raw_max);
        }
        if style.min_width != Length::Auto {
            let raw_min = style.min_width.to_px_with_viewport(
                font_size,
                root_font_size,
                container_w,
                container_h,
            );
            final_w = final_w.max(raw_min);
        }
        box_node.dimensions.content.size.width = final_w;
    }
    box_node.dimensions.content.size.height = box_node.dimensions.content.size.height.max(0.0);
}
