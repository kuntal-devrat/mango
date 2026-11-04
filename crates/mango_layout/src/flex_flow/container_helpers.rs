use mango_css::computed::ComputedStyle;
use mango_css::values::{BoxSizing, Length};

use crate::box_tree::LayoutBox;

pub fn layout_empty_flex_container(
    container: &mut LayoutBox,
    style: &ComputedStyle,
    is_height_content: bool,
    is_vertical_wm: bool,
    content_height: f32,
    container_height: f32,
    font_size: f32,
    root_font_size: f32,
    pad_top: f32,
    pad_bottom: f32,
    border_top: f32,
    border_bottom: f32,
) {
    let is_editable = container
        .get_attribute("contenteditable")
        .is_some_and(|v| v != "false")
        || container.get_attribute("_mango_contenteditable") == Some("true");
    let default_line_h = if is_editable {
        let family = crate::inline_flow::shaping::font_family_for(style);
        let weight = crate::inline_flow::shaping::font_weight_for(style);
        crate::inline_flow::shaping::used_line_height(style, family, weight)
    } else {
        0.0
    };
    let mut final_h = if is_height_content {
        if !is_vertical_wm && is_editable {
            default_line_h
        } else {
            0.0
        }
    } else {
        content_height
    };
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
        final_h = final_h.max(min_content_h);
    }
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
        final_h = final_h.min(max_content_h);
    }
    container.dimensions.content.size.height = final_h;
    if is_vertical_wm && is_editable {
        container.dimensions.content.size.width =
            container.dimensions.content.size.width.max(default_line_h);
    }
}

pub fn resolve_container_cross_bounds(
    style: &ComputedStyle,
    is_row: bool,
    font_size: f32,
    root_font_size: f32,
    container_width: f32,
    container_height: f32,
    total_non_content_h: f32,
    total_non_content_v: f32,
) -> (f32, f32) {
    if is_row {
        let min_c = if style.min_height != Length::Auto {
            let min_h = style.min_height.to_px_with_viewport(
                font_size,
                root_font_size,
                container_height,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (min_h - total_non_content_v).max(0.0)
            } else {
                min_h
            }
        } else {
            0.0
        };
        let max_c = if style.max_height != Length::Auto {
            let max_h = style.max_height.to_px_with_viewport(
                font_size,
                root_font_size,
                container_height,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (max_h - total_non_content_v).max(0.0)
            } else {
                max_h
            }
        } else {
            f32::INFINITY
        };
        (min_c, max_c)
    } else {
        let min_c = if style.min_width != Length::Auto {
            let min_w = style.min_width.to_px_with_viewport(
                font_size,
                root_font_size,
                container_width,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (min_w - total_non_content_h).max(0.0)
            } else {
                min_w
            }
        } else {
            0.0
        };
        let max_c = if style.max_width != Length::Auto {
            let max_w = style.max_width.to_px_with_viewport(
                font_size,
                root_font_size,
                container_width,
                container_height,
            );
            if style.box_sizing == BoxSizing::BorderBox {
                (max_w - total_non_content_h).max(0.0)
            } else {
                max_w
            }
        } else {
            f32::INFINITY
        };
        (min_c, max_c)
    }
}
