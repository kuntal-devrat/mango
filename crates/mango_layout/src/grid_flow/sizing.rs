use mango_css::values::{BoxSizing, Display, FlexDirection, GridTrackSize, Length, Position};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::placement::place_grid_items;
use super::types::PlacedItem;

/// Returns the minimum size (in px) of a track specification, used for
/// calculating how many repetitions fit in `repeat(auto-fill/auto-fit, ...)`.
pub fn track_min_size(
    track: &GridTrackSize,
    font_size: f32,
    container_width: f32,
    container_height: f32,
) -> f32 {
    match track {
        GridTrackSize::Length(l) => {
            l.to_px_with_viewport(font_size, 16.0, container_width, container_height)
        }
        GridTrackSize::MinMax(min_t, _) => {
            track_min_size(min_t, font_size, container_width, container_height)
        }
        GridTrackSize::Fr(_) => 0.0,
        GridTrackSize::Auto | GridTrackSize::MinContent | GridTrackSize::MaxContent => 0.0,
        GridTrackSize::Subgrid => 0.0,
        GridTrackSize::RepeatAutoFill(inner) => {
            track_min_size(inner, font_size, container_width, container_height)
        }
        GridTrackSize::RepeatAutoFit(inner) => {
            track_min_size(inner, font_size, container_width, container_height)
        }
    }
}

/// Returns the resolved minimum pixel size for a `minmax()` track's minimum component.
/// For `min-content` / `max-content`, measures the column's child content.
pub fn track_min_px(
    min_track: &GridTrackSize,
    font_size: f32,
    container_width: f32,
    container_height: f32,
    col: usize,
    placed_items: &[PlacedItem],
    container: &LayoutBox,
) -> f32 {
    match min_track {
        GridTrackSize::Length(l) => {
            l.to_px_with_viewport(font_size, 16.0, container_width, container_height)
        }
        GridTrackSize::MinContent => {
            measure_col_content_min_width(col, placed_items, container, font_size)
        }
        GridTrackSize::MaxContent => {
            measure_col_content_max_width(col, placed_items, container, font_size)
        }
        GridTrackSize::Auto => {
            measure_col_content_min_width(col, placed_items, container, font_size)
        }
        _ => 0.0,
    }
}

/// Measures the maximum intrinsic content width of all items placed in the given column.
pub fn measure_col_content_max_width(
    col: usize,
    placed_items: &[PlacedItem],
    container: &LayoutBox,
    font_size: f32,
) -> f32 {
    let mut max_w = 0.0f32;
    for item in placed_items {
        if item.col_start <= col && col < item.col_start + item.col_span {
            let child = &container.children[item.child_idx];
            let w = child_intrinsic_width(child, font_size, false);
            // For multi-column-span items, attribute a proportional share
            let share = if item.col_span > 1 {
                w / item.col_span as f32
            } else {
                w
            };
            max_w = max_w.max(share);
        }
    }
    max_w
}

/// Measures the minimum intrinsic content width of all items placed in the given column.
pub fn measure_col_content_min_width(
    col: usize,
    placed_items: &[PlacedItem],
    container: &LayoutBox,
    font_size: f32,
) -> f32 {
    let mut max_w = 0.0f32;
    for item in placed_items {
        if item.col_start <= col && col < item.col_start + item.col_span {
            let child = &container.children[item.child_idx];
            let w = child_intrinsic_width(child, font_size, true);
            let share = if item.col_span > 1 {
                w / item.col_span as f32
            } else {
                w
            };
            max_w = max_w.max(share);
        }
    }
    max_w
}

/// Returns the intrinsic width of a child layout box.
/// When `is_min` is true, returns the min-content width (longest unbreakable word);
/// when false, returns the max-content width (unconstrained line).
pub fn child_intrinsic_width(child: &LayoutBox, font_size: f32, is_min: bool) -> f32 {
    let non_content_h = if let Some(s) = &child.style {
        let fs = s.font_size;
        let rfs = s.root_font_size;
        let pl = s.padding_left.to_px(fs, rfs, 0.0);
        let pr = s.padding_right.to_px(fs, rfs, 0.0);
        let bl = s.border_left_width;
        let br = s.border_right_width;
        let ml = match s.margin_left {
            Length::Px(px) if px > 0.0 => px,
            Length::Rem(rem) if rem > 0.0 => rem * rfs,
            Length::Em(em) if em > 0.0 => em * fs,
            _ => 0.0,
        };
        let mr = match s.margin_right {
            Length::Px(px) if px > 0.0 => px,
            Length::Rem(rem) if rem > 0.0 => rem * rfs,
            Length::Em(em) if em > 0.0 => em * fs,
            _ => 0.0,
        };
        pl + pr + bl + br + ml + mr
    } else {
        0.0
    };

    // If the child has an explicit width, use that.
    // Indefinite percentage widths must be treated as auto for intrinsic sizing (CSS Sizing 3 §5.1).
    if let Some(cs) = &child.style
        && cs.width != Length::Auto
        && !matches!(cs.width, Length::Percent(_))
    {
        let px = cs.width.to_px_with_viewport(
            cs.font_size,
            16.0,
            0.0, // no percentage basis for intrinsic measurement
            0.0,
        );
        let res = if cs.box_sizing == BoxSizing::BorderBox {
            let fs = cs.font_size;
            let rfs = cs.root_font_size;
            let ml = match cs.margin_left {
                Length::Px(px) if px > 0.0 => px,
                Length::Rem(rem) if rem > 0.0 => rem * rfs,
                Length::Em(em) if em > 0.0 => em * fs,
                _ => 0.0,
            };
            let mr = match cs.margin_right {
                Length::Px(px) if px > 0.0 => px,
                Length::Rem(rem) if rem > 0.0 => rem * rfs,
                Length::Em(em) if em > 0.0 => em * fs,
                _ => 0.0,
            };
            px + ml + mr
        } else {
            px + non_content_h
        };
        return res;
    }

    let fs = child
        .style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or(font_size);
    let content_w = match &child.box_type {
        BoxType::TextNode(t) => {
            let (weight, family, letter_spacing) = if let Some(s) = &child.style {
                let bold = crate::inline_flow::is_bold_weight(s.font_weight, s.font_size);
                let w = if bold {
                    mango_render::FontWeight::Bold
                } else {
                    mango_render::FontWeight::Regular
                };
                let f = mango_render::FontFamily::from_css_name(&s.font_family);
                let ls = s.letter_spacing.to_px(s.font_size, s.root_font_size, 0.0);
                (w, f, ls)
            } else {
                (
                    mango_render::FontWeight::Regular,
                    mango_render::FontFamily::SansSerif,
                    0.0,
                )
            };
            if is_min {
                // Min-content: longest single word
                t.split_whitespace()
                    .map(|word| {
                        crate::inline_flow::measure_text_width_with_style_and_spacing(
                            word,
                            fs,
                            weight,
                            family,
                            letter_spacing,
                        )
                    })
                    .fold(0.0f32, f32::max)
            } else {
                crate::inline_flow::measure_text_width_with_style_and_spacing(
                    t,
                    fs,
                    weight,
                    family,
                    letter_spacing,
                )
            }
        }
        BoxType::ReplacedElement {
            intrinsic_width, ..
        }
        | BoxType::IFrame {
            intrinsic_width, ..
        }
        | BoxType::Video {
            intrinsic_width, ..
        }
        | BoxType::Audio {
            intrinsic_width, ..
        }
        | BoxType::Canvas {
            intrinsic_width, ..
        } => *intrinsic_width,
        _ => {
            // For block/inline containers, recurse into children.
            // In vertical formatting contexts (blocks, column flex), boxes stack vertically,
            // so container width is the max of the child widths, not the sum.
            let has_blocks = child.children.iter().any(|c| !c.is_inline());
            let is_column_flex = child
                .style
                .as_ref()
                .map(|s| {
                    matches!(s.display, Display::Flex | Display::InlineFlex)
                        && matches!(
                            s.flex_direction,
                            FlexDirection::Column | FlexDirection::ColumnReverse
                        )
                })
                .unwrap_or(false);

            let is_vertical = has_blocks || is_column_flex;

            let mut total = 0.0f32;
            for c in &child.children {
                let w = child_intrinsic_width(c, fs, is_min);
                if is_min || is_vertical {
                    total = total.max(w);
                } else {
                    total += w;
                }
            }
            total
        }
    };

    let mut final_w = content_w;
    if let Some(cs) = &child.style {
        let fs = cs.font_size;
        let rfs = cs.root_font_size;
        if cs.max_width != Length::Auto && !matches!(cs.max_width, Length::Percent(_)) {
            let max_px = cs.max_width.to_px_with_viewport(fs, rfs, 0.0, 0.0);
            final_w = final_w.min(max_px);
        }
        if cs.min_width != Length::Auto && !matches!(cs.min_width, Length::Percent(_)) {
            let min_px = cs.min_width.to_px_with_viewport(fs, rfs, 0.0, 0.0);
            final_w = final_w.max(min_px);
        }
    }
    final_w + non_content_h
}

/// Measures the intrinsic min-content and max-content width of a CSS grid container (CSS Grid §11.1).
pub fn measure_grid_intrinsic_widths(container: &LayoutBox, font_size: f32) -> (f32, f32) {
    let style = container.style.clone().unwrap_or_default();
    let c_font_size = if style.font_size > 0.0 {
        style.font_size
    } else {
        font_size
    };

    let mut in_flow_indices = Vec::new();
    for (idx, child) in container.children.iter().enumerate() {
        let disp = child
            .style
            .as_ref()
            .map(|s| s.display)
            .unwrap_or(Display::Block);
        let pos = child
            .style
            .as_ref()
            .map(|s| s.position)
            .unwrap_or(Position::Static);
        if disp != Display::None && !matches!(pos, Position::Absolute | Position::Fixed) {
            in_flow_indices.push(idx);
        }
    }
    in_flow_indices.sort_by_key(|&idx| {
        container.children[idx]
            .style
            .as_ref()
            .map(|s| s.order)
            .unwrap_or(0)
    });

    let area_cols = style
        .grid_template_areas
        .iter()
        .map(|row| row.len())
        .max()
        .unwrap_or(0);
    let mut template_cols = if !style.grid_template_columns.is_empty() {
        let mut expanded = Vec::new();
        for t in &style.grid_template_columns {
            match t {
                GridTrackSize::RepeatAutoFill(inner) => {
                    expanded.push((**inner).clone());
                }
                GridTrackSize::RepeatAutoFit(inner) => {
                    let count = in_flow_indices.len().max(1);
                    for _ in 0..count {
                        expanded.push((**inner).clone());
                    }
                }
                other => expanded.push(other.clone()),
            }
        }
        expanded
    } else if area_cols > 0 {
        vec![GridTrackSize::Auto; area_cols]
    } else {
        vec![GridTrackSize::Auto]
    };

    let initial_num_cols = template_cols.len().max(area_cols).max(1);
    let explicit_row_count = style
        .grid_template_rows
        .len()
        .max(style.grid_template_areas.len())
        .max(1);

    let (placed_items, num_cols) = place_grid_items(
        &in_flow_indices,
        container,
        &style,
        initial_num_cols,
        explicit_row_count,
    );

    while template_cols.len() < num_cols {
        template_cols.push(GridTrackSize::Auto);
    }

    let col_gap = style
        .column_gap
        .to_px_with_viewport(c_font_size, 16.0, 0.0, 0.0);

    let mut total_min = 0.0f32;
    let mut total_max = 0.0f32;

    for c in 0..num_cols {
        let track = &template_cols[c];
        let (c_min, c_max) = match track {
            GridTrackSize::Length(l) if !matches!(l, Length::Percent(_)) => {
                let px = l.to_px_with_viewport(c_font_size, 16.0, 0.0, 0.0);
                (px, px)
            }
            GridTrackSize::MinContent => {
                let min_w = measure_col_content_min_width(c, &placed_items, container, c_font_size);
                (min_w, min_w)
            }
            GridTrackSize::MaxContent => {
                let max_w = measure_col_content_max_width(c, &placed_items, container, c_font_size);
                (max_w, max_w)
            }
            GridTrackSize::Auto | GridTrackSize::Fr(_) => {
                let min_w = measure_col_content_min_width(c, &placed_items, container, c_font_size);
                let max_w = measure_col_content_max_width(c, &placed_items, container, c_font_size);
                (min_w, max_w)
            }
            GridTrackSize::MinMax(min_t, max_t) => {
                let min_w = match &**min_t {
                    GridTrackSize::Length(l) if !matches!(l, Length::Percent(_)) => {
                        l.to_px_with_viewport(c_font_size, 16.0, 0.0, 0.0)
                    }
                    GridTrackSize::MaxContent => {
                        measure_col_content_max_width(c, &placed_items, container, c_font_size)
                    }
                    _ => measure_col_content_min_width(c, &placed_items, container, c_font_size),
                };
                let max_w = match &**max_t {
                    GridTrackSize::Length(l) if !matches!(l, Length::Percent(_)) => {
                        l.to_px_with_viewport(c_font_size, 16.0, 0.0, 0.0)
                    }
                    GridTrackSize::MinContent => {
                        measure_col_content_min_width(c, &placed_items, container, c_font_size)
                    }
                    _ => measure_col_content_max_width(c, &placed_items, container, c_font_size),
                };
                (min_w, max_w.max(min_w))
            }
            _ => {
                let min_w = measure_col_content_min_width(c, &placed_items, container, c_font_size);
                let max_w = measure_col_content_max_width(c, &placed_items, container, c_font_size);
                (min_w, max_w)
            }
        };

        total_min += c_min;
        total_max += c_max;
    }

    if num_cols > 1 {
        let gaps = (num_cols - 1) as f32 * col_gap;
        total_min += gaps;
        total_max += gaps;
    }

    (total_min, total_max)
}
