use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::types::apply_opacity;

pub(crate) fn paint_text(
    box_node: &LayoutBox,
    style: Option<&ComputedStyle>,
    border_box: Rect,
    content_x: f32,
    content_y: f32,
    opacity: f32,
    list: &mut DisplayList,
) {
    // 3. Paint Text (for text nodes)
    if let BoxType::TextNode(text) = &box_node.box_type
        && let Some(s) = style
        && !text.is_empty()
        && s.font_size > 0.5
        && (s.color.a > 0
            || !s.text_decoration.is_none()
            || s.text_emphasis_style.mark_char().is_some()
            || !s.all_text_shadows().is_empty())
    {
        let is_bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
        let family = mango_render::FontFamily::from_css_name(&s.font_family);
        let style_font = match s.font_style {
            mango_css::values::FontStyle::Italic => mango_render::FontStyle::Italic,
            mango_css::values::FontStyle::Oblique => mango_render::FontStyle::Oblique,
            mango_css::values::FontStyle::Normal => mango_render::FontStyle::Normal,
        };
        let decoration = if s.text_decoration.has_underline() {
            mango_render::TextDecoration::Underline
        } else if s.text_decoration.has_line_through() {
            mango_render::TextDecoration::LineThrough
        } else if s.text_decoration.has_overline() {
            mango_render::TextDecoration::Overline
        } else {
            mango_render::TextDecoration::None
        };

        // Custom text-decoration rendering (underline, overline, line-through, thickness, color)
        let text_w = box_node.dimensions.content.width();
        if text_w > 0.0 && !s.text_decoration.is_none() {
            let decor_color = apply_opacity(s.resolved_text_decoration_color(), opacity);
            let thickness_px = match s.text_decoration_thickness {
                mango_css::values::TextDecorationThickness::Auto
                | mango_css::values::TextDecorationThickness::FromFont => {
                    (s.font_size / 14.0).max(1.0)
                }
                mango_css::values::TextDecorationThickness::Length(len) => {
                    len.to_px(s.font_size, 16.0, text_w).max(1.0)
                }
            };
            if s.text_decoration.has_underline() {
                let offset_px = s.text_underline_offset.to_px(s.font_size, 16.0, text_w);
                let line_y = content_y + (s.font_size * 0.8) + 2.0 + offset_px;
                list.push(DisplayCommand::DrawLine {
                    x1: content_x,
                    y1: line_y,
                    x2: content_x + text_w,
                    y2: line_y,
                    color: decor_color,
                    thickness: thickness_px,
                });
            }
            if s.text_decoration.has_overline() {
                let line_y = content_y + 1.0;
                list.push(DisplayCommand::DrawLine {
                    x1: content_x,
                    y1: line_y,
                    x2: content_x + text_w,
                    y2: line_y,
                    color: decor_color,
                    thickness: thickness_px,
                });
            }
            if s.text_decoration.has_line_through() {
                let line_y = content_y + (s.font_size * 0.5);
                list.push(DisplayCommand::DrawLine {
                    x1: content_x,
                    y1: line_y,
                    x2: content_x + text_w,
                    y2: line_y,
                    color: decor_color,
                    thickness: thickness_px,
                });
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
        // Multiple shadows paint back-to-front (first shadow painted on top per CSS specs).
        for shadow in s.all_text_shadows().iter().rev() {
            if shadow.color.a > 0
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
        }
        let is_clip_text = s.background_clip == mango_css::values::BackgroundClip::Text;
        let text_gradient = if is_clip_text {
            s.background_gradient.as_ref().or_else(|| {
                s.background_layers
                    .iter()
                    .find_map(|l| l.gradient.as_deref())
            })
        } else {
            None
        };

        if let Some(grad) = text_gradient {
            let mut resolved_grad = grad.clone();
            resolved_grad.resolve_current_color(s.color);
            list.push(DisplayCommand::DrawTextWithGradient {
                text: text.clone(),
                x: content_x,
                y: content_y,
                gradient: Box::new(resolved_grad),
                gradient_rect: border_box,
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
        } else if s.color.a > 0 {
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
    }
}
