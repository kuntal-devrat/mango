use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::types::{apply_opacity, format_media_time};

pub(crate) fn paint_media(
    box_node: &LayoutBox,
    style: Option<&ComputedStyle>,
    _border_box: Rect,
    content_x: f32,
    content_y: f32,
    opacity: f32,
    list: &mut DisplayList,
) {
    // 3b. Paint Replaced Element (e.g. <img>) with object-fit / object-position support
    if let BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    } = &box_node.box_type
    {
        let draw_w = if box_node.dimensions.content.width() > 0.0 {
            box_node.dimensions.content.width()
        } else {
            *intrinsic_width
        };
        let draw_h = if box_node.dimensions.content.height() > 0.0 {
            box_node.dimensions.content.height()
        } else {
            *intrinsic_height
        };

        // Resolve object-fit mode
        let object_fit = style
            .map(|s| s.object_fit)
            .unwrap_or(mango_css::values::ObjectFit::Fill);

        // Compute the rendered image dimensions based on object-fit
        let src_ratio = if *intrinsic_height > 0.0 {
            *intrinsic_width / *intrinsic_height
        } else {
            1.0
        };
        let dst_ratio = if draw_h > 0.0 { draw_w / draw_h } else { 1.0 };

        let (render_w, render_h) = match object_fit {
            mango_css::values::ObjectFit::Fill => (draw_w, draw_h),
            mango_css::values::ObjectFit::Contain => {
                if src_ratio > dst_ratio {
                    // Wider than box: fit to width
                    (draw_w, draw_w / src_ratio)
                } else {
                    // Taller than box: fit to height
                    (draw_h * src_ratio, draw_h)
                }
            }
            mango_css::values::ObjectFit::Cover => {
                if src_ratio > dst_ratio {
                    // Wider than box: fit to height (overflow width)
                    (draw_h * src_ratio, draw_h)
                } else {
                    // Taller than box: fit to width (overflow height)
                    (draw_w, draw_w / src_ratio)
                }
            }
            mango_css::values::ObjectFit::None => (*intrinsic_width, *intrinsic_height),
            mango_css::values::ObjectFit::ScaleDown => {
                // Use contain if it would produce a smaller image, otherwise none
                let contain_dims = if src_ratio > dst_ratio {
                    (draw_w, draw_w / src_ratio)
                } else {
                    (draw_h * src_ratio, draw_h)
                };
                if contain_dims.0 < *intrinsic_width || contain_dims.1 < *intrinsic_height {
                    contain_dims
                } else {
                    (*intrinsic_width, *intrinsic_height)
                }
            }
        };

        // Resolve object-position offsets (default: 50% 50% = centered)
        let (pos_x_len, pos_y_len) = style.map(|s| s.object_position).unwrap_or((
            mango_css::values::Length::Percent(50.0),
            mango_css::values::Length::Percent(50.0),
        ));
        let fs = style.map(|s| s.font_size).unwrap_or(16.0);
        let offset_x_pos = match pos_x_len {
            mango_css::values::Length::Percent(pct) => (draw_w - render_w) * (pct / 100.0),
            other => other.to_px(fs, 16.0, draw_w),
        };
        let offset_y_pos = match pos_y_len {
            mango_css::values::Length::Percent(pct) => (draw_h - render_h) * (pct / 100.0),
            other => other.to_px(fs, 16.0, draw_h),
        };

        let img_x = content_x + offset_x_pos;
        let img_y = content_y + offset_y_pos;

        let target_w = render_w.round().max(1.0) as u32;
        let target_h = render_h.round().max(1.0) as u32;
        let src_w = intrinsic_width.round().max(1.0) as u32;
        let src_h = intrinsic_height.round().max(1.0) as u32;

        let dynamic_pixels = box_node.get_attribute("src").and_then(|src| {
            let trimmed = src.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Some(img) = mango_render::image_decode::decode_data_uri(trimmed) {
                return Some(
                    mango_render::image_decode::get_or_resize_cached(
                        trimmed, &img, target_w, target_h,
                    )
                    .pixels,
                );
            }
            if let Some(img) = mango_render::image_decode::get_cached_image(trimmed) {
                return Some(
                    mango_render::image_decode::get_or_resize_cached(
                        trimmed, &img, target_w, target_h,
                    )
                    .pixels,
                );
            }
            if trimmed.starts_with("//") {
                let https_url = format!("https:{trimmed}");
                if let Some(img) = mango_render::image_decode::get_cached_image(&https_url) {
                    return Some(
                        mango_render::image_decode::get_or_resize_cached(
                            &https_url, &img, target_w, target_h,
                        )
                        .pixels,
                    );
                }
            }
            None
        });

        let final_pixels = if let Some(px) = dynamic_pixels {
            px
        } else if (src_w != target_w || src_h != target_h) && !pixels.is_empty() {
            let img = mango_render::image_decode::DecodedImage {
                width: src_w,
                height: src_h,
                pixels: pixels.clone(),
            };
            let cache_key = box_node.get_attribute("src").unwrap_or("");
            if !cache_key.is_empty() {
                mango_render::image_decode::get_or_resize_cached(
                    cache_key, &img, target_w, target_h,
                )
                .pixels
            } else {
                img.resize(target_w, target_h).pixels
            }
        } else {
            pixels.clone()
        };

        // For cover mode, clip to the element box to hide overflow
        let needs_clip = matches!(
            object_fit,
            mango_css::values::ObjectFit::Cover | mango_css::values::ObjectFit::None
        ) && (render_w > draw_w || render_h > draw_h);

        if needs_clip {
            list.push(DisplayCommand::PushClip {
                rect: Rect::new(content_x, content_y, draw_w, draw_h),
            });
        }

        list.push(DisplayCommand::DrawImage {
            x: img_x,
            y: img_y,
            width: render_w,
            height: render_h,
            pixels: final_pixels.into(),
        });

        if needs_clip {
            list.push(DisplayCommand::PopClip);
        }
    }

    // 3d. Paint <iframe> Element (nested browsing context container)
    if let BoxType::IFrame {
        src,
        srcdoc: _,
        sandbox,
        intrinsic_width: _,
        intrinsic_height: _,
    } = &box_node.box_type
    {
        let frame_w = box_node.dimensions.content.width();
        let frame_h = box_node.dimensions.content.height();
        if frame_w > 0.0 && frame_h > 0.0 {
            let frame_rect = Rect::new(content_x, content_y, frame_w, frame_h);
            // HTML5 iframe browsing context viewport defaults to white background
            list.push(DisplayCommand::FillRect {
                rect: frame_rect,
                color: apply_opacity(Color::WHITE, opacity),
            });

            // If no DOM children were generated (e.g. external link or empty iframe),
            // render frame placeholder card with origin/URL and sandbox badge
            if box_node.children.is_empty() {
                render_iframe_embed_card(src, sandbox, frame_rect, opacity, list);
            }
        }
    }

    // 3e. Paint <video> Element
    if let BoxType::Video {
        src,
        has_controls,
        is_playing,
        is_muted,
        current_time,
        duration,
        intrinsic_width,
        intrinsic_height,
        poster_pixels,
        poster_width,
        poster_height,
        ..
    } = &box_node.box_type
    {
        let draw_w = if box_node.dimensions.content.width() > 0.0 {
            box_node.dimensions.content.width()
        } else {
            *intrinsic_width
        };
        let draw_h = if box_node.dimensions.content.height() > 0.0 {
            box_node.dimensions.content.height()
        } else {
            *intrinsic_height
        };
        if draw_w > 0.0 && draw_h > 0.0 {
            let video_rect = Rect::new(content_x, content_y, draw_w, draw_h);
            render_video_element(
                src,
                poster_pixels.as_deref(),
                *poster_width,
                *poster_height,
                *has_controls,
                *is_playing,
                *is_muted,
                *current_time,
                *duration,
                video_rect,
                opacity,
                list,
            );
        }
    }

    // 3f. Paint <audio> Element
    if let BoxType::Audio {
        src,
        has_controls,
        is_playing,
        is_muted,
        current_time,
        duration,
        intrinsic_width,
        intrinsic_height,
        ..
    } = &box_node.box_type
        && *has_controls
    {
        let draw_w = if box_node.dimensions.content.width() > 0.0 {
            box_node.dimensions.content.width()
        } else {
            *intrinsic_width
        };
        let draw_h = if box_node.dimensions.content.height() > 0.0 {
            box_node.dimensions.content.height()
        } else {
            *intrinsic_height
        };
        if draw_w > 0.0 && draw_h > 0.0 {
            let audio_rect = Rect::new(content_x, content_y, draw_w, draw_h);
            render_audio_element(
                src,
                *is_playing,
                *is_muted,
                *current_time,
                *duration,
                audio_rect,
                opacity,
                list,
            );
        }
    }

    // 3g. Paint <canvas> Element
    if let BoxType::Canvas {
        canvas_key,
        intrinsic_width,
        intrinsic_height,
        pixels,
        ..
    } = &box_node.box_type
    {
        let draw_w = if box_node.dimensions.content.width() > 0.0 {
            box_node.dimensions.content.width()
        } else {
            *intrinsic_width
        };
        let draw_h = if box_node.dimensions.content.height() > 0.0 {
            box_node.dimensions.content.height()
        } else {
            *intrinsic_height
        };
        if draw_w > 0.0 && draw_h > 0.0 {
            let active_pixels = if let Some(key) = canvas_key {
                mango_render::get_canvas_pixels(*key)
                    .map(|(_, _, px)| px)
                    .or_else(|| pixels.clone())
            } else {
                pixels.clone()
            };

            if let Some(px) = active_pixels {
                list.push(DisplayCommand::DrawImage {
                    x: content_x,
                    y: content_y,
                    width: draw_w,
                    height: draw_h,
                    pixels: px.into(),
                });
            }
        }
    }
}

/// Renders an embedded preview card for an `<iframe>` when child DOM is not rendered locally.
pub(crate) fn render_iframe_embed_card(
    src: &str,
    sandbox: &crate::box_model::IFrameSandbox,
    rect: Rect,
    opacity: f32,
    list: &mut DisplayList,
) {
    if rect.width() < 20.0 || rect.height() < 20.0 {
        return;
    }

    // Top status / address bar
    let header_h = 24.0f32.min(rect.height() * 0.4);
    let header_rect = Rect::new(rect.x(), rect.y(), rect.width(), header_h);
    list.push(DisplayCommand::FillRect {
        rect: header_rect,
        color: apply_opacity(Color::rgb(243, 244, 246), opacity),
    });

    // Divider line below header
    list.push(DisplayCommand::FillRect {
        rect: Rect::new(rect.x(), rect.y() + header_h - 1.0, rect.width(), 1.0),
        color: apply_opacity(Color::rgb(229, 231, 235), opacity),
    });

    // Badge / label text in header
    let display_title = if src.is_empty() {
        "about:blank".to_string()
    } else {
        src.to_string()
    };
    let title_font_size = 11.0f32;
    list.push(DisplayCommand::draw_text(
        format!("[Frame: {}]", display_title),
        rect.x() + 8.0,
        rect.y() + (header_h - title_font_size) / 2.0,
        apply_opacity(Color::rgb(75, 85, 99), opacity),
        title_font_size,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    ));

    // If sandboxed, render a sandbox security badge
    if sandbox.is_sandboxed && rect.height() > header_h + 20.0 {
        let badge_y = rect.y() + header_h + 8.0;
        list.push(DisplayCommand::draw_text(
            "🔒 Sandboxed context (restricted permissions)",
            rect.x() + 8.0,
            badge_y,
            apply_opacity(Color::rgb(156, 163, 175), opacity),
            10.0,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
        ));
    }
}

/// Renders an HTML5 `<video>` element with dark cinematic canvas, poster image, and controls bar.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_video_element(
    src: &str,
    poster_pixels: Option<&[u32]>,
    poster_w: u32,
    poster_h: u32,
    has_controls: bool,
    is_playing: bool,
    is_muted: bool,
    current_time: f32,
    duration: f32,
    rect: Rect,
    opacity: f32,
    list: &mut DisplayList,
) {
    if rect.width() < 10.0 || rect.height() < 10.0 {
        return;
    }

    // HTML5 §4.8.9: When a video element represents nothing (empty src, no poster, no controls),
    // it should be transparent and not render a default surface.
    if src.is_empty() && poster_pixels.is_none() && !has_controls {
        return;
    }

    // 1. Default video surface (Chromium SK_ColorDKGRAY #333333 = (51, 51, 51))
    list.push(DisplayCommand::FillRect {
        rect,
        color: apply_opacity(Color::rgb(51, 51, 51), opacity),
    });

    // 2. Poster image (if provided and decoded)
    let mut painted_poster = false;
    if let Some(pixels) = poster_pixels
        && poster_w > 0
        && poster_h > 0
    {
        list.push(DisplayCommand::DrawImage {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
            pixels: pixels.to_vec().into(),
        });
        painted_poster = true;
    }

    // 3. Center play overlay badge (if not playing and controls are requested)
    if !is_playing && has_controls && !src.is_empty() {
        let overlay_y_offset = if has_controls && rect.height() >= 40.0 {
            16.0
        } else {
            0.0
        };
        let center_x = rect.x() + rect.width() / 2.0;
        let center_y = rect.y() + rect.height() / 2.0 - overlay_y_offset;

        let badge_size = 48.0f32.min(rect.height() * 0.5).min(rect.width() * 0.5);
        if badge_size >= 20.0 {
            let badge_rect = Rect::new(
                center_x - badge_size / 2.0,
                center_y - badge_size / 2.0,
                badge_size,
                badge_size,
            );
            let r = badge_size / 2.0;
            list.push(DisplayCommand::FillRoundedRect {
                rect: badge_rect,
                radii: [r, r, r, r],
                color: apply_opacity(Color::rgb(24, 24, 27), opacity * 0.85),
            });
            list.push(DisplayCommand::DrawBorder {
                rect: badge_rect,
                color: apply_opacity(Color::rgb(255, 255, 255), opacity * 0.3),
                widths: mango_render::BorderWidths {
                    top: 1.5,
                    right: 1.5,
                    bottom: 1.5,
                    left: 1.5,
                },
                radii: [r, r, r, r],
            });
            let play_font_size = badge_size * 0.45;
            list.push(DisplayCommand::draw_text(
                "▶",
                center_x - play_font_size * 0.35,
                center_y - play_font_size * 0.5,
                apply_opacity(Color::WHITE, opacity),
                play_font_size,
                mango_render::FontWeight::Bold,
                mango_render::FontFamily::SansSerif,
            ));
        }

        // Title / source indicator at top-left
        if !painted_poster && rect.height() >= 60.0 && rect.width() >= 120.0 {
            let label = if src.is_empty() {
                "HTML5 Video".to_string()
            } else if let Some(filename) = src.split('/').next_back() {
                if !filename.is_empty() {
                    format!("Video: {}", filename)
                } else {
                    "HTML5 Video".to_string()
                }
            } else {
                "HTML5 Video".to_string()
            };
            list.push(DisplayCommand::draw_text(
                &label,
                rect.x() + 10.0,
                rect.y() + 10.0,
                apply_opacity(Color::rgb(209, 213, 219), opacity),
                11.0,
                mango_render::FontWeight::Regular,
                mango_render::FontFamily::SansSerif,
            ));
        }
    }

    // 4. Native Control Bar (when controls attribute is active)
    if has_controls && rect.height() >= 36.0 {
        let bar_h = 36.0f32.min(rect.height() * 0.35);
        let bar_y = rect.y() + rect.height() - bar_h;
        let bar_rect = Rect::new(rect.x(), bar_y, rect.width(), bar_h);

        // Dark translucent glass background
        list.push(DisplayCommand::FillRect {
            rect: bar_rect,
            color: apply_opacity(Color::rgb(15, 23, 42), opacity * 0.95),
        });
        // Separator top line
        list.push(DisplayCommand::FillRect {
            rect: Rect::new(rect.x(), bar_y, rect.width(), 1.0),
            color: apply_opacity(Color::rgb(51, 65, 85), opacity),
        });

        // 4a. Play / Pause button
        let play_glyph = if is_playing { "❚❚" } else { "▶" };
        list.push(DisplayCommand::draw_text(
            play_glyph,
            rect.x() + 12.0,
            bar_y + (bar_h - 13.0) / 2.0,
            apply_opacity(Color::WHITE, opacity),
            13.0,
            mango_render::FontWeight::Bold,
            mango_render::FontFamily::SansSerif,
        ));

        // 4b. Current time / Duration text
        let time_str = format!(
            "{} / {}",
            format_media_time(current_time),
            format_media_time(duration)
        );
        list.push(DisplayCommand::draw_text(
            &time_str,
            rect.x() + 34.0,
            bar_y + (bar_h - 11.0) / 2.0,
            apply_opacity(Color::rgb(156, 163, 175), opacity),
            11.0,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
        ));

        // 4c. Scrubber track
        let track_start = rect.x() + 115.0;
        let track_end = (rect.x() + rect.width() - 75.0).max(track_start + 20.0);
        let track_w = track_end - track_start;
        let track_y = bar_y + (bar_h - 4.0) / 2.0;

        // Background track line
        list.push(DisplayCommand::FillRect {
            rect: Rect::new(track_start, track_y, track_w, 4.0),
            color: apply_opacity(Color::rgb(75, 85, 99), opacity),
        });

        // Played progress fill
        let progress_ratio = if duration > 0.0 {
            (current_time / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let played_w = track_w * progress_ratio;
        list.push(DisplayCommand::FillRect {
            rect: Rect::new(track_start, track_y, played_w, 4.0),
            color: apply_opacity(Color::rgb(239, 68, 68), opacity),
        });

        // Knob indicator
        list.push(DisplayCommand::FillRect {
            rect: Rect::new(track_start + played_w - 3.0, track_y - 3.0, 7.0, 10.0),
            color: apply_opacity(Color::WHITE, opacity),
        });

        // 4d. Mute button icon
        let mute_glyph = if is_muted { "🔇" } else { "🔊" };
        list.push(DisplayCommand::draw_text(
            mute_glyph,
            rect.x() + rect.width() - 65.0,
            bar_y + (bar_h - 12.0) / 2.0,
            apply_opacity(Color::rgb(209, 213, 219), opacity),
            12.0,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
        ));

        // 4e. Fullscreen icon
        list.push(DisplayCommand::draw_text(
            "⛶",
            rect.x() + rect.width() - 30.0,
            bar_y + (bar_h - 12.0) / 2.0,
            apply_opacity(Color::rgb(156, 163, 175), opacity),
            12.0,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
        ));
    }
}

/// Renders an HTML5 `<audio>` element with sleek dark slate container, play circle, and audio track.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_audio_element(
    _src: &str,
    is_playing: bool,
    is_muted: bool,
    current_time: f32,
    duration: f32,
    rect: Rect,
    opacity: f32,
    list: &mut DisplayList,
) {
    if rect.width() < 20.0 || rect.height() < 10.0 {
        return;
    }

    // 1. Sleek rounded player card (#1e293b with #334155 border)
    let radius = 8.0f32.min(rect.height() / 2.0);
    list.push(DisplayCommand::FillRoundedRect {
        rect,
        radii: [radius, radius, radius, radius],
        color: apply_opacity(Color::rgb(30, 41, 59), opacity),
    });
    list.push(DisplayCommand::DrawBorder {
        rect,
        color: apply_opacity(Color::rgb(51, 65, 85), opacity),
        widths: mango_render::BorderWidths {
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
            left: 1.0,
        },
        radii: [radius, radius, radius, radius],
    });

    // 2. Play / Pause Button Circle (#3b82f6)
    let btn_size = 24.0f32.min(rect.height() - 8.0);
    let btn_rect = Rect::new(
        rect.x() + 8.0,
        rect.y() + (rect.height() - btn_size) / 2.0,
        btn_size,
        btn_size,
    );
    let btn_r = btn_size / 2.0;
    list.push(DisplayCommand::FillRoundedRect {
        rect: btn_rect,
        radii: [btn_r, btn_r, btn_r, btn_r],
        color: apply_opacity(Color::rgb(59, 130, 246), opacity),
    });

    let play_glyph = if is_playing { "❚❚" } else { "▶" };
    let glyph_size = btn_size * 0.45;
    list.push(DisplayCommand::draw_text(
        play_glyph,
        btn_rect.x() + (btn_size - glyph_size) / 2.0 + if is_playing { 0.0 } else { 1.5 },
        btn_rect.y() + (btn_size - glyph_size) / 2.0,
        apply_opacity(Color::WHITE, opacity),
        glyph_size,
        mango_render::FontWeight::Bold,
        mango_render::FontFamily::SansSerif,
    ));

    // 3. Time text (e.g. "0:00 / 3:30")
    let time_str = format!(
        "{} / {}",
        format_media_time(current_time),
        format_media_time(duration)
    );
    list.push(DisplayCommand::draw_text(
        &time_str,
        rect.x() + 38.0,
        rect.y() + (rect.height() - 11.0) / 2.0,
        apply_opacity(Color::rgb(203, 213, 225), opacity),
        11.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    ));

    // 4. Progress / waveform track
    let track_start = rect.x() + 115.0;
    let track_end = (rect.x() + rect.width() - 45.0).max(track_start + 15.0);
    let track_w = track_end - track_start;
    let track_y = rect.y() + (rect.height() - 4.0) / 2.0;

    // Track background
    list.push(DisplayCommand::FillRect {
        rect: Rect::new(track_start, track_y, track_w, 4.0),
        color: apply_opacity(Color::rgb(71, 85, 105), opacity),
    });

    // Active progress
    let progress_ratio = if duration > 0.0 {
        (current_time / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let played_w = track_w * progress_ratio;
    list.push(DisplayCommand::FillRect {
        rect: Rect::new(track_start, track_y, played_w, 4.0),
        color: apply_opacity(Color::rgb(56, 189, 248), opacity),
    });
    list.push(DisplayCommand::FillRect {
        rect: Rect::new(track_start + played_w - 2.0, track_y - 2.0, 5.0, 8.0),
        color: apply_opacity(Color::WHITE, opacity),
    });

    // 5. Volume/Mute button
    let mute_glyph = if is_muted { "🔇" } else { "🔊" };
    list.push(DisplayCommand::draw_text(
        mute_glyph,
        rect.x() + rect.width() - 36.0,
        rect.y() + (rect.height() - 12.0) / 2.0,
        apply_opacity(Color::rgb(203, 213, 225), opacity),
        12.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    ));
}
