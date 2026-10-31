use mango_css::computed::ComputedStyle;
use mango_css::values::{AlignContent, AlignItems, AlignSelf, JustifyContent, Length};

use crate::box_tree::LayoutBox;

use super::metrics::ItemMetric;

pub struct LineCrossMetrics {
    pub cross_sizes: Vec<f32>,
    pub first_ascents: Vec<f32>,
    pub last_ascents: Vec<f32>,
    pub first_group_crosses: Vec<f32>,
    pub last_group_crosses: Vec<f32>,
}

pub fn compute_line_cross_sizes(
    lines: &[Vec<usize>],
    item_metrics: &[ItemMetric],
    container: &LayoutBox,
    style: &ComputedStyle,
    is_row: bool,
    is_wrap: bool,
    container_cross_size: f32,
    min_cross_size: f32,
    max_cross_size: f32,
) -> LineCrossMetrics {
    let mut line_cross_sizes: Vec<f32> = Vec::with_capacity(lines.len());
    let mut line_baseline_ascents: Vec<f32> = Vec::with_capacity(lines.len());
    let mut line_last_baseline_ascents: Vec<f32> = Vec::with_capacity(lines.len());
    let mut line_first_group_crosses: Vec<f32> = Vec::with_capacity(lines.len());
    let mut line_last_group_crosses: Vec<f32> = Vec::with_capacity(lines.len());

    for line in lines {
        let mut max_ascent = 0.0f32;
        let mut max_descent = 0.0f32;
        let mut max_non_baseline_cross = 0.0f32;
        let mut has_baseline = false;

        let mut max_last_ascent = 0.0f32;
        let mut max_last_descent = 0.0f32;
        let mut has_last_baseline = false;

        let mut non_collapsed_max_ascent = 0.0f32;
        let mut non_collapsed_max_descent = 0.0f32;
        let mut has_non_collapsed_baseline = false;

        let mut non_collapsed_max_last_ascent = 0.0f32;
        let mut non_collapsed_max_last_descent = 0.0f32;
        let mut has_non_collapsed_last_baseline = false;

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
                AlignSelf::LastBaseline => AlignItems::LastBaseline,
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

                if !m.is_collapsed {
                    has_non_collapsed_baseline = true;
                    non_collapsed_max_ascent = non_collapsed_max_ascent.max(ascent);
                    non_collapsed_max_descent = non_collapsed_max_descent.max(descent);
                }
            } else if is_row && align == AlignItems::LastBaseline {
                has_last_baseline = true;
                let ascent = m.last_baseline_ascent;
                let descent = (outer_cross - ascent).max(0.0);
                max_last_ascent = max_last_ascent.max(ascent);
                max_last_descent = max_last_descent.max(descent);

                if !m.is_collapsed {
                    has_non_collapsed_last_baseline = true;
                    non_collapsed_max_last_ascent = non_collapsed_max_last_ascent.max(ascent);
                    non_collapsed_max_last_descent = non_collapsed_max_last_descent.max(descent);
                }
            } else {
                max_non_baseline_cross = max_non_baseline_cross.max(outer_cross);
            }
        }

        let first_group_cross = if has_baseline {
            max_ascent + max_descent
        } else {
            0.0
        };
        let last_group_cross = if has_last_baseline {
            max_last_ascent + max_last_descent
        } else {
            0.0
        };

        let mut l_cross = if !is_wrap && container_cross_size > 0.0 {
            container_cross_size
        } else {
            max_non_baseline_cross
                .max(first_group_cross)
                .max(last_group_cross)
        };

        if !is_wrap {
            l_cross = l_cross.max(min_cross_size).min(max_cross_size);
        }

        let non_collapsed_first_group_cross = if has_non_collapsed_baseline {
            non_collapsed_max_ascent + non_collapsed_max_descent
        } else {
            0.0
        };
        let non_collapsed_last_group_cross = if has_non_collapsed_last_baseline {
            non_collapsed_max_last_ascent + non_collapsed_max_last_descent
        } else {
            0.0
        };

        line_cross_sizes.push(l_cross);
        line_baseline_ascents.push(if has_non_collapsed_baseline {
            non_collapsed_max_ascent
        } else {
            0.0
        });
        line_last_baseline_ascents.push(if has_non_collapsed_last_baseline {
            non_collapsed_max_last_ascent
        } else {
            0.0
        });
        line_first_group_crosses.push(non_collapsed_first_group_cross);
        line_last_group_crosses.push(non_collapsed_last_group_cross);
    }

    LineCrossMetrics {
        cross_sizes: line_cross_sizes,
        first_ascents: line_baseline_ascents,
        last_ascents: line_last_baseline_ascents,
        first_group_crosses: line_first_group_crosses,
        last_group_crosses: line_last_group_crosses,
    }
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

    let is_wrap_reverse = style.flex_wrap == mango_css::values::FlexWrap::WrapReverse;
    if free_cross > 0.0 && is_wrap {
        match style.align_content {
            AlignContent::Normal if line_count == 1 => {
                if is_wrap_reverse {
                    (free_cross, cross_gap)
                } else {
                    (0.0, cross_gap)
                }
            }
            AlignContent::Normal | AlignContent::Stretch => {
                let extra_per_line = free_cross / line_count as f32;
                for size in line_cross_sizes.iter_mut() {
                    *size += extra_per_line;
                }
                (0.0, cross_gap)
            }
            AlignContent::FlexStart => {
                if is_wrap_reverse {
                    (free_cross, cross_gap)
                } else {
                    (0.0, cross_gap)
                }
            }
            AlignContent::FlexEnd => {
                if is_wrap_reverse {
                    (0.0, cross_gap)
                } else {
                    (free_cross, cross_gap)
                }
            }
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
    is_reverse: bool,
    is_container_main_definite: bool,
    container_main_size: f32,
    main_gap: f32,
) -> (f32, f32) {
    let _line_item_count = line.len();
    let non_collapsed_count = line
        .iter()
        .filter(|&&idx| !item_metrics[idx].is_collapsed)
        .count();
    let total_gaps = if non_collapsed_count > 1 {
        (non_collapsed_count - 1) as f32 * main_gap
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
    let remaining_free = if is_container_main_definite || container_main_size > 0.0 {
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
            JustifyContent::Start => {
                let is_dir_reverse = matches!(
                    style.flex_direction,
                    mango_css::values::FlexDirection::RowReverse
                        | mango_css::values::FlexDirection::ColumnReverse
                );
                let off = if is_dir_reverse { remaining_free } else { 0.0 };
                (off, main_gap)
            }
            JustifyContent::End => {
                let is_dir_reverse = matches!(
                    style.flex_direction,
                    mango_css::values::FlexDirection::RowReverse
                        | mango_css::values::FlexDirection::ColumnReverse
                );
                let off = if is_dir_reverse { 0.0 } else { remaining_free };
                (off, main_gap)
            }
            JustifyContent::Left => {
                let off = if is_row {
                    if is_reverse { remaining_free } else { 0.0 }
                } else {
                    let is_dir_reverse = matches!(
                        style.flex_direction,
                        mango_css::values::FlexDirection::RowReverse
                            | mango_css::values::FlexDirection::ColumnReverse
                    );
                    if is_dir_reverse { remaining_free } else { 0.0 }
                };
                (off, main_gap)
            }
            JustifyContent::Right => {
                let off = if is_row {
                    if is_reverse { 0.0 } else { remaining_free }
                } else {
                    let is_dir_reverse = matches!(
                        style.flex_direction,
                        mango_css::values::FlexDirection::RowReverse
                            | mango_css::values::FlexDirection::ColumnReverse
                    );
                    if is_dir_reverse { remaining_free } else { 0.0 }
                };
                (off, main_gap)
            }
            JustifyContent::SpaceBetween => {
                if non_collapsed_count > 1 {
                    (
                        0.0,
                        main_gap + remaining_free / (non_collapsed_count - 1) as f32,
                    )
                } else {
                    (0.0, main_gap)
                }
            }
            JustifyContent::SpaceAround => {
                if non_collapsed_count > 0 {
                    let per_item = remaining_free / non_collapsed_count as f32;
                    (per_item / 2.0, main_gap + per_item)
                } else {
                    (0.0, main_gap)
                }
            }
            JustifyContent::SpaceEvenly => {
                let spacing = (remaining_free + total_gaps) / (non_collapsed_count + 1) as f32;
                (spacing, spacing)
            }
        }
    }
}
