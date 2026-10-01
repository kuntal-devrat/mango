//! Painter: rasterizes display list commands into a pixel buffer.
//!
//! This is the bridge between the abstract display list and the raw pixel
//! buffer owned by `mango_platform`. The painter executes each command
//! in order, writing pixels into the buffer.
//!
//! ## Phase 2b Changes
//! - Removed `scroll_y` parameter — scroll offset is now handled entirely by `browser.rs`.
//! - Text rendering delegates to `FontManager` for anti-aliased TrueType glyphs.
//! - Added `DrawImage` support for decoded image blitting.

use mango_core::{Color, Rect};

use crate::display_list::{DisplayCommand, DisplayList};
use crate::font::{TextDecoration, font_manager};

/// Rasterizes a [`DisplayList`] into a pixel buffer.
///
/// The buffer is a flat `&mut [u32]` in `0xRRGGBB` format with the given
/// Intersects two rectangles, returning a rectangle with zero width/height if they do not overlap.
#[inline]
fn intersect_rect(a: mango_core::Rect, b: mango_core::Rect) -> mango_core::Rect {
    let x1 = a.x().max(b.x());
    let y1 = a.y().max(b.y());
    let x2 = (a.x() + a.width()).min(b.x() + b.width());
    let y2 = (a.y() + a.height()).min(b.y() + b.height());
    let w = (x2 - x1).max(0.0);
    let h = (y2 - y1).max(0.0);
    mango_core::Rect::new(x1, y1, w, h)
}

/// Rasterizes a [`DisplayList`] into a pixel buffer.
///
/// The buffer is a flat `&mut [u32]` in `0xRRGGBB` format with the given
/// width and height. Scroll offset is NOT applied here — the caller must
/// translate coordinates before building the display list.
pub fn paint(display_list: &DisplayList, buffer: &mut [u32], buf_width: u32, buf_height: u32) {
    let fm = font_manager();
    let mut clip_stack: Vec<mango_core::Rect> = Vec::new();
    let default_clip = mango_core::Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32);
    let mut transform_stack: Vec<[f32; 6]> = Vec::new();
    let mut blend_mode_stack: Vec<mango_css::values::BlendMode> = Vec::new();
    let mut filter_stack: Vec<(Vec<mango_css::values::FilterFunction>, mango_core::Rect)> =
        Vec::new();
    let mut clip_path_stack: Vec<(mango_css::values::ClipPath, mango_core::Rect, Vec<u32>)> =
        Vec::new();

    for command in display_list.iter() {
        let active_clip = clip_stack.last().copied().unwrap_or(default_clip);
        let transform = transform_stack.last().copied().unwrap_or(IDENTITY);
        let active_blend = blend_mode_stack
            .last()
            .copied()
            .unwrap_or(mango_css::values::BlendMode::Normal);
        match command {
            DisplayCommand::PushClip { rect } => {
                let current = clip_stack.last().copied().unwrap_or(default_clip);
                let intersected = intersect_rect(current, *rect);
                clip_stack.push(intersected);
            }
            DisplayCommand::PopClip => {
                clip_stack.pop();
            }
            DisplayCommand::PushTransform { matrix } => {
                transform_stack.push(concat_matrix(transform, *matrix));
            }
            DisplayCommand::PopTransform => {
                transform_stack.pop();
            }
            DisplayCommand::FillGradient {
                rect,
                gradient,
                radii,
                opacity,
            } => {
                fill_gradient(
                    buffer,
                    buf_width,
                    buf_height,
                    rect,
                    gradient,
                    *radii,
                    *opacity,
                    active_clip,
                    transform,
                );
            }
            DisplayCommand::DrawTextShadow {
                text,
                x,
                y,
                color,
                font_size,
                weight,
                family,
                style,
                blur_radius,
                letter_spacing,
            } => {
                if color.a == 0 || *font_size <= 0.5 || text.is_empty() {
                    continue;
                }
                // Proxy blur: draw the same run repeatedly through a small disc of
                // offsets with proportionally reduced alpha.
                let blur = blur_radius.max(0.0);
                if blur <= 0.5 {
                    fm.rasterize_text_clipped(
                        buffer,
                        buf_width,
                        buf_height,
                        *x as i32,
                        *y as i32,
                        text,
                        *color,
                        *font_size,
                        *weight,
                        *family,
                        *style,
                        *letter_spacing,
                        active_clip,
                    );
                } else {
                    let radius = (blur * 0.5).clamp(1.0, 6.0);
                    let steps = radius.ceil() as i32;
                    let samples = ((steps * 2 + 1) * (steps * 2 + 1)) as f32;
                    let base_alpha = color.a as f32;
                    let step_alpha = (base_alpha * 1.6 / samples).clamp(4.0, 255.0) as u8;
                    let faded = Color::rgba(color.r, color.g, color.b, step_alpha);
                    for dy in -steps..=steps {
                        for dx in -steps..=steps {
                            let dist = ((dx * dx + dy * dy) as f32).sqrt();
                            if dist > radius + 0.5 {
                                continue;
                            }
                            fm.rasterize_text_clipped(
                                buffer,
                                buf_width,
                                buf_height,
                                *x as i32 + dx,
                                *y as i32 + dy,
                                text,
                                faded,
                                *font_size,
                                *weight,
                                *family,
                                *style,
                                *letter_spacing,
                                active_clip,
                            );
                        }
                    }
                }
            }
            DisplayCommand::FillRect { rect, color } => {
                if !is_identity(transform) {
                    fill_rect_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        rect,
                        *color,
                        None,
                        active_clip,
                        transform,
                    );
                    continue;
                }
                let r = intersect_rect(active_clip, *rect);
                if r.width() > 0.0 && r.height() > 0.0 {
                    let x = r.x() as i32;
                    let y = r.y() as i32;
                    let w = r.width() as u32;
                    let h = r.height() as u32;
                    fill_rect_mode(
                        buffer,
                        buf_width,
                        buf_height,
                        x,
                        y,
                        w,
                        h,
                        *color,
                        active_blend,
                    );
                }
            }
            DisplayCommand::FillRoundedRect { rect, color, radii } => {
                if !is_identity(transform) {
                    fill_rect_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        rect,
                        *color,
                        Some(*radii),
                        active_clip,
                        transform,
                    );
                    continue;
                }
                fill_rounded_rect(
                    buffer,
                    buf_width,
                    buf_height,
                    rect,
                    *color,
                    *radii,
                    active_clip,
                );
            }
            DisplayCommand::DrawBoxShadow {
                rect,
                color,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                radii,
                inset,
            } => {
                if color.a == 0 {
                    continue;
                }
                if *inset {
                    let inset_clip = intersect_rect(active_clip, *rect);
                    if inset_clip.width() <= 0.0 || inset_clip.height() <= 0.0 {
                        continue;
                    }
                    let blur = blur_radius.max(0.0);
                    if blur <= 0.5 {
                        let inner_rect = mango_core::Rect::new(
                            rect.x() + offset_x + spread_radius,
                            rect.y() + offset_y + spread_radius,
                            (rect.width() - spread_radius * 2.0).max(0.0),
                            (rect.height() - spread_radius * 2.0).max(0.0),
                        );
                        let clip_out = Some((&inner_rect, *radii));
                        fill_rounded_rect_clipped(
                            buffer, buf_width, buf_height, rect, *color, *radii, clip_out,
                            inset_clip,
                        );
                    } else {
                        let steps = (blur.min(16.0) / 2.0).ceil().max(1.0) as i32;
                        let base_alpha = color.a as f32;
                        for step in (0..=steps).rev() {
                            let t = (step as f32) / (steps as f32);
                            let cur_spread = spread_radius + blur * t;
                            let step_alpha = ((base_alpha * (1.0 - t * 0.6)) / (steps as f32 + 1.0))
                                .clamp(0.0, 255.0)
                                as u8;
                            if step_alpha == 0 {
                                continue;
                            }
                            let step_color = Color::rgba(color.r, color.g, color.b, step_alpha);
                            let inner_rect = mango_core::Rect::new(
                                rect.x() + offset_x + cur_spread,
                                rect.y() + offset_y + cur_spread,
                                (rect.width() - cur_spread * 2.0).max(0.0),
                                (rect.height() - cur_spread * 2.0).max(0.0),
                            );
                            let clip_out = Some((&inner_rect, *radii));
                            fill_rounded_rect_clipped(
                                buffer, buf_width, buf_height, rect, step_color, *radii, clip_out,
                                inset_clip,
                            );
                        }
                    }
                    continue;
                }
                let clip_out = Some((rect, *radii));
                let blur = blur_radius.max(0.0);
                if blur <= 0.5 {
                    let shadow_rect = mango_core::Rect::new(
                        rect.x() + offset_x - spread_radius,
                        rect.y() + offset_y - spread_radius,
                        (rect.width() + spread_radius * 2.0).max(0.0),
                        (rect.height() + spread_radius * 2.0).max(0.0),
                    );
                    let r = intersect_rect(active_clip, shadow_rect);
                    if r.width() > 0.0 && r.height() > 0.0 {
                        fill_rounded_rect_clipped(
                            buffer,
                            buf_width,
                            buf_height,
                            &shadow_rect,
                            *color,
                            *radii,
                            clip_out,
                            active_clip,
                        );
                    }
                } else {
                    let steps = (blur.min(16.0) / 2.0).ceil().max(1.0) as i32;
                    let base_alpha = color.a as f32;
                    for step in (0..=steps).rev() {
                        let t = (step as f32) / (steps as f32);
                        let cur_spread = spread_radius + blur * t;
                        let step_alpha = ((base_alpha * (1.0 - t * 0.6)) / (steps as f32 + 1.0))
                            .clamp(0.0, 255.0) as u8;
                        if step_alpha == 0 {
                            continue;
                        }
                        let step_color = Color::rgba(color.r, color.g, color.b, step_alpha);
                        let shadow_rect = mango_core::Rect::new(
                            rect.x() + offset_x - cur_spread,
                            rect.y() + offset_y - cur_spread,
                            (rect.width() + cur_spread * 2.0).max(0.0),
                            (rect.height() + cur_spread * 2.0).max(0.0),
                        );
                        let step_radii = [
                            (radii[0] + cur_spread).max(0.0),
                            (radii[1] + cur_spread).max(0.0),
                            (radii[2] + cur_spread).max(0.0),
                            (radii[3] + cur_spread).max(0.0),
                        ];
                        let r = intersect_rect(active_clip, shadow_rect);
                        if r.width() > 0.0 && r.height() > 0.0 {
                            fill_rounded_rect_clipped(
                                buffer,
                                buf_width,
                                buf_height,
                                &shadow_rect,
                                step_color,
                                step_radii,
                                clip_out,
                                active_clip,
                            );
                        }
                    }
                }
            }
            DisplayCommand::DrawBorder {
                rect,
                color,
                widths,
                radii,
            } => {
                if !is_identity(transform) {
                    let x = rect.x();
                    let y = rect.y();
                    let w = rect.width();
                    let h = rect.height();
                    let edges = [
                        mango_core::Rect::new(x, y, w, widths.top),
                        mango_core::Rect::new(x, y + h - widths.bottom, w, widths.bottom),
                        mango_core::Rect::new(x, y, widths.left, h),
                        mango_core::Rect::new(x + w - widths.right, y, widths.right, h),
                    ];
                    let present = [
                        widths.top > 0.0,
                        widths.bottom > 0.0,
                        widths.left > 0.0,
                        widths.right > 0.0,
                    ];
                    for (edge, keep) in edges.iter().zip(present.iter()) {
                        if *keep {
                            fill_rect_transformed(
                                buffer,
                                buf_width,
                                buf_height,
                                edge,
                                *color,
                                None,
                                active_clip,
                                transform,
                            );
                        }
                    }
                    continue;
                }

                if radii.iter().any(|&r| r > 0.0) {
                    let inner_x = rect.x() + widths.left;
                    let inner_y = rect.y() + widths.top;
                    let inner_w = (rect.width() - widths.left - widths.right).max(0.0);
                    let inner_h = (rect.height() - widths.top - widths.bottom).max(0.0);
                    let inner_rect = mango_core::Rect::new(inner_x, inner_y, inner_w, inner_h);
                    let inner_radii = [
                        (radii[0] - widths.top.max(widths.left)).max(0.0),
                        (radii[1] - widths.top.max(widths.right)).max(0.0),
                        (radii[2] - widths.bottom.max(widths.right)).max(0.0),
                        (radii[3] - widths.bottom.max(widths.left)).max(0.0),
                    ];
                    fill_rounded_rect_clipped(
                        buffer,
                        buf_width,
                        buf_height,
                        rect,
                        *color,
                        *radii,
                        Some((&inner_rect, inner_radii)),
                        active_clip,
                    );
                    continue;
                }
                let x = rect.x();
                let y = rect.y();
                let w = rect.width();
                let h = rect.height();

                // CSS Border Triangle detection:
                // When an element has zero (or near-zero) content dimensions, borders meet at the center forming triangles.
                let is_zero_content =
                    w <= widths.left + widths.right + 0.5 && h <= widths.top + widths.bottom + 0.5;
                if is_zero_content
                    && (widths.top > 0.0
                        || widths.bottom > 0.0
                        || widths.left > 0.0
                        || widths.right > 0.0)
                {
                    if widths.top > 0.0 && widths.bottom == 0.0 {
                        // Downward-pointing triangle
                        let start_y = y.round() as i32;
                        let end_y = (y + widths.top).round() as i32;
                        for cur_y in start_y..end_y {
                            let t = if widths.top > 0.0 {
                                (cur_y as f32 - y) / widths.top
                            } else {
                                0.0
                            };
                            let row_x1 = x + widths.left * t;
                            let row_x2 = x + w - widths.right * t;
                            let clip_x1 = (row_x1.max(active_clip.x())).round() as i32;
                            let clip_x2 = (row_x2.min(active_clip.right())).round() as i32;
                            if clip_x2 > clip_x1
                                && cur_y >= active_clip.y() as i32
                                && cur_y < active_clip.bottom() as i32
                            {
                                fill_rect(
                                    buffer,
                                    buf_width,
                                    buf_height,
                                    clip_x1,
                                    cur_y,
                                    (clip_x2 - clip_x1) as u32,
                                    1,
                                    *color,
                                );
                            }
                        }
                        continue;
                    } else if widths.bottom > 0.0 && widths.top == 0.0 {
                        // Upward-pointing triangle
                        let start_y = y.round() as i32;
                        let end_y = (y + widths.bottom).round() as i32;
                        for cur_y in start_y..end_y {
                            let t = if widths.bottom > 0.0 {
                                (y + widths.bottom - cur_y as f32) / widths.bottom
                            } else {
                                0.0
                            };
                            let row_x1 = x + widths.left * t;
                            let row_x2 = x + w - widths.right * t;
                            let clip_x1 = (row_x1.max(active_clip.x())).round() as i32;
                            let clip_x2 = (row_x2.min(active_clip.right())).round() as i32;
                            if clip_x2 > clip_x1
                                && cur_y >= active_clip.y() as i32
                                && cur_y < active_clip.bottom() as i32
                            {
                                fill_rect(
                                    buffer,
                                    buf_width,
                                    buf_height,
                                    clip_x1,
                                    cur_y,
                                    (clip_x2 - clip_x1) as u32,
                                    1,
                                    *color,
                                );
                            }
                        }
                        continue;
                    } else if widths.left > 0.0 && widths.right == 0.0 {
                        // Rightward-pointing triangle
                        let start_x = x.round() as i32;
                        let end_x = (x + widths.left).round() as i32;
                        for cur_x in start_x..end_x {
                            let t = if widths.left > 0.0 {
                                (cur_x as f32 - x) / widths.left
                            } else {
                                0.0
                            };
                            let col_y1 = y + widths.top * t;
                            let col_y2 = y + h - widths.bottom * t;
                            let clip_y1 = (col_y1.max(active_clip.y())).round() as i32;
                            let clip_y2 = (col_y2.min(active_clip.bottom())).round() as i32;
                            if clip_y2 > clip_y1
                                && cur_x >= active_clip.x() as i32
                                && cur_x < active_clip.right() as i32
                            {
                                fill_rect(
                                    buffer,
                                    buf_width,
                                    buf_height,
                                    cur_x,
                                    clip_y1,
                                    1,
                                    (clip_y2 - clip_y1) as u32,
                                    *color,
                                );
                            }
                        }
                        continue;
                    } else if widths.right > 0.0 && widths.left == 0.0 {
                        // Leftward-pointing triangle
                        let start_x = x.round() as i32;
                        let end_x = (x + widths.right).round() as i32;
                        for cur_x in start_x..end_x {
                            let t = if widths.right > 0.0 {
                                (x + widths.right - cur_x as f32) / widths.right
                            } else {
                                0.0
                            };
                            let col_y1 = y + widths.top * t;
                            let col_y2 = y + h - widths.bottom * t;
                            let clip_y1 = (col_y1.max(active_clip.y())).round() as i32;
                            let clip_y2 = (col_y2.min(active_clip.bottom())).round() as i32;
                            if clip_y2 > clip_y1
                                && cur_x >= active_clip.x() as i32
                                && cur_x < active_clip.right() as i32
                            {
                                fill_rect(
                                    buffer,
                                    buf_width,
                                    buf_height,
                                    cur_x,
                                    clip_y1,
                                    1,
                                    (clip_y2 - clip_y1) as u32,
                                    *color,
                                );
                            }
                        }
                        continue;
                    }
                }

                // Top border
                if widths.top > 0.0 {
                    let top_rect =
                        intersect_rect(active_clip, mango_core::Rect::new(x, y, w, widths.top));
                    if top_rect.width() > 0.0 && top_rect.height() > 0.0 {
                        fill_rect(
                            buffer,
                            buf_width,
                            buf_height,
                            top_rect.x() as i32,
                            top_rect.y() as i32,
                            top_rect.width() as u32,
                            top_rect.height() as u32,
                            *color,
                        );
                    }
                }
                // Bottom border
                if widths.bottom > 0.0 {
                    let bottom_rect = intersect_rect(
                        active_clip,
                        mango_core::Rect::new(x, y + h - widths.bottom, w, widths.bottom),
                    );
                    if bottom_rect.width() > 0.0 && bottom_rect.height() > 0.0 {
                        fill_rect(
                            buffer,
                            buf_width,
                            buf_height,
                            bottom_rect.x() as i32,
                            bottom_rect.y() as i32,
                            bottom_rect.width() as u32,
                            bottom_rect.height() as u32,
                            *color,
                        );
                    }
                }
                // Left border
                if widths.left > 0.0 {
                    let left_rect =
                        intersect_rect(active_clip, mango_core::Rect::new(x, y, widths.left, h));
                    if left_rect.width() > 0.0 && left_rect.height() > 0.0 {
                        fill_rect(
                            buffer,
                            buf_width,
                            buf_height,
                            left_rect.x() as i32,
                            left_rect.y() as i32,
                            left_rect.width() as u32,
                            left_rect.height() as u32,
                            *color,
                        );
                    }
                }
                // Right border
                if widths.right > 0.0 {
                    let right_rect = intersect_rect(
                        active_clip,
                        mango_core::Rect::new(x + w - widths.right, y, widths.right, h),
                    );
                    if right_rect.width() > 0.0 && right_rect.height() > 0.0 {
                        fill_rect(
                            buffer,
                            buf_width,
                            buf_height,
                            right_rect.x() as i32,
                            right_rect.y() as i32,
                            right_rect.width() as u32,
                            right_rect.height() as u32,
                            *color,
                        );
                    }
                }
            }
            DisplayCommand::DrawText {
                text,
                x,
                y,
                color,
                font_size,
                weight,
                family,
                style,
                decoration,
                letter_spacing,
            } => {
                if color.a == 0 || *font_size <= 0.5 || text.is_empty() {
                    continue;
                }
                let is_rotated = transform[1].abs() > 1e-4 || transform[2].abs() > 1e-4;
                if is_rotated {
                    let (text_w, text_h) = fm.measure_text_with_spacing(
                        text,
                        *font_size,
                        *weight,
                        *family,
                        *letter_spacing,
                    );
                    let w_u32 = text_w.ceil().max(1.0) as u32;
                    let h_u32 = text_h.ceil().max(1.0) as u32;
                    let mut text_buf = vec![0u32; (w_u32 * h_u32) as usize];
                    let local_clip = mango_core::Rect::new(0.0, 0.0, text_w, text_h);
                    fm.rasterize_text_clipped(
                        &mut text_buf,
                        w_u32,
                        h_u32,
                        0,
                        0,
                        text,
                        *color,
                        *font_size,
                        *weight,
                        *family,
                        *style,
                        *letter_spacing,
                        local_clip,
                    );
                    blit_image_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        *x,
                        *y,
                        text_w,
                        text_h,
                        &text_buf,
                        active_clip,
                        transform,
                        active_blend,
                    );
                } else {
                    let (draw_x, draw_y, draw_size) = if is_identity(transform) {
                        (*x, *y, *font_size)
                    } else {
                        let (tx, ty) = apply_matrix(transform, *x, *y);
                        let scale = matrix_scale(transform);
                        (tx, ty, *font_size * scale)
                    };
                    if draw_size <= 0.5 {
                        continue;
                    }
                    fm.rasterize_text_clipped(
                        buffer,
                        buf_width,
                        buf_height,
                        draw_x as i32,
                        draw_y as i32,
                        text,
                        *color,
                        draw_size,
                        *weight,
                        *family,
                        *style,
                        *letter_spacing * (draw_size / font_size.max(0.001)),
                        active_clip,
                    );
                }

                if *decoration != TextDecoration::None && is_identity(transform) {
                    let (text_w, _) = fm.measure_text_with_spacing(
                        text,
                        *font_size,
                        *weight,
                        *family,
                        *letter_spacing,
                    );
                    let font = fm.select_font(*family, *weight);
                    let ascent = font
                        .horizontal_line_metrics(font_size.max(1.0))
                        .map(|lm| lm.ascent)
                        .unwrap_or(*font_size * 0.8);
                    let thickness = (*font_size / 14.0).max(1.0);

                    let line_rect = match decoration {
                        TextDecoration::Underline => {
                            let line_y = *y + ascent + 2.0;
                            mango_core::Rect::new(*x, line_y, text_w.ceil(), thickness)
                        }
                        TextDecoration::LineThrough => {
                            let line_y = *y + ascent - (*font_size * 0.3);
                            mango_core::Rect::new(*x, line_y, text_w.ceil(), thickness)
                        }
                        TextDecoration::Overline => {
                            let line_y = *y + 1.0;
                            mango_core::Rect::new(*x, line_y, text_w.ceil(), thickness)
                        }
                        TextDecoration::None => mango_core::Rect::new(0.0, 0.0, 0.0, 0.0),
                    };
                    let r = intersect_rect(active_clip, line_rect);
                    if r.width() > 0.0 && r.height() > 0.0 {
                        fill_rect(
                            buffer,
                            buf_width,
                            buf_height,
                            r.x() as i32,
                            r.y() as i32,
                            r.width() as u32,
                            r.height() as u32,
                            *color,
                        );
                    }
                }
            }
            DisplayCommand::DrawLine {
                x1,
                y1,
                x2,
                y2,
                color,
                thickness,
            } => {
                let line_rect = if (y1 - y2).abs() < 1.0 {
                    mango_core::Rect::new(*x1, *y1, (*x2 - *x1).max(0.0), *thickness)
                } else if (x1 - x2).abs() < 1.0 {
                    mango_core::Rect::new(*x1, *y1, *thickness, (*y2 - *y1).max(0.0))
                } else {
                    mango_core::Rect::new(0.0, 0.0, 0.0, 0.0)
                };
                if !is_identity(transform) {
                    fill_rect_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        &line_rect,
                        *color,
                        None,
                        active_clip,
                        transform,
                    );
                    continue;
                }
                let r = intersect_rect(active_clip, line_rect);
                if r.width() > 0.0 && r.height() > 0.0 {
                    fill_rect(
                        buffer,
                        buf_width,
                        buf_height,
                        r.x() as i32,
                        r.y() as i32,
                        r.width() as u32,
                        r.height() as u32,
                        *color,
                    );
                }
            }
            DisplayCommand::DrawImage {
                x,
                y,
                width,
                height,
                pixels,
            } => {
                if !is_identity(transform) {
                    blit_image_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        *x,
                        *y,
                        *width,
                        *height,
                        pixels,
                        active_clip,
                        transform,
                        active_blend,
                    );
                    continue;
                }
                blit_image(
                    buffer,
                    buf_width,
                    buf_height,
                    *x as i32,
                    *y as i32,
                    *width as u32,
                    *height as u32,
                    pixels,
                    active_clip,
                    active_blend,
                );
            }
            DisplayCommand::PushFilter { filters, rect } => {
                filter_stack.push((filters.clone(), *rect));
            }
            DisplayCommand::PopFilter => {
                if let Some((filters, rect)) = filter_stack.pop() {
                    apply_filter_to_rect(buffer, buf_width, buf_height, &rect, &filters);
                }
            }
            DisplayCommand::PushBackdropFilter { filters, rect } => {
                apply_filter_to_rect(buffer, buf_width, buf_height, rect, filters);
            }
            DisplayCommand::PopBackdropFilter => {}
            DisplayCommand::PushBlendMode { mode } => {
                blend_mode_stack.push(*mode);
            }
            DisplayCommand::PopBlendMode => {
                blend_mode_stack.pop();
            }
            DisplayCommand::PushClipPath { clip_path, rect } => {
                let snapshot = snapshot_rect(buffer, buf_width, buf_height, rect);
                clip_path_stack.push((*clip_path.clone(), *rect, snapshot));
            }
            DisplayCommand::PopClipPath => {
                if let Some((clip_path, rect, snapshot)) = clip_path_stack.pop() {
                    restore_outside_clip_path(
                        buffer,
                        buf_width,
                        buf_height,
                        &rect,
                        active_clip,
                        &clip_path,
                        &snapshot,
                    );
                }
            }
            DisplayCommand::DrawBorderImage {
                rect,
                pixels,
                img_width,
                img_height,
                slice,
                widths,
                repeat_h,
                repeat_v,
                fill,
            } => {
                draw_border_image(
                    buffer,
                    buf_width,
                    buf_height,
                    rect,
                    pixels,
                    *img_width,
                    *img_height,
                    *slice,
                    *widths,
                    *repeat_h,
                    *repeat_v,
                    *fill,
                    active_clip,
                    transform,
                );
            }
            DisplayCommand::DrawTextWithGradient {
                text,
                x,
                y,
                gradient,
                gradient_rect,
                font_size,
                weight,
                family,
                style,
                decoration: _,
                letter_spacing,
            } => {
                fm.rasterize_text_gradient_clipped(
                    buffer,
                    buf_width,
                    buf_height,
                    *x as i32,
                    *y as i32,
                    text,
                    gradient,
                    *gradient_rect,
                    *font_size,
                    *weight,
                    *family,
                    *style,
                    *letter_spacing,
                    active_clip,
                );
            }
        }
    }
}

// ── Affine transform support ────────────────────────────────────────────────

/// The identity 2D affine matrix `matrix(1, 0, 0, 1, 0, 0)`.
const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Returns true when `m` is (numerically) the identity transform.
#[inline]
fn is_identity(m: [f32; 6]) -> bool {
    (m[0] - 1.0).abs() < 1e-4
        && m[1].abs() < 1e-4
        && m[2].abs() < 1e-4
        && (m[3] - 1.0).abs() < 1e-4
        && m[4].abs() < 1e-4
        && m[5].abs() < 1e-4
}

/// Composes two affine matrices: `outer * inner` (apply `inner` first).
#[inline]
fn concat_matrix(outer: [f32; 6], inner: [f32; 6]) -> [f32; 6] {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

/// Maps a point through an affine matrix.
#[inline]
fn apply_matrix(m: [f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// Computes the inverse of an affine matrix, or `None` when it is singular.
fn invert_matrix(m: [f32; 6]) -> Option<[f32; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-6 {
        return None;
    }
    let inv_det = 1.0 / det;
    let a = m[3] * inv_det;
    let b = -m[1] * inv_det;
    let c = -m[2] * inv_det;
    let d = m[0] * inv_det;
    let e = -(a * m[4] + c * m[5]);
    let f = -(b * m[4] + d * m[5]);
    Some([a, b, c, d, e, f])
}

/// Approximates the uniform scale factor of a matrix (geometric mean of axes).
fn matrix_scale(m: [f32; 6]) -> f32 {
    let det = (m[0] * m[3] - m[1] * m[2]).abs();
    det.sqrt().max(0.0001)
}

/// Axis-aligned bounding box of a transformed rectangle.
fn transformed_bounds(rect: &mango_core::Rect, m: [f32; 6]) -> mango_core::Rect {
    let corners = [
        (rect.x(), rect.y()),
        (rect.x() + rect.width(), rect.y()),
        (rect.x(), rect.y() + rect.height()),
        (rect.x() + rect.width(), rect.y() + rect.height()),
    ];
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (cx, cy) in corners {
        let (tx, ty) = apply_matrix(m, cx, cy);
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
        max_x = max_x.max(tx);
        max_y = max_y.max(ty);
    }
    mango_core::Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

/// Fills a rectangle through an affine transform, optionally clipped to rounded corners.
///
/// Uses inverse mapping: every destination pixel inside the transformed quad tests
/// its source coordinate against the original rectangle (and corner radii).
#[allow(clippy::too_many_arguments)]
fn fill_rect_transformed(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    color: Color,
    radii: Option<[f32; 4]>,
    clip: mango_core::Rect,
    transform: [f32; 6],
) {
    if color.a == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };
    let bounds = intersect_rect(clip, transformed_bounds(rect, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let x0 = bounds.x().max(0.0).floor() as i32;
    let y0 = bounds.y().max(0.0).floor() as i32;
    let x1 = ((bounds.x() + bounds.width()).ceil() as i32).clamp(0, buf_width as i32);
    let y1 = ((bounds.y() + bounds.height()).ceil() as i32).clamp(0, buf_height as i32);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;
    let e = 0.5;
    for py in y_start..y_end {
        let fy = py as f32 + 0.5;
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let (sx, sy) = apply_matrix(inv, fx, fy);
            // Sample with a half-pixel tolerance so transformed edges stay solid.
            if sx < rect.x() - e || sx > rect.x() + rect.width() + e {
                continue;
            }
            if sy < rect.y() - e || sy > rect.y() + rect.height() + e {
                continue;
            }
            if let Some(r) = radii
                && !is_inside_rounded_rect(sx, sy, rect, r)
            {
                continue;
            }
            let idx = row + px as usize;
            if idx < buffer.len() {
                buffer[idx] = if color.a < 255 {
                    blend_pixel(color, buffer[idx])
                } else {
                    color.to_rgb_u32()
                };
            }
        }
    }
}

/// Blits decoded image pixels through an affine transform using nearest-neighbour mapping.
#[allow(clippy::too_many_arguments)]
fn blit_image_transformed(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: f32,
    y: f32,
    img_width: f32,
    img_height: f32,
    pixels: &[u32],
    clip: mango_core::Rect,
    transform: [f32; 6],
    blend_mode: mango_css::values::BlendMode,
) {
    if img_width <= 0.0 || img_height <= 0.0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };
    let quad = mango_core::Rect::new(x, y, img_width, img_height);
    let bounds = intersect_rect(clip, transformed_bounds(&quad, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    // DrawImage commands carry pixels already resized to `img_width × img_height`;
    // sample that grid directly (clamped for callers with mismatched buffers).
    let src_stride = img_width.max(1.0) as usize;

    let x0 = bounds.x().max(0.0).floor() as i32;
    let y0 = bounds.y().max(0.0).floor() as i32;
    let x1 = ((bounds.x() + bounds.width()).ceil() as i32).clamp(0, buf_width as i32);
    let y1 = ((bounds.y() + bounds.height()).ceil() as i32).clamp(0, buf_height as i32);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;

    for py in y_start..y_end {
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let (sx, sy) = apply_matrix(inv, fx, fy);
            if sx < x || sx >= x + img_width || sy < y || sy >= y + img_height {
                continue;
            }
            let u = ((sx - x) / img_width).clamp(0.0, 0.9999);
            let v = ((sy - y) / img_height).clamp(0.0, 0.9999);
            let samp_x = u * (src_stride as f32);
            let samp_y = v * img_height;
            let pixel = sample_bilinear(
                pixels,
                src_stride as u32,
                img_height.max(1.0) as u32,
                samp_x,
                samp_y,
            );
            let alpha = (pixel >> 24) & 0xFF;
            if alpha == 0 {
                continue;
            }
            let idx = row + px as usize;
            if idx >= buffer.len() {
                continue;
            }
            let c = Color::rgba(
                ((pixel >> 16) & 0xFF) as u8,
                ((pixel >> 8) & 0xFF) as u8,
                (pixel & 0xFF) as u8,
                alpha as u8,
            );
            if alpha == 255 && blend_mode == mango_css::values::BlendMode::Normal {
                buffer[idx] = pixel & 0x00FFFFFF;
            } else {
                buffer[idx] = blend_pixel_mode(c, buffer[idx], blend_mode);
            }
        }
    }
}

/// Rasterizes a CSS gradient into the given rectangle, honoring rounded corners
/// and the active transform.
#[allow(clippy::too_many_arguments)]
fn fill_gradient(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    gradient: &mango_css::values::Gradient,
    radii: [f32; 4],
    opacity: f32,
    clip: mango_core::Rect,
    transform: [f32; 6],
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || opacity <= 0.0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };
    let bounds = intersect_rect(clip, transformed_bounds(rect, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }

    let grad_w = rect.width().max(1.0) as u32;
    let grad_h = rect.height().max(1.0) as u32;
    let pixels = gradient.rasterize(grad_w, grad_h);
    if pixels.is_empty() {
        return;
    }
    let has_radii = radii.iter().any(|r| *r > 0.0);
    let alpha_scale = opacity.clamp(0.0, 1.0);

    let x_start = bounds.x().max(0.0) as u32;
    let y_start = bounds.y().max(0.0) as u32;
    let x_end = ((bounds.x() + bounds.width()) as u32).min(buf_width);
    let y_end = ((bounds.y() + bounds.height()) as u32).min(buf_height);

    for py in y_start..y_end {
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let (sx, sy) = apply_matrix(inv, fx, fy);
            if sx < rect.x() || sx >= rect.x() + rect.width() {
                continue;
            }
            if sy < rect.y() || sy >= rect.y() + rect.height() {
                continue;
            }
            if has_radii && !is_inside_rounded_rect(sx, sy, rect, radii) {
                continue;
            }
            let gx = ((sx - rect.x()) / rect.width() * grad_w as f32) as u32;
            let gy = ((sy - rect.y()) / rect.height() * grad_h as f32) as u32;
            let gx = gx.min(grad_w.saturating_sub(1));
            let gy = gy.min(grad_h.saturating_sub(1));
            let src = pixels[(gy * grad_w + gx) as usize];
            let mut color = Color::rgba(
                ((src >> 16) & 0xFF) as u8,
                ((src >> 8) & 0xFF) as u8,
                (src & 0xFF) as u8,
                ((src >> 24) & 0xFF) as u8,
            );
            if alpha_scale < 1.0 {
                color.a = ((color.a as f32) * alpha_scale).round() as u8;
            }
            if color.a == 0 {
                continue;
            }
            let idx = row + px as usize;
            if idx < buffer.len() {
                buffer[idx] = blend_pixel(color, buffer[idx]);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_border_image(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    pixels: &[u32],
    img_w: u32,
    img_h: u32,
    slice: [f32; 4],
    widths: crate::display_list::BorderWidths,
    repeat_h: mango_css::values::BorderImageRepeat,
    repeat_v: mango_css::values::BorderImageRepeat,
    fill: bool,
    clip: mango_core::Rect,
    transform: [f32; 6],
) {
    if img_w == 0 || img_h == 0 || pixels.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0
    {
        return;
    }
    let s_top = slice[0].clamp(0.0, img_h as f32);
    let s_right = slice[1].clamp(0.0, img_w as f32);
    let s_bottom = slice[2].clamp(0.0, img_h as f32);
    let s_left = slice[3].clamp(0.0, img_w as f32);
    let s_mid_w = (img_w as f32 - s_left - s_right).max(0.0);
    let s_mid_h = (img_h as f32 - s_top - s_bottom).max(0.0);

    let d_x = rect.x();
    let d_y = rect.y();
    let d_w = rect.width();
    let d_h = rect.height();
    let d_top = widths.top.min(d_h * 0.5);
    let d_right = widths.right.min(d_w * 0.5);
    let d_bottom = widths.bottom.min(d_h * 0.5);
    let d_left = widths.left.min(d_w * 0.5);
    let d_mid_w = (d_w - d_left - d_right).max(0.0);
    let d_mid_h = (d_h - d_top - d_bottom).max(0.0);

    // 9 slices: (dest_rect, src_rect, rep_h, rep_v)
    let slices = [
        // 1. Top-left corner
        (
            mango_core::Rect::new(d_x, d_y, d_left, d_top),
            mango_core::Rect::new(0.0, 0.0, s_left, s_top),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 2. Top-right corner
        (
            mango_core::Rect::new(d_x + d_w - d_right, d_y, d_right, d_top),
            mango_core::Rect::new(img_w as f32 - s_right, 0.0, s_right, s_top),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 3. Bottom-right corner
        (
            mango_core::Rect::new(d_x + d_w - d_right, d_y + d_h - d_bottom, d_right, d_bottom),
            mango_core::Rect::new(
                img_w as f32 - s_right,
                img_h as f32 - s_bottom,
                s_right,
                s_bottom,
            ),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 4. Bottom-left corner
        (
            mango_core::Rect::new(d_x, d_y + d_h - d_bottom, d_left, d_bottom),
            mango_core::Rect::new(0.0, img_h as f32 - s_bottom, s_left, s_bottom),
            mango_css::values::BorderImageRepeat::Stretch,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 5. Top edge
        (
            mango_core::Rect::new(d_x + d_left, d_y, d_mid_w, d_top),
            mango_core::Rect::new(s_left, 0.0, s_mid_w, s_top),
            repeat_h,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 6. Bottom edge
        (
            mango_core::Rect::new(d_x + d_left, d_y + d_h - d_bottom, d_mid_w, d_bottom),
            mango_core::Rect::new(s_left, img_h as f32 - s_bottom, s_mid_w, s_bottom),
            repeat_h,
            mango_css::values::BorderImageRepeat::Stretch,
        ),
        // 7. Left edge
        (
            mango_core::Rect::new(d_x, d_y + d_top, d_left, d_mid_h),
            mango_core::Rect::new(0.0, s_top, s_left, s_mid_h),
            mango_css::values::BorderImageRepeat::Stretch,
            repeat_v,
        ),
        // 8. Right edge
        (
            mango_core::Rect::new(d_x + d_w - d_right, d_y + d_top, d_right, d_mid_h),
            mango_core::Rect::new(img_w as f32 - s_right, s_top, s_right, s_mid_h),
            mango_css::values::BorderImageRepeat::Stretch,
            repeat_v,
        ),
    ];

    for (d_rect, s_rect, rep_h, rep_v) in slices {
        if d_rect.width() > 0.0
            && d_rect.height() > 0.0
            && s_rect.width() > 0.0
            && s_rect.height() > 0.0
        {
            blit_border_slice(
                buffer, buf_width, buf_height, &d_rect, pixels, img_w, img_h, &s_rect, rep_h,
                rep_v, clip, transform,
            );
        }
    }

    if fill && d_mid_w > 0.0 && d_mid_h > 0.0 && s_mid_w > 0.0 && s_mid_h > 0.0 {
        let center_d = mango_core::Rect::new(d_x + d_left, d_y + d_top, d_mid_w, d_mid_h);
        let center_s = mango_core::Rect::new(s_left, s_top, s_mid_w, s_mid_h);
        blit_border_slice(
            buffer, buf_width, buf_height, &center_d, pixels, img_w, img_h, &center_s, repeat_h,
            repeat_v, clip, transform,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_border_slice(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    dest: &mango_core::Rect,
    pixels: &[u32],
    img_w: u32,
    img_h: u32,
    src: &mango_core::Rect,
    rep_h: mango_css::values::BorderImageRepeat,
    rep_v: mango_css::values::BorderImageRepeat,
    clip: mango_core::Rect,
    transform: [f32; 6],
) {
    let bounds = intersect_rect(clip, transformed_bounds(dest, transform));
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let Some(inv) = invert_matrix(transform) else {
        return;
    };

    let x_start = bounds.x().max(0.0) as u32;
    let y_start = bounds.y().max(0.0) as u32;
    let x_end = ((bounds.x() + bounds.width()) as u32).min(buf_width);
    let y_end = ((bounds.y() + bounds.height()) as u32).min(buf_height);

    for py in y_start..y_end {
        let row = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let (dx, dy) = apply_matrix(inv, fx, fy);
            if dx < dest.x()
                || dx >= dest.x() + dest.width()
                || dy < dest.y()
                || dy >= dest.y() + dest.height()
            {
                continue;
            }
            let norm_x = (dx - dest.x()) / dest.width().max(1.0);
            let norm_y = (dy - dest.y()) / dest.height().max(1.0);

            let sx = match rep_h {
                mango_css::values::BorderImageRepeat::Stretch => src.x() + norm_x * src.width(),
                _ => src.x() + ((dx - dest.x()) % src.width().max(1.0)),
            };
            let sy = match rep_v {
                mango_css::values::BorderImageRepeat::Stretch => src.y() + norm_y * src.height(),
                _ => src.y() + ((dy - dest.y()) % src.height().max(1.0)),
            };

            let ix = (sx as u32).min(img_w.saturating_sub(1));
            let iy = (sy as u32).min(img_h.saturating_sub(1));
            let pix = pixels[(iy * img_w + ix) as usize];
            let a = ((pix >> 24) & 0xFF) as u8;
            if a == 0 {
                continue;
            }
            let idx = row + px as usize;
            if idx < buffer.len() {
                let r = ((pix >> 16) & 0xFF) as u8;
                let g = ((pix >> 8) & 0xFF) as u8;
                let b = (pix & 0xFF) as u8;
                buffer[idx] = blend_pixel(Color::rgba(r, g, b, a), buffer[idx]);
            }
        }
    }
}

/// Blends a source color with a destination pixel using integer math.
#[inline]
pub fn blend_pixel(src: Color, dst_rgb: u32) -> u32 {
    blend_pixel_mode(src, dst_rgb, mango_css::values::BlendMode::Normal)
}

/// Blends a source color with a destination pixel using CSS blend modes.
pub fn blend_pixel_mode(src: Color, dst_rgb: u32, mode: mango_css::values::BlendMode) -> u32 {
    use mango_css::values::BlendMode;
    if mode == BlendMode::Normal {
        let alpha = src.a as u32;
        if alpha == 255 {
            return src.to_rgb_u32();
        }
        if alpha == 0 {
            return dst_rgb;
        }
        let dst_r = (dst_rgb >> 16) & 0xFF;
        let dst_g = (dst_rgb >> 8) & 0xFF;
        let dst_b = dst_rgb & 0xFF;
        let src_r = src.r as u32;
        let src_g = src.g as u32;
        let src_b = src.b as u32;
        let inv_a = 255 - alpha;
        let out_r = (src_r * alpha + dst_r * inv_a + 127) / 255;
        let out_g = (src_g * alpha + dst_g * inv_a + 127) / 255;
        let out_b = (src_b * alpha + dst_b * inv_a + 127) / 255;
        return (out_r << 16) | (out_g << 8) | out_b;
    }

    let alpha = src.a as f32 / 255.0;
    if alpha <= 0.0 {
        return dst_rgb;
    }

    let cb_r = ((dst_rgb >> 16) & 0xFF) as f32 / 255.0;
    let cb_g = ((dst_rgb >> 8) & 0xFF) as f32 / 255.0;
    let cb_b = (dst_rgb & 0xFF) as f32 / 255.0;

    let cs_r = src.r as f32 / 255.0;
    let cs_g = src.g as f32 / 255.0;
    let cs_b = src.b as f32 / 255.0;

    let blend_ch = |cb: f32, cs: f32| -> f32 {
        match mode {
            BlendMode::Multiply => cb * cs,
            BlendMode::Screen => cb + cs - cb * cs,
            BlendMode::Overlay => {
                if cb <= 0.5 {
                    2.0 * cb * cs
                } else {
                    1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
                }
            }
            BlendMode::Darken => cb.min(cs),
            BlendMode::Lighten => cb.max(cs),
            BlendMode::ColorDodge => {
                if cb <= 0.0 {
                    0.0
                } else if cs >= 1.0 {
                    1.0
                } else {
                    (cb / (1.0 - cs)).min(1.0)
                }
            }
            BlendMode::ColorBurn => {
                if cb >= 1.0 {
                    1.0
                } else if cs <= 0.0 {
                    0.0
                } else {
                    (1.0 - (1.0 - cb) / cs).max(0.0)
                }
            }
            BlendMode::HardLight => {
                if cs <= 0.5 {
                    2.0 * cb * cs
                } else {
                    1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
                }
            }
            BlendMode::SoftLight => {
                if cs <= 0.5 {
                    cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
                } else {
                    let d = if cb <= 0.25 {
                        ((16.0 * cb - 12.0) * cb + 4.0) * cb
                    } else {
                        cb.sqrt()
                    };
                    cb + (2.0 * cs - 1.0) * (d - cb)
                }
            }
            BlendMode::Difference => (cb - cs).abs(),
            BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
            _ => cs,
        }
    };

    let cm_r = blend_ch(cb_r, cs_r);
    let cm_g = blend_ch(cb_g, cs_g);
    let cm_b = blend_ch(cb_b, cs_b);

    let cr_r = ((1.0 - alpha) * cb_r + alpha * cm_r).clamp(0.0, 1.0);
    let cr_g = ((1.0 - alpha) * cb_g + alpha * cm_g).clamp(0.0, 1.0);
    let cr_b = ((1.0 - alpha) * cb_b + alpha * cm_b).clamp(0.0, 1.0);

    let out_r = (cr_r * 255.0).round() as u32;
    let out_g = (cr_g * 255.0).round() as u32;
    let out_b = (cr_b * 255.0).round() as u32;

    (out_r << 16) | (out_g << 8) | out_b
}

fn snapshot_rect(buffer: &[u32], buf_width: u32, buf_height: u32, rect: &Rect) -> Vec<u32> {
    let min_x = (rect.x().max(0.0) as u32).min(buf_width);
    let max_x = ((rect.x() + rect.width()).max(0.0) as u32).min(buf_width);
    let min_y = (rect.y().max(0.0) as u32).min(buf_height);
    let max_y = ((rect.y() + rect.height()).max(0.0) as u32).min(buf_height);
    let mut pixels = Vec::with_capacity(((max_x - min_x) * (max_y - min_y)) as usize);
    for y in min_y..max_y {
        for x in min_x..max_x {
            pixels.push(buffer[(y * buf_width + x) as usize]);
        }
    }
    pixels
}

fn is_point_inside_clip_path(
    clip_path: &mango_css::values::ClipPath,
    bounds: &Rect,
    px: f32,
    py: f32,
) -> bool {
    use mango_css::values::ClipPath;
    match clip_path {
        ClipPath::None => true,
        ClipPath::Circle {
            radius,
            center_x,
            center_y,
        } => {
            let r = match radius {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => {
                    (bounds.width().min(bounds.height())) * pct / 100.0
                }
                _ => 50.0,
            };
            let cx = bounds.x()
                + match center_x {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                    _ => bounds.width() / 2.0,
                };
            let cy = bounds.y()
                + match center_y {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                    _ => bounds.height() / 2.0,
                };
            let dx = px - cx;
            let dy = py - cy;
            dx * dx + dy * dy <= r * r
        }
        ClipPath::Ellipse {
            radius_x,
            radius_y,
            center_x,
            center_y,
        } => {
            let rx = match radius_x {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                _ => bounds.width() / 2.0,
            }
            .max(0.1);
            let ry = match radius_y {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                _ => bounds.height() / 2.0,
            }
            .max(0.1);
            let cx = bounds.x()
                + match center_x {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                    _ => bounds.width() / 2.0,
                };
            let cy = bounds.y()
                + match center_y {
                    mango_css::values::Length::Px(p) => *p,
                    mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                    _ => bounds.height() / 2.0,
                };
            let dx = (px - cx) / rx;
            let dy = (py - cy) / ry;
            dx * dx + dy * dy <= 1.0
        }
        ClipPath::Inset {
            top,
            right,
            bottom,
            left,
            round,
        } => {
            let t = match top {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                _ => 0.0,
            };
            let r = match right {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                _ => 0.0,
            };
            let b = match bottom {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.height() * pct / 100.0,
                _ => 0.0,
            };
            let l = match left {
                mango_css::values::Length::Px(p) => *p,
                mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                _ => 0.0,
            };
            let min_x = bounds.x() + l;
            let max_x = bounds.x() + bounds.width() - r;
            let min_y = bounds.y() + t;
            let max_y = bounds.y() + bounds.height() - b;
            let rw = (max_x - min_x).max(0.0);
            let rh = (max_y - min_y).max(0.0);
            let inset_rect = mango_core::Rect::new(min_x, min_y, rw, rh);
            if let Some(radii_lens) = round {
                let r0 = match radii_lens[0] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                let r1 = match radii_lens[1] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                let r2 = match radii_lens[2] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                let r3 = match radii_lens[3] {
                    mango_css::values::Length::Px(p) => p,
                    mango_css::values::Length::Percent(pct) => rw.min(rh) * pct / 100.0,
                    _ => 0.0,
                };
                is_inside_rounded_rect(px, py, &inset_rect, [r0, r1, r2, r3])
            } else {
                px >= min_x && px <= max_x && py >= min_y && py <= max_y
            }
        }
        ClipPath::Polygon(pts) => {
            if pts.len() < 3 {
                return true;
            }
            let resolved_pts: Vec<(f32, f32)> = pts
                .iter()
                .map(|(x_len, y_len)| {
                    let x = bounds.x()
                        + match x_len {
                            mango_css::values::Length::Px(p) => *p,
                            mango_css::values::Length::Percent(pct) => bounds.width() * pct / 100.0,
                            _ => 0.0,
                        };
                    let y = bounds.y()
                        + match y_len {
                            mango_css::values::Length::Px(p) => *p,
                            mango_css::values::Length::Percent(pct) => {
                                bounds.height() * pct / 100.0
                            }
                            _ => 0.0,
                        };
                    (x, y)
                })
                .collect();

            // Ray-casting even-odd point-in-polygon
            let mut inside = false;
            let n = resolved_pts.len();
            for i in 0..n {
                let (x1, y1) = resolved_pts[i];
                let (x2, y2) = resolved_pts[(i + 1) % n];
                if ((y1 > py) != (y2 > py)) && (px < (x2 - x1) * (py - y1) / (y2 - y1 + 1e-6) + x1)
                {
                    inside = !inside;
                }
            }
            inside
        }
        ClipPath::Url(_) => true,
    }
}

fn restore_outside_clip_path(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    clip: Rect,
    clip_path: &mango_css::values::ClipPath,
    snapshot: &[u32],
) {
    let r = intersect_rect(clip, *rect);
    let min_x = (r.x().max(0.0) as u32).min(buf_width);
    let max_x = ((r.x() + r.width()).max(0.0) as u32).min(buf_width);
    let min_y = (r.y().max(0.0) as u32).min(buf_height);
    let max_y = ((r.y() + r.height()).max(0.0) as u32).min(buf_height);
    let mut snap_idx = 0;
    for y in min_y..max_y {
        for x in min_x..max_x {
            if snap_idx < snapshot.len() {
                if !is_point_inside_clip_path(clip_path, rect, x as f32, y as f32) {
                    buffer[(y * buf_width + x) as usize] = snapshot[snap_idx];
                }
                snap_idx += 1;
            }
        }
    }
}

fn apply_filter_to_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &Rect,
    filters: &[mango_css::values::FilterFunction],
) {
    if filters.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let min_x = (rect.x().max(0.0) as u32).min(buf_width);
    let max_x = ((rect.x() + rect.width()).max(0.0) as u32).min(buf_width);
    let min_y = (rect.y().max(0.0) as u32).min(buf_height);
    let max_y = ((rect.y() + rect.height()).max(0.0) as u32).min(buf_height);
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let w = (max_x - min_x) as usize;
    let h = (max_y - min_y) as usize;

    for filter in filters {
        match filter {
            mango_css::values::FilterFunction::Blur(radius) => {
                let r = (*radius as i32).max(0);
                if r > 0 {
                    box_blur_rect(buffer, buf_width, min_x, min_y, w, h, r);
                }
            }
            mango_css::values::FilterFunction::Brightness(factor) => {
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = (((p >> 16) & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
                        let g = (((p >> 8) & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
                        let b = ((p & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (r << 16) | (g << 8) | b;
                    }
                }
            }
            mango_css::values::FilterFunction::Contrast(factor) => {
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((((p >> 16) & 0xff) as f32 - 128.0) * factor + 128.0)
                            .clamp(0.0, 255.0) as u32;
                        let g = ((((p >> 8) & 0xff) as f32 - 128.0) * factor + 128.0)
                            .clamp(0.0, 255.0) as u32;
                        let b =
                            (((p & 0xff) as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (r << 16) | (g << 8) | b;
                    }
                }
            }
            mango_css::values::FilterFunction::Grayscale(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                        let out_r = (r + (gray - r) * f).round() as u32;
                        let out_g = (g + (gray - g) * f).round() as u32;
                        let out_b = (b + (gray - b) * f).round() as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::Invert(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let out_r = (r + ((255.0 - r) - r) * f).round() as u32;
                        let out_g = (g + ((255.0 - g) - g) * f).round() as u32;
                        let out_b = (b + ((255.0 - b) - b) * f).round() as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::Opacity(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let a = (((p >> 24) & 0xff) as f32 * f).round() as u32;
                        buffer[idx] = (a << 24) | (p & 0x00ffffff);
                    }
                }
            }
            mango_css::values::FilterFunction::Saturate(factor) => {
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let gray = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                        let out_r = (gray + (r - gray) * factor).clamp(0.0, 255.0) as u32;
                        let out_g = (gray + (g - gray) * factor).clamp(0.0, 255.0) as u32;
                        let out_b = (gray + (b - gray) * factor).clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::Sepia(factor) => {
                let f = factor.clamp(0.0, 1.0);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let sr = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0);
                        let sg = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0);
                        let sb = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0);
                        let out_r = (r + (sr - r) * f).round() as u32;
                        let out_g = (g + (sg - g) * f).round() as u32;
                        let out_b = (b + (sb - b) * f).round() as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::HueRotate(deg) => {
                let rad = deg.to_radians();
                let cos_a = rad.cos();
                let sin_a = rad.sin();
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let idx = (y * buf_width + x) as usize;
                        let p = buffer[idx];
                        let r = ((p >> 16) & 0xff) as f32;
                        let g = ((p >> 8) & 0xff) as f32;
                        let b = (p & 0xff) as f32;
                        let out_r = ((0.213 + cos_a * 0.787 - sin_a * 0.213) * r
                            + (0.715 - cos_a * 0.715 - sin_a * 0.715) * g
                            + (0.072 - cos_a * 0.072 + sin_a * 0.928) * b)
                            .clamp(0.0, 255.0) as u32;
                        let out_g = ((0.213 - cos_a * 0.213 + sin_a * 0.143) * r
                            + (0.715 + cos_a * 0.285 + sin_a * 0.140) * g
                            + (0.072 - cos_a * 0.072 - sin_a * 0.283) * b)
                            .clamp(0.0, 255.0) as u32;
                        let out_b = ((0.213 - cos_a * 0.213 - sin_a * 0.787) * r
                            + (0.715 - cos_a * 0.715 + sin_a * 0.715) * g
                            + (0.072 + cos_a * 0.928 + sin_a * 0.072) * b)
                            .clamp(0.0, 255.0) as u32;
                        buffer[idx] = (p & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
            mango_css::values::FilterFunction::DropShadow {
                offset_x,
                offset_y,
                blur,
                color,
            } => {
                let w = (max_x - min_x) as usize;
                let h = (max_y - min_y) as usize;
                if w == 0 || h == 0 || color.a == 0 {
                    continue;
                }
                let mut shadow_alphas = vec![0u8; w * h];
                let ox = offset_x.round() as i32;
                let oy = offset_y.round() as i32;
                for y in 0..h {
                    for x in 0..w {
                        let src_x = x as i32 - ox;
                        let src_y = y as i32 - oy;
                        if src_x >= 0 && src_x < w as i32 && src_y >= 0 && src_y < h as i32 {
                            let src_idx = ((min_y as i32 + src_y) as u32 * buf_width
                                + (min_x as i32 + src_x) as u32)
                                as usize;
                            let alpha = (buffer[src_idx] >> 24) & 0xff;
                            shadow_alphas[y * w + x] =
                                (alpha as f32 * (color.a as f32 / 255.0)).round() as u8;
                        }
                    }
                }
                let r = blur.round() as i32;
                if r > 0 {
                    let mut temp = vec![0u8; w * h];
                    let radius = r.min(16).max(1) as usize;
                    // Horizontal sliding-window blur
                    for y in 0..h {
                        let mut sum = 0u32;
                        let mut count = 0u32;
                        let init_right = radius.min(w.saturating_sub(1));
                        for kx in 0..=init_right {
                            sum += shadow_alphas[y * w + kx] as u32;
                            count += 1;
                        }
                        for x in 0..w {
                            if x > 0 {
                                if x + radius < w {
                                    sum += shadow_alphas[y * w + (x + radius)] as u32;
                                    count += 1;
                                }
                                if x > radius {
                                    sum -= shadow_alphas[y * w + (x - radius - 1)] as u32;
                                    count -= 1;
                                }
                            }
                            temp[y * w + x] = if count > 0 { (sum / count) as u8 } else { 0 };
                        }
                    }
                    // Vertical sliding-window blur
                    for x in 0..w {
                        let mut sum = 0u32;
                        let mut count = 0u32;
                        let init_bottom = radius.min(h.saturating_sub(1));
                        for ky in 0..=init_bottom {
                            sum += temp[ky * w + x] as u32;
                            count += 1;
                        }
                        for y in 0..h {
                            if y > 0 {
                                if y + radius < h {
                                    sum += temp[(y + radius) * w + x] as u32;
                                    count += 1;
                                }
                                if y > radius {
                                    sum -= temp[(y - radius - 1) * w + x] as u32;
                                    count -= 1;
                                }
                            }
                            shadow_alphas[y * w + x] =
                                if count > 0 { (sum / count) as u8 } else { 0 };
                        }
                    }
                }
                // Composite: shadow is drawn under the original element
                for y in 0..h {
                    for x in 0..w {
                        let idx = ((min_y + y as u32) * buf_width + (min_x + x as u32)) as usize;
                        let orig_pixel = buffer[idx];
                        let s_a = shadow_alphas[y * w + x];
                        if s_a > 0 {
                            let shadow_color = Color::rgba(color.r, color.g, color.b, s_a);
                            let shadow_u32 = shadow_color.to_rgb_u32();
                            let orig_alpha = (orig_pixel >> 24) & 0xff;
                            if orig_alpha == 0 {
                                buffer[idx] = shadow_u32;
                            } else if orig_alpha < 255 {
                                let orig_color = Color::rgba(
                                    ((orig_pixel >> 16) & 0xff) as u8,
                                    ((orig_pixel >> 8) & 0xff) as u8,
                                    (orig_pixel & 0xff) as u8,
                                    orig_alpha as u8,
                                );
                                buffer[idx] = blend_pixel(orig_color, shadow_u32);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn box_blur_rect(
    buffer: &mut [u32],
    buf_width: u32,
    min_x: u32,
    min_y: u32,
    w: usize,
    h: usize,
    radius: i32,
) {
    if w == 0 || h == 0 {
        return;
    }
    let r = radius.min(w.max(h) as i32 / 2).max(1) as usize;
    let mut temp = vec![0u32; w * h];

    // Horizontal sliding-window pass
    for row in 0..h {
        let y = min_y + row as u32;
        let mut sum_a = 0u32;
        let mut sum_r = 0u32;
        let mut sum_g = 0u32;
        let mut sum_b = 0u32;
        let mut count = 0u32;

        let right_init = r.min(w.saturating_sub(1));
        for cx in 0..=right_init {
            let px = min_x + cx as u32;
            let p = buffer[(y * buf_width + px) as usize];
            sum_a += (p >> 24) & 0xff;
            sum_r += (p >> 16) & 0xff;
            sum_g += (p >> 8) & 0xff;
            sum_b += p & 0xff;
            count += 1;
        }

        for col in 0..w {
            if col > 0 {
                if col + r < w {
                    let px = min_x + (col + r) as u32;
                    let p = buffer[(y * buf_width + px) as usize];
                    sum_a += (p >> 24) & 0xff;
                    sum_r += (p >> 16) & 0xff;
                    sum_g += (p >> 8) & 0xff;
                    sum_b += p & 0xff;
                    count += 1;
                }
                if col > r {
                    let px = min_x + (col - r - 1) as u32;
                    let p = buffer[(y * buf_width + px) as usize];
                    sum_a -= (p >> 24) & 0xff;
                    sum_r -= (p >> 16) & 0xff;
                    sum_g -= (p >> 8) & 0xff;
                    sum_b -= p & 0xff;
                    count -= 1;
                }
            }

            if count > 0 {
                temp[row * w + col] = ((sum_a / count) << 24)
                    | ((sum_r / count) << 16)
                    | ((sum_g / count) << 8)
                    | (sum_b / count);
            }
        }
    }

    // Vertical sliding-window pass
    for col in 0..w {
        let x = min_x + col as u32;
        let mut sum_a = 0u32;
        let mut sum_r = 0u32;
        let mut sum_g = 0u32;
        let mut sum_b = 0u32;
        let mut count = 0u32;

        let bottom_init = r.min(h.saturating_sub(1));
        for cy in 0..=bottom_init {
            let p = temp[cy * w + col];
            sum_a += (p >> 24) & 0xff;
            sum_r += (p >> 16) & 0xff;
            sum_g += (p >> 8) & 0xff;
            sum_b += p & 0xff;
            count += 1;
        }

        for row in 0..h {
            if row > 0 {
                if row + r < h {
                    let p = temp[(row + r) * w + col];
                    sum_a += (p >> 24) & 0xff;
                    sum_r += (p >> 16) & 0xff;
                    sum_g += (p >> 8) & 0xff;
                    sum_b += p & 0xff;
                    count += 1;
                }
                if row > r {
                    let p = temp[(row - r - 1) * w + col];
                    sum_a -= (p >> 24) & 0xff;
                    sum_r -= (p >> 16) & 0xff;
                    sum_g -= (p >> 8) & 0xff;
                    sum_b -= p & 0xff;
                    count -= 1;
                }
            }

            if count > 0 {
                let y = min_y + row as u32;
                buffer[(y * buf_width + x) as usize] = ((sum_a / count) << 24)
                    | ((sum_r / count) << 16)
                    | ((sum_g / count) << 8)
                    | (sum_b / count);
            }
        }
    }
}

/// Fill a rectangle in the pixel buffer with default blend mode.
#[allow(clippy::too_many_arguments)]
fn fill_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    fill_rect_mode(
        buffer,
        buf_width,
        buf_height,
        x,
        y,
        w,
        h,
        color,
        mango_css::values::BlendMode::Normal,
    );
}

/// Fill a rectangle in the pixel buffer with a specified CSS blend mode.
#[allow(clippy::too_many_arguments)]
fn fill_rect_mode(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
    blend_mode: mango_css::values::BlendMode,
) {
    if color.a == 0 || w == 0 || h == 0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let solid_pixel = color.to_rgb_u32();
    let has_alpha = color.a < 255 || blend_mode != mango_css::values::BlendMode::Normal;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x.saturating_add(w as i32)).clamp(0, buf_width as i32);
    let y1 = (y.saturating_add(h as i32)).clamp(0, buf_height as i32);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;

    for py in y_start..y_end {
        let row_offset = (py * buf_width) as usize;
        for px in x_start..x_end {
            let idx = row_offset + px as usize;
            if idx < buffer.len() {
                if has_alpha {
                    buffer[idx] = blend_pixel_mode(color, buffer[idx], blend_mode);
                } else {
                    buffer[idx] = solid_pixel;
                }
            }
        }
    }
}

fn is_inside_rounded_rect(fx: f32, fy: f32, rect: &mango_core::Rect, radii: [f32; 4]) -> bool {
    let rx = rect.x();
    let ry = rect.y();
    let rw = rect.width();
    let rh = rect.height();
    if fx < rx || fx >= rx + rw || fy < ry || fy >= ry + rh {
        return false;
    }
    let max_r = (rw * 0.5).min(rh * 0.5);
    let r_tl = radii[0].max(0.0).min(max_r);
    let r_tr = radii[1].max(0.0).min(max_r);
    let r_br = radii[2].max(0.0).min(max_r);
    let r_bl = radii[3].max(0.0).min(max_r);

    if r_tl > 0.0 && fx < rx + r_tl && fy < ry + r_tl {
        let dx = (rx + r_tl) - fx;
        let dy = (ry + r_tl) - fy;
        if dx * dx + dy * dy > r_tl * r_tl {
            return false;
        }
    } else if r_tr > 0.0 && fx > rx + rw - r_tr && fy < ry + r_tr {
        let dx = fx - (rx + rw - r_tr);
        let dy = (ry + r_tr) - fy;
        if dx * dx + dy * dy > r_tr * r_tr {
            return false;
        }
    } else if r_br > 0.0 && fx > rx + rw - r_br && fy > ry + rh - r_br {
        let dx = fx - (rx + rw - r_br);
        let dy = fy - (ry + rh - r_br);
        if dx * dx + dy * dy > r_br * r_br {
            return false;
        }
    } else if r_bl > 0.0 && fx < rx + r_bl && fy > ry + rh - r_bl {
        let dx = (rx + r_bl) - fx;
        let dy = fy - (ry + rh - r_bl);
        if dx * dx + dy * dy > r_bl * r_bl {
            return false;
        }
    }
    true
}

/// Fill a rounded rectangle in the pixel buffer, optionally clipping out an inner region.
#[allow(clippy::too_many_arguments)]
fn fill_rounded_rect_clipped(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    color: Color,
    radii: [f32; 4], // [top-left, top-right, bottom-right, bottom-left]
    clip_out: Option<(&mango_core::Rect, [f32; 4])>,
    clip: mango_core::Rect,
) {
    if color.a == 0 {
        return;
    }
    let rx = rect.x();
    let ry = rect.y();
    let rw = rect.width();
    let rh = rect.height();

    if rw <= 0.0 || rh <= 0.0 {
        return;
    }

    // Clamp radii so opposing corners don't overlap
    let max_r = (rw * 0.5).min(rh * 0.5);
    let r_tl = radii[0].max(0.0).min(max_r);
    let r_tr = radii[1].max(0.0).min(max_r);
    let r_br = radii[2].max(0.0).min(max_r);
    let r_bl = radii[3].max(0.0).min(max_r);

    let solid_pixel = color.to_rgb_u32();
    let has_alpha = color.a < 255;

    let cx1 = clip.x().max(0.0) as i32;
    let cy1 = clip.y().max(0.0) as i32;
    let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
    let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

    let x_start = (rx as i32).max(cx1).max(0) as u32;
    let y_start = (ry as i32).max(cy1).max(0) as u32;
    let x_end = (((rx + rw) as i32).min(cx2).max(0) as u32).min(buf_width);
    let y_end = (((ry + rh) as i32).min(cy2).max(0) as u32).min(buf_height);

    for py in y_start..y_end {
        let fy = py as f32 + 0.5;
        let row_offset = (py * buf_width) as usize;
        for px in x_start..x_end {
            let fx = px as f32 + 0.5;

            if let Some((clip_rect, clip_radii)) = clip_out
                && is_inside_rounded_rect(fx, fy, clip_rect, clip_radii)
            {
                continue;
            }

            let mut inside = true;

            // Top-left corner
            if r_tl > 0.0 && fx < rx + r_tl && fy < ry + r_tl {
                let dx = (rx + r_tl) - fx;
                let dy = (ry + r_tl) - fy;
                if dx * dx + dy * dy > r_tl * r_tl {
                    inside = false;
                }
            }
            // Top-right corner
            else if r_tr > 0.0 && fx > rx + rw - r_tr && fy < ry + r_tr {
                let dx = fx - (rx + rw - r_tr);
                let dy = (ry + r_tr) - fy;
                if dx * dx + dy * dy > r_tr * r_tr {
                    inside = false;
                }
            }
            // Bottom-right corner
            else if r_br > 0.0 && fx > rx + rw - r_br && fy > ry + rh - r_br {
                let dx = fx - (rx + rw - r_br);
                let dy = fy - (ry + rh - r_br);
                if dx * dx + dy * dy > r_br * r_br {
                    inside = false;
                }
            }
            // Bottom-left corner
            else if r_bl > 0.0 && fx < rx + r_bl && fy > ry + rh - r_bl {
                let dx = (rx + r_bl) - fx;
                let dy = fy - (ry + rh - r_bl);
                if dx * dx + dy * dy > r_bl * r_bl {
                    inside = false;
                }
            }

            if inside {
                let idx = row_offset + px as usize;
                if idx < buffer.len() {
                    if has_alpha {
                        buffer[idx] = blend_pixel(color, buffer[idx]);
                    } else {
                        buffer[idx] = solid_pixel;
                    }
                }
            }
        }
    }
}

/// Fill a rounded rectangle in the pixel buffer.
#[allow(clippy::too_many_arguments)]
fn fill_rounded_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    rect: &mango_core::Rect,
    color: Color,
    radii: [f32; 4], // [top-left, top-right, bottom-right, bottom-left]
    clip: mango_core::Rect,
) {
    if radii[0] == 0.0 && radii[1] == 0.0 && radii[2] == 0.0 && radii[3] == 0.0 {
        let r = intersect_rect(clip, *rect);
        if r.width() > 0.0 && r.height() > 0.0 {
            fill_rect(
                buffer,
                buf_width,
                buf_height,
                r.x() as i32,
                r.y() as i32,
                r.width() as u32,
                r.height() as u32,
                color,
            );
        }
        return;
    }
    fill_rounded_rect_clipped(
        buffer, buf_width, buf_height, rect, color, radii, None, clip,
    );
}

/// Blits decoded image pixel data into the framebuffer with alpha blending.
#[allow(clippy::too_many_arguments)]
fn blit_image(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    img_width: u32,
    img_height: u32,
    pixels: &[u32],
    clip: mango_core::Rect,
    blend_mode: mango_css::values::BlendMode,
) {
    let cx1 = clip.x().max(0.0) as i32;
    let cy1 = clip.y().max(0.0) as i32;
    let cx2 = ((clip.x() + clip.width()) as i32).min(buf_width as i32);
    let cy2 = ((clip.y() + clip.height()) as i32).min(buf_height as i32);

    let is_exact = pixels.len() == (img_width * img_height) as usize;
    let (src_w, src_h) = if is_exact || img_height == 0 || img_width == 0 {
        (img_width, img_height)
    } else {
        let aspect = img_width as f32 / img_height as f32;
        let h = ((pixels.len() as f32 / aspect).sqrt()).round().max(1.0) as u32;
        let w = (pixels.len() as u32 / h).max(1);
        (w, h)
    };

    for iy in 0..img_height {
        let dest_y = y + iy as i32;
        if dest_y < cy1 || dest_y >= cy2 {
            continue;
        }

        for ix in 0..img_width {
            let dest_x = x + ix as i32;
            if dest_x < cx1 || dest_x >= cx2 {
                continue;
            }

            let pixel = if is_exact {
                let src_idx = (iy * img_width + ix) as usize;
                if src_idx >= pixels.len() {
                    continue;
                }
                pixels[src_idx]
            } else {
                let sx = (ix as f32 + 0.5) * (src_w as f32 / img_width as f32) - 0.5;
                let sy = (iy as f32 + 0.5) * (src_h as f32 / img_height as f32) - 0.5;
                sample_bilinear(pixels, src_w, src_h, sx, sy)
            };

            let alpha = (pixel >> 24) & 0xFF;
            let dest_idx = (dest_y as u32 * buf_width + dest_x as u32) as usize;
            if dest_idx >= buffer.len() {
                continue;
            }

            if alpha == 0 {
                continue;
            }
            let c = Color::rgba(
                ((pixel >> 16) & 0xFF) as u8,
                ((pixel >> 8) & 0xFF) as u8,
                (pixel & 0xFF) as u8,
                alpha as u8,
            );
            if alpha == 255 && blend_mode == mango_css::values::BlendMode::Normal {
                buffer[dest_idx] = pixel & 0x00FFFFFF;
            } else {
                buffer[dest_idx] = blend_pixel_mode(c, buffer[dest_idx], blend_mode);
            }
        }
    }
}

#[inline]
fn sample_bilinear(pixels: &[u32], src_w: u32, src_h: u32, sx: f32, sy: f32) -> u32 {
    let w = src_w as usize;
    let h = src_h as usize;
    if w == 0 || h == 0 || pixels.is_empty() {
        return 0;
    }
    let sx_clamped = sx.clamp(0.0, (w.saturating_sub(1)) as f32);
    let sy_clamped = sy.clamp(0.0, (h.saturating_sub(1)) as f32);
    let x0 = sx_clamped.floor() as usize;
    let y0 = sy_clamped.floor() as usize;
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let fx = sx_clamped - x0 as f32;
    let fy = sy_clamped - y0 as f32;

    let p00 = pixels.get(y0 * w + x0).copied().unwrap_or(0);
    let p10 = pixels.get(y0 * w + x1).copied().unwrap_or(0);
    let p01 = pixels.get(y1 * w + x0).copied().unwrap_or(0);
    let p11 = pixels.get(y1 * w + x1).copied().unwrap_or(0);

    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    let a00 = ((p00 >> 24) & 0xFF) as f32;
    let a10 = ((p10 >> 24) & 0xFF) as f32;
    let a01 = ((p01 >> 24) & 0xFF) as f32;
    let a11 = ((p11 >> 24) & 0xFF) as f32;

    let a = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
    let out_a = a.clamp(0.0, 255.0).round() as u32;

    let has_alpha = a00 > 0.0 || a10 > 0.0 || a01 > 0.0 || a11 > 0.0;
    let alpha_factor = if has_alpha {
        if out_a == 0 {
            return 0;
        }
        out_a as f32 / 255.0
    } else {
        1.0
    };

    let r00 = ((p00 >> 16) & 0xFF) as f32 * if has_alpha { a00 / 255.0 } else { 1.0 };
    let r10 = ((p10 >> 16) & 0xFF) as f32 * if has_alpha { a10 / 255.0 } else { 1.0 };
    let r01 = ((p01 >> 16) & 0xFF) as f32 * if has_alpha { a01 / 255.0 } else { 1.0 };
    let r11 = ((p11 >> 16) & 0xFF) as f32 * if has_alpha { a11 / 255.0 } else { 1.0 };
    let pr = r00 * w00 + r10 * w10 + r01 * w01 + r11 * w11;

    let g00 = ((p00 >> 8) & 0xFF) as f32 * if has_alpha { a00 / 255.0 } else { 1.0 };
    let g10 = ((p10 >> 8) & 0xFF) as f32 * if has_alpha { a10 / 255.0 } else { 1.0 };
    let g01 = ((p01 >> 8) & 0xFF) as f32 * if has_alpha { a01 / 255.0 } else { 1.0 };
    let g11 = ((p11 >> 8) & 0xFF) as f32 * if has_alpha { a11 / 255.0 } else { 1.0 };
    let pg = g00 * w00 + g10 * w10 + g01 * w01 + g11 * w11;

    let b00 = (p00 & 0xFF) as f32 * if has_alpha { a00 / 255.0 } else { 1.0 };
    let b10 = (p10 & 0xFF) as f32 * if has_alpha { a10 / 255.0 } else { 1.0 };
    let b01 = (p01 & 0xFF) as f32 * if has_alpha { a01 / 255.0 } else { 1.0 };
    let b11 = (p11 & 0xFF) as f32 * if has_alpha { a11 / 255.0 } else { 1.0 };
    let pb = b00 * w00 + b10 * w10 + b01 * w01 + b11 * w11;

    let out_r = ((pr / alpha_factor).clamp(0.0, 255.0).round() as u32).min(255);
    let out_g = ((pg / alpha_factor).clamp(0.0, 255.0).round() as u32).min(255);
    let out_b = ((pb / alpha_factor).clamp(0.0, 255.0).round() as u32).min(255);

    if has_alpha {
        (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b
    } else {
        (out_r << 16) | (out_g << 8) | out_b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Rect;

    #[test]
    fn test_blend_pixel() {
        let white = 0x00FFFFFF;
        let black_half = Color::rgba(0, 0, 0, 128);
        let blended = blend_pixel(black_half, white);
        let r = (blended >> 16) & 0xFF;
        let g = (blended >> 8) & 0xFF;
        let b = blended & 0xFF;
        // ~127
        assert!((120..=135).contains(&r));
        assert!((120..=135).contains(&g));
        assert!((120..=135).contains(&b));
    }

    #[test]
    fn test_fill_rounded_rect_corners() {
        let mut buffer = vec![0u32; 100 * 100];
        let rect = Rect::new(10.0, 10.0, 80.0, 80.0);
        let radii = [20.0, 20.0, 20.0, 20.0];
        let red = Color::rgb(255, 0, 0);

        fill_rounded_rect(
            &mut buffer,
            100,
            100,
            &rect,
            red,
            radii,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );

        // Center should definitely be red
        assert_eq!(buffer[50 * 100 + 50], 0x00FF0000);

        // Very top-left pixel (10, 10) of the bounding box should NOT be red due to radius 20
        assert_eq!(buffer[10 * 100 + 10], 0);

        // (30, 30) should be inside
        assert_eq!(buffer[30 * 100 + 30], 0x00FF0000);
    }

    #[test]
    fn test_push_transform_moves_fill() {
        let mut buffer = vec![0u32; 60 * 60];
        let mut dl = DisplayList::new();
        // Draw a red 10x10 square, then the same square translated by (30, 30).
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            color: Color::RED,
        });
        dl.push(DisplayCommand::PushTransform {
            matrix: [1.0, 0.0, 0.0, 1.0, 30.0, 30.0],
        });
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            color: Color::GREEN,
        });
        dl.push(DisplayCommand::PopTransform);

        paint(&dl, &mut buffer, 60, 60);

        assert_eq!(
            buffer[5 * 60 + 5],
            0x00FF0000,
            "untransformed square stays red"
        );
        // CSS `green` is #008000, i.e. 0x00008000 in the 0x00RRGGBB buffer format.
        assert_eq!(
            buffer[35 * 60 + 35],
            0x00008000,
            "translated square lands at +30,+30"
        );
        assert_eq!(
            buffer[5 * 60 + 35],
            0,
            "translation does not smear horizontally"
        );
    }

    #[test]
    fn test_push_transform_scales_fill() {
        let mut buffer = vec![0u32; 40 * 40];
        let mut dl = DisplayList::new();
        dl.push(DisplayCommand::PushTransform {
            matrix: [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
        });
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            color: Color::BLUE,
        });
        dl.push(DisplayCommand::PopTransform);

        paint(&dl, &mut buffer, 40, 40);

        assert_eq!(
            buffer[10 * 40 + 10],
            0x000000FF,
            "scaled fill covers the scaled area"
        );
        assert_eq!(
            buffer[25 * 40 + 25],
            0,
            "scaled fill stops at 2x the source rect"
        );
    }

    #[test]
    fn test_fill_gradient_paints_stops() {
        let mut buffer = vec![0u32; 40 * 10];
        let gradient = mango_css::values::Gradient::Linear {
            angle_deg: 90.0, // to right
            stops: vec![
                mango_css::values::ColorStop::new(Color::RED, Some(0.0), None),
                mango_css::values::ColorStop::new(Color::BLUE, Some(1.0), None),
            ],
            repeating: false,
        };
        let mut dl = DisplayList::new();
        dl.push(DisplayCommand::FillGradient {
            rect: Rect::new(0.0, 0.0, 40.0, 10.0),
            gradient: Box::new(gradient),
            radii: [0.0; 4],
            opacity: 1.0,
        });
        paint(&dl, &mut buffer, 40, 10);

        let left = buffer[5 * 40 + 1];
        let right = buffer[5 * 40 + 38];
        assert!(
            (left >> 16) & 0xFF > 150,
            "left edge is mostly red, got {left:#08x}"
        );
        assert!(
            right & 0xFF > 150,
            "right edge is mostly blue, got {right:#08x}"
        );
    }

    #[test]
    fn test_render_gradients_and_colors() {
        let mut buffer = vec![0u32; 50 * 50];

        // 1. Render conic gradient
        let conic = mango_css::values::Gradient::Conic {
            angle_deg: 45.0,
            stops: vec![
                mango_css::values::ColorStop::new(Color::rgb(255, 0, 0), Some(0.0), None),
                mango_css::values::ColorStop::new(Color::rgb(0, 255, 0), Some(0.5), None),
                mango_css::values::ColorStop::new(Color::rgb(0, 0, 255), Some(1.0), None),
            ],
            repeating: false,
        };
        let mut dl = DisplayList::new();
        dl.push(DisplayCommand::FillGradient {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
            gradient: Box::new(conic),
            radii: [4.0; 4],
            opacity: 0.9,
        });
        paint(&dl, &mut buffer, 50, 50);
        assert!(buffer.iter().any(|&p| p != 0));

        // 2. Render repeating radial gradient
        let mut buf2 = vec![0u32; 60 * 60];
        let rep_rad = mango_css::values::Gradient::Radial {
            stops: vec![
                mango_css::values::ColorStop::new(Color::rgb(255, 255, 255), Some(0.0), None),
                mango_css::values::ColorStop::new(Color::rgb(0, 0, 0), Some(0.2), None),
            ],
            repeating: true,
        };
        let mut dl2 = DisplayList::new();
        dl2.push(DisplayCommand::FillGradient {
            rect: Rect::new(0.0, 0.0, 60.0, 60.0),
            gradient: Box::new(rep_rad),
            radii: [0.0; 4],
            opacity: 1.0,
        });
        paint(&dl2, &mut buf2, 60, 60);
        assert!(buf2.iter().any(|&p| p != 0));
    }

    #[test]
    fn test_clipping_rect_pipeline() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut dl = DisplayList::new();
        // Clip to 20..80 in both X and Y
        dl.push(DisplayCommand::PushClip {
            rect: Rect::new(20.0, 20.0, 60.0, 60.0),
        });
        // Draw a giant red rectangle covering 0..100
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::RED,
        });
        dl.push(DisplayCommand::PopClip);

        paint(&dl, &mut buffer, 100, 100);

        // Outside clip: pixel (10, 10) should be untouched (0)
        assert_eq!(buffer[10 * 100 + 10], 0);
        assert_eq!(buffer[90 * 100 + 90], 0);

        // Inside clip: pixel (30, 30) and (70, 70) should be red
        assert_eq!(buffer[30 * 100 + 30], 0x00FF0000);
        assert_eq!(buffer[70 * 100 + 70], 0x00FF0000);
    }

    #[test]
    fn test_backdrop_filter_and_clip_path() {
        let mut buffer = vec![0x00FFFFFFu32; 100 * 100];
        let mut dl = DisplayList::new();
        // Invert backdrop filter over (20, 20, 40, 40)
        dl.push(DisplayCommand::PushBackdropFilter {
            filters: vec![mango_css::values::FilterFunction::Invert(1.0)],
            rect: Rect::new(20.0, 20.0, 40.0, 40.0),
        });
        paint(&dl, &mut buffer, 100, 100);

        // Outside filter: white (0x00FFFFFF)
        assert_eq!(buffer[10 * 100 + 10] & 0x00ffffff, 0x00ffffff);
        // Inside inverted filter: black (0x00000000)
        assert_eq!(buffer[30 * 100 + 30] & 0x00ffffff, 0x00000000);

        // Test clip path circle
        let mut dl2 = DisplayList::new();
        dl2.push(DisplayCommand::PushClipPath {
            clip_path: Box::new(mango_css::values::ClipPath::Circle {
                radius: mango_css::values::Length::Px(20.0),
                center_x: mango_css::values::Length::Px(50.0),
                center_y: mango_css::values::Length::Px(50.0),
            }),
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        });
        dl2.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::RED,
        });
        dl2.push(DisplayCommand::PopClipPath);

        let mut buffer2 = vec![0u32; 100 * 100];
        paint(&dl2, &mut buffer2, 100, 100);
        // Center (50, 50) is inside circle radius 20: RED
        assert_eq!(buffer2[50 * 100 + 50], 0x00FF0000);
        // (10, 10) is outside: untouched (0)
        assert_eq!(buffer2[10 * 100 + 10], 0);
    }
}
