use mango_core::Point;
use mango_html::dom::NodeId;

use crate::box_model::BoxType;

use super::types::{FormControlHit, LayoutBox, MediaClickAction, MediaControlHit};

impl LayoutBox {
    /// Hit-tests a point against this layout box and its descendants, returning the deepest matching DOM NodeId.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        if self.is_scroll_container() {
            if self.dimensions.padding_box().contains(point) {
                for child in self.children.iter().rev() {
                    let child_pt = if child
                        .style
                        .as_ref()
                        .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
                    {
                        point
                    } else {
                        Point::new(
                            point.x + self.scroll_offset_x,
                            point.y + self.scroll_offset_y,
                        )
                    };
                    if let Some(hit) = child.hit_test(child_pt) {
                        return Some(hit);
                    }
                }
            }
        } else {
            for child in self.children.iter().rev() {
                if let Some(hit) = child.hit_test(point) {
                    return Some(hit);
                }
            }
        }
        if self.is_visible() && self.dimensions.border_box().contains(point) {
            return self.node_id;
        }
        None
    }

    /// Hit-tests a point against this layout box and its descendants, returning `(url, target_frame)`.
    pub fn hit_test_link_full(&self, point: Point) -> Option<(&str, Option<&str>)> {
        if self.is_scroll_container() {
            if self.dimensions.padding_box().contains(point) {
                for child in self.children.iter().rev() {
                    let child_pt = if child
                        .style
                        .as_ref()
                        .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
                    {
                        point
                    } else {
                        Point::new(
                            point.x + self.scroll_offset_x,
                            point.y + self.scroll_offset_y,
                        )
                    };
                    if let Some(target) = child.hit_test_link_full(child_pt) {
                        return Some(target);
                    }
                }
            }
        } else {
            // Check children in reverse order (top-most in stacking order)
            for child in self.children.iter().rev() {
                if let Some(target) = child.hit_test_link_full(point) {
                    return Some(target);
                }
            }
        }
        let border_box = self.dimensions.border_box();
        if self.is_visible() && border_box.contains(point) {
            if !self.map_areas.is_empty() {
                let rel_x = point.x - self.dimensions.content.x();
                let rel_y = point.y - self.dimensions.content.y();
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
    }

    /// Hit-tests a point against this layout box and its descendants, returning the target link URL if clicked.
    pub fn hit_test_link(&self, point: Point) -> Option<&str> {
        self.hit_test_link_full(point).map(|(href, _)| href)
    }

    /// Hit-tests a point against this layout box and its descendants for interactive form elements.
    pub fn hit_test_form_control(&self, point: Point) -> Option<FormControlHit> {
        if self.is_scroll_container() {
            if self.dimensions.padding_box().contains(point) {
                for child in self.children.iter().rev() {
                    let child_pt = if child
                        .style
                        .as_ref()
                        .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
                    {
                        point
                    } else {
                        Point::new(
                            point.x + self.scroll_offset_x,
                            point.y + self.scroll_offset_y,
                        )
                    };
                    if let Some(hit) = child.hit_test_form_control(child_pt) {
                        return Some(hit);
                    }
                }
            }
        } else {
            for child in self.children.iter().rev() {
                if let Some(hit) = child.hit_test_form_control(point) {
                    return Some(hit);
                }
            }
        }
        let border_box = self.dimensions.border_box();
        if self.is_visible()
            && border_box.contains(point)
            && let Some(ref tag) = self.tag_name
        {
            let tag_lower = tag.to_ascii_lowercase();
            if matches!(
                tag_lower.as_str(),
                "input" | "button" | "select" | "textarea" | "summary" | "label"
            ) {
                let form_type = self
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
                let click_offset_x = (point.x - self.dimensions.content.x()).max(0.0);
                let checked = self.get_attribute("checked").is_some()
                    || self
                        .get_attribute("data-mango-checked")
                        .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1");
                return Some(FormControlHit {
                    node_id: self.node_id,
                    tag_name: tag_lower,
                    form_type,
                    name: self.get_attribute("name").unwrap_or("").to_string(),
                    value: self.get_attribute("value").unwrap_or("").to_string(),
                    checked,
                    click_offset_x,
                });
            }
        }
        None
    }

    /// Hit-tests a point against this layout box and its descendants for interactive media (<video> and <audio>) elements.
    pub fn hit_test_media_control(&self, point: Point) -> Option<MediaControlHit> {
        if self.is_scroll_container() {
            if self.dimensions.padding_box().contains(point) {
                for child in self.children.iter().rev() {
                    let child_pt = if child
                        .style
                        .as_ref()
                        .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
                    {
                        point
                    } else {
                        Point::new(
                            point.x + self.scroll_offset_x,
                            point.y + self.scroll_offset_y,
                        )
                    };
                    if let Some(hit) = child.hit_test_media_control(child_pt) {
                        return Some(hit);
                    }
                }
            }
        } else {
            for child in self.children.iter().rev() {
                if let Some(hit) = child.hit_test_media_control(point) {
                    return Some(hit);
                }
            }
        }
        if !self.is_visible() {
            return None;
        }
        let border_box = self.dimensions.border_box();
        if !border_box.contains(point) {
            return None;
        }

        match &self.box_type {
            BoxType::Video {
                src,
                has_controls,
                is_playing,
                is_muted,
                current_time,
                duration,
                ..
            } => {
                let content = self.dimensions.content;
                let action = if *has_controls && content.height() >= 36.0 {
                    let bar_h = 36.0f32.min(content.height() * 0.35);
                    let bar_y = content.y() + content.height() - bar_h;
                    if point.y >= bar_y {
                        let rel_x = point.x - content.x();
                        if rel_x <= 36.0 {
                            MediaClickAction::TogglePlayPause
                        } else if rel_x >= content.width() - 35.0 {
                            MediaClickAction::ToggleFullscreen
                        } else if rel_x >= content.width() - 70.0 {
                            MediaClickAction::ToggleMute
                        } else if rel_x > 36.0 {
                            let track_start = 110.0f32.min(content.width() * 0.3);
                            let track_end = (content.width() - 75.0).max(track_start + 10.0);
                            let ratio =
                                ((rel_x - track_start) / (track_end - track_start)).clamp(0.0, 1.0);
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
                    node_id: self.node_id,
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
                let content = self.dimensions.content;
                let rel_x = point.x - content.x();
                let action = if rel_x <= 36.0 {
                    MediaClickAction::TogglePlayPause
                } else if rel_x >= content.width() - 45.0 {
                    MediaClickAction::ToggleMute
                } else {
                    let track_start = 110.0f32.min(content.width() * 0.3);
                    let track_end = (content.width() - 50.0).max(track_start + 10.0);
                    let ratio = ((rel_x - track_start) / (track_end - track_start)).clamp(0.0, 1.0);
                    MediaClickAction::Seek(ratio * duration)
                };

                Some(MediaControlHit {
                    node_id: self.node_id,
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
