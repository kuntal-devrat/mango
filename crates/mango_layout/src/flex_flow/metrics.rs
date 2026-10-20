use mango_core::EdgeSizes;
use mango_css::computed::ComputedStyle;
use mango_css::values::{Display, Position};

use crate::box_tree::LayoutBox;

use super::cross::resolve_item_cross_size;
use super::item::find_first_baseline_ascent;
use super::sizing::{resolve_item_base_and_limits, resolve_item_main_size};

#[derive(Debug, Clone)]
pub struct ItemMetric {
    pub index: usize,
    pub base_main: f32,
    pub final_main: f32,
    pub min_main: f32,
    pub max_main: f32,
    pub cross: f32,
    pub baseline_ascent: f32,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub pad_main: f32,
    pub pad_cross: f32,
    pub border_main: f32,
    pub border_cross: f32,
    pub margin_main: f32,
    pub margin_cross: f32,
}

pub fn measure_item_metrics(
    container: &mut LayoutBox,
    is_row: bool,
    content_width: f32,
    container_height: f32,
    container_style: &ComputedStyle,
) -> Vec<ItemMetric> {
    let mut item_metrics: Vec<ItemMetric> = Vec::with_capacity(container.children.len());

    for (idx, child) in container.children.iter_mut().enumerate() {
        let child_style = child.style.clone().unwrap_or_default();
        if child_style.display == Display::None
            || matches!(child_style.position, Position::Absolute | Position::Fixed)
        {
            continue;
        }
        let c_fs = child_style.font_size;
        let c_root_fs = child_style.root_font_size;

        let p_top = child_style.padding_top.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let p_right = child_style.padding_right.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let p_bottom = child_style.padding_bottom.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let p_left = child_style.padding_left.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );

        let b_top = child_style.border_top_width;
        let b_right = child_style.border_right_width;
        let b_bottom = child_style.border_bottom_width;
        let b_left = child_style.border_left_width;

        let m_top = child_style.margin_top.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let m_right = child_style.margin_right.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let m_bottom = child_style.margin_bottom.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );
        let m_left = child_style.margin_left.to_px_with_viewport(
            c_fs,
            c_root_fs,
            content_width,
            container_height,
        );

        child.dimensions.padding = EdgeSizes::new(p_top, p_right, p_bottom, p_left);
        child.dimensions.border = EdgeSizes::new(b_top, b_right, b_bottom, b_left);
        child.dimensions.margin = EdgeSizes::new(m_top, m_right, m_bottom, m_left);

        let (pad_main, pad_cross) = if is_row {
            (p_left + p_right, p_top + p_bottom)
        } else {
            (p_top + p_bottom, p_left + p_right)
        };
        let (border_main, border_cross) = if is_row {
            (b_left + b_right, b_top + b_bottom)
        } else {
            (b_top + b_bottom, b_left + b_right)
        };
        let (margin_main, margin_cross) = if is_row {
            (m_left + m_right, m_top + m_bottom)
        } else {
            (m_top + m_bottom, m_left + m_right)
        };

        let (explicit_main, is_main_explicit) = resolve_item_main_size(
            child,
            is_row,
            content_width,
            container_height,
            c_fs,
            c_root_fs,
            pad_main,
            border_main,
            margin_main,
            pad_cross,
            border_cross,
            margin_cross,
        );

        let cross_size = resolve_item_cross_size(
            child,
            is_row,
            explicit_main,
            content_width,
            container_height,
            c_fs,
            c_root_fs,
            pad_main,
            border_main,
            margin_main,
            pad_cross,
            border_cross,
            margin_cross,
            container_style,
        );

        let (base_main, min_main, max_main) = resolve_item_base_and_limits(
            child,
            is_row,
            explicit_main,
            is_main_explicit,
            cross_size,
            content_width,
            container_height,
            c_fs,
            c_root_fs,
            pad_main,
            border_main,
            margin_main,
        );

        let baseline_ascent = if is_row {
            m_top + b_top + p_top + find_first_baseline_ascent(child, c_fs)
        } else {
            m_left + b_left + p_left + find_first_baseline_ascent(child, c_fs)
        };

        item_metrics.push(ItemMetric {
            index: idx,
            base_main,
            final_main: base_main,
            min_main,
            max_main,
            cross: cross_size,
            baseline_ascent,
            flex_grow: child_style.flex_grow,
            flex_shrink: child_style.flex_shrink,
            pad_main,
            pad_cross,
            border_main,
            border_cross,
            margin_main,
            margin_cross,
        });
    }

    item_metrics
}
