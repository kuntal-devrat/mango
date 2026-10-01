//! Display list generation: flattens the laid-out box tree into render commands.
//!
//! Generates background fills, borders, and text drawing commands respecting
//! the CSS stacking and paint order.

use mango_core::{Color, Rect};
use mango_render::display_list::{BorderWidths, DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

/// Generates a [`DisplayList`] from a laid-out box tree.
pub fn build_display_list(root: &LayoutBox) -> DisplayList {
    build_display_list_with_scroll(root, 0.0)
}

/// Generates a [`DisplayList`] with a given vertical scroll offset,
/// properly offsetting `position: sticky` and `position: fixed` elements.
pub fn build_display_list_with_scroll(root: &LayoutBox, scroll_y: f32) -> DisplayList {
    let mut list = DisplayList::new();
    let mut init_offset_y = 0.0;
    if let Some(s) = &root.style
        && s.position == mango_css::values::Position::Fixed
    {
        init_offset_y = scroll_y;
    }
    render_layout_box(root, &mut list, 1.0, 0.0, init_offset_y, scroll_y, None);
    list
}

/// A cached display list record with dirty-tracking and scroll offset (OPT-005).
#[derive(Debug, Clone, Default)]
pub struct DisplayListCache {
    cached_scroll_y: Option<f32>,
    cached_list: Option<DisplayList>,
    dirty_rects: Vec<Rect>,
}

impl DisplayListCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the cached display list if valid, or builds and caches it (OPT-005).
    pub fn get_or_build(&mut self, root: &LayoutBox, scroll_y: f32) -> DisplayList {
        if !root.is_dirty
            && self.cached_scroll_y == Some(scroll_y)
            && let Some(ref list) = self.cached_list
        {
            return list.clone();
        }

        let list = build_display_list_with_scroll(root, scroll_y);
        self.cached_scroll_y = Some(scroll_y);
        self.cached_list = Some(list.clone());
        self.dirty_rects.clear();
        list
    }

    /// Invalidate the display list cache with an optional dirty region (OPT-005).
    pub fn invalidate(&mut self, region: Option<Rect>) {
        self.cached_list = None;
        if let Some(r) = region {
            self.dirty_rects.push(r);
        }
    }

    /// Clears the cached display list completely.
    pub fn clear(&mut self) {
        self.invalidate(None);
    }

    /// Returns the list of dirty rects accumulated since last build.
    pub fn dirty_regions(&self) -> &[Rect] {
        &self.dirty_rects
    }

    /// Returns true if a cached display list is currently available.
    pub fn is_cached(&self) -> bool {
        self.cached_list.is_some()
    }

    /// Updates the cached display list with a newly built one and computes the diff (ARCH-004).
    ///
    /// If an old display list was cached, this returns a `DisplayListDiff` with the minimal
    /// changed commands and consolidated damage rect. Otherwise, it treats the entire new list
    /// as freshly inserted.
    pub fn update_and_diff(
        &mut self,
        new_list: DisplayList,
        scroll_y: f32,
    ) -> mango_render::DisplayListDiff {
        let diff = if let Some(ref old_list) = self.cached_list {
            old_list.diff(&new_list)
        } else {
            let empty = DisplayList::new();
            empty.diff(&new_list)
        };

        if let Some(damage) = diff.damage_rect() {
            self.dirty_rects.push(damage);
        }

        self.cached_scroll_y = Some(scroll_y);
        self.cached_list = Some(new_list);
        diff
    }
}

/// Scales color alpha by opacity factor.
fn apply_opacity(color: Color, opacity: f32) -> Color {
    if opacity >= 1.0 {
        color
    } else {
        let alpha = ((color.a as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
        Color::rgba(color.r, color.g, color.b, alpha)
    }
}

/// Generates a crisp, antialiased magnifying glass search icon in 0xAARRGGBB format.
fn generate_search_icon(size: u32, color: Color) -> Vec<u32> {
    let mut pixels = vec![0u32; (size * size) as usize];
    let cx = size as f32 * 0.38;
    let cy = size as f32 * 0.38;
    let r_out = size as f32 * 0.30;
    let r_in = (size as f32 * 0.18).max(r_out - 2.0);
    let handle_width = (size as f32 * 0.13).max(1.8);

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = px - cx;
            let dy = py - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            // Circular lens rim
            let mut alpha = 0.0f32;
            if dist >= r_in - 0.5 && dist <= r_out + 0.5 {
                let a1 = (dist - (r_in - 0.5)).clamp(0.0, 1.0);
                let a2 = ((r_out + 0.5) - dist).clamp(0.0, 1.0);
                alpha = alpha.max(a1.min(a2));
            }

            // Diagonal handle
            let hx_start = cx + r_out * 0.65;
            let hy_start = cy + r_out * 0.65;
            let hx_end = size as f32 * 0.88;
            let hy_end = size as f32 * 0.88;
            let seg_dx = hx_end - hx_start;
            let seg_dy = hy_end - hy_start;
            let seg_len_sq = seg_dx * seg_dx + seg_dy * seg_dy;
            if seg_len_sq > 0.0 {
                let t = (((px - hx_start) * seg_dx + (py - hy_start) * seg_dy) / seg_len_sq)
                    .clamp(0.0, 1.0);
                let proj_x = hx_start + t * seg_dx;
                let proj_y = hy_start + t * seg_dy;
                let h_dist = ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt();
                let h_alpha = ((handle_width * 0.5 + 0.5) - h_dist).clamp(0.0, 1.0);
                alpha = alpha.max(h_alpha);
            }

            if alpha > 0.0 {
                let a = ((color.a as f32) * alpha).round() as u32;
                pixels[(y * size + x) as usize] = (a << 24)
                    | ((color.r as u32) << 16)
                    | ((color.g as u32) << 8)
                    | (color.b as u32);
            }
        }
    }
    pixels
}

/// Composes two 2D affine matrices: `outer * inner` (apply `inner` first).
fn mul_matrix(outer: [f32; 6], inner: [f32; 6]) -> [f32; 6] {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

/// Resolves a `transform-origin` component against the element's box size.
fn resolve_origin(length: mango_css::values::Length, size: f32) -> f32 {
    match length {
        mango_css::values::Length::Px(px) => px,
        mango_css::values::Length::Percent(pct) => size * pct / 100.0,
        mango_css::values::Length::Em(em) => em * 16.0,
        mango_css::values::Length::Rem(rem) => rem * 16.0,
        _ => 0.0,
    }
}

/// Wraps [`render_layout_box_inner`] to apply CSS visual effects (`mix-blend-mode`, `transform`,
/// `clip-path`, `filter`, and `backdrop-filter`) to the element's whole painted subtree.
fn render_layout_box(
    box_node: &LayoutBox,
    list: &mut DisplayList,
    parent_opacity: f32,
    offset_x: f32,
    offset_y: f32,
    scroll_y: f32,
    containing_block_rect: Option<Rect>,
) {
    let s_ref = box_node.style.as_ref();

    let push_blend =
        s_ref.is_some_and(|st| st.mix_blend_mode != mango_css::values::BlendMode::Normal);
    if push_blend {
        list.push(DisplayCommand::PushBlendMode {
            mode: s_ref.unwrap().mix_blend_mode,
        });
    }

    // A CSS transform establishes a containing block for fixed/absolute descendants
    // and offsets the element and everything inside it.
    let transform_matrix = s_ref.and_then(|s| {
        if s.transform.is_identity() {
            None
        } else {
            let border_box = box_node.dimensions.border_box();
            let origin_x = border_box.x()
                + offset_x
                + resolve_origin(s.transform_origin_x, border_box.width());
            let origin_y = border_box.y()
                + offset_y
                + resolve_origin(s.transform_origin_y, border_box.height());
            // translate(origin) * transform * translate(-origin)
            let to_origin = [1.0, 0.0, 0.0, 1.0, origin_x, origin_y];
            let from_origin = [1.0, 0.0, 0.0, 1.0, -origin_x, -origin_y];
            Some(mul_matrix(
                mul_matrix(
                    to_origin,
                    s.transform
                        .to_matrix_with_size(border_box.width(), border_box.height()),
                ),
                from_origin,
            ))
        }
    });

    if let Some(matrix) = transform_matrix {
        list.push(DisplayCommand::PushTransform { matrix });
    }

    let border_box = box_node.dimensions.border_box();
    let elem_rect = Rect::new(
        border_box.x() + offset_x,
        border_box.y() + offset_y,
        border_box.width(),
        border_box.height(),
    );

    let push_clip_path = s_ref.is_some_and(|st| st.clip_path != mango_css::values::ClipPath::None);
    if push_clip_path {
        list.push(DisplayCommand::PushClipPath {
            clip_path: Box::new(s_ref.unwrap().clip_path.clone()),
            rect: elem_rect,
        });
    }

    let push_filter = s_ref.is_some_and(|st| !st.filter.is_empty());
    if push_filter {
        list.push(DisplayCommand::PushFilter {
            filters: s_ref.unwrap().filter.clone(),
            rect: elem_rect,
        });
    }

    let push_backdrop = s_ref.is_some_and(|st| !st.backdrop_filter.is_empty());
    if push_backdrop {
        list.push(DisplayCommand::PushBackdropFilter {
            filters: s_ref.unwrap().backdrop_filter.clone(),
            rect: elem_rect,
        });
        list.push(DisplayCommand::PopBackdropFilter);
    }

    render_layout_box_inner(
        box_node,
        list,
        parent_opacity,
        offset_x,
        offset_y,
        scroll_y,
        containing_block_rect,
    );

    if push_filter {
        list.push(DisplayCommand::PopFilter);
    }
    if push_clip_path {
        list.push(DisplayCommand::PopClipPath);
    }
    if transform_matrix.is_some() {
        list.push(DisplayCommand::PopTransform);
    }
    if push_blend {
        list.push(DisplayCommand::PopBlendMode);
    }
}

fn render_layout_box_inner(
    box_node: &LayoutBox,
    list: &mut DisplayList,
    parent_opacity: f32,
    offset_x: f32,
    offset_y: f32,
    scroll_y: f32,
    _containing_block_rect: Option<Rect>,
) {
    let style = box_node.style.as_ref();
    let is_visible = style.is_none_or(|s| s.is_visible());
    let opacity = style.map_or(1.0, |s| s.opacity) * parent_opacity;

    if opacity <= 0.001 {
        return;
    }

    if let Some(s) = style
        && (s.overflow_x == mango_css::values::Overflow::Hidden
            || s.overflow_y == mango_css::values::Overflow::Hidden)
        && (box_node.dimensions.padding_box().width() <= 1.0
            || box_node.dimensions.padding_box().height() <= 1.0)
    {
        return;
    }

    let orig_pad_box = box_node.dimensions.padding_box();
    let pad_box = Rect::new(
        orig_pad_box.x() + offset_x,
        orig_pad_box.y() + offset_y,
        orig_pad_box.width(),
        orig_pad_box.height(),
    );

    let orig_border_box = box_node.dimensions.border_box();
    let border_box = Rect::new(
        orig_border_box.x() + offset_x,
        orig_border_box.y() + offset_y,
        orig_border_box.width(),
        orig_border_box.height(),
    );

    let content_x = box_node.dimensions.content.x() + offset_x;
    let content_y = box_node.dimensions.content.y() + offset_y;

    if is_visible {
        let is_text_node = matches!(box_node.box_type, BoxType::TextNode(_));
        if !is_text_node {
            // 0. Paint Box Shadow (before background)
            if let Some(s) = style
                && let Some(bs) = &s.box_shadow
            {
                let shadow_base_rect = Rect::new(
                    border_box.x(),
                    border_box.y(),
                    border_box.width(),
                    border_box.height(),
                );
                if shadow_base_rect.width() > 0.0 && shadow_base_rect.height() > 0.0 {
                    let shadow_color = apply_opacity(bs.color, opacity);
                    list.push(DisplayCommand::DrawBoxShadow {
                        rect: shadow_base_rect,
                        color: shadow_color,
                        offset_x: bs.offset_x,
                        offset_y: bs.offset_y,
                        blur_radius: bs.blur_radius,
                        spread_radius: bs.spread_radius,
                        radii: s.border_radius(),
                        inset: bs.inset,
                    });
                }
            }

            // Modal dialog backdrop: dimming overlay covering viewport
            if box_node.tag_name.as_deref() == Some("dialog")
                && box_node.get_attribute("open").is_some()
                && box_node.get_attribute("data-mango-modal") == Some("true")
            {
                let backdrop_rect = Rect::new(0.0, 0.0, 100_000.0, 100_000.0);
                list.push(DisplayCommand::FillRect {
                    rect: backdrop_rect,
                    color: Color::rgba(0, 0, 0, 102), // 0.4 dimming backdrop
                });
            }

            let content_box = Rect::new(
                content_x,
                content_y,
                box_node.dimensions.content.width(),
                box_node.dimensions.content.height(),
            );

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
                            radii: s.border_radius(),
                        });
                    } else {
                        list.push(DisplayCommand::FillRect {
                            rect: base_rect,
                            color: bg_color,
                        });
                    }
                }

                // Multi-layered or single background (bottom-to-top)
                if !s.background_layers.is_empty() {
                    for layer in s.background_layers.iter().rev() {
                        if layer.clip == mango_css::values::BackgroundClip::Text {
                            continue;
                        }
                        let target_rect = resolve_clip_rect(layer.clip);
                        if target_rect.width() <= 0.0 || target_rect.height() <= 0.0 {
                            continue;
                        }

                        if let Some(gradient) = &layer.gradient {
                            let blend =
                                s.background_blend_mode != mango_css::values::BlendMode::Normal;
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
                                    s.border_radius()
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
                            let blend =
                                s.background_blend_mode != mango_css::values::BlendMode::Normal;
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
                                    s.border_radius()
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
                        mango_css::values::BackgroundSize::Auto => {
                            (img.width as f32, img.height as f32)
                        }
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
                        mango_css::values::Length::Percent(p) => {
                            pad_box.x() + (pad_w - draw_w) * p / 100.0
                        }
                        _ => pad_box.x() + (pad_w - draw_w) / 2.0,
                    };
                    let origin_y = match mask_pos.1 {
                        mango_css::values::Length::Px(px) => pad_box.y() + px,
                        mango_css::values::Length::Percent(p) => {
                            pad_box.y() + (pad_h - draw_h) * p / 100.0
                        }
                        _ => pad_box.y() + (pad_h - draw_h) / 2.0,
                    };

                    let tint =
                        if s.background_color != Color::TRANSPARENT && s.background_color.a > 0 {
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
                        pixels: tinted_pixels,
                    });
                }
            }

            // 2. Paint Borders (on border box)
            if let Some(s) = style {
                let widths = BorderWidths {
                    top: box_node.dimensions.border.top,
                    right: box_node.dimensions.border.right,
                    bottom: box_node.dimensions.border.bottom,
                    left: box_node.dimensions.border.left,
                };

                if widths.top > 0.0
                    || widths.right > 0.0
                    || widths.bottom > 0.0
                    || widths.left > 0.0
                {
                    let border_color = if widths.top > 0.0 {
                        apply_opacity(s.border_top_color, opacity)
                    } else if widths.bottom > 0.0 {
                        apply_opacity(s.border_bottom_color, opacity)
                    } else if widths.left > 0.0 {
                        apply_opacity(s.border_left_color, opacity)
                    } else {
                        apply_opacity(s.border_right_color, opacity)
                    };
                    let border_radii = if s.has_border_radius() {
                        s.border_radius()
                    } else {
                        [0.0; 4]
                    };
                    list.push(DisplayCommand::DrawBorder {
                        rect: border_box,
                        color: border_color,
                        widths,
                        radii: border_radii,
                    });
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
                        let r = s.border_radius();
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

        // 3. Paint Text (for text nodes)
        if let BoxType::TextNode(text) = &box_node.box_type
            && let Some(s) = style
            && !text.is_empty()
            && s.font_size > 0.5
            && s.color.a > 0
        {
            let is_bold = match s.font_weight {
                mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
                mango_css::values::FontWeight::Numeric(w) => w >= 600,
                _ => false,
            };
            let family = mango_render::FontFamily::from_css_name(&s.font_family);
            let style_font = match s.font_style {
                mango_css::values::FontStyle::Italic => mango_render::FontStyle::Italic,
                mango_css::values::FontStyle::Oblique => mango_render::FontStyle::Oblique,
                mango_css::values::FontStyle::Normal => mango_render::FontStyle::Normal,
            };
            let mut decoration = match s.text_decoration {
                mango_css::values::TextDecoration::Underline => {
                    mango_render::TextDecoration::Underline
                }
                mango_css::values::TextDecoration::LineThrough => {
                    mango_render::TextDecoration::LineThrough
                }
                mango_css::values::TextDecoration::Overline => {
                    mango_render::TextDecoration::Overline
                }
                mango_css::values::TextDecoration::None => mango_render::TextDecoration::None,
            };

            // Custom text-underline-offset and text-decoration-thickness handling
            if s.text_decoration == mango_css::values::TextDecoration::Underline
                && (s.text_underline_offset != mango_css::values::Length::Auto
                    || !matches!(
                        s.text_decoration_thickness,
                        mango_css::values::TextDecorationThickness::Auto
                    ))
            {
                let text_w = box_node.dimensions.content.width();
                if text_w > 0.0 {
                    let offset_px = s.text_underline_offset.to_px(s.font_size, 16.0, text_w);
                    let thickness_px = match s.text_decoration_thickness {
                        mango_css::values::TextDecorationThickness::Auto
                        | mango_css::values::TextDecorationThickness::FromFont => {
                            (s.font_size / 14.0).max(1.0)
                        }
                        mango_css::values::TextDecorationThickness::Length(len) => {
                            len.to_px(s.font_size, 16.0, text_w).max(1.0)
                        }
                    };
                    let line_y = content_y + (s.font_size * 0.8) + 2.0 + offset_px;
                    list.push(DisplayCommand::DrawLine {
                        x1: content_x,
                        y1: line_y,
                        x2: content_x + text_w,
                        y2: line_y,
                        color: apply_opacity(s.color, opacity),
                        thickness: thickness_px,
                    });
                    // Suppress default underline in DrawText so it isn't drawn twice
                    decoration = mango_render::TextDecoration::None;
                }
            }

            // Paint text-emphasis marks (e.g. ●, ○, •, etc.) above text glyphs
            if let Some(mark_char) = s.text_emphasis_style.mark_char() {
                let mark_text = mark_char.to_string();
                let emphasis_color = s.text_emphasis_color.unwrap_or(s.color);
                let mark_color = apply_opacity(emphasis_color, opacity);
                let mark_size = (s.font_size * 0.5).max(6.0);
                let mark_y = content_y - mark_size * 0.7;
                let char_count = text.chars().count();
                if char_count > 0 {
                    let total_w = box_node.dimensions.content.width();
                    let char_step = total_w / (char_count as f32);
                    for (i, c) in text.chars().enumerate() {
                        if !c.is_whitespace() {
                            let char_x =
                                content_x + (i as f32 * char_step) + (char_step - mark_size) * 0.5;
                            list.push(DisplayCommand::DrawText {
                                text: mark_text.clone(),
                                x: char_x,
                                y: mark_y,
                                color: mark_color,
                                font_size: mark_size,
                                weight: mango_render::FontWeight::Regular,
                                family,
                                style: mango_render::FontStyle::Normal,
                                decoration: mango_render::TextDecoration::None,
                                letter_spacing: 0.0,
                            });
                        }
                    }
                }
            }

            // Paint any text-shadow runs first so the glyphs sit on top.
            if let Some(shadow) = s.text_shadow
                && shadow.color.a > 0
                && (shadow.offset_x != 0.0 || shadow.offset_y != 0.0 || shadow.blur_radius > 0.0)
            {
                list.push(DisplayCommand::DrawTextShadow {
                    text: text.clone(),
                    x: content_x + shadow.offset_x,
                    y: content_y + shadow.offset_y,
                    color: apply_opacity(shadow.color, opacity),
                    font_size: s.font_size,
                    weight: if is_bold {
                        mango_render::FontWeight::Bold
                    } else {
                        mango_render::FontWeight::Regular
                    },
                    family,
                    style: style_font,
                    blur_radius: shadow.blur_radius,
                    letter_spacing: s.letter_spacing.to_px(
                        s.font_size,
                        16.0,
                        box_node.dimensions.content.width(),
                    ),
                });
            }
            if s.writing_mode == mango_css::values::WritingMode::VerticalRl
                || s.writing_mode == mango_css::values::WritingMode::VerticalLr
            {
                let glyph_advance = s.font_size * 1.1;
                for (idx, ch) in text.chars().enumerate() {
                    list.push(DisplayCommand::DrawText {
                        text: ch.to_string(),
                        x: content_x,
                        y: content_y + idx as f32 * glyph_advance,
                        color: apply_opacity(s.color, opacity),
                        font_size: s.font_size,
                        weight: if is_bold {
                            mango_render::FontWeight::Bold
                        } else {
                            mango_render::FontWeight::Regular
                        },
                        family,
                        style: style_font,
                        decoration,
                        letter_spacing: 0.0,
                    });
                }
            } else {
                list.push(DisplayCommand::DrawText {
                    text: text.clone(),
                    x: content_x,
                    y: content_y,
                    color: apply_opacity(s.color, opacity),
                    font_size: s.font_size,
                    weight: if is_bold {
                        mango_render::FontWeight::Bold
                    } else {
                        mango_render::FontWeight::Regular
                    },
                    family,
                    style: style_font,
                    decoration,
                    letter_spacing: s.letter_spacing.to_px(
                        s.font_size,
                        16.0,
                        box_node.dimensions.content.width(),
                    ),
                });
            }
        }

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
                pixels: final_pixels,
            });

            if needs_clip {
                list.push(DisplayCommand::PopClip);
            }
        }

        // 3c. Paint Form Controls (<input>, <button>, <select>, <textarea>)
        render_form_control(box_node, style, opacity, list, offset_x, offset_y);

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
            node_id,
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
                let active_pixels = if let Some(nid) = node_id {
                    mango_render::get_canvas_pixels(*nid)
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
                        pixels: px,
                    });
                }
            }
        }
    }

    // A <select>, <video>, <audio>, <canvas>, <progress>, or <meter> element displays its content/controls atomically;
    // do not paint fallback/configuration children in page flow
    if matches!(
        box_node.tag_name.as_deref(),
        Some("select")
            | Some("video")
            | Some("audio")
            | Some("canvas")
            | Some("progress")
            | Some("meter")
    ) {
        return;
    }

    // 4. Paint Children ordered by stacking context / z-index (CSS 2.1 Appendix E)
    let current_cb = Some(pad_box);
    let mut sorted_children: Vec<&LayoutBox> = box_node.children.iter().collect();
    sorted_children.sort_by_key(|c| paint_layer_of(c));

    // Root element (<html>) and <body> overflow belongs to the viewport (CSS 2.1 §11.1.1).
    // They must never clip their children in the document display list.
    let is_root_or_body = box_node
        .tag_name
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("html") || t.eq_ignore_ascii_case("body"));

    let clips_x = if let Some(s) = style {
        matches!(
            s.overflow_x,
            mango_css::values::Overflow::Hidden
                | mango_css::values::Overflow::Scroll
                | mango_css::values::Overflow::Auto
        )
    } else {
        false
    };

    let clips_y = if let Some(s) = style {
        matches!(
            s.overflow_y,
            mango_css::values::Overflow::Hidden
                | mango_css::values::Overflow::Scroll
                | mango_css::values::Overflow::Auto
        )
    } else {
        false
    } || matches!(box_node.box_type, BoxType::IFrame { .. });

    let contains_paint = if let Some(s) = style {
        matches!(
            s.contain,
            mango_css::values::Contain::Paint | mango_css::values::Contain::Strict
        )
    } else {
        false
    };
    let clips_children = !is_root_or_body && (clips_x || clips_y || contains_paint);

    let clip_rect = if clips_children && pad_box.width() > 0.0 {
        let r = if clips_y || contains_paint {
            pad_box
        } else {
            // Only clips horizontally; vertical extent is unconstrained
            Rect::new(pad_box.x(), -100_000.0, pad_box.width(), 200_000.0)
        };
        list.push(DisplayCommand::PushClip { rect: r });
        Some(r)
    } else {
        None
    };

    // Skip painting children if content-visibility: hidden
    let is_content_hidden = if let Some(s) = style {
        s.content_visibility == mango_css::values::ContentVisibility::Hidden
    } else {
        false
    };
    if is_content_hidden {
        if clips_children && pad_box.width() > 0.0 {
            list.push(DisplayCommand::PopClip);
        }
        return;
    }

    // Render CSS multi-column rules if defined
    if let Some(s) = style
        && s.column_rule_style != mango_css::values::BorderStyle::None
        && s.column_rule_color.a > 0
    {
        let cols = s.column_count.unwrap_or(0);
        if cols > 1 && pad_box.width() > 0.0 && pad_box.height() > 0.0 {
            let col_gap = s.column_gap.to_px(s.font_size, 16.0, pad_box.width());
            let col_w = ((pad_box.width() - (cols - 1) as f32 * col_gap) / cols as f32).max(1.0);
            let rule_w = s
                .column_rule_width
                .to_px(s.font_size, 16.0, pad_box.width())
                .max(1.0);
            for c in 1..cols {
                let rule_center_x = pad_box.x() + c as f32 * col_w + (c as f32 - 0.5) * col_gap;
                let rule_x = rule_center_x - rule_w * 0.5;
                list.push(DisplayCommand::FillRect {
                    rect: Rect::new(
                        rule_x + offset_x,
                        pad_box.y() + offset_y,
                        rule_w,
                        pad_box.height(),
                    ),
                    color: s.column_rule_color,
                });
            }
        }
    }

    for child in sorted_children {
        let mut child_offset_x = offset_x;
        let mut child_offset_y = offset_y;

        if box_node.is_scroll_container() {
            child_offset_x -= box_node.scroll_offset_x;
            child_offset_y -= box_node.scroll_offset_y;
        }

        if let Some(cs) = &child.style {
            if cs.position == mango_css::values::Position::Fixed {
                if box_node.is_scroll_container() {
                    child_offset_x += box_node.scroll_offset_x;
                    child_offset_y += box_node.scroll_offset_y;
                }
                child_offset_y += scroll_y;
            } else if cs.position == mango_css::values::Position::Sticky {
                let is_container = box_node.is_scroll_container();
                let (viewport_x, viewport_y, viewport_w, viewport_h) = if is_container {
                    (pad_box.x(), pad_box.y(), pad_box.width(), pad_box.height())
                } else {
                    let w = current_cb.map(|r| r.width()).unwrap_or(800.0);
                    let h = current_cb.map(|r| r.height()).unwrap_or(600.0);
                    (0.0, scroll_y, w, h)
                };

                let in_flow_x = child.dimensions.border_box().x() + child_offset_x;
                let in_flow_y = child.dimensions.border_box().y() + child_offset_y;
                let w = child.dimensions.border_box().width();
                let h = child.dimensions.border_box().height();

                let cb_top = current_cb.map(|r| r.y()).unwrap_or(0.0);
                let cb_bottom = current_cb.map(|r| r.bottom()).unwrap_or(f32::INFINITY);
                let cb_left = current_cb.map(|r| r.x()).unwrap_or(0.0);
                let cb_right = current_cb.map(|r| r.right()).unwrap_or(f32::INFINITY);

                // Vertical sticky (top / bottom)
                let max_sticky_y = (cb_bottom - h).max(in_flow_y);
                let min_sticky_y = cb_top.min(in_flow_y);
                let target_y = if cs.top != mango_css::values::Length::Auto {
                    let top_inset = cs.top.to_px(cs.font_size, 16.0, viewport_h);
                    let sticky_thresh = viewport_y + top_inset;
                    if in_flow_y < sticky_thresh {
                        sticky_thresh.clamp(in_flow_y, max_sticky_y)
                    } else {
                        in_flow_y
                    }
                } else if cs.bottom != mango_css::values::Length::Auto {
                    let bottom_inset = cs.bottom.to_px(cs.font_size, 16.0, viewport_h);
                    let sticky_thresh = viewport_y + viewport_h - bottom_inset - h;
                    if in_flow_y > sticky_thresh {
                        sticky_thresh.clamp(min_sticky_y, in_flow_y)
                    } else {
                        in_flow_y
                    }
                } else {
                    in_flow_y
                };
                child_offset_y += target_y - in_flow_y;

                // Horizontal sticky (left / right)
                let max_sticky_x = (cb_right - w).max(in_flow_x);
                let min_sticky_x = cb_left.min(in_flow_x);
                let target_x = if cs.left != mango_css::values::Length::Auto {
                    let left_inset = cs.left.to_px(cs.font_size, 16.0, viewport_w);
                    let sticky_thresh = viewport_x + left_inset;
                    if in_flow_x < sticky_thresh {
                        sticky_thresh.clamp(in_flow_x, max_sticky_x)
                    } else {
                        in_flow_x
                    }
                } else if cs.right != mango_css::values::Length::Auto {
                    let right_inset = cs.right.to_px(cs.font_size, 16.0, viewport_w);
                    let sticky_thresh = viewport_x + viewport_w - right_inset - w;
                    if in_flow_x > sticky_thresh {
                        sticky_thresh.clamp(min_sticky_x, in_flow_x)
                    } else {
                        in_flow_x
                    }
                } else {
                    in_flow_x
                };
                child_offset_x += target_x - in_flow_x;
            }
        }

        let has_transform = style.is_some_and(|s| !s.transform.is_identity());
        let is_fixed_escaped = clip_rect.is_some()
            && !has_transform
            && child
                .style
                .as_ref()
                .is_some_and(|s| s.position == mango_css::values::Position::Fixed);

        if is_fixed_escaped {
            list.push(DisplayCommand::PopClip);
        }

        render_layout_box(
            child,
            list,
            opacity,
            child_offset_x,
            child_offset_y,
            scroll_y,
            current_cb,
        );

        if is_fixed_escaped {
            list.push(DisplayCommand::PushClip {
                rect: clip_rect.unwrap(),
            });
        }
    }

    if clip_rect.is_some() {
        list.push(DisplayCommand::PopClip);
    }

    // Paint container scrollbars if overflow is scroll or auto with overflowing content
    if !is_root_or_body
        && box_node.is_scroll_container()
        && pad_box.width() > 10.0
        && pad_box.height() > 10.0
    {
        let (content_w, content_h) = box_node.scrollable_extent();
        let pad_w = pad_box.width();
        let pad_h = pad_box.height();

        // Vertical scrollbar
        let has_v_scroll = if let Some(s) = style {
            s.overflow_y == mango_css::values::Overflow::Scroll
                || (s.overflow_y == mango_css::values::Overflow::Auto && content_h > pad_h)
        } else {
            false
        };
        if has_v_scroll && pad_h > 20.0 {
            let track_w = 6.0f32;
            let track_x = pad_box.right() - track_w;
            let track_y = pad_box.y();
            let track_h = pad_h;

            list.push(DisplayCommand::FillRect {
                rect: Rect::new(track_x, track_y, track_w, track_h),
                color: Color::rgba(0, 0, 0, 20),
            });

            let thumb_h = ((pad_h / content_h) * pad_h).clamp(16.0, pad_h);
            let max_scroll = (content_h - pad_h).max(1.0);
            let scroll_ratio = (box_node.scroll_offset_y / max_scroll).clamp(0.0, 1.0);
            let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

            list.push(DisplayCommand::FillRoundedRect {
                rect: Rect::new(track_x + 1.0, thumb_y, track_w - 2.0, thumb_h),
                color: Color::rgba(100, 100, 100, 150),
                radii: [2.0; 4],
            });
        }

        // Horizontal scrollbar
        let has_h_scroll = if let Some(s) = style {
            s.overflow_x == mango_css::values::Overflow::Scroll
                || (s.overflow_x == mango_css::values::Overflow::Auto && content_w > pad_w)
        } else {
            false
        };
        if has_h_scroll && pad_w > 20.0 {
            let track_h = 6.0f32;
            let track_y = pad_box.bottom() - track_h;
            let track_x = pad_box.x();
            let track_w = pad_w - if has_v_scroll { 6.0 } else { 0.0 };

            list.push(DisplayCommand::FillRect {
                rect: Rect::new(track_x, track_y, track_w, track_h),
                color: Color::rgba(0, 0, 0, 20),
            });

            let thumb_w = ((pad_w / content_w) * pad_w).clamp(16.0, track_w);
            let max_scroll = (content_w - pad_w).max(1.0);
            let scroll_ratio = (box_node.scroll_offset_x / max_scroll).clamp(0.0, 1.0);
            let thumb_x = track_x + scroll_ratio * (track_w - thumb_w);

            list.push(DisplayCommand::FillRoundedRect {
                rect: Rect::new(thumb_x, track_y + 1.0, thumb_w, track_h - 2.0),
                color: Color::rgba(100, 100, 100, 150),
                radii: [2.0; 4],
            });
        }
    }
}

/// Renders internal contents of form controls (<input>, <select>, <textarea>).
fn render_form_control(
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
    let _border_box = Rect::new(
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
                        let is_bold = match s.font_weight {
                            mango_css::values::FontWeight::Bold
                            | mango_css::values::FontWeight::Bolder => true,
                            mango_css::values::FontWeight::Numeric(w) => w >= 600,
                            _ => false,
                        };
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
                            .and_then(|url| {
                                mango_render::image_decode::get_cached_image(url).or_else(|| {
                                    mango_render::image_decode::get_cached_image(&format!(
                                        "https:{url}"
                                    ))
                                })
                            })
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
                            let icon_size = ((content.width().min(content.height()) * 0.65) as u32)
                                .clamp(14, 24);
                            let x =
                                content.x() + ((content.width() - icon_size as f32) / 2.0).max(0.0);
                            let y = content.y()
                                + ((content.height() - icon_size as f32) / 2.0).max(0.0);
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
                                pixels,
                            });
                        }
                    }
                }
                "range" => {
                    // Slider track + filled portion + draggable thumb (GAP-024).
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
                    // Color swatch + hex label; clicking opens the palette picker (GAP-024).
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
                    // "Choose file..." button + selected filename (GAP-024).
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
                    let y = content.y() + ((content.height() - font_size) / 2.0).max(0.0);
                    let x = content.x() + 4.0;
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
                            let caret_h = (font_size * 1.15).min(content.height() - 4.0).max(12.0);
                            let caret_y =
                                content.y() + ((content.height() - caret_h) / 2.0).max(0.0);
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
                            list.push(DisplayCommand::draw_text(
                                ph,
                                x,
                                y,
                                apply_opacity(ph_color, opacity),
                                font_size,
                                mango_render::FontWeight::Regular,
                                family,
                            ));
                        }
                        let caret_h = (font_size * 1.15).min(content.height() - 4.0).max(12.0);
                        let caret_y = content.y() + ((content.height() - caret_h) / 2.0).max(0.0);
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
                        list.push(DisplayCommand::draw_text(
                            ph,
                            x,
                            y,
                            apply_opacity(ph_color, opacity),
                            font_size,
                            mango_render::FontWeight::Regular,
                            family,
                        ));
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
                    list.push(DisplayCommand::draw_text(
                        ph,
                        x,
                        y,
                        apply_opacity(ph_color, opacity),
                        font_size,
                        mango_render::FontWeight::Regular,
                        family,
                    ));
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

fn render_progress_element(
    box_node: &LayoutBox,
    _style: &mango_css::computed::ComputedStyle,
    opacity: f32,
    list: &mut DisplayList,
    content: Rect,
) {
    if content.width() < 4.0 || content.height() < 4.0 {
        return;
    }
    let track_color = apply_opacity(Color::rgb(230, 230, 230), opacity);
    let border_color = apply_opacity(Color::rgb(190, 195, 200), opacity);
    let radii = [3.0; 4];

    // Track
    list.push(DisplayCommand::FillRoundedRect {
        rect: content,
        color: track_color,
        radii,
    });
    list.push(DisplayCommand::DrawBorder {
        rect: content,
        color: border_color,
        widths: mango_render::display_list::BorderWidths {
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
            left: 1.0,
        },
        radii,
    });

    let max_val = box_node
        .get_attribute("max")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|&m| m > 0.0)
        .unwrap_or(1.0);

    let accent = _style.accent_color.unwrap_or(Color::rgb(0, 120, 215));
    let fill_color = apply_opacity(accent, opacity); // Progress bar fill

    if let Some(val_str) = box_node.get_attribute("value") {
        // Determinate progress
        if let Ok(val) = val_str.parse::<f32>() {
            let ratio = (val / max_val).clamp(0.0, 1.0);
            let fill_w = (content.width() * ratio).max(0.0);
            if fill_w > 0.0 {
                let fill_rect = Rect::new(content.x(), content.y(), fill_w, content.height());
                list.push(DisplayCommand::FillRoundedRect {
                    rect: fill_rect,
                    color: fill_color,
                    radii,
                });
            }
        }
    } else {
        // Indeterminate progress: active indicator centered
        let indet_w = (content.width() * 0.35).clamp(16.0, content.width());
        let indet_x = content.x() + (content.width() - indet_w) * 0.5;
        let indet_rect = Rect::new(indet_x, content.y(), indet_w, content.height());
        list.push(DisplayCommand::FillRoundedRect {
            rect: indet_rect,
            color: fill_color,
            radii,
        });
    }
}

fn render_meter_element(
    box_node: &LayoutBox,
    _style: &mango_css::computed::ComputedStyle,
    opacity: f32,
    list: &mut DisplayList,
    content: Rect,
) {
    if content.width() < 4.0 || content.height() < 4.0 {
        return;
    }
    let track_color = apply_opacity(Color::rgb(235, 238, 242), opacity);
    let border_color = apply_opacity(Color::rgb(180, 185, 190), opacity);
    let radii = [3.0; 4];

    // Gauge track
    list.push(DisplayCommand::FillRoundedRect {
        rect: content,
        color: track_color,
        radii,
    });
    list.push(DisplayCommand::DrawBorder {
        rect: content,
        color: border_color,
        widths: mango_render::display_list::BorderWidths {
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
            left: 1.0,
        },
        radii,
    });

    let min_val = box_node
        .get_attribute("min")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let max_val = box_node
        .get_attribute("max")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0)
        .max(min_val);
    let val = box_node
        .get_attribute("value")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(min_val)
        .clamp(min_val, max_val);

    let low_val = box_node
        .get_attribute("low")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(min_val)
        .clamp(min_val, max_val);
    let high_val = box_node
        .get_attribute("high")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(max_val)
        .clamp(low_val, max_val);
    let opt_val = box_node
        .get_attribute("optimum")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(min_val + (max_val - min_val) / 2.0)
        .clamp(min_val, max_val);

    // HTML5 §4.10.14: Color zones (green, yellow, red)
    let fill_color = if opt_val >= high_val {
        // High is optimal
        if val >= high_val {
            Color::rgb(40, 167, 69) // Green (optimal)
        } else if val >= low_val {
            Color::rgb(255, 193, 7) // Yellow/Orange (suboptimal)
        } else {
            Color::rgb(220, 53, 69) // Red (poor)
        }
    } else if opt_val <= low_val {
        // Low is optimal
        if val <= low_val {
            Color::rgb(40, 167, 69) // Green (optimal)
        } else if val <= high_val {
            Color::rgb(255, 193, 7) // Yellow/Orange (suboptimal)
        } else {
            Color::rgb(220, 53, 69) // Red (poor)
        }
    } else {
        // Middle is optimal
        if val >= low_val && val <= high_val {
            Color::rgb(40, 167, 69) // Green (optimal)
        } else {
            Color::rgb(255, 193, 7) // Yellow/Orange (suboptimal)
        }
    };

    let span = (max_val - min_val).max(0.0001);
    let ratio = ((val - min_val) / span).clamp(0.0, 1.0);
    let fill_w = (content.width() * ratio).max(0.0);
    if fill_w > 0.0 {
        let fill_rect = Rect::new(content.x(), content.y(), fill_w, content.height());
        list.push(DisplayCommand::FillRoundedRect {
            rect: fill_rect,
            color: apply_opacity(fill_color, opacity),
            radii,
        });
    }
}

/// Swatch colors shown by the `<input type="color">` picker palette.
/// Kept here (rather than in the chrome) so the swatch painting and the
/// picker UI always agree on the same 28 entries, in the same order.
pub const COLOR_SWATCHES: [&str; 28] = [
    "#000000", "#333333", "#666666", "#999999", "#cccccc", "#ffffff", "#ff0000", "#ff4040",
    "#ff8000", "#ffb300", "#ffff00", "#d4ff00", "#00e000", "#00ff80", "#00ffff", "#0080ff",
    "#0040ff", "#4000ff", "#8000ff", "#ff00ff", "#ff0080", "#8b4513", "#daa520", "#008080",
    "#2f4f4f", "#800000", "#000080", "#4b0082",
];

/// Draws the up/down spinner arrows on the right edge of an
/// `<input type="number">`. The clickable hit zone in the chrome mirrors
/// this geometry: the right-most 16px, split into top/bottom halves.
fn render_number_spinners(content: Rect, opacity: f32, list: &mut DisplayList) {
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

/// Paint layer priority according to CSS 2.1 Appendix E.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PaintLayer {
    /// Positioned children with negative z-index
    NegativeZ(i32),
    /// Normal flow children (blocks, inlines, floats with position: static)
    NormalFlow,
    /// Positioned children with z-index: auto or z-index: 0
    ZeroOrAutoZ,
    /// Positioned children with positive z-index
    PositiveZ(i32),
}

fn paint_layer_of(box_node: &LayoutBox) -> PaintLayer {
    if let Some(s) = &box_node.style {
        let creates_stacking_context = s.position != mango_css::values::Position::Static
            || s.opacity < 1.0
            || !s.transform.is_identity()
            || !s.filter.is_empty()
            || s.isolation == mango_css::values::Isolation::Isolate
            || s.mix_blend_mode != mango_css::values::BlendMode::Normal
            || s.clip_path != mango_css::values::ClipPath::None
            || s.mask_image.is_some();

        if creates_stacking_context {
            match s.z_index {
                Some(z) if z < 0 => PaintLayer::NegativeZ(z),
                Some(z) if z > 0 => PaintLayer::PositiveZ(z),
                _ => PaintLayer::ZeroOrAutoZ,
            }
        } else {
            PaintLayer::NormalFlow
        }
    } else {
        PaintLayer::NormalFlow
    }
}

/// Renders an embedded preview card for an `<iframe>` when child DOM is not rendered locally.
fn render_iframe_embed_card(
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

fn format_media_time(seconds: f32) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total_secs = seconds as u32;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{}:{:02}", mins, secs)
}

/// Renders an HTML5 `<video>` element with dark cinematic canvas, poster image, and controls bar.
#[allow(clippy::too_many_arguments)]
fn render_video_element(
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

    // 1. Cinematic dark background (#09090b)
    list.push(DisplayCommand::FillRect {
        rect,
        color: apply_opacity(Color::rgb(9, 9, 11), opacity),
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
            pixels: pixels.to_vec(),
        });
        painted_poster = true;
    }

    // 3. Center play overlay badge (if not playing)
    if !is_playing {
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
fn render_audio_element(
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

fn lookup_image_cached(url: &str) -> Option<mango_render::image_decode::DecodedImage> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(img) = mango_render::image_decode::decode_data_uri(trimmed) {
        return Some(img);
    }
    if let Some(img) = mango_render::image_decode::get_cached_image(trimmed) {
        return Some(img);
    }
    if trimmed.starts_with("//") {
        let https_url = format!("https:{trimmed}");
        if let Some(img) = mango_render::image_decode::get_cached_image(&https_url) {
            return Some(img);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn paint_background_image_layer(
    bg_url: &str,
    target_rect: Rect,
    bg_size: mango_css::values::BackgroundSize,
    bg_position: (mango_css::values::Length, mango_css::values::Length),
    bg_repeat: mango_css::values::BackgroundRepeat,
    bg_attachment: mango_css::values::BackgroundAttachment,
    blend_mode: mango_css::values::BlendMode,
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
            0.0
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
                    pixels: resized.pixels,
                });
            }
            mango_css::values::BackgroundRepeat::RepeatX => {
                let mut cur_x = origin_x;
                let max_x = target_rect.right();
                let mut count = 0;
                while cur_x < max_x && count < 64 {
                    list.push(DisplayCommand::DrawImage {
                        x: cur_x,
                        y: origin_y,
                        width: target_w,
                        height: target_h,
                        pixels: resized.pixels.clone(),
                    });
                    cur_x += target_w;
                    count += 1;
                }
            }
            mango_css::values::BackgroundRepeat::RepeatY => {
                let mut cur_y = origin_y;
                let max_y = target_rect.bottom();
                let mut count = 0;
                while cur_y < max_y && count < 64 {
                    list.push(DisplayCommand::DrawImage {
                        x: origin_x,
                        y: cur_y,
                        width: target_w,
                        height: target_h,
                        pixels: resized.pixels.clone(),
                    });
                    cur_y += target_h;
                    count += 1;
                }
            }
            mango_css::values::BackgroundRepeat::Repeat => {
                let mut cur_y = origin_y;
                let max_y = target_rect.bottom();
                let max_x = target_rect.right();
                let mut total_tiles = 0;
                while cur_y < max_y && total_tiles < 128 {
                    let mut cur_x = origin_x;
                    while cur_x < max_x && total_tiles < 128 {
                        list.push(DisplayCommand::DrawImage {
                            x: cur_x,
                            y: cur_y,
                            width: target_w,
                            height: target_h,
                            pixels: resized.pixels.clone(),
                        });
                        cur_x += target_w;
                        total_tiles += 1;
                    }
                    cur_y += target_h;
                }
            }
        }
    }
    if blend {
        list.push(DisplayCommand::PopBlendMode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Rect;
    use mango_css::computed::ComputedStyle;
    use mango_css::values::Position;

    #[test]
    fn test_transform_emits_push_and_pop_around_subtree() {
        let mut parent = LayoutBox::new(BoxType::BlockNode, None);
        parent.dimensions.content = Rect::new(0.0, 0.0, 200.0, 100.0);

        let mut rotated = ComputedStyle::default();
        rotated.background_color = Color::RED;
        rotated.transform =
            mango_css::values::Transform(vec![mango_css::values::TransformFunction::Rotate(45.0)]);
        rotated.transform_origin_x = mango_css::values::Length::Percent(50.0);
        rotated.transform_origin_y = mango_css::values::Length::Percent(50.0);

        let mut child = LayoutBox::new(BoxType::BlockNode, Some(rotated));
        child.dimensions.content = Rect::new(10.0, 10.0, 50.0, 50.0);
        parent.children.push(child);

        let dl = build_display_list(&parent);
        let push_idx = dl
            .iter()
            .position(|c| matches!(c, DisplayCommand::PushTransform { .. }))
            .expect("PushTransform emitted for transform: rotate()");
        let pop_idx = dl
            .iter()
            .position(|c| matches!(c, DisplayCommand::PopTransform))
            .expect("PopTransform emitted");
        assert!(
            push_idx < pop_idx,
            "transform must wrap the painted subtree"
        );

        // The push matrix must include the rotation (b != 0 for a rotated matrix).
        if let Some(DisplayCommand::PushTransform { matrix }) = dl.iter().nth(push_idx) {
            assert!(
                matrix[1].abs() > 0.1,
                "rotation must produce a non-zero b component"
            );
        }

        // Identity transforms must not emit any transform commands.
        let plain = LayoutBox::new(BoxType::BlockNode, Some(ComputedStyle::default()));
        let dl_plain = build_display_list(&plain);
        assert!(
            !dl_plain
                .iter()
                .any(|c| matches!(c, DisplayCommand::PushTransform { .. }))
        );
    }

    #[test]
    fn test_gradient_background_emits_fill_gradient() {
        let mut style = ComputedStyle::default();
        style.background_gradient = Some(mango_css::values::Gradient::Linear {
            angle_deg: 90.0,
            stops: vec![
                mango_css::values::ColorStop::new(Color::RED, Some(0.0), None),
                mango_css::values::ColorStop::new(Color::BLUE, Some(1.0), None),
            ],
            repeating: false,
        });

        let mut root = LayoutBox::new(BoxType::BlockNode, Some(style));
        root.dimensions.content = Rect::new(0.0, 0.0, 120.0, 60.0);

        let dl = build_display_list(&root);
        let gradient_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::FillGradient { .. }));
        match gradient_cmd {
            Some(DisplayCommand::FillGradient { rect, gradient, .. }) => {
                assert_eq!(rect.width(), 120.0);
                assert_eq!(gradient.stops().len(), 2);
            }
            _ => panic!("expected a FillGradient command for a gradient background"),
        }
    }

    #[test]
    fn test_layout_gradients() {
        let mut style = ComputedStyle::default();
        style.color = Color::rgb(0, 128, 255);
        style.background_gradient = Some(mango_css::values::Gradient::Linear {
            angle_deg: 180.0,
            stops: vec![
                mango_css::values::ColorStop {
                    color: Color::BLACK,
                    position: Some(0.0),
                    end_position: None,
                    is_current_color: true,
                },
                mango_css::values::ColorStop::new(Color::TRANSPARENT, Some(1.0), None),
            ],
            repeating: false,
        });

        let mut root = LayoutBox::new(BoxType::BlockNode, Some(style));
        root.dimensions.content = Rect::new(10.0, 20.0, 200.0, 100.0);

        let dl = build_display_list(&root);
        let grad_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::FillGradient { .. }));
        assert!(grad_cmd.is_some());
        if let Some(DisplayCommand::FillGradient { gradient, rect, .. }) = grad_cmd {
            assert_eq!(rect.width(), 200.0);
            assert_eq!(rect.height(), 100.0);
            // CurrentColor must be resolved to style.color
            assert_eq!(gradient.stops()[0].color, Color::rgb(0, 128, 255));
        }
    }

    #[test]
    fn test_text_shadow_emits_shadow_command_before_text() {
        let mut style = ComputedStyle::default();
        style.color = Color::BLACK;
        style.font_size = 16.0;
        style.text_shadow = Some(mango_css::values::TextShadow {
            offset_x: 1.0,
            offset_y: 2.0,
            blur_radius: 3.0,
            color: Color::rgba(0, 0, 0, 128),
        });

        let mut text_box = LayoutBox::new(BoxType::TextNode("Shadowed".to_string()), Some(style));
        text_box.dimensions.content = Rect::new(5.0, 5.0, 80.0, 20.0);

        let dl = build_display_list(&text_box);
        let shadow_idx = dl
            .iter()
            .position(|c| matches!(c, DisplayCommand::DrawTextShadow { .. }))
            .expect("DrawTextShadow emitted for text-shadow");
        let text_idx = dl
            .iter()
            .position(|c| matches!(c, DisplayCommand::DrawText { .. }))
            .expect("DrawText emitted");
        assert!(shadow_idx < text_idx, "shadow must paint behind the glyphs");

        if let Some(DisplayCommand::DrawTextShadow {
            x, y, blur_radius, ..
        }) = dl.iter().nth(shadow_idx)
        {
            assert_eq!(*x, 6.0);
            assert_eq!(*y, 7.0);
            assert_eq!(*blur_radius, 3.0);
        }
    }

    #[test]
    fn test_build_display_list_commands() {
        let mut root = LayoutBox::new(BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        style.background_color = Color::RED;
        style.color = Color::BLACK;
        style.font_size = 16.0;
        style.border_top_width = 2.0;

        root.style = Some(style);
        root.dimensions.content = Rect::new(10.0, 10.0, 100.0, 50.0);
        root.dimensions.border.top = 2.0;

        let text_child = LayoutBox::new(
            BoxType::TextNode("Test Text".to_string()),
            root.style.clone(),
        );
        root.children.push(text_child);

        let dl = build_display_list(&root);
        // Expect: FillRect (background), DrawBorder, DrawText
        assert_eq!(dl.len(), 3);
    }

    #[test]
    fn test_z_index_stacking_order() {
        let mut parent = LayoutBox::new(BoxType::BlockNode, None);
        parent.dimensions.content = Rect::new(0.0, 0.0, 500.0, 500.0);

        // Child 1: positive z-index (+10)
        let mut style_pos = ComputedStyle::default();
        style_pos.position = Position::Relative;
        style_pos.z_index = Some(10);
        style_pos.background_color = Color::BLUE;
        let mut child_pos = LayoutBox::new(BoxType::BlockNode, Some(style_pos));
        child_pos.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

        // Child 2: negative z-index (-5)
        let mut style_neg = ComputedStyle::default();
        style_neg.position = Position::Relative;
        style_neg.z_index = Some(-5);
        style_neg.background_color = Color::RED;
        let mut child_neg = LayoutBox::new(BoxType::BlockNode, Some(style_neg));
        child_neg.dimensions.content = Rect::new(10.0, 10.0, 100.0, 100.0);

        // Child 3: normal flow (static)
        let mut style_norm = ComputedStyle::default();
        style_norm.background_color = Color::GREEN;
        let mut child_norm = LayoutBox::new(BoxType::BlockNode, Some(style_norm));
        child_norm.dimensions.content = Rect::new(20.0, 20.0, 100.0, 100.0);

        // Appended in order: pos (+10), neg (-5), norm (0)
        parent.children.push(child_pos);
        parent.children.push(child_neg);
        parent.children.push(child_norm);

        let dl = build_display_list(&parent);
        // Painted order must be: neg (-5) -> norm (static) -> pos (+10)
        assert_eq!(dl.len(), 3);
        match (&dl[0], &dl[1], &dl[2]) {
            (
                DisplayCommand::FillRect { color: c1, .. },
                DisplayCommand::FillRect { color: c2, .. },
                DisplayCommand::FillRect { color: c3, .. },
            ) => {
                assert_eq!(c1, &Color::RED, "Negative z-index (-5) must paint first");
                assert_eq!(c2, &Color::GREEN, "Normal flow must paint second");
                assert_eq!(c3, &Color::BLUE, "Positive z-index (+10) must paint last");
            }
            _ => panic!("Expected FillRect commands"),
        }
    }

    #[test]
    fn test_border_radius_generates_fill_rounded_rect() {
        let mut root = LayoutBox::new(BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        style.background_color = Color::BLUE;
        style.border_top_left_radius = 8.0;
        style.border_top_right_radius = 8.0;
        style.border_bottom_right_radius = 8.0;
        style.border_bottom_left_radius = 8.0;

        root.style = Some(style);
        root.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

        let dl = build_display_list(&root);
        assert_eq!(dl.len(), 1);
        match &dl[0] {
            DisplayCommand::FillRoundedRect { radii, color, .. } => {
                assert_eq!(*radii, [8.0, 8.0, 8.0, 8.0]);
                assert_eq!(*color, Color::BLUE);
            }
            _ => panic!("Expected FillRoundedRect command"),
        }
    }

    #[test]
    fn test_visibility_hidden_skips_rendering() {
        let mut root = LayoutBox::new(BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        style.background_color = Color::RED;
        style.visibility = mango_css::values::Visibility::Hidden;

        root.style = Some(style);
        root.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

        let dl = build_display_list(&root);
        assert!(dl.is_empty(), "Hidden element should generate no commands");
    }

    #[test]
    fn test_opacity_scales_colors() {
        let mut root = LayoutBox::new(BoxType::BlockNode, None);
        let mut style = ComputedStyle::default();
        style.background_color = Color::rgba(255, 0, 0, 200);
        style.opacity = 0.5;

        root.style = Some(style);
        root.dimensions.content = Rect::new(0.0, 0.0, 100.0, 50.0);

        let dl = build_display_list(&root);
        assert_eq!(dl.len(), 1);
        match &dl[0] {
            DisplayCommand::FillRect { color, .. } => {
                // 200 * 0.5 = 100
                assert_eq!(color.a, 100);
                assert_eq!(color.r, 255);
            }
            _ => panic!("Expected FillRect command"),
        }
    }

    #[test]
    fn test_overflow_hidden_generates_push_and_pop_clip() {
        let mut parent = LayoutBox::new(BoxType::BlockNode, None);
        let mut p_style = ComputedStyle::default();
        p_style.overflow_x = mango_css::values::Overflow::Hidden;
        p_style.overflow_y = mango_css::values::Overflow::Hidden;
        parent.style = Some(p_style);
        parent.dimensions.content = Rect::new(10.0, 10.0, 100.0, 100.0);

        let mut child = LayoutBox::new(BoxType::BlockNode, None);
        let mut c_style = ComputedStyle::default();
        c_style.background_color = Color::RED;
        child.style = Some(c_style);
        child.dimensions.content = Rect::new(10.0, 10.0, 200.0, 200.0);
        parent.children.push(child);

        let dl = build_display_list(&parent);

        let push_clip_idx = dl
            .iter()
            .position(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
        let pop_clip_idx = dl
            .iter()
            .position(|cmd| matches!(cmd, DisplayCommand::PopClip));
        let fill_idx = dl.iter().position(
            |cmd| matches!(cmd, DisplayCommand::FillRect { color, .. } if *color == Color::RED),
        );

        assert!(
            push_clip_idx.is_some(),
            "PushClip must be emitted for overflow: hidden container"
        );
        assert!(
            pop_clip_idx.is_some(),
            "PopClip must be emitted after child rendering"
        );
        assert!(fill_idx.is_some(), "Child fill must be emitted");

        let push_idx = push_clip_idx.unwrap();
        let fill = fill_idx.unwrap();
        let pop_idx = pop_clip_idx.unwrap();

        assert!(
            push_idx < fill && fill < pop_idx,
            "Child fill must be surrounded by PushClip and PopClip"
        );
    }

    #[test]
    fn test_form_controls_checkbox_checked() {
        let mut input_box = LayoutBox::new(BoxType::InlineBlock, None);
        input_box.tag_name = Some("input".to_string());
        input_box
            .attributes
            .push(("type".to_string(), "checkbox".to_string()));
        input_box
            .attributes
            .push(("checked".to_string(), "true".to_string()));
        input_box.style = Some(ComputedStyle::default());
        input_box.dimensions.content = Rect::new(10.0, 10.0, 14.0, 14.0);

        let dl = build_display_list(&input_box);
        // Expect FillRoundedRect (inner checkmark) and DrawText ("✓")
        assert_eq!(dl.len(), 2);
        match &dl[1] {
            DisplayCommand::DrawText { text, .. } => {
                assert_eq!(text, "✓");
            }
            _ => panic!("Expected DrawText checkmark"),
        }
    }

    #[test]
    fn test_select_options() {
        let html_path = if std::path::Path::new("scratch/ddg_results.html").exists() {
            "scratch/ddg_results.html"
        } else {
            "../../scratch/ddg_results.html"
        };
        let css_path = if std::path::Path::new("scratch_ddg.css").exists() {
            "scratch_ddg.css"
        } else {
            "../../scratch_ddg.css"
        };
        if let Ok(html) = std::fs::read_to_string(html_path) {
            println!("Loaded html: {} bytes", html.len());
            let doc = mango_html::parse_html(&html);
            let css_text = std::fs::read_to_string(css_path).unwrap_or_default();
            println!("Loaded css: {} bytes", css_text.len());
            let sheet = mango_css::parser::parse_stylesheet(&css_text);
            let (_box_tree, dl) =
                crate::layout_document(&doc, &[&sheet], mango_core::Size::new(1280.0, 900.0));
            let texts: Vec<&str> = dl
                .iter()
                .filter_map(|cmd| {
                    if let DisplayCommand::DrawText { text, .. } = cmd {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect();

            assert!(
                texts.contains(&"All Regions"),
                "Select should render first option text 'All Regions'"
            );
            assert!(
                texts.contains(&"Any Time"),
                "Select should render first option text 'Any Time'"
            );
            assert!(
                !texts.contains(&"Select..."),
                "Select should not render fallback 'Select...'"
            );

            // Verify search button has DrawImage (vector icon) instead of missing emoji "🔍"
            let has_search_vector_icon = dl
                .iter()
                .any(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }));
            assert!(
                has_search_vector_icon,
                "Search button should render crisp vector icon"
            );
            assert!(
                !texts.contains(&"🔍"),
                "Search button must not output missing emoji glyph '🔍'"
            );
        }
    }

    #[test]
    fn test_input_caret_fill_rect() {
        let mut input_box = LayoutBox::new(BoxType::BlockNode, Some(ComputedStyle::default()));
        input_box.tag_name = Some("input".to_string());
        input_box
            .attributes
            .push(("type".to_string(), "text".to_string()));
        input_box
            .attributes
            .push(("value".to_string(), "hello".to_string()));
        input_box
            .attributes
            .push(("data-mango-focused".to_string(), "true".to_string()));
        input_box
            .attributes
            .push(("_mango_cursor_pos".to_string(), "3".to_string()));
        input_box.dimensions.content = Rect::new(10.0, 10.0, 150.0, 30.0);

        let dl = build_display_list(&input_box);

        // Verify text is drawn clean without '|' injected into it
        let has_clean_text = dl.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text == "hello",
            _ => false,
        });
        assert!(
            has_clean_text,
            "Input display_text must be clean 'hello' without '|'"
        );

        // Verify caret is drawn as a FillRect command
        let has_caret_rect = dl.iter().any(|cmd| match cmd {
            DisplayCommand::FillRect { rect, .. } => rect.width() == 1.5 && rect.x() > 10.0,
            _ => false,
        });
        assert!(
            has_caret_rect,
            "Input caret must be rendered as a FillRect line"
        );
    }

    #[test]
    fn test_position_fixed_offsets_with_scroll() {
        let mut parent = LayoutBox::new(BoxType::BlockNode, None);
        parent.dimensions.content = Rect::new(0.0, 0.0, 500.0, 1000.0);

        let mut fixed_style = ComputedStyle::default();
        fixed_style.position = Position::Fixed;
        fixed_style.background_color = Color::BLUE;
        let mut fixed_child = LayoutBox::new(BoxType::BlockNode, Some(fixed_style));
        fixed_child.dimensions.content = Rect::new(0.0, 10.0, 200.0, 40.0);
        parent.children.push(fixed_child);

        // At scroll_y = 0, y is 10.0
        let dl0 = build_display_list_with_scroll(&parent, 0.0);
        assert_eq!(dl0.len(), 1);
        if let DisplayCommand::FillRect { rect, .. } = &dl0[0] {
            assert_eq!(rect.y(), 10.0);
        } else {
            panic!("Expected FillRect");
        }

        // At scroll_y = 150, fixed element y is offset by +150 to 160.0
        let dl150 = build_display_list_with_scroll(&parent, 150.0);
        assert_eq!(dl150.len(), 1);
        if let DisplayCommand::FillRect { rect, .. } = &dl150[0] {
            assert_eq!(rect.y(), 160.0);
        } else {
            panic!("Expected FillRect");
        }
    }

    #[test]
    fn test_position_sticky_clamping_with_scroll() {
        let mut parent = LayoutBox::new(BoxType::BlockNode, None);
        parent.dimensions.content = Rect::new(0.0, 0.0, 500.0, 1000.0);

        let mut sticky_style = ComputedStyle::default();
        sticky_style.position = Position::Sticky;
        sticky_style.top = mango_css::values::Length::Px(10.0);
        sticky_style.background_color = Color::GREEN;
        let mut sticky_child = LayoutBox::new(BoxType::BlockNode, Some(sticky_style));
        sticky_child.dimensions.content = Rect::new(0.0, 200.0, 200.0, 50.0);
        parent.children.push(sticky_child);

        // At scroll_y = 50: scroll_y + top (60) < 200 => stays at in-flow y = 200.0
        let dl50 = build_display_list_with_scroll(&parent, 50.0);
        if let DisplayCommand::FillRect { rect, .. } = &dl50[0] {
            assert_eq!(rect.y(), 200.0);
        } else {
            panic!("Expected FillRect");
        }

        // At scroll_y = 300: scroll_y + top (310) > 200 => sticks to y = 310.0
        let dl300 = build_display_list_with_scroll(&parent, 300.0);
        if let DisplayCommand::FillRect { rect, .. } = &dl300[0] {
            assert_eq!(rect.y(), 310.0);
        } else {
            panic!("Expected FillRect");
        }

        // At scroll_y = 1200: exceeds parent bottom (1000) => clamped to parent bottom - height = 950.0
        let dl1200 = build_display_list_with_scroll(&parent, 1200.0);
        if let DisplayCommand::FillRect { rect, .. } = &dl1200[0] {
            assert_eq!(rect.y(), 950.0);
        } else {
            panic!("Expected FillRect");
        }
    }

    #[test]
    fn test_background_image_display_command() {
        let fake_url = "https://example.com/bg.png";
        let fake_img = mango_render::image_decode::DecodedImage {
            width: 10,
            height: 10,
            pixels: vec![0xFF00FF00; 100],
        };
        mango_render::cache_image(fake_url, fake_img);

        let mut style = ComputedStyle::default();
        style.background_image = Some(fake_url.to_string());
        style.background_repeat = mango_css::values::BackgroundRepeat::NoRepeat;
        style.background_size = mango_css::values::BackgroundSize::Explicit(
            mango_css::values::Length::Px(40.0),
            mango_css::values::Length::Px(40.0),
        );

        let mut box_node = LayoutBox::new(BoxType::BlockNode, Some(style));
        box_node.dimensions.content = Rect::new(10.0, 20.0, 100.0, 100.0);

        let dl = build_display_list(&box_node);
        let draw_img_cmds: Vec<&DisplayCommand> = dl
            .iter()
            .filter(|cmd| matches!(cmd, DisplayCommand::DrawImage { .. }))
            .collect();
        assert_eq!(draw_img_cmds.len(), 1);
        if let DisplayCommand::DrawImage {
            x,
            y,
            width,
            height,
            pixels,
        } = draw_img_cmds[0]
        {
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 20.0);
            assert_eq!(*width, 40.0);
            assert_eq!(*height, 40.0);
            assert_eq!(pixels.len(), 40 * 40);
        } else {
            panic!("Expected DrawImage");
        }
    }

    #[test]
    fn test_box_shadow_display_command() {
        let mut style = ComputedStyle::default();
        style.box_shadow = Some(mango_css::values::BoxShadow {
            offset_x: 4.0,
            offset_y: 8.0,
            blur_radius: 12.0,
            spread_radius: 2.0,
            color: Color::rgba(0, 0, 0, 100),
            inset: false,
        });

        let mut box_node = LayoutBox::new(BoxType::BlockNode, Some(style));
        box_node.dimensions.content = Rect::new(50.0, 50.0, 200.0, 100.0);

        let dl = build_display_list(&box_node);
        let shadow_cmds: Vec<&DisplayCommand> = dl
            .iter()
            .filter(|cmd| matches!(cmd, DisplayCommand::DrawBoxShadow { .. }))
            .collect();
        assert_eq!(shadow_cmds.len(), 1);
        if let DisplayCommand::DrawBoxShadow {
            rect,
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            ..
        } = shadow_cmds[0]
        {
            assert_eq!(rect.x(), 50.0);
            assert_eq!(rect.y(), 50.0);
            assert_eq!(*offset_x, 4.0);
            assert_eq!(*offset_y, 8.0);
            assert_eq!(*blur_radius, 12.0);
            assert_eq!(*spread_radius, 2.0);
        } else {
            panic!("Expected DrawBoxShadow");
        }
    }

    #[test]
    fn test_outline_display_command() {
        let mut style = ComputedStyle::default();
        style.outline_width = 2.0;
        style.outline_style = mango_css::values::BorderStyle::Solid;
        style.outline_color = Color::RED;
        style.outline_offset = 3.0;

        let mut box_node = LayoutBox::new(BoxType::BlockNode, Some(style));
        box_node.dimensions.content = Rect::new(50.0, 50.0, 200.0, 100.0);

        let dl = build_display_list(&box_node);
        let border_cmds: Vec<&DisplayCommand> = dl
            .iter()
            .filter(|cmd| matches!(cmd, DisplayCommand::DrawBorder { .. }))
            .collect();
        assert_eq!(
            border_cmds.len(),
            1,
            "Should emit 1 DrawBorder command for the outline"
        );
        if let DisplayCommand::DrawBorder {
            rect,
            color,
            widths,
            ..
        } = border_cmds[0]
        {
            assert_eq!(*color, Color::RED);
            assert_eq!(rect.x(), 45.0);
            assert_eq!(rect.y(), 45.0);
            assert_eq!(rect.width(), 210.0);
            assert_eq!(rect.height(), 110.0);
            assert_eq!(widths.top, 2.0);
            assert_eq!(widths.right, 2.0);
            assert_eq!(widths.bottom, 2.0);
            assert_eq!(widths.left, 2.0);
        } else {
            panic!("Expected DrawBorder");
        }
    }

    #[test]
    fn test_iframe_display_commands() {
        let mut iframe_box = LayoutBox::new(
            BoxType::IFrame {
                src: "https://example.com/embed".to_string(),
                srcdoc: None,
                sandbox: crate::box_model::IFrameSandbox::from_attribute(Some("allow-scripts")),
                intrinsic_width: 300.0,
                intrinsic_height: 150.0,
            },
            None,
        );
        iframe_box.dimensions.content = Rect::new(20.0, 30.0, 300.0, 150.0);

        let dl = build_display_list(&iframe_box);

        // Verify white viewport fill is rendered
        let has_white_fill = dl.iter().any(|cmd| match cmd {
            DisplayCommand::FillRect { color, rect } => {
                *color == Color::WHITE && rect.width() == 300.0 && rect.height() == 150.0
            }
            _ => false,
        });
        assert!(
            has_white_fill,
            "IFrame should paint white viewport background"
        );

        // Verify embed card frame title is drawn
        let has_frame_title = dl.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains("example.com/embed"),
            _ => false,
        });
        assert!(
            has_frame_title,
            "IFrame placeholder card should display source URL"
        );

        // Verify sandbox badge is drawn
        let has_sandbox_badge = dl.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains("Sandboxed"),
            _ => false,
        });
        assert!(has_sandbox_badge, "IFrame should display sandbox badge");
    }

    #[test]
    fn test_root_and_body_overflow_clip_exemption() {
        let mut style = ComputedStyle::default();
        style.overflow_x = mango_css::values::Overflow::Hidden;
        style.overflow_y = mango_css::values::Overflow::Visible;

        // 1. Root element (html) with overflow-x: hidden should NOT emit PushClip
        let mut html_box = LayoutBox::new(BoxType::BlockNode, Some(style.clone()));
        html_box.tag_name = Some("html".to_string());
        html_box.dimensions.content = Rect::new(0.0, 0.0, 1280.0, 800.0);

        let dl_html = build_display_list(&html_box);
        let has_clip_html = dl_html
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
        assert!(
            !has_clip_html,
            "html element must not emit PushClip for overflow"
        );

        // 2. Body element with overflow-x: hidden should NOT emit PushClip
        let mut body_box = LayoutBox::new(BoxType::BlockNode, Some(style.clone()));
        body_box.tag_name = Some("body".to_string());
        body_box.dimensions.content = Rect::new(0.0, 0.0, 1280.0, 800.0);

        let dl_body = build_display_list(&body_box);
        let has_clip_body = dl_body
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
        assert!(
            !has_clip_body,
            "body element must not emit PushClip for overflow"
        );

        // 3. Normal container with overflow-x: hidden and overflow-y: visible should have unconstrained vertical clip
        let mut div_box = LayoutBox::new(BoxType::BlockNode, Some(style));
        div_box.tag_name = Some("div".to_string());
        div_box.dimensions.content = Rect::new(10.0, 10.0, 500.0, 300.0);

        let dl_div = build_display_list(&div_box);
        let clip_cmd = dl_div
            .iter()
            .find(|cmd| matches!(cmd, DisplayCommand::PushClip { .. }));
        assert!(clip_cmd.is_some(), "div container should emit PushClip");
        if let Some(DisplayCommand::PushClip { rect }) = clip_cmd {
            assert_eq!(rect.x(), 10.0);
            assert_eq!(rect.width(), 500.0);
            // Vertical extent should be unconstrained so vertical scrolling content isn't clipped
            assert!(
                rect.height() > 10000.0,
                "Vertical extent should be unconstrained for overflow-y: visible"
            );
        }
    }

    #[test]
    fn test_video_and_audio_display_commands() {
        // 1. Video with controls and poster
        let mut video_box = LayoutBox::new(
            BoxType::Video {
                src: "https://example.com/demo.mp4".to_string(),
                poster: Some("data:image/png;base64,demo".to_string()),
                has_controls: true,
                autoplay: false,
                is_loop: false,
                is_muted: false,
                is_playing: false,
                current_time: 15.0,
                duration: 90.0,
                intrinsic_width: 640.0,
                intrinsic_height: 360.0,
                poster_pixels: Some(vec![0xFF00FF; 100]),
                poster_width: 10,
                poster_height: 10,
            },
            None,
        );
        video_box.dimensions.content = Rect::new(0.0, 0.0, 640.0, 360.0);

        let dl_video = build_display_list(&video_box);

        // Verify dark video background
        let has_dark_bg = dl_video.iter().any(|cmd| match cmd {
            DisplayCommand::FillRect { color, rect } => {
                *color == Color::rgb(9, 9, 11) && rect.width() == 640.0
            }
            _ => false,
        });
        assert!(has_dark_bg, "Video should paint dark cinematic background");

        // Verify poster image was painted
        let has_poster = dl_video.iter().any(|cmd| match cmd {
            DisplayCommand::DrawImage { width, height, .. } => *width == 640.0 && *height == 360.0,
            _ => false,
        });
        assert!(has_poster, "Video should paint decoded poster image");

        // Verify controls bar elements (play icon, time, progress track)
        let has_play_btn = dl_video.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains('▶'),
            _ => false,
        });
        assert!(has_play_btn, "Video controls bar should paint play glyph");

        let has_time_text = dl_video.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains("0:15 / 1:30"),
            _ => false,
        });
        assert!(
            has_time_text,
            "Video controls bar should display formatted elapsed and duration timestamps"
        );

        // 2. Audio with controls
        let mut audio_box = LayoutBox::new(
            BoxType::Audio {
                src: "https://example.com/podcast.mp3".to_string(),
                has_controls: true,
                autoplay: false,
                is_loop: false,
                is_muted: false,
                is_playing: true,
                current_time: 45.0,
                duration: 180.0,
                intrinsic_width: 300.0,
                intrinsic_height: 36.0,
            },
            None,
        );
        audio_box.dimensions.content = Rect::new(50.0, 50.0, 300.0, 36.0);

        let dl_audio = build_display_list(&audio_box);

        // Verify audio rounded player pill
        let has_audio_card = dl_audio.iter().any(|cmd| match cmd {
            DisplayCommand::FillRoundedRect { color, rect, .. } => {
                *color == Color::rgb(30, 41, 59) && rect.width() == 300.0
            }
            _ => false,
        });
        assert!(
            has_audio_card,
            "Audio should render dark slate rounded container"
        );

        // Verify pause glyph when playing
        let has_pause_btn = dl_audio.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains("❚❚"),
            _ => false,
        });
        assert!(has_pause_btn, "Playing audio should render pause glyph");

        // Verify timestamp
        let has_audio_time = dl_audio.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains("0:45 / 3:00"),
            _ => false,
        });
        assert!(has_audio_time, "Audio player should format timestamp");
    }

    #[test]
    fn test_canvas_display_list_rendering_and_fallback_suppression() {
        // 1. Canvas with pixel buffer
        let pixels = vec![0xFFFF_0000; 100 * 50]; // 100x50 red pixels
        let mut canvas_box = LayoutBox::new(
            BoxType::Canvas {
                node_id: Some(999),
                width: 100,
                height: 50,
                intrinsic_width: 100.0,
                intrinsic_height: 50.0,
                pixels: Some(pixels),
            },
            None,
        );
        canvas_box.tag_name = Some("canvas".to_string());
        canvas_box.dimensions.content = Rect::new(20.0, 30.0, 100.0, 50.0);

        // Add a fallback child (e.g. <p>Your browser does not support canvas</p>)
        let mut fallback_child = LayoutBox::new(
            BoxType::TextNode("Your browser does not support canvas".to_string()),
            None,
        );
        fallback_child.dimensions.content = Rect::new(20.0, 30.0, 200.0, 20.0);
        canvas_box.children.push(fallback_child);

        let dl = build_display_list(&canvas_box);

        // Verify DrawImage command was emitted with canvas pixels
        let has_draw_image = dl.iter().any(|cmd| match cmd {
            DisplayCommand::DrawImage {
                x,
                y,
                width,
                height,
                pixels,
            } => {
                *x == 20.0
                    && *y == 30.0
                    && *width == 100.0
                    && *height == 50.0
                    && pixels.len() == 100 * 50
            }
            _ => false,
        });
        assert!(
            has_draw_image,
            "Canvas should emit DrawImage command with pixel data"
        );

        // Verify fallback child text is NOT drawn
        let has_fallback_text = dl.iter().any(|cmd| match cmd {
            DisplayCommand::DrawText { text, .. } => text.contains("does not support canvas"),
            _ => false,
        });
        assert!(
            !has_fallback_text,
            "Canvas fallback text must not be rendered"
        );
    }

    #[test]
    fn test_display_list_cache() {
        let mut root = LayoutBox::new(BoxType::BlockNode, None);
        root.dimensions.content = Rect::new(0.0, 0.0, 200.0, 100.0);
        let mut cache = DisplayListCache::new();

        assert!(!cache.is_cached());
        let dl1 = cache.get_or_build(&root, 0.0);
        assert!(cache.is_cached());

        // Mark clean and get again with same scroll_y: must return cached
        root.mark_clean();
        let dl2 = cache.get_or_build(&root, 0.0);
        assert_eq!(dl1.len(), dl2.len());

        // Invalidate dirty rect
        cache.invalidate(Some(Rect::new(10.0, 10.0, 50.0, 50.0)));
        assert!(!cache.is_cached());
        assert_eq!(cache.dirty_regions().len(), 1);

        let dl3 = cache.get_or_build(&root, 0.0);
        assert!(cache.is_cached());
        assert_eq!(cache.dirty_regions().len(), 0);
        assert_eq!(dl1.len(), dl3.len());
    }

    #[test]
    fn test_modal_dialog_backdrop() {
        let mut dialog_box = LayoutBox::new(BoxType::BlockNode, Some(ComputedStyle::default()));
        dialog_box.tag_name = Some("dialog".to_string());
        dialog_box
            .attributes
            .push(("open".to_string(), "".to_string()));
        dialog_box
            .attributes
            .push(("data-mango-modal".to_string(), "true".to_string()));
        dialog_box.dimensions.content = Rect::new(100.0, 100.0, 200.0, 100.0);

        let dl = build_display_list(&dialog_box);

        // Verify dimming backdrop command is emitted
        let has_backdrop = dl.iter().any(|cmd| match cmd {
            DisplayCommand::FillRect { color, rect } => {
                *color == Color::rgba(0, 0, 0, 102) && rect.width() >= 10_000.0
            }
            _ => false,
        });
        assert!(
            has_backdrop,
            "Modal dialog must render dimming backdrop covering viewport"
        );
    }

    #[test]
    fn test_progress_and_meter_rendering() {
        // 1. Determinate progress
        let mut progress_box = LayoutBox::new(BoxType::InlineBlock, Some(ComputedStyle::default()));
        progress_box.tag_name = Some("progress".to_string());
        progress_box
            .attributes
            .push(("value".to_string(), "50".to_string()));
        progress_box
            .attributes
            .push(("max".to_string(), "100".to_string()));
        progress_box.dimensions.content = Rect::new(10.0, 10.0, 160.0, 16.0);

        let dl_progress = build_display_list(&progress_box);
        let has_fill = dl_progress.iter().any(|cmd| match cmd {
            DisplayCommand::FillRoundedRect { rect, color, .. } => {
                rect.width() == 80.0 && *color == Color::rgb(0, 120, 215)
            }
            _ => false,
        });
        assert!(
            has_fill,
            "Progress value 50/100 must render 50% fill (80px)"
        );

        // 2. Meter (optimal zone -> green)
        let mut meter_box = LayoutBox::new(BoxType::InlineBlock, Some(ComputedStyle::default()));
        meter_box.tag_name = Some("meter".to_string());
        meter_box
            .attributes
            .push(("min".to_string(), "0".to_string()));
        meter_box
            .attributes
            .push(("max".to_string(), "100".to_string()));
        meter_box
            .attributes
            .push(("low".to_string(), "33".to_string()));
        meter_box
            .attributes
            .push(("high".to_string(), "66".to_string()));
        meter_box
            .attributes
            .push(("optimum".to_string(), "80".to_string()));
        meter_box
            .attributes
            .push(("value".to_string(), "75".to_string()));
        meter_box.dimensions.content = Rect::new(10.0, 10.0, 100.0, 16.0);

        let dl_meter = build_display_list(&meter_box);
        let has_green_meter = dl_meter.iter().any(|cmd| match cmd {
            DisplayCommand::FillRoundedRect { rect, color, .. } => {
                rect.width() == 75.0 && *color == Color::rgb(40, 167, 69)
            }
            _ => false,
        });
        assert!(
            has_green_meter,
            "Optimal meter must render green fill of 75px"
        );
    }

    #[test]
    fn test_select_optgroup_and_multiple() {
        // Multi-select rendering
        let mut select_box = LayoutBox::new(BoxType::InlineBlock, Some(ComputedStyle::default()));
        select_box.tag_name = Some("select".to_string());
        select_box
            .attributes
            .push(("_mango_is_multiple".to_string(), "true".to_string()));
        select_box.attributes.push((
            "_mango_selected_text".to_string(),
            "Apple, Cherry".to_string(),
        ));
        select_box.dimensions.content = Rect::new(10.0, 10.0, 150.0, 60.0);

        let dl = build_display_list(&select_box);
        let texts: Vec<String> = dl
            .iter()
            .filter_map(|cmd| match cmd {
                DisplayCommand::DrawText { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();

        assert!(
            texts.contains(&"Apple".to_string()),
            "Multi-select must render option 'Apple'"
        );
        assert!(
            texts.contains(&"Cherry".to_string()),
            "Multi-select must render option 'Cherry'"
        );
        assert!(
            !texts.contains(&"▼".to_string()),
            "Multi-select should not render drop-down arrow"
        );
    }

    #[test]
    fn test_visual_effects_display_commands() {
        use mango_css::values::{BlendMode, ClipPath, FilterFunction, Length};

        let mut style = ComputedStyle::default();
        style.mix_blend_mode = BlendMode::Multiply;
        style.clip_path = ClipPath::Circle {
            radius: Length::Px(25.0),
            center_x: Length::Percent(50.0),
            center_y: Length::Percent(50.0),
        };
        style.filter = vec![FilterFunction::Blur(4.0)];
        style.backdrop_filter = vec![FilterFunction::Blur(8.0)];

        let mut b = LayoutBox::new(BoxType::BlockNode, Some(style));
        b.dimensions.content = Rect::new(0.0, 0.0, 100.0, 100.0);

        let dl = build_display_list(&b);

        let has_push_blend = dl.iter().any(|cmd| {
            matches!(
                cmd,
                DisplayCommand::PushBlendMode {
                    mode: BlendMode::Multiply
                }
            )
        });
        let has_pop_blend = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PopBlendMode));
        let has_push_clip = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PushClipPath { .. }));
        let has_pop_clip = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PopClipPath));
        let has_push_filter = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PushFilter { .. }));
        let has_pop_filter = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PopFilter));
        let has_push_backdrop = dl
            .iter()
            .any(|cmd| matches!(cmd, DisplayCommand::PushBackdropFilter { .. }));

        assert!(has_push_blend, "PushBlendMode must be present");
        assert!(has_pop_blend, "PopBlendMode must be present");
        assert!(has_push_clip, "PushClipPath must be present");
        assert!(has_pop_clip, "PopClipPath must be present");
        assert!(has_push_filter, "PushFilter must be present");
        assert!(has_pop_filter, "PopFilter must be present");
        assert!(has_push_backdrop, "PushBackdropFilter must be present");
    }

    #[test]
    fn test_display_list_text_features() {
        use mango_css::values::{
            Length, TextDecoration, TextDecorationThickness, TextEmphasisStyle,
        };

        // 1. Text underline with custom offset and thickness emits DrawLine
        let mut style1 = ComputedStyle::default();
        style1.font_size = 20.0;
        style1.color = Color::rgb(0, 0, 255);
        style1.text_decoration = TextDecoration::Underline;
        style1.text_underline_offset = Length::Px(5.0);
        style1.text_decoration_thickness = TextDecorationThickness::Length(Length::Px(3.0));

        let mut text_box1 = LayoutBox::new(
            BoxType::TextNode("Underlined Text".to_string()),
            Some(style1),
        );
        text_box1.dimensions.content = Rect::new(10.0, 20.0, 150.0, 24.0);

        let dl1 = build_display_list(&text_box1);

        let has_custom_underline = dl1.iter().any(|cmd| match cmd {
            DisplayCommand::DrawLine {
                x1,
                y1,
                x2,
                y2,
                color,
                thickness,
            } => {
                *x1 == 10.0
                    && *x2 == 160.0
                    && (*y1 - y2).abs() < 0.01
                    && *thickness == 3.0
                    && *color == Color::rgb(0, 0, 255)
            }
            _ => false,
        });
        assert!(
            has_custom_underline,
            "Custom text-underline-offset and thickness must emit DrawLine"
        );

        // 2. Text emphasis emits DrawText for mark characters with emphasis color
        let mut style2 = ComputedStyle::default();
        style2.font_size = 20.0;
        style2.color = Color::rgb(0, 0, 0);
        style2.text_emphasis_style = TextEmphasisStyle::FilledCircle;
        style2.text_emphasis_color = Some(Color::rgb(255, 0, 0));

        let mut text_box2 = LayoutBox::new(BoxType::TextNode("ABC".to_string()), Some(style2));
        text_box2.dimensions.content = Rect::new(10.0, 50.0, 60.0, 24.0);

        let dl2 = build_display_list(&text_box2);

        let emphasis_marks: Vec<_> = dl2
            .iter()
            .filter_map(|cmd| match cmd {
                DisplayCommand::DrawText { text, color, .. } if text == "●" => Some(*color),
                _ => None,
            })
            .collect();

        assert_eq!(
            emphasis_marks.len(),
            3,
            "Each non-whitespace character must have an emphasis mark '●'"
        );
        assert_eq!(
            emphasis_marks[0],
            Color::rgb(255, 0, 0),
            "Emphasis mark must use text-emphasis-color"
        );
    }

    #[test]
    fn test_form_controls_accent_color() {
        let accent = Color::rgb(220, 50, 100);

        // 1. Checkbox
        let mut cb_style = ComputedStyle::default();
        cb_style.accent_color = Some(accent);
        let mut cb_box = LayoutBox::new(BoxType::InlineBlock, Some(cb_style));
        cb_box.tag_name = Some("input".to_string());
        cb_box
            .attributes
            .push(("type".to_string(), "checkbox".to_string()));
        cb_box
            .attributes
            .push(("checked".to_string(), "true".to_string()));
        cb_box.dimensions.content = Rect::new(0.0, 0.0, 16.0, 16.0);
        let cb_dl = build_display_list(&cb_box);
        let cb_has_accent = cb_dl.iter().any(
            |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
        );
        assert!(cb_has_accent, "Checked checkbox should use accent-color");

        // 2. Radio
        let mut r_style = ComputedStyle::default();
        r_style.accent_color = Some(accent);
        let mut r_box = LayoutBox::new(BoxType::InlineBlock, Some(r_style));
        r_box.tag_name = Some("input".to_string());
        r_box
            .attributes
            .push(("type".to_string(), "radio".to_string()));
        r_box
            .attributes
            .push(("checked".to_string(), "true".to_string()));
        r_box.dimensions.content = Rect::new(0.0, 0.0, 16.0, 16.0);
        let r_dl = build_display_list(&r_box);
        let r_has_accent = r_dl.iter().any(
            |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
        );
        assert!(r_has_accent, "Checked radio should use accent-color");

        // 3. Range
        let mut rng_style = ComputedStyle::default();
        rng_style.accent_color = Some(accent);
        let mut rng_box = LayoutBox::new(BoxType::InlineBlock, Some(rng_style));
        rng_box.tag_name = Some("input".to_string());
        rng_box
            .attributes
            .push(("type".to_string(), "range".to_string()));
        rng_box
            .attributes
            .push(("value".to_string(), "50".to_string()));
        rng_box.dimensions.content = Rect::new(0.0, 0.0, 100.0, 20.0);
        let rng_dl = build_display_list(&rng_box);
        let rng_has_accent = rng_dl.iter().any(
            |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
        );
        assert!(rng_has_accent, "Range slider should use accent-color");

        // 4. Progress
        let mut prog_style = ComputedStyle::default();
        prog_style.accent_color = Some(accent);
        let mut prog_box = LayoutBox::new(BoxType::InlineBlock, Some(prog_style));
        prog_box.tag_name = Some("progress".to_string());
        prog_box
            .attributes
            .push(("value".to_string(), "50".to_string()));
        prog_box
            .attributes
            .push(("max".to_string(), "100".to_string()));
        prog_box.dimensions.content = Rect::new(0.0, 0.0, 160.0, 16.0);
        let prog_dl = build_display_list(&prog_box);
        let prog_has_accent = prog_dl.iter().any(
            |cmd| matches!(cmd, DisplayCommand::FillRoundedRect { color, .. } if *color == accent),
        );
        assert!(prog_has_accent, "Progress bar should use accent-color");
    }

    #[test]
    fn test_object_fit_contain_cover_none_scale_down() {
        use mango_css::values::ObjectFit;

        // Helper to create a replaced element img box with given object-fit
        let make_img = |fit: ObjectFit, box_w: f32, box_h: f32, iw: f32, ih: f32| -> LayoutBox {
            let mut style = ComputedStyle::default();
            style.object_fit = fit;
            // Default object-position is 50% 50% (centered)
            let mut b = LayoutBox::new(
                BoxType::ReplacedElement {
                    intrinsic_width: iw,
                    intrinsic_height: ih,
                    pixels: vec![0xFFFFFF; (iw as u32 * ih as u32) as usize],
                },
                Some(style),
            );
            b.dimensions.content = Rect::new(10.0, 20.0, box_w, box_h);
            b
        };

        // Intrinsic 200x100 (2:1 ratio) in a 100x100 box
        // Contain: fit to width → rendered 100x50, centered → offset_y = 25
        let img_contain = make_img(ObjectFit::Contain, 100.0, 100.0, 200.0, 100.0);
        let dl = build_display_list(&img_contain);
        let draw_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
        assert!(draw_cmd.is_some(), "Contain should emit DrawImage");
        if let Some(DisplayCommand::DrawImage {
            x,
            y,
            width,
            height,
            ..
        }) = draw_cmd
        {
            assert!(
                (width - 100.0).abs() < 1.0,
                "contain: width should be 100, got {width}"
            );
            assert!(
                (height - 50.0).abs() < 1.0,
                "contain: height should be 50, got {height}"
            );
            // Centered: y offset = 20 + (100 - 50) * 0.5 = 45
            assert!((y - 45.0).abs() < 1.0, "contain: y should be 45, got {y}");
            assert!((x - 10.0).abs() < 1.0, "contain: x should be 10, got {x}");
        }

        // Cover: 200x100 in 100x100 → fit to height → rendered 200x100, centered → offset_x = -50
        let img_cover = make_img(ObjectFit::Cover, 100.0, 100.0, 200.0, 100.0);
        let dl = build_display_list(&img_cover);
        // Cover should emit PushClip before DrawImage
        let has_clip = dl
            .iter()
            .any(|c| matches!(c, DisplayCommand::PushClip { .. }));
        assert!(has_clip, "cover should emit PushClip");
        let draw_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
        if let Some(DisplayCommand::DrawImage {
            x,
            y: _,
            width,
            height,
            ..
        }) = draw_cmd
        {
            assert!(
                (width - 200.0).abs() < 1.0,
                "cover: width should be 200, got {width}"
            );
            assert!(
                (height - 100.0).abs() < 1.0,
                "cover: height should be 100, got {height}"
            );
            // Centered: x offset = 10 + (100 - 200) * 0.5 = -40
            assert!((x - (-40.0)).abs() < 1.0, "cover: x should be -40, got {x}");
        }

        // None: use intrinsic size 200x100 in 100x100 box
        let img_none = make_img(ObjectFit::None, 100.0, 100.0, 200.0, 100.0);
        let dl = build_display_list(&img_none);
        let draw_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
        if let Some(DisplayCommand::DrawImage { width, height, .. }) = draw_cmd {
            assert!(
                (width - 200.0).abs() < 1.0,
                "none: width should be 200, got {width}"
            );
            assert!(
                (height - 100.0).abs() < 1.0,
                "none: height should be 100, got {height}"
            );
        }

        // Scale-down: intrinsic 50x25 in 100x100 → contain would produce 100x50 > intrinsic → use none (50x25)
        let img_sd = make_img(ObjectFit::ScaleDown, 100.0, 100.0, 50.0, 25.0);
        let dl = build_display_list(&img_sd);
        let draw_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
        if let Some(DisplayCommand::DrawImage { width, height, .. }) = draw_cmd {
            assert!(
                (width - 50.0).abs() < 1.0,
                "scale-down (none path): width should be 50, got {width}"
            );
            assert!(
                (height - 25.0).abs() < 1.0,
                "scale-down (none path): height should be 25, got {height}"
            );
        }

        // Scale-down: intrinsic 400x200 in 100x100 → contain produces 100x50 < intrinsic → use contain
        let img_sd2 = make_img(ObjectFit::ScaleDown, 100.0, 100.0, 400.0, 200.0);
        let dl = build_display_list(&img_sd2);
        let draw_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
        if let Some(DisplayCommand::DrawImage { width, height, .. }) = draw_cmd {
            assert!(
                (width - 100.0).abs() < 1.0,
                "scale-down (contain path): width should be 100, got {width}"
            );
            assert!(
                (height - 50.0).abs() < 1.0,
                "scale-down (contain path): height should be 50, got {height}"
            );
        }

        // Fill: should use exact box dimensions
        let img_fill = make_img(ObjectFit::Fill, 100.0, 100.0, 200.0, 100.0);
        let dl = build_display_list(&img_fill);
        let draw_cmd = dl
            .iter()
            .find(|c| matches!(c, DisplayCommand::DrawImage { .. }));
        if let Some(DisplayCommand::DrawImage {
            x,
            y,
            width,
            height,
            ..
        }) = draw_cmd
        {
            assert!(
                (width - 100.0).abs() < 1.0,
                "fill: width should be 100, got {width}"
            );
            assert!(
                (height - 100.0).abs() < 1.0,
                "fill: height should be 100, got {height}"
            );
            assert!((x - 10.0).abs() < 1.0, "fill: x should be 10, got {x}");
            assert!((y - 20.0).abs() < 1.0, "fill: y should be 20, got {y}");
        }
    }
}
