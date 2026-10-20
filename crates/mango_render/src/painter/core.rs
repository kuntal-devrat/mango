//! Main painter rasterization loop and command dispatcher.

use mango_core::Rect;

use crate::damage::DamageRegion;
use crate::display_list::{DisplayCommand, DisplayList};
use crate::painter::affine::{
    IDENTITY, apply_matrix, blit_image_transformed, concat_matrix, fill_rect_transformed,
    is_identity, is_pure_translation, transformed_bounds,
};
use crate::painter::blend::fill_rect_mode;
use crate::painter::borders::{draw_border_image, paint_border};
use crate::painter::effects::{apply_filter_to_rect, restore_outside_clip_path, snapshot_rect};
use crate::painter::image::blit_image;
use crate::painter::shadows::{paint_box_shadow, paint_text_shadow};
use crate::painter::shapes::{draw_line_segment, fill_gradient, fill_rounded_rect, intersect_rect};
use crate::painter::text::{paint_text, paint_text_with_gradient};

/// Rasterizes a [`DisplayList`] into a pixel buffer.
///
/// The buffer is a flat `&mut [u32]` in canonical `0xAARRGGBB` format
/// (alpha in the high byte; fully opaque = `0xFFRRGGBB`) with the given
/// width and height. Scroll offset is NOT applied here — the caller must
/// translate coordinates before building the display list.
pub fn paint(display_list: &DisplayList, buffer: &mut [u32], buf_width: u32, buf_height: u32) {
    paint_with_clip(
        display_list,
        buffer,
        buf_width,
        buf_height,
        Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32),
    );
}

/// Rasterizes only the damaged areas of a [`DisplayList`] into a pixel buffer.
pub fn paint_damage(
    display_list: &DisplayList,
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    damage: &DamageRegion,
) {
    if damage.is_empty() {
        return;
    }
    for rect in damage.rects() {
        paint_with_clip(display_list, buffer, buf_width, buf_height, *rect);
    }
}

/// Rasterizes a [`DisplayList`] restricted to a specified clipping rectangle.
///
/// Commands outside the bounding box of `clip_rect` are skipped, minimizing
/// rasterization cost during localized damage repairs and tile generation.
pub fn paint_with_clip(
    display_list: &DisplayList,
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    clip_rect: Rect,
) {
    let surface_clip = Rect::new(0.0, 0.0, buf_width as f32, buf_height as f32);
    let default_clip = intersect_rect(surface_clip, clip_rect);
    if default_clip.width() <= 0.0 || default_clip.height() <= 0.0 {
        return;
    }

    let mut clip_stack: Vec<Rect> = Vec::new();
    let mut transform_stack: Vec<[f32; 6]> = Vec::new();
    let mut blend_mode_stack: Vec<mango_css::values::BlendMode> = Vec::new();
    let mut filter_stack: Vec<(Vec<mango_css::values::FilterFunction>, Rect)> = Vec::new();
    let mut clip_path_stack: Vec<(mango_css::values::ClipPath, Rect, Vec<u32>)> = Vec::new();

    for command in display_list.iter() {
        let active_clip = clip_stack.last().copied().unwrap_or(default_clip);
        let transform = transform_stack.last().copied().unwrap_or(IDENTITY);
        let active_blend = blend_mode_stack
            .last()
            .copied()
            .unwrap_or(mango_css::values::BlendMode::Normal);

        // Fast bounding box culling: skip drawing commands that cannot touch active_clip
        if let Some(bounds) = command.bounds() {
            let eff_bounds = if is_identity(transform) {
                Some(bounds)
            } else if let Some((tx, ty)) = is_pure_translation(transform) {
                Some(Rect::new(
                    bounds.x() + tx,
                    bounds.y() + ty,
                    bounds.width(),
                    bounds.height(),
                ))
            } else {
                None
            };
            if let Some(b) = eff_bounds
                && !b.intersects(&active_clip)
            {
                continue;
            }
        }

        match command {
            DisplayCommand::PushClip { rect } => {
                let current = clip_stack.last().copied().unwrap_or(default_clip);
                let eff_rect = if is_identity(transform) {
                    *rect
                } else if let Some((tx, ty)) = is_pure_translation(transform) {
                    Rect::new(rect.x() + tx, rect.y() + ty, rect.width(), rect.height())
                } else {
                    transformed_bounds(rect, transform)
                };
                let intersected = intersect_rect(current, eff_rect);
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
                paint_text_shadow(
                    buffer,
                    buf_width,
                    buf_height,
                    text,
                    *x,
                    *y,
                    *color,
                    *font_size,
                    *weight,
                    *family,
                    *style,
                    *blur_radius,
                    *letter_spacing,
                    active_clip,
                );
            }
            DisplayCommand::FillRect { rect, color } => {
                let eff_rect = if is_identity(transform) {
                    *rect
                } else if let Some((tx, ty)) = is_pure_translation(transform) {
                    Rect::new(rect.x() + tx, rect.y() + ty, rect.width(), rect.height())
                } else {
                    fill_rect_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        rect,
                        *color,
                        None,
                        active_clip,
                        transform,
                        active_blend,
                    );
                    continue;
                };
                let r = intersect_rect(active_clip, eff_rect);
                if r.width() > 0.0 && r.height() > 0.0 {
                    fill_rect_mode(
                        buffer,
                        buf_width,
                        buf_height,
                        r.x() as i32,
                        r.y() as i32,
                        r.width() as u32,
                        r.height() as u32,
                        *color,
                        active_blend,
                    );
                }
            }
            DisplayCommand::FillRoundedRect { rect, color, radii } => {
                if is_identity(transform) {
                    fill_rounded_rect(
                        buffer,
                        buf_width,
                        buf_height,
                        rect,
                        *color,
                        *radii,
                        active_clip,
                        active_blend,
                    );
                } else if let Some((tx, ty)) = is_pure_translation(transform) {
                    let moved =
                        Rect::new(rect.x() + tx, rect.y() + ty, rect.width(), rect.height());
                    fill_rounded_rect(
                        buffer,
                        buf_width,
                        buf_height,
                        &moved,
                        *color,
                        *radii,
                        active_clip,
                        active_blend,
                    );
                } else {
                    fill_rect_transformed(
                        buffer,
                        buf_width,
                        buf_height,
                        rect,
                        *color,
                        Some(*radii),
                        active_clip,
                        transform,
                        active_blend,
                    );
                }
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
                paint_box_shadow(
                    buffer,
                    buf_width,
                    buf_height,
                    rect,
                    *color,
                    *offset_x,
                    *offset_y,
                    *blur_radius,
                    *spread_radius,
                    *radii,
                    *inset,
                    active_clip,
                    active_blend,
                );
            }
            DisplayCommand::DrawBorder {
                rect,
                color,
                widths,
                radii,
            } => {
                paint_border(
                    buffer,
                    buf_width,
                    buf_height,
                    rect,
                    *color,
                    widths,
                    radii,
                    active_clip,
                    transform,
                    active_blend,
                );
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
                paint_text(
                    buffer,
                    buf_width,
                    buf_height,
                    text,
                    *x,
                    *y,
                    *color,
                    *font_size,
                    *weight,
                    *family,
                    *style,
                    *decoration,
                    *letter_spacing,
                    active_clip,
                    transform,
                    active_blend,
                );
            }
            DisplayCommand::DrawLine {
                x1,
                y1,
                x2,
                y2,
                color,
                thickness,
            } => {
                if color.a == 0 || *thickness <= 0.0 {
                    continue;
                }
                if is_identity(transform) {
                    draw_line_segment(
                        buffer,
                        buf_width,
                        buf_height,
                        *x1,
                        *y1,
                        *x2,
                        *y2,
                        *color,
                        *thickness,
                        active_clip,
                        active_blend,
                    );
                } else if let Some((tx, ty)) = is_pure_translation(transform) {
                    draw_line_segment(
                        buffer,
                        buf_width,
                        buf_height,
                        *x1 + tx,
                        *y1 + ty,
                        *x2 + tx,
                        *y2 + ty,
                        *color,
                        *thickness,
                        active_clip,
                        active_blend,
                    );
                } else {
                    let (dx1, dy1) = apply_matrix(transform, *x1, *y1);
                    let (dx2, dy2) = apply_matrix(transform, *x2, *y2);
                    let sx = (transform[0] * transform[0] + transform[1] * transform[1]).sqrt();
                    let sy = (transform[2] * transform[2] + transform[3] * transform[3]).sqrt();
                    let mean_scale = ((sx + sy) * 0.5).max(0.0001);
                    draw_line_segment(
                        buffer,
                        buf_width,
                        buf_height,
                        dx1,
                        dy1,
                        dx2,
                        dy2,
                        *color,
                        *thickness * mean_scale,
                        active_clip,
                        active_blend,
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
                    if let Some((tx, ty)) = is_pure_translation(transform) {
                        blit_image(
                            buffer,
                            buf_width,
                            buf_height,
                            (*x + tx) as i32,
                            (*y + ty) as i32,
                            *width as u32,
                            *height as u32,
                            pixels,
                            active_clip,
                            active_blend,
                        );
                        continue;
                    }
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
                paint_text_with_gradient(
                    buffer,
                    buf_width,
                    buf_height,
                    text,
                    *x,
                    *y,
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
