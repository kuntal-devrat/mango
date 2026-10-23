use std::sync::Arc;

use mango_css::values::{Display, FlexDirection, FlexWrap, Length, Position, WritingMode};

use crate::box_tree::LayoutBox;

/// Applies CSS Box Model Level 4 `margin-trim` rules to container children.
pub fn apply_margin_trim(root_box: &mut LayoutBox) {
    let trim_str = match &root_box.style {
        Some(s) if !s.margin_trim.is_empty() && s.margin_trim != "none" => s.margin_trim.clone(),
        _ => return,
    };

    let tokens: Vec<&str> = trim_str.split_whitespace().collect();
    let trim_block_start = tokens.iter().any(|&t| t == "block" || t == "block-start");
    let trim_block_end = tokens.iter().any(|&t| t == "block" || t == "block-end");
    let trim_inline_start = tokens.iter().any(|&t| t == "inline" || t == "inline-start");
    let trim_inline_end = tokens.iter().any(|&t| t == "inline" || t == "inline-end");

    if !trim_block_start && !trim_block_end && !trim_inline_start && !trim_inline_end {
        return;
    }

    let is_vertical = root_box
        .style
        .as_ref()
        .map(|s| {
            matches!(
                s.writing_mode,
                WritingMode::VerticalLr | WritingMode::VerticalRl
            )
        })
        .unwrap_or(false);

    let in_flow_indices: Vec<usize> = root_box
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let pos = c
                .style
                .as_ref()
                .map(|s| s.position)
                .unwrap_or(Position::Static);
            if matches!(pos, Position::Absolute | Position::Fixed) {
                None
            } else {
                Some(i)
            }
        })
        .collect();

    if in_flow_indices.is_empty() {
        return;
    }

    let display = root_box
        .style
        .as_ref()
        .map(|s| s.display)
        .unwrap_or(Display::Block);
    let is_flex = display == Display::Flex || display == Display::InlineFlex;
    let is_grid = display == Display::Grid || display == Display::InlineGrid;

    if is_flex {
        trim_flex_margins(
            root_box,
            &in_flow_indices,
            trim_block_start,
            trim_block_end,
            trim_inline_start,
            trim_inline_end,
            is_vertical,
        );
    } else if is_grid {
        trim_grid_margins(
            root_box,
            &in_flow_indices,
            trim_block_start,
            trim_block_end,
            trim_inline_start,
            trim_inline_end,
            is_vertical,
        );
    } else {
        trim_block_container_margins(
            root_box,
            &in_flow_indices,
            trim_block_start,
            trim_block_end,
            trim_inline_start,
            trim_inline_end,
            is_vertical,
        );
    }
}

fn trim_block_container_margins(
    root_box: &mut LayoutBox,
    in_flow_indices: &[usize],
    trim_block_start: bool,
    trim_block_end: bool,
    trim_inline_start: bool,
    trim_inline_end: bool,
    is_vertical: bool,
) {
    if trim_block_start {
        for &idx in in_flow_indices {
            let child = &mut root_box.children[idx];
            zero_block_start(child, is_vertical);
            if is_self_collapsing(child) {
                zero_block_end(child, is_vertical);
            } else {
                break;
            }
        }
    }

    if trim_block_end {
        for &idx in in_flow_indices.iter().rev() {
            let child = &mut root_box.children[idx];
            zero_block_end(child, is_vertical);
            if is_self_collapsing(child) {
                zero_block_start(child, is_vertical);
            } else {
                break;
            }
        }
    }

    if trim_inline_start {
        for &idx in in_flow_indices {
            zero_inline_start(&mut root_box.children[idx], is_vertical);
        }
    }

    if trim_inline_end {
        for &idx in in_flow_indices {
            zero_inline_end(&mut root_box.children[idx], is_vertical);
        }
    }
}

fn trim_flex_margins(
    root_box: &mut LayoutBox,
    in_flow_indices: &[usize],
    trim_block_start: bool,
    trim_block_end: bool,
    trim_inline_start: bool,
    trim_inline_end: bool,
    is_vertical: bool,
) {
    let (is_column, is_wrap, container_main_limit) = match &root_box.style {
        Some(s) => {
            let col = matches!(
                s.flex_direction,
                FlexDirection::Column | FlexDirection::ColumnReverse
            );
            let wrap = s.flex_wrap != FlexWrap::NoWrap;
            let limit = if col {
                match s.height {
                    Length::Px(h) => h,
                    _ => f32::INFINITY,
                }
            } else {
                match s.width {
                    Length::Px(w) => w,
                    _ => f32::INFINITY,
                }
            };
            (col, wrap, limit)
        }
        None => (false, false, f32::INFINITY),
    };

    let mut lines: Vec<Vec<usize>> = Vec::new();
    if !is_wrap {
        lines.push(in_flow_indices.to_vec());
    } else {
        let mut cur_line = Vec::new();
        let mut cur_main = 0.0f32;
        for &idx in in_flow_indices {
            let item = &root_box.children[idx];
            let item_main = if is_column {
                item.style
                    .as_ref()
                    .and_then(|s| match s.height {
                        Length::Px(h) => Some(h),
                        _ => None,
                    })
                    .unwrap_or(50.0)
            } else {
                item.style
                    .as_ref()
                    .and_then(|s| match s.width {
                        Length::Px(w) => Some(w),
                        _ => None,
                    })
                    .unwrap_or(50.0)
            };

            if !cur_line.is_empty() && cur_main + item_main > container_main_limit + 0.1 {
                lines.push(std::mem::take(&mut cur_line));
                cur_main = 0.0;
            }
            cur_line.push(idx);
            cur_main += item_main;
        }
        if !cur_line.is_empty() {
            lines.push(cur_line);
        }
    }

    if lines.is_empty() {
        return;
    }

    let first_line = lines[0].clone();
    let last_line = lines[lines.len() - 1].clone();

    if !is_column {
        // Flex Row: cross axis = block, main axis = inline
        if trim_block_start {
            for idx in first_line {
                zero_block_start(&mut root_box.children[idx], is_vertical);
            }
        }
        if trim_block_end {
            for idx in last_line {
                zero_block_end(&mut root_box.children[idx], is_vertical);
            }
        }
        if trim_inline_start {
            for line in &lines {
                if let Some(&first_idx) = line.first() {
                    zero_inline_start(&mut root_box.children[first_idx], is_vertical);
                }
            }
        }
        if trim_inline_end {
            for line in &lines {
                if let Some(&last_idx) = line.last() {
                    zero_inline_end(&mut root_box.children[last_idx], is_vertical);
                }
            }
        }
    } else {
        // Flex Column: main axis = block, cross axis = inline
        if trim_block_start {
            for line in &lines {
                if let Some(&first_idx) = line.first() {
                    zero_block_start(&mut root_box.children[first_idx], is_vertical);
                }
            }
        }
        if trim_block_end {
            for line in &lines {
                if let Some(&last_idx) = line.last() {
                    zero_block_end(&mut root_box.children[last_idx], is_vertical);
                }
            }
        }
        if trim_inline_start {
            for idx in first_line {
                zero_inline_start(&mut root_box.children[idx], is_vertical);
            }
        }
        if trim_inline_end {
            for idx in last_line {
                zero_inline_end(&mut root_box.children[idx], is_vertical);
            }
        }
    }
}

fn trim_grid_margins(
    root_box: &mut LayoutBox,
    in_flow_indices: &[usize],
    trim_block_start: bool,
    trim_block_end: bool,
    trim_inline_start: bool,
    trim_inline_end: bool,
    is_vertical: bool,
) {
    let col_count = root_box
        .style
        .as_ref()
        .map(|s| s.grid_template_columns.len().max(1))
        .unwrap_or(1);

    // Simulate auto-placement of grid items into tracks
    let mut occupied = vec![vec![false; col_count]; 64];
    let mut placements: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut cur_row = 0;
    let mut cur_col = 0;
    let mut max_end_row = 0;

    for &idx in in_flow_indices {
        let child = &root_box.children[idx];
        let r_span = child
            .style
            .as_ref()
            .and_then(|s| match &s.grid_row_end {
                mango_css::values::GridPlacement::Span(n) => Some(*n as usize),
                _ => match &s.grid_row_start {
                    mango_css::values::GridPlacement::Span(n) => Some(*n as usize),
                    _ => None,
                },
            })
            .unwrap_or(1)
            .max(1);
        let c_span = child
            .style
            .as_ref()
            .and_then(|s| match &s.grid_column_end {
                mango_css::values::GridPlacement::Span(n) => Some(*n as usize),
                _ => match &s.grid_column_start {
                    mango_css::values::GridPlacement::Span(n) => Some(*n as usize),
                    _ => None,
                },
            })
            .unwrap_or(1)
            .max(1)
            .min(col_count);

        while cur_col + c_span > col_count || (0..c_span).any(|dc| occupied[cur_row][cur_col + dc])
        {
            cur_col += 1;
            if cur_col >= col_count {
                cur_col = 0;
                cur_row += 1;
                while cur_row + r_span >= occupied.len() {
                    occupied.push(vec![false; col_count]);
                }
            }
        }

        while cur_row + r_span >= occupied.len() {
            occupied.push(vec![false; col_count]);
        }
        for dr in 0..r_span {
            for dc in 0..c_span {
                occupied[cur_row + dr][cur_col + dc] = true;
            }
        }
        placements.push((cur_row, cur_col, r_span, c_span));
        max_end_row = max_end_row.max(cur_row + r_span);
        cur_col += c_span;
        if cur_col >= col_count {
            cur_col = 0;
            cur_row += 1;
        }
    }

    for (item_pos, &idx) in in_flow_indices.iter().enumerate() {
        let (row, col, r_span, c_span) = placements[item_pos];
        let is_first_row = row == 0;
        let is_last_row = row + r_span == max_end_row;
        let is_first_col = col == 0;
        let is_last_col = col + c_span >= col_count;

        let child = &mut root_box.children[idx];
        if trim_block_start && is_first_row {
            zero_block_start(child, is_vertical);
        }
        if trim_block_end && is_last_row {
            zero_block_end(child, is_vertical);
        }
        if trim_inline_start && is_first_col {
            zero_inline_start(child, is_vertical);
        }
        if trim_inline_end && is_last_col {
            zero_inline_end(child, is_vertical);
        }
    }
}

fn is_self_collapsing(b: &LayoutBox) -> bool {
    if b.is_inline() {
        return false;
    }
    let pos = b
        .style
        .as_ref()
        .map(|s| s.position)
        .unwrap_or(Position::Static);
    if matches!(pos, Position::Absolute | Position::Fixed) {
        return false;
    }
    let style = match &b.style {
        Some(s) => s,
        None => return true,
    };
    if style.height != Length::Auto && style.height != Length::Px(0.0) {
        return false;
    }
    if style.min_height != Length::Auto && style.min_height != Length::Px(0.0) {
        return false;
    }
    if style.padding_top != Length::Px(0.0)
        || style.padding_bottom != Length::Px(0.0)
        || style.border_top_width > 0.0
        || style.border_bottom_width > 0.0
    {
        return false;
    }
    b.children.is_empty() || b.children.iter().all(is_self_collapsing)
}

fn zero_block_start(child: &mut LayoutBox, is_vertical: bool) {
    if is_vertical {
        zero_margin_left(child);
    } else {
        zero_margin_top(child);
    }
}

fn zero_block_end(child: &mut LayoutBox, is_vertical: bool) {
    if is_vertical {
        zero_margin_right(child);
    } else {
        zero_margin_bottom(child);
    }
}

fn zero_inline_start(child: &mut LayoutBox, is_vertical: bool) {
    if is_vertical {
        zero_margin_top(child);
    } else {
        zero_margin_left(child);
    }
}

fn zero_inline_end(child: &mut LayoutBox, is_vertical: bool) {
    if is_vertical {
        zero_margin_bottom(child);
    } else {
        zero_margin_right(child);
    }
}

fn zero_margin_top(child: &mut LayoutBox) {
    if let Some(style) = &mut child.style {
        Arc::make_mut(style).margin_top = Length::Px(0.0);
    }
    child.dimensions.margin.top = 0.0;
}

fn zero_margin_bottom(child: &mut LayoutBox) {
    if let Some(style) = &mut child.style {
        Arc::make_mut(style).margin_bottom = Length::Px(0.0);
    }
    child.dimensions.margin.bottom = 0.0;
}

fn zero_margin_left(child: &mut LayoutBox) {
    if let Some(style) = &mut child.style {
        Arc::make_mut(style).margin_left = Length::Px(0.0);
    }
    child.dimensions.margin.left = 0.0;
}

fn zero_margin_right(child: &mut LayoutBox) {
    if let Some(style) = &mut child.style {
        Arc::make_mut(style).margin_right = Length::Px(0.0);
    }
    child.dimensions.margin.right = 0.0;
}
