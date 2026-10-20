use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{BorderWidths, DisplayCommand, DisplayList};

use crate::box_tree::LayoutBox;

use super::types::{apply_opacity, lookup_image_cached};

pub(crate) fn paint_borders(
    box_node: &LayoutBox,
    style: Option<&ComputedStyle>,
    border_box: Rect,
    opacity: f32,
    list: &mut DisplayList,
) {
    // 2. Paint Borders (on border box)
    if let Some(s) = style {
        let widths = BorderWidths {
            top: box_node.dimensions.border.top,
            right: box_node.dimensions.border.right,
            bottom: box_node.dimensions.border.bottom,
            left: box_node.dimensions.border.left,
        };

        let has_borders =
            widths.top > 0.0 || widths.right > 0.0 || widths.bottom > 0.0 || widths.left > 0.0;

        if has_borders {
            let mut painted_border_image = false;
            if let Some(bi) = &s.border_image
                && let Some(src) = &bi.source
                && let Some(img) = lookup_image_cached(src)
            {
                let img_w = img.width as f32;
                let img_h = img.height as f32;
                let slice_top = match bi.slice[0] {
                    mango_css::values::Length::Percent(p) => img_h * p / 100.0,
                    mango_css::values::Length::Px(px) => px,
                    _ => img_h,
                };
                let slice_right = match bi.slice[1] {
                    mango_css::values::Length::Percent(p) => img_w * p / 100.0,
                    mango_css::values::Length::Px(px) => px,
                    _ => img_w,
                };
                let slice_bottom = match bi.slice[2] {
                    mango_css::values::Length::Percent(p) => img_h * p / 100.0,
                    mango_css::values::Length::Px(px) => px,
                    _ => img_h,
                };
                let slice_left = match bi.slice[3] {
                    mango_css::values::Length::Percent(p) => img_w * p / 100.0,
                    mango_css::values::Length::Px(px) => px,
                    _ => img_w,
                };

                list.push(DisplayCommand::DrawBorderImage {
                    rect: border_box,
                    pixels: img.pixels.clone().into(),
                    img_width: img.width,
                    img_height: img.height,
                    slice: [slice_top, slice_right, slice_bottom, slice_left],
                    widths,
                    repeat_h: bi.repeat_h,
                    repeat_v: bi.repeat_v,
                    fill: bi.fill,
                });
                painted_border_image = true;
            }

            if !painted_border_image {
                let s_top_color = apply_opacity(s.border_top_color, opacity);
                let s_right_color = apply_opacity(s.border_right_color, opacity);
                let s_bottom_color = apply_opacity(s.border_bottom_color, opacity);
                let s_left_color = apply_opacity(s.border_left_color, opacity);

                let has_radii = s.has_border_radius();
                let same_colors = s_top_color == s_right_color
                    && s_top_color == s_bottom_color
                    && s_top_color == s_left_color;

                if has_radii || same_colors {
                    let border_color = if widths.top > 0.0 {
                        s_top_color
                    } else if widths.bottom > 0.0 {
                        s_bottom_color
                    } else if widths.left > 0.0 {
                        s_left_color
                    } else {
                        s_right_color
                    };
                    let border_radii = if has_radii {
                        s.border_radius_with_size(border_box.width(), border_box.height())
                    } else {
                        [0.0; 4]
                    };
                    list.push(DisplayCommand::DrawBorder {
                        rect: border_box,
                        color: border_color,
                        widths,
                        radii: border_radii,
                    });
                } else {
                    // Rectangular borders with distinct side colors: emit per-side DrawBorder.
                    // If the element has zero (or near-zero) content dimensions, borders form CSS triangles
                    // meeting at the apex; preserve adjacent widths so the triangle slope can be rasterized.
                    let is_zero_content = box_node.dimensions.content.size.width <= 0.5
                        && box_node.dimensions.content.size.height <= 0.5;

                    if widths.top > 0.0
                        && s.border_top_style != mango_css::values::BorderStyle::None
                    {
                        list.push(DisplayCommand::DrawBorder {
                            rect: border_box,
                            color: s_top_color,
                            widths: BorderWidths {
                                top: widths.top,
                                right: if is_zero_content { widths.right } else { 0.0 },
                                bottom: 0.0,
                                left: if is_zero_content { widths.left } else { 0.0 },
                            },
                            radii: [0.0; 4],
                        });
                    }
                    if widths.right > 0.0
                        && s.border_right_style != mango_css::values::BorderStyle::None
                    {
                        list.push(DisplayCommand::DrawBorder {
                            rect: border_box,
                            color: s_right_color,
                            widths: BorderWidths {
                                top: if is_zero_content { widths.top } else { 0.0 },
                                right: widths.right,
                                bottom: if is_zero_content { widths.bottom } else { 0.0 },
                                left: 0.0,
                            },
                            radii: [0.0; 4],
                        });
                    }
                    if widths.bottom > 0.0
                        && s.border_bottom_style != mango_css::values::BorderStyle::None
                    {
                        list.push(DisplayCommand::DrawBorder {
                            rect: border_box,
                            color: s_bottom_color,
                            widths: BorderWidths {
                                top: 0.0,
                                right: if is_zero_content { widths.right } else { 0.0 },
                                bottom: widths.bottom,
                                left: if is_zero_content { widths.left } else { 0.0 },
                            },
                            radii: [0.0; 4],
                        });
                    }
                    if widths.left > 0.0
                        && s.border_left_style != mango_css::values::BorderStyle::None
                    {
                        list.push(DisplayCommand::DrawBorder {
                            rect: border_box,
                            color: s_left_color,
                            widths: BorderWidths {
                                top: if is_zero_content { widths.top } else { 0.0 },
                                right: 0.0,
                                bottom: if is_zero_content { widths.bottom } else { 0.0 },
                                left: widths.left,
                            },
                            radii: [0.0; 4],
                        });
                    }
                }
            }
        }

        // 2b. Paint Outline (on border box + outline-offset, does not affect layout geometry)
        if !matches!(
            s.outline_style,
            mango_css::values::BorderStyle::None | mango_css::values::BorderStyle::Hidden
        ) && s.outline_width > 0.0
            && border_box.width() > 0.0
            && border_box.height() > 0.0
        {
            let off = s.outline_offset;
            let ow = s.outline_width;
            let outline_rect = Rect::new(
                border_box.x() - off - ow,
                border_box.y() - off - ow,
                border_box.width() + 2.0 * (off + ow),
                border_box.height() + 2.0 * (off + ow),
            );
            let outline_color = apply_opacity(s.outline_color, opacity);
            let outline_radii = if s.has_border_radius() {
                let r = s.border_radius_with_size(border_box.width(), border_box.height());
                [
                    r[0] + off + ow,
                    r[1] + off + ow,
                    r[2] + off + ow,
                    r[3] + off + ow,
                ]
            } else {
                [0.0; 4]
            };
            list.push(DisplayCommand::DrawBorder {
                rect: outline_rect,
                color: outline_color,
                widths: BorderWidths::all(ow),
                radii: outline_radii,
            });
        }
    }
}
