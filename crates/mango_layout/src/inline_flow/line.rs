use std::sync::Arc;

use mango_core::{EdgeSizes, Point, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::{Display, Overflow, Position, TextAlign};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;

use super::atom::{InlineAtom, InlineAtomKind};
use super::metrics::{measure_text_width_with_style, measure_text_width_with_style_and_spacing};
use super::shaping::{
    font_family_for, font_weight_for, is_rtl_char, shape_and_bidi_text_with_dir, used_line_height,
};

/// Computes the distance from the top margin edge of an inline-block to the baseline
/// of its last in-flow line box, adhering to CSS 2.1 §10.8.1.
pub(crate) fn find_inline_block_last_baseline_from_top(box_: &LayoutBox) -> Option<f32> {
    if let Some(s) = &box_.style
        && (s.overflow_x != Overflow::Visible || s.overflow_y != Overflow::Visible)
    {
        return None;
    }

    if matches!(box_.box_type, BoxType::TextNode(_)) {
        let fs = box_.style.as_ref().map(|s| s.font_size).unwrap_or(16.0);
        let family = font_family_for(box_.style.as_deref().unwrap_or(&ComputedStyle::default()));
        let weight = font_weight_for(box_.style.as_deref().unwrap_or(&ComputedStyle::default()));
        let (ascent, _, _) = mango_render::font::font_manager().font_metrics(family, weight, fs);
        return Some(ascent);
    }

    for child in box_.children.iter().rev() {
        let is_in_flow = child
            .style
            .as_ref()
            .map(|s| matches!(s.position, Position::Static | Position::Relative))
            .unwrap_or(true);
        let is_visible_disp = child
            .style
            .as_ref()
            .map(|s| s.display != Display::None)
            .unwrap_or(true);

        if is_in_flow
            && is_visible_disp
            && let Some(child_baseline) = find_inline_block_last_baseline_from_top(child)
        {
            let top_offset = (child.dimensions.content.y() - box_.dimensions.content.y()).max(0.0);
            let margin_top = box_.dimensions.margin.top
                + box_.dimensions.border.top
                + box_.dimensions.padding.top;
            return Some(margin_top + top_offset + child_baseline);
        }
    }
    None
}

pub(crate) fn finalize_line(
    line_atoms: &[InlineAtom],
    start_x: f32,
    available_width: f32,
    line_y: f32,
    text_align: TextAlign,
    out: &mut Vec<LayoutBox>,
    is_last_line: bool,
) -> f32 {
    if line_atoms.is_empty() {
        return 16.0;
    }

    // Strip trailing space from the line measurement and display (unless break-spaces)
    let effective_atoms = if line_atoms
        .last()
        .map(|a| a.is_space() && a.style.white_space != mango_css::values::WhiteSpace::BreakSpaces)
        .unwrap_or(false)
    {
        &line_atoms[..line_atoms.len() - 1]
    } else {
        line_atoms
    };

    let atoms_for_metrics = if effective_atoms.is_empty() {
        line_atoms
    } else {
        effective_atoms
    };

    let line_content_width: f32 = effective_atoms.iter().map(|a| a.width).sum();
    let min_line_height = atoms_for_metrics
        .iter()
        .map(|a| a.height)
        .fold(0.0f32, f32::max);

    // CSS 2.1 §10.8: Calculate unified baseline and total line box height across all inline boxes and text runs
    let mut max_above = 0.0f32;
    let mut max_below = 0.0f32;

    for atom in atoms_for_metrics {
        match &atom.kind {
            InlineAtomKind::Text { .. } => {
                let family = font_family_for(&atom.style);
                let weight = font_weight_for(&atom.style);
                let (ascent, descent, _) = mango_render::font::font_manager().font_metrics(
                    family,
                    weight,
                    atom.style.font_size,
                );
                let lh = used_line_height(&atom.style, family, weight);
                let natural_h = ascent + descent;
                let half_leading = (lh - natural_h) / 2.0;
                let above = half_leading + ascent;
                let below = half_leading + descent;
                max_above = max_above.max(above);
                max_below = max_below.max(below);
            }
            InlineAtomKind::AtomicBox(atomic_box) => {
                if atomic_box.tag_name.as_deref() == Some("ruby") {
                    let rt_h = atomic_box
                        .children
                        .first()
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(0.0);
                    let base_h = atomic_box
                        .children
                        .get(1)
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(atom.height - rt_h);
                    let base_ascent = base_h * 0.8;
                    let base_descent = base_h * 0.2;
                    max_above = max_above.max(rt_h + base_ascent);
                    max_below = max_below.max(base_descent);
                } else {
                    match atom.style.vertical_align {
                        mango_css::values::VerticalAlign::Baseline => {
                            if let Some(baseline_dist) =
                                find_inline_block_last_baseline_from_top(atomic_box)
                            {
                                max_above = max_above.max(baseline_dist);
                                max_below = max_below.max((atom.height - baseline_dist).max(0.0));
                            } else {
                                max_above = max_above.max(atom.height);
                            }
                        }
                        mango_css::values::VerticalAlign::Middle => {
                            let x_mid = atom.style.font_size * 0.25;
                            max_above = max_above.max(atom.height * 0.5 + x_mid);
                            max_below = max_below.max((atom.height * 0.5 - x_mid).max(0.0));
                        }
                        _ => {
                            max_above = max_above.max(atom.height);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if max_above + max_below <= 0.0 {
        max_above = min_line_height * 0.8;
        max_below = min_line_height * 0.2;
    }
    let baseline_from_line_top = max_above;
    let actual_line_height = (max_above + max_below).max(min_line_height).max(1.0);

    let is_rtl = atoms_for_metrics
        .first()
        .map(|a| a.style.direction == mango_css::values::Direction::Rtl)
        .unwrap_or(false);

    let mut space_expansion = 0.0f32;
    if text_align == TextAlign::Justify && !is_last_line {
        let free_space = (available_width - line_content_width).max(0.0);
        let space_count = effective_atoms.iter().filter(|a| a.is_space()).count();
        if space_count > 0 && free_space > 0.0 {
            space_expansion = free_space / space_count as f32;
        }
    }

    let offset_x = match text_align {
        TextAlign::Left => 0.0,
        TextAlign::Center => ((available_width - line_content_width) / 2.0).max(0.0),
        TextAlign::Right => (available_width - line_content_width).max(0.0),
        TextAlign::Justify => {
            if is_last_line && is_rtl {
                (available_width - line_content_width).max(0.0)
            } else {
                0.0
            }
        }
    };

    // Merge contiguous text atoms on this line that share identical style and link target.
    struct MergedRun {
        text: String,
        style: Arc<ComputedStyle>,
        start_x: f32,
        width: f32,
        link_target: Option<String>,
        link_frame: Option<String>,
        /// Font ascent, in pixels, at this run's font size.
        ascent: f32,
        /// Font descent (positive), in pixels, at this run's font size.
        descent: f32,
    }

    let mut runs: Vec<MergedRun> = Vec::new();
    let mut cursor_x = start_x + offset_x;

    let flush_text_runs = |runs: &mut Vec<MergedRun>, out: &mut Vec<LayoutBox>| {
        for mut run in runs.drain(..) {
            let is_rtl_dir = run.style.direction == mango_css::values::Direction::Rtl;
            let is_override = matches!(
                run.style.unicode_bidi,
                mango_css::values::UnicodeBidi::BidiOverride
                    | mango_css::values::UnicodeBidi::IsolateOverride
            );
            if is_override && is_rtl_dir {
                run.text = run.text.chars().rev().collect();
            } else if run.text.chars().any(is_rtl_char) || is_rtl_dir {
                run.text = shape_and_bidi_text_with_dir(&run.text, is_rtl_dir);
            }
            let ascent = run.ascent;
            let descent = run.descent;
            let natural_h = ascent + descent;
            // Top of this run's inline box: the line baseline minus its own ascent.
            let text_top = line_y + baseline_from_line_top - ascent;

            let run_y = match run.style.vertical_align {
                mango_css::values::VerticalAlign::Baseline => text_top,
                mango_css::values::VerticalAlign::Top => line_y,
                mango_css::values::VerticalAlign::Bottom => {
                    line_y + (actual_line_height - natural_h).max(0.0)
                }
                mango_css::values::VerticalAlign::Middle => {
                    line_y + baseline_from_line_top - (ascent * 0.5) - (natural_h * 0.5)
                }
                mango_css::values::VerticalAlign::Super => text_top - (ascent * 0.35),
                mango_css::values::VerticalAlign::Sub => text_top + (descent * 0.5),
                mango_css::values::VerticalAlign::TextTop => text_top,
                mango_css::values::VerticalAlign::TextBottom => {
                    line_y + (actual_line_height - natural_h).max(0.0)
                }
            };

            let mut box_node = LayoutBox::new(BoxType::TextNode(run.text), Some(run.style));
            box_node.dimensions = Dimensions {
                content: Rect::new(run.start_x, run_y, run.width, natural_h),
                padding: EdgeSizes::ZERO,
                border: EdgeSizes::ZERO,
                margin: EdgeSizes::ZERO,
            };
            box_node.link_target = run.link_target;
            box_node.link_frame = run.link_frame;
            out.push(box_node);
        }
    };

    for atom in effective_atoms {
        let atom_w = if atom.is_space() {
            atom.width + space_expansion
        } else {
            atom.width
        };

        match &atom.kind {
            InlineAtomKind::Text { text, .. } => {
                let can_merge = if let Some(last_run) = runs.last_mut() {
                    last_run.link_target == atom.link_target
                        && last_run.link_frame == atom.link_frame
                        && last_run.style.color == atom.style.color
                        && last_run.style.font_size == atom.style.font_size
                        && last_run.style.font_weight == atom.style.font_weight
                        && last_run.style.font_family == atom.style.font_family
                        && last_run.style.font_style == atom.style.font_style
                        && last_run.style.text_decoration == atom.style.text_decoration
                        && last_run.style.letter_spacing == atom.style.letter_spacing
                        && last_run.style.vertical_align == atom.style.vertical_align
                        && (last_run.style.word_spacing == mango_css::values::Length::Px(0.0)
                            && atom.style.word_spacing == mango_css::values::Length::Px(0.0))
                } else {
                    false
                };

                if can_merge {
                    let last_run = runs.last_mut().unwrap();
                    last_run.text.push_str(text);
                    last_run.width += atom_w;
                } else {
                    let family = font_family_for(&atom.style);
                    let weight = font_weight_for(&atom.style);
                    let (ascent, descent, _) = mango_render::font::font_manager().font_metrics(
                        family,
                        weight,
                        atom.style.font_size,
                    );
                    runs.push(MergedRun {
                        text: text.clone(),
                        start_x: cursor_x,
                        width: atom_w,
                        link_target: atom.link_target.clone(),
                        link_frame: atom.link_frame.clone(),
                        ascent,
                        descent,
                        style: atom.style.clone(),
                    });
                }
                cursor_x += atom_w;
            }
            InlineAtomKind::AtomicBox(atomic_box) => {
                flush_text_runs(&mut runs, out);

                let mut placed = (**atomic_box).clone();
                let box_outer_x = cursor_x;
                let box_outer_y = if atomic_box.tag_name.as_deref() == Some("ruby") {
                    let rt_h = atomic_box
                        .children
                        .first()
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(0.0);
                    let base_h = atomic_box
                        .children
                        .get(1)
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(atom.height - rt_h);
                    let base_ascent = base_h * 0.8;
                    line_y + baseline_from_line_top - (rt_h + base_ascent)
                } else {
                    match atom.style.vertical_align {
                        mango_css::values::VerticalAlign::Baseline => {
                            if let Some(baseline_dist) =
                                find_inline_block_last_baseline_from_top(atomic_box)
                            {
                                line_y + baseline_from_line_top - baseline_dist
                            } else {
                                line_y + baseline_from_line_top - atom.height
                            }
                        }
                        mango_css::values::VerticalAlign::Top => line_y,
                        mango_css::values::VerticalAlign::Bottom => {
                            line_y + (actual_line_height - atom.height).max(0.0)
                        }
                        mango_css::values::VerticalAlign::Middle => {
                            let x_mid = atom.style.font_size * 0.25;
                            line_y + baseline_from_line_top - x_mid - (atom.height * 0.5)
                        }
                        mango_css::values::VerticalAlign::Super => {
                            line_y + baseline_from_line_top - atom.height - 4.0
                        }
                        mango_css::values::VerticalAlign::Sub => {
                            line_y + baseline_from_line_top - atom.height + 4.0
                        }
                        mango_css::values::VerticalAlign::TextTop => line_y,
                        mango_css::values::VerticalAlign::TextBottom => {
                            line_y + (actual_line_height - atom.height).max(0.0)
                        }
                    }
                };

                let new_content_x = box_outer_x
                    + placed.dimensions.margin.left
                    + placed.dimensions.border.left
                    + placed.dimensions.padding.left;
                let new_content_y = box_outer_y
                    + placed.dimensions.margin.top
                    + placed.dimensions.border.top
                    + placed.dimensions.padding.top;

                let dx = new_content_x - placed.dimensions.content.x();
                let dy = new_content_y - placed.dimensions.content.y();
                placed.dimensions.content.origin = Point::new(new_content_x, new_content_y);
                crate::block_flow::shift_descendants(&mut placed, dx, dy);

                if placed.link_target.is_none() && atom.link_target.is_some() {
                    placed.link_target = atom.link_target.clone();
                }
                if placed.link_frame.is_none() && atom.link_frame.is_some() {
                    placed.link_frame = atom.link_frame.clone();
                }

                out.push(placed);
                cursor_x += atom.width;
            }
            InlineAtomKind::Spacing => {
                flush_text_runs(&mut runs, out);
                cursor_x += atom.width;
            }
            InlineAtomKind::LineBreak => {
                flush_text_runs(&mut runs, out);
            }
            InlineAtomKind::WordBreakOpportunity | InlineAtomKind::SoftHyphen => {}
        }
    }

    // Handle text-overflow: ellipsis
    let has_ellipsis = effective_atoms
        .iter()
        .any(|a| a.style.text_overflow == mango_css::values::TextOverflow::Ellipsis);
    if has_ellipsis && available_width > 0.0 {
        let max_x = start_x + available_width;
        if let Some(overflow_idx) = runs.iter().position(|r| r.start_x + r.width > max_x) {
            let mut target_idx = overflow_idx;
            loop {
                let r = &runs[target_idx];
                let font_size = r.style.font_size;
                let weight = font_weight_for(&r.style);
                let family = font_family_for(&r.style);
                let ell_w = measure_text_width_with_style("…", font_size, weight, family);
                let avail_for_text = max_x - r.start_x - ell_w;

                if avail_for_text > 0.0 || target_idx == 0 {
                    let run = &mut runs[target_idx];
                    let mut s = run.text.clone();
                    while !s.is_empty() {
                        let w = measure_text_width_with_style(&s, font_size, weight, family);
                        if w <= avail_for_text.max(0.0) {
                            break;
                        }
                        s.pop();
                    }
                    s.push('…');
                    run.width = measure_text_width_with_style(&s, font_size, weight, family);
                    run.text = s;
                    runs.truncate(target_idx + 1);
                    break;
                } else if target_idx > 0 {
                    target_idx -= 1;
                } else {
                    break;
                }
            }
        }
    }

    flush_text_runs(&mut runs, out);
    actual_line_height
}

pub(crate) fn split_overflow_text(
    text: &str,
    style: &Arc<ComputedStyle>,
    available_width: f32,
    height: f32,
    link_target: Option<&str>,
    link_frame: Option<&str>,
    container_width: f32,
) -> Vec<InlineAtom> {
    let font_size = style.font_size;
    let weight = font_weight_for(style);
    let family = font_family_for(style);
    let letter_spacing_px = style.letter_spacing.to_px(font_size, 16.0, container_width);

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut cur_w = 0.0;

    for ch in text.chars() {
        let s = ch.to_string();
        let w = measure_text_width_with_style_and_spacing(
            &s,
            font_size,
            weight,
            family,
            letter_spacing_px,
        );
        if !current.is_empty() && (cur_w + w > available_width) {
            chunks.push(InlineAtom {
                kind: InlineAtomKind::Text {
                    text: std::mem::take(&mut current),
                    is_space: false,
                },
                style: style.clone(),
                width: cur_w,
                height,
                link_target: link_target.map(|s| s.to_string()),
                link_frame: link_frame.map(|s| s.to_string()),
            });
            cur_w = 0.0;
        }
        current.push(ch);
        cur_w += w;
    }

    if !current.is_empty() {
        chunks.push(InlineAtom {
            kind: InlineAtomKind::Text {
                text: current,
                is_space: false,
            },
            style: style.clone(),
            width: cur_w,
            height,
            link_target: link_target.map(|s| s.to_string()),
            link_frame: link_frame.map(|s| s.to_string()),
        });
    }

    chunks
}

pub(crate) fn try_auto_hyphenate(
    text: &str,
    style: &ComputedStyle,
    max_w: f32,
    container_width: f32,
) -> Option<(String, String)> {
    let font_size = style.font_size;
    let weight = font_weight_for(style);
    let family = font_family_for(style);
    let letter_spacing = style.letter_spacing.to_px(font_size, 16.0, container_width);

    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 6 {
        return None;
    }

    let mut best_split = None;
    for i in (2..=chars.len() - 2).rev() {
        let prefix: String = chars[..i].iter().collect();
        let prefix_with_hyphen = format!("{}-", prefix);
        let w = measure_text_width_with_style_and_spacing(
            &prefix_with_hyphen,
            font_size,
            weight,
            family,
            letter_spacing,
        );
        if w <= max_w {
            let suffix: String = chars[i..].iter().collect();
            best_split = Some((prefix, suffix));
            break;
        }
    }
    best_split
}

#[allow(dead_code)]
pub(crate) fn to_roman(mut n: usize) -> String {
    let roman_pairs = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut result = String::new();
    for (val, sym) in roman_pairs {
        while n >= val {
            result.push_str(sym);
            n -= val;
        }
    }
    if result.is_empty() {
        "I".to_string()
    } else {
        result
    }
}
