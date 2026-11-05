//! Container scrollbar painting.

use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use super::types::apply_opacity;
use crate::box_tree::LayoutBox;

pub(crate) fn paint_container_scrollbars(
    box_node: &LayoutBox,
    style: Option<&ComputedStyle>,
    pad_box: Rect,
    opacity: f32,
    list: &mut DisplayList,
) {
    let scrollbar_width = style.map(|s| s.scrollbar_width).unwrap_or_default();
    if scrollbar_width == mango_css::values::ScrollbarWidth::None {
        return;
    }
    let (track_thickness, thumb_inset) = match scrollbar_width {
        mango_css::values::ScrollbarWidth::Thin => (4.0f32, 0.5f32),
        _ => (6.0f32, 1.0f32),
    };
    let (custom_thumb, custom_track) = style
        .and_then(|s| s.scrollbar_color)
        .map(|(th, tr)| {
            (
                Some(apply_opacity(th, opacity)),
                Some(apply_opacity(tr, opacity)),
            )
        })
        .unwrap_or((None, None));

    let track_color = custom_track.unwrap_or(Color::rgba(0, 0, 0, 20));
    let thumb_color = custom_thumb.unwrap_or(Color::rgba(100, 100, 100, 150));

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
    let is_v_scrollbar_on_left = style.is_some_and(|s| {
        s.writing_mode == mango_css::values::WritingMode::VerticalRl
            || (s.writing_mode == mango_css::values::WritingMode::HorizontalTb
                && s.direction == mango_css::values::Direction::Rtl)
    });
    if has_v_scroll && pad_h >= track_thickness && pad_w >= track_thickness {
        let track_w = track_thickness;
        let track_x = if is_v_scrollbar_on_left {
            pad_box.x()
        } else {
            pad_box.right() - track_w
        };
        let track_y = pad_box.y();
        let track_h = pad_h;

        list.push(DisplayCommand::FillRect {
            rect: Rect::new(track_x, track_y, track_w, track_h),
            color: track_color,
        });

        let thumb_h = ((pad_h / content_h) * pad_h).clamp(16.0f32.min(pad_h), pad_h);
        let max_scroll = (content_h - pad_h).max(1.0);
        let scroll_ratio = (box_node.scroll_offset_y.abs() / max_scroll).clamp(0.0, 1.0);
        let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

        list.push(DisplayCommand::FillRoundedRect {
            rect: Rect::new(
                track_x + thumb_inset,
                thumb_y,
                track_w - thumb_inset * 2.0,
                thumb_h,
            ),
            color: thumb_color,
            radii: [thumb_inset * 2.0; 4],
        });
    }

    // Horizontal scrollbar
    let has_h_scroll = if let Some(s) = style {
        s.overflow_x == mango_css::values::Overflow::Scroll
            || (s.overflow_x == mango_css::values::Overflow::Auto && content_w > pad_w)
    } else {
        false
    };
    if has_h_scroll && pad_w >= track_thickness && pad_h >= track_thickness {
        let track_h = track_thickness;
        let track_y = pad_box.bottom() - track_h;
        let track_x = pad_box.x()
            + if has_v_scroll && is_v_scrollbar_on_left {
                track_thickness
            } else {
                0.0
            };
        let track_w = pad_w - if has_v_scroll { track_thickness } else { 0.0 };

        list.push(DisplayCommand::FillRect {
            rect: Rect::new(track_x, track_y, track_w, track_h),
            color: track_color,
        });

        let thumb_w = ((pad_w / content_w) * pad_w).clamp(16.0f32.min(track_w), track_w);
        let max_scroll = (content_w - pad_w).max(1.0);
        let scroll_ratio = (box_node.scroll_offset_x.abs() / max_scroll).clamp(0.0, 1.0);
        let thumb_x = track_x + scroll_ratio * (track_w - thumb_w);

        list.push(DisplayCommand::FillRoundedRect {
            rect: Rect::new(
                thumb_x,
                track_y + thumb_inset,
                thumb_w,
                track_h - thumb_inset * 2.0,
            ),
            color: thumb_color,
            radii: [thumb_inset * 2.0; 4],
        });
    }
}
