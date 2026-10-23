use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::Length;

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::item::layout_flex_item;
use super::metrics::ItemMetric;

pub fn build_flex_lines(
    item_metrics: &[ItemMetric],
    is_wrap: bool,
    is_container_main_definite: bool,
    container_main_size: f32,
    main_gap: f32,
) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current_line: Vec<usize> = Vec::new();
    let mut current_line_main = 0.0f32;

    for (metric_idx, metric) in item_metrics.iter().enumerate() {
        let outer_item_main =
            metric.base_main + metric.pad_main + metric.border_main + metric.margin_main;
        let item_gap = if current_line.is_empty() {
            0.0
        } else {
            main_gap
        };

        if is_wrap
            && is_container_main_definite
            && !current_line.is_empty()
            && (current_line_main + item_gap + outer_item_main > container_main_size + 0.5)
        {
            lines.push(std::mem::take(&mut current_line));
            current_line_main = 0.0;
        }

        current_line_main += if current_line.is_empty() {
            0.0
        } else {
            main_gap
        } + outer_item_main;
        current_line.push(metric_idx);
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
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
        let total_gaps = if line_item_count > 1 {
            (line_item_count - 1) as f32 * main_gap
        } else {
            0.0
        };

        if is_container_main_definite {
            let mut is_frozen = vec![false; line_item_count];
            let mut item_sizes: Vec<f32> = line
                .iter()
                .map(|&idx| item_metrics[idx].base_main)
                .collect();

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
                item_metrics[idx].final_main = item_sizes[i];
            }
        }

        // If row container, measure items whose height is Auto to resolve actual cross_size
        if is_row {
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
                        0.0,
                    ));
                    let mut item_float_ctx = FloatContext::new();
                    let c_fs = child_style.font_size;
                    layout_flex_item(child, &item_cb, &mut item_float_ctx, c_fs);
                    if child.dimensions.content.size.height > 0.0 {
                        m.cross = child.dimensions.content.size.height;
                    }
                }
            }
        }
    }
}
