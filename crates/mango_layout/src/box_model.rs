//! Box model types: block, inline, anonymous blocks, and text boxes.

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

/// Security and capability restrictions applied to an `<iframe>` nested browsing context.
///
/// Implements standard WHATWG HTML5 sandboxing flags:
/// - `allow-scripts`
/// - `allow-same-origin`
/// - `allow-forms`
/// - `allow-top-navigation`
/// - `allow-popups`
/// - `allow-modals`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IFrameSandbox {
    pub is_sandboxed: bool,
    pub allow_scripts: bool,
    pub allow_same_origin: bool,
    pub allow_forms: bool,
    pub allow_top_navigation: bool,
    pub allow_popups: bool,
    pub allow_modals: bool,
}

impl Default for IFrameSandbox {
    fn default() -> Self {
        Self {
            is_sandboxed: false,
            allow_scripts: true,
            allow_same_origin: true,
            allow_forms: true,
            allow_top_navigation: true,
            allow_popups: true,
            allow_modals: true,
        }
    }
}

impl IFrameSandbox {
    /// Parses a standard HTML5 `sandbox` attribute value.
    ///
    /// If `sandbox_attr` is `None`, sandboxing is inactive.
    /// If `sandbox_attr` is `Some("")`, all restrictions are enforced.
    /// Otherwise, space-separated tokens lift specific restrictions.
    pub fn from_attribute(sandbox_attr: Option<&str>) -> Self {
        match sandbox_attr {
            None => Self::default(),
            Some(tokens_str) => {
                let tokens: Vec<&str> = tokens_str.split_whitespace().collect();
                Self {
                    is_sandboxed: true,
                    allow_scripts: tokens.iter().any(|&t| t.eq_ignore_ascii_case("allow-scripts")),
                    allow_same_origin: tokens.iter().any(|&t| t.eq_ignore_ascii_case("allow-same-origin")),
                    allow_forms: tokens.iter().any(|&t| t.eq_ignore_ascii_case("allow-forms")),
                    allow_top_navigation: tokens.iter().any(|&t| {
                        t.eq_ignore_ascii_case("allow-top-navigation")
                            || t.eq_ignore_ascii_case("allow-top-navigation-by-user-activation")
                    }),
                    allow_popups: tokens.iter().any(|&t| t.eq_ignore_ascii_case("allow-popups")),
                    allow_modals: tokens.iter().any(|&t| t.eq_ignore_ascii_case("allow-modals")),
                }
            }
        }
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_type_predicates() {
        assert!(BoxType::BlockNode.is_block());
        assert!(!BoxType::BlockNode.is_inline());

        assert!(BoxType::AnonymousBlock.is_block());
        assert!(BoxType::AnonymousBlock.is_anonymous());

        assert!(BoxType::InlineNode.is_inline());
        assert!(!BoxType::InlineNode.is_block());

        let text = BoxType::TextNode("Hello".to_string());
        assert!(text.is_inline());
        assert!(text.is_text());

        let replaced = BoxType::ReplacedElement {
            intrinsic_width: 100.0,
            intrinsic_height: 50.0,
            pixels: vec![],
        };
        assert!(!replaced.is_block());
        assert!(replaced.is_replaced());
        assert!(replaced.is_inline());

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
        assert!(!video.is_audio());

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
        assert!(!audio.is_video());
    }

    #[test]
    fn test_iframe_sandbox_parsing() {
        let unconstrained = IFrameSandbox::from_attribute(None);
        assert!(!unconstrained.is_sandboxed);
        assert!(unconstrained.allow_scripts);
        assert!(unconstrained.allow_same_origin);

        let locked = IFrameSandbox::from_attribute(Some(""));
        assert!(locked.is_sandboxed);
        assert!(!locked.allow_scripts);
        assert!(!locked.allow_same_origin);
        assert!(!locked.allow_forms);

        let scripts_only = IFrameSandbox::from_attribute(Some("allow-scripts allow-forms"));
        assert!(scripts_only.is_sandboxed);
        assert!(scripts_only.allow_scripts);
        assert!(scripts_only.allow_forms);
        assert!(!scripts_only.allow_same_origin);
        assert!(!scripts_only.allow_popups);
    }
}
