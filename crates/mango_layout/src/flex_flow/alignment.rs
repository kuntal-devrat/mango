use mango_css::computed::ComputedStyle;
use mango_css::values::{AlignContent, AlignItems, AlignSelf, JustifyContent, Length};

use crate::box_tree::LayoutBox;

use super::metrics::ItemMetric;

pub fn compute_line_cross_sizes(
    lines: &[Vec<usize>],
    item_metrics: &[ItemMetric],
    container: &LayoutBox,
    style: &ComputedStyle,
    is_row: bool,
    is_wrap: bool,
    container_cross_size: f32,
) -> (Vec<f32>, Vec<f32>) {
    let mut line_cross_sizes: Vec<f32> = Vec::with_capacity(lines.len());
    let mut line_baseline_ascents: Vec<f32> = Vec::with_capacity(lines.len());

    for line in lines {
        let mut max_ascent = 0.0f32;
        let mut max_descent = 0.0f32;
        let mut max_non_baseline_cross = 0.0f32;
        let mut has_baseline = false;

        for &idx in line {
            let m = &item_metrics[idx];
            let child = &container.children[m.index];
            let child_style = child.style.as_ref();
            let align = match child_style.map(|s| s.align_self).unwrap_or(AlignSelf::Auto) {
                AlignSelf::Auto => style.align_items,
                AlignSelf::Stretch => AlignItems::Stretch,
                AlignSelf::FlexStart => AlignItems::FlexStart,
                AlignSelf::FlexEnd => AlignItems::FlexEnd,
                AlignSelf::Center => AlignItems::Center,
                AlignSelf::Baseline => AlignItems::Baseline,
                AlignSelf::SafeCenter => AlignItems::SafeCenter,
                AlignSelf::SafeFlexStart => AlignItems::SafeFlexStart,
                AlignSelf::SafeFlexEnd => AlignItems::SafeFlexEnd,
                AlignSelf::SafeStart => AlignItems::SafeStart,
                AlignSelf::SafeEnd => AlignItems::SafeEnd,
            };

            let outer_cross = m.cross + m.pad_cross + m.border_cross + m.margin_cross;
            if is_row && align == AlignItems::Baseline {
                has_baseline = true;
                let ascent = m.baseline_ascent;
                let descent = (outer_cross - ascent).max(0.0);
                max_ascent = max_ascent.max(ascent);
                max_descent = max_descent.max(descent);
            } else {
                max_non_baseline_cross = max_non_baseline_cross.max(outer_cross);
            }
        }

        let l_cross = if has_baseline {
            max_non_baseline_cross.max(max_ascent + max_descent)
        } else {
            max_non_baseline_cross
        }
        .max(if !is_wrap { container_cross_size } else { 0.0 });

        line_cross_sizes.push(l_cross);
        line_baseline_ascents.push(if has_baseline { max_ascent } else { 0.0 });
    }

    (line_cross_sizes, line_baseline_ascents)
}

pub fn compute_align_content_spacing(
    line_cross_sizes: &mut [f32],
    style: &ComputedStyle,
    is_wrap: bool,
    container_cross_size: f32,
    cross_gap: f32,
) -> (f32, f32) {
    let line_count = line_cross_sizes.len();
    let total_lines_cross: f32 = line_cross_sizes.iter().sum::<f32>()
        + if line_count > 1 {
            (line_count - 1) as f32 * cross_gap
        } else {
            0.0
        };

    let free_cross = if is_wrap && container_cross_size > total_lines_cross {
        container_cross_size - total_lines_cross
    } else {
        0.0
    };

    if free_cross > 0.0 && is_wrap {
        match style.align_content {
            AlignContent::Normal | AlignContent::Stretch => {
                let extra_per_line = free_cross / line_count as f32;
                for size in line_cross_sizes.iter_mut() {
                    *size += extra_per_line;
                }
                (0.0, cross_gap)
            }
            AlignContent::FlexStart => (0.0, cross_gap),
            AlignContent::FlexEnd => (free_cross, cross_gap),
            AlignContent::Center => (free_cross / 2.0, cross_gap),
            AlignContent::SpaceBetween => {
                if line_count > 1 {
                    (0.0, cross_gap + free_cross / (line_count - 1) as f32)
                } else {
                    (0.0, cross_gap)
                }
            }
            AlignContent::SpaceAround => {
                if line_count > 0 {
                    let per_line = free_cross / line_count as f32;
                    (per_line / 2.0, cross_gap + per_line)
                } else {
                    (0.0, cross_gap)
                }
            }
            AlignContent::SpaceEvenly => {
                let spacing = free_cross / (line_count + 1) as f32;
                (spacing, cross_gap + spacing)
            }
        }
    } else {
        (0.0, cross_gap)
    }
}

pub fn compute_justify_content_spacing(
    line: &[usize],
    item_metrics: &[ItemMetric],
    container: &mut LayoutBox,
    style: &ComputedStyle,
    is_row: bool,
    is_container_main_definite: bool,
    container_main_size: f32,
    main_gap: f32,
) -> (f32, f32) {
    let line_item_count = line.len();
    let total_gaps = if line_item_count > 1 {
        (line_item_count - 1) as f32 * main_gap
    } else {
        0.0
    };

    let final_line_main: f32 = line
        .iter()
        .map(|&idx| {
            let m = &item_metrics[idx];
            m.final_main + m.pad_main + m.border_main + m.margin_main
        })
        .sum();
    let remaining_free = if is_container_main_definite {
        (container_main_size - final_line_main - total_gaps).max(0.0)
    } else {
        0.0
    };

    let mut main_auto_margin_count = 0;
    for &idx in line {
        let ch = &container.children[item_metrics[idx].index];
        if let Some(cs) = &ch.style {
            if is_row {
                if cs.margin_left == Length::Auto {
                    main_auto_margin_count += 1;
                }
                if cs.margin_right == Length::Auto {
                    main_auto_margin_count += 1;
                }
            } else {
                if cs.margin_top == Length::Auto {
                    main_auto_margin_count += 1;
                }
                if cs.margin_bottom == Length::Auto {
                    main_auto_margin_count += 1;
                }
            }
        }
    }

    if main_auto_margin_count > 0 && remaining_free > 0.0 {
        let auto_margin_add = remaining_free / main_auto_margin_count as f32;
        for &idx in line {
            let ch = &mut container.children[item_metrics[idx].index];
            if let Some(cs) = &ch.style {
                if is_row {
                    if cs.margin_left == Length::Auto {
                        ch.dimensions.margin.left = auto_margin_add;
                    }
                    if cs.margin_right == Length::Auto {
                        ch.dimensions.margin.right = auto_margin_add;
                    }
                } else {
                    if cs.margin_top == Length::Auto {
                        ch.dimensions.margin.top = auto_margin_add;
                    }
                    if cs.margin_bottom == Length::Auto {
                        ch.dimensions.margin.bottom = auto_margin_add;
                    }
                }
            }
        }
        (0.0, main_gap)
    } else {
        match style.justify_content {
            JustifyContent::Normal | JustifyContent::FlexStart => (0.0, main_gap),
            JustifyContent::FlexEnd => (remaining_free, main_gap),
            JustifyContent::Center => (remaining_free / 2.0, main_gap),
            JustifyContent::SpaceBetween => {
                if line_item_count > 1 {
                    (
                        0.0,
                        main_gap + remaining_free / (line_item_count - 1) as f32,
                    )
                } else {
                    (0.0, main_gap)
                }
            }
            JustifyContent::SpaceAround => {
                if line_item_count > 0 {
                    let per_item = remaining_free / line_item_count as f32;
                    (per_item / 2.0, main_gap + per_item)
                } else {
                    (0.0, main_gap)
                }
            }
            JustifyContent::SpaceEvenly => {
                let spacing = (remaining_free + total_gaps) / (line_item_count + 1) as f32;
                (spacing, spacing)
            }
        }
    }
}
