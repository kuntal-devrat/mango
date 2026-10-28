use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::{BoxSizing, Length};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::layout_flex_item;
use super::metrics::ItemMetric;

pub fn build_flex_lines(
    item_metrics: &[ItemMetric],
    style: &ComputedStyle,
    is_container_main_definite: bool,
    container_main_size: f32,
    main_gap: f32,
) -> Vec<Vec<usize>> {
    let n = item_metrics.len();
    if n == 0 {
        return Vec::new();
    }

    if !style.flex_wrap.is_wrap() {
        return vec![(0..n).collect()];
    }

    let floored_outer_sizes: Vec<f32> = item_metrics
        .iter()
        .map(|m| {
            let hypo = m.base_main.clamp(m.min_main, m.max_main.max(m.min_main));
            (hypo + m.pad_main + m.border_main + m.margin_main).max(0.0)
        })
        .collect();

    // Standard greedy wrapping when not balanced
    if !style.flex_wrap.is_balanced() {
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut current_line: Vec<usize> = Vec::new();
        let mut current_line_main = 0.0f32;

        for (metric_idx, &outer_item_main) in floored_outer_sizes.iter().enumerate() {
            let is_collapsed = item_metrics[metric_idx].is_collapsed;
            let item_gap = if current_line.is_empty() || is_collapsed {
                0.0
            } else {
                main_gap
            };

            if is_container_main_definite
                && !current_line.is_empty()
                && !is_collapsed
                && (current_line_main + item_gap + outer_item_main > container_main_size + 0.5)
            {
                lines.push(std::mem::take(&mut current_line));
                current_line_main = 0.0;
            }

            if !is_collapsed {
                current_line_main += if current_line.is_empty() {
                    0.0
                } else {
                    main_gap
                } + outer_item_main;
            }
            current_line.push(metric_idx);
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        return lines;
    }

    // CSS Flexbox Level 2 Line Balancing Algorithm (§ 9.7)
    // 1. Initial greedy line count calculation
    let greedy_line_count = if is_container_main_definite {
        let mut count = 0;
        let mut cur_line_main = 0.0f32;
        let mut cur_line_items = 0;

        for &size in &floored_outer_sizes {
            let gap = if cur_line_items == 0 { 0.0 } else { main_gap };
            let overflows_previous = cur_line_items > 0 && cur_line_main > container_main_size;
            let will_overflow =
                cur_line_items > 0 && (cur_line_main + gap + size > container_main_size + 0.5);

            if overflows_previous || will_overflow {
                count += 1;
                cur_line_main = size;
                cur_line_items = 1;
            } else {
                cur_line_main += gap + size;
                cur_line_items += 1;
            }
        }
        if cur_line_items > 0 {
            count += 1;
        }
        count.max(1)
    } else {
        1
    };

    // 2. Establish balancing target line count
    let mut target_k = greedy_line_count;
    if let Some(min_lines) = style.flex_line_count {
        target_k = target_k.max(min_lines as usize);
    }
    target_k = target_k.min(n).max(1);

    if target_k <= 1 {
        return vec![(0..n).collect()];
    }

    // 3. Dynamic programming to partition n items into target_k contiguous lines
    // minimizing the sum of squared errors across lines: (target_w - line_size)^2
    let target_w = if is_container_main_definite {
        container_main_size
    } else {
        0.0
    };

    let calc_line_size = |from: usize, to: usize| -> f32 {
        let items_len: f32 = floored_outer_sizes[from..to].iter().sum();
        let gaps_len = (to - from - 1) as f32 * main_gap;
        items_len + gaps_len
    };

    let is_slice_valid = |from: usize, to: usize| -> bool {
        if to - from == 1 {
            // A single item always fits on a line even if it overflows on its own
            true
        } else if is_container_main_definite {
            let l_size = calc_line_size(from, to);
            l_size <= container_main_size + 0.5
        } else {
            true
        }
    };

    let slice_error = |from: usize, to: usize| -> f64 {
        let l_size = calc_line_size(from, to);
        let diff = (target_w - l_size) as f64;
        diff * diff
    };

    let mut dp: Vec<Vec<Option<(f64, Vec<usize>)>>> = vec![vec![None; n + 1]; target_k + 1];

    for j in 1..=(n - target_k + 1) {
        if is_slice_valid(0, j) {
            dp[1][j] = Some((slice_error(0, j), vec![j]));
        }
    }

    for m in 2..=target_k {
        for j in m..=(n - target_k + m) {
            let mut best: Option<(f64, Vec<usize>)> = None;
            for p in (m - 1)..j {
                if let Some((prev_cost, prev_counts)) = &dp[m - 1][p] {
                    if is_slice_valid(p, j) {
                        let candidate_cost = prev_cost + slice_error(p, j);
                        let mut candidate_counts = prev_counts.clone();
                        candidate_counts.push(j - p);

                        let is_better = match &best {
                            None => true,
                            Some((best_cost, best_counts)) => {
                                if candidate_cost < best_cost - 1e-4 {
                                    true
                                } else if (candidate_cost - best_cost).abs() <= 1e-4 {
                                    // Spec tie-breaker: select the sequence that assigns
                                    // the most items to the first line, then second, etc.
                                    candidate_counts > *best_counts
                                } else {
                                    false
                                }
                            }
                        };
                        if is_better {
                            best = Some((candidate_cost, candidate_counts));
                        }
                    }
                }
            }
            dp[m][j] = best;
        }
    }

    if let Some((_, counts)) = dp[target_k][n].take() {
        let mut balanced_lines = Vec::with_capacity(target_k);
        let mut cur = 0;
        for c in counts {
            balanced_lines.push((cur..cur + c).collect());
            cur += c;
        }
        balanced_lines
    } else {
        // Fallback to greedy line construction if no partition satisfied all constraints
        let mut lines = Vec::new();
        let mut cur = Vec::new();
        let mut cur_size = 0.0f32;
        for (i, &size) in floored_outer_sizes.iter().enumerate() {
            let gap = if cur.is_empty() { 0.0 } else { main_gap };
            if is_container_main_definite
                && !cur.is_empty()
                && (cur_size + gap + size > container_main_size + 0.5)
            {
                lines.push(std::mem::take(&mut cur));
                cur_size = 0.0;
            }
            cur_size += if cur.is_empty() { 0.0 } else { main_gap } + size;
            cur.push(i);
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        lines
    }
}

pub fn resolve_flexible_lengths(
    lines: &[Vec<usize>],
    item_metrics: &mut [ItemMetric],
    container: &mut LayoutBox,
    is_row: bool,
    is_container_main_definite: bool,
    container_main_size: f32,
    container_height: f32,
    main_gap: f32,
    _style: &ComputedStyle,
    _is_percent_indefinite: bool,
) {
    for line in lines {
        let line_item_count = line.len();
        let non_collapsed_count = line
            .iter()
            .filter(|&&idx| !item_metrics[idx].is_collapsed)
            .count();
        let total_gaps = if non_collapsed_count > 1 {
            (non_collapsed_count - 1) as f32 * main_gap
        } else {
            0.0
        };

        if is_container_main_definite {
            let mut is_frozen = vec![false; line_item_count];
            let mut item_sizes: Vec<f32> = line
                .iter()
                .map(|&idx| item_metrics[idx].base_main)
                .collect();

            let initial_used: f32 = total_gaps
                + line
                    .iter()
                    .map(|&idx| {
                        let m = &item_metrics[idx];
                        m.base_main + m.pad_main + m.border_main + m.margin_main
                    })
                    .sum::<f32>();
            let initial_free = container_main_size - initial_used;

            for (i, &idx) in line.iter().enumerate() {
                let m = &item_metrics[idx];
                if initial_free >= 0.0 && m.flex_grow <= 0.0 {
                    is_frozen[i] = true;
                    item_sizes[i] = m.base_main.clamp(m.min_main, m.max_main.max(m.min_main));
                } else if initial_free < 0.0 && m.flex_shrink <= 0.0 {
                    is_frozen[i] = true;
                    item_sizes[i] = m.base_main.clamp(m.min_main, m.max_main.max(m.min_main));
                }
            }

            for _ in 0..line_item_count {
                let mut used_space = total_gaps;
                for (i, &idx) in line.iter().enumerate() {
                    let m = &item_metrics[idx];
                    let size = if is_frozen[i] {
                        item_sizes[i]
                    } else {
                        m.base_main
                    };
                    used_space += size + m.pad_main + m.border_main + m.margin_main;
                }
                let cur_free_space = container_main_size - used_space;

                if cur_free_space > 0.0 {
                    let sum_grow: f32 = line
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !is_frozen[*i])
                        .map(|(_, &idx)| item_metrics[idx].flex_grow)
                        .sum();

                    if sum_grow <= 0.0 {
                        break;
                    }

                    let mut has_violation = false;
                    for (i, &idx) in line.iter().enumerate() {
                        if is_frozen[i] {
                            continue;
                        }
                        let m = &item_metrics[idx];
                        if m.flex_grow > 0.0 {
                            let target = m.base_main + cur_free_space * (m.flex_grow / sum_grow);
                            if target > m.max_main {
                                item_sizes[i] = m.max_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else if target < m.min_main {
                                item_sizes[i] = m.min_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else {
                                item_sizes[i] = target;
                            }
                        }
                    }
                    if !has_violation {
                        break;
                    }
                } else if cur_free_space < 0.0 {
                    let sum_scaled_shrink: f32 = line
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !is_frozen[*i])
                        .map(|(_, &idx)| {
                            item_metrics[idx].flex_shrink * item_metrics[idx].base_main
                        })
                        .sum();

                    if sum_scaled_shrink <= 0.0 {
                        break;
                    }

                    let mut has_violation = false;
                    for (i, &idx) in line.iter().enumerate() {
                        if is_frozen[i] {
                            continue;
                        }
                        let m = &item_metrics[idx];
                        if m.flex_shrink > 0.0 {
                            let sub = (cur_free_space.abs())
                                * (m.flex_shrink * m.base_main / sum_scaled_shrink);
                            let target = m.base_main - sub;
                            if target < m.min_main {
                                item_sizes[i] = m.min_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else if target > m.max_main {
                                item_sizes[i] = m.max_main;
                                is_frozen[i] = true;
                                has_violation = true;
                            } else {
                                item_sizes[i] = target.max(0.0);
                            }
                        }
                    }
                    if !has_violation {
                        break;
                    }
                } else {
                    break;
                }
            }

            for (i, &idx) in line.iter().enumerate() {
                let (min_m, max_m) = (item_metrics[idx].min_main, item_metrics[idx].max_main);
                item_metrics[idx].final_main = item_sizes[i].clamp(min_m, max_m.max(min_m));
            }
        }

        // If row container, measure items whose height is Auto to resolve actual cross_size
        if is_row {
            let eff_line_cross = if _style.flex_wrap.is_balanced()
                && container_height > 0.0
                && !lines.is_empty()
            {
                let cross_gap =
                    resolve_flex_cross_gap(_style, is_row, container_main_size, container_height);
                (container_height - (lines.len() - 1) as f32 * cross_gap).max(0.0)
                    / lines.len() as f32
            } else {
                0.0
            };
            for &idx in line {
                let m = &mut item_metrics[idx];
                let child = &mut container.children[m.index];
                let child_style = child.style.clone().unwrap_or_default();
                let is_child_h_auto = child_style.height == Length::Auto
                    || (matches!(child_style.height, Length::Percent(_))
                        && (container_height <= 0.0
                            || container
                                .style
                                .as_ref()
                                .map(|s| s.height == Length::Auto)
                                .unwrap_or(true)));
                if is_child_h_auto {
                    child.dimensions.content.size.width = m.final_main;
                    let item_cb = Dimensions::new(Rect::new(
                        0.0,
                        0.0,
                        m.final_main + m.pad_main + m.border_main,
                        eff_line_cross,
                    ));
                    let mut item_float_ctx = FloatContext::new();
                    let c_fs = child_style.font_size;
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    let aspect_ratio =
                        child_style.aspect_ratio.filter(|&r| r > 0.0).or_else(|| {
                            match &child.box_type {
                                BoxType::ReplacedElement {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::IFrame {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::Video {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::Audio {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::Canvas {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                } if *intrinsic_height > 0.0 => {
                                    Some(*intrinsic_width / *intrinsic_height)
                                }
                                _ => None,
                            }
                        });
                    if let Some(ratio) = aspect_ratio {
                        let mut h = (m.final_main / ratio).max(0.0);
                        let c_root_fs = child_style.root_font_size;
                        if child_style.min_height != Length::Auto {
                            let min_h = child_style.min_height.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                container_height,
                                container_height,
                            );
                            let min_content = if child_style.box_sizing == BoxSizing::BorderBox {
                                (min_h - m.pad_cross - m.border_cross).max(0.0)
                            } else {
                                min_h
                            };
                            h = h.max(min_content);
                        }
                        if child_style.max_height != Length::Auto {
                            let max_h = child_style.max_height.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                container_height,
                                container_height,
                            );
                            let max_content = if child_style.box_sizing == BoxSizing::BorderBox {
                                (max_h - m.pad_cross - m.border_cross).max(0.0)
                            } else {
                                max_h
                            };
                            h = h.min(max_content);
                        }
                        m.cross = h;
                        child.dimensions.content.size.height = h;
                    } else if child.dimensions.content.size.height > 0.0 {
                        m.cross = child.dimensions.content.size.height;
                    }
                }
            }
        } else {
            for &idx in line {
                let m = &mut item_metrics[idx];
                let child = &mut container.children[m.index];
                let child_style = child.style.clone().unwrap_or_default();
                let is_child_w_auto = child_style.width == Length::Auto
                    || matches!(child_style.width, Length::Percent(p) if p >= 99.9);
                if is_child_w_auto {
                    let aspect_ratio =
                        child_style.aspect_ratio.filter(|&r| r > 0.0).or_else(|| {
                            match &child.box_type {
                                BoxType::ReplacedElement {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::IFrame {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::Video {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::Audio {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                }
                                | BoxType::Canvas {
                                    intrinsic_width,
                                    intrinsic_height,
                                    ..
                                } if *intrinsic_height > 0.0 => {
                                    Some(*intrinsic_width / *intrinsic_height)
                                }
                                _ => None,
                            }
                        });
                    if let Some(ratio) = aspect_ratio {
                        let mut w = (m.final_main * ratio).max(0.0);
                        let c_fs = child_style.font_size;
                        let c_root_fs = child_style.root_font_size;
                        if child_style.min_width != Length::Auto {
                            let min_w = child_style.min_width.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                container_main_size,
                                container_height,
                            );
                            let min_content = if child_style.box_sizing == BoxSizing::BorderBox {
                                (min_w - m.pad_cross - m.border_cross).max(0.0)
                            } else {
                                min_w
                            };
                            w = w.max(min_content);
                        }
                        if child_style.max_width != Length::Auto {
                            let max_w = child_style.max_width.to_px_with_viewport(
                                c_fs,
                                c_root_fs,
                                container_main_size,
                                container_height,
                            );
                            let max_content = if child_style.box_sizing == BoxSizing::BorderBox {
                                (max_w - m.pad_cross - m.border_cross).max(0.0)
                            } else {
                                max_w
                            };
                            w = w.min(max_content);
                        }
                        m.cross = w;
                        child.dimensions.content.size.width = w;
                    }
                }
            }
        }
    }
}

/// Resolves the flex container's main-axis gap (column-gap for row, row-gap for column),
/// handling percentage resolution against the inner main-axis size of the flex container.
/// Per CSS Box Alignment Level 3 §8, percentage gaps resolve against the container's
/// inner main size if definite, or 0.0 if indefinite.
pub fn resolve_flex_main_gap(
    style: &ComputedStyle,
    is_row: bool,
    container_main_size: f32,
    container_height: f32,
    is_container_main_definite: bool,
) -> f32 {
    let gap_length = if is_row {
        &style.column_gap
    } else {
        &style.row_gap
    };

    if !is_container_main_definite && gap_length.has_percent() {
        return 0.0;
    }

    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    gap_length
        .to_px_with_viewport(
            font_size,
            root_font_size,
            container_main_size,
            container_height,
        )
        .max(0.0)
}

/// Resolves the flex container's cross-axis gap (row-gap for row, column-gap for column),
/// handling percentage resolution against the inner cross-axis size of the flex container.
/// If the cross-axis size is indefinite (e.g. height is 0/auto for row containers),
/// percentage gaps resolve to 0.0.
pub fn resolve_flex_cross_gap(
    style: &ComputedStyle,
    is_row: bool,
    container_cross_size: f32,
    container_height: f32,
) -> f32 {
    let gap_length = if is_row {
        &style.row_gap
    } else {
        &style.column_gap
    };

    if (container_cross_size <= 0.0 || (is_row && container_height <= 0.0))
        && gap_length.has_percent()
    {
        return 0.0;
    }

    let font_size = style.font_size;
    let root_font_size = style.root_font_size;
    gap_length
        .to_px_with_viewport(
            font_size,
            root_font_size,
            container_cross_size,
            container_height,
        )
        .max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_flex_main_gap_row_percentage() {
        let mut style = ComputedStyle::default();
        style.column_gap = Length::Percent(10.0);
        let gap = resolve_flex_main_gap(&style, true, 200.0, 100.0, true);
        assert_eq!(gap, 20.0);
    }

    #[test]
    fn test_resolve_flex_main_gap_column_percentage() {
        let mut style = ComputedStyle::default();
        style.row_gap = Length::Percent(15.0);
        let gap = resolve_flex_main_gap(&style, false, 300.0, 300.0, true);
        assert_eq!(gap, 45.0);
    }

    #[test]
    fn test_resolve_flex_main_gap_indefinite_resolves_zero() {
        let mut style = ComputedStyle::default();
        style.column_gap = Length::Percent(10.0);
        let gap = resolve_flex_main_gap(&style, true, 200.0, 100.0, false);
        assert_eq!(gap, 0.0);

        // Fixed length still resolves even if indefinite
        style.column_gap = Length::Px(15.0);
        let gap_fixed = resolve_flex_main_gap(&style, true, 200.0, 100.0, false);
        assert_eq!(gap_fixed, 15.0);
    }

    #[test]
    fn test_resolve_flex_cross_gap_definite_and_indefinite() {
        let mut style = ComputedStyle::default();
        style.row_gap = Length::Percent(10.0);
        // Definite row cross size (height 200px)
        let gap = resolve_flex_cross_gap(&style, true, 200.0, 200.0);
        assert_eq!(gap, 20.0);

        // Indefinite row cross size (height 0.0) -> resolves to 0.0
        let gap_indef = resolve_flex_cross_gap(&style, true, 0.0, 0.0);
        assert_eq!(gap_indef, 0.0);
    }
}
