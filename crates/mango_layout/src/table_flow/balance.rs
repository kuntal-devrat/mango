//! Dynamic programming partition balancing for multi-column and intrinsic widths.

pub(crate) fn balance_intrinsic_max_content(sizes: &[f32], k: usize, gap: f32) -> f32 {
    let n = sizes.len();
    if n == 0 {
        return 0.0;
    }
    if k <= 1 || n <= 1 {
        let total: f32 = sizes.iter().sum();
        let gaps = if n > 1 { (n - 1) as f32 * gap } else { 0.0 };
        return total + gaps;
    }
    let k = k.min(n);

    let calc_len = |from: usize, to: usize| -> f32 {
        let items: f32 = sizes[from..to].iter().sum();
        let gaps = (to - from - 1) as f32 * gap;
        items + gaps
    };

    // dp[m][j] = (min_max_len: f32, counts: Vec<usize>)
    let mut dp: Vec<Vec<Option<(f32, Vec<usize>)>>> = vec![vec![None; n + 1]; k + 1];

    for j in 1..=(n - k + 1) {
        dp[1][j] = Some((calc_len(0, j), vec![j]));
    }

    for m in 2..=k {
        for j in m..=(n - k + m) {
            let mut best: Option<(f32, Vec<usize>)> = None;
            for p in (m - 1)..j {
                if let Some((prev_max, prev_counts)) = &dp[m - 1][p] {
                    let cur_len = calc_len(p, j);
                    let candidate_max = prev_max.max(cur_len);
                    let mut candidate_counts = prev_counts.clone();
                    candidate_counts.push(j - p);

                    let is_better = match &best {
                        None => true,
                        Some((best_max, best_counts)) => {
                            if candidate_max < *best_max - 1e-3 {
                                true
                            } else if (candidate_max - *best_max).abs() <= 1e-3 {
                                candidate_counts > *best_counts
                            } else {
                                false
                            }
                        }
                    };
                    if is_better {
                        best = Some((candidate_max, candidate_counts));
                    }
                }
            }
            dp[m][j] = best;
        }
    }

    dp[k][n]
        .as_ref()
        .map(|(val, _)| *val)
        .unwrap_or_else(|| sizes.iter().sum::<f32>() + (n - 1) as f32 * gap)
}

pub(crate) fn apply_min_max_width_bounds(
    node: &crate::box_tree::LayoutBox,
    font_size: f32,
    min_w: &mut f32,
    max_w: &mut f32,
) {
    let Some(style) = &node.style else {
        return;
    };
    let fs = style.font_size;
    let rfs = style.root_font_size;
    let non_content_h = super::intrinsic::get_node_horizontal_spacing(node, font_size);

    let min_bound = match style.min_width {
        mango_css::values::Length::MinContent => Some(*min_w),
        mango_css::values::Length::MaxContent | mango_css::values::Length::Content => Some(*max_w),
        mango_css::values::Length::Auto | mango_css::values::Length::Percent(_) => None,
        ref len => {
            let px = len.to_px(fs, rfs, 0.0);
            let w = if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                (px - non_content_h).max(0.0)
            } else {
                px
            };
            Some(w)
        }
    };

    let mut max_bound = match style.max_width {
        mango_css::values::Length::MinContent => Some(*min_w),
        mango_css::values::Length::MaxContent | mango_css::values::Length::Content => Some(*max_w),
        mango_css::values::Length::Auto | mango_css::values::Length::Percent(_) => None,
        ref len => {
            let px = len.to_px(fs, rfs, 0.0);
            let w = if style.box_sizing == mango_css::values::BoxSizing::BorderBox {
                (px - non_content_h).max(0.0)
            } else {
                px
            };
            Some(w)
        }
    };

    if let (Some(min_b), Some(max_b)) = (min_bound, max_bound)
        && min_b > max_b
    {
        max_bound = Some(min_b);
    }
    if let Some(mw) = max_bound {
        *max_w = (*max_w).min(mw);
        *min_w = (*min_w).min(mw);
    }
    if let Some(mw) = min_bound {
        *min_w = (*min_w).max(mw);
        *max_w = (*max_w).max(mw);
    }
}
