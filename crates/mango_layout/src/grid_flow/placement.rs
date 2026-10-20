use std::collections::HashSet;

use mango_css::computed::ComputedStyle;
use mango_css::values::GridPlacement;

use crate::box_tree::LayoutBox;

use super::types::PlacedItem;

pub fn find_area_bounds(areas: &[Vec<String>], name: &str) -> Option<(usize, usize, usize, usize)> {
    let lower = name.to_ascii_lowercase();
    let mut min_r = None;
    let mut max_r = 0;
    let mut min_c = None;
    let mut max_c = 0;

    for (r, row) in areas.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if cell == &lower {
                min_r = Some(min_r.map_or(r, |mr: usize| mr.min(r)));
                max_r = max_r.max(r);
                min_c = Some(min_c.map_or(c, |mc: usize| mc.min(c)));
                max_c = max_c.max(c);
            }
        }
    }

    match (min_r, min_c) {
        (Some(r1), Some(c1)) => Some((r1, c1, max_r + 1, max_c + 1)),
        _ => None,
    }
}

/// Resolves a 1-based CSS grid line index to a 0-based track index.
/// Positive lines (1-based) map directly to track indices (`line - 1`) without
/// clamping so that out-of-bounds lines expand implicit grid tracks (CSS Grid §8.5).
/// Negative lines count backwards from the end of the explicit grid.
pub fn resolve_line(line: i32, explicit_tracks: usize) -> usize {
    if line > 0 {
        (line - 1).max(0) as usize
    } else if line < 0 {
        let idx = (explicit_tracks as i32 + 1 + line).max(0);
        (idx as usize).min(explicit_tracks)
    } else {
        0
    }
}

/// Resolves a named grid line or area name to a 0-based track index.
///
/// First checks explicit named lines (`grid_column_lines` / `grid_row_lines`),
/// then falls back to `grid-template-areas` to derive implicit `-start` / `-end` lines.
///
/// `is_column` selects the axis, `is_end` selects the end boundary of an area.
pub fn resolve_grid_line_name(
    name: &str,
    named_lines: &[(String, usize)],
    areas: &[Vec<String>],
    is_column: bool,
    is_end: bool,
) -> Option<usize> {
    let lower = name.to_ascii_lowercase();

    // 1. Check explicit named lines (e.g. [main-start] 1fr [main-end])
    for (line_name, idx) in named_lines {
        if line_name.eq_ignore_ascii_case(&lower) {
            return Some(*idx);
        }
    }

    // 2. Check for implicit lines from named areas.
    //    A named area "foo" creates implicit lines "foo-start" and "foo-end".
    //    Also, placing `grid-area: foo` uses the area bounds directly.
    let area_name = lower
        .strip_suffix("-start")
        .or_else(|| lower.strip_suffix("-end"))
        .unwrap_or(&lower);

    if let Some(bounds) = find_area_bounds(areas, area_name) {
        let (row_start, col_start, row_end, col_end) = bounds;
        if lower.ends_with("-start") {
            return Some(if is_column { col_start } else { row_start });
        } else if lower.ends_with("-end") {
            return Some(if is_column { col_end } else { row_end });
        } else {
            // Direct area name: start gets start, end gets end
            return Some(if is_end {
                if is_column { col_end } else { row_end }
            } else {
                if is_column { col_start } else { row_start }
            });
        }
    }

    None
}

/// Resolves start and end grid placements into an optional explicit start track and span,
/// plus an auto-placement span (CSS Grid §8.3).
///
/// Handles `Line / Line` (including inverted lines requiring swap), `Line / Span`,
/// `Line / Auto`, `Span / Line` (reverse placement), `Auto / Line`, and named areas.
pub fn resolve_placement(
    start: &GridPlacement,
    end: &GridPlacement,
    explicit_tracks: usize,
    named_lines: &[(String, usize)],
    areas: &[Vec<String>],
    is_column: bool,
) -> (Option<(usize, usize)>, usize) {
    match (start, end) {
        (GridPlacement::Line(start_line), GridPlacement::Line(end_line)) => {
            let s = resolve_line(*start_line, explicit_tracks);
            let e = resolve_line(*end_line, explicit_tracks);
            if s <= e {
                let span = if e > s { e - s } else { 1 };
                (Some((s, span)), 1)
            } else {
                // Swapped lines per CSS Grid §8.3.2
                let span = s - e;
                (Some((e, span)), 1)
            }
        }
        (GridPlacement::Line(start_line), GridPlacement::Span(span_val)) => {
            let s = resolve_line(*start_line, explicit_tracks);
            (Some((s, (*span_val as usize).max(1))), 1)
        }
        (GridPlacement::Line(start_line), GridPlacement::Auto) => {
            let s = resolve_line(*start_line, explicit_tracks);
            (Some((s, 1)), 1)
        }
        (GridPlacement::Span(span_val), GridPlacement::Line(end_line)) => {
            let e = resolve_line(*end_line, explicit_tracks);
            let span = (*span_val as usize).max(1);
            let s = e.saturating_sub(span);
            (Some((s, span)), 1)
        }
        (GridPlacement::Auto, GridPlacement::Line(end_line)) => {
            let e = resolve_line(*end_line, explicit_tracks);
            let s = e.saturating_sub(1);
            (Some((s, 1)), 1)
        }
        (GridPlacement::Area(name), GridPlacement::Area(end_name)) => {
            let s = resolve_grid_line_name(name, named_lines, areas, is_column, false);
            let e = resolve_grid_line_name(end_name, named_lines, areas, is_column, true);
            match (s, e) {
                (Some(s_idx), Some(e_idx)) => {
                    if s_idx <= e_idx {
                        let span = if e_idx > s_idx { e_idx - s_idx } else { 1 };
                        (Some((s_idx, span)), 1)
                    } else {
                        (Some((e_idx, s_idx - e_idx)), 1)
                    }
                }
                (Some(s_idx), None) => (Some((s_idx, 1)), 1),
                (None, Some(e_idx)) => (Some((e_idx.saturating_sub(1), 1)), 1),
                _ => (None, 1),
            }
        }
        (GridPlacement::Area(name), GridPlacement::Span(span_val)) => {
            let s = resolve_grid_line_name(name, named_lines, areas, is_column, false);
            if let Some(s_idx) = s {
                (Some((s_idx, (*span_val as usize).max(1))), 1)
            } else {
                (None, (*span_val as usize).max(1))
            }
        }
        (GridPlacement::Span(span_val), GridPlacement::Area(name)) => {
            let e = resolve_grid_line_name(name, named_lines, areas, is_column, true);
            let span = (*span_val as usize).max(1);
            if let Some(e_idx) = e {
                (Some((e_idx.saturating_sub(span), span)), 1)
            } else {
                (None, span)
            }
        }
        (GridPlacement::Area(name), GridPlacement::Auto) => {
            let s = resolve_grid_line_name(name, named_lines, areas, is_column, false);
            let e = resolve_grid_line_name(name, named_lines, areas, is_column, true);
            match (s, e) {
                (Some(s_idx), Some(e_idx)) => {
                    let span = if e_idx > s_idx { e_idx - s_idx } else { 1 };
                    (Some((s_idx, span)), 1)
                }
                (Some(s_idx), None) => (Some((s_idx, 1)), 1),
                _ => (None, 1),
            }
        }
        (GridPlacement::Auto, GridPlacement::Area(name)) => {
            let e = resolve_grid_line_name(name, named_lines, areas, is_column, true);
            if let Some(e_idx) = e {
                (Some((e_idx.saturating_sub(1), 1)), 1)
            } else {
                (None, 1)
            }
        }
        (GridPlacement::Span(span_val), _) => (None, (*span_val as usize).max(1)),
        (_, GridPlacement::Span(span_val)) => (None, (*span_val as usize).max(1)),
        _ => (None, 1),
    }
}

/// Places in-flow grid items into grid cells according to CSS Grid §8.
/// Returns the list of placed items and the updated total number of columns.
pub fn place_grid_items(
    in_flow_indices: &[usize],
    container: &LayoutBox,
    style: &ComputedStyle,
    initial_num_cols: usize,
    explicit_row_count: usize,
) -> (Vec<PlacedItem>, usize) {
    let mut num_cols = initial_num_cols;
    let auto_flow = style.grid_auto_flow;
    let is_col_flow = auto_flow.is_column();
    let is_dense = auto_flow.is_dense();

    let mut placed_items = Vec::new();
    let mut occupied = HashSet::<(usize, usize)>::new();
    let mut auto_row = 0usize;
    let mut auto_col = 0usize;

    for &child_idx in in_flow_indices {
        let child = &container.children[child_idx];
        let child_style = child.style.clone().unwrap_or_default();

        let (col_explicit, col_auto_span) = resolve_placement(
            &child_style.grid_column_start,
            &child_style.grid_column_end,
            num_cols,
            &style.grid_column_lines,
            &style.grid_template_areas,
            true,
        );

        let (row_explicit, row_auto_span) = resolve_placement(
            &child_style.grid_row_start,
            &child_style.grid_row_end,
            explicit_row_count,
            &style.grid_row_lines,
            &style.grid_template_areas,
            false,
        );

        let (col_start, col_span, row_start, row_span) = match (col_explicit, row_explicit) {
            (Some((cs, cspan)), Some((rs, rspan))) => (cs, cspan, rs, rspan),
            (Some((cs, cspan)), None) => {
                let rspan = row_auto_span;
                let mut r = if is_dense { 0 } else { auto_row };
                loop {
                    let can_fit = (r..r + rspan)
                        .all(|ri| (cs..cs + cspan).all(|ci| !occupied.contains(&(ri, ci))));
                    if can_fit {
                        break;
                    }
                    r += 1;
                }
                (cs, cspan, r, rspan)
            }
            (None, Some((rs, rspan))) => {
                let cspan = col_auto_span;
                let mut c = if is_dense { 0 } else { auto_col };
                loop {
                    let can_fit = (rs..rs + rspan)
                        .all(|ri| (c..c + cspan).all(|ci| !occupied.contains(&(ri, ci))));
                    if can_fit {
                        break;
                    }
                    c += 1;
                }
                (c, cspan, rs, rspan)
            }
            (None, None) => {
                let cspan = col_auto_span;
                let rspan = row_auto_span;
                if is_dense {
                    if is_col_flow {
                        let mut found = (0, 0);
                        'dense_col: for c in 0..10_000 {
                            for r in 0..explicit_row_count.max(100) {
                                let can_fit = (r..r + rspan).all(|ri| {
                                    (c..c + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                                });
                                if can_fit {
                                    found = (c, r);
                                    break 'dense_col;
                                }
                            }
                        }
                        (found.0, cspan, found.1, rspan)
                    } else {
                        let cspan = cspan.min(num_cols.max(1));
                        let mut found = (0, 0);
                        'dense_row: for r in 0..10_000 {
                            for c in 0..num_cols {
                                if c + cspan > num_cols {
                                    continue;
                                }
                                let can_fit = (r..r + rspan).all(|ri| {
                                    (c..c + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                                });
                                if can_fit {
                                    found = (c, r);
                                    break 'dense_row;
                                }
                            }
                        }
                        (found.0, cspan, found.1, rspan)
                    }
                } else if is_col_flow {
                    let target_rows = explicit_row_count.max(1);
                    loop {
                        if auto_row + rspan > target_rows && auto_row > 0 {
                            auto_col += 1;
                            auto_row = 0;
                        }
                        let can_fit = (auto_row..auto_row + rspan).all(|ri| {
                            (auto_col..auto_col + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                        });
                        if can_fit {
                            let cs = auto_col;
                            let rs = auto_row;
                            auto_row += rspan;
                            break (cs, cspan, rs, rspan);
                        }
                        auto_row += 1;
                    }
                } else {
                    loop {
                        if auto_col + cspan > num_cols {
                            auto_row += 1;
                            auto_col = 0;
                        }
                        let can_fit = (auto_row..auto_row + rspan).all(|ri| {
                            (auto_col..auto_col + cspan).all(|ci| !occupied.contains(&(ri, ci)))
                        });
                        if can_fit {
                            let cs = auto_col;
                            let rs = auto_row;
                            auto_col += cspan;
                            break (cs, cspan, rs, rspan);
                        }
                        auto_row += 1;
                    }
                }
            }
        };

        for r in row_start..row_start + row_span {
            for c in col_start..col_start + col_span {
                occupied.insert((r, c));
            }
        }
        num_cols = num_cols.max(col_start + col_span);

        if !is_dense {
            if is_col_flow {
                auto_col = col_start;
                auto_row = row_start + row_span;
            } else {
                auto_col = col_start + col_span;
                if auto_col >= num_cols {
                    auto_row = row_start + row_span;
                    auto_col = 0;
                } else {
                    auto_row = row_start;
                }
            }
        }

        placed_items.push(PlacedItem {
            child_idx,
            col_start,
            col_span,
            row_start,
            row_span,
        });
    }

    (placed_items, num_cols)
}
