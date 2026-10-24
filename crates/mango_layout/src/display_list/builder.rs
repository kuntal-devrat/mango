use mango_core::Rect;
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::background::paint_background;
use super::borders::paint_borders;
use super::forms::render_form_control;
use super::media::paint_media;
use super::stacking::paint_children_and_scrollbars;
use super::text::paint_text;
use super::types::{mul_matrix, resolve_origin};

/// Generates a [`DisplayList`] from a laid-out box tree.
pub fn build_display_list(root: &LayoutBox) -> DisplayList {
    build_display_list_with_scroll(root, 0.0)
}

/// Generates a [`DisplayList`] with a given vertical scroll offset,
/// properly offsetting `position: sticky` and `position: fixed` elements.
pub fn build_display_list_with_scroll(root: &LayoutBox, scroll_y: f32) -> DisplayList {
    build_display_list_with_scroll_xy(root, 0.0, scroll_y)
}

/// Generates a [`DisplayList`] with horizontal and vertical scroll offsets,
/// properly offsetting `position: sticky` and `position: fixed` elements.
pub fn build_display_list_with_scroll_xy(
    root: &LayoutBox,
    scroll_x: f32,
    scroll_y: f32,
) -> DisplayList {
    let mut list = DisplayList::new();
    let mut init_offset_x = 0.0;
    let mut init_offset_y = 0.0;
    if let Some(s) = &root.style
        && s.position == mango_css::values::Position::Fixed
    {
        init_offset_x = scroll_x;
        init_offset_y = scroll_y;
    }
    render_layout_box(
        root,
        &mut list,
        1.0,
        init_offset_x,
        init_offset_y,
        scroll_x,
        scroll_y,
        None,
        None,
        0.0,
        0.0,
    );
    list
}

/// Wraps [`render_layout_box_inner`] to apply CSS visual effects (`mix-blend-mode`, `transform`,
/// `clip-path`, `filter`, and `backdrop-filter`) to the element's whole painted subtree.
pub(crate) fn render_layout_box(
    box_node: &LayoutBox,
    list: &mut DisplayList,
    parent_opacity: f32,
    offset_x: f32,
    offset_y: f32,
    scroll_x: f32,
    scroll_y: f32,
    scroll_viewport: Option<Rect>,
    table_rect: Option<Rect>,
    escaped_scroll_x: f32,
    escaped_scroll_y: f32,
) {
    let s_ref = box_node.style.as_ref();

    let push_blend =
        s_ref.is_some_and(|st| st.mix_blend_mode != mango_css::values::BlendMode::Normal);
    if push_blend {
        list.push(DisplayCommand::PushBlendMode {
            mode: s_ref.unwrap().mix_blend_mode,
        });
    }

    // A CSS transform establishes a containing block for fixed/absolute descendants
    // and offsets the element and everything inside it.
    let transform_matrix = s_ref.and_then(|s| {
        if s.transform.is_identity() {
            None
        } else {
            let border_box = box_node.dimensions.border_box();
            let origin_x = border_box.x()
                + offset_x
                + resolve_origin(s.transform_origin_x, border_box.width());
            let origin_y = border_box.y()
                + offset_y
                + resolve_origin(s.transform_origin_y, border_box.height());
            // translate(origin) * transform * translate(-origin)
            let to_origin = [1.0, 0.0, 0.0, 1.0, origin_x, origin_y];
            let from_origin = [1.0, 0.0, 0.0, 1.0, -origin_x, -origin_y];
            Some(mul_matrix(
                mul_matrix(
                    to_origin,
                    s.transform
                        .to_matrix_with_size(border_box.width(), border_box.height()),
                ),
                from_origin,
            ))
        }
    });

    if let Some(matrix) = transform_matrix {
        list.push(DisplayCommand::PushTransform { matrix });
    }

    let border_box = box_node.dimensions.border_box();
    let elem_rect = Rect::new(
        border_box.x() + offset_x,
        border_box.y() + offset_y,
        border_box.width(),
        border_box.height(),
    );

    let push_clip_path = s_ref.is_some_and(|st| st.clip_path != mango_css::values::ClipPath::None);
    if push_clip_path {
        list.push(DisplayCommand::PushClipPath {
            clip_path: Box::new(s_ref.unwrap().clip_path.clone()),
            rect: elem_rect,
        });
    }

    let push_filter = s_ref.is_some_and(|st| !st.filter.is_empty());
    if push_filter {
        list.push(DisplayCommand::PushFilter {
            filters: s_ref.unwrap().filter.clone(),
            rect: elem_rect,
        });
    }

    let push_backdrop = s_ref.is_some_and(|st| !st.backdrop_filter.is_empty());
    if push_backdrop {
        list.push(DisplayCommand::PushBackdropFilter {
            filters: s_ref.unwrap().backdrop_filter.clone(),
            rect: elem_rect,
        });
    }

    render_layout_box_inner(
        box_node,
        list,
        parent_opacity,
        offset_x,
        offset_y,
        scroll_x,
        scroll_y,
        scroll_viewport,
        table_rect,
        escaped_scroll_x,
        escaped_scroll_y,
    );

    if push_backdrop {
        list.push(DisplayCommand::PopBackdropFilter);
    }
    if push_filter {
        list.push(DisplayCommand::PopFilter);
    }
    if push_clip_path {
        list.push(DisplayCommand::PopClipPath);
    }
    if transform_matrix.is_some() {
        list.push(DisplayCommand::PopTransform);
    }
    if push_blend {
        list.push(DisplayCommand::PopBlendMode);
    }
}

fn render_layout_box_inner(
    box_node: &LayoutBox,
    list: &mut DisplayList,
    parent_opacity: f32,
    offset_x: f32,
    offset_y: f32,
    scroll_x: f32,
    scroll_y: f32,
    scroll_viewport: Option<Rect>,
    table_rect: Option<Rect>,
    escaped_scroll_x: f32,
    escaped_scroll_y: f32,
) {
    let style = box_node.style.as_deref();
    let is_visible = style.is_none_or(|s| s.is_visible());
    let opacity = style.map_or(1.0, |s| s.opacity) * parent_opacity;

    if opacity <= 0.001 {
        return;
    }

    if let Some(s) = style
        && (s.overflow_x == mango_css::values::Overflow::Hidden
            || s.overflow_y == mango_css::values::Overflow::Hidden)
        && (box_node.dimensions.padding_box().width() <= 1.0
            || box_node.dimensions.padding_box().height() <= 1.0)
    {
        return;
    }

    let orig_pad_box = box_node.dimensions.padding_box();
    let pad_box = Rect::new(
        orig_pad_box.x() + offset_x,
        orig_pad_box.y() + offset_y,
        orig_pad_box.width(),
        orig_pad_box.height(),
    );

    let orig_border_box = box_node.dimensions.border_box();
    let border_box = Rect::new(
        orig_border_box.x() + offset_x,
        orig_border_box.y() + offset_y,
        orig_border_box.width(),
        orig_border_box.height(),
    );

    let content_x = box_node.dimensions.content.x() + offset_x;
    let content_y = box_node.dimensions.content.y() + offset_y;

    let content_box = Rect::new(
        content_x,
        content_y,
        box_node.dimensions.content.width(),
        box_node.dimensions.content.height(),
    );

    if is_visible {
        let is_text_node = matches!(box_node.box_type, BoxType::TextNode(_));
        if !is_text_node {
            paint_background(
                box_node,
                style,
                border_box,
                pad_box,
                content_box,
                opacity,
                scroll_x,
                scroll_y,
                list,
            );
            paint_borders(box_node, style, border_box, opacity, list);
        }

        paint_text(
            box_node, style, border_box, content_x, content_y, opacity, list,
        );
        paint_media(
            box_node, style, border_box, content_x, content_y, opacity, list,
        );

        let is_form_control = matches!(
            box_node.tag_name.as_deref(),
            Some("input" | "button" | "select" | "textarea" | "progress" | "meter")
        );
        if is_form_control {
            render_form_control(box_node, style, opacity, list, offset_x, offset_y);
        }

        let is_atomic = matches!(
            box_node.tag_name.as_deref(),
            Some("select" | "video" | "audio" | "canvas" | "progress" | "meter")
        );
        if is_atomic {
            return;
        }
    }

    paint_children_and_scrollbars(
        box_node,
        style,
        pad_box,
        offset_x,
        offset_y,
        scroll_x,
        scroll_y,
        opacity,
        list,
        scroll_viewport,
        table_rect,
        escaped_scroll_x,
        escaped_scroll_y,
    );
}
