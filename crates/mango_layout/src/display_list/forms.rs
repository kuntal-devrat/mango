use mango_core::{Color, Rect};
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::progress::{render_meter_element, render_progress_element};
use super::types::{apply_opacity, generate_search_icon, lookup_image_cached};

pub(crate) fn render_form_control(
    box_node: &LayoutBox,
    style: Option<&mango_css::computed::ComputedStyle>,
    opacity: f32,
    list: &mut DisplayList,
    offset_x: f32,
    offset_y: f32,
) {
    let Some(tag) = box_node.tag_name.as_deref() else {
        return;
    };
    let Some(s) = style else {
        return;
    };

    let content = Rect::new(
        box_node.dimensions.content.x() + offset_x,
        box_node.dimensions.content.y() + offset_y,
        box_node.dimensions.content.width(),
        box_node.dimensions.content.height(),
    );
    let border_box = Rect::new(
        box_node.dimensions.border_box().x() + offset_x,
        box_node.dimensions.border_box().y() + offset_y,
        box_node.dimensions.border_box().width(),
        box_node.dimensions.border_box().height(),
    );

    match tag {
        "input" => {
            let input_type = box_node
                .get_attribute("type")
                .unwrap_or("text")
                .to_ascii_lowercase();

            match input_type.as_str() {
                "checkbox" => {
                    let box_size = content.width().min(content.height()).clamp(10.0, 24.0);
                    let cx = content.x() + content.width() / 2.0;
                    let cy = content.y() + content.height() / 2.0;
                    let outer_rect =
                        Rect::new(cx - box_size / 2.0, cy - box_size / 2.0, box_size, box_size);

                    if box_node.get_attribute("checked").is_some() {
                        let accent = s.accent_color.unwrap_or(Color::rgb(0, 120, 215));
                        let check_color = apply_opacity(accent, opacity);
                        list.push(DisplayCommand::FillRoundedRect {
                            rect: outer_rect,
                            color: check_color,
                            radii: [2.0, 2.0, 2.0, 2.0],
                        });
                        list.push(DisplayCommand::draw_text(
                            "✓",
                            content.x() + 1.0,
                            content.y() - 1.0,
                            Color::WHITE,
                            10.0,
                            mango_render::FontWeight::Bold,
                            mango_render::FontFamily::SansSerif,
                        ));
                    } else {
                        let border_color = apply_opacity(Color::rgb(114, 119, 125), opacity);
                        list.push(DisplayCommand::FillRoundedRect {
                            rect: outer_rect,
                            color: border_color,
                            radii: [2.0, 2.0, 2.0, 2.0],
                        });
                        let inner_rect = Rect::new(
                            outer_rect.x() + 1.25,
                            outer_rect.y() + 1.25,
                            (box_size - 2.5).max(1.0),
                            (box_size - 2.5).max(1.0),
                        );
                        list.push(DisplayCommand::FillRoundedRect {
                            rect: inner_rect,
                            color: apply_opacity(Color::WHITE, opacity),
                            radii: [1.0, 1.0, 1.0, 1.0],
                        });
                    }
                }
                "radio" => {
                    let box_size = content.width().min(content.height()).clamp(10.0, 24.0);
                    let cx = content.x() + content.width() / 2.0;
                    let cy = content.y() + content.height() / 2.0;
                    let outer_rect =
                        Rect::new(cx - box_size / 2.0, cy - box_size / 2.0, box_size, box_size);
                    let r = box_size / 2.0;

                    let is_checked = box_node.get_attribute("checked").is_some();
                    let accent = s.accent_color.unwrap_or(Color::rgb(0, 120, 215));
                    let border_color = if is_checked {
                        apply_opacity(accent, opacity)
                    } else {
                        apply_opacity(Color::rgb(114, 119, 125), opacity)
                    };
                    let bg_color = apply_opacity(Color::WHITE, opacity);

                    // Outer border ring
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: outer_rect,
                        color: border_color,
                        radii: [r, r, r, r],
                    });
                    // Inner background circle
                    let border_width = 1.5f32;
                    let inner_w = (box_size - border_width * 2.0).max(2.0);
                    let inner_rect = Rect::new(
                        outer_rect.x() + border_width,
                        outer_rect.y() + border_width,
                        inner_w,
                        inner_w,
                    );
                    let inner_r = inner_w / 2.0;
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: inner_rect,
                        color: bg_color,
                        radii: [inner_r, inner_r, inner_r, inner_r],
                    });

                    // Inner dot if checked
                    if is_checked {
                        let dot_color = apply_opacity(accent, opacity);
                        let dot_w = (box_size * 0.45).round().max(4.0);
                        let dot_rect = Rect::new(cx - dot_w / 2.0, cy - dot_w / 2.0, dot_w, dot_w);
                        let dot_r = dot_w / 2.0;
                        list.push(DisplayCommand::FillRoundedRect {
                            rect: dot_rect,
                            color: dot_color,
                            radii: [dot_r, dot_r, dot_r, dot_r],
                        });
                    }
                }
                "submit" | "button" | "reset" => {
                    let label =
                        box_node
                            .get_attribute("value")
                            .unwrap_or(match input_type.as_str() {
                                "submit" => "Submit",
                                "reset" => "Reset",
                                _ => "Button",
                            });
                    if !label.is_empty() {
                        let font_size = s.font_size;
                        let is_bold =
                            crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                        let weight = if is_bold {
                            mango_render::FontWeight::Bold
                        } else {
                            mango_render::FontWeight::Regular
                        };
                        let family = mango_render::FontFamily::from_css_name(&s.font_family);
                        let text_w = crate::inline_flow::measure_text_width_with_style(
                            label, font_size, weight, family,
                        );
                        let x = content.x() + ((content.width() - text_w) / 2.0).max(0.0);
                        let y = content.y() + ((content.height() - font_size) / 2.0).max(0.0);
                        list.push(DisplayCommand::draw_text(
                            label,
                            x,
                            y,
                            apply_opacity(s.color, opacity),
                            font_size,
                            weight,
                            family,
                        ));
                    } else {
                        let has_bg_img = s
                            .background_image
                            .as_ref()
                            .and_then(|url| lookup_image_cached(url))
                            .is_some();
                        if !has_bg_img
                            && (box_node
                                .get_attribute("class")
                                .map(|c| c.contains("search__button"))
                                .unwrap_or(false)
                                || box_node
                                    .get_attribute("alt")
                                    .map(|a| a.eq_ignore_ascii_case("search"))
                                    .unwrap_or(false)
                                || box_node
                                    .get_attribute("title")
                                    .map(|t| t.eq_ignore_ascii_case("search"))
                                    .unwrap_or(false))
                        {
                            let box_w = border_box.width().max(content.width());
                            let box_h = border_box.height().max(content.height());
                            let icon_size = ((box_w.min(box_h) * 0.55) as u32).clamp(14, 20);
                            let x = border_box.x() + ((box_w - icon_size as f32) / 2.0).max(0.0);
                            let y = border_box.y() + ((box_h - icon_size as f32) / 2.0).max(0.0);
                            let is_colored_bg = s.background_color.a > 0
                                && s.background_color != Color::WHITE
                                && s.background_color != Color::TRANSPARENT;
                            let icon_color = if is_colored_bg {
                                apply_opacity(Color::WHITE, opacity)
                            } else {
                                apply_opacity(s.color, opacity)
                            };
                            let pixels = generate_search_icon(icon_size, icon_color);
                            list.push(DisplayCommand::DrawImage {
                                x,
                                y,
                                width: icon_size as f32,
                                height: icon_size as f32,
                                pixels: pixels.into(),
                            });
                        }
                    }
                }
                "range" => {
                    // Slider track + filled portion + draggable thumb
                    let min_v: f32 = box_node
                        .get_attribute("min")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0.0);
                    let max_v: f32 = box_node
                        .get_attribute("max")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(100.0);
                    let val: f32 = box_node
                        .get_attribute("value")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(min_v);
                    let lo = min_v.min(max_v);
                    let hi = min_v.max(max_v);
                    let span = hi - lo;
                    let frac = if span > f32::EPSILON {
                        ((val - lo) / span).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let track_h = (content.height() * 0.25).clamp(3.0, 6.0);
                    let track_y = content.y() + (content.height() - track_h) / 2.0;
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: Rect::new(content.x(), track_y, content.width().max(1.0), track_h),
                        color: apply_opacity(Color::rgb(196, 200, 205), opacity),
                        radii: [track_h / 2.0; 4],
                    });

                    let accent = s.accent_color.unwrap_or(Color::rgb(0, 120, 215));
                    let fill_w = (content.width() * frac).max(0.0);
                    if fill_w > 0.5 {
                        list.push(DisplayCommand::FillRoundedRect {
                            rect: Rect::new(content.x(), track_y, fill_w, track_h),
                            color: apply_opacity(accent, opacity),
                            radii: [track_h / 2.0; 4],
                        });
                    }

                    let thumb_r = (content.height() / 2.0 - 1.0).clamp(5.0, 9.0);
                    let thumb_x = (content.x() + fill_w - thumb_r).max(content.x());
                    let thumb_y = content.y() + (content.height() - thumb_r * 2.0) / 2.0;
                    let thumb = Rect::new(thumb_x, thumb_y, thumb_r * 2.0, thumb_r * 2.0);
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: thumb,
                        color: apply_opacity(accent, opacity),
                        radii: [thumb_r; 4],
                    });
                    list.push(DisplayCommand::DrawBorder {
                        rect: thumb,
                        color: apply_opacity(accent, opacity),
                        widths: mango_render::display_list::BorderWidths {
                            top: 1.0,
                            right: 1.0,
                            bottom: 1.0,
                            left: 1.0,
                        },
                        radii: [thumb_r; 4],
                    });
                }
                "color" => {
                    // Color swatch + hex label; clicking opens the palette picker.
                    let raw = box_node.get_attribute("value").unwrap_or("#000000");
                    let swatch_color =
                        mango_css::values::Value::parse_color(raw).unwrap_or(Color::rgb(0, 0, 0));
                    let inset = 2.0f32;
                    let sw = (content.width() * 0.45).clamp(24.0, 72.0).max(4.0);
                    let sh = (content.height() - inset * 2.0).max(4.0);
                    let swatch = Rect::new(content.x() + inset, content.y() + inset, sw, sh);
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: swatch,
                        color: apply_opacity(swatch_color, opacity),
                        radii: [3.0; 4],
                    });
                    list.push(DisplayCommand::DrawBorder {
                        rect: swatch,
                        color: apply_opacity(Color::rgb(120, 124, 128), opacity),
                        widths: mango_render::display_list::BorderWidths {
                            top: 1.0,
                            right: 1.0,
                            bottom: 1.0,
                            left: 1.0,
                        },
                        radii: [3.0; 4],
                    });

                    let label = if raw.starts_with('#') {
                        raw.to_ascii_uppercase()
                    } else {
                        raw.to_string()
                    };
                    let font_size = (s.font_size * 0.9).max(10.0);
                    let family = mango_render::FontFamily::from_css_name(&s.font_family);
                    list.push(DisplayCommand::draw_text(
                        &label,
                        swatch.right() + 6.0,
                        content.y() + ((content.height() - font_size) / 2.0).max(0.0),
                        apply_opacity(s.color, opacity),
                        font_size,
                        mango_render::FontWeight::Regular,
                        family,
                    ));
                }
                "file" => {
                    // "Choose file..." button + selected filename.
                    let filename = box_node
                        .get_attribute("data-mango-filename")
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            box_node
                                .get_attribute("value")
                                .map(|s| s.to_string())
                                .filter(|s| !s.is_empty())
                        })
                        .unwrap_or_default();

                    let font_size = s.font_size;
                    let family = mango_render::FontFamily::from_css_name(&s.font_family);
                    let btn_w = (content.width() * 0.4).clamp(84.0, 132.0).max(4.0);
                    let btn_h = (content.height() - 6.0).max(16.0);
                    let btn = Rect::new(
                        content.x() + 3.0,
                        content.y() + ((content.height() - btn_h) / 2.0).max(0.0),
                        btn_w,
                        btn_h,
                    );
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: btn,
                        color: apply_opacity(Color::rgb(240, 241, 243), opacity),
                        radii: [4.0; 4],
                    });
                    list.push(DisplayCommand::DrawBorder {
                        rect: btn,
                        color: apply_opacity(Color::rgb(180, 185, 195), opacity),
                        widths: mango_render::display_list::BorderWidths {
                            top: 1.0,
                            right: 1.0,
                            bottom: 1.0,
                            left: 1.0,
                        },
                        radii: [4.0; 4],
                    });

                    let btn_label = "Choose file...";
                    let btn_text_w = btn_label.len() as f32 * font_size * 0.55;
                    let btn_text_x = btn.x() + ((btn.width() - btn_text_w) / 2.0).max(3.0);
                    let text_y = content.y() + ((content.height() - font_size) / 2.0).max(0.0);
                    list.push(DisplayCommand::draw_text(
                        btn_label,
                        btn_text_x,
                        text_y,
                        apply_opacity(s.color, opacity),
                        font_size,
                        mango_render::FontWeight::Regular,
                        family,
                    ));

                    let name_x = btn.right() + 8.0;
                    if name_x + 20.0 < content.right() {
                        if filename.is_empty() {
                            list.push(DisplayCommand::draw_text(
                                "No file chosen",
                                name_x,
                                text_y,
                                apply_opacity(Color::rgb(120, 124, 128), opacity),
                                font_size,
                                mango_render::FontWeight::Regular,
                                family,
                            ));
                        } else {
                            list.push(DisplayCommand::draw_text(
                                &filename,
                                name_x,
                                text_y,
                                apply_opacity(s.color, opacity),
                                font_size,
                                mango_render::FontWeight::Regular,
                                family,
                            ));
                        }
                    }
                }
                _ => {
                    // Text / search / password / email / url / date / time / number / etc.
                    let val = box_node.get_attribute("value");
                    let placeholder = box_node.get_attribute("placeholder");
                    let is_focused = box_node.get_attribute("data-mango-focused") == Some("true");
                    let font_size = s.font_size;
                    let y = border_box.y() + ((border_box.height() - font_size) / 2.0).max(0.0);
                    let x = content.x().max(border_box.x() + 8.0);
                    let family = mango_render::FontFamily::from_css_name(&s.font_family);

                    if let Some(v) = val
                        && !v.is_empty()
                    {
                        let display_text = if input_type == "password" {
                            "•".repeat(v.chars().count())
                        } else {
                            v.to_string()
                        };

                        list.push(DisplayCommand::draw_text(
                            &display_text,
                            x,
                            y,
                            apply_opacity(s.color, opacity),
                            font_size,
                            mango_render::FontWeight::Regular,
                            family,
                        ));

                        if is_focused {
                            let cursor_pos = box_node
                                .get_attribute("_mango_cursor_pos")
                                .and_then(|s| s.parse::<usize>().ok())
                                .unwrap_or(display_text.chars().count());
                            let chars: Vec<char> = display_text.chars().collect();
                            let c_idx = cursor_pos.min(chars.len());
                            let text_before: String = chars[..c_idx].iter().collect();
                            let text_w = crate::inline_flow::measure_text_width_with_style(
                                &text_before,
                                font_size,
                                mango_render::FontWeight::Regular,
                                family,
                            );

                            let caret_x = x + text_w;
                            let caret_h =
                                (font_size * 1.15).min(border_box.height() - 4.0).max(12.0);
                            let caret_y =
                                border_box.y() + ((border_box.height() - caret_h) / 2.0).max(0.0);
                            list.push(DisplayCommand::FillRect {
                                rect: Rect::new(caret_x, caret_y, 1.5, caret_h),
                                color: apply_opacity(s.color, opacity),
                            });
                        }
                    } else if is_focused {
                        if let Some(ph) = placeholder
                            && !ph.is_empty()
                        {
                            let ph_color = box_node
                                .get_attribute("_mango_placeholder_color")
                                .and_then(mango_css::values::Value::parse_color)
                                .unwrap_or(Color::rgb(140, 140, 140));
                            let ph_opacity = box_node
                                .get_attribute("_mango_placeholder_opacity")
                                .and_then(|v| v.parse::<f32>().ok())
                                .unwrap_or(1.0);
                            if ph_color.a > 0 && ph_opacity > 0.0 {
                                list.push(DisplayCommand::draw_text(
                                    ph,
                                    x,
                                    y,
                                    apply_opacity(ph_color, opacity * ph_opacity),
                                    font_size,
                                    mango_render::FontWeight::Regular,
                                    family,
                                ));
                            }
                        }
                        let caret_h = (font_size * 1.15).min(border_box.height() - 4.0).max(12.0);
                        let caret_y =
                            border_box.y() + ((border_box.height() - caret_h) / 2.0).max(0.0);
                        list.push(DisplayCommand::FillRect {
                            rect: Rect::new(x, caret_y, 1.5, caret_h),
                            color: apply_opacity(s.color, opacity),
                        });
                    } else if let Some(ph) = placeholder
                        && !ph.is_empty()
                    {
                        let ph_color = box_node
                            .get_attribute("_mango_placeholder_color")
                            .and_then(mango_css::values::Value::parse_color)
                            .unwrap_or(Color::rgb(140, 140, 140));
                        let ph_opacity = box_node
                            .get_attribute("_mango_placeholder_opacity")
                            .and_then(|v| v.parse::<f32>().ok())
                            .unwrap_or(1.0);
                        if ph_color.a > 0 && ph_opacity > 0.0 {
                            list.push(DisplayCommand::draw_text(
                                ph,
                                x,
                                y,
                                apply_opacity(ph_color, opacity * ph_opacity),
                                font_size,
                                mango_render::FontWeight::Regular,
                                family,
                            ));
                        }
                    }

                    if input_type == "number" {
                        render_number_spinners(content, opacity, list);
                    }
                }
            }
        }
        "select" => {
            let font_size = s.font_size;
            let family = mango_render::FontFamily::from_css_name(&s.font_family);
            let is_multiple = box_node.get_attribute("_mango_is_multiple") == Some("true");
            let size_attr = box_node
                .get_attribute("size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(if is_multiple { 4 } else { 1 });

            let selected_text = box_node
                .get_attribute("_mango_selected_text")
                .or_else(|| box_node.get_attribute("value"))
                .unwrap_or("Select...");

            if is_multiple || size_attr > 1 {
                let items: Vec<&str> = selected_text
                    .split(", ")
                    .filter(|s| !s.is_empty())
                    .collect();
                let mut line_y = content.y() + 4.0;
                for (idx, item) in items.iter().enumerate().take(size_attr.max(3)) {
                    if idx > 0 {
                        line_y += font_size + 4.0;
                    }
                    if line_y + font_size > content.bottom() {
                        break;
                    }
                    let item_rect = Rect::new(
                        content.x() + 2.0,
                        line_y - 1.0,
                        content.width() - 4.0,
                        font_size + 2.0,
                    );
                    list.push(DisplayCommand::FillRoundedRect {
                        rect: item_rect,
                        color: apply_opacity(Color::rgb(204, 232, 255), opacity),
                        radii: [2.0; 4],
                    });
                    list.push(DisplayCommand::draw_text(
                        *item,
                        content.x() + 6.0,
                        line_y,
                        apply_opacity(Color::rgb(0, 50, 120), opacity),
                        font_size,
                        mango_render::FontWeight::Regular,
                        family,
                    ));
                }
            } else {
                let y = content.y() + ((content.height() - font_size) / 2.0).max(0.0);
                let x = content.x() + 6.0;

                list.push(DisplayCommand::draw_text(
                    selected_text,
                    x,
                    y,
                    apply_opacity(s.color, opacity),
                    font_size,
                    mango_render::FontWeight::Regular,
                    family,
                ));

                if s.appearance != mango_css::values::Appearance::None {
                    let arrow_x = (content.x() + 124.0)
                        .min(content.right() - 14.0)
                        .max(x + 10.0);
                    list.push(DisplayCommand::draw_text(
                        "▼",
                        arrow_x,
                        y + 1.0,
                        apply_opacity(Color::rgb(100, 100, 100), opacity),
                        9.0,
                        mango_render::FontWeight::Regular,
                        mango_render::FontFamily::SansSerif,
                    ));
                }
            }
        }
        "textarea" => {
            let val = box_node.get_attribute("value");
            let placeholder = box_node.get_attribute("placeholder");
            let font_size = s.font_size;
            let x = content.x() + 4.0;
            let y = content.y() + 4.0;
            let family = mango_render::FontFamily::from_css_name(&s.font_family);

            let has_child_text = box_node.children.iter().any(|c| match &c.box_type {
                BoxType::TextNode(t) => !t.trim().is_empty(),
                _ => c.children.iter().any(|gc| match &gc.box_type {
                    BoxType::TextNode(t) => !t.trim().is_empty(),
                    _ => false,
                }),
            });

            if !has_child_text {
                if let Some(v) = val
                    && !v.is_empty()
                {
                    list.push(DisplayCommand::draw_text(
                        v,
                        x,
                        y,
                        apply_opacity(s.color, opacity),
                        font_size,
                        mango_render::FontWeight::Regular,
                        family,
                    ));
                } else if let Some(ph) = placeholder
                    && !ph.is_empty()
                {
                    let ph_color = box_node
                        .get_attribute("_mango_placeholder_color")
                        .and_then(mango_css::values::Value::parse_color)
                        .unwrap_or(Color::rgb(140, 140, 140));
                    let ph_opacity = box_node
                        .get_attribute("_mango_placeholder_opacity")
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(1.0);
                    if ph_color.a > 0 && ph_opacity > 0.0 {
                        list.push(DisplayCommand::draw_text(
                            ph,
                            x,
                            y,
                            apply_opacity(ph_color, opacity * ph_opacity),
                            font_size,
                            mango_render::FontWeight::Regular,
                            family,
                        ));
                    }
                }
            }
        }
        "summary" => {
            let is_open = box_node.get_attribute("_mango_details_open") == Some("true");
            let marker = if is_open { "▼" } else { "►" };
            let font_size = s.font_size * 0.75;
            let marker_x = (content.x() - font_size - 4.0).max(2.0);
            let marker_y = content.y() + ((content.height() - font_size) / 2.0).max(0.0);
            list.push(DisplayCommand::draw_text(
                marker,
                marker_x,
                marker_y,
                apply_opacity(s.color, opacity),
                font_size,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
            ));
        }
        "progress" => {
            render_progress_element(box_node, s, opacity, list, content);
        }
        "meter" => {
            render_meter_element(box_node, s, opacity, list, content);
        }
        _ => {}
    }
}

pub(crate) fn render_number_spinners(content: Rect, opacity: f32, list: &mut DisplayList) {
    if content.width() < 28.0 || content.height() < 16.0 {
        return;
    }
    let arrow_font = (content.height() / 2.0 - 1.0).clamp(7.0, 11.0);
    let x = content.right() - arrow_font - 4.0;
    let half = content.height() / 2.0;
    let top_y = content.y() + ((half - arrow_font) / 2.0).max(0.0);
    let bottom_y = content.y() + half + ((half - arrow_font) / 2.0).max(0.0);
    let color = apply_opacity(Color::rgb(90, 94, 99), opacity);
    list.push(DisplayCommand::draw_text(
        "▲",
        x,
        top_y,
        color,
        arrow_font,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    ));
    list.push(DisplayCommand::draw_text(
        "▼",
        x,
        bottom_y,
        color,
        arrow_font,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    ));
}
