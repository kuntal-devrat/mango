use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::builder::render_layout_box;
use super::types::apply_opacity;

fn sticky_axis_target(
    in_flow: f32,
    size: f32,
    viewport_start: f32,
    viewport_size: f32,
    start: mango_css::values::Length,
    end: mango_css::values::Length,
    font_size: f32,
    containing_start: f32,
    containing_end: f32,
    margin_start: f32,
    margin_end: f32,
) -> f32 {
    let has_start = start != mango_css::values::Length::Auto;
    let has_end = end != mango_css::values::Length::Auto;
    if !has_start && !has_end {
        return in_flow;
    }

    let start_px = if has_start {
        start.to_px(font_size, 16.0, viewport_size)
    } else {
        0.0
    };
    let mut end_px = if has_end {
        end.to_px(font_size, 16.0, viewport_size)
    } else {
        0.0
    };

    // CSS Position 3 expands the view rectangle's auto side when the sticky
    // border box is larger than the inset-constrained rectangle. This keeps
    // large-top/large-bottom sticky boxes reachable instead of disabling
    // sticky positioning entirely.
    let oversized_view = viewport_size - start_px - end_px < size;
    if oversized_view {
        if has_start {
            end_px = viewport_size - start_px - size;
        }
    }

    let start_edge = viewport_start + start_px + margin_start;
    let end_edge = viewport_start + viewport_size - end_px - size - margin_end;
    let mut target = in_flow;

    if oversized_view && has_start && start_px > 0.0 {
        // With an oversized sticky box, the start inset is a displacement
        // from its scrolled in-flow position; anchoring it to the scrollport
        // edge would incorrectly erase the scroll offset.
        target = in_flow + start_px;
    } else if has_start && in_flow < start_edge {
        target = start_edge;
    }
    if has_end && target > end_edge {
        target = end_edge;
    }

    // A sticky translation cannot move the border box outside its containing
    // block. If the requested movement would require an inverted range (for
    // example, an item larger than the containing block), leave it in-flow.
    if target > in_flow {
        let max_target = containing_end - size;
        if max_target >= in_flow {
            target = target.min(max_target);
        } else {
            target = in_flow;
        }
    } else if target < in_flow {
        if containing_start <= in_flow {
            target = target.max(containing_start);
        } else {
            target = in_flow;
        }
    }

    target
}

/// Computes the sticky-positioning displacement for `child` within `parent`.
///
/// Shared by the painter and by layout-geometry publication so that
/// `getBoundingClientRect()` observes exactly the offset that is painted.
///
/// `escaped_scroll_x/y` carries the scroll-container scroll offset that was
/// canceled by a fixed or absolutely-positioned ancestor.  When non-zero the
/// in-flow position has had the scroll added back (so the element does not
/// visually scroll), but sticky math must still see the scrolled position.
/// We subtract the escaped amount to recover the effective scrollport
/// coordinate (CSS Position 3 §6.2 — the scroll container is determined by
/// the nearest scroll-container ancestor, regardless of intervening
/// fixed/absolute elements).
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
    escaped_scroll_x: f32,
    escaped_scroll_y: f32,
) -> (f32, f32) {
    let (viewport_x, viewport_y, viewport_w, viewport_h) = match active_viewport {
        Some(vp) => (vp.x(), vp.y(), vp.width(), vp.height()),
        None => {
            let (vw, vh) = mango_css::values::get_current_viewport();
            (scroll_x, scroll_y, vw, vh)
        }
    };

    // Recover the effective scrollport coordinate: subtract the scroll
    // offset that a fixed/absolute ancestor canceled.
    let in_flow_x = child.dimensions.border_box().x() + child_offset_x - escaped_scroll_x;
    let in_flow_y = child.dimensions.border_box().y() + child_offset_y - escaped_scroll_y;
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

    let target_y = sticky_axis_target(
        in_flow_y,
        h,
        viewport_y,
        viewport_h,
        cs.top,
        cs.bottom,
        cs.font_size,
        cb_top,
        cb_bottom,
        child.dimensions.margin.top,
        child.dimensions.margin.bottom,
    );
    let target_x = sticky_axis_target(
        in_flow_x,
        w,
        viewport_x,
        viewport_w,
        cs.left,
        cs.right,
        cs.font_size,
        cb_left,
        cb_right,
        child.dimensions.margin.left,
        child.dimensions.margin.right,
    );

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
    escaped_scroll_x: f32,
    escaped_scroll_y: f32,
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
        // Tracks scroll-container scroll that was canceled by a fixed
        // ancestor, so sticky descendants can recover the scrolled position.
        let mut child_escaped_sx = escaped_scroll_x;
        let mut child_escaped_sy = escaped_scroll_y;

        if box_node.is_scroll_container() {
            child_offset_x -= box_node.scroll_offset_x;
            child_offset_y -= box_node.scroll_offset_y;
        }

        if let Some(cs) = &child.style {
            if cs.position == mango_css::values::Position::Fixed {
                if box_node.is_scroll_container() {
                    child_offset_x += box_node.scroll_offset_x;
                    child_offset_y += box_node.scroll_offset_y;
                    // The scroll offset was added back (canceled). Track it so
                    // sticky descendants can subtract it to recover the
                    // effective scrolled position.  Accumulate in case an
                    // outer scroll container was also escaped.
                    child_escaped_sx += box_node.scroll_offset_x;
                    child_escaped_sy += box_node.scroll_offset_y;
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
                    child_escaped_sx,
                    child_escaped_sy,
                );
                // sticky_shift subtracts escaped_scroll internally to compute
                // the effective scrollport position, but the painted position
                // (border_box + offset) still includes the escaped scroll.
                // Subtract it here so the element paints at the target.
                child_offset_x += dx - child_escaped_sx;
                child_offset_y += dy - child_escaped_sy;
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
            child_escaped_sx,
            child_escaped_sy,
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
        escaped_scroll_x: f32,
        escaped_scroll_y: f32,
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
            let mut child_escaped_sx = escaped_scroll_x;
            let mut child_escaped_sy = escaped_scroll_y;

            if node.is_scroll_container() {
                child_offset_x -= node.scroll_offset_x;
                child_offset_y -= node.scroll_offset_y;
            }

            if let Some(cs) = &child.style {
                if cs.position == mango_css::values::Position::Fixed {
                    if node.is_scroll_container() {
                        child_offset_x += node.scroll_offset_x;
                        child_offset_y += node.scroll_offset_y;
                        child_escaped_sx += node.scroll_offset_x;
                        child_escaped_sy += node.scroll_offset_y;
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
                        child_escaped_sx,
                        child_escaped_sy,
                    );
                    child_offset_x += dx - child_escaped_sx;
                    child_offset_y += dy - child_escaped_sy;
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
                child_escaped_sx,
                child_escaped_sy,
                out,
            );
        }
    }

    out.clear();
    walk(
        root, 0.0, 0.0, None, None, scroll_x, scroll_y, 0.0, 0.0, out,
    );
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
