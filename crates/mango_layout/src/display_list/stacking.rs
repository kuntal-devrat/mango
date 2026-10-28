use mango_core::{Color, Rect};
use mango_css::computed::ComputedStyle;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::builder::render_layout_box;
use super::paint_layer::{creates_stacking_context, paint_layer_of};
use super::types::apply_opacity;

#[derive(Debug, Clone, Copy)]
pub(crate) struct StickyContainingBlock {
    pub rect: Rect,
    pub scroll_right: Option<f32>,
    pub scroll_bottom: Option<f32>,
}

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
    containing_scroll_end: Option<f32>,
    inline_offset: f32,
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

    let start_edge = viewport_start + start_px + inline_offset;
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

    let eff_containing_end = if oversized_view && has_start && start_px > 0.0 {
        containing_scroll_end
            .unwrap_or(containing_end)
            .max(containing_end)
    } else {
        containing_end
    };

    // A sticky translation cannot move the border box outside its containing
    // block. If the requested movement would require an inverted range (for
    // example, an item larger than the containing block), leave it in-flow.
    if target > in_flow {
        let max_target = eff_containing_end - size;
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
    sticky_cb: Option<StickyContainingBlock>,
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
    let (cb_top, cb_bottom, cb_left, cb_right, containing_scroll_right, containing_scroll_bottom) =
        if is_table_part && let Some(tb) = active_table {
            (tb.y(), tb.bottom(), tb.x(), tb.right(), None, None)
        } else if let Some(scb) = sticky_cb {
            (
                scb.rect.y() + parent.dimensions.padding.top,
                scb.rect.bottom() - parent.dimensions.padding.bottom,
                scb.rect.x() + parent.dimensions.padding.left,
                scb.rect.right() - parent.dimensions.padding.right,
                scb.scroll_right,
                scb.scroll_bottom,
            )
        } else {
            let (sr, sb) = if parent.is_scroll_container() {
                let (sw, sh) = parent.scrollable_extent();
                (
                    Some(pad_box.x() + sw - parent.dimensions.padding.right),
                    Some(pad_box.y() + sh - parent.dimensions.padding.bottom),
                )
            } else {
                (None, None)
            };
            (
                pad_box.y() + parent.dimensions.padding.top,
                pad_box.bottom() - parent.dimensions.padding.bottom,
                pad_box.x() + parent.dimensions.padding.left,
                pad_box.right() - parent.dimensions.padding.right,
                sr,
                sb,
            )
        };

    let is_inline_text = matches!(child.box_type, BoxType::TextNode(_));
    let inline_offset_y = if is_inline_text {
        child.dimensions.margin.top
    } else {
        0.0
    };
    let inline_offset_x = if is_inline_text {
        child.dimensions.margin.left
    } else {
        0.0
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
        containing_scroll_bottom,
        inline_offset_y,
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
        containing_scroll_right,
        inline_offset_x,
        child.dimensions.margin.right,
    );

    (target_x - in_flow_x, target_y - in_flow_y)
}

struct PaintItem<'a> {
    child: &'a LayoutBox,
    is_flex_or_grid_item: bool,
    is_sc: bool,
    orig_idx: usize,
    offset_x: f32,
    offset_y: f32,
}

fn collect_hoisted_stacking_contexts<'a>(
    parent: &'a LayoutBox,
    offset_x: f32,
    offset_y: f32,
    out: &mut Vec<PaintItem<'a>>,
    orig_idx_counter: &mut usize,
) {
    if matches!(
        parent.tag_name.as_deref(),
        Some("table" | "thead" | "tbody" | "tfoot" | "tr")
    ) {
        return;
    }
    let parent_is_fg = parent.style.as_ref().is_some_and(|s| {
        matches!(
            s.display,
            mango_css::values::Display::Flex
                | mango_css::values::Display::InlineFlex
                | mango_css::values::Display::Grid
                | mango_css::values::Display::InlineGrid
        )
    });
    let mut child_offset_x = offset_x;
    let mut child_offset_y = offset_y;
    if parent.is_scroll_container() {
        child_offset_x -= parent.scroll_offset_x;
        child_offset_y -= parent.scroll_offset_y;
    }
    for child in &parent.children {
        if child.is_out_of_flow() {
            continue;
        }
        let is_item = parent_is_fg;
        let should_hoist = is_item && child.style.as_ref().and_then(|s| s.z_index).is_some();
        if should_hoist {
            *orig_idx_counter += 1;
            out.push(PaintItem {
                child,
                is_flex_or_grid_item: true,
                is_sc: true,
                orig_idx: *orig_idx_counter,
                offset_x: child_offset_x,
                offset_y: child_offset_y,
            });
        } else if !creates_stacking_context(child, is_item) {
            collect_hoisted_stacking_contexts(
                child,
                child_offset_x,
                child_offset_y,
                out,
                orig_idx_counter,
            );
        }
    }
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
    sticky_cb: Option<StickyContainingBlock>,
    skip_html_bg: bool,
    skip_body_bg: bool,
    is_stacking_context: bool,
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
    let is_flex_or_grid = style.is_some_and(|s| {
        matches!(
            s.display,
            mango_css::values::Display::Flex
                | mango_css::values::Display::InlineFlex
                | mango_css::values::Display::Grid
                | mango_css::values::Display::InlineGrid
        )
    });
    let mut items: Vec<PaintItem> = Vec::new();
    let mut orig_idx_counter = box_node.children.len();

    if is_stacking_context {
        for (idx, child) in box_node.children.iter().enumerate() {
            let is_item = is_flex_or_grid && !child.is_out_of_flow();
            let is_sc = creates_stacking_context(child, is_item);
            if is_sc {
                items.push(PaintItem {
                    child,
                    is_flex_or_grid_item: is_item,
                    is_sc: true,
                    orig_idx: idx,
                    offset_x,
                    offset_y,
                });
            } else {
                items.push(PaintItem {
                    child,
                    is_flex_or_grid_item: is_item,
                    is_sc: false,
                    orig_idx: idx,
                    offset_x,
                    offset_y,
                });
                let mut child_off_x = offset_x;
                let mut child_off_y = offset_y;
                if box_node.is_scroll_container() {
                    child_off_x -= box_node.scroll_offset_x;
                    child_off_y -= box_node.scroll_offset_y;
                }
                collect_hoisted_stacking_contexts(
                    child,
                    child_off_x,
                    child_off_y,
                    &mut items,
                    &mut orig_idx_counter,
                );
            }
        }
    } else {
        for (idx, child) in box_node.children.iter().enumerate() {
            let is_item = is_flex_or_grid && !child.is_out_of_flow();
            let is_sc = creates_stacking_context(child, is_item);
            let should_hoist = is_item && child.style.as_ref().and_then(|s| s.z_index).is_some();
            if should_hoist {
                continue;
            }
            items.push(PaintItem {
                child,
                is_flex_or_grid_item: is_item,
                is_sc,
                orig_idx: idx,
                offset_x,
                offset_y,
            });
        }
    }

    items.sort_by(|a, b| {
        let layer_a = paint_layer_of(a.child, a.is_flex_or_grid_item);
        let layer_b = paint_layer_of(b.child, b.is_flex_or_grid_item);
        if layer_a != layer_b {
            return layer_a.cmp(&layer_b);
        }
        if a.is_flex_or_grid_item && b.is_flex_or_grid_item {
            let order_a = a.child.style.as_ref().map(|s| s.order).unwrap_or(0);
            let order_b = b.child.style.as_ref().map(|s| s.order).unwrap_or(0);
            if order_a != order_b {
                return order_a.cmp(&order_b);
            }
        }
        a.orig_idx.cmp(&b.orig_idx)
    });

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

    let traps_absolute = traps_fixed
        || is_root_or_body
        || style.is_some_and(|s| {
            s.position != mango_css::values::Position::Static
                || matches!(
                    s.contain,
                    mango_css::values::Contain::Layout | mango_css::values::Contain::Content
                )
        });

    let mut inline_shifts: std::collections::HashMap<mango_html::dom::NodeId, (f32, f32)> =
        std::collections::HashMap::new();

    let (_max_scroll_x, _max_scroll_y) = if box_node.is_scroll_container() {
        box_node.max_scroll()
    } else {
        (0.0, 0.0)
    };
    let base_child_offset_x = if box_node.is_scroll_container() {
        offset_x - box_node.scroll_offset_x
    } else {
        offset_x
    };
    let base_child_offset_y = if box_node.is_scroll_container() {
        offset_y - box_node.scroll_offset_y
    } else {
        offset_y
    };

    for child in &box_node.children {
        if let Some(cs) = &child.style {
            if cs.position == mango_css::values::Position::Sticky {
                if let Some(nid) = child.node_id {
                    inline_shifts.entry(nid).or_insert_with(|| {
                        let (dx, dy) = sticky_shift(
                            box_node,
                            child,
                            cs,
                            pad_box,
                            active_viewport,
                            active_table,
                            base_child_offset_x,
                            base_child_offset_y,
                            scroll_x,
                            scroll_y,
                            escaped_scroll_x,
                            escaped_scroll_y,
                            sticky_cb,
                        );
                        (dx - escaped_scroll_x, dy - escaped_scroll_y)
                    });
                }
            }
        }
    }

    let should_escape = clips_children || box_node.is_scroll_container();
    let mut normal_children = Vec::new();
    let mut escaped_children = Vec::new();
    for item in items {
        let child = item.child;
        let child_pos = child.style.as_ref().map(|s| s.position);
        let is_fixed_escaped =
            should_escape && child_pos == Some(mango_css::values::Position::Fixed) && !traps_fixed;
        let is_absolute_escaped = should_escape
            && child_pos == Some(mango_css::values::Position::Absolute)
            && !traps_absolute
            && child.containing_inline_id.is_none();
        if is_fixed_escaped || is_absolute_escaped {
            escaped_children.push(item);
        } else {
            normal_children.push(item);
        }
    }

    for item in normal_children {
        let child = item.child;
        let mut child_offset_x = item.offset_x;
        let mut child_offset_y = item.offset_y;

        if box_node.is_scroll_container()
            && (item.offset_x - offset_x).abs() < 1e-4
            && (item.offset_y - offset_y).abs() < 1e-4
        {
            let (max_x, max_y) = box_node.max_scroll();
            let is_rtl_x = box_node.style.as_ref().is_some_and(|s| {
                s.writing_mode == mango_css::values::WritingMode::VerticalRl
                    || s.direction == mango_css::values::Direction::Rtl
            });
            let eff_sx = if is_rtl_x {
                box_node.scroll_offset_x.clamp(-max_x, 0.0)
            } else {
                box_node.scroll_offset_x.clamp(0.0, max_x)
            };
            let is_rtl_y = box_node.style.as_ref().is_some_and(|s| {
                (s.writing_mode == mango_css::values::WritingMode::VerticalRl
                    || s.writing_mode == mango_css::values::WritingMode::VerticalLr)
                    && s.direction == mango_css::values::Direction::Rtl
            });
            let eff_sy = if is_rtl_y {
                box_node.scroll_offset_y.clamp(-max_y, 0.0)
            } else {
                box_node.scroll_offset_y.clamp(0.0, max_y)
            };
            child_offset_x -= eff_sx;
            child_offset_y -= eff_sy;
        }

        let child_escaped_sx = escaped_scroll_x;
        let child_escaped_sy = escaped_scroll_y;

        if let Some(cs) = &child.style {
            if cs.position == mango_css::values::Position::Fixed && !traps_fixed {
                child_offset_x += scroll_x;
                child_offset_y += scroll_y;
            } else if cs.position == mango_css::values::Position::Sticky {
                let (dx, dy) = if let Some(nid) = child.node_id
                    && let Some(&(idx, idy)) = inline_shifts.get(&nid)
                {
                    (idx + child_escaped_sx, idy + child_escaped_sy)
                } else {
                    sticky_shift(
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
                        sticky_cb,
                    )
                };
                child_offset_x += dx - child_escaped_sx;
                child_offset_y += dy - child_escaped_sy;
            }
        }

        if let Some(inline_id) = child.containing_inline_id {
            if let Some(&(idx, idy)) = inline_shifts.get(&inline_id) {
                child_offset_x += idx;
                child_offset_y += idy;
            }
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
            sticky_cb,
            skip_html_bg,
            skip_body_bg,
            item.is_sc,
        );
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
    }

    for item in escaped_children {
        let child = item.child;
        let mut child_offset_x = item.offset_x;
        let mut child_offset_y = item.offset_y;

        let child_pos = child.style.as_ref().map(|s| s.position);
        let is_fixed_escaped =
            child_pos == Some(mango_css::values::Position::Fixed) && !traps_fixed;

        if is_fixed_escaped {
            child_offset_x += scroll_x;
            child_offset_y += scroll_y;
        }

        if let Some(inline_id) = child.containing_inline_id {
            if let Some(&(idx, idy)) = inline_shifts.get(&inline_id) {
                child_offset_x += idx;
                child_offset_y += idy;
            }
        }

        render_layout_box(
            child,
            list,
            opacity,
            child_offset_x,
            child_offset_y,
            scroll_x,
            scroll_y,
            scroll_viewport,
            active_table,
            escaped_scroll_x,
            escaped_scroll_y,
            sticky_cb,
            skip_html_bg,
            skip_body_bg,
            item.is_sc,
        );
    }
}

pub use super::box_rects::collect_box_rects_with_scroll;
