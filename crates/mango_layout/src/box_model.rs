//! CSS Box Model types: box categories, dimensions, margin collapsing,
//! iframe sandboxing, flow-relative logical geometries, and replaced sizing.
//!
//! Conforms to:
//! - CSS 2.1 Chapter 8 (Box model, margin collapsing, padding, border)
//! - CSS 2.1 §10.3.2 & §10.6.2 (Replaced element sizing)
//! - CSS Box Model Module Level 3
//! - CSS Sizing Module Level 3
//! - CSS Logical Properties and Values Level 1
//! - CSS Overflow Module Level 3
//! - WHATWG HTML Living Standard §4.8.5.1 (`iframe` sandbox attributes)

use mango_core::{EdgeSizes, Rect, Size};
use mango_css::values::{BoxSizing, ObjectFit};

pub use crate::dimensions::Dimensions;

/// The structural category of a layout box.
#[derive(Debug, Clone, PartialEq)]
pub enum BoxType {
    /// A block container box (e.g. `<div>`, `<p>`, `<h1>`).
    BlockNode,
    /// An inline box (e.g. `<span>`, `<a>`, `<b>`).
    InlineNode,
    /// An inline-block box that formats contents as a block but flows inline.
    InlineBlock,
    /// An anonymous block box generated to enclose inline children inside a block container.
    AnonymousBlock,
    /// A text run inside an inline context.
    TextNode(String),
    /// A replaced element (e.g. `<img>`) with intrinsic dimensions and decoded pixel data.
    ReplacedElement {
        intrinsic_width: f32,
        intrinsic_height: f32,
        /// Pixel data in `0xRRGGBB` format, row-major.
        pixels: Vec<u32>,
    },
    /// An embedded `<iframe>` element representing a nested browsing context.
    IFrame {
        src: String,
        srcdoc: Option<String>,
        sandbox: IFrameSandbox,
        intrinsic_width: f32,
        intrinsic_height: f32,
    },
    /// A replaced `<video>` element with poster, playback state, and native controls.
    Video {
        src: String,
        poster: Option<String>,
        has_controls: bool,
        autoplay: bool,
        is_loop: bool,
        is_muted: bool,
        is_playing: bool,
        current_time: f32,
        duration: f32,
        intrinsic_width: f32,
        intrinsic_height: f32,
        poster_pixels: Option<Vec<u32>>,
        poster_width: u32,
        poster_height: u32,
    },
    /// A replaced `<audio>` element with sources, playback state, and native audio bar controls.
    Audio {
        src: String,
        has_controls: bool,
        autoplay: bool,
        is_loop: bool,
        is_muted: bool,
        is_playing: bool,
        current_time: f32,
        duration: f32,
        intrinsic_width: f32,
        intrinsic_height: f32,
    },
    /// A replaced `<canvas>` element with backing pixel buffer and intrinsic dimensions.
    Canvas {
        node_id: Option<usize>,
        width: u32,
        height: u32,
        intrinsic_width: f32,
        intrinsic_height: f32,
        pixels: Option<Vec<u32>>,
    },
}

impl BoxType {
    /// Returns `true` if this box participates in a block formatting context as a block box.
    #[inline]
    pub fn is_block(&self) -> bool {
        matches!(self, BoxType::BlockNode | BoxType::AnonymousBlock)
    }

    /// Returns `true` if this box is an inline-level box or text node.
    #[inline]
    pub fn is_inline(&self) -> bool {
        matches!(
            self,
            BoxType::InlineNode
                | BoxType::InlineBlock
                | BoxType::TextNode(_)
                | BoxType::ReplacedElement { .. }
                | BoxType::IFrame { .. }
                | BoxType::Video { .. }
                | BoxType::Audio { .. }
                | BoxType::Canvas { .. }
        )
    }

    /// Returns `true` if this box is an inline-block box.
    #[inline]
    pub fn is_inline_block(&self) -> bool {
        matches!(self, BoxType::InlineBlock)
    }

    /// Returns `true` if this box is an atomic inline-level box (CSS 2.1 §9.2.2 / CSS Display 3).
    ///
    /// Atomic inlines participate in an inline formatting context as a single unbreakable
    /// rectangular unit; margins, padding, and borders apply on all four sides.
    #[inline]
    pub fn is_atomic_inline(&self) -> bool {
        matches!(
            self,
            BoxType::InlineBlock
                | BoxType::ReplacedElement { .. }
                | BoxType::IFrame { .. }
                | BoxType::Video { .. }
                | BoxType::Audio { .. }
                | BoxType::Canvas { .. }
        )
    }

    /// Returns `true` if this box is an anonymous block box.
    #[inline]
    pub fn is_anonymous(&self) -> bool {
        matches!(self, BoxType::AnonymousBlock)
    }

    /// Returns `true` if this box is a text node.
    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self, BoxType::TextNode(_))
    }

    /// Returns `true` if this box is a replaced element (e.g. `<img>`, `<iframe>`, `<video>`, `<audio>`, `<canvas>`).
    #[inline]
    pub fn is_replaced(&self) -> bool {
        matches!(
            self,
            BoxType::ReplacedElement { .. }
                | BoxType::IFrame { .. }
                | BoxType::Video { .. }
                | BoxType::Audio { .. }
                | BoxType::Canvas { .. }
        )
    }

    /// Returns `true` if this box is an `<iframe>` element.
    #[inline]
    pub fn is_iframe(&self) -> bool {
        matches!(self, BoxType::IFrame { .. })
    }

    /// Returns `true` if this box is a `<video>` element.
    #[inline]
    pub fn is_video(&self) -> bool {
        matches!(self, BoxType::Video { .. })
    }

    /// Returns `true` if this box is an `<audio>` element.
    #[inline]
    pub fn is_audio(&self) -> bool {
        matches!(self, BoxType::Audio { .. })
    }

    /// Returns `true` if this box is a `<canvas>` element.
    #[inline]
    pub fn is_canvas(&self) -> bool {
        matches!(self, BoxType::Canvas { .. })
    }

    /// Returns `true` if this box represents an embedded media or graphics surface (video, audio, canvas).
    #[inline]
    pub fn is_media(&self) -> bool {
        matches!(
            self,
            BoxType::Video { .. } | BoxType::Audio { .. } | BoxType::Canvas { .. }
        )
    }

    /// Returns `true` if this box can contain other visual child boxes.
    #[inline]
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            BoxType::BlockNode
                | BoxType::InlineNode
                | BoxType::InlineBlock
                | BoxType::AnonymousBlock
        )
    }

    /// Extracts intrinsic dimensions `(width, height)` for replaced elements, if defined.
    #[inline]
    pub fn intrinsic_size(&self) -> Option<(f32, f32)> {
        match self {
            BoxType::ReplacedElement {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::IFrame {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Video {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Audio {
                intrinsic_width,
                intrinsic_height,
                ..
            }
            | BoxType::Canvas {
                intrinsic_width,
                intrinsic_height,
                ..
            } => {
                if *intrinsic_width > 0.0 || *intrinsic_height > 0.0 {
                    Some((*intrinsic_width, *intrinsic_height))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Calculates intrinsic aspect ratio `(width / height)` for replaced elements, if valid.
    #[inline]
    pub fn intrinsic_aspect_ratio(&self) -> Option<f32> {
        let (w, h) = self.intrinsic_size()?;
        if w > 0.0 && h > 0.0 {
            Some(w / h)
        } else {
            None
        }
    }

    /// Returns string text content if this is a [`BoxType::TextNode`].
    #[inline]
    pub fn text_content(&self) -> Option<&str> {
        match self {
            BoxType::TextNode(text) => Some(text.as_str()),
            _ => None,
        }
    }

    /// Returns a mutable reference to string text content if this is a [`BoxType::TextNode`].
    #[inline]
    pub fn text_content_mut(&mut self) -> Option<&mut String> {
        match self {
            BoxType::TextNode(text) => Some(text),
            _ => None,
        }
    }
}

/// Security and capability restrictions applied to an `<iframe>` nested browsing context.
///
/// Implements standard WHATWG HTML5 sandboxing flags:
/// - `allow-scripts`
/// - `allow-same-origin`
/// - `allow-forms`
/// - `allow-top-navigation`
/// - `allow-top-navigation-by-user-activation`
/// - `allow-popups`
/// - `allow-popups-to-escape-sandbox`
/// - `allow-modals`
/// - `allow-downloads`
/// - `allow-pointer-lock`
/// - `allow-orientation-lock`
/// - `allow-presentation`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IFrameSandbox {
    pub is_sandboxed: bool,
    pub allow_scripts: bool,
    pub allow_same_origin: bool,
    pub allow_forms: bool,
    pub allow_top_navigation: bool,
    pub allow_top_navigation_by_user_activation: bool,
    pub allow_popups: bool,
    pub allow_popups_to_escape_sandbox: bool,
    pub allow_modals: bool,
    pub allow_downloads: bool,
    pub allow_pointer_lock: bool,
    pub allow_orientation_lock: bool,
    pub allow_presentation: bool,
}

impl Default for IFrameSandbox {
    fn default() -> Self {
        Self {
            is_sandboxed: false,
            allow_scripts: true,
            allow_same_origin: true,
            allow_forms: true,
            allow_top_navigation: true,
            allow_top_navigation_by_user_activation: true,
            allow_popups: true,
            allow_popups_to_escape_sandbox: true,
            allow_modals: true,
            allow_downloads: true,
            allow_pointer_lock: true,
            allow_orientation_lock: true,
            allow_presentation: true,
        }
    }
}

impl IFrameSandbox {
    /// Creates an empty sandbox configuration where all capabilities are restricted.
    pub fn locked() -> Self {
        Self {
            is_sandboxed: true,
            allow_scripts: false,
            allow_same_origin: false,
            allow_forms: false,
            allow_top_navigation: false,
            allow_top_navigation_by_user_activation: false,
            allow_popups: false,
            allow_popups_to_escape_sandbox: false,
            allow_modals: false,
            allow_downloads: false,
            allow_pointer_lock: false,
            allow_orientation_lock: false,
            allow_presentation: false,
        }
    }

    /// Parses a standard HTML5 `sandbox` attribute value.
    ///
    /// If `sandbox_attr` is `None`, sandboxing is inactive.
    /// If `sandbox_attr` is `Some("")`, all restrictions are enforced.
    /// Otherwise, space-separated tokens lift specific restrictions.
    ///
    /// Optimized for zero heap allocations during token iteration.
    pub fn from_attribute(sandbox_attr: Option<&str>) -> Self {
        match sandbox_attr {
            None => Self::default(),
            Some(tokens_str) => {
                let mut sandbox = Self::locked();
                for token in tokens_str.split_whitespace() {
                    if token.eq_ignore_ascii_case("allow-scripts") {
                        sandbox.allow_scripts = true;
                    } else if token.eq_ignore_ascii_case("allow-same-origin") {
                        sandbox.allow_same_origin = true;
                    } else if token.eq_ignore_ascii_case("allow-forms") {
                        sandbox.allow_forms = true;
                    } else if token.eq_ignore_ascii_case("allow-top-navigation") {
                        sandbox.allow_top_navigation = true;
                    } else if token.eq_ignore_ascii_case("allow-top-navigation-by-user-activation")
                    {
                        sandbox.allow_top_navigation_by_user_activation = true;
                    } else if token.eq_ignore_ascii_case("allow-popups") {
                        sandbox.allow_popups = true;
                    } else if token.eq_ignore_ascii_case("allow-popups-to-escape-sandbox") {
                        sandbox.allow_popups_to_escape_sandbox = true;
                    } else if token.eq_ignore_ascii_case("allow-modals") {
                        sandbox.allow_modals = true;
                    } else if token.eq_ignore_ascii_case("allow-downloads") {
                        sandbox.allow_downloads = true;
                    } else if token.eq_ignore_ascii_case("allow-pointer-lock") {
                        sandbox.allow_pointer_lock = true;
                    } else if token.eq_ignore_ascii_case("allow-orientation-lock") {
                        sandbox.allow_orientation_lock = true;
                    } else if token.eq_ignore_ascii_case("allow-presentation") {
                        sandbox.allow_presentation = true;
                    }
                }
                sandbox
            }
        }
    }

    /// Serializes current active sandbox permissions into a standard space-separated attribute string.
    pub fn to_attribute_value(&self) -> String {
        if !self.is_sandboxed {
            return String::new();
        }
        let mut tokens = Vec::new();
        if self.allow_scripts {
            tokens.push("allow-scripts");
        }
        if self.allow_same_origin {
            tokens.push("allow-same-origin");
        }
        if self.allow_forms {
            tokens.push("allow-forms");
        }
        if self.allow_top_navigation {
            tokens.push("allow-top-navigation");
        }
        if self.allow_top_navigation_by_user_activation {
            tokens.push("allow-top-navigation-by-user-activation");
        }
        if self.allow_popups {
            tokens.push("allow-popups");
        }
        if self.allow_popups_to_escape_sandbox {
            tokens.push("allow-popups-to-escape-sandbox");
        }
        if self.allow_modals {
            tokens.push("allow-modals");
        }
        if self.allow_downloads {
            tokens.push("allow-downloads");
        }
        if self.allow_pointer_lock {
            tokens.push("allow-pointer-lock");
        }
        if self.allow_orientation_lock {
            tokens.push("allow-orientation-lock");
        }
        if self.allow_presentation {
            tokens.push("allow-presentation");
        }
        tokens.join(" ")
    }
}

/// Concentric visual areas of the CSS box model (CSS Box Model Module Level 3 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoxArea {
    /// The innermost content area, where text, child elements, or media render.
    Content,
    /// The area bounded by the padding edge.
    Padding,
    /// The area bounded by the border edge.
    Border,
    /// The outermost area bounded by the margin edge.
    Margin,
}

/// Tracks positive and negative margins during vertical margin collapsing
/// conforming to CSS 2.1 §8.3.1 and Blink's `MarginStrut`.
///
/// Under CSS 2.1 §8.3.1:
/// "When two or more margins collapse, the resulting margin width is the maximum
/// of the adjoining positive margins (or zero if there are none), minus the
/// maximum of the absolute values of the adjoining negative margins (or zero if there are none)."
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MarginStrut {
    /// Maximum positive adjoining margin (or 0.0 if none).
    pub positive: f32,
    /// Minimum (most negative) adjoining negative margin (or 0.0 if none).
    pub negative: f32,
    /// Whether any non-zero margins have been contributed to this strut.
    pub has_margins: bool,
}

impl MarginStrut {
    /// Creates a new empty margin strut.
    #[inline]
    pub const fn new() -> Self {
        Self {
            positive: 0.0,
            negative: 0.0,
            has_margins: false,
        }
    }

    /// Appends a single margin value to this strut.
    #[inline]
    pub fn append(&mut self, margin: f32) {
        if margin.is_nan() {
            return;
        }
        self.has_margins = true;
        if margin >= 0.0 {
            self.positive = self.positive.max(margin);
        } else {
            self.negative = self.negative.min(margin);
        }
    }

    /// Appends another margin strut, combining positive and negative bounds.
    #[inline]
    pub fn append_strut(&mut self, other: MarginStrut) {
        if !other.has_margins {
            return;
        }
        self.has_margins = true;
        self.positive = self.positive.max(other.positive);
        self.negative = self.negative.min(other.negative);
    }

    /// Solves the collapsed margin per CSS 2.1 §8.3.1: `max_pos - abs(max_neg)`.
    #[inline]
    pub fn solve(&self) -> f32 {
        if !self.has_margins {
            0.0
        } else {
            self.positive + self.negative
        }
    }

    /// Clears the accumulated margin state.
    #[inline]
    pub fn reset(&mut self) {
        self.positive = 0.0;
        self.negative = 0.0;
        self.has_margins = false;
    }
}

/// Writing mode orientations (CSS Writing Modes Level 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// Text and inline flow direction (CSS 2.1 §9.3.2 / CSS Writing Modes 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// Flow-relative logical edge sizes conforming to CSS Logical Properties and Values Level 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalEdgeSizes {
    pub inline_start: f32,
    pub inline_end: f32,
    pub block_start: f32,
    pub block_end: f32,
}

impl LogicalEdgeSizes {
    pub const ZERO: Self = Self {
        inline_start: 0.0,
        inline_end: 0.0,
        block_start: 0.0,
        block_end: 0.0,
    };

    pub fn new(inline_start: f32, inline_end: f32, block_start: f32, block_end: f32) -> Self {
        Self {
            inline_start,
            inline_end,
            block_start,
            block_end,
        }
    }

    /// Converts physical [`EdgeSizes`] to flow-relative [`LogicalEdgeSizes`].
    pub fn from_physical(physical: &EdgeSizes, mode: WritingMode, dir: Direction) -> Self {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => Self {
                    inline_start: physical.left,
                    inline_end: physical.right,
                    block_start: physical.top,
                    block_end: physical.bottom,
                },
                Direction::Rtl => Self {
                    inline_start: physical.right,
                    inline_end: physical.left,
                    block_start: physical.top,
                    block_end: physical.bottom,
                },
            },
            WritingMode::VerticalRl => match dir {
                Direction::Ltr => Self {
                    inline_start: physical.top,
                    inline_end: physical.bottom,
                    block_start: physical.right,
                    block_end: physical.left,
                },
                Direction::Rtl => Self {
                    inline_start: physical.bottom,
                    inline_end: physical.top,
                    block_start: physical.right,
                    block_end: physical.left,
                },
            },
            WritingMode::VerticalLr => match dir {
                Direction::Ltr => Self {
                    inline_start: physical.top,
                    inline_end: physical.bottom,
                    block_start: physical.left,
                    block_end: physical.right,
                },
                Direction::Rtl => Self {
                    inline_start: physical.bottom,
                    inline_end: physical.top,
                    block_start: physical.left,
                    block_end: physical.right,
                },
            },
        }
    }

    /// Converts flow-relative [`LogicalEdgeSizes`] to physical [`EdgeSizes`].
    pub fn to_physical(&self, mode: WritingMode, dir: Direction) -> EdgeSizes {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => EdgeSizes::new(
                    self.block_start,
                    self.inline_end,
                    self.block_end,
                    self.inline_start,
                ),
                Direction::Rtl => EdgeSizes::new(
                    self.block_start,
                    self.inline_start,
                    self.block_end,
                    self.inline_end,
                ),
            },
            WritingMode::VerticalRl => match dir {
                Direction::Ltr => EdgeSizes::new(
                    self.inline_start,
                    self.block_start,
                    self.inline_end,
                    self.block_end,
                ),
                Direction::Rtl => EdgeSizes::new(
                    self.inline_end,
                    self.block_start,
                    self.inline_start,
                    self.block_end,
                ),
            },
            WritingMode::VerticalLr => match dir {
                Direction::Ltr => EdgeSizes::new(
                    self.inline_start,
                    self.block_end,
                    self.inline_end,
                    self.block_start,
                ),
                Direction::Rtl => EdgeSizes::new(
                    self.inline_end,
                    self.block_end,
                    self.inline_start,
                    self.block_start,
                ),
            },
        }
    }

    #[inline]
    pub fn inline_sum(&self) -> f32 {
        self.inline_start + self.inline_end
    }

    #[inline]
    pub fn block_sum(&self) -> f32 {
        self.block_start + self.block_end
    }
}

/// Flow-relative rectangle in writing-mode coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalRect {
    pub inline_offset: f32,
    pub block_offset: f32,
    pub inline_size: f32,
    pub block_size: f32,
}

impl LogicalRect {
    pub fn new(
        inline_offset: f32,
        block_offset: f32,
        inline_size: f32,
        block_size: f32,
    ) -> Self {
        Self {
            inline_offset,
            block_offset,
            inline_size,
            block_size,
        }
    }

    /// Converts physical [`Rect`] within a container of given dimensions to [`LogicalRect`].
    pub fn from_physical(
        rect: &Rect,
        container_w: f32,
        _container_h: f32,
        mode: WritingMode,
        dir: Direction,
    ) -> Self {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => Self {
                    inline_offset: rect.x(),
                    block_offset: rect.y(),
                    inline_size: rect.width(),
                    block_size: rect.height(),
                },
                Direction::Rtl => Self {
                    inline_offset: container_w - rect.right(),
                    block_offset: rect.y(),
                    inline_size: rect.width(),
                    block_size: rect.height(),
                },
            },
            WritingMode::VerticalRl => match dir {
                Direction::Ltr => Self {
                    inline_offset: rect.y(),
                    block_offset: container_w - rect.right(),
                    inline_size: rect.height(),
                    block_size: rect.width(),
                },
                Direction::Rtl => Self {
                    inline_offset: rect.y(),
                    block_offset: container_w - rect.right(),
                    inline_size: rect.height(),
                    block_size: rect.width(),
                },
            },
            WritingMode::VerticalLr => Self {
                inline_offset: rect.y(),
                block_offset: rect.x(),
                inline_size: rect.height(),
                block_size: rect.width(),
            },
        }
    }

    /// Converts [`LogicalRect`] back into physical [`Rect`].
    pub fn to_physical(
        &self,
        container_w: f32,
        _container_h: f32,
        mode: WritingMode,
        dir: Direction,
    ) -> Rect {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => Rect::new(
                    self.inline_offset,
                    self.block_offset,
                    self.inline_size,
                    self.block_size,
                ),
                Direction::Rtl => Rect::new(
                    container_w - self.inline_offset - self.inline_size,
                    self.block_offset,
                    self.inline_size,
                    self.block_size,
                ),
            },
            WritingMode::VerticalRl => Rect::new(
                container_w - self.block_offset - self.block_size,
                self.inline_offset,
                self.block_size,
                self.inline_size,
            ),
            WritingMode::VerticalLr => Rect::new(
                self.block_offset,
                self.inline_offset,
                self.block_size,
                self.inline_size,
            ),
        }
    }
}

/// Represents scrollable and visual (ink) overflow bounds conforming to CSS Overflow 3
/// and Blink's `LayoutBox::ScrollableOverflowRect` / `VisualOverflowRect`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OverflowModel {
    /// Bounding rectangle for scrollable content, used to compute scroll extents.
    pub scrollable_overflow: Rect,
    /// Bounding rectangle for ink overflow, including box-shadows, outlines, and border decorations.
    pub visual_overflow: Rect,
}

impl OverflowModel {
    /// Creates an overflow model initialized to the box's border rectangle.
    pub fn new(border_box: Rect) -> Self {
        Self {
            scrollable_overflow: border_box,
            visual_overflow: border_box,
        }
    }

    /// Unites a child's scrollable overflow rect into this box's scrollable overflow.
    pub fn unite_scrollable(&mut self, child_rect: Rect) {
        if child_rect.width() <= 0.0 || child_rect.height() <= 0.0 {
            return;
        }
        let min_x = self.scrollable_overflow.x().min(child_rect.x());
        let min_y = self.scrollable_overflow.y().min(child_rect.y());
        let max_x = self.scrollable_overflow.right().max(child_rect.right());
        let max_y = self.scrollable_overflow.bottom().max(child_rect.bottom());
        self.scrollable_overflow = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
    }

    /// Unites a visual decoration (outline, shadow) into this box's visual overflow.
    pub fn unite_visual(&mut self, rect: Rect) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let min_x = self.visual_overflow.x().min(rect.x());
        let min_y = self.visual_overflow.y().min(rect.y());
        let max_x = self.visual_overflow.right().max(rect.right());
        let max_y = self.visual_overflow.bottom().max(rect.bottom());
        self.visual_overflow = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
    }

    /// Returns `true` if children overflow beyond the border box in either dimension.
    pub fn has_scrollable_overflow(&self, border_box: Rect) -> bool {
        self.scrollable_overflow.right() > border_box.right() + 0.5
            || self.scrollable_overflow.bottom() > border_box.bottom() + 0.5
            || self.scrollable_overflow.x() < border_box.x() - 0.5
            || self.scrollable_overflow.y() < border_box.y() - 0.5
    }

    /// Returns the scrollable extent `(max_x - right, max_y - bottom)` extending beyond the border box.
    pub fn scrollable_extent(&self, border_box: Rect) -> (f32, f32) {
        let ext_x = (self.scrollable_overflow.right() - border_box.right()).max(0.0);
        let ext_y = (self.scrollable_overflow.bottom() - border_box.bottom()).max(0.0);
        (ext_x, ext_y)
    }
}

/// Resolves the used width and height of a replaced element conforming to
/// CSS 2.1 §10.3.2, §10.6.2 and CSS Sizing Module Level 3 §5.2.
pub fn resolve_replaced_size(
    specified_width: Option<f32>,
    specified_height: Option<f32>,
    intrinsic_width: Option<f32>,
    intrinsic_height: Option<f32>,
    aspect_ratio: Option<f32>,
    default_fallback: (f32, f32),
) -> (f32, f32) {
    let ratio = aspect_ratio
        .or_else(|| match (intrinsic_width, intrinsic_height) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(w / h),
            _ => None,
        });

    match (specified_width, specified_height) {
        // 1. Both dimensions specified explicitly.
        (Some(w), Some(h)) => (w.max(0.0), h.max(0.0)),

        // 2. Width specified, height auto.
        (Some(w), None) => {
            let used_w = w.max(0.0);
            let used_h = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                (used_w / r).max(0.0)
            } else if let Some(ih) = intrinsic_height {
                ih.max(0.0)
            } else {
                default_fallback.1
            };
            (used_w, used_h)
        }

        // 3. Height specified, width auto.
        (None, Some(h)) => {
            let used_h = h.max(0.0);
            let used_w = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                (used_h * r).max(0.0)
            } else if let Some(iw) = intrinsic_width {
                iw.max(0.0)
            } else {
                default_fallback.0
            };
            (used_w, used_h)
        }

        // 4. Both dimensions auto.
        (None, None) => match (intrinsic_width, intrinsic_height) {
            (Some(iw), Some(ih)) => (iw.max(0.0), ih.max(0.0)),
            (Some(iw), None) => {
                let used_w = iw.max(0.0);
                let used_h = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                    (used_w / r).max(0.0)
                } else {
                    default_fallback.1
                };
                (used_w, used_h)
            }
            (None, Some(ih)) => {
                let used_h = ih.max(0.0);
                let used_w = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                    (used_h * r).max(0.0)
                } else {
                    default_fallback.0
                };
                (used_w, used_h)
            }
            (None, None) => {
                if let Some(r) = ratio.filter(|&r| r > 0.0) {
                    let (dw, dh) = default_fallback;
                    if dw / dh > r {
                        ((dh * r).max(0.0), dh)
                    } else {
                        (dw, (dw / r).max(0.0))
                    }
                } else {
                    default_fallback
                }
            }
        },
    }
}

/// Computes the destination rectangle of replaced content within its content box
/// according to `object-fit` and `object-position` (CSS Images Module Level 3 §5.5).
pub fn resolve_object_fit_rect(
    content_box: Rect,
    natural_size: (f32, f32),
    object_fit: ObjectFit,
    object_position: (f32, f32),
) -> Rect {
    let (box_w, box_h) = (content_box.width(), content_box.height());
    let (nat_w, nat_h) = natural_size;

    if box_w <= 0.0 || box_h <= 0.0 || nat_w <= 0.0 || nat_h <= 0.0 {
        return content_box;
    }

    let (draw_w, draw_h) = match object_fit {
        ObjectFit::Fill => (box_w, box_h),
        ObjectFit::Contain => {
            let scale = (box_w / nat_w).min(box_h / nat_h);
            (nat_w * scale, nat_h * scale)
        }
        ObjectFit::Cover => {
            let scale = (box_w / nat_w).max(box_h / nat_h);
            (nat_w * scale, nat_h * scale)
        }
        ObjectFit::None => (nat_w, nat_h),
        ObjectFit::ScaleDown => {
            let scale_contain = (box_w / nat_w).min(box_h / nat_h);
            if scale_contain < 1.0 {
                (nat_w * scale_contain, nat_h * scale_contain)
            } else {
                (nat_w, nat_h)
            }
        }
    };

    // Calculate position: object_position is (percent_x, percent_y), defaulting to (0.5, 0.5)
    let offset_x = content_box.x() + (box_w - draw_w) * object_position.0;
    let offset_y = content_box.y() + (box_h - draw_h) * object_position.1;

    Rect::new(offset_x, offset_y, draw_w, draw_h)
}

/// Converts a specified size to content-box size according to `box-sizing` (CSS Box Sizing 3).
#[inline]
pub fn adjust_for_box_sizing(
    size: Size,
    box_sizing: BoxSizing,
    padding: &EdgeSizes,
    border: &EdgeSizes,
) -> Size {
    match box_sizing {
        BoxSizing::ContentBox => size,
        BoxSizing::BorderBox => Size::new(
            (size.width - padding.horizontal() - border.horizontal()).max(0.0),
            (size.height - padding.vertical() - border.vertical()).max(0.0),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_type_predicates() {
        assert!(BoxType::BlockNode.is_block());
        assert!(!BoxType::BlockNode.is_inline());
        assert!(BoxType::BlockNode.is_container());

        assert!(BoxType::AnonymousBlock.is_block());
        assert!(BoxType::AnonymousBlock.is_anonymous());

        assert!(BoxType::InlineNode.is_inline());
        assert!(!BoxType::InlineNode.is_block());
        assert!(!BoxType::InlineNode.is_atomic_inline());

        let inline_block = BoxType::InlineBlock;
        assert!(inline_block.is_inline());
        assert!(inline_block.is_inline_block());
        assert!(inline_block.is_atomic_inline());

        let mut text = BoxType::TextNode("Hello CSS".to_string());
        assert!(text.is_inline());
        assert!(text.is_text());
        assert_eq!(text.text_content(), Some("Hello CSS"));
        if let Some(t) = text.text_content_mut() {
            t.push_str(" World");
        }
        assert_eq!(text.text_content(), Some("Hello CSS World"));

        let replaced = BoxType::ReplacedElement {
            intrinsic_width: 100.0,
            intrinsic_height: 50.0,
            pixels: vec![],
        };
        assert!(!replaced.is_block());
        assert!(replaced.is_replaced());
        assert!(replaced.is_inline());
        assert!(replaced.is_atomic_inline());
        assert_eq!(replaced.intrinsic_size(), Some((100.0, 50.0)));
        assert_eq!(replaced.intrinsic_aspect_ratio(), Some(2.0));

        let iframe = BoxType::IFrame {
            src: "https://example.com".to_string(),
            srcdoc: None,
            sandbox: IFrameSandbox::default(),
            intrinsic_width: 300.0,
            intrinsic_height: 150.0,
        };
        assert!(!iframe.is_block());
        assert!(iframe.is_replaced());
        assert!(iframe.is_inline());
        assert!(iframe.is_iframe());
        assert!(iframe.is_atomic_inline());
        assert_eq!(iframe.intrinsic_size(), Some((300.0, 150.0)));
        assert_eq!(iframe.intrinsic_aspect_ratio(), Some(2.0));

        let video = BoxType::Video {
            src: "https://example.com/movie.mp4".to_string(),
            poster: None,
            has_controls: true,
            autoplay: false,
            is_loop: false,
            is_muted: false,
            is_playing: false,
            current_time: 0.0,
            duration: 120.0,
            intrinsic_width: 640.0,
            intrinsic_height: 360.0,
            poster_pixels: None,
            poster_width: 0,
            poster_height: 0,
        };
        assert!(!video.is_block());
        assert!(video.is_replaced());
        assert!(video.is_inline());
        assert!(video.is_video());
        assert!(video.is_media());
        assert!(video.is_atomic_inline());
        assert_eq!(video.intrinsic_size(), Some((640.0, 360.0)));
        assert_eq!(video.intrinsic_aspect_ratio(), Some(640.0 / 360.0));

        let audio = BoxType::Audio {
            src: "https://example.com/song.mp3".to_string(),
            has_controls: true,
            autoplay: false,
            is_loop: false,
            is_muted: false,
            is_playing: false,
            current_time: 0.0,
            duration: 180.0,
            intrinsic_width: 300.0,
            intrinsic_height: 36.0,
        };
        assert!(!audio.is_block());
        assert!(audio.is_replaced());
        assert!(audio.is_inline());
        assert!(audio.is_audio());
        assert!(audio.is_media());
        assert!(!audio.is_video());

        let canvas = BoxType::Canvas {
            node_id: None,
            width: 800,
            height: 600,
            intrinsic_width: 800.0,
            intrinsic_height: 600.0,
            pixels: None,
        };
        assert!(canvas.is_canvas());
        assert!(canvas.is_media());
        assert_eq!(canvas.intrinsic_size(), Some((800.0, 600.0)));
    }

    #[test]
    fn test_iframe_sandbox_all_whatwg_flags() {
        let unconstrained = IFrameSandbox::from_attribute(None);
        assert!(!unconstrained.is_sandboxed);
        assert!(unconstrained.allow_scripts);
        assert!(unconstrained.allow_same_origin);
        assert!(unconstrained.allow_downloads);
        assert!(unconstrained.allow_pointer_lock);

        let locked = IFrameSandbox::from_attribute(Some(""));
        assert!(locked.is_sandboxed);
        assert!(!locked.allow_scripts);
        assert!(!locked.allow_same_origin);
        assert!(!locked.allow_forms);
        assert!(!locked.allow_downloads);
        assert!(!locked.allow_pointer_lock);

        let modern = IFrameSandbox::from_attribute(Some(
            "allow-scripts allow-downloads allow-pointer-lock allow-top-navigation-by-user-activation",
        ));
        assert!(modern.is_sandboxed);
        assert!(modern.allow_scripts);
        assert!(modern.allow_downloads);
        assert!(modern.allow_pointer_lock);
        assert!(modern.allow_top_navigation_by_user_activation);
        assert!(!modern.allow_top_navigation);
        assert!(!modern.allow_forms);

        let serialized = modern.to_attribute_value();
        assert!(serialized.contains("allow-scripts"));
        assert!(serialized.contains("allow-downloads"));
        assert!(serialized.contains("allow-pointer-lock"));
        assert!(serialized.contains("allow-top-navigation-by-user-activation"));
    }

    #[test]
    fn test_margin_strut_css21_formula() {
        let mut strut = MarginStrut::new();
        assert_eq!(strut.solve(), 0.0);

        // Positive margins: max is kept
        strut.append(20.0);
        strut.append(10.0);
        strut.append(30.0);
        assert_eq!(strut.solve(), 30.0);

        // Negative margins: min is kept
        strut.append(-5.0);
        strut.append(-15.0);
        strut.append(-10.0);
        // Formula: max_pos (30.0) - abs(min_neg) (15.0) = 15.0
        assert_eq!(strut.solve(), 15.0);

        // Combined struts
        let mut strut2 = MarginStrut::new();
        strut2.append(40.0);
        strut2.append(-25.0);

        strut.append_strut(strut2);
        // max_pos = max(30, 40) = 40; min_neg = min(-15, -25) = -25.
        // solve = 40 - 25 = 15.
        assert_eq!(strut.solve(), 15.0);

        strut.reset();
        assert_eq!(strut.solve(), 0.0);
    }

    #[test]
    fn test_logical_edge_sizes_conversion() {
        let physical = EdgeSizes::new(10.0, 20.0, 30.0, 40.0); // top=10, right=20, bottom=30, left=40

        // Horizontal-TB, LTR: inline-start=left(40), inline-end=right(20), block-start=top(10), block-end=bottom(30)
        let logical_ltr =
            LogicalEdgeSizes::from_physical(&physical, WritingMode::HorizontalTb, Direction::Ltr);
        assert_eq!(logical_ltr.inline_start, 40.0);
        assert_eq!(logical_ltr.inline_end, 20.0);
        assert_eq!(logical_ltr.block_start, 10.0);
        assert_eq!(logical_ltr.block_end, 30.0);
        assert_eq!(
            logical_ltr.to_physical(WritingMode::HorizontalTb, Direction::Ltr),
            physical
        );

        // Horizontal-TB, RTL: inline-start=right(20), inline-end=left(40)
        let logical_rtl =
            LogicalEdgeSizes::from_physical(&physical, WritingMode::HorizontalTb, Direction::Rtl);
        assert_eq!(logical_rtl.inline_start, 20.0);
        assert_eq!(logical_rtl.inline_end, 40.0);
        assert_eq!(
            logical_rtl.to_physical(WritingMode::HorizontalTb, Direction::Rtl),
            physical
        );
    }

    #[test]
    fn test_overflow_model() {
        let border_box = Rect::new(0.0, 0.0, 200.0, 200.0);
        let mut overflow = OverflowModel::new(border_box);
        assert!(!overflow.has_scrollable_overflow(border_box));

        overflow.unite_scrollable(Rect::new(50.0, 50.0, 250.0, 100.0)); // right = 300
        assert!(overflow.has_scrollable_overflow(border_box));
        let (ext_x, ext_y) = overflow.scrollable_extent(border_box);
        assert_eq!(ext_x, 100.0);
        assert_eq!(ext_y, 0.0);
    }

    #[test]
    fn test_resolve_replaced_size_algorithm() {
        // Both specified
        assert_eq!(
            resolve_replaced_size(
                Some(100.0),
                Some(80.0),
                Some(200.0),
                Some(150.0),
                None,
                (300.0, 150.0)
            ),
            (100.0, 80.0)
        );

        // Width specified, height auto with aspect ratio
        assert_eq!(
            resolve_replaced_size(
                Some(200.0),
                None,
                Some(400.0),
                Some(200.0),
                None,
                (300.0, 150.0)
            ),
            (200.0, 100.0)
        );

        // Height specified, width auto with aspect ratio
        assert_eq!(
            resolve_replaced_size(
                None,
                Some(100.0),
                Some(400.0),
                Some(200.0),
                None,
                (300.0, 150.0)
            ),
            (200.0, 100.0)
        );

        // Both auto, intrinsic sizes exist
        assert_eq!(
            resolve_replaced_size(
                None,
                None,
                Some(640.0),
                Some(480.0),
                None,
                (300.0, 150.0)
            ),
            (640.0, 480.0)
        );
    }

    #[test]
    fn test_resolve_object_fit_rect() {
        let content_box = Rect::new(0.0, 0.0, 100.0, 100.0);
        let natural = (200.0, 100.0); // 2:1 aspect ratio

        // Contain: scaled to 100x50, centered vertically at y=25
        let contain =
            resolve_object_fit_rect(content_box, natural, ObjectFit::Contain, (0.5, 0.5));
        assert_eq!(contain, Rect::new(0.0, 25.0, 100.0, 50.0));

        // Cover: scaled to 200x100, centered horizontally at x=-50
        let cover = resolve_object_fit_rect(content_box, natural, ObjectFit::Cover, (0.5, 0.5));
        assert_eq!(cover, Rect::new(-50.0, 0.0, 200.0, 100.0));

        // Fill: stretched to 100x100
        let fill = resolve_object_fit_rect(content_box, natural, ObjectFit::Fill, (0.5, 0.5));
        assert_eq!(fill, Rect::new(0.0, 0.0, 100.0, 100.0));
    }
}
