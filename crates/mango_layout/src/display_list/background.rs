use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;

use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_tree::LayoutBox;

use super::types::{apply_opacity, lookup_image_cached};

pub(crate) fn paint_background(
    box_node: &LayoutBox,
    style: Option<&ComputedStyle>,
    border_box: Rect,
    pad_box: Rect,
    content_box: Rect,
    opacity: f32,
    scroll_x: f32,
    scroll_y: f32,
    list: &mut DisplayList,
) {
    // 0. Paint Box Shadow (before background)
    // Multiple shadows paint back-to-front (first shadow painted on top per CSS specs).
    if let Some(s) = style {
        let shadows = s.all_box_shadows();
        if !shadows.is_empty() {
            let shadow_base_rect = Rect::new(
                border_box.x(),
                border_box.y(),
                border_box.width(),
                border_box.height(),
            );
            if shadow_base_rect.width() > 0.0 && shadow_base_rect.height() > 0.0 {
                let radii =
                    s.border_radius_with_size(shadow_base_rect.width(), shadow_base_rect.height());
                for bs in shadows.iter().rev() {
                    let shadow_color = apply_opacity(bs.color, opacity);
                    list.push(DisplayCommand::DrawBoxShadow {
                        rect: shadow_base_rect,
                        color: shadow_color,
                        offset_x: bs.offset_x,
                        offset_y: bs.offset_y,
                        blur_radius: bs.blur_radius,
                        spread_radius: bs.spread_radius,
                        radii,
                        inset: bs.inset,
                    });
                }
            }
        }
    }

    // Modal dialog / popover backdrop: dimming overlay covering viewport
    let is_modal_dialog = box_node.tag_name.as_deref() == Some("dialog")
        && box_node.get_attribute("open").is_some()
        && box_node.get_attribute("data-mango-modal") == Some("true");
    let is_auto_popover = box_node.get_attribute("data-mango-popover-open") == Some("true")
        || box_node.get_attribute(":popover-open") == Some("true");

    if is_modal_dialog || is_auto_popover {
        let is_backdrop_hidden = box_node
            .get_attribute("_mango_backdrop_visibility")
            .is_some_and(|v| v.eq_ignore_ascii_case("Hidden"));
        if !is_backdrop_hidden {
            let custom_backdrop = box_node
                .get_attribute("_mango_backdrop_bg")
                .and_then(mango_css::values::Value::parse_color);
            let backdrop_rect = Rect::new(0.0, 0.0, 100_000.0, 100_000.0);
            let default_color = if is_modal_dialog {
                Color::rgba(0, 0, 0, 102) // 0.4 dimming backdrop
            } else {
                Color::TRANSPARENT // Popovers have transparent backdrop by default (WHATWG HTML § 15.3.3)
            };
            let backdrop_color = custom_backdrop.unwrap_or(default_color);
            if backdrop_color != Color::TRANSPARENT && backdrop_color.a > 0 {
                list.push(DisplayCommand::FillRect {
                    rect: backdrop_rect,
                    color: backdrop_color,
                });
            }
        }
    }

    let resolve_clip_rect = |clip: mango_css::values::BackgroundClip| -> Rect {
        match clip {
            mango_css::values::BackgroundClip::BorderBox => border_box,
            mango_css::values::BackgroundClip::PaddingBox => pad_box,
            mango_css::values::BackgroundClip::ContentBox => content_box,
            mango_css::values::BackgroundClip::Text => Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    };

    // 1. Paint Background (respecting background-clip)
    if let Some(s) = style
        && s.is_visible()
        && s.background_clip != mango_css::values::BackgroundClip::Text
    {
        let bg_color = apply_opacity(s.background_color, opacity);
        let base_rect = resolve_clip_rect(s.background_clip);
        if s.mask_image.is_none()
            && bg_color != Color::TRANSPARENT
            && bg_color.a > 0
            && base_rect.width() > 0.0
            && base_rect.height() > 0.0
        {
            if s.has_border_radius()
                && (s.background_clip == mango_css::values::BackgroundClip::BorderBox
                    || s.background_clip == mango_css::values::BackgroundClip::PaddingBox)
            {
                list.push(DisplayCommand::FillRoundedRect {
                    rect: base_rect,
                    color: bg_color,
                    radii: s.border_radius_with_size(base_rect.width(), base_rect.height()),
                });
            } else {
                list.push(DisplayCommand::FillRect {
                    rect: base_rect,
                    color: bg_color,
                });
            }
        }

        // Multi-layered or single background (bottom-to-top)
        if s.is_visible() && !s.background_layers.is_empty() {
            for layer in s.background_layers.iter().rev() {
                if layer.clip == mango_css::values::BackgroundClip::Text {
                    continue;
                }
                let target_rect = resolve_clip_rect(layer.clip);
                if target_rect.width() <= 0.0 || target_rect.height() <= 0.0 {
                    continue;
                }

                if let Some(gradient) = &layer.gradient {
                    let blend = s.background_blend_mode != mango_css::values::BlendMode::Normal;
                    if blend {
                        list.push(DisplayCommand::PushBlendMode {
                            mode: s.background_blend_mode,
                        });
                    }
                    let mut resolved_grad = (**gradient).clone();
                    resolved_grad.resolve_current_color(s.color);
                    list.push(DisplayCommand::FillGradient {
                        rect: target_rect,
                        gradient: Box::new(resolved_grad),
                        radii: if s.has_border_radius()
                            && layer.clip == mango_css::values::BackgroundClip::BorderBox
                        {
                            s.border_radius_with_size(target_rect.width(), target_rect.height())
                        } else {
                            [0.0; 4]
                        },
                        opacity,
                    });
                    if blend {
                        list.push(DisplayCommand::PopBlendMode);
                    }
                }

                if let Some(bg_url) = &layer.image {
                    paint_background_image_layer(
                        bg_url,
                        target_rect,
                        layer.size,
                        layer.position,
                        layer.repeat,
                        layer.attachment,
                        s.background_blend_mode,
                        scroll_x,
                        scroll_y,
                        list,
                    );
                }
            }
        } else {
            // Fall back to single gradient
            if let Some(gradient) = &s.background_gradient {
                let target_rect = resolve_clip_rect(s.background_clip);
                if target_rect.width() > 0.0 && target_rect.height() > 0.0 {
                    let blend = s.background_blend_mode != mango_css::values::BlendMode::Normal;
                    if blend {
                        list.push(DisplayCommand::PushBlendMode {
                            mode: s.background_blend_mode,
                        });
                    }
                    let mut resolved_grad = gradient.clone();
                    resolved_grad.resolve_current_color(s.color);
                    list.push(DisplayCommand::FillGradient {
                        rect: target_rect,
                        gradient: Box::new(resolved_grad),
                        radii: if s.has_border_radius() {
                            s.border_radius_with_size(target_rect.width(), target_rect.height())
                        } else {
                            [0.0; 4]
                        },
                        opacity,
                    });
                    if blend {
                        list.push(DisplayCommand::PopBlendMode);
                    }
                }
            }

            // Fall back to single background image
            if let Some(bg_url) = &s.background_image {
                let target_rect = resolve_clip_rect(s.background_clip);
                if target_rect.width() > 0.0 && target_rect.height() > 0.0 {
                    paint_background_image_layer(
                        bg_url,
                        target_rect,
                        s.background_size,
                        s.background_position,
                        s.background_repeat,
                        s.background_attachment,
                        s.background_blend_mode,
                        scroll_x,
                        scroll_y,
                        list,
                    );
                }
            }
        }
    }

    // 1c. Paint Masked Background (CSS mask-image / -webkit-mask-image)
    if let Some(s) = style
        && let Some(mask_url) = &s.mask_image
        && pad_box.width() > 0.0
        && pad_box.height() > 0.0
    {
        let pad_w = pad_box.width();
        let pad_h = pad_box.height();
        if let Some(img) = lookup_image_cached(mask_url) {
            let mask_size = s.mask_size.unwrap_or(s.background_size);
            let (draw_w, draw_h) = match mask_size {
                mango_css::values::BackgroundSize::Auto => (img.width as f32, img.height as f32),
                mango_css::values::BackgroundSize::Cover => {
                    let scale = (pad_w / img.width as f32).max(pad_h / img.height as f32);
                    (img.width as f32 * scale, img.height as f32 * scale)
                }
                mango_css::values::BackgroundSize::Contain => {
                    let scale = (pad_w / img.width as f32).min(pad_h / img.height as f32);
                    (img.width as f32 * scale, img.height as f32 * scale)
                }
                mango_css::values::BackgroundSize::Explicit(w, h) => {
                    let is_w_auto = matches!(w, mango_css::values::Length::Auto);
                    let is_h_auto = matches!(h, mango_css::values::Length::Auto);
                    let img_w = img.width as f32;
                    let img_h = img.height as f32;

                    if is_w_auto && is_h_auto {
                        (img_w, img_h)
                    } else if is_w_auto {
                        let height_px = match h {
                            mango_css::values::Length::Px(px) => px,
                            mango_css::values::Length::Percent(p) => pad_h * p / 100.0,
                            _ => img_h,
                        };
                        let width_px = if img_h > 0.0 {
                            height_px * (img_w / img_h)
                        } else {
                            img_w
                        };
                        (width_px, height_px)
                    } else if is_h_auto {
                        let width_px = match w {
                            mango_css::values::Length::Px(px) => px,
                            mango_css::values::Length::Percent(p) => pad_w * p / 100.0,
                            _ => img_w,
                        };
                        let height_px = if img_w > 0.0 {
                            width_px * (img_h / img_w)
                        } else {
                            img_h
                        };
                        (width_px, height_px)
                    } else {
                        let width_px = match w {
                            mango_css::values::Length::Px(px) => px,
                            mango_css::values::Length::Percent(p) => pad_w * p / 100.0,
                            _ => img_w,
                        };
                        let height_px = match h {
                            mango_css::values::Length::Px(px) => px,
                            mango_css::values::Length::Percent(p) => pad_h * p / 100.0,
                            _ => img_h,
                        };
                        (width_px, height_px)
                    }
                }
            };

            let target_w = draw_w.max(1.0);
            let target_h = draw_h.max(1.0);
            let resized = img.resize(target_w as u32, target_h as u32);

            let mask_pos = s.mask_position.unwrap_or(s.background_position);
            let origin_x = match mask_pos.0 {
                mango_css::values::Length::Px(px) => pad_box.x() + px,
                mango_css::values::Length::Percent(p) => pad_box.x() + (pad_w - draw_w) * p / 100.0,
                _ => pad_box.x() + (pad_w - draw_w) / 2.0,
            };
            let origin_y = match mask_pos.1 {
                mango_css::values::Length::Px(px) => pad_box.y() + px,
                mango_css::values::Length::Percent(p) => pad_box.y() + (pad_h - draw_h) * p / 100.0,
                _ => pad_box.y() + (pad_h - draw_h) / 2.0,
            };

            let tint = if s.background_color != Color::TRANSPARENT && s.background_color.a > 0 {
                apply_opacity(s.background_color, opacity)
            } else if s.color != Color::TRANSPARENT && s.color.a > 0 {
                apply_opacity(s.color, opacity)
            } else {
                apply_opacity(Color::BLACK, opacity)
            };

            let mut tinted_pixels = Vec::with_capacity(resized.pixels.len());
            for &pix in &resized.pixels {
                let a = match s.mask_mode {
                    mango_css::values::MaskMode::Luminance => {
                        let r = ((pix >> 16) & 0xFF) as f32;
                        let g = ((pix >> 8) & 0xFF) as f32;
                        let b = (pix & 0xFF) as f32;
                        (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
                    }
                    _ => ((pix >> 24) & 0xFF) as f32 / 255.0,
                };
                if a <= 0.0 {
                    tinted_pixels.push(0);
                } else {
                    let final_a = (a * (tint.a as f32 / 255.0) * 255.0).round() as u32;
                    let out_pixel = (final_a << 24)
                        | ((tint.r as u32) << 16)
                        | ((tint.g as u32) << 8)
                        | (tint.b as u32);
                    tinted_pixels.push(out_pixel);
                }
            }

            list.push(DisplayCommand::DrawImage {
                x: origin_x,
                y: origin_y,
                width: target_w,
                height: target_h,
                pixels: tinted_pixels.into(),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_background_image_layer(
    bg_url: &str,
    target_rect: Rect,
    bg_size: mango_css::values::BackgroundSize,
    bg_position: (mango_css::values::Length, mango_css::values::Length),
    bg_repeat: mango_css::values::BackgroundRepeat,
    bg_attachment: mango_css::values::BackgroundAttachment,
    blend_mode: mango_css::values::BlendMode,
    scroll_x: f32,
    scroll_y: f32,
    list: &mut DisplayList,
) {
    let blend = blend_mode != mango_css::values::BlendMode::Normal;
    if blend {
        list.push(DisplayCommand::PushBlendMode { mode: blend_mode });
    }
    let pad_w = target_rect.width();
    let pad_h = target_rect.height();

    if let Some(img) = lookup_image_cached(bg_url) {
        let (draw_w, draw_h) = match bg_size {
            mango_css::values::BackgroundSize::Auto => (img.width as f32, img.height as f32),
            mango_css::values::BackgroundSize::Cover => {
                let scale = (pad_w / img.width as f32).max(pad_h / img.height as f32);
                (img.width as f32 * scale, img.height as f32 * scale)
            }
            mango_css::values::BackgroundSize::Contain => {
                let scale = (pad_w / img.width as f32).min(pad_h / img.height as f32);
                (img.width as f32 * scale, img.height as f32 * scale)
            }
            mango_css::values::BackgroundSize::Explicit(w, h) => {
                let is_w_auto = matches!(w, mango_css::values::Length::Auto);
                let is_h_auto = matches!(h, mango_css::values::Length::Auto);
                let img_w = img.width as f32;
                let img_h = img.height as f32;

                if is_w_auto && is_h_auto {
                    (img_w, img_h)
                } else if is_w_auto {
                    let height_px = match h {
                        mango_css::values::Length::Px(px) => px,
                        mango_css::values::Length::Percent(p) => pad_h * p / 100.0,
                        _ => img_h,
                    };
                    let width_px = if img_h > 0.0 {
                        height_px * (img_w / img_h)
                    } else {
                        img_w
                    };
                    (width_px, height_px)
                } else if is_h_auto {
                    let width_px = match w {
                        mango_css::values::Length::Px(px) => px,
                        mango_css::values::Length::Percent(p) => pad_w * p / 100.0,
                        _ => img_w,
                    };
                    let height_px = if img_w > 0.0 {
                        width_px * (img_h / img_w)
                    } else {
                        img_h
                    };
                    (width_px, height_px)
                } else {
                    let width_px = match w {
                        mango_css::values::Length::Px(px) => px,
                        mango_css::values::Length::Percent(p) => pad_w * p / 100.0,
                        _ => img_w,
                    };
                    let height_px = match h {
                        mango_css::values::Length::Px(px) => px,
                        mango_css::values::Length::Percent(p) => pad_h * p / 100.0,
                        _ => img_h,
                    };
                    (width_px, height_px)
                }
            }
        };

        let target_w = draw_w.max(1.0);
        let target_h = draw_h.max(1.0);
        let resized = img.resize(target_w as u32, target_h as u32);

        let base_x = if bg_attachment == mango_css::values::BackgroundAttachment::Fixed {
            scroll_x
        } else {
            target_rect.x()
        };
        let base_y = if bg_attachment == mango_css::values::BackgroundAttachment::Fixed {
            scroll_y
        } else {
            target_rect.y()
        };

        let origin_x = match bg_position.0 {
            mango_css::values::Length::Px(px) => base_x + px,
            mango_css::values::Length::Percent(p) => base_x + (pad_w - draw_w) * p / 100.0,
            _ => base_x,
        };
        let origin_y = match bg_position.1 {
            mango_css::values::Length::Px(px) => base_y + px,
            mango_css::values::Length::Percent(p) => base_y + (pad_h - draw_h) * p / 100.0,
            _ => base_y,
        };

        match bg_repeat {
            mango_css::values::BackgroundRepeat::NoRepeat => {
                list.push(DisplayCommand::DrawImage {
                    x: origin_x,
                    y: origin_y,
                    width: target_w,
                    height: target_h,
                    pixels: resized.pixels.clone().into(),
                });
            }
            mango_css::values::BackgroundRepeat::RepeatX => {
                list.push(DisplayCommand::PushClip { rect: target_rect });
                let mut start_x = origin_x;
                while start_x > target_rect.x() {
                    start_x -= target_w;
                }
                let mut cur_x = start_x;
                let max_x = target_rect.right();
                let mut count = 0;
                while cur_x < max_x && count < 64 {
                    list.push(DisplayCommand::DrawImage {
                        x: cur_x,
                        y: origin_y,
                        width: target_w,
                        height: target_h,
                        pixels: resized.pixels.clone().into(),
                    });
                    cur_x += target_w;
                    count += 1;
                }
                list.push(DisplayCommand::PopClip);
            }
            mango_css::values::BackgroundRepeat::RepeatY => {
                list.push(DisplayCommand::PushClip { rect: target_rect });
                let mut start_y = origin_y;
                while start_y > target_rect.y() {
                    start_y -= target_h;
                }
                let mut cur_y = start_y;
                let max_y = target_rect.bottom();
                let mut count = 0;
                while cur_y < max_y && count < 64 {
                    list.push(DisplayCommand::DrawImage {
                        x: origin_x,
                        y: cur_y,
                        width: target_w,
                        height: target_h,
                        pixels: resized.pixels.clone().into(),
                    });
                    cur_y += target_h;
                    count += 1;
                }
                list.push(DisplayCommand::PopClip);
            }
            mango_css::values::BackgroundRepeat::Repeat => {
                list.push(DisplayCommand::PushClip { rect: target_rect });
                let mut start_y = origin_y;
                while start_y > target_rect.y() {
                    start_y -= target_h;
                }
                let mut start_x = origin_x;
                while start_x > target_rect.x() {
                    start_x -= target_w;
                }
                let mut cur_y = start_y;
                let max_y = target_rect.bottom();
                let max_x = target_rect.right();
                let mut total_tiles = 0;
                while cur_y < max_y && total_tiles < 128 {
                    let mut cur_x = start_x;
                    while cur_x < max_x && total_tiles < 128 {
                        list.push(DisplayCommand::DrawImage {
                            x: cur_x,
                            y: cur_y,
                            width: target_w,
                            height: target_h,
                            pixels: resized.pixels.clone().into(),
                        });
                        cur_x += target_w;
                        total_tiles += 1;
                    }
                    cur_y += target_h;
                }
                list.push(DisplayCommand::PopClip);
            }
        }
    }
    if blend {
        list.push(DisplayCommand::PopBlendMode);
    }
}
