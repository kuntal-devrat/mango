use std::sync::Arc;

use mango_core::{Point, Rect};
use mango_css::values::{TextAlign, WritingMode};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::float::FloatContext;

use super::atom::{InlineAtom, InlineAtomKind};
use super::collector::collect_inline_atoms;
use super::line::{finalize_line, split_overflow_text, to_roman, try_auto_hyphenate};
use super::metrics::{
    compute_balanced_line_width, measure_text_width, measure_text_width_with_style,
    measure_text_width_with_style_and_spacing,
};
use super::shaping::{font_family_for, font_weight_for};
use super::vertical::layout_vertical_inline_children;

/// Executes inline formatting context layout for the children of a block container.
///
/// Converts the inline children into positioned line boxes, breaking lines at word boundaries
/// according to available width and applying horizontal text alignment.
pub fn layout_inline_children(container: &mut LayoutBox, float_ctx: &mut FloatContext) -> f32 {
    let container_x = container.dimensions.content.x();
    let container_y = container.dimensions.content.y();
    let container_width = container.dimensions.content.width();
    let text_align = container
        .style
        .as_ref()
        .map(|s| match s.text_align {
            TextAlign::Start => {
                if s.direction == mango_css::values::Direction::Rtl {
                    TextAlign::Right
                } else {
                    TextAlign::Left
                }
            }
            TextAlign::End => {
                if s.direction == mango_css::values::Direction::Rtl {
                    TextAlign::Left
                } else {
                    TextAlign::Right
                }
            }
            other => other,
        })
        .unwrap_or(TextAlign::Left);

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
                0.0
            }
        }
    } else {
        0.0
    };

    let writing_mode = container
        .style
        .as_ref()
        .map(|s| s.writing_mode)
        .unwrap_or(WritingMode::HorizontalTb);
    if matches!(
        writing_mode,
        WritingMode::VerticalRl | WritingMode::VerticalLr
    ) {
        if container.dimensions.content.size.height == 0.0 && container_height > 0.0 {
            container.dimensions.content.size.height = container_height;
        }
        return layout_vertical_inline_children(container, float_ctx, writing_mode);
    }

    // 1. Preserve unflattened source children for idempotent relayout passes
    if container.raw_children.is_none() {
        container.raw_children = Some(std::mem::take(&mut container.children));
    }

    // Check for CSS Multi-column layout on this inline container
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
        .map(|s| s.column_gap.to_px(font_size, 16.0, container_width))
        .unwrap_or(0.0);
    let resolved_cols = match (col_count_opt, col_width_opt) {
        (Some(c), Some(w_len)) => {
            let w_px = w_len.to_px(font_size, 16.0, container_width);
            if w_px > 0.0 {
                let max_cols = ((container_width + gap) / (w_px + gap)).floor().max(1.0) as usize;
                c.min(max_cols).max(1)
            } else {
                c.max(1)
            }
        }
        (Some(c), None) => c.max(1),
        (None, Some(w_len)) => {
            let w_px = w_len.to_px(font_size, 16.0, container_width);
            if w_px > 0.0 {
                ((container_width + gap) / (w_px + gap)).floor().max(1.0) as usize
            } else {
                1
            }
        }
        (None, None) => 1,
    };

    let (layout_width, target_col_h) = if resolved_cols > 1 {
        let col_w =
            ((container_width - (resolved_cols - 1) as f32 * gap) / resolved_cols as f32).max(1.0);
        let col_h = if container_height > 0.0 {
            container_height
        } else {
            0.0
        };
        (col_w, col_h)
    } else {
        (container_width, container_height)
    };

    // Collect all inline atoms from children (no synthetic spaces between TextNodes)
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
                layout_width,
                container_height,
                initial_bidi_override,
            );
        }
    }

    // 1b. Prepend list marker atom if container is a list item
    let is_list_item = container.style.as_ref().map(|s| s.display)
        == Some(mango_css::values::Display::ListItem)
        || container.tag_name.as_deref() == Some("li");

    if is_list_item {
        let mut style = container.style.as_deref().cloned().unwrap_or_default();
        if let Some(c) = container
            .get_attribute("_mango_marker_color")
            .and_then(mango_css::values::Value::parse_color)
        {
            style.color = c;
        }

        let marker_str = if let Some(custom) = container.get_attribute("_mango_marker_content") {
            Some(custom.to_string())
        } else {
            match style.list_style_type {
                mango_css::values::ListStyleType::None => None,
                mango_css::values::ListStyleType::Disc => Some("• ".to_string()),
                mango_css::values::ListStyleType::Circle => Some("○ ".to_string()),
                mango_css::values::ListStyleType::Square => Some("▪ ".to_string()),
                mango_css::values::ListStyleType::Decimal => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .or_else(|| container.get_attribute("value"))
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    Some(format!("{}. ", idx))
                }
                mango_css::values::ListStyleType::LowerAlpha => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    let ch = (b'a' + ((idx - 1) % 26) as u8) as char;
                    Some(format!("{}. ", ch))
                }
                mango_css::values::ListStyleType::UpperAlpha => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    let ch = (b'A' + ((idx - 1) % 26) as u8) as char;
                    Some(format!("{}. ", ch))
                }
                mango_css::values::ListStyleType::LowerRoman => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    Some(format!("{}. ", to_roman(idx).to_lowercase()))
                }
                mango_css::values::ListStyleType::UpperRoman => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    Some(format!("{}. ", to_roman(idx)))
                }
            }
        };

        if let Some(m_text) = marker_str {
            let already_has_bullet = atoms
                .iter()
                .find_map(|a| match &a.kind {
                    InlineAtomKind::Text { text, .. } if !text.trim().is_empty() => {
                        let trimmed = text.trim_start();
                        Some(
                            trimmed.starts_with('•')
                                || trimmed.starts_with('\u{2022}')
                                || trimmed.starts_with('○')
                                || trimmed.starts_with('▪')
                                || trimmed.starts_with(&m_text),
                        )
                    }
                    _ => None,
                })
                .unwrap_or(false);

            if !already_has_bullet {
                let m_font_size = style.font_size;
                let m_w = measure_text_width(&m_text, m_font_size);
                let m_h = (m_font_size * 1.2).ceil();
                atoms.insert(
                    0,
                    InlineAtom {
                        kind: InlineAtomKind::Text {
                            text: m_text,
                            is_space: false,
                        },
                        style: Arc::new(style),
                        width: m_w,
                        height: m_h,
                        link_target: None,
                        link_frame: None,
                        owner_inline_id: None,
                    },
                );
            }
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

    // 2. Break atoms into lines
    let mut positioned_boxes = Vec::new();
    let mut current_y = container_y;

    let is_container_rtl = container
        .style
        .as_ref()
        .map(|s| s.direction == mango_css::values::Direction::Rtl)
        .unwrap_or(false);

    let text_indent = container
        .style
        .as_ref()
        .map(|s| s.text_indent.to_px(s.font_size, 16.0, layout_width))
        .unwrap_or(0.0);
    let mut is_first_line = true;

    let text_wrap = container
        .style
        .as_ref()
        .map(|s| s.text_wrap)
        .unwrap_or(mango_css::values::TextWrap::Wrap);

    let balance_width = if text_wrap == mango_css::values::TextWrap::Balance && layout_width > 0.0 {
        compute_balanced_line_width(&atoms, layout_width)
    } else {
        None
    };

    let get_line_x_and_width = |min_x: f32, max_x: f32, is_first: bool| -> (f32, f32, f32) {
        let line_indent = if is_first { text_indent } else { 0.0 };
        let avail_w = (max_x - min_x - line_indent).max(0.0);
        let wrap_w = if let Some(bw) = balance_width {
            avail_w.min(bw)
        } else {
            avail_w
        };
        let start_x = if is_container_rtl {
            min_x
        } else {
            min_x + line_indent
        };
        (start_x, avail_w, wrap_w)
    };

    let max_lines = container
        .style
        .as_ref()
        .and_then(|s| match s.line_clamp {
            mango_css::values::LineClamp::Lines(n) if n > 0 => Some(n as usize),
            _ => None,
        })
        .or_else(|| {
            atoms.first().and_then(|a| match a.style.line_clamp {
                mango_css::values::LineClamp::Lines(n) if n > 0 => Some(n as usize),
                _ => None,
            })
        });
    let mut line_count = 0;
    let mut orphan_prevented = false;

    let mut line_atoms: Vec<InlineAtom> = Vec::new();
    let mut line_width = 0.0;

    let mut atom_queue: std::collections::VecDeque<InlineAtom> = atoms.into();

    while let Some(atom) = atom_queue.pop_front() {
        if let InlineAtomKind::LineBreak { is_br } = atom.kind {
            let (min_x, max_x) =
                float_ctx.available_span(current_y, atom.height, container_x, layout_width);
            let (line_x, available_width, _) = get_line_x_and_width(min_x, max_x, is_first_line);
            let block_start_x = if is_container_rtl { max_x } else { min_x };
            let line_h = finalize_line(
                &line_atoms,
                line_x,
                available_width,
                current_y,
                text_align,
                &mut positioned_boxes,
                true,
                block_start_x,
            );
            let step_h = line_h.max(atom.height);
            if is_br {
                let mut br_box = LayoutBox::new(BoxType::InlineNode, Some(atom.style.clone()));
                br_box.node_id = atom.owner_inline_id;
                br_box.tag_name = Some("br".to_string());
                br_box.dimensions.content = Rect::new(line_x, current_y, 0.0, step_h);
                positioned_boxes.push(br_box);
            }
            current_y += step_h;
            is_first_line = false;
            line_atoms.clear();
            line_width = 0.0;
            line_count += 1;
            if let Some(max) = max_lines
                && line_count >= max
            {
                break;
            }
            continue;
        }

        if matches!(atom.kind, InlineAtomKind::SoftHyphen) {
            line_atoms.push(atom);
            continue;
        }

        let atom_height = atom.height;
        let (mut min_x, mut max_x) =
            float_ctx.available_span(current_y, atom_height, container_x, layout_width);
        let (mut line_x, mut available_width, mut wrap_width) =
            get_line_x_and_width(min_x, max_x, is_first_line);

        // CSS 2.1 §9.5: If a line box is too small to contain any content due to floats, shift downward
        if line_atoms.is_empty()
            && (available_width <= 0.0
                || (atom.width > available_width
                    && float_ctx.has_floats_at(current_y, atom_height)))
            && let Some(next_y) = float_ctx.next_vertical_opportunity(current_y)
        {
            current_y = next_y;
            let (new_min, new_max) =
                float_ctx.available_span(current_y, atom_height, container_x, layout_width);
            min_x = new_min;
            max_x = new_max;
            let (lx, aw, ww) = get_line_x_and_width(min_x, max_x, is_first_line);
            line_x = lx;
            available_width = aw;
            wrap_width = ww;
        }

        let is_pre = matches!(
            atom.style.white_space,
            mango_css::values::WhiteSpace::Pre
                | mango_css::values::WhiteSpace::PreWrap
                | mango_css::values::WhiteSpace::BreakSpaces
        );
        // Discard leading space on a new line (unless pre/pre-wrap/break-spaces)
        if line_atoms.is_empty() && atom.is_space() && !is_pre {
            continue;
        }

        let is_nowrap = atom.style.white_space == mango_css::values::WhiteSpace::Nowrap;

        // Check overflow-wrap: break-word / anywhere or word-break: break-word
        let can_break_word = (atom.style.overflow_wrap != mango_css::values::OverflowWrap::Normal
            || atom.style.word_break == mango_css::values::WordBreak::BreakWord)
            && !is_nowrap;

        if can_break_word
            && (atom.width > wrap_width)
            && let InlineAtomKind::Text {
                text,
                is_space: false,
            } = &atom.kind
            && text.chars().count() > 1
            && wrap_width > 0.0
        {
            let mut chunks = split_overflow_text(
                text.as_str(),
                &atom.style,
                wrap_width,
                atom.height,
                atom.link_target.as_deref(),
                atom.link_frame.as_deref(),
                layout_width,
                atom.owner_inline_id,
            );
            if !chunks.is_empty() {
                let first = chunks.remove(0);
                for chk in chunks.into_iter().rev() {
                    atom_queue.push_front(chk);
                }
                atom_queue.push_front(first);
                continue;
            }
        }

        // Automatic hyphenation check
        let is_auto_hyphens = atom.style.hyphens == mango_css::values::Hyphens::Auto && !is_nowrap;
        if is_auto_hyphens
            && (line_width + atom.width > wrap_width)
            && let InlineAtomKind::Text {
                text,
                is_space: false,
            } = &atom.kind
            && text.chars().count() >= 6
        {
            let (can_fit_here, remaining_w) = if line_atoms.is_empty() {
                (true, wrap_width)
            } else if wrap_width > line_width + 20.0 {
                (true, wrap_width - line_width)
            } else {
                (false, 0.0)
            };

            if can_fit_here
                && let Some((prefix, suffix)) =
                    try_auto_hyphenate(text.as_str(), &atom.style, remaining_w, container_width)
            {
                let hyphen_atom = InlineAtom {
                    kind: InlineAtomKind::Text {
                        text: format!("{}-", prefix),
                        is_space: false,
                    },
                    style: atom.style.clone(),
                    width: remaining_w,
                    height: atom.height,
                    link_target: atom.link_target.clone(),
                    link_frame: atom.link_frame.clone(),
                    owner_inline_id: atom.owner_inline_id,
                };
                line_atoms.push(hyphen_atom);
                let block_start_x = if is_container_rtl { max_x } else { min_x };
                let line_h = finalize_line(
                    &line_atoms,
                    line_x,
                    available_width,
                    current_y,
                    text_align,
                    &mut positioned_boxes,
                    false,
                    block_start_x,
                );
                current_y += line_h;
                is_first_line = false;
                line_atoms.clear();
                line_width = 0.0;
                line_count += 1;

                if let Some(max) = max_lines
                    && line_count >= max
                {
                    if let Some(last_box) = positioned_boxes.last_mut()
                        && let BoxType::TextNode(t) = &mut last_box.box_type
                        && !t.ends_with('…')
                    {
                        t.push('…');
                        if let Some(st) = &last_box.style {
                            let font_size = st.font_size;
                            let weight = font_weight_for(st);
                            let family = font_family_for(st);
                            let letter_spacing_px =
                                st.letter_spacing.to_px(font_size, 16.0, container_width);
                            last_box.dimensions.content.size.width =
                                measure_text_width_with_style_and_spacing(
                                    t,
                                    font_size,
                                    weight,
                                    family,
                                    letter_spacing_px,
                                );
                        }
                    }
                    break;
                }

                let suffix_w = measure_text_width_with_style(
                    &suffix,
                    atom.style.font_size,
                    font_weight_for(&atom.style),
                    font_family_for(&atom.style),
                );
                let suffix_atom = InlineAtom {
                    kind: InlineAtomKind::Text {
                        text: suffix,
                        is_space: false,
                    },
                    style: atom.style.clone(),
                    width: suffix_w,
                    height: atom.height,
                    link_target: atom.link_target,
                    link_frame: atom.link_frame,
                    owner_inline_id: atom.owner_inline_id,
                };
                atom_queue.push_front(suffix_atom);
                continue;
            }
        }

        if !line_atoms.is_empty() && !is_nowrap && (line_width + atom.width > wrap_width) {
            // CSS Text 4 §7: text-wrap: pretty orphan prevention
            if text_wrap == mango_css::values::TextWrap::Pretty && !orphan_prevented {
                let remaining_words = (!atom.is_space() as usize)
                    + atom_queue
                        .iter()
                        .take_while(|a| !matches!(a.kind, InlineAtomKind::LineBreak { .. }))
                        .filter(|a| !a.is_space())
                        .count();
                let current_words = line_atoms.iter().filter(|a| !a.is_space()).count();
                if remaining_words == 1 && current_words >= 2 {
                    if let Some(last_word_idx) = line_atoms.iter().rposition(|a| !a.is_space()) {
                        let pull_idx =
                            if last_word_idx > 0 && line_atoms[last_word_idx - 1].is_space() {
                                last_word_idx - 1
                            } else {
                                last_word_idx
                            };
                        let pulled: Vec<InlineAtom> = line_atoms.drain(pull_idx..).collect();
                        atom_queue.push_front(atom);
                        for p in pulled.into_iter().rev() {
                            atom_queue.push_front(p);
                        }
                        orphan_prevented = true;

                        let block_start_x = if is_container_rtl { max_x } else { min_x };
                        let line_h = finalize_line(
                            &line_atoms,
                            line_x,
                            available_width,
                            current_y,
                            text_align,
                            &mut positioned_boxes,
                            false,
                            block_start_x,
                        );
                        current_y += line_h;
                        is_first_line = false;
                        line_atoms.clear();
                        line_width = 0.0;
                        line_count += 1;
                        continue;
                    }
                }
            }

            // Check if there was a SoftHyphen in line_atoms
            if let Some(sh_pos) = line_atoms
                .iter()
                .rposition(|a| matches!(a.kind, InlineAtomKind::SoftHyphen))
            {
                let has_space_after_sh = line_atoms[sh_pos + 1..].iter().any(|a| {
                    a.is_space() || matches!(a.kind, InlineAtomKind::WordBreakOpportunity)
                });
                if !has_space_after_sh {
                    let carryover: Vec<InlineAtom> = line_atoms.drain(sh_pos + 1..).collect();
                    for c in carryover.into_iter().rev() {
                        atom_queue.push_front(c);
                    }
                    line_atoms.pop(); // Remove SoftHyphen
                    if let Some(prev) = line_atoms.last_mut()
                        && let InlineAtomKind::Text { text, .. } = &mut prev.kind
                        && !text.ends_with('-')
                    {
                        text.push('-');
                        let font_size = prev.style.font_size;
                        let weight = font_weight_for(&prev.style);
                        let family = font_family_for(&prev.style);
                        let letter_spacing_px =
                            prev.style
                                .letter_spacing
                                .to_px(font_size, 16.0, layout_width);
                        prev.width = measure_text_width_with_style_and_spacing(
                            text,
                            font_size,
                            weight,
                            family,
                            letter_spacing_px,
                        );
                    }
                }
            }

            // Line break
            let block_start_x = if is_container_rtl { max_x } else { min_x };
            let line_h = finalize_line(
                &line_atoms,
                line_x,
                available_width,
                current_y,
                text_align,
                &mut positioned_boxes,
                false,
                block_start_x,
            );
            current_y += line_h;
            is_first_line = false;
            line_atoms.clear();
            line_width = 0.0;
            line_count += 1;

            if let Some(max) = max_lines
                && line_count >= max
            {
                if let Some(last_box) = positioned_boxes.last_mut()
                    && let BoxType::TextNode(t) = &mut last_box.box_type
                    && !t.ends_with('…')
                {
                    t.push('…');
                    if let Some(st) = &last_box.style {
                        let font_size = st.font_size;
                        let weight = font_weight_for(st);
                        let family = font_family_for(st);
                        let letter_spacing_px =
                            st.letter_spacing.to_px(font_size, 16.0, layout_width);
                        last_box.dimensions.content.size.width =
                            measure_text_width_with_style_and_spacing(
                                t,
                                font_size,
                                weight,
                                family,
                                letter_spacing_px,
                            );
                    }
                }
                break;
            }

            if atom.is_space() && !is_pre {
                continue;
            }
        }

        line_width += atom.width;
        line_atoms.push(atom);
    }

    if !line_atoms.is_empty() && max_lines.is_none_or(|max| line_count < max) {
        let line_height = line_atoms.iter().map(|a| a.height).fold(0.0f32, f32::max);
        let (min_x, max_x) =
            float_ctx.available_span(current_y, line_height, container_x, layout_width);
        let (line_x, available_width, _) = get_line_x_and_width(min_x, max_x, is_first_line);
        let block_start_x = if is_container_rtl { max_x } else { min_x };

        let line_h = finalize_line(
            &line_atoms,
            line_x,
            available_width,
            current_y,
            text_align,
            &mut positioned_boxes,
            true,
            block_start_x,
        );
        current_y += line_h;
    }

    resolve_inline_containing_blocks(container, &mut positioned_boxes, writing_mode, float_ctx);

    if resolved_cols > 1 {
        let col_h = if target_col_h > 0.0 {
            target_col_h
        } else {
            let total_content_h = current_y - container_y;
            (total_content_h / resolved_cols as f32).max(20.0)
        };

        let mut fragmented_boxes = Vec::new();
        for mut b in positioned_boxes {
            let rel_top = (b.dimensions.content.y() - container_y).max(0.0);
            let rel_bottom = (b.dimensions.content.bottom() - container_y).max(0.0);
            let start_col = (rel_top / col_h).floor() as usize;
            let end_col = if rel_bottom > rel_top {
                ((rel_bottom - 0.001) / col_h).floor() as usize
            } else {
                start_col
            };
            let start_col = start_col.min(resolved_cols - 1);
            let end_col = end_col.min(resolved_cols - 1);

            if start_col == end_col {
                let col_idx = start_col;
                let col_x = if is_container_rtl {
                    container_x + (resolved_cols - 1 - col_idx) as f32 * (layout_width + gap)
                } else {
                    container_x + col_idx as f32 * (layout_width + gap)
                };
                let dx = col_x - container_x;
                let dy = -(col_idx as f32 * col_h);
                b.dimensions.content.origin.x += dx;
                b.dimensions.content.origin.y += dy;
                crate::block_flow::shift_descendants(&mut b, dx, dy);
                fragmented_boxes.push(b);
            } else {
                for c in start_col..=end_col {
                    let col_top = c as f32 * col_h;
                    let col_bottom = (c + 1) as f32 * col_h;
                    let slice_top = rel_top.max(col_top);
                    let slice_bottom = rel_bottom.min(col_bottom);
                    let slice_h = slice_bottom - slice_top;
                    if slice_h > 0.0 {
                        let mut frag = b.clone();
                        let col_x = if is_container_rtl {
                            container_x + (resolved_cols - 1 - c) as f32 * (layout_width + gap)
                        } else {
                            container_x + c as f32 * (layout_width + gap)
                        };
                        let new_x = col_x + (b.dimensions.content.x() - container_x);
                        let new_y = container_y + (slice_top - col_top);
                        let dx = new_x - frag.dimensions.content.x();
                        let dy = new_y - frag.dimensions.content.y();
                        frag.dimensions.content.origin = Point::new(new_x, new_y);
                        frag.dimensions.content.size.height = slice_h;
                        crate::block_flow::shift_descendants(&mut frag, dx, dy);
                        fragmented_boxes.push(frag);
                    }
                }
            }
        }
        positioned_boxes = fragmented_boxes;
    }

    let total_height = if resolved_cols > 1 {
        if container_height > 0.0 {
            container_height
        } else {
            let total_content_h = current_y - container_y;
            (total_content_h / resolved_cols as f32).max(20.0)
        }
    } else {
        current_y - container_y
    };
    container.children = positioned_boxes;
    total_height
}

pub(crate) use super::containing_block::resolve_inline_containing_blocks;
