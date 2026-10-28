use mango_core::{Point, Rect};
use mango_html::dom::NodeId;

use crate::box_model::BoxType;

use super::types::{FormControlHit, LayoutBox, MediaClickAction, MediaControlHit};

fn map_point_through_transform(b: &LayoutBox, point: Point) -> Option<Point> {
    if let Some(s) = &b.style {
        if !s.transform.is_identity() {
            let border_box = b.dimensions.border_box();
            let [a, b_mat, c, d, e, f] = s
                .transform
                .to_matrix_with_size(border_box.width(), border_box.height());
            let ox = crate::display_list::types::resolve_origin(
                s.transform_origin_x,
                border_box.width(),
            );
            let oy = crate::display_list::types::resolve_origin(
                s.transform_origin_y,
                border_box.height(),
            );
            let cx = border_box.x() + ox;
            let cy = border_box.y() + oy;
            let dx = point.x - cx;
            let dy = point.y - cy;
            let inv = mango_render::invert_matrix([a, b_mat, c, d, e, f])?;
            let lx = inv[0] * dx + inv[2] * dy + inv[4];
            let ly = inv[1] * dx + inv[3] * dy + inv[5];
            return Some(Point::new(cx + lx, cy + ly));
        }
    }
    Some(point)
}

fn box_clips_descendants(b: &LayoutBox) -> bool {
    let is_root_or_body = b
        .tag_name
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("html") || t.eq_ignore_ascii_case("body"));
    if is_root_or_body {
        return false;
    }
    if b.is_scroll_container() || matches!(b.box_type, BoxType::IFrame { .. }) {
        return true;
    }
    if let Some(s) = &b.style {
        matches!(
            s.contain,
            mango_css::values::Contain::Paint | mango_css::values::Contain::Strict
        )
    } else {
        false
    }
}

fn is_point_inside_box_clip(b: &LayoutBox, point: Point) -> bool {
    if let Some(s) = &b.style {
        if s.clip_path != mango_css::values::ClipPath::None {
            let border_box = b.dimensions.border_box();
            if !mango_render::is_point_inside_clip_path(&s.clip_path, &border_box, point.x, point.y)
            {
                return false;
            }
        }
        if s.clip != mango_css::values::ClipRect::Auto
            && matches!(
                s.position,
                mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
            )
        {
            if let mango_css::values::ClipRect::Rect {
                top,
                right,
                bottom,
                left,
            } = s.clip
            {
                let fs = s.font_size;
                let rfs = s.root_font_size;
                let border_box = b.dimensions.border_box();
                let bw = border_box.width();
                let bh = border_box.height();
                let top_px = match top {
                    mango_css::values::Length::Auto => 0.0,
                    l => l.to_px_with_viewport(fs, rfs, bh, bh),
                };
                let left_px = match left {
                    mango_css::values::Length::Auto => 0.0,
                    l => l.to_px_with_viewport(fs, rfs, bw, bw),
                };
                let bottom_px = match bottom {
                    mango_css::values::Length::Auto => bh,
                    l => l.to_px_with_viewport(fs, rfs, bh, bh),
                };
                let right_px = match right {
                    mango_css::values::Length::Auto => bw,
                    l => l.to_px_with_viewport(fs, rfs, bw, bw),
                };
                let w = (right_px - left_px).max(0.0);
                let h = (bottom_px - top_px).max(0.0);
                let clip_r = Rect::new(border_box.x() + left_px, border_box.y() + top_px, w, h);
                if !clip_r.contains(point) {
                    return false;
                }
            }
        }
    }
    if box_clips_descendants(b) {
        let pad_box = b.dimensions.padding_box();
        if !pad_box.contains(point) {
            return false;
        }
        if let Some(s) = &b.style {
            if s.has_border_radius() {
                let radii = s.border_radius_with_size(pad_box.width(), pad_box.height());
                if !mango_render::is_inside_rounded_rect(point.x, point.y, &pad_box, radii) {
                    return false;
                }
            }
        }
    }
    true
}

fn point_hits_box(b: &LayoutBox, point: Point) -> bool {
    if !b.is_visible() {
        return false;
    }
    if let Some(s) = &b.style {
        if s.pointer_events == mango_css::values::PointerEvents::None {
            return false;
        }
        if s.clip_path != mango_css::values::ClipPath::None {
            let border_box = b.dimensions.border_box();
            if !mango_render::is_point_inside_clip_path(&s.clip_path, &border_box, point.x, point.y)
            {
                return false;
            }
        }
        if s.clip != mango_css::values::ClipRect::Auto
            && matches!(
                s.position,
                mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
            )
        {
            if let mango_css::values::ClipRect::Rect {
                top,
                right,
                bottom,
                left,
            } = s.clip
            {
                let fs = s.font_size;
                let rfs = s.root_font_size;
                let border_box = b.dimensions.border_box();
                let bw = border_box.width();
                let bh = border_box.height();
                let top_px = match top {
                    mango_css::values::Length::Auto => 0.0,
                    l => l.to_px_with_viewport(fs, rfs, bh, bh),
                };
                let left_px = match left {
                    mango_css::values::Length::Auto => 0.0,
                    l => l.to_px_with_viewport(fs, rfs, bw, bw),
                };
                let bottom_px = match bottom {
                    mango_css::values::Length::Auto => bh,
                    l => l.to_px_with_viewport(fs, rfs, bh, bh),
                };
                let right_px = match right {
                    mango_css::values::Length::Auto => bw,
                    l => l.to_px_with_viewport(fs, rfs, bw, bw),
                };
                let w = (right_px - left_px).max(0.0);
                let h = (bottom_px - top_px).max(0.0);
                let clip_r = Rect::new(border_box.x() + left_px, border_box.y() + top_px, w, h);
                if !clip_r.contains(point) {
                    return false;
                }
            }
        }
        if s.has_border_radius() {
            let border_box = b.dimensions.border_box();
            if !border_box.contains(point) {
                return false;
            }
            let radii = s.border_radius_with_size(border_box.width(), border_box.height());
            return mango_render::is_inside_rounded_rect(point.x, point.y, &border_box, radii);
        }
    }
    b.dimensions.border_box().contains(point)
}

#[inline]
fn is_stacking_context_box(b: &LayoutBox) -> bool {
    b.tag_name
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("html"))
        || crate::display_list::paint_layer::creates_stacking_context(b, false)
}

#[inline]
fn child_point(parent: &LayoutBox, child: &LayoutBox, local_pt: Point) -> Point {
    if parent.is_scroll_container() {
        if child
            .style
            .as_ref()
            .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
        {
            local_pt
        } else {
            Point::new(
                local_pt.x + parent.scroll_offset_x,
                local_pt.y + parent.scroll_offset_y,
            )
        }
    } else {
        local_pt
    }
}

fn sorted_children_for_hit_test<'a>(
    parent: &'a LayoutBox,
) -> (Vec<&'a LayoutBox>, Vec<&'a LayoutBox>) {
    if parent.children.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let is_flex_or_grid = parent.style.as_ref().is_some_and(|s| {
        matches!(
            s.display,
            mango_css::values::Display::Flex
                | mango_css::values::Display::InlineFlex
                | mango_css::values::Display::Grid
                | mango_css::values::Display::InlineGrid
        )
    });
    let mut sorted: Vec<(usize, &'a LayoutBox)> = parent.children.iter().enumerate().collect();
    sorted.sort_by(|(idx_a, a), (idx_b, b)| {
        let is_item_a = is_flex_or_grid && !a.is_out_of_flow();
        let is_item_b = is_flex_or_grid && !b.is_out_of_flow();
        let layer_a = crate::display_list::paint_layer::paint_layer_of(a, is_item_a);
        let layer_b = crate::display_list::paint_layer::paint_layer_of(b, is_item_b);
        if layer_a != layer_b {
            return layer_b.cmp(&layer_a); // Descending paint order: top layer first
        }
        if is_flex_or_grid {
            let order_a = a.style.as_ref().map(|s| s.order).unwrap_or(0);
            let order_b = b.style.as_ref().map(|s| s.order).unwrap_or(0);
            if order_a != order_b {
                return order_b.cmp(&order_a);
            }
        }
        idx_b.cmp(idx_a)
    });

    let mut non_neg = Vec::with_capacity(sorted.len());
    let mut neg = Vec::new();
    for (_, child) in sorted {
        let is_item = is_flex_or_grid && !child.is_out_of_flow();
        if matches!(
            crate::display_list::paint_layer::paint_layer_of(child, is_item),
            crate::display_list::paint_layer::PaintLayer::NegativeZ(_)
        ) {
            neg.push(child);
        } else {
            non_neg.push(child);
        }
    }
    (non_neg, neg)
}

impl LayoutBox {
    /// Hit-tests a point against this layout box and its descendants, returning the deepest matching DOM NodeId.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        let local_pt = map_point_through_transform(self, point)?;

        let inside_clip = is_point_inside_box_clip(self, local_pt);
        let is_sc = is_stacking_context_box(self);

        if inside_clip {
            let (non_neg, neg) = sorted_children_for_hit_test(self);
            for child in non_neg {
                let child_pt = child_point(self, child, local_pt);
                if let Some(hit) = child.hit_test(child_pt) {
                    return Some(hit);
                }
            }

            if is_sc {
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(hit) = child.hit_test(child_pt) {
                        return Some(hit);
                    }
                }
                if point_hits_box(self, local_pt) {
                    return self.node_id;
                }
            } else {
                if point_hits_box(self, local_pt) {
                    return self.node_id;
                }
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(hit) = child.hit_test(child_pt) {
                        return Some(hit);
                    }
                }
            }
        } else if point_hits_box(self, local_pt) {
            return self.node_id;
        }

        None
    }

    /// Hit-tests a point against this layout box and its descendants, returning `(url, target_frame)`.
    pub fn hit_test_link_full(&self, point: Point) -> Option<(&str, Option<&str>)> {
        let local_pt = map_point_through_transform(self, point)?;

        let inside_clip = is_point_inside_box_clip(self, local_pt);
        let is_sc = is_stacking_context_box(self);

        let self_link = || -> Option<(&str, Option<&str>)> {
            if point_hits_box(self, local_pt) {
                if !self.map_areas.is_empty() {
                    let rel_x = local_pt.x - self.dimensions.content.x();
                    let rel_y = local_pt.y - self.dimensions.content.y();
                    for area in &self.map_areas {
                        if area.shape.contains_point(rel_x, rel_y)
                            && let Some(ref href) = area.href
                        {
                            return Some((href.as_str(), None));
                        }
                    }
                }
                if let Some(ref target) = self.link_target {
                    return Some((target.as_str(), self.link_frame.as_deref()));
                }
            }
            None
        };

        if inside_clip {
            let (non_neg, neg) = sorted_children_for_hit_test(self);
            for child in non_neg {
                let child_pt = child_point(self, child, local_pt);
                if let Some(target) = child.hit_test_link_full(child_pt) {
                    return Some(target);
                }
            }

            if is_sc {
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(target) = child.hit_test_link_full(child_pt) {
                        return Some(target);
                    }
                }
                if let Some(target) = self_link() {
                    return Some(target);
                }
            } else {
                if let Some(target) = self_link() {
                    return Some(target);
                }
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(target) = child.hit_test_link_full(child_pt) {
                        return Some(target);
                    }
                }
            }
        } else if let Some(target) = self_link() {
            return Some(target);
        }

        None
    }

    /// Hit-tests a point against this layout box and its descendants, returning the target link URL if clicked.
    pub fn hit_test_link(&self, point: Point) -> Option<&str> {
        self.hit_test_link_full(point).map(|(href, _)| href)
    }

    /// Hit-tests a point against this layout box and its descendants for interactive form elements.
    pub fn hit_test_form_control(&self, point: Point) -> Option<FormControlHit> {
        let local_pt = map_point_through_transform(self, point)?;

        let inside_clip = is_point_inside_box_clip(self, local_pt);
        let is_sc = is_stacking_context_box(self);

        let self_fc = |b: &LayoutBox| -> Option<FormControlHit> {
            if point_hits_box(b, local_pt)
                && let Some(ref tag) = b.tag_name
            {
                let tag_lower = tag.to_ascii_lowercase();
                if matches!(
                    tag_lower.as_str(),
                    "input" | "button" | "select" | "textarea" | "summary" | "label"
                ) {
                    let form_type = b
                        .get_attribute("type")
                        .unwrap_or(if tag_lower == "button" {
                            "button"
                        } else if tag_lower == "select" {
                            "select"
                        } else if tag_lower == "textarea" {
                            "textarea"
                        } else if tag_lower == "summary" {
                            "summary"
                        } else if tag_lower == "label" {
                            "label"
                        } else {
                            "text"
                        })
                        .to_ascii_lowercase();
                    let click_offset_x = (local_pt.x - b.dimensions.content.x()).max(0.0);
                    let checked = b.get_attribute("checked").is_some()
                        || b.get_attribute("data-mango-checked")
                            .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1");
                    return Some(FormControlHit {
                        node_id: b.node_id,
                        tag_name: tag_lower,
                        form_type,
                        name: b.get_attribute("name").unwrap_or("").to_string(),
                        value: b.get_attribute("value").unwrap_or("").to_string(),
                        checked,
                        click_offset_x,
                    });
                }
            }
            None
        };

        if inside_clip {
            let (non_neg, neg) = sorted_children_for_hit_test(self);
            for child in non_neg {
                let child_pt = child_point(self, child, local_pt);
                if let Some(hit) = child.hit_test_form_control(child_pt) {
                    return Some(hit);
                }
            }

            if is_sc {
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(hit) = child.hit_test_form_control(child_pt) {
                        return Some(hit);
                    }
                }
                if let Some(hit) = self_fc(self) {
                    return Some(hit);
                }
            } else {
                if let Some(hit) = self_fc(self) {
                    return Some(hit);
                }
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(hit) = child.hit_test_form_control(child_pt) {
                        return Some(hit);
                    }
                }
            }
        } else if let Some(hit) = self_fc(self) {
            return Some(hit);
        }

        None
    }

    /// Hit-tests a point against this layout box and its descendants for interactive media (<video> and <audio>) elements.
    pub fn hit_test_media_control(&self, point: Point) -> Option<MediaControlHit> {
        let local_pt = map_point_through_transform(self, point)?;

        let inside_clip = is_point_inside_box_clip(self, local_pt);
        let is_sc = is_stacking_context_box(self);

        let self_media = |b: &LayoutBox| -> Option<MediaControlHit> {
            if !point_hits_box(b, local_pt) {
                return None;
            }
            match &b.box_type {
                BoxType::Video {
                    src,
                    has_controls,
                    is_playing,
                    is_muted,
                    current_time,
                    duration,
                    ..
                } => {
                    let content = b.dimensions.content;
                    let action = if *has_controls && content.height() >= 36.0 {
                        let bar_h = 36.0f32.min(content.height() * 0.35);
                        let bar_y = content.y() + content.height() - bar_h;
                        if local_pt.y >= bar_y {
                            let rel_x = local_pt.x - content.x();
                            if rel_x <= 36.0 {
                                MediaClickAction::TogglePlayPause
                            } else if rel_x >= content.width() - 35.0 {
                                MediaClickAction::ToggleFullscreen
                            } else if rel_x >= content.width() - 70.0 {
                                MediaClickAction::ToggleMute
                            } else if rel_x > 36.0 {
                                let track_start = 110.0f32.min(content.width() * 0.3);
                                let track_end = (content.width() - 75.0).max(track_start + 10.0);
                                let ratio = ((rel_x - track_start) / (track_end - track_start))
                                    .clamp(0.0, 1.0);
                                MediaClickAction::Seek(ratio * duration)
                            } else {
                                MediaClickAction::TogglePlayPause
                            }
                        } else {
                            MediaClickAction::TogglePlayPause
                        }
                    } else {
                        MediaClickAction::TogglePlayPause
                    };

                    Some(MediaControlHit {
                        node_id: b.node_id,
                        is_video: true,
                        src: src.clone(),
                        is_playing: *is_playing,
                        is_muted: *is_muted,
                        current_time: *current_time,
                        duration: *duration,
                        has_controls: *has_controls,
                        action,
                    })
                }
                BoxType::Audio {
                    src,
                    has_controls,
                    is_playing,
                    is_muted,
                    current_time,
                    duration,
                    ..
                } => {
                    if !*has_controls {
                        return None;
                    }
                    let content = b.dimensions.content;
                    let rel_x = local_pt.x - content.x();
                    let action = if rel_x <= 36.0 {
                        MediaClickAction::TogglePlayPause
                    } else if rel_x >= content.width() - 45.0 {
                        MediaClickAction::ToggleMute
                    } else {
                        let track_start = 110.0f32.min(content.width() * 0.3);
                        let track_end = (content.width() - 50.0).max(track_start + 10.0);
                        let ratio =
                            ((rel_x - track_start) / (track_end - track_start)).clamp(0.0, 1.0);
                        MediaClickAction::Seek(ratio * duration)
                    };

                    Some(MediaControlHit {
                        node_id: b.node_id,
                        is_video: false,
                        src: src.clone(),
                        is_playing: *is_playing,
                        is_muted: *is_muted,
                        current_time: *current_time,
                        duration: *duration,
                        has_controls: *has_controls,
                        action,
                    })
                }
                _ => None,
            }
        };

        if inside_clip {
            let (non_neg, neg) = sorted_children_for_hit_test(self);
            for child in non_neg {
                let child_pt = child_point(self, child, local_pt);
                if let Some(hit) = child.hit_test_media_control(child_pt) {
                    return Some(hit);
                }
            }

            if is_sc {
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(hit) = child.hit_test_media_control(child_pt) {
                        return Some(hit);
                    }
                }
                if let Some(hit) = self_media(self) {
                    return Some(hit);
                }
            } else {
                if let Some(hit) = self_media(self) {
                    return Some(hit);
                }
                for child in neg {
                    let child_pt = child_point(self, child, local_pt);
                    if let Some(hit) = child.hit_test_media_control(child_pt) {
                        return Some(hit);
                    }
                }
            }
        } else if let Some(hit) = self_media(self) {
            return Some(hit);
        }

        None
    }

    /// Recursively searches for the LayoutBox corresponding to a given DOM `NodeId`.
    pub fn find_box_for_node(&self, node_id: NodeId) -> Option<&LayoutBox> {
        if self.node_id == Some(node_id) {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_box_for_node(node_id) {
                return Some(found);
            }
        }
        None
    }
}
