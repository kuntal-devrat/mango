use std::sync::Arc;

use mango_css::computed::ComputedStyle;
use mango_css::values::WritingMode;
use mango_html::dom::NodeId;

use crate::box_model::BoxType;
use crate::dimensions::Dimensions;

/// A node in the layout tree representing a visual box.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutBox {
    /// The specific type of layout box (Block, Inline, AnonymousBlock, TextNode).
    pub box_type: BoxType,
    /// Box geometry (content, padding, border, margin).
    pub dimensions: Dimensions,
    /// Computed CSS style for this box, if backed by an element or anonymous block.
    pub style: Option<Arc<ComputedStyle>>,
    /// Child boxes.
    pub children: Vec<LayoutBox>,
    /// Hyperlink URL target if this box or an ancestor is an `<a>` element.
    pub link_target: Option<String>,
    /// Target browsing context or frame (e.g. "_blank", "_parent", "_top", or frame name).
    pub link_frame: Option<String>,
    /// DOM node identifier, if backed by a DOM element.
    pub node_id: Option<NodeId>,
    /// Element tag name (e.g. "div", "input", "table"), if backed by an element.
    pub tag_name: Option<String>,
    /// Element HTML attributes, if backed by an element.
    pub attributes: Vec<(String, String)>,
    /// Image map active hit-testing areas if this box is an <img> or <object> with usemap.
    pub map_areas: Vec<MapArea>,
    /// Dirty flag for display list invalidation.
    pub is_dirty: bool,
    /// Unflattened original child boxes preserved for idempotent relayout.
    pub raw_children: Option<Vec<LayoutBox>>,
    /// Horizontal scroll offset for scrollable containers.
    pub scroll_offset_x: f32,
    /// Vertical scroll offset for scrollable containers.
    pub scroll_offset_y: f32,
    /// Whether this box establishes an independent formatting context (e.g. flex or grid item).
    pub is_independent_formatting_context: bool,
    /// Cached result of "does this subtree contain a positioned box?".
    ///
    /// Paint-layer assignment asks this for every child; recomputing it
    /// recursively per child is quadratic on deep documents. The cache is
    /// filled lazily and is valid for one display-list build (the box tree is
    /// rebuilt or explicitly dirtied whenever structure changes).
    pub positioned_descendant_cache: std::sync::OnceLock<bool>,
    /// Explicitly overridden containing block (e.g. for absolute elements inside fragmented inline elements).
    pub custom_containing_block: Option<Dimensions>,
    /// Containing block for fixed elements when an ancestor traps fixed positioning (transform, will-change, etc.).
    pub fixed_containing_block: Option<Dimensions>,
    /// DOM node identifier of containing inline element, if this box is an out-of-flow child of an inline.
    pub containing_inline_id: Option<NodeId>,
    /// Computed style of containing positioned inline ancestor, used to resolve static position relative offsets.
    pub positioned_inline_style: Option<Arc<ComputedStyle>>,
    /// Original hypothetical in-flow static position before positioning adjustments.
    pub static_position: Option<mango_core::Point>,
    /// Whether the static position represents the bottom edge of the hypothetical box (e.g. in RTL vertical flow).
    pub is_static_bottom_anchored: bool,
    /// Writing mode of the containing block, used to resolve alignment axes.
    pub containing_block_writing_mode: Option<WritingMode>,
}

/// Supported geometric shapes for `<area>` elements in HTML image maps.
#[derive(Debug, Clone, PartialEq)]
pub enum AreaShape {
    Default,
    Rect {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Poly {
        points: Vec<(f32, f32)>,
    },
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
                let (chunks, _) = coords.as_chunks::<2>();
                for chunk in chunks {
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
                    AreaShape::Rect {
                        left,
                        top,
                        right,
                        bottom,
                    }
                } else {
                    AreaShape::Default
                }
            }
        }
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        match self {
            AreaShape::Default => true,
            AreaShape::Rect {
                left,
                top,
                right,
                bottom,
            } => x >= *left && x <= *right && y >= *top && y <= *bottom,
            AreaShape::Circle { cx, cy, r } => {
                if *r <= 0.0 {
                    return false;
                }
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
                    let intersect =
                        ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
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

thread_local! {
    static IMAGE_MAP_REGISTRY: std::cell::RefCell<std::collections::HashMap<String, Vec<MapArea>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn register_image_map(name: &str, areas: Vec<MapArea>) {
    IMAGE_MAP_REGISTRY.with(|reg| {
        reg.borrow_mut().insert(name.to_ascii_lowercase(), areas);
    });
}

pub fn get_image_map(name: &str) -> Option<Vec<MapArea>> {
    let clean_name = name.trim().trim_start_matches('#').to_ascii_lowercase();
    IMAGE_MAP_REGISTRY.with(|reg| reg.borrow().get(&clean_name).cloned())
}

pub fn clear_image_maps() {
    IMAGE_MAP_REGISTRY.with(|reg| {
        reg.borrow_mut().clear();
    });
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

/// Robust, whitespace-tolerant HTML/SVG dimension attribute parser.
/// Parses pixel strings like `"300"`, `"300px"`, `"  300 PX "` into non-negative floats.
#[inline]
pub(crate) fn parse_dimension_attr(val: &str) -> Option<f32> {
    let s = val.trim();
    let s = if s.to_ascii_lowercase().ends_with("px") {
        s[..s.len() - 2].trim_end()
    } else {
        s
    };
    s.parse::<f32>().ok().filter(|&v| v >= 0.0)
}
