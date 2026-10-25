use mango_core::Rect;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::stacking::{StickyContainingBlock, sticky_shift};

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
        sticky_cb: Option<StickyContainingBlock>,
        out: &mut std::collections::HashMap<u32, [f32; 4]>,
    ) {
        if let Some(node_id) = node.node_id {
            let border = node.dimensions.border_box();
            let bx = border.x() + offset_x;
            let by = border.y() + offset_y;
            let bw = border.width();
            let bh = border.height();

            let rect = if let Some(style) = &node.style
                && !style.transform.is_identity()
            {
                let [a, b, c, d, e, f] = style.transform.to_matrix_with_size(bw, bh);
                let cx = bx + bw / 2.0;
                let cy = by + bh / 2.0;
                let corners = [
                    (-bw / 2.0, -bh / 2.0),
                    (bw / 2.0, -bh / 2.0),
                    (bw / 2.0, bh / 2.0),
                    (-bw / 2.0, bh / 2.0),
                ];
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for (dx, dy) in corners {
                    let px = cx + a * dx + c * dy + e;
                    let py = cy + b * dx + d * dy + f;
                    min_x = min_x.min(px);
                    max_x = max_x.max(px);
                    min_y = min_y.min(py);
                    max_y = max_y.max(py);
                }
                [min_x, min_y, max_x - min_x, max_y - min_y]
            } else {
                [bx, by, bw, bh]
            };
            out.insert(node_id.raw(), rect);
        }

        if matches!(node.box_type, BoxType::IFrame { .. }) {
            return;
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

        let is_root_or_body = node
            .tag_name
            .as_deref()
            .is_some_and(|t| t == "html" || t == "body");

        let style = node.style.as_deref();
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

        let (_max_scroll_x, _max_scroll_y) = if node.is_scroll_container() {
            node.max_scroll()
        } else {
            (0.0, 0.0)
        };
        let base_child_offset_x = if node.is_scroll_container() {
            offset_x - node.scroll_offset_x
        } else {
            offset_x
        };
        let base_child_offset_y = if node.is_scroll_container() {
            offset_y - node.scroll_offset_y
        } else {
            offset_y
        };

        for child in &node.children {
            if let Some(cs) = &child.style {
                if cs.position == mango_css::values::Position::Sticky {
                    if let Some(nid) = child.node_id {
                        inline_shifts.entry(nid).or_insert_with(|| {
                            let (dx, dy) = sticky_shift(
                                node,
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

        for child in &node.children {
            let mut child_offset_x = offset_x;
            let mut child_offset_y = offset_y;

            let child_pos = child.style.as_ref().map(|s| s.position);
            let _is_in_flow = child_pos.unwrap_or(mango_css::values::Position::Static)
                == mango_css::values::Position::Static;

            if node.is_scroll_container() {
                child_offset_x -= node.scroll_offset_x;
                child_offset_y -= node.scroll_offset_y;
            }

            let is_fixed_escaped =
                child_pos == Some(mango_css::values::Position::Fixed) && !traps_fixed;
            let is_absolute_escaped = child_pos == Some(mango_css::values::Position::Absolute)
                && !traps_absolute
                && child.containing_inline_id.is_none();
            let is_child_escaped = is_fixed_escaped || is_absolute_escaped;

            let child_escaped_sx = escaped_scroll_x;
            let child_escaped_sy = escaped_scroll_y;

            if is_child_escaped {
                if node.is_scroll_container() {
                    child_offset_x += node.scroll_offset_x;
                    child_offset_y += node.scroll_offset_y;
                }
                if is_fixed_escaped {
                    child_offset_x += scroll_x;
                    child_offset_y += scroll_y;
                }
            } else if let Some(cs) = &child.style {
                if cs.position == mango_css::values::Position::Sticky {
                    let (dx, dy) = if let Some(nid) = child.node_id
                        && let Some(&(idx, idy)) = inline_shifts.get(&nid)
                    {
                        (idx + child_escaped_sx, idy + child_escaped_sy)
                    } else {
                        sticky_shift(
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

            let child_viewport = if is_child_escaped {
                scroll_viewport
            } else {
                active_viewport
            };

            let is_non_atomic_inline =
                matches!(child.box_type, BoxType::InlineNode | BoxType::TextNode(_))
                    || child
                        .style
                        .as_ref()
                        .is_some_and(|s| s.display == mango_css::values::Display::Inline);
            let current_sticky_cb = if child.is_anonymous() || is_non_atomic_inline {
                sticky_cb
            } else {
                let child_pad = child.dimensions.padding_box();
                let child_pad_box = Rect::new(
                    child_pad.x() + child_offset_x,
                    child_pad.y() + child_offset_y,
                    child_pad.width(),
                    child_pad.height(),
                );
                let (scroll_right, scroll_bottom) = if child.is_scroll_container() {
                    let (sw, sh) = child.scrollable_extent();
                    (
                        Some(child_pad_box.x() + sw - child.dimensions.padding.right),
                        Some(child_pad_box.y() + sh - child.dimensions.padding.bottom),
                    )
                } else {
                    (None, None)
                };
                Some(StickyContainingBlock {
                    rect: child_pad_box,
                    scroll_right,
                    scroll_bottom,
                })
            };

            walk(
                child,
                child_offset_x,
                child_offset_y,
                child_viewport,
                active_table,
                scroll_x,
                scroll_y,
                child_escaped_sx,
                child_escaped_sy,
                current_sticky_cb,
                out,
            );
        }
    }

    out.clear();
    walk(
        root, 0.0, 0.0, None, None, scroll_x, scroll_y, 0.0, 0.0, None, out,
    );
}
