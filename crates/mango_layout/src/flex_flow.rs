//! CSS Flexible Box Layout Module Level 1 implementation.
//!
//! Handles flex container formatting: main-axis sizing, flex-line construction,
//! flex-grow / flex-shrink resolution, `justify-content` alignment, `align-items` / `align-self`
//! cross-axis positioning, and multi-line wrapping with gaps.

use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Display, FlexDirection, FlexWrap,
    JustifyContent, Length, Overflow, Position,
};

use crate::block_flow::layout_positioned_children;
use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

/// Helper function to lay out a flex item according to its formatting context.
fn layout_flex_item(
    child: &mut LayoutBox,
    item_cb: &Dimensions,
    float_ctx: &mut FloatContext,
    c_fs: f32,
) {
    let c_disp = child.style.as_ref().map(|s| s.display);
    if matches!(
        c_disp,
        Some(mango_css::values::Display::Flex) | Some(mango_css::values::Display::InlineFlex)
    ) {
        layout_flex(child, item_cb, float_ctx);
    } else if matches!(
        c_disp,
        Some(mango_css::values::Display::Grid) | Some(mango_css::values::Display::InlineGrid)
    ) {
        crate::grid_flow::layout_grid(child, item_cb, float_ctx);
    } else if let BoxType::TextNode(ref t) = child.box_type {
        let text_w = crate::inline_flow::measure_text_width_with_style(
            t,
            c_fs,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
        );
        child.dimensions.content.size.width = text_w;
        child.dimensions.content.size.height = c_fs * 1.3;
    } else if child.box_type.is_replaced() {
        // Replaced element dimensions already set by flex_flow
    } else if child.box_type.is_inline_block() || child.is_block() {
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
fn find_first_baseline_ascent(box_: &LayoutBox, default_fs: f32) -> f32 {
    if matches!(box_.box_type, BoxType::TextNode(_)) {
        let fs = box_.style.as_ref().map(|s| s.font_size).unwrap_or(default_fs);
        return fs * 0.8;
    }
    for child in &box_.children {
        if child.style.as_ref().map(|s| s.display != Display::None).unwrap_or(true) {
            let child_ascent = find_first_baseline_ascent(child, default_fs);
            if child_ascent > 0.0 {
                return child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
                    + child_ascent;
            }
        }
    }
    let fs = box_.style.as_ref().map(|s| s.font_size).unwrap_or(default_fs);
    fs * 0.8
}

/// Executes Flexbox layout on a flex container box.
pub fn layout_flex(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    let container_width = containing_block.content.width();
    let container_height = if containing_block.content.height() > 0.0 {
        containing_block.content.height()
    } else if let Some(style) = &container.style {
        match style.height {
            Length::Px(px) => px,
            _ => 0.0,
        }
    } else {
        0.0
    };

    let style = container.style.clone().unwrap_or_default();
    let font_size = style.font_size;
    let root_font_size = style.root_font_size;

    // 1. Resolve container padding, borders, and margins
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

    container.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    container.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);

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
    let total_non_content_v = pad_top + pad_bottom + border_top + border_bottom;

    let is_percent_indefinite =
        matches!(style.height, Length::Percent(_)) && containing_block.content.height() <= 0.0;

    let content_height = if style.height != Length::Auto && !is_percent_indefinite {
        let raw_h = style.height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            (raw_h - total_non_content_v).max(0.0)
        } else {
            raw_h
        }
    } else if containing_block.content.height() > 0.0
        && container.dimensions.content.size.height > 0.0
        && (containing_block.content.height()
            - total_non_content_v
            - container.dimensions.content.size.height)
            .abs()
            < 0.5
    {
        container.dimensions.content.size.height
    } else {
        0.0
    };

    let is_inline_flex = style.display == Display::InlineFlex;
    let is_width_content = matches!(
        style.width,
        Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
    );
    let (intrinsic_min, intrinsic_max) = if is_width_content
        || (is_inline_flex && is_width_auto)
        || matches!(
            style.max_width,
            Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
        )
        || matches!(
            style.min_width,
            Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
        ) {
        let mut memo = std::collections::HashMap::new();
        crate::table_flow::measure_box_intrinsic_widths(
            container,
            font_size,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
            &mut memo,
        )
    } else {
        (0.0, 0.0)
    };

    let mut content_width = if is_width_auto {
        if is_margin_left_auto {
            margin_left = 0.0;
        }
        if is_margin_right_auto {
            margin_right = 0.0;
        }
        let avail = (container_width - total_non_content_h - margin_left - margin_right).max(0.0);
        if is_inline_flex {
            let max_c = (intrinsic_max - total_non_content_h).max(0.0);
            let min_c = (intrinsic_min - total_non_content_h).max(0.0);
            avail.min(max_c).max(min_c)
        } else {
            avail
        }
    } else if is_width_content {
        match style.width {
            Length::MinContent => (intrinsic_min - total_non_content_h).max(0.0),
            Length::MaxContent | Length::Content => (intrinsic_max - total_non_content_h).max(0.0),
            Length::FitContent => {
                let avail =
                    (container_width - total_non_content_h - margin_left - margin_right).max(0.0);
                let max_c = (intrinsic_max - total_non_content_h).max(0.0);
                let min_c = (intrinsic_min - total_non_content_h).max(0.0);
                avail.min(max_c).max(min_c)
            }
            _ => (container_width - total_non_content_h).max(0.0),
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
            raw_w
        }
    };

    if style.max_width != Length::Auto {
        let max_content_w = match style.max_width {
            Length::MinContent => (intrinsic_min - total_non_content_h).max(0.0),
            Length::MaxContent | Length::Content => (intrinsic_max - total_non_content_h).max(0.0),
            Length::FitContent => {
                let avail =
                    (container_width - total_non_content_h - margin_left - margin_right).max(0.0);
                let max_c = (intrinsic_max - total_non_content_h).max(0.0);
                let min_c = (intrinsic_min - total_non_content_h).max(0.0);
                avail.min(max_c).max(min_c)
            }
            _ => {
                let raw_max = style.max_width.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    container_width,
                    container_height,
                );
                if style.box_sizing == BoxSizing::BorderBox {
                    (raw_max - total_non_content_h).max(0.0)
                } else {
                    raw_max
                }
            }
        };
        content_width = content_width.min(max_content_w);
    }
    if style.min_width != Length::Auto {
        let min_content_w = match style.min_width {
            Length::MinContent => (intrinsic_min - total_non_content_h).max(0.0),
            Length::MaxContent | Length::Content => (intrinsic_max - total_non_content_h).max(0.0),
            Length::FitContent => {
                let avail =
                    (container_width - total_non_content_h - margin_left - margin_right).max(0.0);
                let max_c = (intrinsic_max - total_non_content_h).max(0.0);
                let min_c = (intrinsic_min - total_non_content_h).max(0.0);
                avail.min(max_c).max(min_c)
            }
            _ => {
                let raw_min = style.min_width.to_px_with_viewport(
                    font_size,
                    root_font_size,
                    container_width,
                    container_height,
                );
                if style.box_sizing == BoxSizing::BorderBox {
                    (raw_min - total_non_content_h).max(0.0)
                } else {
                    raw_min
                }
            }
        };
        content_width = content_width.max(min_content_w);
    }

    let underflow =
        container_width - (content_width + total_non_content_h + margin_left + margin_right);

    if is_margin_left_auto && is_margin_right_auto {
        margin_left = (underflow / 2.0).max(0.0);
        margin_right = (underflow / 2.0).max(0.0);
    } else if is_margin_left_auto {
        margin_left = underflow.max(0.0);
    } else if is_margin_right_auto {
        margin_right = underflow.max(0.0);
    } else if !is_width_auto {
        margin_right += underflow;
    }

    container.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    container.dimensions.content.size.width = content_width;

    // Position container in parent
    let (container_x, container_y) = if container.dimensions.content.origin != Point::ZERO {
        (
            container.dimensions.content.origin.x,
            container.dimensions.content.origin.y,
        )
    } else {
        (
            containing_block.content.x() + margin_left + border_left + pad_left,
            containing_block.content.y() + margin_top + border_top + pad_top,
        )
    };
    container.dimensions.content.origin = Point::new(container_x, container_y);

    if container.children.is_empty() {
        container.dimensions.content.size.height = 0.0;
        return;
    }

    // 2. Identify flex direction and axes
    let is_row = matches!(
        style.flex_direction,
        FlexDirection::Row | FlexDirection::RowReverse
    );
    let is_reverse = matches!(
        style.flex_direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    );
    let is_wrap = style.flex_wrap != FlexWrap::NoWrap;
    let is_wrap_reverse = style.flex_wrap == FlexWrap::WrapReverse;

    let main_gap = if is_row {
        style.column_gap.to_px_with_viewport(
            font_size,
            root_font_size,
            content_width,
            container_height,
        )
    } else {
        style.row_gap.to_px_with_viewport(
            font_size,
            root_font_size,
            content_width,
            container_height,
        )
    };

    let cross_gap = if is_row {
        style.row_gap.to_px_with_viewport(
            font_size,
            root_font_size,
            content_width,
            container_height,
        )
    } else {
        style.column_gap.to_px_with_viewport(
            font_size,
            root_font_size,
            content_width,
            container_height,
        )
    };

    // 3. Sort children by `order` (stable sort)
    container
        .children
        .sort_by_key(|c| c.style.as_ref().map(|s| s.order).unwrap_or(0));

    // 4. Measure hypothetical main and cross sizes of each in-flow item
    struct ItemMetric {
        index: usize,
        base_main: f32,
        final_main: f32,
        min_main: f32,
        max_main: f32,
        cross: f32,
        baseline_ascent: f32,
        flex_grow: f32,
        flex_shrink: f32,
        pad_main: f32,
        pad_cross: f32,
        border_main: f32,
        border_cross: f32,
        margin_main: f32,
        margin_cross: f32,
    }

    let mut item_metrics: Vec<ItemMetric> = Vec::with_capacity(container.children.len());

    for (idx, child) in container.children.iter_mut().enumerate() {
        let child_style = child.style.clone().unwrap_or_default();
        if child_style.display == Display::None
            || matches!(child_style.position, Position::Absolute | Position::Fixed)
        {
            continue;
        }
        let c_fs = child_style.font_size;
        let c_root_fs = child_style.root_font_size;

        let p_top = child_style.padding_top.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let p_right = child_style.padding_right.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let p_bottom = child_style.padding_bottom.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let p_left = child_style.padding_left.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );

        let b_top = child_style.border_top_width;
        let b_right = child_style.border_right_width;
        let b_bottom = child_style.border_bottom_width;
        let b_left = child_style.border_left_width;

        let m_top = child_style.margin_top.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let m_right = child_style.margin_right.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let m_bottom = child_style.margin_bottom.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let m_left = child_style.margin_left.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );

        child.dimensions.padding = EdgeSizes::new(p_top, p_right, p_bottom, p_left);
        child.dimensions.border = EdgeSizes::new(b_top, b_right, b_bottom, b_left);
        child.dimensions.margin = EdgeSizes::new(m_top, m_right, m_bottom, m_left);

        let (pad_main, pad_cross) = if is_row {
            (p_left + p_right, p_top + p_bottom)
        } else {
            (p_top + p_bottom, p_left + p_right)
        };
        let (border_main, border_cross) = if is_row {
            (b_left + b_right, b_top + b_bottom)
        } else {
            (b_top + b_bottom, b_left + b_right)
        };
        let (margin_main, margin_cross) = if is_row {
            (m_left + m_right, m_top + m_bottom)
        } else {
            (m_top + m_bottom, m_left + m_right)
        };

        // Resolve item main size
        let basis = &child_style.flex_basis;
        let is_basis_content = matches!(basis, Length::Content) || basis.is_content();
        let is_basis_min_content = matches!(basis, Length::MinContent);
        let is_basis_max_content = matches!(basis, Length::MaxContent);
        let is_basis_fit_content = matches!(basis, Length::FitContent);
        let is_basis_auto = basis.is_auto();

        let mut is_main_explicit = false;
        let explicit_main = if is_row {
            if is_basis_min_content {
                let mut memo = std::collections::HashMap::new();
                let (c_min, _) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                (c_min - pad_main - border_main).max(0.0)
            } else if is_basis_max_content || is_basis_content {
                let mut memo = std::collections::HashMap::new();
                let (_, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                (c_max - pad_main - border_main).max(0.0)
            } else if is_basis_fit_content {
                let mut memo = std::collections::HashMap::new();
                let (c_min, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                let fit = content_width.min(c_max).max(c_min);
                (fit - pad_main - border_main).max(0.0)
            } else if !is_basis_auto {
                is_main_explicit = true;
                basis.to_px_with_viewport(c_fs, c_root_fs, content_width, container_height)
            } else if matches!(child_style.width, Length::MinContent) {
                let mut memo = std::collections::HashMap::new();
                let (c_min, _) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                (c_min - pad_main - border_main).max(0.0)
            } else if matches!(child_style.width, Length::MaxContent | Length::Content) {
                let mut memo = std::collections::HashMap::new();
                let (_, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                (c_max - pad_main - border_main).max(0.0)
            } else if matches!(child_style.width, Length::FitContent) {
                let mut memo = std::collections::HashMap::new();
                let (c_min, c_max) = crate::table_flow::measure_box_intrinsic_widths(
                    child,
                    c_fs,
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    &mut memo,
                );
                let fit = content_width.min(c_max).max(c_min);
                (fit - pad_main - border_main).max(0.0)
            } else if child_style.width != Length::Auto {
                is_main_explicit = true;
                child_style.width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    content_width,
                    container_height,
                )
            } else if let Some(ratio) = child_style.aspect_ratio
                && ratio > 0.0
                && child_style.height != Length::Auto
            {
                let h = child_style.height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                (h * ratio).max(0.0)
            } else {
                match &child.box_type {
                    BoxType::ReplacedElement {
                        intrinsic_width,
                        intrinsic_height,
                        ..
                    }
                    | BoxType::IFrame {
                        intrinsic_width,
                        intrinsic_height,
                        ..
                    }
                    | BoxType::Video {
                        intrinsic_width,
                        intrinsic_height,
                        ..
                    }
                    | BoxType::Audio {
                        intrinsic_width,
                        intrinsic_height,
                        ..
                    }
                    | BoxType::Canvas {
                        intrinsic_width,
                        intrinsic_height,
                        ..
                    } => {
                        if child_style.height != Length::Auto && *intrinsic_height > 0.0 {
                            let ratio = if let Some(r) = child_style.aspect_ratio
                                && r > 0.0
                            {
                                r
                            } else {
                                *intrinsic_width / *intrinsic_height
                            };
                            let h = child_style.height.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                container_height,
                                container_height,
                            );
                            (h * ratio).max(0.0)
                        } else {
                            *intrinsic_width
                        }
                    }
                    _ => 0.0,
                }
            }
        } else if is_basis_min_content
            || is_basis_max_content
            || is_basis_content
            || is_basis_fit_content
        {
            0.0
        } else if !is_basis_auto {
            is_main_explicit = true;
            basis.to_px_with_viewport(c_fs, c_root_fs, container_height, container_height)
        } else if child_style.height != Length::Auto
            && !matches!(
                child_style.height,
                Length::MinContent | Length::MaxContent | Length::FitContent | Length::Content
            )
        {
            is_main_explicit = true;
            child_style.height.to_px_with_viewport(
                c_fs,
                c_root_fs,
                container_height,
                container_height,
            )
        } else if let Some(ratio) = child_style.aspect_ratio
            && ratio > 0.0
            && child_style.width != Length::Auto
        {
            let w = child_style.width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                container_height,
            );
            (w / ratio).max(0.0)
        } else {
            match &child.box_type {
                BoxType::ReplacedElement {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::IFrame {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::Video {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::Audio {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::Canvas {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                } => {
                    if child_style.width != Length::Auto && *intrinsic_width > 0.0 {
                        let ratio = if let Some(r) = child_style.aspect_ratio
                            && r > 0.0
                        {
                            r
                        } else {
                            *intrinsic_width / *intrinsic_height
                        };
                        let w = child_style.width.to_px_with_viewport(
                            c_fs,
                            c_root_fs,
                            content_width,
                            container_height,
                        );
                        (w / ratio).max(0.0)
                    } else {
                        *intrinsic_height
                    }
                }
                _ => {
                    let item_w = if child_style.width != Length::Auto {
                        child_style.width.to_px_with_viewport(
                            c_fs,
                            c_root_fs,
                            content_width,
                            container_height,
                        )
                    } else {
                        (content_width - pad_cross - border_cross - margin_cross).max(0.0)
                    };
                    child.dimensions.content.size.width = item_w;
                    let item_cb = Dimensions::new(Rect::new(
                        0.0,
                        0.0,
                        item_w + pad_cross + border_cross,
                        0.0,
                    ));
                    let mut item_float_ctx = FloatContext::new();
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    child.dimensions.content.size.height.max(c_fs * 1.3)
                }
            }
        };

        // In CSS, percentage height against an indefinite (auto) container height computes to auto
        let is_child_height_auto = child_style.height == Length::Auto
            || (matches!(child_style.height, Length::Percent(_))
                && (container_height <= 0.0
                    || container
                        .style
                        .as_ref()
                        .map(|s| s.height == Length::Auto)
                        .unwrap_or(true)));

        // Resolve cross size
        let mut cross_size = if is_row {
            if !is_child_height_auto {
                let h = child_style.height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                if child_style.box_sizing == BoxSizing::BorderBox {
                    (h - pad_cross - border_cross).max(0.0)
                } else {
                    h
                }
            } else if let Some(ratio) = child_style.aspect_ratio
                && ratio > 0.0
                && explicit_main > 0.0
            {
                (explicit_main / ratio).max(0.0)
            } else {
                match &child.box_type {
                    BoxType::ReplacedElement {
                        intrinsic_height,
                        intrinsic_width,
                        ..
                    }
                    | BoxType::IFrame {
                        intrinsic_height,
                        intrinsic_width,
                        ..
                    }
                    | BoxType::Video {
                        intrinsic_height,
                        intrinsic_width,
                        ..
                    }
                    | BoxType::Audio {
                        intrinsic_height,
                        intrinsic_width,
                        ..
                    }
                    | BoxType::Canvas {
                        intrinsic_height,
                        intrinsic_width,
                        ..
                    } => {
                        if explicit_main > 0.0 && *intrinsic_width > 0.0 && *intrinsic_height > 0.0
                        {
                            let ratio = if let Some(r) = child_style.aspect_ratio
                                && r > 0.0
                            {
                                r
                            } else {
                                *intrinsic_width / *intrinsic_height
                            };
                            (explicit_main / ratio).max(0.0)
                        } else {
                            *intrinsic_height
                        }
                    }
                    _ if child.tag_name.as_deref() == Some("input") => {
                        let itype = child.get_attribute("type").unwrap_or("text");
                        if itype == "checkbox" || itype == "radio" {
                            16.0
                        } else {
                            24.0
                        }
                    }
                    _ => {
                        let item_w = if explicit_main > 0.0 {
                            explicit_main
                        } else if child_style.width != Length::Auto {
                            child_style.width.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                content_width,
                                container_height,
                            )
                        } else {
                            (content_width - pad_main - border_main - margin_main).max(0.0)
                        };
                        child.dimensions.content.size.width = item_w;
                        let item_cb = Dimensions::new(Rect::new(
                            0.0,
                            0.0,
                            item_w + pad_main + border_main,
                            0.0,
                        ));
                        let mut item_float_ctx = FloatContext::new();
                        layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                        child.dimensions.content.size.height.max(c_fs * 1.3)
                    }
                }
            }
        } else if child_style.width != Length::Auto {
            let w = child_style.width.to_px_with_viewport(
                c_fs,
                c_root_fs,
                content_width,
                container_height,
            );
            if child_style.box_sizing == BoxSizing::BorderBox {
                (w - pad_cross - border_cross).max(0.0)
            } else {
                w
            }
        } else if let Some(ratio) = child_style.aspect_ratio
            && ratio > 0.0
            && explicit_main > 0.0
        {
            (explicit_main * ratio).max(0.0)
        } else if let Some(w) = match &child.box_type {
            BoxType::ReplacedElement {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::IFrame {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Video {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Audio {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Canvas {
                intrinsic_width,
                intrinsic_height,
                ..
            } => {
                if explicit_main > 0.0 && *intrinsic_width > 0.0 && *intrinsic_height > 0.0 {
                    let ratio = if let Some(r) = child_style.aspect_ratio
                        && r > 0.0
                    {
                        r
                    } else {
                        *intrinsic_width / *intrinsic_height
                    };
                    Some((explicit_main * ratio).max(0.0))
                } else {
                    Some(*intrinsic_width)
                }
            }
            _ => None,
        } {
            w
        } else {
            let item_align = match child_style.align_self {
                AlignSelf::Auto => style.align_items,
                AlignSelf::Stretch => AlignItems::Stretch,
                AlignSelf::FlexStart => AlignItems::FlexStart,
                AlignSelf::FlexEnd => AlignItems::FlexEnd,
                AlignSelf::Center => AlignItems::Center,
                AlignSelf::Baseline => AlignItems::Baseline,
            };
            if item_align == AlignItems::Stretch {
                (content_width - pad_cross - border_cross - margin_cross).max(0.0)
            } else {
                let fit_w = crate::block_flow::calculate_shrink_to_fit_width(child, content_width);
                if fit_w > 0.0 {
                    (fit_w - pad_cross - border_cross).max(0.0)
                } else {
                    let mut memo = std::collections::HashMap::new();
                    let (_, max_w) = crate::table_flow::measure_box_intrinsic_widths(
                        child,
                        c_fs,
                        mango_render::FontWeight::Regular,
                        mango_render::FontFamily::SansSerif,
                        &mut memo,
                    );
                    if max_w > 0.0 {
                        (max_w - pad_cross - border_cross).max(0.0)
                    } else {
                        (content_width - pad_cross - border_cross - margin_cross).max(0.0)
                    }
                }
            }
        };

        // Clamp cross size by min/max cross constraints
        if is_row {
            if child_style.max_height != Length::Auto {
                let max_h = child_style.max_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                let max_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_h - pad_cross - border_cross).max(0.0)
                } else {
                    max_h
                };
                cross_size = cross_size.min(max_content_h);
            }
            if child_style.min_height != Length::Auto {
                let min_h = child_style.min_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                let min_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_h - pad_cross - border_cross).max(0.0)
                } else {
                    min_h
                };
                cross_size = cross_size.max(min_content_h);
            }
        } else {
            if child_style.max_width != Length::Auto {
                let max_w = child_style.max_width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    content_width,
                    container_height,
                );
                let max_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_w - pad_cross - border_cross).max(0.0)
                } else {
                    max_w
                };
                cross_size = cross_size.min(max_content_w);
            }
            if child_style.min_width != Length::Auto {
                let min_w = child_style.min_width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    content_width,
                    container_height,
                );
                let min_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_w - pad_cross - border_cross).max(0.0)
                } else {
                    min_w
                };
                cross_size = cross_size.max(min_content_w);
            }
        }

        // If explicit main size is > 0 and was author-specified (not measured intrinsic content),
        // adjust for box-sizing: border-box. Intrinsic content-measured sizes already exclude padding.
        // If explicit main size was author-specified (including 0.0 for flex-basis: 0),
        // adjust for box-sizing: border-box. Intrinsic content-measured sizes already exclude padding.
        let mut base_main = if is_main_explicit {
            if child_style.box_sizing == BoxSizing::BorderBox {
                (explicit_main - pad_main - border_main).max(0.0)
            } else {
                explicit_main.max(0.0)
            }
        } else if explicit_main > 0.0 {
            explicit_main
        } else {
            // Intrinsic sizing: measure text or replaced element
            match &child.box_type {
                BoxType::TextNode(t) => {
                    let w = crate::inline_flow::measure_text_width_with_style(
                        t,
                        c_fs,
                        mango_render::FontWeight::Regular,
                        mango_render::FontFamily::SansSerif,
                    );
                    if is_row { w } else { c_fs * 1.3 }
                }
                BoxType::ReplacedElement {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::IFrame {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::Video {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::Audio {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                }
                | BoxType::Canvas {
                    intrinsic_width,
                    intrinsic_height,
                    ..
                } => {
                    if is_row {
                        *intrinsic_width
                    } else {
                        *intrinsic_height
                    }
                }
                _ => {
                    if is_row {
                        let fit_w =
                            crate::block_flow::calculate_shrink_to_fit_width(child, content_width);
                        if fit_w > 0.0 { fit_w } else { 60.0 }
                    } else {
                        // Lay out column item at cross_size to get its actual content height on a clone
                        let mut child_clone = child.clone();
                        child_clone.dimensions.content.size.width = cross_size;
                        let item_cb = Dimensions::new(Rect::new(0.0, 0.0, cross_size, 0.0));
                        let mut item_float_ctx = FloatContext::new();
                        layout_flex_item(&mut child_clone, &item_cb, &mut item_float_ctx, c_fs);
                        child_clone.dimensions.content.size.height
                    }
                }
            }
        };

        // Determine min_main and max_main constraints, including automatic minimum size (CSS Flexbox §4.5)
        let mut min_main = 0.0f32;
        let mut max_main = f32::INFINITY;

        if is_row {
            if child_style.max_width != Length::Auto {
                let max_w = child_style.max_width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    content_width,
                    container_height,
                );
                let max_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_w - pad_main - border_main).max(0.0)
                } else {
                    max_w
                };
                max_main = max_content_w;
            }
            if child_style.min_width != Length::Auto {
                let min_w = child_style.min_width.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    content_width,
                    container_height,
                );
                let min_content_w = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_w - pad_main - border_main).max(0.0)
                } else {
                    min_w
                };
                min_main = min_content_w;
            } else if child_style.overflow_x == Overflow::Visible {
                let auto_min = match &child.box_type {
                    BoxType::ReplacedElement { intrinsic_width, .. }
                    | BoxType::IFrame { intrinsic_width, .. }
                    | BoxType::Video { intrinsic_width, .. }
                    | BoxType::Audio { intrinsic_width, .. }
                    | BoxType::Canvas { intrinsic_width, .. } => *intrinsic_width,
                    BoxType::TextNode(_) => {
                        let mut memo = std::collections::HashMap::new();
                        let (min_w, _) = crate::table_flow::measure_box_intrinsic_widths(
                            child,
                            c_fs,
                            mango_render::FontWeight::Regular,
                            mango_render::FontFamily::SansSerif,
                            &mut memo,
                        );
                        min_w
                    }
                    _ => 0.0,
                };
                min_main = auto_min.min(max_main);
            }
        } else {
            if child_style.max_height != Length::Auto {
                let max_h = child_style.max_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                let max_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                    (max_h - pad_main - border_main).max(0.0)
                } else {
                    max_h
                };
                max_main = max_content_h;
            }
            if child_style.min_height != Length::Auto {
                let min_h = child_style.min_height.to_px_with_viewport(
                    c_fs,
                    c_root_fs,
                    container_height,
                    container_height,
                );
                let min_content_h = if child_style.box_sizing == BoxSizing::BorderBox {
                    (min_h - pad_main - border_main).max(0.0)
                } else {
                    min_h
                };
                min_main = min_content_h;
            } else if child_style.overflow_y == Overflow::Visible {
                let auto_min = match &child.box_type {
                    BoxType::ReplacedElement { intrinsic_height, .. }
                    | BoxType::IFrame { intrinsic_height, .. }
                    | BoxType::Video { intrinsic_height, .. }
                    | BoxType::Audio { intrinsic_height, .. }
                    | BoxType::Canvas { intrinsic_height, .. } => *intrinsic_height,
                    _ => 0.0,
                };
                min_main = auto_min.min(max_main);
            }
        }

        base_main = base_main.clamp(min_main, max_main);

        let baseline_ascent = if is_row {
            m_top + b_top + p_top + find_first_baseline_ascent(child, c_fs)
        } else {
            m_left + b_left + p_left + find_first_baseline_ascent(child, c_fs)
        };

        item_metrics.push(ItemMetric {
            index: idx,
            base_main,
            final_main: base_main,
            min_main,
            max_main,
            cross: cross_size,
            baseline_ascent,
            flex_grow: child_style.flex_grow,
            flex_shrink: child_style.flex_shrink,
            pad_main,
            pad_cross,
            border_main,
            border_cross,
            margin_main,
            margin_cross,
        });
    }

    // 5. Construct flex lines (multi-line wrapping)
    let is_container_main_definite = if is_row {
        true
    } else {
        content_height > 0.0 || (style.height != Length::Auto && !is_percent_indefinite)
    };
    let container_main_size = if is_row {
        content_width
    } else {
        content_height
    };

    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current_line: Vec<usize> = Vec::new();
    let mut current_line_main = 0.0f32;

    for (metric_idx, metric) in item_metrics.iter().enumerate() {
        let outer_item_main =
            metric.base_main + metric.pad_main + metric.border_main + metric.margin_main;
        let item_gap = if current_line.is_empty() {
            0.0
        } else {
            main_gap
        };

        if is_wrap
            && is_container_main_definite
            && !current_line.is_empty()
            && (current_line_main + item_gap + outer_item_main > container_main_size + 0.5)
        {
            lines.push(std::mem::take(&mut current_line));
            current_line_main = 0.0;
        }

        current_line_main += if current_line.is_empty() {
            0.0
        } else {
            main_gap
        } + outer_item_main;
        current_line.push(metric_idx);
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // Resolve flexible main sizes and measure cross sizes for each line
    for line in &lines {
        let line_item_count = line.len();
        let total_gaps = if line_item_count > 1 {
            (line_item_count - 1) as f32 * main_gap
        } else {
            0.0
        };

        // Distribute free space with iterative violation resolution (CSS Flexbox §9.7)
        if is_container_main_definite {
            let mut is_frozen = vec![false; line_item_count];
            let mut item_sizes: Vec<f32> =
                line.iter().map(|&idx| item_metrics[idx].base_main).collect();

            for _ in 0..line_item_count {
                let mut used_space = total_gaps;
                for (i, &idx) in line.iter().enumerate() {
                    let m = &item_metrics[idx];
                    let size = if is_frozen[i] {
                        item_sizes[i]
                    } else {
                        m.base_main
                    };
                    used_space += size + m.pad_main + m.border_main + m.margin_main;
                }
                let cur_free_space = container_main_size - used_space;

                if cur_free_space > 0.0 {
                    let sum_grow: f32 = line
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !is_frozen[*i])
                        .map(|(_, &idx)| item_metrics[idx].flex_grow)
                        .sum();

                    if sum_grow <= 0.0 {
                        break;
                    }

                    let mut has_violation = false;
                    for (i, &idx) in line.iter().enumerate() {
                        if is_frozen[i] {
                            continue;
                        }
                        let m = &item_metrics[idx];
                        if m.flex_grow > 0.0 {
                            let target = m.base_main + cur_free_space * (m.flex_grow / sum_grow);
                            if target > m.max_main {
                                item_sizes[i] = m.max_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else if target < m.min_main {
                                item_sizes[i] = m.min_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else {
                                item_sizes[i] = target;
                            }
                        }
                    }
                    if !has_violation {
                        break;
                    }
                } else if cur_free_space < 0.0 {
                    let sum_scaled_shrink: f32 = line
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !is_frozen[*i])
                        .map(|(_, &idx)| {
                            item_metrics[idx].flex_shrink * item_metrics[idx].base_main
                        })
                        .sum();

                    if sum_scaled_shrink <= 0.0 {
                        break;
                    }

                    let mut has_violation = false;
                    for (i, &idx) in line.iter().enumerate() {
                        if is_frozen[i] {
                            continue;
                        }
                        let m = &item_metrics[idx];
                        if m.flex_shrink > 0.0 {
                            let sub = (cur_free_space.abs())
                                * (m.flex_shrink * m.base_main / sum_scaled_shrink);
                            let target = m.base_main - sub;
                            if target < m.min_main {
                                item_sizes[i] = m.min_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else if target > m.max_main {
                                item_sizes[i] = m.max_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else {
                                item_sizes[i] = target.max(0.0);
                            }
                        }
                    }
                    if !has_violation {
                        break;
                    }
                } else {
                    break;
                }
            }

            for (i, &idx) in line.iter().enumerate() {
                item_metrics[idx].final_main = item_sizes[i];
            }
        }

        // If row container, measure items whose height is Auto to resolve actual cross_size
        if is_row {
            for &idx in line {
                let m = &mut item_metrics[idx];
                let child = &mut container.children[m.index];
                let child_style = child.style.clone().unwrap_or_default();
                let is_child_h_auto = child_style.height == Length::Auto
                    || (matches!(child_style.height, Length::Percent(_))
                        && (container_height <= 0.0
                            || container
                                .style
                                .as_ref()
                                .map(|s| s.height == Length::Auto)
                                .unwrap_or(true)));
                if is_child_h_auto {
                    child.dimensions.content.size.width = m.final_main;
                    let item_cb = Dimensions::new(Rect::new(
                        0.0,
                        0.0,
                        m.final_main + m.pad_main + m.border_main,
                        0.0,
                    ));
                    let mut item_float_ctx = FloatContext::new();
                    let c_fs = child_style.font_size;
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    if child.dimensions.content.size.height > 0.0 {
                        m.cross = child.dimensions.content.size.height;
                    }
                }
            }
        }
    }

    // Pre-calculate line cross sizes for align-content distribution
    let container_cross_size = if is_row {
        if content_height > 0.0 {
            content_height
        } else {
            0.0
        }
    } else {
        content_width
    };

    let mut line_cross_sizes: Vec<f32> = Vec::with_capacity(lines.len());
    let mut line_baseline_ascents: Vec<f32> = Vec::with_capacity(lines.len());
    for line in &lines {
        let mut max_ascent = 0.0f32;
        let mut max_descent = 0.0f32;
        let mut max_non_baseline_cross = 0.0f32;
        let mut has_baseline = false;

        for &idx in line {
            let m = &item_metrics[idx];
            let child = &container.children[m.index];
            let child_style = child.style.as_ref();
            let align = match child_style.map(|s| s.align_self).unwrap_or(AlignSelf::Auto) {
                AlignSelf::Auto => style.align_items,
                AlignSelf::Stretch => AlignItems::Stretch,
                AlignSelf::FlexStart => AlignItems::FlexStart,
                AlignSelf::FlexEnd => AlignItems::FlexEnd,
                AlignSelf::Center => AlignItems::Center,
                AlignSelf::Baseline => AlignItems::Baseline,
            };

            let outer_cross = m.cross + m.pad_cross + m.border_cross + m.margin_cross;
            if is_row && align == AlignItems::Baseline {
                has_baseline = true;
                let ascent = m.baseline_ascent;
                let descent = (outer_cross - ascent).max(0.0);
                max_ascent = max_ascent.max(ascent);
                max_descent = max_descent.max(descent);
            } else {
                max_non_baseline_cross = max_non_baseline_cross.max(outer_cross);
            }
        }

        let l_cross = if has_baseline {
            max_non_baseline_cross.max(max_ascent + max_descent)
        } else {
            max_non_baseline_cross
        }
        .max(if !is_wrap { container_cross_size } else { 0.0 });

        line_cross_sizes.push(l_cross);
        line_baseline_ascents.push(if has_baseline { max_ascent } else { 0.0 });
    }

    let line_count = lines.len();
    let total_lines_cross: f32 = line_cross_sizes.iter().sum::<f32>()
        + if line_count > 1 {
            (line_count - 1) as f32 * cross_gap
        } else {
            0.0
        };

    let free_cross = if is_wrap && container_cross_size > total_lines_cross {
        container_cross_size - total_lines_cross
    } else {
        0.0
    };

    let (mut cross_cursor, line_spacing) = if free_cross > 0.0 && is_wrap {
        match style.align_content {
            AlignContent::Stretch => {
                let extra_per_line = free_cross / line_count as f32;
                for size in &mut line_cross_sizes {
                    *size += extra_per_line;
                }
                (0.0, cross_gap)
            }
            AlignContent::FlexStart => (0.0, cross_gap),
            AlignContent::FlexEnd => (free_cross, cross_gap),
            AlignContent::Center => (free_cross / 2.0, cross_gap),
            AlignContent::SpaceBetween => {
                if line_count > 1 {
                    (0.0, cross_gap + free_cross / (line_count - 1) as f32)
                } else {
                    (0.0, cross_gap)
                }
            }
            AlignContent::SpaceAround => {
                if line_count > 0 {
                    let per_line = free_cross / line_count as f32;
                    (per_line / 2.0, cross_gap + per_line)
                } else {
                    (0.0, cross_gap)
                }
            }
            AlignContent::SpaceEvenly => {
                let spacing = free_cross / (line_count + 1) as f32;
                (spacing, cross_gap + spacing)
            }
        }
    } else {
        (0.0, cross_gap)
    };

    for (line_idx, line) in lines.iter().enumerate() {
        let line_item_count = line.len();
        let total_gaps = if line_item_count > 1 {
            (line_item_count - 1) as f32 * main_gap
        } else {
            0.0
        };

        // Recompute remaining space after grow/shrink for justify-content
        let final_line_main: f32 = line
            .iter()
            .map(|&idx| {
                let m = &item_metrics[idx];
                m.final_main + m.pad_main + m.border_main + m.margin_main
            })
            .sum();
        let remaining_free = if is_container_main_definite {
            (container_main_size - final_line_main - total_gaps).max(0.0)
        } else {
            0.0
        };

        // Count main axis auto margins
        let mut main_auto_margin_count = 0;
        for &idx in line {
            let ch = &container.children[item_metrics[idx].index];
            if let Some(cs) = &ch.style {
                if is_row {
                    if cs.margin_left == Length::Auto {
                        main_auto_margin_count += 1;
                    }
                    if cs.margin_right == Length::Auto {
                        main_auto_margin_count += 1;
                    }
                } else {
                    if cs.margin_top == Length::Auto {
                        main_auto_margin_count += 1;
                    }
                    if cs.margin_bottom == Length::Auto {
                        main_auto_margin_count += 1;
                    }
                }
            }
        }

        // Compute item main offsets via auto margins or justify-content
        let (mut main_cursor, item_spacing) = if main_auto_margin_count > 0 && remaining_free > 0.0
        {
            let auto_margin_add = remaining_free / main_auto_margin_count as f32;
            for &idx in line {
                let ch = &mut container.children[item_metrics[idx].index];
                if let Some(cs) = &ch.style {
                    if is_row {
                        if cs.margin_left == Length::Auto {
                            ch.dimensions.margin.left = auto_margin_add;
                        }
                        if cs.margin_right == Length::Auto {
                            ch.dimensions.margin.right = auto_margin_add;
                        }
                    } else {
                        if cs.margin_top == Length::Auto {
                            ch.dimensions.margin.top = auto_margin_add;
                        }
                        if cs.margin_bottom == Length::Auto {
                            ch.dimensions.margin.bottom = auto_margin_add;
                        }
                    }
                }
            }
            (0.0, main_gap)
        } else {
            match style.justify_content {
                JustifyContent::FlexStart => (0.0, main_gap),
                JustifyContent::FlexEnd => (remaining_free, main_gap),
                JustifyContent::Center => (remaining_free / 2.0, main_gap),
                JustifyContent::SpaceBetween => {
                    if line_item_count > 1 {
                        (
                            0.0,
                            main_gap + remaining_free / (line_item_count - 1) as f32,
                        )
                    } else {
                        (0.0, main_gap)
                    }
                }
                JustifyContent::SpaceAround => {
                    if line_item_count > 0 {
                        let per_item = remaining_free / line_item_count as f32;
                        (per_item / 2.0, main_gap + per_item)
                    } else {
                        (0.0, main_gap)
                    }
                }
                JustifyContent::SpaceEvenly => {
                    let spacing = (remaining_free + total_gaps) / (line_item_count + 1) as f32;
                    (spacing, spacing)
                }
            }
        };

        let line_cross_size = line_cross_sizes[line_idx];

        // Position each item on the line in logical main-start order
        let item_indices: Vec<usize> = line.clone();

        for &metric_idx in &item_indices {
            let m = &item_metrics[metric_idx];
            let child = &mut container.children[m.index];
            let child_style = child.style.clone().unwrap_or_default();

            let item_align = match child_style.align_self {
                AlignSelf::Auto => style.align_items,
                AlignSelf::Stretch => AlignItems::Stretch,
                AlignSelf::FlexStart => AlignItems::FlexStart,
                AlignSelf::FlexEnd => AlignItems::FlexEnd,
                AlignSelf::Center => AlignItems::Center,
                AlignSelf::Baseline => AlignItems::Baseline,
            };

            let is_child_cross_auto = if is_row {
                child_style.height == Length::Auto
                    || (matches!(child_style.height, Length::Percent(_))
                        && container
                            .style
                            .as_ref()
                            .map(|s| s.height == Length::Auto)
                            .unwrap_or(true))
            } else {
                child_style.width == Length::Auto
            };

            let mut item_cross = m.cross;
            if item_align == AlignItems::Stretch && is_child_cross_auto {
                item_cross =
                    (line_cross_size - m.pad_cross - m.border_cross - m.margin_cross).max(0.0);
            }

            let has_cross_auto_margin = if is_row {
                child_style.margin_top == Length::Auto || child_style.margin_bottom == Length::Auto
            } else {
                child_style.margin_left == Length::Auto || child_style.margin_right == Length::Auto
            };

            let mut cross_offset = 0.0;
            if has_cross_auto_margin {
                let free_cross = (line_cross_size
                    - (item_cross + m.pad_cross + m.border_cross + m.margin_cross))
                    .max(0.0);
                if is_row {
                    let is_top_auto = child_style.margin_top == Length::Auto;
                    let is_bottom_auto = child_style.margin_bottom == Length::Auto;
                    if is_top_auto && is_bottom_auto {
                        child.dimensions.margin.top = free_cross / 2.0;
                        child.dimensions.margin.bottom = free_cross / 2.0;
                    } else if is_top_auto {
                        child.dimensions.margin.top = free_cross;
                    } else if is_bottom_auto {
                        child.dimensions.margin.bottom = free_cross;
                    }
                } else {
                    let is_left_auto = child_style.margin_left == Length::Auto;
                    let is_right_auto = child_style.margin_right == Length::Auto;
                    if is_left_auto && is_right_auto {
                        child.dimensions.margin.left = free_cross / 2.0;
                        child.dimensions.margin.right = free_cross / 2.0;
                    } else if is_left_auto {
                        child.dimensions.margin.left = free_cross;
                    } else if is_right_auto {
                        child.dimensions.margin.right = free_cross;
                    }
                }
            } else {
                cross_offset = match item_align {
                    AlignItems::FlexStart | AlignItems::Stretch => 0.0,
                    AlignItems::Baseline => {
                        if is_row && line_baseline_ascents[line_idx] > 0.0 {
                            (line_baseline_ascents[line_idx] - m.baseline_ascent).max(0.0)
                        } else {
                            0.0
                        }
                    }
                    AlignItems::FlexEnd => {
                        line_cross_size
                            - (item_cross + m.pad_cross + m.border_cross + m.margin_cross)
                    }
                    AlignItems::Center => {
                        (line_cross_size
                            - (item_cross + m.pad_cross + m.border_cross + m.margin_cross))
                            / 2.0
                    }
                };
            }

            let old_x = child.dimensions.content.x();
            let old_y = child.dimensions.content.y();

            let item_outer_main = m.final_main + m.pad_main + m.border_main + m.margin_main;
            if is_row {
                let x = if is_reverse {
                    container_x
                        + container_main_size
                        - main_cursor
                        - item_outer_main
                        + child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left
                } else {
                    container_x
                        + main_cursor
                        + child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left
                };
                let y = container_y
                    + cross_cursor
                    + cross_offset
                    + child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top;
                child.dimensions.content = Rect::new(x, y, m.final_main, item_cross);
                main_cursor += item_outer_main + item_spacing;
            } else {
                let x = container_x
                    + cross_cursor
                    + cross_offset
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left;
                let y = if is_reverse {
                    container_y
                        + container_main_size
                        - main_cursor
                        - item_outer_main
                        + child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top
                } else {
                    container_y
                        + main_cursor
                        + child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top
                };
                child.dimensions.content = Rect::new(x, y, item_cross, m.final_main);
                main_cursor += item_outer_main + item_spacing;
            }

            let was_stretched = item_align == AlignItems::Stretch
                && is_child_cross_auto
                && (item_cross - m.cross).abs() > 0.5;
            if !was_stretched && (is_child_cross_auto && is_row) {
                // Already laid out in row measurement pass at this exact height; shift descendants to final placed position
                let dx = child.dimensions.content.x() - old_x;
                let dy = child.dimensions.content.y() - old_y;
                crate::block_flow::shift_descendants(child, dx, dy);
            } else {
                // Recursively lay out the contents of the flex item
                let item_containing_block = Dimensions::new(child.dimensions.border_box());
                let mut item_float_ctx = FloatContext::new();
                let c_fs = child.style.as_ref().map(|s| s.font_size).unwrap_or(16.0);
                layout_flex_item(child, &item_containing_block, &mut item_float_ctx, c_fs);
                if was_stretched {
                    if is_row {
                        child.dimensions.content.size.height = item_cross;
                    } else {
                        child.dimensions.content.size.width = item_cross;
                    }
                }
            }
        }

        cross_cursor += line_cross_size + line_spacing;
    }

    // 7. Calculate container final height
    if style.height != Length::Auto && !is_percent_indefinite {
        let h = style.height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        if style.box_sizing == BoxSizing::BorderBox {
            container.dimensions.content.size.height =
                (h - pad_top - pad_bottom - border_top - border_bottom).max(0.0);
        } else {
            container.dimensions.content.size.height = h;
        }
    } else if content_height > 0.0 {
        container.dimensions.content.size.height = content_height;
    } else if is_row {
        container.dimensions.content.size.height = (cross_cursor - cross_gap).max(0.0);
    } else {
        let max_bottom = container
            .children
            .iter()
            .map(|c| c.dimensions.margin_box().bottom())
            .fold(container_y, f32::max);
        container.dimensions.content.size.height = (max_bottom - container_y).max(0.0);
    }

    // Clamp container height by min-height and max-height constraints (G1)
    if style.max_height != Length::Auto {
        let max_h = style.max_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (max_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        } else {
            max_h
        };
        container.dimensions.content.size.height =
            container.dimensions.content.size.height.min(max_content_h);
    }
    if style.min_height != Length::Auto {
        let min_h = style.min_height.to_px_with_viewport(
            font_size,
            root_font_size,
            container_height,
            container_height,
        );
        let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
            (min_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        } else {
            min_h
        };
        container.dimensions.content.size.height =
            container.dimensions.content.size.height.max(min_content_h);
    }

    if is_wrap_reverse {
        if is_row {
            let total_h = container.dimensions.content.height();
            for child in &mut container.children {
                if child
                    .style
                    .as_ref()
                    .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                    .unwrap_or(false)
                {
                    continue;
                }
                let orig_y = child.dimensions.content.y() - container_y;
                let new_y = container_y + total_h - orig_y - child.dimensions.border_box().height();
                let dy = new_y - child.dimensions.content.y();
                child.dimensions.content.origin.y = new_y;
                crate::block_flow::shift_descendants(child, 0.0, dy);
            }
        } else {
            let total_w = container.dimensions.content.width();
            for child in &mut container.children {
                if child
                    .style
                    .as_ref()
                    .map(|s| matches!(s.position, Position::Absolute | Position::Fixed))
                    .unwrap_or(false)
                {
                    continue;
                }
                let orig_x = child.dimensions.content.x() - container_x;
                let new_x = container_x + total_w - orig_x - child.dimensions.border_box().width();
                let dx = new_x - child.dimensions.content.x();
                child.dimensions.content.origin.x = new_x;
                crate::block_flow::shift_descendants(child, dx, 0.0);
            }
        }
    }

    // 8. Lay out out-of-flow positioned children (CSS Flexbox §4.1)
    layout_positioned_children(container, float_ctx);
}
