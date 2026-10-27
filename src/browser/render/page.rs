//! Page content display list translation, viewport clipping, and highlight rendering.

use mango_core::{Color, Rect};
use mango_render::display_list::{DisplayCommand, DisplayList};

use super::super::BrowserChrome;
use crate::chrome_ui::*;

impl BrowserChrome {
    /// Builds the complete display list for the browser window.
    pub fn build_display_list(&self, (width, height): (u32, u32)) -> DisplayList {
        let mut dl = DisplayList::new();
        let w = width as f32;
        let h = height as f32;

        let content_y = HEADER_HEIGHT + 1.0;
        let content_h = (h - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);

        // Content area background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, content_y, w, content_h),
            color: self.canvas_bg,
        });

        // Translate & clip page commands via display list cache
        let page_dl = if let Some(root) = &self.root_box {
            self.display_list_cache
                .lock()
                .unwrap()
                .get_or_build(root, self.scroll_y)
        } else {
            self.page_display_list.clone()
        };

        // The page may only paint inside the content viewport. A single clip around
        // the translated page commands keeps page overflow (wide tables, transformed
        // layers, absolutely positioned art) out of the browser chrome and lets
        // partially scrolled lines and images draw clipped, as Chromium does, instead
        // of popping in and out one baseline at a time.
        dl.push(DisplayCommand::PushClip {
            rect: Rect::new(0.0, content_y, w, content_h),
        });

        for cmd in page_dl.into_iter() {
            match cmd {
                DisplayCommand::FillRect { rect, color } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    if clamped_bottom > clamped_top {
                        dl.push(DisplayCommand::FillRect {
                            rect: Rect::new(
                                rect.x(),
                                clamped_top,
                                rect.width(),
                                clamped_bottom - clamped_top,
                            ),
                            color,
                        });
                    }
                }
                DisplayCommand::FillRoundedRect { rect, color, radii } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    if translated_y + rect.height() >= content_y
                        && translated_y <= content_y + content_h
                    {
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                            color,
                            radii,
                        });
                    }
                }
                DisplayCommand::DrawBorder {
                    rect,
                    color,
                    widths,
                    radii,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    if translated_y + rect.height() >= content_y
                        && translated_y <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawBorder {
                            rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                            color,
                            widths,
                            radii,
                        });
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
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let translated_rect =
                        Rect::new(rect.x(), translated_y, rect.width(), rect.height());
                    let max_blur = blur_radius + spread_radius;
                    if translated_rect.bottom() + max_blur >= content_y
                        && translated_rect.y() - max_blur <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawBoxShadow {
                            rect: translated_rect,
                            color,
                            offset_x,
                            offset_y,
                            blur_radius,
                            spread_radius,
                            radii,
                            inset,
                        });
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
                    let translated_y = y + content_y - self.scroll_y;
                    // Cull only runs whose glyph box (ascent above the baseline, descent
                    // below it) cannot touch the content band; the rest is clipped.
                    if translated_y + font_size * 0.3 >= content_y
                        && translated_y - font_size * 1.2 <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawText {
                            text,
                            x,
                            y: translated_y,
                            color,
                            font_size,
                            weight,
                            family,
                            style,
                            decoration,
                            letter_spacing,
                        });
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
                    let ty1 = y1 + content_y - self.scroll_y;
                    let ty2 = y2 + content_y - self.scroll_y;
                    let (lo, hi) = (ty1.min(ty2) - thickness, ty1.max(ty2) + thickness);
                    if hi >= content_y && lo <= content_y + content_h {
                        dl.push(DisplayCommand::DrawLine {
                            x1,
                            y1: ty1,
                            x2,
                            y2: ty2,
                            color,
                            thickness,
                        });
                    }
                }
                DisplayCommand::DrawImage {
                    x,
                    y,
                    width,
                    height,
                    pixels,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    // The content clip crops the visible rows, so the image is pushed
                    // once (no per-row re-slicing) and the painter clips it.
                    if translated_y + height >= content_y && translated_y <= content_y + content_h {
                        dl.push(DisplayCommand::DrawImage {
                            x,
                            y: translated_y,
                            width,
                            height,
                            pixels,
                        });
                    }
                }
                DisplayCommand::FillGradient {
                    rect,
                    gradient,
                    radii,
                    opacity,
                } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    if clamped_bottom > clamped_top {
                        dl.push(DisplayCommand::FillGradient {
                            rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                            gradient,
                            radii,
                            opacity,
                        });
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
                    blur_radius,
                    letter_spacing,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    if translated_y + font_size * 0.3 + blur_radius >= content_y
                        && translated_y - font_size * 1.2 - blur_radius <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawTextShadow {
                            text,
                            x,
                            y: translated_y,
                            color,
                            font_size,
                            weight,
                            family,
                            style,
                            blur_radius,
                            letter_spacing,
                        });
                    }
                }
                DisplayCommand::PushTransform { matrix } => {
                    let sy = content_y - self.scroll_y;
                    let m_screen = [
                        matrix[0],
                        matrix[1],
                        matrix[2],
                        matrix[3],
                        matrix[4] - matrix[2] * sy,
                        matrix[5] + (1.0 - matrix[3]) * sy,
                    ];
                    dl.push(DisplayCommand::PushTransform { matrix: m_screen });
                }
                DisplayCommand::PopTransform => {
                    dl.push(DisplayCommand::PopTransform);
                }
                DisplayCommand::PushClip { rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    let clamped_top = translated_y.max(content_y);
                    let clamped_bottom = (translated_y + rect.height()).min(content_y + content_h);
                    dl.push(DisplayCommand::PushClip {
                        rect: Rect::new(
                            rect.x(),
                            clamped_top,
                            rect.width(),
                            (clamped_bottom - clamped_top).max(0.0),
                        ),
                    });
                }
                DisplayCommand::PopClip => {
                    dl.push(DisplayCommand::PopClip);
                }
                DisplayCommand::PushFilter { filters, rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    dl.push(DisplayCommand::PushFilter {
                        filters,
                        rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                    });
                }
                DisplayCommand::PopFilter => {
                    dl.push(DisplayCommand::PopFilter);
                }
                DisplayCommand::PushBackdropFilter { filters, rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    dl.push(DisplayCommand::PushBackdropFilter {
                        filters,
                        rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                    });
                }
                DisplayCommand::PopBackdropFilter => {
                    dl.push(DisplayCommand::PopBackdropFilter);
                }
                DisplayCommand::PushBlendMode { mode } => {
                    dl.push(DisplayCommand::PushBlendMode { mode });
                }
                DisplayCommand::PopBlendMode => {
                    dl.push(DisplayCommand::PopBlendMode);
                }
                DisplayCommand::PushClipPath { clip_path, rect } => {
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    dl.push(DisplayCommand::PushClipPath {
                        clip_path,
                        rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                    });
                }
                DisplayCommand::PopClipPath => {
                    dl.push(DisplayCommand::PopClipPath);
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
                    let translated_y = rect.y() + content_y - self.scroll_y;
                    if translated_y + rect.height() >= content_y
                        && translated_y <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawBorderImage {
                            rect: Rect::new(rect.x(), translated_y, rect.width(), rect.height()),
                            pixels,
                            img_width,
                            img_height,
                            slice,
                            widths,
                            repeat_h,
                            repeat_v,
                            fill,
                        });
                    }
                }
                DisplayCommand::DrawTextWithGradient {
                    text,
                    x,
                    y,
                    gradient,
                    font_size,
                    weight,
                    family,
                    style,
                    decoration,
                    letter_spacing,
                    gradient_rect,
                } => {
                    let translated_y = y + content_y - self.scroll_y;
                    let translated_rect_y = gradient_rect.y() + content_y - self.scroll_y;
                    if translated_y + font_size * 0.3 >= content_y
                        && translated_y - font_size * 1.2 <= content_y + content_h
                    {
                        dl.push(DisplayCommand::DrawTextWithGradient {
                            text,
                            x,
                            y: translated_y,
                            gradient,
                            font_size,
                            weight,
                            family,
                            style,
                            decoration,
                            letter_spacing,
                            gradient_rect: Rect::new(
                                gradient_rect.x(),
                                translated_rect_y,
                                gradient_rect.width(),
                                gradient_rect.height(),
                            ),
                        });
                    }
                }
            }
        }

        // Render Find-in-Page match highlights
        if self.find_in_page.active && !self.find_in_page.matches.is_empty() {
            for (i, m) in self.find_in_page.matches.iter().enumerate() {
                let trans_y = m.y() + content_y - self.scroll_y;
                let clamped_top = trans_y.max(content_y);
                let clamped_bottom = (trans_y + m.height()).min(content_y + content_h);
                if clamped_bottom > clamped_top {
                    let is_curr = i == self.find_in_page.current_match;
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: Rect::new(
                            m.x() - 2.0,
                            clamped_top,
                            m.width() + 4.0,
                            clamped_bottom - clamped_top,
                        ),
                        color: if is_curr {
                            Color::rgba(255, 140, 0, 220)
                        } else {
                            Color::rgba(255, 235, 59, 150)
                        },
                        radii: [3.0; 4],
                    });
                }
            }
        }

        dl.push(DisplayCommand::PopClip);

        // Append Chrome UI display commands
        let chrome_dl = self.build_chrome_display_list((width, height));
        for cmd in chrome_dl.into_iter() {
            dl.push(cmd);
        }

        // Append Scrollbar overlay commands if active
        if let Some((bounds, s_dl)) = self.build_scrollbar_display_list((width, height)) {
            for mut cmd in s_dl.into_iter() {
                match &mut cmd {
                    DisplayCommand::FillRect { rect, .. } => {
                        *rect = Rect::new(
                            rect.x() + bounds.x(),
                            rect.y() + bounds.y(),
                            rect.width(),
                            rect.height(),
                        );
                    }
                    DisplayCommand::FillRoundedRect { rect, .. } => {
                        *rect = Rect::new(
                            rect.x() + bounds.x(),
                            rect.y() + bounds.y(),
                            rect.width(),
                            rect.height(),
                        );
                    }
                    _ => {}
                }
                dl.push(cmd);
            }
        }

        dl
    }
}
