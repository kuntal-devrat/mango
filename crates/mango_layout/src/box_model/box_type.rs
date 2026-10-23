use super::iframe::IFrameSandbox;

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
        /// Document-scoped canvas backing-store key.
        canvas_key: Option<u64>,
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
