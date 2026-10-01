//! Table formatting context: column sizing, row heights, grid positioning,
//! collapsed border resolution, caption positioning, and fixed/auto table algorithms.
//!
//! Implements CSS 2.1 §17 (Tables) with:
//! - Auto and Fixed table layout algorithms (`table-layout: auto | fixed`)
//! - 2D grid matrix mapping for full `colspan` / `rowspan` support
//! - Multi-row height distribution for spanning cells
//! - Collapsed border model (`border-collapse: collapse`) with CSS conflict resolution
//! - `<caption>` positioning (`caption-side: top | bottom`)
//! - `<colgroup>` and `<col>` span and width distribution
//! - Recursive nested table intrinsic width measurement
//! - Table row and cell percentage height resolution with vertical surplus distribution

use mango_core::{Color, EdgeSizes, Point, Rect};
use mango_css::values::{
    BorderCollapse, BorderStyle, BoxSizing, CaptionSide, Display, Length, TableLayout,
    VerticalAlign,
};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

/// Lays out a table container box, captions, and its grid of rows and cells.
pub fn layout_table(
    table_box: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) {
    // 1. Calculate horizontal dimensions
    calculate_table_width(table_box, containing_block);

    // 2. Calculate vertical position (respecting clearance) if not already positioned by float layout
    let is_floated = table_box
        .style
        .as_ref()
        .map(|s| s.float != mango_css::values::Float::None)
        .unwrap_or(false);
    if !is_floated {
        let start_y = containing_block.content.y();
        calculate_table_position(table_box, containing_block, start_y, float_ctx);
    }

    // 3. Lay out table captions, rows and cells
    layout_table_contents(table_box, containing_block, float_ctx);
}

fn calculate_table_width(table_box: &mut LayoutBox, containing_block: &Dimensions) {
    let style = table_box.style.clone().unwrap_or_default();
    let container_width = containing_block.content.width();
    let container_height = if containing_block.content.height() > 0.0 {
        containing_block.content.height()
    } else {
        600.0
    };
    let font_size = style.font_size;
    let pad_top = style
        .padding_top
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_right = style
        .padding_right
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_bottom = style
        .padding_bottom
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let pad_left = style
        .padding_left
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let border_top = style.border_top_width;
    let border_right = style.border_right_width;
    let border_bottom = style.border_bottom_width;
    let border_left = style.border_left_width;

    table_box.dimensions.padding = EdgeSizes::new(pad_top, pad_right, pad_bottom, pad_left);
    table_box.dimensions.border =
        EdgeSizes::new(border_top, border_right, border_bottom, border_left);

    let margin_top = style
        .margin_top
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let margin_bottom = style
        .margin_bottom
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let mut margin_left = style
        .margin_left
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);
    let mut margin_right = style
        .margin_right
        .to_px_with_viewport(font_size, 16.0, container_width, container_height);

    let total_non_content_h = pad_left + pad_right + border_left + border_right;

    let mut explicit_width = None;
    if let Some(w_attr) = table_box.get_attribute("width") {
        let w_str = w_attr.trim();
        if let Some(pct_str) = w_str.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>() {
                explicit_width = Some(container_width * (pct / 100.0));
            }
        } else {
            let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
            if let Ok(px) = num_str.parse::<f32>() {
                if px > 0.0 {
                    explicit_width = Some(px);
                }
            }
        }
    }

    let content_width = match style.width {
        Length::Auto => {
            if let Some(w) = explicit_width {
                match style.box_sizing {
                    BoxSizing::ContentBox => w,
                    BoxSizing::BorderBox => (w - total_non_content_h).max(0.0),
                }
            } else {
                (container_width - total_non_content_h - margin_left - margin_right).max(0.0)
            }
        }
        ref len => {
            let px = len.to_px_with_viewport(font_size, 16.0, container_width, container_height);
            match style.box_sizing {
                BoxSizing::ContentBox => px,
                BoxSizing::BorderBox => (px - total_non_content_h).max(0.0),
            }
        }
    };

    if style.margin_left == Length::Auto && style.margin_right == Length::Auto {
        let remaining = (container_width - content_width - total_non_content_h).max(0.0);
        margin_left = remaining / 2.0;
        margin_right = remaining / 2.0;
    } else if style.margin_left == Length::Auto {
        let remaining =
            (container_width - content_width - total_non_content_h - margin_right).max(0.0);
        margin_left = remaining;
    } else if style.margin_right == Length::Auto {
        let remaining =
            (container_width - content_width - total_non_content_h - margin_left).max(0.0);
        margin_right = remaining;
    }

    table_box.dimensions.margin =
        EdgeSizes::new(margin_top, margin_right, margin_bottom, margin_left);
    table_box.dimensions.content.size.width = content_width;
}

fn calculate_table_position(
    table_box: &mut LayoutBox,
    containing_block: &Dimensions,
    start_y: f32,
    _float_ctx: &mut FloatContext,
) {
    let x = containing_block.content.x()
        + table_box.dimensions.margin.left
        + table_box.dimensions.border.left
        + table_box.dimensions.padding.left;

    let y = start_y
        + table_box.dimensions.margin.top
        + table_box.dimensions.border.top
        + table_box.dimensions.padding.top;

    table_box.dimensions.content.origin = Point::new(x, y);
}

fn is_table_row(box_node: &LayoutBox) -> bool {
    box_node.style.as_ref().map(|s| s.display) == Some(Display::TableRow)
        || box_node.tag_name.as_deref() == Some("tr")
}

fn is_table_row_group(box_node: &LayoutBox) -> bool {
    matches!(
        box_node.tag_name.as_deref(),
        Some("thead") | Some("tbody") | Some("tfoot")
    ) || matches!(
        box_node.style.as_ref().map(|s| s.display),
        Some(Display::TableRowGroup)
            | Some(Display::TableHeaderGroup)
            | Some(Display::TableFooterGroup)
    )
}

fn is_table_cell(box_node: &LayoutBox) -> bool {
    box_node.style.as_ref().map(|s| s.display) == Some(Display::TableCell)
        || matches!(box_node.tag_name.as_deref(), Some("td") | Some("th"))
}

fn is_table_caption(box_node: &LayoutBox) -> bool {
    box_node.tag_name.as_deref() == Some("caption")
        || box_node.style.as_ref().map(|s| s.display) == Some(Display::TableCaption)
}

fn is_table_col(box_node: &LayoutBox) -> bool {
    box_node.tag_name.as_deref() == Some("col")
        || box_node.style.as_ref().map(|s| s.display) == Some(Display::TableColumn)
}

fn is_table_colgroup(box_node: &LayoutBox) -> bool {
    box_node.tag_name.as_deref() == Some("colgroup")
        || box_node.style.as_ref().map(|s| s.display) == Some(Display::TableColumnGroup)
}

/// Extracts `<colgroup>` and `<col>` column width specifications.
fn extract_colgroup_specs(table_box: &LayoutBox, table_width: f32) -> Vec<Option<f32>> {
    let mut specs = Vec::new();

    let parse_w = |node: &LayoutBox| -> Option<f32> {
        if let Some(style) = &node.style {
            match style.width {
                Length::Px(px) if px > 0.0 => return Some(px),
                Length::Percent(pct) if pct > 0.0 => return Some(table_width * (pct / 100.0)),
                _ => {}
            }
        }
        if let Some(w_attr) = node.get_attribute("width") {
            let w_str = w_attr.trim();
            if let Some(pct_str) = w_str.strip_suffix('%') {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    return Some(table_width * (pct / 100.0));
                }
            } else {
                let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
                if let Ok(px) = num_str.parse::<f32>() {
                    if px > 0.0 {
                        return Some(px);
                    }
                }
            }
        }
        None
    };

    let parse_span = |node: &LayoutBox| -> usize {
        node.get_attribute("span")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .max(1)
    };

    for child in &table_box.children {
        if is_table_col(child) {
            let span = parse_span(child);
            let w = parse_w(child);
            for _ in 0..span {
                specs.push(w);
            }
        } else if is_table_colgroup(child) {
            let col_children: Vec<&LayoutBox> = child.children.iter().filter(|c| is_table_col(c)).collect();
            if !col_children.is_empty() {
                for col in col_children {
                    let span = parse_span(col);
                    let w = parse_w(col).or_else(|| parse_w(child));
                    for _ in 0..span {
                        specs.push(w);
                    }
                }
            } else {
                let span = parse_span(child);
                let w = parse_w(child);
                for _ in 0..span {
                    specs.push(w);
                }
            }
        }
    }

    specs
}

/// Measures the intrinsic min-content and max-content width of a node.
/// Recognizes nested tables and computes their min/max content recursively.
pub(crate) fn measure_box_intrinsic_widths(
    node: &LayoutBox,
    default_font_size: f32,
    default_weight: mango_render::FontWeight,
    default_family: mango_render::FontFamily,
    memo: &mut std::collections::HashMap<usize, (f32, f32)>,
) -> (f32, f32) {
    let key = node as *const LayoutBox as usize;
    if let Some(&res) = memo.get(&key) {
        return res;
    }

    let font_size = node
        .style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or(default_font_size);
    let is_bold = node
        .style
        .as_ref()
        .map(|s| match s.font_weight {
            mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
            mango_css::values::FontWeight::Numeric(w) => w >= 600,
            _ => false,
        })
        .unwrap_or(false);
    let weight = if is_bold {
        mango_render::FontWeight::Bold
    } else {
        default_weight
    };
    let family = node
        .style
        .as_ref()
        .map(|s| mango_render::FontFamily::from_css_name(&s.font_family))
        .unwrap_or(default_family);

    // 1. Text node measurement
    if let Some(text) = node.text() {
        let mut max_word_w = 0.0f32;
        let mut max_line_w = 0.0f32;

        for line in text.split('\n') {
            let line_w =
                crate::inline_flow::measure_text_width_with_style(line, font_size, weight, family);
            if line_w > max_line_w {
                max_line_w = line_w;
            }
            for word in line.split_whitespace() {
                let word_w = crate::inline_flow::measure_text_width_with_style(
                    word,
                    font_size,
                    weight,
                    family,
                );
                if word_w > max_word_w {
                    max_word_w = word_w;
                }
            }
        }
        let res = (max_word_w, max_line_w);
        memo.insert(key, res);
        return res;
    }

    // 2. Replaced elements
    if matches!(
        node.tag_name.as_deref(),
        Some("img")
            | Some("svg")
            | Some("input")
            | Some("button")
            | Some("select")
            | Some("iframe")
    ) {
        let mut w = None;
        if let Some(w_attr) = node.get_attribute("width") {
            let trimmed = w_attr.trim().trim_end_matches("px");
            if let Ok(val) = trimmed.parse::<f32>() {
                if val > 0.0 {
                    w = Some(val);
                }
            }
        }
        if w.is_none() {
            if let Some(style) = &node.style {
                match style.width {
                    Length::Px(px) if px > 0.0 => w = Some(px),
                    _ => {}
                }
            }
        }
        let fixed_w = w.unwrap_or(if node.dimensions.content.width() > 0.0 {
            node.dimensions.content.width()
        } else {
            16.0
        });
        let res = (fixed_w, fixed_w);
        memo.insert(key, res);
        return res;
    }

    // 2.5 Flex container intrinsic measurement
    if matches!(
        node.style.as_ref().map(|s| s.display),
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) {
        let is_row = matches!(
            node.style.as_ref().map(|s| s.flex_direction),
            Some(mango_css::values::FlexDirection::Row) | Some(mango_css::values::FlexDirection::RowReverse) | None
        );
        let col_gap = node.style.as_ref().map(|s| s.column_gap.to_px(font_size, font_size, 0.0)).unwrap_or(0.0);
        let mut min_w = 0.0f32;
        let mut max_w = 0.0f32;
        let mut child_count = 0;
        for child in &node.children {
            let (c_min, c_max) = measure_box_intrinsic_widths(child, font_size, weight, family, memo);
            if is_row {
                min_w = min_w.max(c_min);
                max_w += c_max;
                child_count += 1;
            } else {
                min_w = min_w.max(c_min);
                max_w = max_w.max(c_max);
            }
        }
        if is_row && child_count > 1 {
            max_w += (child_count - 1) as f32 * col_gap;
        }
        let non_content_h = node.dimensions.padding.left
            + node.dimensions.padding.right
            + node.dimensions.border.left
            + node.dimensions.border.right;
        let res = (min_w + non_content_h, max_w + non_content_h);
        memo.insert(key, res);
        return res;
    }

    // 3. Nested table intrinsic measurement
    if node.tag_name.as_deref() == Some("table")
        || node.style.as_ref().map(|s| s.display) == Some(Display::Table)
    {
        let mut explicit_table_w = None;
        if let Some(w_attr) = node.get_attribute("width") {
            let trimmed = w_attr.trim().trim_end_matches("px");
            if let Ok(val) = trimmed.parse::<f32>() {
                if val > 0.0 {
                    explicit_table_w = Some(val);
                }
            }
        }
        if explicit_table_w.is_none() {
            if let Some(style) = &node.style {
                match style.width {
                    Length::Px(px) if px > 0.0 => explicit_table_w = Some(px),
                    _ => {}
                }
            }
        }

        // Measure columns in the nested table
        let mut n_cols_min: Vec<f32> = Vec::new();
        let mut n_cols_max: Vec<f32> = Vec::new();

        let mut visit_nested_row = |row: &LayoutBox| {
            let mut c_idx = 0;
            for cell in &row.children {
                if is_table_cell(cell) {
                    let span = cell
                        .get_attribute("colspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    while n_cols_min.len() < c_idx + span {
                        n_cols_min.push(0.0);
                        n_cols_max.push(0.0);
                    }
                    if span == 1 {
                        let (c_min, c_max) = measure_box_intrinsic_widths(
                            cell,
                            default_font_size,
                            default_weight,
                            default_family,
                            memo,
                        );
                        n_cols_min[c_idx] = n_cols_min[c_idx].max(c_min);
                        n_cols_max[c_idx] = n_cols_max[c_idx].max(c_max);
                    }
                    c_idx += span;
                }
            }
        };

        for child in &node.children {
            if is_table_row_group(child) {
                for r in &child.children {
                    if is_table_row(r) {
                        visit_nested_row(r);
                    }
                }
            } else if is_table_row(child) {
                visit_nested_row(child);
            }
        }

        let non_content_h = node.dimensions.padding.left
            + node.dimensions.padding.right
            + node.dimensions.border.left
            + node.dimensions.border.right;

        let table_min = n_cols_min.iter().sum::<f32>() + non_content_h;
        let table_max = explicit_table_w.unwrap_or(n_cols_max.iter().sum::<f32>() + non_content_h);

        let res = (table_min.max(1.0), table_max.max(table_min));
        memo.insert(key, res);
        return res;
    }

    // 4. General container: max of child intrinsic widths
    let mut sum_min = 0.0f32;
    let mut sum_max = 0.0f32;
    for child in &node.children {
        let (c_min, c_max) = measure_box_intrinsic_widths(child, font_size, weight, family, memo);
        sum_min = sum_min.max(c_min);
        sum_max = sum_max.max(c_max);
    }
    let res = (sum_min, sum_max);
    memo.insert(key, res);
    res
}

fn cell_intrinsic_metrics(
    cell: &LayoutBox,
    table_width: f32,
    memo: &mut std::collections::HashMap<usize, (f32, f32)>,
) -> (f32, f32, Option<f32>) {
    let mut explicit_w = None;
    if let Some(style) = &cell.style {
        match style.width {
            Length::Px(px) if px > 0.0 => explicit_w = Some(px),
            Length::Percent(pct) if pct > 0.0 => explicit_w = Some(table_width * (pct / 100.0)),
            _ => {}
        }
    }
    if explicit_w.is_none() {
        if let Some(w_attr) = cell.get_attribute("width") {
            let w_str = w_attr.trim();
            if let Some(pct_str) = w_str.strip_suffix('%') {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    explicit_w = Some(table_width * (pct / 100.0));
                }
            } else {
                let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
                if let Ok(px) = num_str.parse::<f32>() {
                    if px > 0.0 {
                        explicit_w = Some(px);
                    }
                }
            }
        }
    }

    let h_padding = cell.dimensions.padding.left
        + cell.dimensions.padding.right
        + cell.dimensions.border.left
        + cell.dimensions.border.right;

    let font_size = cell.style.as_ref().map(|s| s.font_size).unwrap_or(14.0);
    let is_bold = cell
        .style
        .as_ref()
        .map(|s| match s.font_weight {
            mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
            mango_css::values::FontWeight::Numeric(w) => w >= 600,
            _ => false,
        })
        .unwrap_or(false);
    let weight = if is_bold {
        mango_render::FontWeight::Bold
    } else {
        mango_render::FontWeight::Regular
    };
    let family = cell
        .style
        .as_ref()
        .map(|s| mango_render::FontFamily::from_css_name(&s.font_family))
        .unwrap_or(mango_render::FontFamily::SansSerif);

    let (content_min, content_max) =
        measure_box_intrinsic_widths(cell, font_size, weight, family, memo);

    let min_content = (content_min + h_padding).max(1.0);
    let max_content = (content_max + h_padding).max(min_content);

    let final_min = explicit_w.map(|w| w.min(min_content)).unwrap_or(min_content);
    let final_max = explicit_w.unwrap_or(max_content);

    (final_min, final_max, explicit_w)
}

#[derive(Debug, Clone, Copy)]
struct CellRef {
    group_idx: Option<usize>,
    row_idx: usize,
    cell_idx: usize,
    start_row: usize,
    start_col: usize,
    row_span: usize,
    col_span: usize,
}

#[derive(Debug, Clone, Copy)]
struct BorderSide {
    width: f32,
    style: BorderStyle,
    color: Color,
    precedence: u8, // 4 = cell, 3 = row, 2 = rowgroup, 1 = table
}

impl Default for BorderSide {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderStyle::None,
            color: Color::TRANSPARENT,
            precedence: 0,
        }
    }
}

fn style_priority(s: BorderStyle) -> u8 {
    match s {
        BorderStyle::Double => 8,
        BorderStyle::Solid => 7,
        BorderStyle::Dashed => 6,
        BorderStyle::Dotted => 5,
        BorderStyle::Ridge => 4,
        BorderStyle::Outset => 3,
        BorderStyle::Groove => 2,
        BorderStyle::Inset => 1,
        BorderStyle::None | BorderStyle::Hidden => 0,
    }
}

/// Resolves conflicting borders per CSS 2.1 §17.6.2.1.
fn resolve_border_conflict(a: BorderSide, b: BorderSide) -> BorderSide {
    // 1. Hidden wins unconditionally and suppresses border
    if a.style == BorderStyle::Hidden || b.style == BorderStyle::Hidden {
        return BorderSide {
            width: 0.0,
            style: BorderStyle::Hidden,
            color: Color::TRANSPARENT,
            precedence: 255,
        };
    }

    // 2. None has the lowest priority
    if a.style == BorderStyle::None && b.style != BorderStyle::None {
        return b;
    }
    if b.style == BorderStyle::None && a.style != BorderStyle::None {
        return a;
    }

    // 3. Compare widths (larger width wins)
    if (a.width - b.width).abs() > 0.1 {
        return if a.width > b.width { a } else { b };
    }

    // 4. Compare style priority
    let a_prio = style_priority(a.style);
    let b_prio = style_priority(b.style);
    if a_prio != b_prio {
        return if a_prio > b_prio { a } else { b };
    }

    // 5. Compare element precedence (cell > row > rowgroup > table)
    if a.precedence != b.precedence {
        return if a.precedence > b.precedence { a } else { b };
    }

    // 6. Tie-breaker: prefer first side
    a
}

/// Main layout function orchestrating captions, rows, cells, and border collapse.
fn layout_table_contents(
    table_box: &mut LayoutBox,
    containing_block: &Dimensions,
    _float_ctx: &mut FloatContext,
) {
    let style = table_box.style.clone().unwrap_or_default();
    let is_collapse = style.border_collapse == BorderCollapse::Collapse;
    let is_fixed = style.table_layout == TableLayout::Fixed;

    let border_spacing = if is_collapse {
        0.0
    } else if let Some(cs) = table_box
        .get_attribute("cellspacing")
        .and_then(|s| s.trim().parse::<f32>().ok())
    {
        cs
    } else {
        style.border_spacing
    };

    let mut table_w = table_box.dimensions.content.width();
    let table_x = table_box.dimensions.content.x();
    let mut current_y = table_box.dimensions.content.y();

    // 1. Separate captions into top and bottom
    let mut top_caption_indices = Vec::new();
    let mut bottom_caption_indices = Vec::new();
    for (idx, child) in table_box.children.iter().enumerate() {
        if is_table_caption(child) {
            let side = child
                .style
                .as_ref()
                .map(|s| s.caption_side)
                .unwrap_or(style.caption_side);
            let is_bottom = side == CaptionSide::Bottom
                || child
                    .get_attribute("align")
                    .map(|a| a.trim().eq_ignore_ascii_case("bottom"))
                    .unwrap_or(false);
            if is_bottom {
                bottom_caption_indices.push(idx);
            } else {
                top_caption_indices.push(idx);
            }
        }
    }

    // Lay out top captions
    for &cap_idx in &top_caption_indices {
        let caption = &mut table_box.children[cap_idx];
        let mut caption_cb = Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
        caption_cb.content.size.width = table_w;
        let mut cap_float_ctx = FloatContext::new();
        crate::block_flow::layout_block(caption, &caption_cb, &mut cap_float_ctx);
        current_y += caption.dimensions.margin_box().height();
    }

    let grid_start_y = current_y;

    // 2. Identify `<colgroup>` and `<col>` column width specifications
    let col_specs = extract_colgroup_specs(table_box, table_w);

    // 3. Collect row references and count total columns and rows
    let has_row_groups = table_box.children.iter().any(is_table_row_group);

    // (group_idx, row_idx)
    let mut row_refs: Vec<(Option<usize>, usize)> = Vec::new();

    if has_row_groups {
        for (g_idx, group) in table_box.children.iter().enumerate() {
            if is_table_row_group(group) {
                for (r_idx, row) in group.children.iter().enumerate() {
                    if is_table_row(row) {
                        row_refs.push((Some(g_idx), r_idx));
                    }
                }
            } else if is_table_row(group) {
                row_refs.push((None, g_idx));
            }
        }
    } else {
        for (r_idx, row) in table_box.children.iter().enumerate() {
            if is_table_row(row) {
                row_refs.push((None, r_idx));
            }
        }
    }

    if row_refs.is_empty() {
        // Lay out bottom captions if any
        for &cap_idx in &bottom_caption_indices {
            let caption = &mut table_box.children[cap_idx];
            let mut caption_cb = Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
            caption_cb.content.size.width = table_w;
            let mut cap_float_ctx = FloatContext::new();
            crate::block_flow::layout_block(caption, &caption_cb, &mut cap_float_ctx);
            current_y += caption.dimensions.margin_box().height();
        }
        table_box.dimensions.content.size.height =
            (current_y - table_box.dimensions.content.y()).max(10.0);
        return;
    }

    // Determine number of columns and build initial 2D cell placements
    let mut num_cols = col_specs.len();
    let num_rows = row_refs.len();

    // Preliminary pass to find max columns
    {
        let mut active_rowspans: Vec<usize> = Vec::new();
        for &(g_idx, r_idx) in &row_refs {
            let row = if let Some(gi) = g_idx {
                &table_box.children[gi].children[r_idx]
            } else {
                &table_box.children[r_idx]
            };
            let mut col_idx = 0;
            for cell in &row.children {
                if is_table_cell(cell) {
                    while col_idx < active_rowspans.len() && active_rowspans[col_idx] > 0 {
                        col_idx += 1;
                    }
                    let c_span = cell
                        .get_attribute("colspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    let r_span = cell
                        .get_attribute("rowspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);

                    if r_span > 1 {
                        while active_rowspans.len() < col_idx + c_span {
                            active_rowspans.push(0);
                        }
                        for k in 0..c_span {
                            active_rowspans[col_idx + k] = r_span;
                        }
                    }
                    col_idx += c_span;
                }
            }
            num_cols = num_cols.max(col_idx);
            for rem in active_rowspans.iter_mut() {
                if *rem > 0 {
                    *rem -= 1;
                }
            }
        }
    }

    num_cols = num_cols.max(1);

    // Build complete 2D grid matrix of CellRefs
    let mut grid: Vec<Vec<Option<CellRef>>> = vec![vec![None; num_cols]; num_rows];
    let mut all_cells: Vec<CellRef> = Vec::new();

    for (grid_r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
        let row = if let Some(gi) = g_idx {
            &table_box.children[gi].children[r_idx]
        } else {
            &table_box.children[r_idx]
        };

        let mut grid_c = 0;
        for (cell_idx, cell) in row.children.iter().enumerate() {
            if is_table_cell(cell) {
                while grid_c < num_cols && grid[grid_r][grid_c].is_some() {
                    grid_c += 1;
                }
                if grid_c >= num_cols {
                    break;
                }

                let c_span = cell
                    .get_attribute("colspan")
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let r_span = cell
                    .get_attribute("rowspan")
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);

                let actual_c_span = c_span.min(num_cols - grid_c);
                let actual_r_span = r_span.min(num_rows - grid_r);

                let cell_ref = CellRef {
                    group_idx: g_idx,
                    row_idx: r_idx,
                    cell_idx,
                    start_row: grid_r,
                    start_col: grid_c,
                    row_span: actual_r_span,
                    col_span: actual_c_span,
                };
                all_cells.push(cell_ref);

                for ro in 0..actual_r_span {
                    for co in 0..actual_c_span {
                        if grid_r + ro < num_rows && grid_c + co < num_cols {
                            grid[grid_r + ro][grid_c + co] = Some(cell_ref);
                        }
                    }
                }

                grid_c += actual_c_span;
            }
        }
    }

    // 4. Column Width Calculation (Fixed vs Auto)
    let total_spacing = (num_cols + 1) as f32 * border_spacing;
    let available_w = (table_w - total_spacing).max(0.0);
    let mut col_widths = vec![0.0f32; num_cols];

    if is_fixed {
        // --- Fixed Table Layout Algorithm (CSS 2.1 §17.5.2.1) ---
        // Column widths are determined solely by:
        // 1. <col> / <colgroup> elements
        // 2. Cells in the first row with explicit width
        // 3. Remaining columns divide the rest equally
        let mut fixed_widths = vec![None::<f32>; num_cols];

        for i in 0..num_cols {
            if let Some(w) = col_specs.get(i).copied().flatten() {
                fixed_widths[i] = Some(w);
            }
        }

        // Check first row's cells for remaining unassigned columns
        if !row_refs.is_empty() {
            let &(g0, r0) = &row_refs[0];
            let first_row = if let Some(gi) = g0 {
                &table_box.children[gi].children[r0]
            } else {
                &table_box.children[r0]
            };
            let mut c_idx = 0;
            for cell in &first_row.children {
                if is_table_cell(cell) && c_idx < num_cols {
                    let span = cell
                        .get_attribute("colspan")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    let actual_span = span.min(num_cols - c_idx);

                    let mut explicit_w = None;
                    if let Some(s) = &cell.style {
                        match s.width {
                            Length::Px(px) if px > 0.0 => explicit_w = Some(px),
                            Length::Percent(pct) if pct > 0.0 => {
                                explicit_w = Some(available_w * (pct / 100.0))
                            }
                            _ => {}
                        }
                    }
                    if explicit_w.is_none() {
                        if let Some(w_attr) = cell.get_attribute("width") {
                            let w_str = w_attr.trim();
                            if let Some(pct_str) = w_str.strip_suffix('%') {
                                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                                    explicit_w = Some(available_w * (pct / 100.0));
                                }
                            } else {
                                let num_str = w_str.strip_suffix("px").unwrap_or(w_str).trim();
                                if let Ok(px) = num_str.parse::<f32>() {
                                    if px > 0.0 {
                                        explicit_w = Some(px);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(ew) = explicit_w {
                        let per_col_w = ew / actual_span as f32;
                        for k in 0..actual_span {
                            if fixed_widths[c_idx + k].is_none() {
                                fixed_widths[c_idx + k] = Some(per_col_w);
                            }
                        }
                    }
                    c_idx += actual_span;
                }
            }
        }

        let assigned_sum: f32 = fixed_widths.iter().filter_map(|&w| w).sum();
        let unassigned_count = fixed_widths.iter().filter(|w| w.is_none()).count();
        let remaining_w = (available_w - assigned_sum).max(0.0);
        let equal_share = if unassigned_count > 0 {
            remaining_w / unassigned_count as f32
        } else {
            0.0
        };

        for i in 0..num_cols {
            col_widths[i] = fixed_widths[i].unwrap_or(equal_share).max(1.0);
        }
    } else {
        // --- Auto Table Layout Algorithm (CSS 2.1 §17.5.2.2) ---
        let mut col_min_widths = vec![4.0f32; num_cols];
        let mut col_max_widths = vec![4.0f32; num_cols];
        let mut col_explicit_widths = vec![None::<f32>; num_cols];
        let mut intrinsic_memo = std::collections::HashMap::new();

        // Seed with <col> specs
        for i in 0..num_cols {
            if let Some(w) = col_specs.get(i).copied().flatten() {
                col_min_widths[i] = col_min_widths[i].max(w);
                col_max_widths[i] = col_max_widths[i].max(w);
                col_explicit_widths[i] = Some(w);
            }
        }

        // Pass 1: single-column cells
        for cell_ref in &all_cells {
            if cell_ref.col_span == 1 {
                let cell = if let Some(gi) = cell_ref.group_idx {
                    &table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
                } else {
                    &table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
                };
                let (c_min, c_max, c_exp) =
                    cell_intrinsic_metrics(cell, table_w, &mut intrinsic_memo);
                let c_idx = cell_ref.start_col;
                col_min_widths[c_idx] = col_min_widths[c_idx].max(c_min);
                col_max_widths[c_idx] = col_max_widths[c_idx].max(c_max);
                if let Some(ew) = c_exp {
                    col_explicit_widths[c_idx] =
                        Some(col_explicit_widths[c_idx].unwrap_or(0.0).max(ew));
                }
            }
        }

        // Pass 2: multi-column cells
        for cell_ref in &all_cells {
            if cell_ref.col_span > 1 {
                let cell = if let Some(gi) = cell_ref.group_idx {
                    &table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
                } else {
                    &table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
                };
                let (c_min, c_max, _) = cell_intrinsic_metrics(cell, table_w, &mut intrinsic_memo);
                let c_idx = cell_ref.start_col;
                let actual_span = cell_ref.col_span;
                let internal_spacing = (actual_span - 1) as f32 * border_spacing;

                let span_min_sum: f32 = (0..actual_span).map(|k| col_min_widths[c_idx + k]).sum::<f32>()
                    + internal_spacing;
                if c_min > span_min_sum {
                    let diff = (c_min - span_min_sum) / actual_span as f32;
                    for k in 0..actual_span {
                        col_min_widths[c_idx + k] += diff;
                    }
                }

                let span_max_sum: f32 = (0..actual_span).map(|k| col_max_widths[c_idx + k]).sum::<f32>()
                    + internal_spacing;
                if c_max > span_max_sum {
                    let diff = (c_max - span_max_sum) / actual_span as f32;
                    for k in 0..actual_span {
                        col_max_widths[c_idx + k] += diff;
                    }
                }
            }
        }

        for i in 0..num_cols {
            col_min_widths[i] = col_min_widths[i].max(4.0);
            col_max_widths[i] = col_max_widths[i].max(col_min_widths[i]);
        }

        let sum_min: f32 = col_min_widths.iter().sum();
        let sum_max: f32 = col_max_widths.iter().sum();

        if available_w <= sum_min {
            for i in 0..num_cols {
                col_widths[i] = col_min_widths[i];
            }
        } else if available_w <= sum_max {
            let extra = available_w - sum_min;
            let diff_sum = (sum_max - sum_min).max(1.0);
            for i in 0..num_cols {
                let col_diff = (col_max_widths[i] - col_min_widths[i]).max(0.0);
                col_widths[i] = (col_min_widths[i] + extra * (col_diff / diff_sum)).max(1.0);
            }
        } else {
            let surplus = available_w - sum_max;
            let num_auto = col_explicit_widths.iter().filter(|w| w.is_none()).count();
            if num_auto > 0 {
                let auto_max_sum: f32 = col_max_widths
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| col_explicit_widths[*i].is_none())
                    .map(|(_, m)| *m)
                    .sum();
                for i in 0..num_cols {
                    if col_explicit_widths[i].is_none() {
                        let share = if auto_max_sum > 0.0 {
                            col_max_widths[i] / auto_max_sum
                        } else {
                            1.0 / num_auto as f32
                        };
                        col_widths[i] = (col_max_widths[i] + surplus * share).max(1.0);
                    } else {
                        col_widths[i] = col_max_widths[i].max(1.0);
                    }
                }
            } else if sum_max > 0.0 {
                for i in 0..num_cols {
                    col_widths[i] =
                        (col_max_widths[i] + surplus * (col_max_widths[i] / sum_max)).max(1.0);
                }
            } else {
                let eq = (available_w / num_cols as f32).max(1.0);
                for i in 0..num_cols {
                    col_widths[i] = eq;
                }
            }
        }
    }

    let actual_table_w = col_widths.iter().sum::<f32>() + total_spacing;
    if actual_table_w > table_w {
        table_w = actual_table_w;
        table_box.dimensions.content.size.width = table_w;
    }

    // 5. Collapsed Border Resolution Model (CSS 2.1 §17.6.2)
    if is_collapse {
        let extract_cell_border = |cell: &LayoutBox, side: &str| -> BorderSide {
            let s = cell.style.as_ref();
            let (w, st, c) = match side {
                "top" => (
                    s.map_or(0.0, |st| st.border_top_width),
                    s.map_or(BorderStyle::None, |st| st.border_top_style),
                    s.map_or(Color::BLACK, |st| st.border_top_color),
                ),
                "bottom" => (
                    s.map_or(0.0, |st| st.border_bottom_width),
                    s.map_or(BorderStyle::None, |st| st.border_bottom_style),
                    s.map_or(Color::BLACK, |st| st.border_bottom_color),
                ),
                "left" => (
                    s.map_or(0.0, |st| st.border_left_width),
                    s.map_or(BorderStyle::None, |st| st.border_left_style),
                    s.map_or(Color::BLACK, |st| st.border_left_color),
                ),
                "right" => (
                    s.map_or(0.0, |st| st.border_right_width),
                    s.map_or(BorderStyle::None, |st| st.border_right_style),
                    s.map_or(Color::BLACK, |st| st.border_right_color),
                ),
                _ => (0.0, BorderStyle::None, Color::BLACK),
            };
            BorderSide {
                width: w,
                style: st,
                color: c,
                precedence: 4,
            }
        };

        // Horizontal edges: between row r and row r+1 (size: (num_rows - 1) x num_cols)
        let mut horiz_edges: Vec<Vec<BorderSide>> =
            vec![vec![BorderSide::default(); num_cols]; num_rows.saturating_sub(1)];
        for r in 0..num_rows.saturating_sub(1) {
            for c in 0..num_cols {
                if let (Some(top_cell_ref), Some(bot_cell_ref)) = (grid[r][c], grid[r + 1][c]) {
                    if top_cell_ref.start_row != bot_cell_ref.start_row
                        || top_cell_ref.start_col != bot_cell_ref.start_col
                    {
                        let top_cell = if let Some(gi) = top_cell_ref.group_idx {
                            &table_box.children[gi].children[top_cell_ref.row_idx].children[top_cell_ref.cell_idx]
                        } else {
                            &table_box.children[top_cell_ref.row_idx].children[top_cell_ref.cell_idx]
                        };
                        let bot_cell = if let Some(gi) = bot_cell_ref.group_idx {
                            &table_box.children[gi].children[bot_cell_ref.row_idx].children[bot_cell_ref.cell_idx]
                        } else {
                            &table_box.children[bot_cell_ref.row_idx].children[bot_cell_ref.cell_idx]
                        };
                        let side_top = extract_cell_border(top_cell, "bottom");
                        let side_bot = extract_cell_border(bot_cell, "top");
                        horiz_edges[r][c] = resolve_border_conflict(side_top, side_bot);
                    }
                }
            }
        }

        // Vertical edges: between col c and col c+1 (size: num_rows x (num_cols - 1))
        let mut vert_edges: Vec<Vec<BorderSide>> =
            vec![vec![BorderSide::default(); num_cols.saturating_sub(1)]; num_rows];
        for r in 0..num_rows {
            for c in 0..num_cols.saturating_sub(1) {
                if let (Some(left_cell_ref), Some(right_cell_ref)) = (grid[r][c], grid[r][c + 1]) {
                    if left_cell_ref.start_row != right_cell_ref.start_row
                        || left_cell_ref.start_col != right_cell_ref.start_col
                    {
                        let left_cell = if let Some(gi) = left_cell_ref.group_idx {
                            &table_box.children[gi].children[left_cell_ref.row_idx].children[left_cell_ref.cell_idx]
                        } else {
                            &table_box.children[left_cell_ref.row_idx].children[left_cell_ref.cell_idx]
                        };
                        let right_cell = if let Some(gi) = right_cell_ref.group_idx {
                            &table_box.children[gi].children[right_cell_ref.row_idx].children[right_cell_ref.cell_idx]
                        } else {
                            &table_box.children[right_cell_ref.row_idx].children[right_cell_ref.cell_idx]
                        };
                        let side_left = extract_cell_border(left_cell, "right");
                        let side_right = extract_cell_border(right_cell, "left");
                        vert_edges[r][c] = resolve_border_conflict(side_left, side_right);
                    }
                }
            }
        }

        // Apply collapsed borders:
        // Top edge of internal boundary stays on top cell bottom border; lower cell top border becomes 0.
        // Left edge of internal boundary stays on left cell right border; right cell left border becomes 0.
        for r in 0..num_rows {
            for c in 0..num_cols {
                if let Some(cell_ref) = grid[r][c] {
                    if r == cell_ref.start_row && c == cell_ref.start_col {
                        let cell = if let Some(gi) = cell_ref.group_idx {
                            &mut table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
                        } else {
                            &mut table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
                        };

                        // Bottom border update
                        let bot_boundary_r = cell_ref.start_row + cell_ref.row_span - 1;
                        if bot_boundary_r < num_rows.saturating_sub(1) {
                            let resolved = horiz_edges[bot_boundary_r][cell_ref.start_col];
                            cell.dimensions.border.bottom = resolved.width;
                            if let Some(s) = cell.style.as_mut() {
                                s.border_bottom_width = resolved.width;
                                s.border_bottom_style = resolved.style;
                                s.border_bottom_color = resolved.color;
                            }
                        }

                        // Top border update: if not row 0, set top border to 0
                        if cell_ref.start_row > 0 {
                            cell.dimensions.border.top = 0.0;
                            if let Some(s) = cell.style.as_mut() {
                                s.border_top_width = 0.0;
                                s.border_top_style = BorderStyle::None;
                            }
                        }

                        // Right border update
                        let right_boundary_c = cell_ref.start_col + cell_ref.col_span - 1;
                        if right_boundary_c < num_cols.saturating_sub(1) {
                            let resolved = vert_edges[cell_ref.start_row][right_boundary_c];
                            cell.dimensions.border.right = resolved.width;
                            if let Some(s) = cell.style.as_mut() {
                                s.border_right_width = resolved.width;
                                s.border_right_style = resolved.style;
                                s.border_right_color = resolved.color;
                            }
                        }

                        // Left border update: if not col 0, set left border to 0
                        if cell_ref.start_col > 0 {
                            cell.dimensions.border.left = 0.0;
                            if let Some(s) = cell.style.as_mut() {
                                s.border_left_width = 0.0;
                                s.border_left_style = BorderStyle::None;
                            }
                        }
                    }
                }
            }
        }
    }

    // 6. Measure cell heights and compute natural row heights
    let mut row_natural_heights = vec![0.0f32; num_rows];
    let mut cell_layouts: std::collections::HashMap<(usize, usize, usize), (f32, f32)> =
        std::collections::HashMap::new();

    // Check specified row heights (from row style or row height attribute)
    for (r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
        let row = if let Some(gi) = g_idx {
            &table_box.children[gi].children[r_idx]
        } else {
            &table_box.children[r_idx]
        };
        let mut spec_h = None;
        if let Some(s) = &row.style {
            match s.height {
                Length::Px(px) if px >= 0.0 => spec_h = Some(px),
                _ => {}
            }
        }
        if spec_h.is_none() {
            if let Some(h_attr) = row.get_attribute("height") {
                let h_str = h_attr.trim();
                let num_str = h_str.strip_suffix("px").unwrap_or(h_str).trim();
                if let Ok(px) = num_str.parse::<f32>() {
                    if px >= 0.0 {
                        spec_h = Some(px);
                    }
                }
            }
        }
        if let Some(sh) = spec_h {
            row_natural_heights[r] = row_natural_heights[r].max(sh);
        }
    }

    // Layout single-row cells first to determine row heights
    for cell_ref in &all_cells {
        let col_start = cell_ref.start_col;
        let c_span = cell_ref.col_span;
        let spanned_w: f32 = (0..c_span).map(|k| col_widths[col_start + k]).sum();
        let intervening_spacing = (c_span.saturating_sub(1)) as f32 * border_spacing;
        let cell_w = spanned_w + intervening_spacing;

        let cell = if let Some(gi) = cell_ref.group_idx {
            &mut table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
        } else {
            &mut table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
        };

        let mut cell_cb = Dimensions::new(Rect::new(0.0, 0.0, cell_w, 10000.0));
        cell_cb.content.size.width = cell_w;
        let mut cell_float_ctx = FloatContext::new();
        crate::block_flow::layout_block(cell, &cell_cb, &mut cell_float_ctx);

        let cell_total_h = cell.dimensions.margin_box().height();
        let cell_key = (
            cell_ref.group_idx.unwrap_or(usize::MAX),
            cell_ref.row_idx,
            cell_ref.cell_idx,
        );
        cell_layouts.insert(cell_key, (cell_w, cell_total_h));

        if cell_ref.row_span == 1 {
            row_natural_heights[cell_ref.start_row] =
                row_natural_heights[cell_ref.start_row].max(cell_total_h);
        }
    }

    // Distribute shortfall for multi-row cells (rowspan > 1)
    for cell_ref in &all_cells {
        if cell_ref.row_span > 1 {
            let cell_key = (
                cell_ref.group_idx.unwrap_or(usize::MAX),
                cell_ref.row_idx,
                cell_ref.cell_idx,
            );
            let &(_, cell_total_h) = cell_layouts.get(&cell_key).unwrap();

            let r_start = cell_ref.start_row;
            let r_span = cell_ref.row_span;
            let current_spanned_h: f32 = (0..r_span)
                .map(|k| row_natural_heights[r_start + k])
                .sum::<f32>()
                + (r_span - 1) as f32 * border_spacing;

            if cell_total_h > current_spanned_h {
                let diff = (cell_total_h - current_spanned_h) / r_span as f32;
                for k in 0..r_span {
                    row_natural_heights[r_start + k] += diff;
                }
            }
        }
    }

    // 7. Table row/cell percentage heights and vertical surplus distribution
    let mut explicit_table_h = None;
    if let Some(s) = &table_box.style {
        match s.height {
            Length::Px(px) if px > 0.0 => explicit_table_h = Some(px),
            Length::Percent(pct) if pct > 0.0 => {
                let cb_h = containing_block.content.height();
                if cb_h > 0.0 {
                    explicit_table_h = Some(cb_h * (pct / 100.0));
                }
            }
            _ => {}
        }
    }
    if explicit_table_h.is_none() {
        if let Some(h_attr) = table_box.get_attribute("height") {
            let h_str = h_attr.trim();
            if let Some(pct_str) = h_str.strip_suffix('%') {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    let cb_h = containing_block.content.height();
                    if cb_h > 0.0 {
                        explicit_table_h = Some(cb_h * (pct / 100.0));
                    }
                }
            } else {
                let num_str = h_str.strip_suffix("px").unwrap_or(h_str).trim();
                if let Ok(px) = num_str.parse::<f32>() {
                    if px > 0.0 {
                        explicit_table_h = Some(px);
                    }
                }
            }
        }
    }

    let top_captions_h = grid_start_y - table_box.dimensions.content.y();
    let total_v_spacing = (num_rows + 1) as f32 * border_spacing;

    if let Some(target_table_h) = explicit_table_h {
        let available_grid_h = (target_table_h - top_captions_h - total_v_spacing).max(0.0);

        // Resolve row percentage heights against available_grid_h
        for (r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
            let row = if let Some(gi) = g_idx {
                &table_box.children[gi].children[r_idx]
            } else {
                &table_box.children[r_idx]
            };
            if let Some(s) = &row.style {
                if let Length::Percent(pct) = s.height {
                    let row_pct_h = available_grid_h * (pct / 100.0);
                    row_natural_heights[r] = row_natural_heights[r].max(row_pct_h);
                }
            }
        }

        let sum_rows: f32 = row_natural_heights.iter().sum();
        if sum_rows < available_grid_h && !row_natural_heights.is_empty() {
            let surplus = available_grid_h - sum_rows;
            if sum_rows > 0.0 {
                for r in 0..num_rows {
                    row_natural_heights[r] += surplus * (row_natural_heights[r] / sum_rows);
                }
            } else {
                let eq = surplus / num_rows as f32;
                for r in 0..num_rows {
                    row_natural_heights[r] += eq;
                }
            }
        }
    }

    // 8. Position rows and cells in 2D space and apply vertical alignment
    let mut row_y = grid_start_y + border_spacing;
    let mut row_positions_y = Vec::with_capacity(num_rows);

    for r in 0..num_rows {
        row_positions_y.push(row_y);
        row_y += row_natural_heights[r] + border_spacing;
    }

    // Set row dimensions
    for (r, &(g_idx, r_idx)) in row_refs.iter().enumerate() {
        let row = if let Some(gi) = g_idx {
            &mut table_box.children[gi].children[r_idx]
        } else {
            &mut table_box.children[r_idx]
        };
        row.dimensions.content.origin = Point::new(table_x, row_positions_y[r]);
        row.dimensions.content.size.width = table_w;
        row.dimensions.content.size.height = row_natural_heights[r];
    }

    // Set rowgroup dimensions
    if has_row_groups {
        for group in &mut table_box.children {
            if is_table_row_group(group) {
                let first_row_y = group
                    .children
                    .iter()
                    .filter(|c| is_table_row(c))
                    .map(|c| c.dimensions.content.origin.y)
                    .next()
                    .unwrap_or(grid_start_y);
                let last_row_bottom = group
                    .children
                    .iter()
                    .filter(|c| is_table_row(c))
                    .map(|c| c.dimensions.content.origin.y + c.dimensions.content.size.height)
                    .last()
                    .unwrap_or(first_row_y);

                group.dimensions.content.origin = Point::new(table_x, first_row_y);
                group.dimensions.content.size.width = table_w;
                group.dimensions.content.size.height = last_row_bottom - first_row_y;
            }
        }
    }

    // Position and stretch every cell
    for cell_ref in &all_cells {
        let cell_key = (
            cell_ref.group_idx.unwrap_or(usize::MAX),
            cell_ref.row_idx,
            cell_ref.cell_idx,
        );
        let &(cell_w, _) = cell_layouts.get(&cell_key).unwrap();

        let c_start = cell_ref.start_col;
        let r_start = cell_ref.start_row;
        let r_span = cell_ref.row_span;

        let cell_x = table_x
            + border_spacing
            + (0..c_start)
                .map(|k| col_widths[k] + border_spacing)
                .sum::<f32>();
        let cell_y = row_positions_y[r_start];

        let spanned_h: f32 = (0..r_span)
            .map(|k| row_natural_heights[r_start + k])
            .sum::<f32>()
            + (r_span.saturating_sub(1)) as f32 * border_spacing;

        let cell = if let Some(gi) = cell_ref.group_idx {
            &mut table_box.children[gi].children[cell_ref.row_idx].children[cell_ref.cell_idx]
        } else {
            &mut table_box.children[cell_ref.row_idx].children[cell_ref.cell_idx]
        };

        cell.dimensions.content.origin.x =
            cell_x + cell.dimensions.padding.left + cell.dimensions.border.left;
        cell.dimensions.content.origin.y =
            cell_y + cell.dimensions.padding.top + cell.dimensions.border.top;
        cell.dimensions.content.size.width = (cell_w
            - cell.dimensions.padding.left
            - cell.dimensions.padding.right
            - cell.dimensions.border.left
            - cell.dimensions.border.right)
            .max(1.0);

        let non_content_v = cell.dimensions.padding.top
            + cell.dimensions.padding.bottom
            + cell.dimensions.border.top
            + cell.dimensions.border.bottom;
        let allocated_h = (spanned_h - non_content_v).max(0.0);
        let natural_h = cell.dimensions.content.size.height;
        let extra_h = (allocated_h - natural_h).max(0.0);

        cell.dimensions.content.size.height = allocated_h.max(1.0);

        // Vertical alignment inside table cell
        if extra_h > 0.5 {
            let va = cell
                .style
                .as_ref()
                .map(|s| s.vertical_align)
                .unwrap_or(VerticalAlign::Middle);

            let dy = match va {
                VerticalAlign::Top => 0.0,
                VerticalAlign::Middle => extra_h / 2.0,
                VerticalAlign::Bottom => extra_h,
                VerticalAlign::Baseline => 0.0,
                _ => 0.0,
            };

            if dy > 0.0 {
                crate::block_flow::shift_descendants(cell, 0.0, dy);
            }
        }
    }

    current_y = row_y;

    // 9. Lay out bottom captions
    for &cap_idx in &bottom_caption_indices {
        let caption = &mut table_box.children[cap_idx];
        let mut caption_cb = Dimensions::new(Rect::new(table_x, current_y, table_w, 10000.0));
        caption_cb.content.size.width = table_w;
        let mut cap_float_ctx = FloatContext::new();
        crate::block_flow::layout_block(caption, &caption_cb, &mut cap_float_ctx);
        current_y += caption.dimensions.margin_box().height();
    }

    table_box.dimensions.content.size.height =
        (current_y - table_box.dimensions.content.y()).max(10.0);
}
