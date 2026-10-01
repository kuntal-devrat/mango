//! Box tree generation: constructs layout boxes from styled nodes, including anonymous block boxes.
//!
//! Implements CSS 2.1 §9.2.1.1: If a block container has mixed inline and block children,
//! contiguous sequences of inline boxes are enclosed in an anonymous block box.

use mango_css::computed::ComputedStyle;
use mango_css::values::Display;
use mango_html::dom::NodeId;

use crate::box_model::{BoxType, IFrameSandbox};
use crate::dimensions::Dimensions;
use crate::style_tree::StyledNode;

/// A node in the layout tree representing a visual box.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutBox {
    /// The specific type of layout box (Block, Inline, AnonymousBlock, TextNode).
    pub box_type: BoxType,
    /// Box geometry (content, padding, border, margin).
    pub dimensions: Dimensions,
    /// Computed CSS style for this box, if backed by an element or anonymous block.
    pub style: Option<ComputedStyle>,
    /// Child boxes.
    pub children: Vec<LayoutBox>,
    /// Hyperlink URL target if this box or an ancestor is an `<a>` element.
    pub link_target: Option<String>,
    /// DOM node identifier, if backed by a DOM element.
    pub node_id: Option<NodeId>,
    /// Element tag name (e.g. "div", "input", "table"), if backed by an element.
    pub tag_name: Option<String>,
    /// Element HTML attributes, if backed by an element.
    pub attributes: Vec<(String, String)>,
    /// Image map active hit-testing areas if this box is an <img> or <object> with usemap.
    pub map_areas: Vec<MapArea>,
    /// Dirty flag for display list invalidation (OPT-005).
    pub is_dirty: bool,
    /// Unflattened original child boxes preserved for idempotent relayout (GAP-003).
    pub raw_children: Option<Vec<LayoutBox>>,
    /// Horizontal scroll offset for scrollable containers (PRD 8.1).
    pub scroll_offset_x: f32,
    /// Vertical scroll offset for scrollable containers (PRD 8.1).
    pub scroll_offset_y: f32,
}

/// Supported geometric shapes for `<area>` elements in HTML image maps.
#[derive(Debug, Clone, PartialEq)]
pub enum AreaShape {
    Default,
    Rect { left: f32, top: f32, right: f32, bottom: f32 },
    Circle { cx: f32, cy: f32, r: f32 },
    Poly { points: Vec<(f32, f32)> },
}

impl AreaShape {
    pub fn parse(shape_str: Option<&str>, coords_str: &str) -> Self {
        let shape = shape_str.unwrap_or("rect").to_ascii_lowercase();
        let coords = parse_coords(coords_str);
        match shape.as_str() {
            "circle" | "circ" => {
                if coords.len() >= 3 {
                    AreaShape::Circle {
                        cx: coords[0],
                        cy: coords[1],
                        r: coords[2].max(0.0),
                    }
                } else {
                    AreaShape::Default
                }
            }
            "poly" | "polygon" => {
                let mut points = Vec::new();
                for chunk in coords.chunks_exact(2) {
                    points.push((chunk[0], chunk[1]));
                }
                if points.len() >= 3 {
                    AreaShape::Poly { points }
                } else {
                    AreaShape::Default
                }
            }
            "default" => AreaShape::Default,
            _ => {
                if coords.len() >= 4 {
                    let left = coords[0].min(coords[2]);
                    let top = coords[1].min(coords[3]);
                    let right = coords[0].max(coords[2]);
                    let bottom = coords[1].max(coords[3]);
                    AreaShape::Rect { left, top, right, bottom }
                } else {
                    AreaShape::Default
                }
            }
        }
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        match self {
            AreaShape::Default => true,
            AreaShape::Rect { left, top, right, bottom } => {
                x >= *left && x <= *right && y >= *top && y <= *bottom
            }
            AreaShape::Circle { cx, cy, r } => {
                let dx = x - cx;
                let dy = y - cy;
                dx * dx + dy * dy <= r * r
            }
            AreaShape::Poly { points } => {
                if points.len() < 3 {
                    return false;
                }
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let (xi, yi) = points[i];
                    let (xj, yj) = points[j];
                    let intersect = ((yi > y) != (yj > y))
                        && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
                    if intersect {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }
}

fn parse_coords(coords_str: &str) -> Vec<f32> {
    coords_str
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect()
}

/// An active clickable area within an HTML image map (`<map>` / `<area>`).
#[derive(Debug, Clone, PartialEq)]
pub struct MapArea {
    pub shape: AreaShape,
    pub href: Option<String>,
    pub alt: Option<String>,
    pub target: Option<String>,
}

static IMAGE_MAP_REGISTRY: std::sync::OnceLock<std::sync::RwLock<std::collections::HashMap<String, Vec<MapArea>>>> =
    std::sync::OnceLock::new();

fn image_map_registry() -> &'static std::sync::RwLock<std::collections::HashMap<String, Vec<MapArea>>> {
    IMAGE_MAP_REGISTRY.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()))
}

pub fn register_image_map(name: &str, areas: Vec<MapArea>) {
    if let Ok(mut reg) = image_map_registry().write() {
        reg.insert(name.to_ascii_lowercase(), areas);
    }
}

pub fn get_image_map(name: &str) -> Option<Vec<MapArea>> {
    let clean_name = name.trim().trim_start_matches('#').to_ascii_lowercase();
    if let Ok(reg) = image_map_registry().read() {
        reg.get(&clean_name).cloned()
    } else {
        None
    }
}

pub fn clear_image_maps() {
    if let Ok(mut reg) = image_map_registry().write() {
        reg.clear();
    }
}


/// Result of hit-testing an interactive form control in the box tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FormControlHit {
    pub node_id: Option<NodeId>,
    pub tag_name: String,
    pub form_type: String,
    pub name: String,
    pub value: String,
    pub checked: bool,
    pub click_offset_x: f32,
}

/// Result of hit-testing an interactive media (<video> or <audio>) element.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaControlHit {
    pub node_id: Option<NodeId>,
    pub is_video: bool,
    pub src: String,
    pub is_playing: bool,
    pub is_muted: bool,
    pub current_time: f32,
    pub duration: f32,
    pub has_controls: bool,
    pub action: MediaClickAction,
}

/// The specific user interaction triggered on a media element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaClickAction {
    TogglePlayPause,
    ToggleMute,
    Seek(f32),
    ToggleFullscreen,
}

impl LayoutBox {
    pub fn new(box_type: BoxType, style: Option<ComputedStyle>) -> Self {
        Self {
            box_type,
            dimensions: Dimensions::default(),
            style,
            children: Vec::new(),
            link_target: None,
            node_id: None,
            tag_name: None,
            attributes: Vec::new(),
            map_areas: Vec::new(),
            is_dirty: true,
            raw_children: None,
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
        }
    }

    /// Marks this box and all descendants as dirty for display list invalidation (OPT-005).
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        for child in &mut self.children {
            child.mark_dirty();
        }
    }

    /// Marks this box and all descendants as clean after display list generation (OPT-005).
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
        for child in &mut self.children {
            child.mark_clean();
        }
    }

    /// Returns true if this box acts as a scroll container (PRD 8.1).
    pub fn is_scroll_container(&self) -> bool {
        if let Some(s) = &self.style {
            matches!(
                s.overflow_x,
                mango_css::values::Overflow::Scroll | mango_css::values::Overflow::Auto
            ) || matches!(
                s.overflow_y,
                mango_css::values::Overflow::Scroll | mango_css::values::Overflow::Auto
            )
        } else {
            false
        }
    }

    /// Computes the bounding extent of all in-flow child boxes relative to content box origin.
    pub fn scrollable_extent(&self) -> (f32, f32) {
        let mut max_x = self.dimensions.content.width();
        let mut max_y = self.dimensions.content.height();
        let content_origin_x = self.dimensions.content.x();
        let content_origin_y = self.dimensions.content.y();

        for child in &self.children {
            let is_out_of_flow = child
                .style
                .as_ref()
                .is_some_and(|s| matches!(s.position, mango_css::values::Position::Fixed | mango_css::values::Position::Absolute));
            if !is_out_of_flow {
                let mb = child.dimensions.margin_box();
                max_x = max_x.max(mb.right() - content_origin_x);
                max_y = max_y.max(mb.bottom() - content_origin_y);
            }
        }
        (max_x, max_y)
    }

    /// Returns the maximum allowed scroll offsets (max_scroll_x, max_scroll_y).
    pub fn max_scroll(&self) -> (f32, f32) {
        let (extent_x, extent_y) = self.scrollable_extent();
        let max_x = (extent_x - self.dimensions.content.width()).max(0.0);
        let max_y = (extent_y - self.dimensions.content.height()).max(0.0);
        (max_x, max_y)
    }

    /// Scrolls the container by (delta_x, delta_y), returning the delta consumed.
    pub fn scroll_by(&mut self, delta_x: f32, delta_y: f32) -> (f32, f32) {
        let (max_x, max_y) = self.max_scroll();
        let old_x = self.scroll_offset_x;
        let old_y = self.scroll_offset_y;

        self.scroll_offset_x = (self.scroll_offset_x + delta_x).clamp(0.0, max_x);
        self.scroll_offset_y = (self.scroll_offset_y + delta_y).clamp(0.0, max_y);

        (self.scroll_offset_x - old_x, self.scroll_offset_y - old_y)
    }

    /// Finds the innermost scrollable container that contains the point `(x, y)`.
    pub fn find_scrollable_container_at(&self, x: f32, y: f32) -> Option<&LayoutBox> {
        let pad_box = self.dimensions.padding_box();
        if !pad_box.contains(mango_core::Point::new(x, y)) {
            return None;
        }

        for child in self.children.iter().rev() {
            if let Some(found) = child.find_scrollable_container_at(x, y) {
                return Some(found);
            }
        }

        if self.is_scroll_container() {
            Some(self)
        } else {
            None
        }
    }

    /// Scrolls the innermost container at (x, y) by (delta_x, delta_y).
    /// If that container cannot consume the entire delta (reaches boundary),
    /// propagates the remainder up to outer nested scroll containers.
    /// Returns the total unconsumed (delta_x, delta_y) that can be applied to page scroll.
    pub fn dispatch_nested_scroll(&mut self, x: f32, y: f32, mut delta_x: f32, mut delta_y: f32) -> (f32, f32) {
        let pad_box = self.dimensions.padding_box();
        if !pad_box.contains(mango_core::Point::new(x, y)) {
            return (delta_x, delta_y);
        }

        // Recurse into children first (innermost container gets first opportunity)
        for child in self.children.iter_mut().rev() {
            let (rem_x, rem_y) = child.dispatch_nested_scroll(x, y, delta_x, delta_y);
            delta_x = rem_x;
            delta_y = rem_y;
            if delta_x == 0.0 && delta_y == 0.0 {
                return (0.0, 0.0);
            }
        }

        // If this box is a scroll container, consume whatever delta remains
        if self.is_scroll_container() {
            let (consumed_x, consumed_y) = self.scroll_by(delta_x, delta_y);
            delta_x -= consumed_x;
            delta_y -= consumed_y;
        }

        (delta_x, delta_y)
    }

    pub fn new_anonymous_block(style: Option<ComputedStyle>) -> Self {
        let style = style.map(|mut s| {
            s.display = Display::Block;
            s.clear = mango_css::values::Clear::None;
            s.float = mango_css::values::Float::None;
            s.position = mango_css::values::Position::Static;
            s.margin_top = mango_css::values::Length::Px(0.0);
            s.margin_right = mango_css::values::Length::Px(0.0);
            s.margin_bottom = mango_css::values::Length::Px(0.0);
            s.margin_left = mango_css::values::Length::Px(0.0);
            s.padding_top = mango_css::values::Length::Px(0.0);
            s.padding_right = mango_css::values::Length::Px(0.0);
            s.padding_bottom = mango_css::values::Length::Px(0.0);
            s.padding_left = mango_css::values::Length::Px(0.0);
            s.border_top_width = 0.0;
            s.border_right_width = 0.0;
            s.border_bottom_width = 0.0;
            s.border_left_width = 0.0;
            s.width = mango_css::values::Length::Auto;
            s.height = mango_css::values::Length::Auto;
            s.min_width = mango_css::values::Length::Auto;
            s.max_width = mango_css::values::Length::Auto;
            s.min_height = mango_css::values::Length::Auto;
            s.max_height = mango_css::values::Length::Auto;
            s.background_color = mango_core::Color::TRANSPARENT;
            s.background_image = None;
            s.box_shadow = None;
            s.outline_width = 0.0;
            s
        });
        Self {
            box_type: BoxType::AnonymousBlock,
            dimensions: Dimensions::default(),
            style,
            children: Vec::new(),
            link_target: None,
            node_id: None,
            tag_name: None,
            attributes: Vec::new(),
            map_areas: Vec::new(),
            is_dirty: true,
            raw_children: None,
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
        }
    }

    /// Looks up an attribute value by name (case-insensitive).
    #[inline]
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Hit-tests a point against this layout box and its descendants, returning the deepest matching DOM NodeId.
    pub fn hit_test(&self, point: mango_core::Point) -> Option<mango_html::dom::NodeId> {
        for child in self.children.iter().rev() {
            if let Some(hit) = child.hit_test(point) {
                return Some(hit);
            }
        }
        if self.dimensions.border_box().contains(point) {
            return self.node_id;
        }
        None
    }

    /// Hit-tests a point against this layout box and its descendants, returning the target link URL if clicked.
    pub fn hit_test_link(&self, point: mango_core::Point) -> Option<&str> {
        // Check children in reverse order (top-most in stacking order)
        for child in self.children.iter().rev() {
            if let Some(target) = child.hit_test_link(point) {
                return Some(target);
            }
        }
        let border_box = self.dimensions.border_box();
        if border_box.contains(point) {
            if !self.map_areas.is_empty() {
                let rel_x = point.x - self.dimensions.content.x();
                let rel_y = point.y - self.dimensions.content.y();
                for area in &self.map_areas {
                    if area.shape.contains_point(rel_x, rel_y) {
                        if let Some(ref href) = area.href {
                            return Some(href.as_str());
                        }
                    }
                }
            }
            return self.link_target.as_deref();
        }
        None
    }

    /// Hit-tests a point against this layout box and its descendants for interactive form elements.
    pub fn hit_test_form_control(&self, point: mango_core::Point) -> Option<FormControlHit> {
        for child in self.children.iter().rev() {
            if let Some(hit) = child.hit_test_form_control(point) {
                return Some(hit);
            }
        }
        let border_box = self.dimensions.border_box();
        if border_box.contains(point)
            && let Some(ref tag) = self.tag_name
        {
            let tag_lower = tag.to_ascii_lowercase();
            if matches!(tag_lower.as_str(), "input" | "button" | "select" | "textarea" | "summary" | "label") {
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
                let click_offset_x = (point.x - self.dimensions.content.x() - 4.0).max(0.0);
                return Some(FormControlHit {
                    node_id: self.node_id,
                    tag_name: tag_lower,
                    form_type,
                    name: self.get_attribute("name").unwrap_or("").to_string(),
                    value: self.get_attribute("value").unwrap_or("").to_string(),
                    checked: self.get_attribute("checked").is_some(),
                    click_offset_x,
                });
            }
        }
        None
    }

    /// Hit-tests a point against this layout box and its descendants for interactive media (<video> and <audio>) elements.
    pub fn hit_test_media_control(&self, point: mango_core::Point) -> Option<MediaControlHit> {
        for child in self.children.iter().rev() {
            if let Some(hit) = child.hit_test_media_control(point) {
                return Some(hit);
            }
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
                            let ratio = ((rel_x - track_start) / (track_end - track_start)).clamp(0.0, 1.0);
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

    #[inline]
    pub fn is_block(&self) -> bool {
        match &self.box_type {
            BoxType::BlockNode | BoxType::AnonymousBlock => true,
            BoxType::InlineNode | BoxType::InlineBlock | BoxType::TextNode(_) => false,
            _ => {
                if let Some(style) = &self.style {
                    match style.display {
                        mango_css::values::Display::Block
                        | mango_css::values::Display::Flex
                        | mango_css::values::Display::Grid => true,
                        mango_css::values::Display::Inline
                        | mango_css::values::Display::InlineBlock
                        | mango_css::values::Display::InlineFlex
                        | mango_css::values::Display::InlineGrid => false,
                        _ => self.box_type.is_block(),
                    }
                } else {
                    self.box_type.is_block()
                }
            }
        }
    }

    #[inline]
    pub fn is_inline(&self) -> bool {
        !self.is_block()
    }

    #[inline]
    pub fn is_anonymous(&self) -> bool {
        self.box_type.is_anonymous()
    }

    #[inline]
    pub fn is_text(&self) -> bool {
        self.box_type.is_text()
    }

    #[inline]
    pub fn is_replaced(&self) -> bool {
        self.box_type.is_replaced()
    }

    #[inline]
    pub fn is_iframe(&self) -> bool {
        self.box_type.is_iframe()
    }

    pub fn text(&self) -> Option<&str> {
        match &self.box_type {
            BoxType::TextNode(t) => Some(t.as_str()),
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

fn collect_child_boxes(node: &StyledNode, out: &mut Vec<LayoutBox>) {
    if node.style.display == Display::Contents {
        for child in &node.children {
            collect_child_boxes(child, out);
        }
    } else {
        out.push(build_box_tree(node));
    }
}

/// Builds the complete layout box tree from a styled node.
pub fn build_box_tree(styled_node: &StyledNode) -> LayoutBox {
    // Detect <img> elements — generate a ReplacedElement box
    if styled_node.tag_name.as_deref() == Some("img") {
        return build_replaced_element(styled_node);
    }

    // Detect <embed> elements — generate a ReplacedElement box or fallback container
    if styled_node.tag_name.as_deref() == Some("embed") {
        return build_embed_element(styled_node);
    }

    // Detect <object> elements — decode media or gracefully fall back to child nodes
    if styled_node.tag_name.as_deref() == Some("object") {
        if let Some(obj_box) = try_build_object_element(styled_node) {
            return obj_box;
        }
    }

    // Detect <svg> elements — generate a ReplacedElement box with vector rasterization
    if styled_node.tag_name.as_deref() == Some("svg") {
        return build_svg_replaced_element(styled_node);
    }

    // Detect <iframe> elements — generate a nested browsing context box
    if styled_node.tag_name.as_deref() == Some("iframe") {
        return build_iframe_element(styled_node);
    }

    // Detect <video> elements — generate a Video replaced box
    if styled_node.tag_name.as_deref() == Some("video") {
        return build_video_element(styled_node);
    }

    // Detect <audio> elements — generate an Audio replaced box
    if styled_node.tag_name.as_deref() == Some("audio") {
        return build_audio_element(styled_node);
    }

    // Detect <canvas> elements — generate a Canvas replaced box
    if styled_node.tag_name.as_deref() == Some("canvas") {
        return build_canvas_element(styled_node);
    }

    // Detect <select> elements — extract selected option and suppress child option text flow
    if styled_node.tag_name.as_deref() == Some("select") {
        let is_multiple = styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("multiple"));
        let mut selected_texts = Vec::new();
        let mut first_text = None;

        fn extract_option_text(node: &StyledNode) -> String {
            let mut s = String::new();
            if let Some(ref t) = node.text {
                s.push_str(t);
            }
            for c in &node.children {
                s.push_str(&extract_option_text(c));
            }
            s
        }

        fn collect_options<'a>(node: &'a StyledNode, options: &mut Vec<&'a StyledNode>) {
            for c in &node.children {
                if c.tag_name.as_deref() == Some("option") {
                    options.push(c);
                } else if c.tag_name.as_deref() == Some("optgroup") {
                    collect_options(c, options);
                }
            }
        }

        let mut all_options = Vec::new();
        collect_options(styled_node, &mut all_options);

        for opt in all_options {
            let opt_text = extract_option_text(opt).trim().to_string();
            let is_selected = opt.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("selected"));
            if is_selected && !opt_text.is_empty() {
                selected_texts.push(opt_text.clone());
            }
            if first_text.is_none() && !opt_text.is_empty() {
                first_text = Some(opt_text);
            }
        }

        let box_type = match styled_node.style.display {
            Display::Block => BoxType::BlockNode,
            _ => BoxType::InlineBlock,
        };
        let mut root_box = LayoutBox::new(box_type, Some(styled_node.style.clone()));
        root_box.node_id = styled_node.node_id;
        root_box.tag_name = styled_node.tag_name.clone();
        root_box.attributes = styled_node.attributes.clone();

        if is_multiple {
            root_box.attributes.push(("_mango_is_multiple".to_string(), "true".to_string()));
            let text = if selected_texts.is_empty() {
                String::new()
            } else {
                selected_texts.join(", ")
            };
            root_box.attributes.push(("_mango_selected_text".to_string(), text));
        } else if let Some(text) = selected_texts.into_iter().next().or(first_text) {
            root_box.attributes.push(("_mango_selected_text".to_string(), text));
        }
        return root_box;
    }

    let box_type = if let Some(text) = &styled_node.text {
        BoxType::TextNode(text.clone())
    } else {
        match styled_node.style.display {
            Display::Block
            | Display::FlowRoot
            | Display::Grid
            | Display::Flex
            | Display::Table
            | Display::TableRow
            | Display::TableCell
            | Display::TableCaption
            | Display::TableRowGroup
            | Display::TableHeaderGroup
            | Display::TableFooterGroup
            | Display::TableColumn
            | Display::TableColumnGroup
            | Display::ListItem => BoxType::BlockNode,
            Display::Inline | Display::Ruby | Display::RubyBase | Display::RubyText => BoxType::InlineNode,
            Display::InlineBlock | Display::InlineFlex | Display::InlineGrid => BoxType::InlineBlock,
            Display::None | Display::Contents => BoxType::AnonymousBlock, // Handled before reaching here
        }
    };

    let mut root_box = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    root_box.node_id = styled_node.node_id;
    root_box.tag_name = styled_node.tag_name.clone();
    root_box.attributes = styled_node.attributes.clone();

    // Recursively build children, hoisting Display::Contents children directly into parent
    let mut raw_children: Vec<LayoutBox> = Vec::new();
    for child in &styled_node.children {
        collect_child_boxes(child, &mut raw_children);
    }

    let is_flex = styled_node.style.display == Display::Flex || styled_node.style.display == Display::InlineFlex;
    let is_grid = styled_node.style.display == Display::Grid || styled_node.style.display == Display::InlineGrid;
    let is_table_context = matches!(
        styled_node.style.display,
        Display::Table
            | Display::TableRow
            | Display::TableRowGroup
            | Display::TableHeaderGroup
            | Display::TableFooterGroup
            | Display::TableColumn
            | Display::TableColumnGroup
    ) || matches!(
        styled_node.tag_name.as_deref(),
        Some("table") | Some("tbody") | Some("thead") | Some("tfoot") | Some("tr") | Some("colgroup")
    );

    if is_flex || is_grid {
        // In CSS Flexbox and Grid, children become items directly without anonymous block wrapping
        root_box.children = raw_children;
    } else if is_table_context {
        // Filter out whitespace-only text nodes between table rows, cells, and colgroups
        root_box.children = raw_children
            .into_iter()
            .filter(|c| {
                if let Some(t) = c.text() {
                    !t.chars().all(|ch| ch.is_ascii_whitespace())
                } else {
                    true
                }
            })
            .collect();
    } else if root_box.is_block() {
        // Apply CSS 2.1 §9.2.1.1: If block container has mixed block and inline children,
        // wrap contiguous sequences of inline children into anonymous block boxes.
        let has_blocks = raw_children.iter().any(|c| c.is_block());
        let has_inlines = raw_children.iter().any(|c| c.is_inline());

        if has_blocks && has_inlines {
            let mut final_children = Vec::new();
            let mut current_anonymous: Option<LayoutBox> = None;

            for child in raw_children {
                if child.is_inline() {
                    let anon = current_anonymous.get_or_insert_with(|| {
                        LayoutBox::new_anonymous_block(Some(styled_node.style.clone()))
                    });
                    anon.children.push(child);
                } else {
                    if let Some(anon) = current_anonymous.take() {
                        let is_all_empty = anon.children.iter().all(|c| {
                            c.text().map(|t| t.chars().all(|ch| ch.is_ascii_whitespace())).unwrap_or(false)
                        });
                        if !is_all_empty {
                            final_children.push(anon);
                        }
                    }
                    final_children.push(child);
                }
            }

            if let Some(anon) = current_anonymous.take() {
                let is_all_empty = anon.children.iter().all(|c| {
                    c.text().map(|t| t.chars().all(|ch| ch.is_ascii_whitespace())).unwrap_or(false)
                });
                if !is_all_empty {
                    final_children.push(anon);
                }
            }

            root_box.children = final_children;
        } else {
            root_box.children = raw_children;
        }
    } else {
        root_box.children = raw_children;
    }

    if styled_node.tag_name.as_deref() == Some("a")
        && let Some((_, href)) = styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("href"))
    {
        let href_str = href.clone();
        root_box.link_target = Some(href_str.clone());
        for child in &mut root_box.children {
            propagate_link_target(child, &href_str);
        }
    }

    root_box
}

fn propagate_link_target(box_node: &mut LayoutBox, href: &str) {
    if box_node.link_target.is_none() {
        box_node.link_target = Some(href.to_string());
    }
    for child in &mut box_node.children {
        propagate_link_target(child, href);
    }
}

/// Splits a `srcset` attribute into candidates without incorrectly splitting on commas inside `data:` URIs.
pub fn parse_srcset_candidates(srcset: &str) -> Vec<&str> {
    let mut list = Vec::new();
    let s = srcset.trim();
    if s.is_empty() {
        return list;
    }
    let mut start = 0;
    let mut in_data_header = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if s[i..].starts_with("data:") {
            in_data_header = true;
        }
        if bytes[i] == b',' {
            if in_data_header {
                in_data_header = false;
            } else {
                let candidate = s[start..i].trim();
                if !candidate.is_empty() {
                    list.push(candidate);
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        list.push(tail);
    }
    list
}

/// Builds a [`ReplacedElement`](BoxType::ReplacedElement) for `<img>` elements.
///
/// Reads `src`, `width`, and `height` attributes. Supports `data:` URIs for inline images.
fn build_replaced_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };

    // Parse explicit width/height: prefer CSS style if px, otherwise HTML attributes
    let explicit_w: Option<f32> = match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };
    let explicit_h: Option<f32> = match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };

    // Helper to try decoding data URI or getting from cache (including with https: prefix)
    let lookup_image = |url: &str| -> Option<mango_render::image_decode::DecodedImage> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(img) = mango_render::image_decode::decode_data_uri(trimmed) {
            return Some(img);
        }
        if let Some(img) = mango_render::image_decode::get_cached_image(trimmed) {
            return Some(img);
        }
        if trimmed.starts_with("//") {
            let https_url = format!("https:{trimmed}");
            if let Some(img) = mango_render::image_decode::get_cached_image(&https_url) {
                return Some(img);
            }
        }
        None
    };

    // 1. First priority (GAP-019): check <picture> / <source> responsive image candidates
    let mut decoded = None;
    if let Some(pic_sources) = get_attr("_mango_picture_sources") {
        for line in pic_sources.lines() {
            let mut parts = line.split('\t');
            let srcset = parts.next().unwrap_or("");
            let media = parts.next().unwrap_or("");
            let mime = parts.next().unwrap_or("");

            // Check media query against viewport
            if !media.is_empty() && !mango_css::matches_media_query_size(media, 800.0, 600.0) {
                continue;
            }
            // Check MIME type
            if !mime.is_empty() {
                let lower_mime = mime.to_ascii_lowercase();
                if !lower_mime.starts_with("image/") {
                    continue;
                }
            }
            // Candidate matched! Try each candidate in srcset
            for candidate in parse_srcset_candidates(srcset) {
                let url = candidate.split_whitespace().next().unwrap_or("");
                if let Some(img) = lookup_image(url) {
                    decoded = Some(img);
                    break;
                }
            }
            if decoded.is_some() {
                break;
            }
        }
    }

    let src = get_attr("src").unwrap_or("");
    if decoded.is_none() {
        decoded = lookup_image(src);
    }

    // Fallback: check srcset (candidates separated by commas)
    if decoded.is_none() && let Some(srcset) = get_attr("srcset") {
        for candidate in parse_srcset_candidates(srcset) {
            let url = candidate.split_whitespace().next().unwrap_or("");
            if let Some(img) = lookup_image(url) {
                decoded = Some(img);
                break;
            }
        }
    }

    // Fallback: check data-src
    if decoded.is_none() && let Some(data_src) = get_attr("data-src") {
        decoded = lookup_image(data_src);
    }

    let (intrinsic_width, intrinsic_height, pixels) = if let Some(img) = decoded {
        let natural_w = img.width as f32;
        let natural_h = img.height as f32;
        let aspect_ratio = if let Some(css_ratio) = styled_node.style.aspect_ratio.filter(|&r| r > 0.0) {
            css_ratio
        } else if natural_h > 0.0 {
            natural_w / natural_h
        } else {
            1.0
        };

        let (w, h) = match (explicit_w, explicit_h) {
            (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
            (Some(ew), None) => {
                let ew = ew.max(1.0);
                (ew, (ew / aspect_ratio).max(1.0))
            }
            (None, Some(eh)) => {
                let eh = eh.max(1.0);
                (((eh * aspect_ratio).max(1.0)), eh)
            }
            (None, None) => (natural_w.max(1.0), natural_h.max(1.0)),
        };

        let scaled = if !src.is_empty() {
            mango_render::image_decode::get_or_resize_cached(src, &img, w as u32, h as u32)
        } else {
            img.resize(w as u32, h as u32)
        };
        (w, h, scaled.pixels)
    } else {
        // Use placeholder for missing/failed images
        let aspect_ratio = styled_node.style.aspect_ratio.filter(|&r| r > 0.0).unwrap_or(4.0 / 3.0);
        let (w, h) = match (explicit_w, explicit_h) {
            (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
            (Some(ew), None) => (ew.max(1.0), (ew / aspect_ratio).max(1.0)),
            (None, Some(eh)) => (((eh * aspect_ratio).max(1.0)), eh.max(1.0)),
            (None, None) => (64.0, 64.0),
        };
        let placeholder = mango_render::image_decode::create_placeholder(w as u32, h as u32);
        (w, h, placeholder.pixels)
    };

    let box_type = BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    if let Some(usemap) = get_attr("usemap") {
        if let Some(areas) = get_image_map(usemap) {
            b.map_areas = areas;
        }
    }

    b
}

/// Builds a layout box for `<embed>` elements.
fn build_embed_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let explicit_w: Option<f32> = match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };
    let explicit_h: Option<f32> = match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };

    let w = explicit_w.unwrap_or(300.0).max(1.0);
    let h = explicit_h.unwrap_or(150.0).max(1.0);

    let src = get_attr("src").unwrap_or("").trim();
    let decoded = if !src.is_empty() {
        if let Some(img) = mango_render::image_decode::decode_data_uri(src) {
            Some(img)
        } else if let Some(img) = mango_render::image_decode::get_cached_image(src) {
            Some(img)
        } else {
            None
        }
    } else {
        None
    };

    let pixels = if let Some(img) = decoded {
        img.resize(w as u32, h as u32).pixels
    } else {
        mango_render::image_decode::create_placeholder(w as u32, h as u32).pixels
    };

    let box_type = BoxType::ReplacedElement {
        intrinsic_width: w,
        intrinsic_height: h,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();
    b
}

/// Attempts to build a [`BoxType::ReplacedElement`] for `<object>` elements if media data is valid.
/// If media decoding fails or data is missing, returns `None` so `build_box_tree` gracefully
/// falls back to rendering the child fallback nodes.
fn try_build_object_element(styled_node: &StyledNode) -> Option<LayoutBox> {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let data = get_attr("data").or_else(|| get_attr("src"))?.trim();
    if data.is_empty() {
        return None;
    }

    let decoded = if let Some(img) = mango_render::image_decode::decode_data_uri(data) {
        Some(img)
    } else if let Some(img) = mango_render::image_decode::get_cached_image(data) {
        Some(img)
    } else {
        None
    };

    let img = decoded?;

    let explicit_w: Option<f32> = match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };
    let explicit_h: Option<f32> = match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };

    let natural_w = img.width as f32;
    let natural_h = img.height as f32;
    let aspect_ratio = if let Some(css_ratio) = styled_node.style.aspect_ratio.filter(|&r| r > 0.0) {
        css_ratio
    } else if natural_h > 0.0 {
        natural_w / natural_h
    } else {
        1.0
    };

    let (w, h) = match (explicit_w, explicit_h) {
        (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
        (Some(ew), None) => (ew.max(1.0), (ew / aspect_ratio).max(1.0)),
        (None, Some(eh)) => (((eh * aspect_ratio).max(1.0)), eh.max(1.0)),
        (None, None) => (natural_w.max(1.0), natural_h.max(1.0)),
    };

    let scaled = img.resize(w as u32, h as u32);
    let box_type = BoxType::ReplacedElement {
        intrinsic_width: w,
        intrinsic_height: h,
        pixels: scaled.pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    if let Some(usemap) = get_attr("usemap") {
        if let Some(areas) = get_image_map(usemap) {
            b.map_areas = areas;
        }
    }

    Some(b)
}

/// Builds a [`ReplacedElement`](BoxType::ReplacedElement) for `<svg>` elements.
fn build_svg_replaced_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    // Serialize styled node to SVG XML
    let svg_xml = serialize_svg_styled_node(styled_node);

    // Dimensions: check CSS style.width/height, then SVG width/height attributes, then viewBox
    let (intrinsic_w_opt, intrinsic_h_opt) = mango_render::get_svg_intrinsic_dimensions(&svg_xml);

    let explicit_w: Option<f32> = match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };

    let explicit_h: Option<f32> = match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(|v| v.trim_end_matches("px").parse().ok()),
    };

    let aspect_ratio = if let Some(css_ratio) = styled_node.style.aspect_ratio.filter(|&r| r > 0.0) {
        Some(css_ratio)
    } else if let (Some(iw), Some(ih)) = (intrinsic_w_opt, intrinsic_h_opt) {
        if ih > 0.0 {
            Some(iw / ih)
        } else {
            None
        }
    } else {
        None
    };

    let (w, h) = match (explicit_w, explicit_h) {
        (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
        (Some(ew), None) => {
            let ew = ew.max(1.0);
            if let Some(ratio) = aspect_ratio {
                (ew, (ew / ratio).max(1.0))
            } else if let Some(ih) = intrinsic_h_opt {
                (ew, ih.max(1.0))
            } else {
                (ew, ew)
            }
        }
        (None, Some(eh)) => {
            let eh = eh.max(1.0);
            if let Some(ratio) = aspect_ratio {
                (((eh * ratio).max(1.0)), eh)
            } else if let Some(iw) = intrinsic_w_opt {
                (iw.max(1.0), eh)
            } else {
                (eh, eh)
            }
        }
        (None, None) => {
            let def_w = intrinsic_w_opt.unwrap_or(300.0);
            let def_h = intrinsic_h_opt.unwrap_or_else(|| {
                if let Some(ratio) = aspect_ratio {
                    def_w / ratio
                } else {
                    150.0
                }
            });
            (def_w.max(1.0), def_h.max(1.0))
        }
    };

    let current_color = styled_node.style.color;
    let decoded = mango_render::render_svg(&svg_xml, w as u32, h as u32, current_color);

    let pixels = if let Some(img) = decoded {
        img.pixels
    } else {
        mango_render::image_decode::create_placeholder(w as u32, h as u32).pixels
    };

    let intrinsic_width = intrinsic_w_opt.unwrap_or(w);
    let intrinsic_height = intrinsic_h_opt.unwrap_or(h);

    let box_type = BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();
    b
}

fn serialize_svg_styled_node(node: &StyledNode) -> String {
    let mut out = String::new();
    serialize_svg_recursive(node, &mut out);
    out
}

fn serialize_svg_recursive(node: &StyledNode, out: &mut String) {
    if let Some(ref tag) = node.tag_name {
        out.push('<');
        out.push_str(tag);
        for (k, v) in &node.attributes {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            for ch in v.chars() {
                if ch == '"' {
                    out.push_str("&quot;");
                } else if ch == '&' {
                    out.push_str("&amp;");
                } else {
                    out.push(ch);
                }
            }
            out.push('"');
        }

        if node.children.is_empty() && node.text.is_none() {
            out.push_str("/>");
        } else {
            out.push('>');
            if let Some(ref t) = node.text {
                out.push_str(t);
            }
            for child in &node.children {
                serialize_svg_recursive(child, out);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
    }
}

/// Builds a [`BoxType::IFrame`] layout box for `<iframe>` elements.
///
/// Implements HTML5 §4.8.5 `<iframe>`:
/// - Resolves intrinsic dimensions (default 300×150 per HTML5 spec, overridden by width/height attributes or CSS)
/// - Extracts security sandbox settings ([`IFrameSandbox`])
/// - Parses and lays out inner document from `srcdoc` or `data:text/html` into child boxes
/// - Sets navigation target for interactive click-through
pub fn build_iframe_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let src = get_attr("src").unwrap_or("").trim().to_string();
    let srcdoc = get_attr("srcdoc").map(|s| s.to_string());
    let sandbox = IFrameSandbox::from_attribute(get_attr("sandbox"));

    // Standard HTML5 iframe defaults to 300x150
    let mut intrinsic_width = 300.0f32;
    let mut intrinsic_height = 150.0f32;

    if let Some(w_attr) = get_attr("width") {
        if let Ok(w) = w_attr.trim_end_matches("px").parse::<f32>() {
            if w > 0.0 {
                intrinsic_width = w;
            }
        }
    }
    if let Some(h_attr) = get_attr("height") {
        if let Ok(h) = h_attr.trim_end_matches("px").parse::<f32>() {
            if h > 0.0 {
                intrinsic_height = h;
            }
        }
    }

    // Prefer CSS styled dimensions if specified in pixels
    match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::IFrame {
        src: src.clone(),
        srcdoc: srcdoc.clone(),
        sandbox,
        intrinsic_width,
        intrinsic_height,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    // If src is a link/URL, set link target for navigation when clicked if no interactive children
    if !src.is_empty() && !src.starts_with("data:") && src != "about:blank" {
        b.link_target = Some(src.clone());
    }

    // Build nested browsing context content:
    // 1. If `srcdoc` is provided, parse and lay out inner document
    // 2. If `src` is `data:text/html,...`, decode and lay out
    let inner_html = if let Some(ref doc_html) = srcdoc {
        Some(doc_html.clone())
    } else if src.starts_with("data:text/html") {
        decode_data_uri_html(&src)
    } else {
        None
    };

    if let Some(html) = inner_html {
        let inner_doc = mango_html::parse_html(&html);
        if let Some(inner_styled_root) = crate::style_tree::build_style_tree(&inner_doc, &[]) {
            let inner_box = build_box_tree(&inner_styled_root);
            b.children.push(inner_box);
        }
    } else {
        // Fallback content inside <iframe>...</iframe> for older/unsupported frame markup
        for child in &styled_node.children {
            b.children.push(build_box_tree(child));
        }
    }

    b
}

/// Decodes an inline HTML data URI (`data:text/html,...` or `data:text/html;base64,...`).
fn decode_data_uri_html(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("data:")?;
    let comma_pos = rest.find(',')?;
    let metadata = &rest[..comma_pos];
    let payload = &rest[comma_pos + 1..];

    if metadata.contains(";base64") {
        let clean: Vec<u8> = payload.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        base64_decode_bytes(&clean).and_then(|bytes| String::from_utf8(bytes).ok())
    } else {
        Some(percent_decode_str(payload))
    }
}

fn percent_decode_str(input: &str) -> String {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(b);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn base64_decode_bytes(input: &[u8]) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let filtered: Vec<u8> = input.iter().copied().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);
    for chunk in filtered.chunks(4) {
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push((b2 << 6) | b3);
        }
    }
    Some(out)
}

/// Builds a [`BoxType::Video`] layout box for `<video>` elements.
///
/// Implements HTML5 §4.8.9 `<video>`:
/// - Resolves `src` from element attributes or child `<source>` elements
/// - Resolves and decodes `poster` image (supports `data:` URIs and cached HTTP URLs)
/// - Extracts playback controls (`controls`, `autoplay`, `loop`, `muted`) and runtime state
/// - Resolves intrinsic dimensions (defaults to 300×150 per HTML5 spec, or poster dimensions, overridden by CSS/attributes)
pub fn build_video_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    // 1. Resolve source: check element attribute first, then child <source> elements
    let mut src = get_attr("src").unwrap_or("").trim().to_string();
    if src.is_empty() {
        for child in &styled_node.children {
            if child.tag_name.as_deref() == Some("source") {
                if let Some((_, s)) = child.attributes.iter().find(|(k, _)| k.eq_ignore_ascii_case("src")) {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        src = trimmed.to_string();
                        break;
                    }
                }
            }
        }
    }

    // 2. Resolve poster image
    let poster = get_attr("poster").map(|s| s.trim().to_string());
    let mut poster_pixels = None;
    let mut poster_width = 0u32;
    let mut poster_height = 0u32;

    if let Some(ref poster_url) = poster {
        if poster_url.starts_with("data:image/") {
            if let Some(decoded) = mango_render::decode_data_uri(poster_url) {
                poster_width = decoded.width;
                poster_height = decoded.height;
                poster_pixels = Some(decoded.pixels);
            }
        } else if let Some(cached) = mango_render::get_cached_image(poster_url) {
            poster_width = cached.width;
            poster_height = cached.height;
            poster_pixels = Some(cached.pixels);
        }
    }

    // 3. Flags and runtime state
    let has_controls = get_attr("controls").is_some()
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("controls"));
    let autoplay = get_attr("autoplay").is_some()
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("autoplay"));
    let is_loop = get_attr("loop").is_some()
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("loop"));
    let is_muted = get_attr("muted").is_some()
        || get_attr("data-mango-muted") == Some("true")
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("muted"));

    let is_playing = get_attr("data-mango-playing") == Some("true") || (autoplay && get_attr("data-mango-playing") != Some("false"));
    let current_time = get_attr("data-mango-time").and_then(|t| t.parse().ok()).unwrap_or(0.0f32);
    let duration = get_attr("data-mango-duration").and_then(|d| d.parse().ok()).unwrap_or(if !src.is_empty() { 180.0f32 } else { 0.0f32 });

    // 4. Default dimensions (300x150 per HTML5 spec, or poster dimensions)
    let mut intrinsic_width = if poster_width > 0 { poster_width as f32 } else { 300.0f32 };
    let mut intrinsic_height = if poster_height > 0 { poster_height as f32 } else { 150.0f32 };

    if let Some(w_attr) = get_attr("width") {
        if let Ok(w) = w_attr.trim_end_matches("px").parse::<f32>() {
            if w > 0.0 {
                intrinsic_width = w;
            }
        }
    }
    if let Some(h_attr) = get_attr("height") {
        if let Ok(h) = h_attr.trim_end_matches("px").parse::<f32>() {
            if h > 0.0 {
                intrinsic_height = h;
            }
        }
    }

    match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::Video {
        src,
        poster,
        has_controls,
        autoplay,
        is_loop,
        is_muted,
        is_playing,
        current_time,
        duration,
        intrinsic_width,
        intrinsic_height,
        poster_pixels,
        poster_width,
        poster_height,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    // Preserve child nodes (e.g. <track>, fallback text)
    for child in &styled_node.children {
        b.children.push(build_box_tree(child));
    }
    b
}

/// Builds a [`BoxType::Audio`] layout box for `<audio>` elements.
///
/// Implements HTML5 §4.8.10 `<audio>`:
/// - Resolves `src` from element attributes or child `<source>` elements
/// - Extracts controls and playback state
/// - Sized to standard 300×36 bar when controls are enabled, or 0×0 / hidden when without controls
pub fn build_audio_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let mut src = get_attr("src").unwrap_or("").trim().to_string();
    if src.is_empty() {
        for child in &styled_node.children {
            if child.tag_name.as_deref() == Some("source") {
                if let Some((_, s)) = child.attributes.iter().find(|(k, _)| k.eq_ignore_ascii_case("src")) {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        src = trimmed.to_string();
                        break;
                    }
                }
            }
        }
    }

    let has_controls = get_attr("controls").is_some()
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("controls"));
    let autoplay = get_attr("autoplay").is_some()
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("autoplay"));
    let is_loop = get_attr("loop").is_some()
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("loop"));
    let is_muted = get_attr("muted").is_some()
        || get_attr("data-mango-muted") == Some("true")
        || styled_node.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case("muted"));

    let is_playing = get_attr("data-mango-playing") == Some("true") || (autoplay && get_attr("data-mango-playing") != Some("false"));
    let current_time = get_attr("data-mango-time").and_then(|t| t.parse().ok()).unwrap_or(0.0f32);
    let duration = get_attr("data-mango-duration").and_then(|d| d.parse().ok()).unwrap_or(if !src.is_empty() { 210.0f32 } else { 0.0f32 });

    let mut intrinsic_width = if has_controls { 300.0f32 } else { 0.0f32 };
    let mut intrinsic_height = if has_controls { 36.0f32 } else { 0.0f32 };

    match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::Audio {
        src,
        has_controls,
        autoplay,
        is_loop,
        is_muted,
        is_playing,
        current_time,
        duration,
        intrinsic_width,
        intrinsic_height,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    for child in &styled_node.children {
        b.children.push(build_box_tree(child));
    }
    b
}

/// Builds a [`BoxType::Canvas`] layout box for `<canvas>` elements.
///
/// Implements HTML5 §4.12.5 `<canvas>`:
/// - Resolves pixel dimensions from width/height attributes or CSS styles (defaulting to 300×150)
/// - Queries the global canvas store for backing pixel data
/// - Fallback child content is NOT rendered to page layout per specification
pub fn build_canvas_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let mut width = 300u32;
    let mut height = 150u32;

    if let Some(w_attr) = get_attr("width") {
        if let Ok(w) = w_attr.trim_end_matches("px").parse::<u32>() {
            if w > 0 {
                width = w;
            }
        }
    }
    if let Some(h_attr) = get_attr("height") {
        if let Ok(h) = h_attr.trim_end_matches("px").parse::<u32>() {
            if h > 0 {
                height = h;
            }
        }
    }

    let node_idx = styled_node.node_id.map(|id| id.raw() as usize);

    // If canvas is already instantiated in CANVAS_REGISTRY, retrieve its actual buffer & dimensions
    let mut pixels = None;
    if let Some(nid) = node_idx {
        if let Some((cw, ch, c_pixels)) = mango_render::get_canvas_pixels(nid) {
            width = cw;
            height = ch;
            pixels = Some(c_pixels);
        }
    }

    let mut intrinsic_width = width as f32;
    let mut intrinsic_height = height as f32;

    match styled_node.style.width {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        mango_css::values::Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::Canvas {
        node_id: node_idx,
        width,
        height,
        intrinsic_width,
        intrinsic_height,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();
    // Do NOT push children: canvas child nodes are fallback content only!
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anonymous_block_box_generation() {
        let mut parent_style = ComputedStyle::default();
        parent_style.display = Display::Block;

        let mut inline_style = ComputedStyle::default();
        inline_style.display = Display::Inline;

        let mut block_style = ComputedStyle::default();
        block_style.display = Display::Block;

        // Parent block with: [Inline, Block, Text]
        let mut parent = StyledNode::new(None, parent_style);
        let child_inline = StyledNode::new(None, inline_style.clone());
        let child_block = StyledNode::new(None, block_style);
        let child_text = StyledNode::with_text(None, inline_style, "World".to_string());

        parent.children.push(child_inline);
        parent.children.push(child_block);
        parent.children.push(child_text);

        let box_tree = build_box_tree(&parent);
        assert_eq!(box_tree.box_type, BoxType::BlockNode);

        // Children should be:
        // 1. AnonymousBlock(containing child_inline)
        // 2. BlockNode
        // 3. AnonymousBlock(containing child_text)
        assert_eq!(box_tree.children.len(), 3);
        assert!(box_tree.children[0].is_anonymous());
        assert_eq!(box_tree.children[0].children.len(), 1);
        assert!(box_tree.children[0].children[0].is_inline());

        assert!(box_tree.children[1].is_block());
        assert!(!box_tree.children[1].is_anonymous());

        assert!(box_tree.children[2].is_anonymous());
        assert_eq!(box_tree.children[2].children.len(), 1);
        assert!(box_tree.children[2].children[0].is_text());
    }

    #[test]
    fn test_replaced_element_img_generation() {
        let mut style = ComputedStyle::default();
        style.display = Display::Inline;

        let mut img_node = StyledNode::new(None, style);
        img_node.tag_name = Some("img".to_string());
        img_node.attributes.push(("width".to_string(), "120".to_string()));
        img_node.attributes.push(("height".to_string(), "80".to_string()));
        img_node.attributes.push(("src".to_string(), "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string()));

        let box_tree = build_box_tree(&img_node);
        assert!(box_tree.is_replaced());
        if let BoxType::ReplacedElement { intrinsic_width, intrinsic_height, pixels } = &box_tree.box_type {
            assert_eq!(*intrinsic_width, 120.0);
            assert_eq!(*intrinsic_height, 80.0);
            assert_eq!(pixels.len(), 120 * 80);
        } else {
            panic!("Expected ReplacedElement");
        }
    }

    #[test]
    fn test_iframe_box_tree_generation_with_srcdoc() {
        let mut style = ComputedStyle::default();
        style.display = Display::InlineBlock;

        let mut iframe_node = StyledNode::new(None, style);
        iframe_node.tag_name = Some("iframe".to_string());
        iframe_node.attributes.push(("width".to_string(), "450".to_string()));
        iframe_node.attributes.push(("height".to_string(), "250".to_string()));
        iframe_node.attributes.push(("sandbox".to_string(), "allow-scripts allow-forms".to_string()));
        iframe_node.attributes.push(("srcdoc".to_string(), "<p>Nested Document</p>".to_string()));

        let box_tree = build_box_tree(&iframe_node);
        assert!(box_tree.is_replaced());
        assert!(box_tree.is_iframe());

        if let BoxType::IFrame {
            src: _,
            srcdoc,
            sandbox,
            intrinsic_width,
            intrinsic_height,
        } = &box_tree.box_type
        {
            assert_eq!(*intrinsic_width, 450.0);
            assert_eq!(*intrinsic_height, 250.0);
            assert!(sandbox.is_sandboxed);
            assert!(sandbox.allow_scripts);
            assert!(sandbox.allow_forms);
            assert!(!sandbox.allow_same_origin);
            assert_eq!(srcdoc.as_deref(), Some("<p>Nested Document</p>"));
        } else {
            panic!("Expected BoxType::IFrame");
        }

        // Inner document must be parsed and attached as child boxes
        assert!(!box_tree.children.is_empty(), "Inner srcdoc must produce child layout boxes");
    }

    #[test]
    fn test_picture_source_selection() {
        let mut style = ComputedStyle::default();
        style.display = Display::Inline;

        let mut img_node = StyledNode::new(None, style);
        img_node.tag_name = Some("img".to_string());
        // 1x1 red PNG data URI
        let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let sources_attr = format!("{data_url}\t(min-width: 300px)\timage/png");
        img_node.attributes.push(("_mango_picture_sources".to_string(), sources_attr));
        img_node.attributes.push(("src".to_string(), "http://example.com/fallback.jpg".to_string()));

        let box_tree = build_box_tree(&img_node);
        assert!(box_tree.is_replaced());
        if let BoxType::ReplacedElement { intrinsic_width, intrinsic_height, pixels } = &box_tree.box_type {
            assert_eq!(*intrinsic_width, 1.0);
            assert_eq!(*intrinsic_height, 1.0);
            assert_eq!(pixels.len(), 1);
        } else {
            panic!("Expected ReplacedElement from <picture> source");
        }
    }

    #[test]
    fn test_image_map_shapes_and_hit_testing() {
        // Register an image map with rect, circle, poly, and default shapes
        let areas = vec![
            MapArea {
                shape: AreaShape::Rect { left: 10.0, top: 10.0, right: 50.0, bottom: 50.0 },
                href: Some("/rect-target".to_string()),
                alt: Some("Rectangle".to_string()),
                target: None,
            },
            MapArea {
                shape: AreaShape::Circle { cx: 100.0, cy: 100.0, r: 20.0 },
                href: Some("/circle-target".to_string()),
                alt: Some("Circle".to_string()),
                target: None,
            },
            MapArea {
                shape: AreaShape::Poly {
                    points: vec![(150.0, 10.0), (200.0, 10.0), (175.0, 60.0)],
                },
                href: Some("/poly-target".to_string()),
                alt: Some("Triangle".to_string()),
                target: None,
            },
            MapArea {
                shape: AreaShape::Default,
                href: Some("/default-target".to_string()),
                alt: Some("Default".to_string()),
                target: None,
            },
        ];
        register_image_map("navmap", areas);

        let mut img_style = ComputedStyle::default();
        img_style.display = Display::InlineBlock;
        let mut img_node = StyledNode::new(None, img_style);
        img_node.tag_name = Some("img".to_string());
        img_node.attributes.push(("usemap".to_string(), "#navmap".to_string()));
        img_node.attributes.push(("width".to_string(), "300".to_string()));
        img_node.attributes.push(("height".to_string(), "200".to_string()));

        let mut img_box = build_box_tree(&img_node);
        assert_eq!(img_box.map_areas.len(), 4);

        // Position the box at (0, 0, 300, 200)
        img_box.dimensions.content = mango_core::Rect::new(0.0, 0.0, 300.0, 200.0);

        // Hit inside rectangle (25, 25)
        let hit_rect = img_box.hit_test_link(mango_core::Point::new(25.0, 25.0));
        assert_eq!(hit_rect, Some("/rect-target"));

        // Hit inside circle (105, 105)
        let hit_circle = img_box.hit_test_link(mango_core::Point::new(105.0, 105.0));
        assert_eq!(hit_circle, Some("/circle-target"));

        // Hit inside triangle polygon (175, 25)
        let hit_poly = img_box.hit_test_link(mango_core::Point::new(175.0, 25.0));
        assert_eq!(hit_poly, Some("/poly-target"));

        // Hit outside specific shapes falls to default
        let hit_default = img_box.hit_test_link(mango_core::Point::new(280.0, 180.0));
        assert_eq!(hit_default, Some("/default-target"));

        // Hit outside the image bounds returns None
        let hit_outside = img_box.hit_test_link(mango_core::Point::new(400.0, 400.0));
        assert_eq!(hit_outside, None);
    }

    #[test]
    fn test_object_graceful_fallback_and_embed() {
        // 1. Embed element produces replaced element
        let mut embed_node = StyledNode::new(None, ComputedStyle::default());
        embed_node.tag_name = Some("embed".to_string());
        embed_node.attributes.push(("width".to_string(), "200".to_string()));
        embed_node.attributes.push(("height".to_string(), "100".to_string()));
        let embed_box = build_box_tree(&embed_node);
        assert!(embed_box.is_replaced());

        // 2. Object with invalid data gracefully falls back to children
        let mut object_node = StyledNode::new(None, ComputedStyle::default());
        object_node.tag_name = Some("object".to_string());
        object_node.attributes.push(("data".to_string(), "invalid://missing.swf".to_string()));

        let mut child_p = StyledNode::new(None, ComputedStyle::default());
        child_p.tag_name = Some("p".to_string());
        child_p.text = Some("Flash plugin not supported".to_string());
        object_node.children.push(child_p);

        let object_box = build_box_tree(&object_node);
        // Falls through to render children!
        assert!(!object_box.is_replaced());
        assert_eq!(object_box.children.len(), 1);
        assert_eq!(object_box.children[0].text(), Some("Flash plugin not supported"));
    }

    #[test]
    fn test_svg_viewbox_intrinsic_sizing_with_width_or_height() {
        let style = ComputedStyle::default();

        // Case 1: Pure viewBox "0 0 200 100" with no width/height attributes
        let mut svg_node1 = StyledNode::new(None, style.clone());
        svg_node1.tag_name = Some("svg".to_string());
        svg_node1.attributes.push(("viewBox".to_string(), "0 0 200 100".to_string()));

        let box_tree1 = build_box_tree(&svg_node1);
        assert!(box_tree1.is_replaced());
        if let BoxType::ReplacedElement { intrinsic_width, intrinsic_height, .. } = &box_tree1.box_type {
            assert_eq!(*intrinsic_width, 200.0, "Intrinsic width should be taken from viewBox");
            assert_eq!(*intrinsic_height, 100.0, "Intrinsic height should be taken from viewBox");
        } else {
            panic!("Expected ReplacedElement for SVG");
        }

        // Case 2: SVG with viewBox "0 0 200 100" and only width="400" attribute
        // Height should be derived from viewBox aspect ratio: 400 / (200/100) = 200
        let mut svg_node2 = StyledNode::new(None, style.clone());
        svg_node2.tag_name = Some("svg".to_string());
        svg_node2.attributes.push(("viewBox".to_string(), "0 0 200 100".to_string()));
        svg_node2.attributes.push(("width".to_string(), "400".to_string()));

        let box_tree2 = build_box_tree(&svg_node2);
        assert!(box_tree2.is_replaced());
        if let BoxType::ReplacedElement { intrinsic_width, intrinsic_height, .. } = &box_tree2.box_type {
            assert_eq!(*intrinsic_width, 400.0);
            assert_eq!(*intrinsic_height, 200.0, "Height should be derived from viewBox aspect ratio");
        } else {
            panic!("Expected ReplacedElement for SVG");
        }

        // Case 3: SVG with viewBox "0 0 100 200" and only height="100" attribute
        // Width should be derived from viewBox aspect ratio: 100 * (100/200) = 50
        let mut svg_node3 = StyledNode::new(None, style.clone());
        svg_node3.tag_name = Some("svg".to_string());
        svg_node3.attributes.push(("viewBox".to_string(), "0 0 100 200".to_string()));
        svg_node3.attributes.push(("height".to_string(), "100".to_string()));

        let box_tree3 = build_box_tree(&svg_node3);
        assert!(box_tree3.is_replaced());
        if let BoxType::ReplacedElement { intrinsic_width, intrinsic_height, .. } = &box_tree3.box_type {
            assert_eq!(*intrinsic_width, 50.0, "Width should be derived from viewBox aspect ratio");
            assert_eq!(*intrinsic_height, 100.0);
        } else {
            panic!("Expected ReplacedElement for SVG");
        }

        // Case 4: SVG with viewBox "0 0 200 100" placed in block layout with CSS width: 600px, height: auto
        let mut svg_node4 = StyledNode::new(None, style);
        svg_node4.tag_name = Some("svg".to_string());
        svg_node4.attributes.push(("viewBox".to_string(), "0 0 200 100".to_string()));
        let mut box_tree4 = build_box_tree(&svg_node4);
        let mut css_style = ComputedStyle::default();
        css_style.width = mango_css::Length::Px(600.0);
        css_style.height = mango_css::Length::Auto;
        box_tree4.style = Some(css_style);

        let cb = Dimensions::new(mango_core::Rect::new(0.0, 0.0, 1000.0, 800.0));
        let mut float_ctx = crate::FloatContext::new();
        crate::block_flow::layout_block(&mut box_tree4, &cb, &mut float_ctx);

        assert_eq!(box_tree4.dimensions.content.width(), 600.0);
        assert_eq!(box_tree4.dimensions.content.height(), 300.0, "Auto height in block layout should preserve viewBox 2:1 ratio");
    }
}


