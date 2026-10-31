use mango_core::Point;
use mango_css::values::WritingMode;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::float::FloatContext;

use super::atom::{InlineAtom, InlineAtomKind};
use super::collector::collect_inline_atoms;
use super::metrics::measure_text_width_with_style_and_spacing;
use super::shaping::{font_family_for, font_weight_for, used_line_height};

pub(crate) fn layout_vertical_inline_children(
    container: &mut LayoutBox,
    _float_ctx: &mut FloatContext,
    writing_mode: WritingMode,
) -> f32 {
    let container_x = container.dimensions.content.x();
    let container_y = container.dimensions.content.y();
    let container_width = container.dimensions.content.width();
    let container_height = if container.dimensions.content.height() > 0.0 {
        container.dimensions.content.height()
    } else if let Some(style) = &container.style {
        let font_size = style.font_size;
        let root_font_size = style.root_font_size;
        let (_, vp_h) = mango_css::get_current_viewport();
        let h = style
            .height
            .to_px_with_viewport(font_size, root_font_size, vp_h, vp_h);
        if h > 0.0 {
            h
        } else {
            let max_h = style
                .max_height
                .to_px_with_viewport(font_size, root_font_size, vp_h, vp_h);
            if max_h > 0.0 && style.max_height != mango_css::values::Length::Auto {
                max_h
            } else {
                400.0
            }
        }
    } else {
        400.0
    };

    let text_align = container
        .style
        .as_ref()
        .map(|s| match s.text_align {
            mango_css::values::TextAlign::Start => {
                if s.direction == mango_css::values::Direction::Rtl {
                    mango_css::values::TextAlign::Right
                } else {
                    mango_css::values::TextAlign::Left
                }
            }
            mango_css::values::TextAlign::End => {
                if s.direction == mango_css::values::Direction::Rtl {
                    mango_css::values::TextAlign::Left
                } else {
                    mango_css::values::TextAlign::Right
                }
            }
            other => other,
        })
        .unwrap_or(mango_css::values::TextAlign::Left);

    let col_count_opt = container.style.as_ref().and_then(|s| s.column_count);
    let col_width_opt = container
        .style
        .as_ref()
        .and_then(|s| s.column_width.as_ref());
    let font_size = container
        .style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or(16.0);
    let gap = container
        .style
        .as_ref()
        .map(|s| s.column_gap.to_px(font_size, 16.0, container_height))
        .unwrap_or(0.0);
    let resolved_cols = match (col_count_opt, col_width_opt) {
        (Some(c), Some(w_len)) => {
            let w_px = w_len.to_px(font_size, 16.0, container_height);
            if w_px > 0.0 {
                let max_cols = ((container_height + gap) / (w_px + gap)).floor().max(1.0) as usize;
                c.min(max_cols).max(1)
            } else {
                c.max(1)
            }
        }
        (Some(c), None) => c.max(1),
        (None, Some(w_len)) => {
            let w_px = w_len.to_px(font_size, 16.0, container_height);
            if w_px > 0.0 {
                ((container_height + gap) / (w_px + gap)).floor().max(1.0) as usize
            } else {
                1
            }
        }
        (None, None) => 1,
    };

    let col_inline_size = if resolved_cols > 1 {
        ((container_height - (resolved_cols - 1) as f32 * gap) / resolved_cols as f32).max(1.0)
    } else {
        container_height
    };

    let max_col_h = col_inline_size;

    if container.raw_children.is_none() {
        container.raw_children = Some(std::mem::take(&mut container.children));
    }

    let initial_bidi_override = container.style.as_ref().and_then(|s| match s.unicode_bidi {
        mango_css::values::UnicodeBidi::BidiOverride
        | mango_css::values::UnicodeBidi::IsolateOverride => Some(s.unicode_bidi),
        _ => None,
    });
    let mut atoms: Vec<InlineAtom> = Vec::new();
    if let Some(source_children) = container.raw_children.as_deref() {
        for child in source_children {
            collect_inline_atoms(
                child,
                &mut atoms,
                child.link_target.as_deref(),
                child.link_frame.as_deref(),
                container_width,
                max_col_h,
                initial_bidi_override,
            );
        }
    }

    if atoms.is_empty() {
        let mut oof_children = Vec::new();
        if let Some(source_children) = container.raw_children.as_deref() {
            for sc in source_children {
                let is_oof = sc.style.as_ref().is_some_and(|s| {
                    matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
                });
                if is_oof {
                    oof_children.push(sc.clone());
                }
            }
        }
        container.children = oof_children;
        return 0.0;
    }

    struct VertLine {
        boxes: Vec<LayoutBox>,
        height: f32,
    }

    let mut lines: Vec<VertLine> = Vec::new();
    let mut cur_line_boxes: Vec<LayoutBox> = Vec::new();
    let mut cur_y = container_y;
    let (family, weight) = if let Some(s) = container.style.as_ref() {
        (font_family_for(s), font_weight_for(s))
    } else {
        (
            mango_render::FontFamily::Serif,
            mango_render::FontWeight::Regular,
        )
    };
    let base_col_w = container
        .style
        .as_ref()
        .map_or(16.0, |s| used_line_height(s, family, weight));

    let strip_trailing_space = |boxes: &mut Vec<LayoutBox>, cur_y: &mut f32| {
        let mut i = boxes.len();
        while i > 0 {
            let idx = i - 1;
            let is_oof = matches!(
                boxes[idx].style.as_ref().map(|s| s.position),
                Some(mango_css::values::Position::Absolute | mango_css::values::Position::Fixed)
            );
            if is_oof {
                i -= 1;
                continue;
            }
            if let BoxType::TextNode(ref s) = boxes[idx].box_type {
                if s == " " {
                    let removed_h = boxes[idx].dimensions.content.size.height;
                    *cur_y -= removed_h;
                    boxes.remove(idx);
                    for oof_b in &mut boxes[idx..] {
                        oof_b.dimensions.content.origin.y -= removed_h;
                    }
                    i = idx;
                    continue;
                }
            }
            break;
        }
    };

    let is_nowrap = container.style.as_ref().is_some_and(|s| {
        matches!(
            s.white_space,
            mango_css::values::WhiteSpace::Nowrap | mango_css::values::WhiteSpace::Pre
        )
    });

    for atom in atoms {
        if matches!(atom.kind, InlineAtomKind::LineBreak { .. }) {
            strip_trailing_space(&mut cur_line_boxes, &mut cur_y);
            lines.push(VertLine {
                boxes: std::mem::take(&mut cur_line_boxes),
                height: cur_y - container_y,
            });
            cur_y = container_y;
            continue;
        }

        match atom.kind {
            InlineAtomKind::Text { text, is_space } => {
                let font_size = atom.style.font_size;
                let atom_weight = font_weight_for(&atom.style);
                let atom_family = font_family_for(&atom.style);
                let letter_spacing_px =
                    atom.style
                        .letter_spacing
                        .to_px(font_size, 16.0, container_width);

                if is_space {
                    if cur_line_boxes.is_empty() {
                        continue;
                    }
                    let space_adv = measure_text_width_with_style_and_spacing(
                        &text,
                        font_size,
                        atom_weight,
                        atom_family,
                        letter_spacing_px,
                    )
                    .max(1.0);
                    if !is_nowrap && cur_y + space_adv > container_y + max_col_h {
                        continue;
                    }
                    let mut box_node =
                        LayoutBox::new(BoxType::TextNode(text), Some(atom.style.clone()));
                    box_node.link_target = atom.link_target.clone();
                    box_node.node_id = atom.owner_inline_id;
                    box_node.dimensions.content.origin = Point::new(0.0, cur_y);
                    box_node.dimensions.content.size = mango_core::Size::new(base_col_w, space_adv);
                    cur_line_boxes.push(box_node);
                    cur_y += space_adv;
                    continue;
                }

                // Word wrapping: calculate total word advance
                let word_adv: f32 = text
                    .chars()
                    .map(|ch| {
                        measure_text_width_with_style_and_spacing(
                            &ch.to_string(),
                            font_size,
                            atom_weight,
                            atom_family,
                            letter_spacing_px,
                        )
                        .max(1.0)
                    })
                    .sum();

                if !is_nowrap
                    && cur_y + word_adv > container_y + max_col_h
                    && !cur_line_boxes.is_empty()
                {
                    strip_trailing_space(&mut cur_line_boxes, &mut cur_y);
                    lines.push(VertLine {
                        boxes: std::mem::take(&mut cur_line_boxes),
                        height: cur_y - container_y,
                    });
                    cur_y = container_y;
                }

                for ch in text.chars() {
                    let ch_str = ch.to_string();
                    let glyph_adv = measure_text_width_with_style_and_spacing(
                        &ch_str,
                        font_size,
                        atom_weight,
                        atom_family,
                        letter_spacing_px,
                    )
                    .max(1.0);

                    if !is_nowrap
                        && cur_y + glyph_adv > container_y + max_col_h
                        && !cur_line_boxes.is_empty()
                    {
                        strip_trailing_space(&mut cur_line_boxes, &mut cur_y);
                        lines.push(VertLine {
                            boxes: std::mem::take(&mut cur_line_boxes),
                            height: cur_y - container_y,
                        });
                        cur_y = container_y;
                    }

                    let mut box_node =
                        LayoutBox::new(BoxType::TextNode(ch_str), Some(atom.style.clone()));
                    box_node.link_target = atom.link_target.clone();
                    box_node.node_id = atom.owner_inline_id;
                    box_node.dimensions.content.origin = Point::new(0.0, cur_y);
                    box_node.dimensions.content.size = mango_core::Size::new(base_col_w, glyph_adv);
                    cur_line_boxes.push(box_node);
                    cur_y += glyph_adv;
                }
            }
            InlineAtomKind::AtomicBox(mut atomic_box) => {
                let adv = atomic_box.dimensions.margin_box().height().max(16.0);
                if !is_nowrap && cur_y + adv > container_y + max_col_h && !cur_line_boxes.is_empty()
                {
                    lines.push(VertLine {
                        boxes: std::mem::take(&mut cur_line_boxes),
                        height: cur_y - container_y,
                    });
                    cur_y = container_y;
                }
                atomic_box.dimensions.content.origin = Point::new(0.0, cur_y);
                cur_line_boxes.push(*atomic_box);
                cur_y += adv;
            }
            InlineAtomKind::OutOfFlow(oof) | InlineAtomKind::Float(oof) => {
                let mut placed = *oof;
                placed.dimensions.content.origin = Point::new(0.0, cur_y);
                placed.containing_inline_id = atom.owner_inline_id;
                cur_line_boxes.push(placed);
            }
            InlineAtomKind::Spacing => {
                cur_y += atom.width;
            }
            _ => {}
        }
    }

    if !cur_line_boxes.is_empty() {
        strip_trailing_space(&mut cur_line_boxes, &mut cur_y);
        lines.push(VertLine {
            boxes: cur_line_boxes,
            height: cur_y - container_y,
        });
    }

    let get_line_width = |line: &VertLine| -> f32 {
        let mut w = 0.0f32;
        let mut has_text = false;
        for b in &line.boxes {
            if let BoxType::TextNode(_) = b.box_type {
                has_text = true;
            } else {
                w = w.max(b.dimensions.margin_box().width());
            }
        }
        if has_text || w <= 0.0 {
            w = w.max(base_col_w);
        }
        w
    };

    let line_widths: Vec<f32> = lines.iter().map(get_line_width).collect();
    let total_width: f32 = if line_widths.is_empty() {
        base_col_w
    } else {
        line_widths.iter().sum()
    };
    let is_width_auto = container
        .style
        .as_ref()
        .map(|s| s.width == mango_css::values::Length::Auto)
        .unwrap_or(true);
    let eff_container_w = if !is_width_auto && container_width > 0.0 {
        container_width
    } else {
        total_width
    };

    let mut positioned_boxes = Vec::new();
    let mut max_line_h = 0.0f32;

    let get_line_x = |idx: usize, widths: &[f32]| -> f32 {
        match writing_mode {
            WritingMode::VerticalRl | WritingMode::SidewaysRl => {
                let sum_to_idx: f32 = widths.iter().take(idx + 1).sum();
                container_x + eff_container_w - sum_to_idx
            }
            _ => {
                let sum_before_idx: f32 = widths.iter().take(idx).sum();
                container_x + sum_before_idx
            }
        }
    };

    let is_rtl = container
        .style
        .as_ref()
        .is_some_and(|s| s.direction == mango_css::values::Direction::Rtl);

    let has_fixed_height = container.dimensions.content.height() > 0.0
        || container
            .style
            .as_ref()
            .is_some_and(|s| s.height != mango_css::values::Length::Auto);
    let total_max_line_h = lines.iter().map(|l| l.height).fold(0.0f32, f32::max);
    let eff_col_h = if has_fixed_height {
        max_col_h
    } else {
        total_max_line_h
    };

    let line_heights: Vec<f32> = lines.iter().map(|l| l.height).collect();

    for (line_idx, line) in lines.into_iter().enumerate() {
        max_line_h = max_line_h.max(line.height);
        let line_x = get_line_x(line_idx, &line_widths);

        let mut boxes = line.boxes;
        let n_boxes = boxes.len();
        if n_boxes == 0 {
            continue;
        }

        let is_box_rtl = |b: &LayoutBox| -> bool {
            let is_override = b.style.as_ref().is_some_and(|s| {
                matches!(
                    s.unicode_bidi,
                    mango_css::values::UnicodeBidi::BidiOverride
                        | mango_css::values::UnicodeBidi::IsolateOverride
                ) && s.direction == mango_css::values::Direction::Rtl
            });
            if is_override {
                return true;
            }
            if let BoxType::TextNode(ref text) = b.box_type {
                if text.chars().any(crate::inline_flow::shaping::is_rtl_char) {
                    return true;
                }
            }
            false
        };

        // Group into direction segments
        let mut seg_ranges: Vec<(usize, usize, bool, f32)> = Vec::new();
        let mut s_idx = 0;
        while s_idx < n_boxes {
            let seg_rtl = is_box_rtl(&boxes[s_idx]);
            let mut e_idx = s_idx;
            let mut seg_h = 0.0f32;
            while e_idx < n_boxes {
                let b_rtl = is_box_rtl(&boxes[e_idx]);
                if b_rtl != seg_rtl {
                    break;
                }
                let is_oof = matches!(
                    boxes[e_idx].style.as_ref().map(|s| s.position),
                    Some(
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
                );
                if !is_oof {
                    seg_h += boxes[e_idx].dimensions.content.size.height;
                }
                e_idx += 1;
            }
            seg_ranges.push((s_idx, e_idx, seg_rtl, seg_h));
            s_idx = e_idx;
        }

        // Compute segment bounds along vertical inline axis
        let mut cur_top = if text_align == mango_css::values::TextAlign::Center {
            container_y + (eff_col_h - line.height).max(0.0) / 2.0
        } else if is_rtl {
            container_y + eff_col_h - line.height
        } else {
            container_y
        };

        let mut seg_bounds = Vec::new();
        for &(_, _, _, seg_h) in &seg_ranges {
            let seg_bottom = cur_top + seg_h;
            seg_bounds.push((cur_top, seg_bottom));
            cur_top = seg_bottom;
        }

        for (seg_i, &(s_idx, e_idx, seg_rtl, _)) in seg_ranges.iter().enumerate() {
            let (seg_top, seg_bottom) = seg_bounds[seg_i];
            let mut seg_cursor = if seg_rtl { seg_bottom } else { seg_top };

            for b_idx in s_idx..e_idx {
                let mut b = std::mem::replace(
                    &mut boxes[b_idx],
                    LayoutBox::new(BoxType::AnonymousBlock, None),
                );
                let is_oof = matches!(
                    b.style.as_ref().map(|s| s.position),
                    Some(
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
                );

                if !is_oof {
                    let b_h = b.dimensions.content.size.height;
                    if seg_rtl {
                        seg_cursor -= b_h;
                        b.dimensions.content.origin.y = seg_cursor;
                    } else {
                        b.dimensions.content.origin.y = seg_cursor;
                        seg_cursor += b_h;
                    }
                } else {
                    b.dimensions.content.origin.y = seg_cursor;
                }

                if is_oof {
                    let is_block_level = b
                        .style
                        .as_ref()
                        .map(|s| {
                            matches!(
                                s.original_display,
                                mango_css::values::Display::Block
                                    | mango_css::values::Display::Flex
                                    | mango_css::values::Display::Grid
                                    | mango_css::values::Display::Table
                                    | mango_css::values::Display::ListItem
                                    | mango_css::values::Display::FlowRoot
                            )
                        })
                        .unwrap_or(false);

                    let target_line_idx = if is_block_level {
                        line_idx + 1
                    } else {
                        line_idx
                    };
                    let base_x = get_line_x(target_line_idx, &line_widths);
                    let b_rtl = b
                        .style
                        .as_ref()
                        .map_or(is_rtl, |s| s.direction == mango_css::values::Direction::Rtl);
                    let base_y = if is_block_level {
                        let target_h = line_heights.get(target_line_idx).copied().unwrap_or(0.0);
                        if is_rtl {
                            if b_rtl {
                                container_y + eff_col_h
                            } else {
                                container_y + eff_col_h - target_h
                            }
                        } else {
                            if b_rtl {
                                container_y + target_h
                            } else {
                                container_y
                            }
                        }
                    } else {
                        b.dimensions.content.origin.y
                    };

                    let (pos_dx, pos_dy) = if let Some(ref pos_style) = b.positioned_inline_style {
                        let fs = pos_style.font_size;
                        let dx = if pos_style.left != mango_css::values::Length::Auto {
                            pos_style
                                .left
                                .to_px_with_viewport(fs, 16.0, eff_container_w, max_col_h)
                        } else if pos_style.right != mango_css::values::Length::Auto {
                            -pos_style.right.to_px_with_viewport(
                                fs,
                                16.0,
                                eff_container_w,
                                max_col_h,
                            )
                        } else {
                            0.0
                        };
                        let dy = if pos_style.top != mango_css::values::Length::Auto {
                            pos_style
                                .top
                                .to_px_with_viewport(fs, 16.0, max_col_h, max_col_h)
                        } else if pos_style.bottom != mango_css::values::Length::Auto {
                            -pos_style
                                .bottom
                                .to_px_with_viewport(fs, 16.0, max_col_h, max_col_h)
                        } else {
                            0.0
                        };
                        (dx, dy)
                    } else {
                        (0.0, 0.0)
                    };

                    let is_bottom_anchored = if is_block_level { b_rtl } else { seg_rtl };
                    b.is_static_bottom_anchored = is_bottom_anchored;
                    b.dimensions.content.origin = Point::new(base_x + pos_dx, base_y + pos_dy);
                    b.static_position = Some(b.dimensions.content.origin);
                } else {
                    b.dimensions.content.origin.x = line_x;
                    if b.style.as_ref().map(|s| s.position)
                        == Some(mango_css::values::Position::Relative)
                    {
                        let cb_dims = crate::dimensions::Dimensions::new(mango_core::Rect::new(
                            0.0,
                            0.0,
                            eff_container_w,
                            max_col_h,
                        ));
                        crate::block_flow::positioned::apply_relative_offset(&mut b, &cb_dims);
                    }
                }
                positioned_boxes.push(b);
            }
        }
    }

    if is_width_auto || container.dimensions.content.size.width <= 0.0 {
        let mut w = total_width;
        if let Some(s) = &container.style {
            let fs = s.font_size;
            let rfs = s.root_font_size;
            if s.max_width != mango_css::values::Length::Auto {
                let mw =
                    s.max_width
                        .to_px_with_viewport(fs, rfs, container_width, container_height);
                let max_content_w = if s.box_sizing == mango_css::values::BoxSizing::BorderBox {
                    (mw - container.dimensions.padding.horizontal()
                        - container.dimensions.border.horizontal())
                    .max(0.0)
                } else {
                    mw
                };
                w = w.min(max_content_w);
            }
        }
        container.dimensions.content.size.width = w;
    }

    super::container::resolve_inline_containing_blocks(
        container,
        &mut positioned_boxes,
        writing_mode,
        _float_ctx,
    );

    if resolved_cols > 1 {
        let mut fragmented_boxes = Vec::new();
        let is_rl = matches!(
            writing_mode,
            WritingMode::VerticalRl | WritingMode::SidewaysRl
        );
        let col_block_size = eff_container_w;
        let right_edge = container_x + col_block_size;

        for mut b in positioned_boxes {
            let (rel_start, rel_end) = if is_rl {
                let s = (right_edge - b.dimensions.content.right()).max(0.0);
                let e = (right_edge - b.dimensions.content.x()).max(0.0);
                (s, e)
            } else {
                let s = (b.dimensions.content.x() - container_x).max(0.0);
                let e = (b.dimensions.content.right() - container_x).max(0.0);
                (s, e)
            };

            let start_col = (rel_start / col_block_size).floor() as usize;
            let end_col = if rel_end > rel_start {
                ((rel_end - 0.001) / col_block_size).floor() as usize
            } else {
                start_col
            };
            let start_col = start_col.min(resolved_cols - 1);
            let end_col = end_col.min(resolved_cols - 1);

            if start_col == end_col {
                let c = start_col;
                let col_y = if is_rtl {
                    container_y + (resolved_cols - 1 - c) as f32 * (col_inline_size + gap)
                } else {
                    container_y + c as f32 * (col_inline_size + gap)
                };
                let new_x = if is_rl {
                    b.dimensions.content.x() + c as f32 * col_block_size
                } else {
                    b.dimensions.content.x() - c as f32 * col_block_size
                };
                let new_y = col_y + (b.dimensions.content.y() - container_y);
                let dx = new_x - b.dimensions.content.x();
                let dy = new_y - b.dimensions.content.y();
                b.dimensions.content.origin = Point::new(new_x, new_y);
                crate::block_flow::shift_descendants(&mut b, dx, dy);
                fragmented_boxes.push(b);
            } else {
                for c in start_col..=end_col {
                    let col_b_start = c as f32 * col_block_size;
                    let col_b_end = (c + 1) as f32 * col_block_size;
                    let slice_start = rel_start.max(col_b_start);
                    let slice_end = rel_end.min(col_b_end);
                    let slice_w = slice_end - slice_start;
                    if slice_w > 0.0 {
                        let mut frag = b.clone();
                        let col_y = if is_rtl {
                            container_y + (resolved_cols - 1 - c) as f32 * (col_inline_size + gap)
                        } else {
                            container_y + c as f32 * (col_inline_size + gap)
                        };
                        let new_x = if is_rl {
                            right_edge - (slice_end - col_b_start)
                        } else {
                            container_x + (slice_start - col_b_start)
                        };
                        let new_y = col_y + (b.dimensions.content.y() - container_y);
                        let dx = new_x - frag.dimensions.content.x();
                        let dy = new_y - frag.dimensions.content.y();
                        frag.dimensions.content.origin = Point::new(new_x, new_y);
                        frag.dimensions.content.size.width = slice_w;
                        crate::block_flow::shift_descendants(&mut frag, dx, dy);
                        fragmented_boxes.push(frag);
                    }
                }
            }
        }
        positioned_boxes = fragmented_boxes;
        container.dimensions.content.size.height = container_height;
    }

    container.children = positioned_boxes;
    if has_fixed_height && container_height > 0.0 {
        container.dimensions.content.size.height = container_height;
        container_height
    } else {
        container.dimensions.content.size.height = max_line_h;
        max_line_h
    }
}
