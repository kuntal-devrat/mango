use mango_core::Point;
use mango_css::values::{
    AlignSelf, BoxSizing, Direction, Display, JustifySelf, Length, Position, WritingMode,
};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::container::layout_block_children;
use super::sizing::{calculate_block_height, calculate_block_width};

thread_local! {
    static ANCHOR_REGISTRY: std::cell::RefCell<std::collections::HashMap<String, mango_core::Rect>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Registers an element's border box under its anchor-name ident.
pub fn register_anchor(name: &str, rect: mango_core::Rect) {
    ANCHOR_REGISTRY.with(|reg| {
        reg.borrow_mut().insert(name.to_string(), rect);
    });
}

/// Looks up an anchor box rectangle by name.
pub fn get_anchor(name: &str) -> Option<mango_core::Rect> {
    ANCHOR_REGISTRY.with(|reg| reg.borrow().get(name).copied())
}

/// Clears all registered anchors in the current thread.
pub fn clear_anchors() {
    ANCHOR_REGISTRY.with(|reg| reg.borrow_mut().clear());
}

/// Recursively registers any box with `anchor_name` in the subtree.
pub fn collect_anchors(box_node: &LayoutBox) {
    if let Some(style) = &box_node.style {
        if let Some(ref name) = style.anchor_name {
            register_anchor(name, box_node.dimensions.border_box());
        }
    }
    for child in &box_node.children {
        collect_anchors(child);
    }
}

/// Positions out-of-flow absolute and fixed children against the containing box's padding box.
pub fn layout_positioned_children(box_node: &mut LayoutBox, float_ctx: &mut FloatContext) {
    let cb = box_node.dimensions;
    layout_positioned_children_with_cb(box_node, &cb, float_ctx);
}

/// Positions out-of-flow children, resolving containing block according to CSS 2.1 § 10.1:
/// an element with `position: static` does not establish a containing block for `absolute` children.
pub fn layout_positioned_children_with_cb(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    collect_anchors(box_node);
    let is_positioned_ancestor = box_node
        .style
        .as_ref()
        .map(|s| {
            !matches!(s.position, Position::Static)
                || !s.transform.is_identity()
                || !s.filter.is_empty()
                || !s.backdrop_filter.is_empty()
                || matches!(
                    s.contain,
                    mango_css::values::Contain::Paint
                        | mango_css::values::Contain::Strict
                        | mango_css::values::Contain::Layout
                        | mango_css::values::Contain::Content
                )
                || s.will_change
        })
        .unwrap_or(false);
    let container_dims = if is_positioned_ancestor {
        box_node.dimensions
    } else {
        *containing_block
    };
    let cb_wm = if is_positioned_ancestor {
        box_node
            .style
            .as_ref()
            .map(|s| s.writing_mode)
            .unwrap_or(WritingMode::HorizontalTb)
    } else {
        box_node
            .containing_block_writing_mode
            .unwrap_or(WritingMode::HorizontalTb)
    };
    let box_traps_fixed = box_node
        .style
        .as_ref()
        .map(|s| {
            !s.transform.is_identity()
                || !s.filter.is_empty()
                || !s.backdrop_filter.is_empty()
                || matches!(
                    s.contain,
                    mango_css::values::Contain::Paint
                        | mango_css::values::Contain::Strict
                        | mango_css::values::Contain::Layout
                        | mango_css::values::Contain::Content
                )
                || s.will_change
        })
        .unwrap_or(false);
    for child in box_node.children.iter_mut() {
        if box_traps_fixed {
            child.fixed_containing_block = Some(box_node.dimensions);
        } else if child.fixed_containing_block.is_none() {
            child.fixed_containing_block = box_node.fixed_containing_block;
        }
        let child_pos = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        if matches!(child_pos, Position::Absolute | Position::Fixed) {
            if child.containing_inline_id.is_some() {
                continue;
            }
            child.containing_block_writing_mode = Some(cb_wm);
            let child_custom_cb = if child.containing_inline_id.is_some() {
                child.custom_containing_block
            } else {
                None
            };
            let cb = if let Some(ref custom_cb) = child_custom_cb {
                custom_cb
            } else {
                &container_dims
            };
            layout_positioned_element(child, cb, float_ctx);
        } else {
            child.containing_block_writing_mode = Some(cb_wm);
        }
    }
}

/// Applies relative positioning offsets (`top`, `bottom`, `left`, `right`)
/// to a box without altering surrounding in-flow layout geometry.
pub(crate) fn apply_relative_offset(box_node: &mut LayoutBox, containing_block: &Dimensions) {
    if let Some(style) = &box_node.style {
        let font_size = style.font_size;
        let cw = containing_block.content.width();
        let ch = if containing_block.content.height() > 0.0 {
            containing_block.content.height()
        } else {
            0.0
        };

        let dx = if style.left != Length::Auto {
            style.left.to_px_with_viewport(font_size, 16.0, cw, ch)
        } else if style.right != Length::Auto {
            -style.right.to_px_with_viewport(font_size, 16.0, cw, ch)
        } else {
            0.0
        };

        let dy = if style.top != Length::Auto {
            style.top.to_px_with_viewport(font_size, 16.0, ch, ch)
        } else if style.bottom != Length::Auto {
            -style.bottom.to_px_with_viewport(font_size, 16.0, ch, ch)
        } else {
            0.0
        };

        box_node.dimensions.content.origin.x += dx;
        box_node.dimensions.content.origin.y += dy;
        shift_descendants(box_node, dx, dy);
    }
}

/// Positions an out-of-flow absolute or fixed element against its containing block.
pub(crate) fn layout_positioned_element(
    box_node: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    if std::env::var("WPT_TEST_FILTER")
        .map(|f| f.contains("vrl-rtl-rtl-in-multicol"))
        .unwrap_or(false)
    {
        eprintln!(
            "LAYOUT_POSITIONED_ELEMENT ENTRY: tag={:?} id={:?} inline_id={:?} current_origin={:?} static_pos={:?}",
            box_node.tag_name,
            box_node.node_id,
            box_node.containing_inline_id,
            box_node.dimensions.content.origin,
            box_node.static_position
        );
    }
    if box_node.static_position.is_none() {
        box_node.static_position = Some(box_node.dimensions.content.origin);
    }
    let static_pt = box_node
        .static_position
        .unwrap_or(box_node.dimensions.content.origin);
    let static_x = static_pt.x;
    let static_y = static_pt.y;

    let style = box_node.style.clone().unwrap_or_default();
    let font_size = style.font_size;
    let (vp_w, vp_h) = mango_css::values::get_current_viewport();

    let is_top_layer = (box_node.tag_name.as_deref() == Some("dialog")
        && box_node.get_attribute("open").is_some())
        || box_node.get_attribute("data-mango-popover-open") == Some("true")
        || box_node.get_attribute(":popover-open") == Some("true")
        || box_node.get_attribute("data-mango-popover-open") == Some("pending")
        || box_node.get_attribute(":popover-open") == Some("pending");

    let fixed_cb_storage;
    let custom_cb_copy = if box_node.containing_inline_id.is_some() {
        box_node.custom_containing_block
    } else {
        None
    };
    let fixed_cb_copy = box_node.fixed_containing_block;
    let cb = if let Some(ref custom_cb) = custom_cb_copy {
        custom_cb
    } else if style.position == Position::Fixed {
        if let Some(ref fcb) = fixed_cb_copy {
            fcb
        } else {
            fixed_cb_storage = Dimensions::new(mango_core::Rect::new(0.0, 0.0, vp_w, vp_h));
            &fixed_cb_storage
        }
    } else if is_top_layer {
        fixed_cb_storage = Dimensions::new(mango_core::Rect::new(0.0, 0.0, vp_w, vp_h));
        &fixed_cb_storage
    } else {
        containing_block
    };

    let pad_box = if let Some(ref custom_cb) = custom_cb_copy {
        custom_cb.padding_box()
    } else if (style.position == Position::Fixed && fixed_cb_copy.is_none()) || is_top_layer {
        mango_core::Rect::new(0.0, 0.0, vp_w, vp_h)
    } else {
        cb.padding_box()
    };

    let abspos_cb = Dimensions::new(pad_box);
    let sizing_cb = if style.position == Position::Absolute {
        &abspos_cb
    } else {
        cb
    };

    calculate_block_width(box_node, sizing_cb);
    if style.height != Length::Auto {
        calculate_block_height(box_node, sizing_cb);
    }

    let pad_w = pad_box.width();
    let pad_h = pad_box.height();

    let resolve_anchor_coord =
        |len: &Length, is_x: bool, default_anchor: Option<&String>| -> Option<f32> {
            if let Length::Anchor {
                name_id,
                side,
                offset,
            } = len
            {
                let name = name_id
                    .and_then(mango_css::values::get_interned_anchor_name)
                    .or_else(|| default_anchor.cloned())?;
                let rect = get_anchor(&name)?;
                let coord =
                    match side {
                        mango_css::values::AnchorSide::Left
                        | mango_css::values::AnchorSide::Start => rect.left(),
                        mango_css::values::AnchorSide::Right
                        | mango_css::values::AnchorSide::End => rect.right(),
                        mango_css::values::AnchorSide::Center => {
                            if is_x {
                                rect.origin.x + rect.width() * 0.5
                            } else {
                                rect.origin.y + rect.height() * 0.5
                            }
                        }
                        mango_css::values::AnchorSide::Top => rect.top(),
                        mango_css::values::AnchorSide::Bottom => rect.bottom(),
                    };
                Some(coord + offset)
            } else {
                None
            }
        };

    let calc_left_px = |l: &Length| -> f32 {
        if let Some(target_x) = resolve_anchor_coord(l, true, style.position_anchor.as_ref()) {
            target_x - pad_box.x()
        } else {
            l.to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
        }
    };
    let calc_right_px = |l: &Length| -> f32 {
        if let Some(target_x) = resolve_anchor_coord(l, true, style.position_anchor.as_ref()) {
            pad_box.right() - target_x
        } else {
            l.to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
        }
    };
    let calc_top_px = |l: &Length| -> f32 {
        if let Some(target_y) = resolve_anchor_coord(l, false, style.position_anchor.as_ref()) {
            target_y - pad_box.y()
        } else {
            l.to_px_with_viewport(font_size, 16.0, pad_h, pad_h)
        }
    };
    let calc_bottom_px = |l: &Length| -> f32 {
        if let Some(target_y) = resolve_anchor_coord(l, false, style.position_anchor.as_ref()) {
            pad_box.bottom() - target_y
        } else {
            l.to_px_with_viewport(font_size, 16.0, pad_h, pad_h)
        }
    };

    let cb_wm = if style.position == Position::Fixed && custom_cb_copy.is_none() && !is_top_layer {
        WritingMode::HorizontalTb
    } else {
        box_node
            .containing_block_writing_mode
            .unwrap_or(WritingMode::HorizontalTb)
    };
    let is_table =
        matches!(style.display, Display::Table) || box_node.tag_name.as_deref() == Some("table");
    let (allows_stretch_w, allows_stretch_h) = if cb_wm.is_vertical() {
        (
            !is_table && matches!(style.align_self, AlignSelf::Auto | AlignSelf::Stretch),
            !is_table && matches!(style.justify_self, JustifySelf::Auto | JustifySelf::Stretch),
        )
    } else {
        (
            !is_table && matches!(style.justify_self, JustifySelf::Auto | JustifySelf::Stretch),
            !is_table && matches!(style.align_self, AlignSelf::Auto | AlignSelf::Stretch),
        )
    };

    // 1. Horizontal positioning (CSS 2.1 § 10.3.7)
    let is_left_auto = style.left == Length::Auto;
    let is_right_auto = style.right == Length::Auto;
    let is_margin_l_auto = style.margin_left == Length::Auto;
    let is_margin_r_auto = style.margin_right == Length::Auto;

    let margin_l = if is_margin_l_auto {
        0.0
    } else {
        style
            .margin_left
            .to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
    };
    let margin_r = if is_margin_r_auto {
        0.0
    } else {
        style
            .margin_right
            .to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
    };

    // If both left and right are specified and width is auto, width expands to fill the span
    let has_left_right_auto_width =
        !is_left_auto && !is_right_auto && style.width == Length::Auto && allows_stretch_w;
    let mut stretched_w = None;
    if has_left_right_auto_width {
        let non_content_h = box_node.dimensions.padding.left
            + box_node.dimensions.padding.right
            + box_node.dimensions.border.left
            + box_node.dimensions.border.right;
        let left_px = calc_left_px(&style.left);
        let right_px = calc_right_px(&style.right);
        let tentative_w =
            (pad_w - left_px - right_px - margin_l - margin_r - non_content_h).max(0.0);
        let mut clamped_w = tentative_w;
        if style.max_width != Length::Auto {
            let max_w = style
                .max_width
                .to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
            let max_content_w = if style.box_sizing == BoxSizing::BorderBox {
                (max_w - non_content_h).max(0.0)
            } else {
                max_w
            };
            clamped_w = clamped_w.min(max_content_w);
        }
        if style.min_width != Length::Auto {
            let min_w = style
                .min_width
                .to_px_with_viewport(font_size, 16.0, pad_w, pad_h);
            let min_content_w = if style.box_sizing == BoxSizing::BorderBox {
                (min_w - non_content_h).max(0.0)
            } else {
                min_w
            };
            clamped_w = clamped_w.max(min_content_w);
        }
        box_node.dimensions.content.size.width = clamped_w;
        stretched_w = Some(clamped_w);
    }

    // 2. Vertical positioning (CSS 2.1 § 10.6.4)
    let is_top_auto = style.top == Length::Auto;
    let is_bottom_auto = style.bottom == Length::Auto;
    let is_margin_t_auto = style.margin_top == Length::Auto;
    let is_margin_b_auto = style.margin_bottom == Length::Auto;

    // In CSS 2.1 §8.3, percentage margins are relative to containing block WIDTH (pad_w)
    let margin_t = if is_margin_t_auto {
        0.0
    } else {
        style
            .margin_top
            .to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
    };
    let margin_b = if is_margin_b_auto {
        0.0
    } else {
        style
            .margin_bottom
            .to_px_with_viewport(font_size, 16.0, pad_w, pad_h)
    };

    // If both top and bottom are specified and height is auto, height expands to fill the span
    let has_top_bottom_auto_height =
        !is_top_auto && !is_bottom_auto && style.height == Length::Auto && allows_stretch_h;
    let mut stretched_h = None;
    if has_top_bottom_auto_height {
        let non_content_v = box_node.dimensions.padding.top
            + box_node.dimensions.padding.bottom
            + box_node.dimensions.border.top
            + box_node.dimensions.border.bottom;
        let top_px = calc_top_px(&style.top);
        let bottom_px = calc_bottom_px(&style.bottom);
        let tentative_h =
            (pad_h - top_px - bottom_px - margin_t - margin_b - non_content_v).max(0.0);
        let mut clamped_h = tentative_h;
        if style.max_height != Length::Auto {
            let max_h = style
                .max_height
                .to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
            let max_content_h = if style.box_sizing == BoxSizing::BorderBox {
                (max_h - non_content_v).max(0.0)
            } else {
                max_h
            };
            clamped_h = clamped_h.min(max_content_h);
        }
        if style.min_height != Length::Auto {
            let min_h = style
                .min_height
                .to_px_with_viewport(font_size, 16.0, pad_h, pad_h);
            let min_content_h = if style.box_sizing == BoxSizing::BorderBox {
                (min_h - non_content_v).max(0.0)
            } else {
                min_h
            };
            clamped_h = clamped_h.max(min_content_h);
        }
        box_node.dimensions.content.size.height = clamped_h;
        stretched_h = Some(clamped_h);
    }

    let border_box_w = box_node.dimensions.border_box().width();
    let border_box_h = box_node.dimensions.border_box().height();

    let border_box_x = if !is_left_auto && !is_right_auto {
        let left_px = calc_left_px(&style.left);
        let right_px = calc_right_px(&style.right);
        if is_margin_l_auto && is_margin_r_auto {
            let free_space = pad_w - left_px - right_px - border_box_w;
            let m = (free_space / 2.0).max(0.0);
            box_node.dimensions.margin.left = m;
            box_node.dimensions.margin.right = m;
            pad_box.x() + left_px + m
        } else if is_margin_l_auto {
            let m = (pad_w - left_px - right_px - border_box_w - margin_r).max(0.0);
            box_node.dimensions.margin.left = m;
            box_node.dimensions.margin.right = margin_r;
            pad_box.x() + left_px + m
        } else if is_margin_r_auto {
            box_node.dimensions.margin.left = margin_l;
            box_node.dimensions.margin.right =
                (pad_w - left_px - right_px - border_box_w - margin_l).max(0.0);
            pad_box.x() + left_px + margin_l
        } else {
            box_node.dimensions.margin.left = margin_l;
            box_node.dimensions.margin.right = margin_r;
            pad_box.x() + left_px + margin_l
        }
    } else if !is_left_auto {
        let left_px = calc_left_px(&style.left);
        box_node.dimensions.margin.left = margin_l;
        box_node.dimensions.margin.right = margin_r;
        pad_box.x() + left_px + margin_l
    } else if !is_right_auto {
        let right_px = calc_right_px(&style.right);
        box_node.dimensions.margin.left = margin_l;
        box_node.dimensions.margin.right = margin_r;
        pad_box.right() - right_px - margin_r - border_box_w
    } else {
        box_node.dimensions.margin.left = margin_l;
        box_node.dimensions.margin.right = margin_r;
        let base_x = if is_top_layer {
            match style.writing_mode {
                WritingMode::HorizontalTb => {
                    if style.direction == Direction::Rtl {
                        pad_box.right() - border_box_w
                    } else {
                        pad_box.left()
                    }
                }
                WritingMode::VerticalRl | WritingMode::SidewaysRl => pad_box.right() - border_box_w,
                WritingMode::VerticalLr | WritingMode::SidewaysLr => pad_box.left(),
            }
        } else if box_node.containing_inline_id.is_some()
            || static_x != 0.0
            || containing_block.content.x() == 0.0
        {
            static_x
        } else {
            containing_block.content.x()
        };
        base_x + margin_l
    };

    let is_bottom_anchored_auto_height =
        is_top_auto && !is_bottom_auto && style.height == Length::Auto;

    let is_bottom_anchored_static_y = box_node.is_static_bottom_anchored;

    let border_box_y = if !is_top_auto && !is_bottom_auto {
        let top_px = calc_top_px(&style.top);
        let bottom_px = calc_bottom_px(&style.bottom);
        if is_margin_t_auto && is_margin_b_auto {
            let free_space = pad_h - top_px - bottom_px - border_box_h;
            let m = (free_space / 2.0).max(0.0);
            box_node.dimensions.margin.top = m;
            box_node.dimensions.margin.bottom = m;
            pad_box.y() + top_px + m
        } else if is_margin_t_auto {
            let m = (pad_h - top_px - bottom_px - border_box_h - margin_b).max(0.0);
            box_node.dimensions.margin.top = m;
            box_node.dimensions.margin.bottom = margin_b;
            pad_box.y() + top_px + m
        } else if is_margin_b_auto {
            box_node.dimensions.margin.top = margin_t;
            box_node.dimensions.margin.bottom =
                (pad_h - top_px - bottom_px - border_box_h - margin_t).max(0.0);
            pad_box.y() + top_px + margin_t
        } else {
            box_node.dimensions.margin.top = margin_t;
            box_node.dimensions.margin.bottom = margin_b;
            pad_box.y() + top_px + margin_t
        }
    } else if !is_top_auto {
        let top_px = calc_top_px(&style.top);
        box_node.dimensions.margin.top = margin_t;
        box_node.dimensions.margin.bottom = margin_b;
        pad_box.y() + top_px + margin_t
    } else if !is_bottom_auto {
        let bottom_px = calc_bottom_px(&style.bottom);
        box_node.dimensions.margin.top = margin_t;
        box_node.dimensions.margin.bottom = margin_b;
        if is_bottom_anchored_auto_height {
            let base_y = if static_y >= containing_block.content.y() {
                static_y
            } else {
                containing_block.content.y()
            };
            base_y + margin_t
        } else {
            pad_box.bottom() - bottom_px - margin_b - border_box_h
        }
    } else {
        box_node.dimensions.margin.top = margin_t;
        box_node.dimensions.margin.bottom = margin_b;
        let base_y = if is_top_layer {
            match style.writing_mode {
                WritingMode::HorizontalTb => pad_box.top(),
                WritingMode::VerticalLr | WritingMode::VerticalRl => {
                    if style.direction == Direction::Rtl {
                        pad_box.bottom() - border_box_h
                    } else {
                        pad_box.top()
                    }
                }
                WritingMode::SidewaysLr => {
                    if style.direction == Direction::Ltr {
                        pad_box.bottom() - border_box_h
                    } else {
                        pad_box.top()
                    }
                }
                WritingMode::SidewaysRl => {
                    if style.direction == Direction::Rtl {
                        pad_box.bottom() - border_box_h
                    } else {
                        pad_box.top()
                    }
                }
            }
        } else if box_node.containing_inline_id.is_some()
            || static_y != 0.0
            || containing_block.content.y() == 0.0
        {
            if style.writing_mode.is_vertical() && is_bottom_anchored_static_y {
                static_y - border_box_h
            } else {
                static_y
            }
        } else {
            containing_block.content.y()
        };
        base_y + margin_t
    };

    let mut final_box_x = border_box_x;
    let mut final_box_y = border_box_y;

    // CSS Anchor Positioning & @position-try fallbacks
    let overflows = final_box_y + border_box_h > pad_box.bottom()
        || final_box_y < pad_box.top()
        || final_box_x + border_box_w > pad_box.right()
        || final_box_x < pad_box.x();

    if overflows && !style.position_try_fallbacks.is_empty() {
        for fallback in &style.position_try_fallbacks {
            let fb_clean = fallback.trim();
            if fb_clean.eq_ignore_ascii_case("flip-block") {
                if let Length::Anchor {
                    name_id,
                    side,
                    offset,
                } = style.top
                {
                    let flipped_side = match side {
                        mango_css::values::AnchorSide::Bottom => mango_css::values::AnchorSide::Top,
                        mango_css::values::AnchorSide::Top => mango_css::values::AnchorSide::Bottom,
                        s => s,
                    };
                    if let Some(target_y) = resolve_anchor_coord(
                        &Length::Anchor {
                            name_id,
                            side: flipped_side,
                            offset,
                        },
                        false,
                        style.position_anchor.as_ref(),
                    ) {
                        let test_y = if side == mango_css::values::AnchorSide::Bottom {
                            target_y - margin_b - border_box_h
                        } else {
                            target_y + margin_t
                        };
                        if test_y >= pad_box.top() && test_y + border_box_h <= pad_box.bottom() {
                            final_box_y = test_y;
                            break;
                        }
                    }
                }
            } else if fb_clean.eq_ignore_ascii_case("flip-inline") {
                if let Length::Anchor {
                    name_id,
                    side,
                    offset,
                } = style.left
                {
                    let flipped_side = match side {
                        mango_css::values::AnchorSide::Right => mango_css::values::AnchorSide::Left,
                        mango_css::values::AnchorSide::Left => mango_css::values::AnchorSide::Right,
                        s => s,
                    };
                    if let Some(target_x) = resolve_anchor_coord(
                        &Length::Anchor {
                            name_id,
                            side: flipped_side,
                            offset,
                        },
                        true,
                        style.position_anchor.as_ref(),
                    ) {
                        let test_x = if side == mango_css::values::AnchorSide::Right {
                            target_x - margin_r - border_box_w
                        } else {
                            target_x + margin_l
                        };
                        if test_x >= pad_box.x() && test_x + border_box_w <= pad_box.right() {
                            final_box_x = test_x;
                            break;
                        }
                    }
                }
            }
        }
    }

    // Set content box origin from border box origin before laying out children
    let content_x =
        final_box_x + box_node.dimensions.border.left + box_node.dimensions.padding.left;
    let content_y = final_box_y + box_node.dimensions.border.top + box_node.dimensions.padding.top;
    box_node.dimensions.content.origin = Point::new(content_x, content_y);

    // A positioned element may have been laid out once at its static position
    // before its containing block and offsets were resolved. Rebuild its
    // formatting descendants from the preserved source children so stale
    // text/inline boxes are not painted at both the static and final origins.
    if box_node.raw_children.is_none() && !box_node.children.is_empty() {
        box_node.raw_children = Some(box_node.children.clone());
    }
    if let Some(raw_children) = box_node.raw_children.clone() {
        box_node.children = raw_children;
    }

    let is_flex_or_grid = matches!(
        style.display,
        Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
    );

    if style.display == Display::Table || box_node.tag_name.as_deref() == Some("table") {
        crate::table_flow::layout_table(box_node, cb, float_ctx);
    } else if matches!(style.display, Display::Flex | Display::InlineFlex) {
        crate::flex_flow::layout_flex(box_node, cb, float_ctx);
        if let Some(sh) = stretched_h {
            box_node.dimensions.content.size.height = sh;
        }
        if let Some(sw) = stretched_w {
            box_node.dimensions.content.size.width = sw;
        }
    } else if style.display == Display::Grid || style.display == Display::InlineGrid {
        crate::grid_flow::layout_grid(box_node, cb, float_ctx);
        if let Some(sh) = stretched_h {
            box_node.dimensions.content.size.height = sh;
        }
        if let Some(sw) = stretched_w {
            box_node.dimensions.content.size.width = sw;
        }
    } else {
        layout_block_children(box_node, cb, float_ctx);
        if let Some(sh) = stretched_h {
            box_node.dimensions.content.size.height = sh;
        } else if !has_top_bottom_auto_height {
            calculate_block_height(box_node, cb);
        }
        if let Some(sw) = stretched_w {
            box_node.dimensions.content.size.width = sw;
        }
        center_button_children(box_node);
    }

    if is_bottom_anchored_auto_height {
        let bottom_px = calc_bottom_px(&style.bottom);
        let actual_border_box_h = box_node.dimensions.border_box().height();
        let actual_box_y = pad_box.bottom() - bottom_px - margin_b - actual_border_box_h;
        let actual_content_y =
            actual_box_y + box_node.dimensions.border.top + box_node.dimensions.padding.top;
        let dy = actual_content_y - box_node.dimensions.content.y();
        box_node.dimensions.content.origin.y = actual_content_y;
        shift_descendants(box_node, 0.0, dy);
    }

    if !is_top_auto && !is_bottom_auto && !has_top_bottom_auto_height {
        let top_px = calc_top_px(&style.top);
        let bottom_px = calc_bottom_px(&style.bottom);
        let actual_border_box_h = box_node.dimensions.border_box().height();
        let actual_box_y = if is_margin_t_auto && is_margin_b_auto {
            let free_space = (pad_h - top_px - bottom_px - actual_border_box_h).max(0.0);
            let m = free_space / 2.0;
            box_node.dimensions.margin.top = m;
            box_node.dimensions.margin.bottom = m;
            pad_box.y() + top_px + m
        } else if is_margin_t_auto {
            let m = (pad_h - top_px - bottom_px - actual_border_box_h - margin_b).max(0.0);
            box_node.dimensions.margin.top = m;
            box_node.dimensions.margin.bottom = margin_b;
            pad_box.y() + top_px + m
        } else if is_margin_b_auto {
            let m = (pad_h - top_px - bottom_px - actual_border_box_h - margin_t).max(0.0);
            box_node.dimensions.margin.top = margin_t;
            box_node.dimensions.margin.bottom = m;
            pad_box.y() + top_px + margin_t
        } else {
            box_node.dimensions.margin.top = margin_t;
            box_node.dimensions.margin.bottom = margin_b;
            pad_box.y() + top_px + margin_t
        };
        let actual_content_y =
            actual_box_y + box_node.dimensions.border.top + box_node.dimensions.padding.top;
        let dy = actual_content_y - box_node.dimensions.content.y();
        if dy.abs() > 0.001 {
            box_node.dimensions.content.origin.y = actual_content_y;
            shift_descendants(box_node, 0.0, dy);
        }
    }

    if !is_left_auto && !is_right_auto && !has_left_right_auto_width {
        let left_px = calc_left_px(&style.left);
        let right_px = calc_right_px(&style.right);
        let actual_border_box_w = box_node.dimensions.border_box().width();
        let actual_box_x = if is_margin_l_auto && is_margin_r_auto {
            let free_space = (pad_w - left_px - right_px - actual_border_box_w).max(0.0);
            let m = free_space / 2.0;
            box_node.dimensions.margin.left = m;
            box_node.dimensions.margin.right = m;
            pad_box.x() + left_px + m
        } else if is_margin_l_auto {
            let m = (pad_w - left_px - right_px - actual_border_box_w - margin_r).max(0.0);
            box_node.dimensions.margin.left = m;
            box_node.dimensions.margin.right = margin_r;
            pad_box.x() + left_px + m
        } else if is_margin_r_auto {
            let m = (pad_w - left_px - right_px - actual_border_box_w - margin_l).max(0.0);
            box_node.dimensions.margin.left = margin_l;
            box_node.dimensions.margin.right = m;
            pad_box.x() + left_px + margin_l
        } else {
            box_node.dimensions.margin.left = margin_l;
            box_node.dimensions.margin.right = margin_r;
            pad_box.x() + left_px + margin_l
        };
        let actual_content_x =
            actual_box_x + box_node.dimensions.border.left + box_node.dimensions.padding.left;
        let dx = actual_content_x - box_node.dimensions.content.x();
        if dx.abs() > 0.001 {
            box_node.dimensions.content.origin.x = actual_content_x;
            shift_descendants(box_node, dx, 0.0);
        }
    }

    let is_static_y = is_top_auto && is_bottom_auto;
    if is_static_y
        && !is_top_layer
        && style.writing_mode.is_vertical()
        && is_bottom_anchored_static_y
    {
        let actual_border_box_h = box_node.dimensions.border_box().height();
        let actual_box_y = static_y - actual_border_box_h + margin_t;
        let actual_content_y =
            actual_box_y + box_node.dimensions.border.top + box_node.dimensions.padding.top;
        let dy = actual_content_y - box_node.dimensions.content.y();
        if dy.abs() > 0.001 {
            box_node.dimensions.content.origin.y = actual_content_y;
            shift_descendants(box_node, 0.0, dy);
        }
    }

    if let Some(name) = &style.anchor_name {
        register_anchor(name, box_node.dimensions.border_box());
    }

    // Flex and Grid already lay out their out-of-flow positioned children at the end of their pass
    if !is_flex_or_grid {
        layout_positioned_children(box_node, float_ctx);
    }
}

pub(crate) fn shift_descendants(box_node: &mut LayoutBox, dx: f32, dy: f32) {
    for child in &mut box_node.children {
        let child_pos = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        if child_pos == Position::Fixed {
            let (shift_x, shift_y) = if let Some(style) = &child.style {
                let sx = if style.left == Length::Auto && style.right == Length::Auto {
                    dx
                } else {
                    0.0
                };
                let sy = if style.top == Length::Auto && style.bottom == Length::Auto {
                    dy
                } else {
                    0.0
                };
                (sx, sy)
            } else {
                (dx, dy)
            };
            child.dimensions.content.origin.x += shift_x;
            child.dimensions.content.origin.y += shift_y;
            if let Some(ref mut pt) = child.static_position {
                pt.x += shift_x;
                pt.y += shift_y;
            }
            shift_descendants(child, shift_x, shift_y);
            continue;
        }
        child.dimensions.content.origin.x += dx;
        child.dimensions.content.origin.y += dy;
        if let Some(ref mut pt) = child.static_position {
            pt.x += dx;
            pt.y += dy;
        }
        shift_descendants(child, dx, dy);
    }
}

pub(crate) fn center_button_children(box_node: &mut LayoutBox) {
    if box_node.tag_name.as_deref() != Some("button") || box_node.children.is_empty() {
        return;
    }
    let button_h = box_node.dimensions.content.height();
    if button_h <= 0.0 {
        return;
    }
    let content_top = box_node.dimensions.content.y();
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for child in &box_node.children {
        let b = child.dimensions.margin_box();
        min_y = min_y.min(b.y());
        max_y = max_y.max(b.bottom());
    }
    if min_y < max_y && (max_y - min_y) < button_h {
        let children_h = max_y - min_y;
        let target_y = content_top + (button_h - children_h) / 2.0;
        let delta_y = target_y - min_y;
        if delta_y.abs() > 0.5 {
            for child in &mut box_node.children {
                child.dimensions.content.origin.y += delta_y;
                shift_descendants(child, 0.0, delta_y);
            }
        }
    }
}
