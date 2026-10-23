use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::builder::render_layout_box;
use super::types::apply_opacity;

/// Computes the sticky-positioning displacement for `child` within `parent`.
///
/// Shared by the painter and by layout-geometry publication so that
/// `getBoundingClientRect()` observes exactly the offset that is painted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sticky_shift(
    parent: &LayoutBox,
    child: &LayoutBox,
    cs: &ComputedStyle,
    pad_box: Rect,
    active_viewport: Option<Rect>,
    active_table: Option<Rect>,
    child_offset_x: f32,
    child_offset_y: f32,
    scroll_x: f32,
    scroll_y: f32,
) -> (f32, f32) {
    let (viewport_x, viewport_y, viewport_w, viewport_h) = match active_viewport {
        Some(vp) => (vp.x(), vp.y(), vp.width(), vp.height()),
        None => {
            let (vw, vh) = mango_css::values::get_current_viewport();
            (scroll_x, scroll_y, vw, vh)
        }
    };

    let in_flow_x = child.dimensions.border_box().x() + child_offset_x;
    let in_flow_y = child.dimensions.border_box().y() + child_offset_y;
    let w = child.dimensions.border_box().width();
    let h = child.dimensions.border_box().height();

    // Containing block is the table for table parts, otherwise the parent's content box.
    let is_table_part = matches!(
        parent.tag_name.as_deref(),
        Some("tr" | "tbody" | "thead" | "tfoot" | "table")
    );
    let (cb_top, cb_bottom, cb_left, cb_right) = if is_table_part && let Some(tb) = active_table {
        (tb.y(), tb.bottom(), tb.x(), tb.right())
    } else {
        (
            pad_box.y() + parent.dimensions.padding.top,
            pad_box.bottom() - parent.dimensions.padding.bottom,
            pad_box.x() + parent.dimensions.padding.left,
            pad_box.right() - parent.dimensions.padding.right,
        )
    };

    // Vertical sticky (top / bottom per CSS Position 3 §6.2)
    let has_top = cs.top != mango_css::values::Length::Auto;
    let has_bottom = cs.bottom != mango_css::values::Length::Auto;
    let top_inset = if has_top {
        cs.top.to_px(cs.font_size, 16.0, viewport_h)
    } else {
        0.0
    };
    let bottom_inset = if has_bottom {
        cs.bottom.to_px(cs.font_size, 16.0, viewport_h)
    } else {
        0.0
    };
    // If the inset plus the box cannot fit in the scrollport, the view
    // rectangle collapses; the box then simply follows its inset instead of
    // being clamped back inside its containing block (CSS Position 3 §6.2).
    let clamp_to_cb_y = top_inset + h <= viewport_h && bottom_inset + h <= viewport_h;
    let max_sticky_y = if clamp_to_cb_y {
        (cb_bottom - h).max(in_flow_y)
    } else {
        f32::INFINITY
    };
    let min_sticky_y = if clamp_to_cb_y {
        cb_top.min(in_flow_y)
    } else {
        f32::NEG_INFINITY
    };
    let top_thresh = viewport_y + top_inset + child.dimensions.margin.top;
    let bottom_thresh = viewport_y + viewport_h - bottom_inset - h - child.dimensions.margin.bottom;

    let mut target_y = in_flow_y;
    let ignore_bottom = has_top && has_bottom && (h + top_inset + bottom_inset > viewport_h);
    if has_bottom && !ignore_bottom && in_flow_y > bottom_thresh {
        target_y = bottom_thresh.clamp(min_sticky_y, in_flow_y);
    }
    if has_top && in_flow_y < top_thresh {
        target_y = top_thresh.clamp(in_flow_y, max_sticky_y);
    }

    // Horizontal sticky (left / right per CSS Position 3 §6.2)
    let has_left = cs.left != mango_css::values::Length::Auto;
    let has_right = cs.right != mango_css::values::Length::Auto;
    let left_inset = if has_left {
        cs.left.to_px(cs.font_size, 16.0, viewport_w)
    } else {
        0.0
    };
    let right_inset = if has_right {
        cs.right.to_px(cs.font_size, 16.0, viewport_w)
    } else {
        0.0
    };
    let clamp_to_cb_x = left_inset + w <= viewport_w && right_inset + w <= viewport_w;
    let max_sticky_x = if clamp_to_cb_x {
        (cb_right - w).max(in_flow_x)
    } else {
        f32::INFINITY
    };
    let min_sticky_x = if clamp_to_cb_x {
        cb_left.min(in_flow_x)
    } else {
        f32::NEG_INFINITY
    };
    let left_thresh = viewport_x + left_inset + child.dimensions.margin.left;
    let right_thresh = viewport_x + viewport_w - right_inset - w - child.dimensions.margin.right;

    let mut target_x = in_flow_x;
    let ignore_right = has_left && has_right && (w + left_inset + right_inset > viewport_w);
    if has_right && !ignore_right && in_flow_x > right_thresh {
        target_x = right_thresh.clamp(min_sticky_x, in_flow_x);
    }
    if has_left && in_flow_x < left_thresh {
        target_x = left_thresh.clamp(in_flow_x, max_sticky_x);
    }

    (target_x - in_flow_x, target_y - in_flow_y)
}

pub(crate) fn paint_children_and_scrollbars(
    box_node: &LayoutBox,
    style: Option<&ComputedStyle>,
    pad_box: Rect,
    offset_x: f32,
    offset_y: f32,
    scroll_x: f32,
    scroll_y: f32,
    opacity: f32,
    list: &mut DisplayList,
    scroll_viewport: Option<Rect>,
    table_rect: Option<Rect>,
) {
    let active_viewport = if box_node.is_scroll_container() {
        Some(pad_box)
    } else {
        scroll_viewport
    };
    let active_table = if box_node.tag_name.as_deref() == Some("table") {
        Some(pad_box)
    } else {
        table_rect
    };
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
                child_offset_x += scroll_x;
                child_offset_y += scroll_y;
            } else if cs.position == mango_css::values::Position::Sticky {
                let (dx, dy) = sticky_shift(
                    box_node,
                    child,
                    cs,
                    pad_box,
                    active_viewport,
                    active_table,
                    child_offset_x,
                    child_offset_y,
                    scroll_x,
                    scroll_y,
                );
                child_offset_x += dx;
                child_offset_y += dy;
            }
        }

        let traps_fixed = style.is_some_and(|s| {
            !s.transform.is_identity()
                || !s.filter.is_empty()
                || !s.backdrop_filter.is_empty()
                || matches!(
                    s.contain,
                    mango_css::values::Contain::Paint | mango_css::values::Contain::Strict
                )
                || s.will_change
        });
        let is_fixed_escaped = clip_rect.is_some()
            && !traps_fixed
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
            scroll_x,
            scroll_y,
            active_viewport,
            active_table,
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
        let scrollbar_width = style.map(|s| s.scrollbar_width).unwrap_or_default();
        if scrollbar_width != mango_css::values::ScrollbarWidth::None {
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
            if has_v_scroll && pad_h > 20.0 {
                let track_w = track_thickness;
                let track_x = pad_box.right() - track_w;
                let track_y = pad_box.y();
                let track_h = pad_h;

                list.push(DisplayCommand::FillRect {
                    rect: Rect::new(track_x, track_y, track_w, track_h),
                    color: track_color,
                });

                let thumb_h = ((pad_h / content_h) * pad_h).clamp(16.0, pad_h);
                let max_scroll = (content_h - pad_h).max(1.0);
                let scroll_ratio = (box_node.scroll_offset_y / max_scroll).clamp(0.0, 1.0);
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
            if has_h_scroll && pad_w > 20.0 {
                let track_h = track_thickness;
                let track_y = pad_box.bottom() - track_h;
                let track_x = pad_box.x();
                let track_w = pad_w - if has_v_scroll { track_thickness } else { 0.0 };

                list.push(DisplayCommand::FillRect {
                    rect: Rect::new(track_x, track_y, track_w, track_h),
                    color: track_color,
                });

                let thumb_w = ((pad_w / content_w) * pad_w).clamp(16.0, track_w);
                let max_scroll = (content_w - pad_w).max(1.0);
                let scroll_ratio = (box_node.scroll_offset_x / max_scroll).clamp(0.0, 1.0);
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
    }
}

/// Collects border-box geometry with sticky offsets applied.
///
/// `getBoundingClientRect()` and `offsetTop`/`offsetLeft` observe the sticky
/// offset, so geometry publication must use the same displacement the painter
/// uses. Keys are raw DOM node ids.
pub fn collect_box_rects_with_scroll(
    root: &LayoutBox,
    scroll_x: f32,
    scroll_y: f32,
    out: &mut std::collections::HashMap<u32, [f32; 4]>,
) {
    fn walk(
        node: &LayoutBox,
        offset_x: f32,
        offset_y: f32,
        scroll_viewport: Option<Rect>,
        table_rect: Option<Rect>,
        scroll_x: f32,
        scroll_y: f32,
        out: &mut std::collections::HashMap<u32, [f32; 4]>,
    ) {
        if let Some(node_id) = node.node_id {
            let border = node.dimensions.border_box();
            out.insert(
                node_id.raw(),
                [
                    border.x() + offset_x,
                    border.y() + offset_y,
                    border.width(),
                    border.height(),
                ],
            );
        }

        let orig_pad_box = node.dimensions.padding_box();
        let pad_box = Rect::new(
            orig_pad_box.x() + offset_x,
            orig_pad_box.y() + offset_y,
            orig_pad_box.width(),
            orig_pad_box.height(),
        );
        let active_viewport = if node.is_scroll_container() {
            Some(pad_box)
        } else {
            scroll_viewport
        };
        let active_table = if node.tag_name.as_deref() == Some("table") {
            Some(pad_box)
        } else {
            table_rect
        };

        for child in &node.children {
            let mut child_offset_x = offset_x;
            let mut child_offset_y = offset_y;
            if node.is_scroll_container() {
                child_offset_x -= node.scroll_offset_x;
                child_offset_y -= node.scroll_offset_y;
            }

            if let Some(cs) = &child.style {
                if cs.position == mango_css::values::Position::Fixed {
                    if node.is_scroll_container() {
                        child_offset_x += node.scroll_offset_x;
                        child_offset_y += node.scroll_offset_y;
                    }
                    child_offset_x += scroll_x;
                    child_offset_y += scroll_y;
                } else if cs.position == mango_css::values::Position::Sticky {
                    let (dx, dy) = sticky_shift(
                        node,
                        child,
                        cs,
                        pad_box,
                        active_viewport,
                        active_table,
                        child_offset_x,
                        child_offset_y,
                        scroll_x,
                        scroll_y,
                    );
                    child_offset_x += dx;
                    child_offset_y += dy;
                }
            }

            walk(
                child,
                child_offset_x,
                child_offset_y,
                active_viewport,
                active_table,
                scroll_x,
                scroll_y,
                out,
            );
        }
    }

    out.clear();
    walk(root, 0.0, 0.0, None, None, scroll_x, scroll_y, out);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PaintLayer {
    /// Positioned children with negative z-index (CSS 2.1 App E layer 2)
    NegativeZ(i32),
    /// Block-level normal flow backgrounds (CSS 2.1 App E layer 3)
    BlockBackgrounds,
    /// Non-positioned floats (CSS 2.1 App E layer 4)
    Float,
    /// In-flow inline content (CSS 2.1 App E layer 5)
    Inline,
    /// Positioned children with z-index: auto or z-index: 0 (CSS 2.1 App E layer 6)
    ZeroOrAutoZ,
    /// Positioned children with positive z-index (CSS 2.1 App E layer 7)
    PositiveZ(i32),
    /// Top-layer elements (modal dialogs and open popovers) rendered above all normal z-index stacking contexts
    TopLayer(u32),
}

pub(crate) fn paint_layer_of(box_node: &LayoutBox) -> PaintLayer {
    let is_top_layer = (box_node.tag_name.as_deref() == Some("dialog")
        && box_node.get_attribute("open").is_some())
        || box_node.get_attribute("data-mango-popover-open") == Some("true")
        || box_node.get_attribute(":popover-open") == Some("true");
    if is_top_layer {
        return PaintLayer::TopLayer(1);
    }
    if let Some(s) = &box_node.style {
        let creates_stacking_context = s.position != mango_css::values::Position::Static
            || s.opacity < 1.0
            || !s.transform.is_identity()
            || !s.filter.is_empty()
            || !s.backdrop_filter.is_empty()
            || s.isolation == mango_css::values::Isolation::Isolate
            || s.mix_blend_mode != mango_css::values::BlendMode::Normal
            || s.clip_path != mango_css::values::ClipPath::None
            || s.mask_image.is_some()
            || matches!(
                s.contain,
                mango_css::values::Contain::Paint | mango_css::values::Contain::Strict
            )
            || s.will_change;

        if creates_stacking_context {
            match s.z_index {
                Some(z) if z < 0 => PaintLayer::NegativeZ(z),
                Some(z) if z > 0 => PaintLayer::PositiveZ(z),
                _ => PaintLayer::ZeroOrAutoZ,
            }
        } else if s.float != mango_css::values::Float::None {
            PaintLayer::Float
        } else if box_node.is_inline() {
            PaintLayer::Inline
        } else if has_positioned_descendant(box_node) {
            PaintLayer::ZeroOrAutoZ
        } else {
            PaintLayer::BlockBackgrounds
        }
    } else if box_node.is_inline() {
        PaintLayer::Inline
    } else if has_positioned_descendant(box_node) {
        PaintLayer::ZeroOrAutoZ
    } else {
        PaintLayer::BlockBackgrounds
    }
}

fn has_positioned_descendant(b: &LayoutBox) -> bool {
    // Memoized per box: assigning paint layers previously rescanned each
    // child's subtree, which is quadratic on deep documents.
    b.has_positioned_descendant()
}
