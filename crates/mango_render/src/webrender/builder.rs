//! Display list builder for WebRender.
//!
//! Translates high-level Mango [`DisplayList`] commands into hardware-accelerated
//! WebRender display items and manages spatial hierarchies, clip chains, and image uploads.

use webrender::api::units::*;
use webrender::api::*;
use webrender::{RenderApi, Transaction};

use super::types::*;
use crate::display_list::{DisplayCommand, DisplayList};
use crate::font::font_manager;

/// Result of building a WebRender display list, including the display list itself
/// and any transient GPU resource keys that should be cleaned up after presentation.
pub struct BuiltFrame {
    pub pipeline_id: PipelineId,
    pub display_list: BuiltDisplayList,
    pub transient_images: Vec<ImageKey>,
}

/// Translates a [`DisplayList`] into WebRender display items.
pub fn build_webrender_display_list(
    display_list: &DisplayList,
    pipeline_id: PipelineId,
    width: u32,
    height: u32,
    api: &RenderApi,
    txn: &mut Transaction,
) -> BuiltFrame {
    let mut builder = DisplayListBuilder::new(pipeline_id);
    builder.begin();

    let root_spatial = SpatialId::root_scroll_node(pipeline_id);
    let mut spatial_stack = vec![root_spatial];
    let mut clip_stack = vec![ClipChainId::INVALID];
    let mut transient_images = Vec::new();

    let viewport_rect = LayoutRect::from_origin_and_size(
        LayoutPoint::zero(),
        LayoutSize::new(width as f32, height as f32),
    );

    for cmd in display_list.iter() {
        let current_spatial = *spatial_stack.last().unwrap();
        let current_clip = *clip_stack.last().unwrap();
        let space_and_clip = SpaceAndClipInfo {
            spatial_id: current_spatial,
            clip_chain_id: current_clip,
        };
        let common = CommonItemProperties::new(viewport_rect, space_and_clip);

        match cmd {
            DisplayCommand::FillRect { rect, color } => {
                if color.a > 0 && rect.width() > 0.0 && rect.height() > 0.0 {
                    builder.push_rect(&common, to_layout_rect(rect), to_colorf(*color));
                }
            }

            DisplayCommand::FillRoundedRect { rect, color, radii } => {
                if color.a > 0 && rect.width() > 0.0 && rect.height() > 0.0 {
                    let has_radii = radii.iter().any(|r| *r > 0.0);
                    if has_radii {
                        let region = ComplexClipRegion::new(
                            to_layout_rect(rect),
                            to_border_radius(*radii),
                            ClipMode::Clip,
                        );
                        let clip_id = builder.define_clip_rounded_rect(current_spatial, region);
                        let parent_clip = if current_clip != ClipChainId::INVALID {
                            Some(current_clip)
                        } else {
                            None
                        };
                        let clip_chain = builder.define_clip_chain(parent_clip, [clip_id]);
                        let rounded_space_and_clip = SpaceAndClipInfo {
                            spatial_id: current_spatial,
                            clip_chain_id: clip_chain,
                        };
                        let rounded_common =
                            CommonItemProperties::new(viewport_rect, rounded_space_and_clip);
                        builder.push_rect(&rounded_common, to_layout_rect(rect), to_colorf(*color));
                    } else {
                        builder.push_rect(&common, to_layout_rect(rect), to_colorf(*color));
                    }
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
                if color.a > 0 {
                    let box_bounds = to_layout_rect(rect);
                    let offset = LayoutVector2D::new(*offset_x, *offset_y);
                    let border_rad = to_border_radius(*radii);
                    let clip_mode = if *inset {
                        BoxShadowClipMode::Inset
                    } else {
                        BoxShadowClipMode::Outset
                    };
                    builder.push_box_shadow(
                        &common,
                        box_bounds,
                        offset,
                        to_colorf(*color),
                        *blur_radius,
                        *spread_radius,
                        border_rad,
                        border_rad,
                        clip_mode,
                    );
                }
            }

            DisplayCommand::DrawBorder {
                rect,
                color,
                widths,
                radii,
            } => {
                if color.a > 0
                    && (widths.top > 0.0
                        || widths.right > 0.0
                        || widths.bottom > 0.0
                        || widths.left > 0.0)
                {
                    let side = BorderSide {
                        color: to_colorf(*color),
                        style: BorderStyle::Solid,
                    };
                    let normal = NormalBorder {
                        left: side,
                        right: side,
                        top: side,
                        bottom: side,
                        radius: to_border_radius(*radii),
                        do_aa: true,
                    };
                    builder.push_border(
                        &common,
                        to_layout_rect(rect),
                        to_border_widths(widths),
                        BorderDetails::Normal(normal),
                    );
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
                if color.a > 0 && *thickness > 0.0 {
                    let min_x = x1.min(*x2) - thickness / 2.0;
                    let min_y = y1.min(*y2) - thickness / 2.0;
                    let max_x = x1.max(*x2) + thickness / 2.0;
                    let max_y = y1.max(*y2) + thickness / 2.0;
                    let line_rect = LayoutRect::from_origin_and_size(
                        to_layout_point(min_x, min_y),
                        to_layout_size(max_x - min_x, max_y - min_y),
                    );
                    builder.push_rect(&common, line_rect, to_colorf(*color));
                }
            }

            DisplayCommand::DrawImage {
                x,
                y,
                width: w,
                height: h,
                pixels,
            } => {
                if *w > 0.0 && *h > 0.0 && !pixels.is_empty() {
                    let (src_w, src_h) = if pixels.len() == (w.round() * h.round()) as usize {
                        (w.round() as u32, h.round() as u32)
                    } else {
                        let aspect = *w / *h;
                        let sh = ((pixels.len() as f32 / aspect).sqrt()).round().max(1.0) as u32;
                        let sw = (pixels.len() as u32 / sh).max(1);
                        (sw, sh)
                    };

                    let img_key = upload_pixels(api, txn, src_w, src_h, pixels);
                    transient_images.push(img_key);
                    let bounds = LayoutRect::from_origin_and_size(
                        to_layout_point(*x, *y),
                        to_layout_size(*w, *h),
                    );
                    builder.push_image(
                        &common,
                        bounds,
                        ImageRendering::Auto,
                        AlphaType::PremultipliedAlpha,
                        img_key,
                        ColorF::WHITE,
                    );
                }
            }

            DisplayCommand::FillGradient {
                rect,
                gradient,
                radii: _,
                opacity,
            } => {
                if rect.width() > 0.0 && rect.height() > 0.0 && *opacity > 0.0 {
                    let grad_w = rect.width().ceil().max(1.0) as u32;
                    let grad_h = rect.height().ceil().max(1.0) as u32;
                    let raw_pixels = gradient.rasterize(grad_w, grad_h);
                    if !raw_pixels.is_empty() {
                        let img_key = upload_pixels(api, txn, grad_w, grad_h, &raw_pixels);
                        transient_images.push(img_key);
                        let bounds = to_layout_rect(rect);
                        builder.push_image(
                            &common,
                            bounds,
                            ImageRendering::Auto,
                            AlphaType::PremultipliedAlpha,
                            img_key,
                            ColorF::new(1.0, 1.0, 1.0, *opacity),
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
                decoration: _,
                letter_spacing,
            } => {
                if color.a > 0 && *font_size > 0.5 && !text.is_empty() {
                    let fm = font_manager();
                    let (tw, th) = fm.measure_text_with_spacing(
                        text,
                        *font_size,
                        *weight,
                        *family,
                        *letter_spacing,
                    );
                    let u_w = tw.ceil().max(1.0) as u32;
                    let u_h = th.ceil().max(1.0) as u32;
                    let mut text_buf = vec![0u32; (u_w * u_h) as usize];
                    fm.rasterize_text_clipped(
                        &mut text_buf,
                        u_w,
                        u_h,
                        0,
                        0,
                        text,
                        *color,
                        *font_size,
                        *weight,
                        *family,
                        *style,
                        *letter_spacing,
                        mango_core::Rect::new(0.0, 0.0, tw, th),
                    );

                    let img_key = upload_pixels(api, txn, u_w, u_h, &text_buf);
                    transient_images.push(img_key);
                    let bounds = LayoutRect::from_origin_and_size(
                        to_layout_point(*x, *y),
                        to_layout_size(tw, th),
                    );
                    builder.push_image(
                        &common,
                        bounds,
                        ImageRendering::Auto,
                        AlphaType::PremultipliedAlpha,
                        img_key,
                        ColorF::WHITE,
                    );
                }
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
                blur_radius: _,
                letter_spacing,
            } => {
                if color.a > 0 && *font_size > 0.5 && !text.is_empty() {
                    let fm = font_manager();
                    let (tw, th) = fm.measure_text_with_spacing(
                        text,
                        *font_size,
                        *weight,
                        *family,
                        *letter_spacing,
                    );
                    let u_w = tw.ceil().max(1.0) as u32;
                    let u_h = th.ceil().max(1.0) as u32;
                    let mut text_buf = vec![0u32; (u_w * u_h) as usize];
                    fm.rasterize_text_clipped(
                        &mut text_buf,
                        u_w,
                        u_h,
                        0,
                        0,
                        text,
                        *color,
                        *font_size,
                        *weight,
                        *family,
                        *style,
                        *letter_spacing,
                        mango_core::Rect::new(0.0, 0.0, tw, th),
                    );

                    let img_key = upload_pixels(api, txn, u_w, u_h, &text_buf);
                    transient_images.push(img_key);
                    let bounds = LayoutRect::from_origin_and_size(
                        to_layout_point(*x, *y),
                        to_layout_size(tw, th),
                    );
                    builder.push_image(
                        &common,
                        bounds,
                        ImageRendering::Auto,
                        AlphaType::PremultipliedAlpha,
                        img_key,
                        ColorF::WHITE,
                    );
                }
            }

            DisplayCommand::DrawBorderImage {
                rect,
                pixels,
                img_width,
                img_height,
                ..
            } => {
                if rect.width() > 0.0 && rect.height() > 0.0 && !pixels.is_empty() {
                    let img_key = upload_pixels(api, txn, *img_width, *img_height, pixels);
                    transient_images.push(img_key);
                    builder.push_image(
                        &common,
                        to_layout_rect(rect),
                        ImageRendering::Auto,
                        AlphaType::PremultipliedAlpha,
                        img_key,
                        ColorF::WHITE,
                    );
                }
            }

            DisplayCommand::DrawTextWithGradient {
                text,
                x,
                y,
                font_size,
                weight,
                family,
                letter_spacing,
                ..
            } => {
                if *font_size > 0.5 && !text.is_empty() {
                    let fm = font_manager();
                    let (tw, th) = fm.measure_text_with_spacing(
                        text,
                        *font_size,
                        *weight,
                        *family,
                        *letter_spacing,
                    );
                    let u_w = tw.ceil().max(1.0) as u32;
                    let u_h = th.ceil().max(1.0) as u32;
                    let mut text_buf = vec![0u32; (u_w * u_h) as usize];
                    fm.rasterize_text_clipped(
                        &mut text_buf,
                        u_w,
                        u_h,
                        0,
                        0,
                        text,
                        mango_core::Color::rgba(255, 255, 255, 255),
                        *font_size,
                        *weight,
                        *family,
                        crate::font::FontStyle::Normal,
                        *letter_spacing,
                        mango_core::Rect::new(0.0, 0.0, tw, th),
                    );

                    let img_key = upload_pixels(api, txn, u_w, u_h, &text_buf);
                    transient_images.push(img_key);
                    let bounds = LayoutRect::from_origin_and_size(
                        to_layout_point(*x, *y),
                        to_layout_size(tw, th),
                    );
                    builder.push_image(
                        &common,
                        bounds,
                        ImageRendering::Auto,
                        AlphaType::PremultipliedAlpha,
                        img_key,
                        ColorF::WHITE,
                    );
                }
            }

            DisplayCommand::PushTransform { matrix } => {
                let transform = to_layout_transform(*matrix);
                let new_spatial = builder.push_reference_frame(
                    LayoutPoint::zero(),
                    current_spatial,
                    TransformStyle::Flat,
                    PropertyBinding::Value(transform),
                    ReferenceFrameKind::Transform {
                        is_2d_scale_translation: false,
                        should_snap: false,
                        paired_with_perspective: false,
                    },
                );
                spatial_stack.push(new_spatial);
            }

            DisplayCommand::PopTransform => {
                if spatial_stack.len() > 1 {
                    builder.pop_reference_frame();
                    spatial_stack.pop();
                }
            }

            DisplayCommand::PushClip { rect } => {
                let clip_id = builder.define_clip_rect(current_spatial, to_layout_rect(rect));
                let parent_clip = if current_clip != ClipChainId::INVALID {
                    Some(current_clip)
                } else {
                    None
                };
                let new_clip_chain = builder.define_clip_chain(parent_clip, [clip_id]);
                clip_stack.push(new_clip_chain);
            }

            DisplayCommand::PopClip => {
                if clip_stack.len() > 1 {
                    clip_stack.pop();
                }
            }

            DisplayCommand::PushFilter { .. }
            | DisplayCommand::PopFilter
            | DisplayCommand::PushBackdropFilter { .. }
            | DisplayCommand::PopBackdropFilter
            | DisplayCommand::PushBlendMode { .. }
            | DisplayCommand::PopBlendMode
            | DisplayCommand::PushClipPath { .. }
            | DisplayCommand::PopClipPath => {
                // Filters and blend modes currently fall through gracefully in display list builder
            }
        }
    }

    let (_, built_dl) = builder.end();
    BuiltFrame {
        pipeline_id,
        display_list: built_dl,
        transient_images,
    }
}

/// Uploads RGBA8/BGRA8 pixels to WebRender.
fn upload_pixels(
    api: &RenderApi,
    txn: &mut Transaction,
    width: u32,
    height: u32,
    pixels: &[u32],
) -> ImageKey {
    let key = api.generate_image_key();
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);
    for &p in pixels.iter().take((width * height) as usize) {
        let mut a = ((p >> 24) & 0xFF) as u8;
        let r = ((p >> 16) & 0xFF) as u8;
        let g = ((p >> 8) & 0xFF) as u8;
        let b = (p & 0xFF) as u8;
        if a == 0 && (r != 0 || g != 0 || b != 0) {
            a = 255;
        }
        bytes.push(r);
        bytes.push(g);
        bytes.push(b);
        bytes.push(a);
    }
    // Pad if pixels slice is smaller than width * height
    while bytes.len() < (width * height * 4) as usize {
        bytes.push(0);
    }

    let descriptor = ImageDescriptor::new(
        width as i32,
        height as i32,
        ImageFormat::RGBA8,
        ImageDescriptorFlags::empty(),
    );
    txn.add_image(key, descriptor, ImageData::new(bytes), None);
    key
}
