use std::sync::Arc;

use mango_css::computed::ComputedStyle;
use mango_css::values::Display;

use crate::box_model::BoxType;
use crate::dimensions::Dimensions;

use super::types::LayoutBox;

impl LayoutBox {
    /// Counts this box and descendants without allocating. Used by browser
    /// memory policy before retaining a layout snapshot in BFCache.
    pub fn descendant_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(LayoutBox::descendant_count)
            .sum::<usize>()
    }

    pub fn new(box_type: BoxType, style: Option<Arc<ComputedStyle>>) -> Self {
        Self {
            box_type,
            dimensions: Dimensions::default(),
            style,
            children: Vec::new(),
            link_target: None,
            link_frame: None,
            node_id: None,
            tag_name: None,
            attributes: Vec::new(),
            map_areas: Vec::new(),
            is_dirty: true,
            raw_children: None,
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
            is_independent_formatting_context: false,
            positioned_descendant_cache: std::sync::OnceLock::new(),
            custom_containing_block: None,
            fixed_containing_block: None,
            containing_inline_id: None,
            positioned_inline_style: None,
            static_position: None,
            is_static_bottom_anchored: false,
            containing_block_writing_mode: None,
        }
    }

    pub fn new_with_style(box_type: BoxType, style: ComputedStyle) -> Self {
        Self::new(box_type, Some(Arc::new(style)))
    }

    pub fn set_style(&mut self, style: impl Into<Arc<ComputedStyle>>) {
        self.style = Some(style.into());
    }

    /// Marks this box and all descendants as dirty for display list invalidation.
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        for child in &mut self.children {
            child.mark_dirty();
        }
    }

    /// Marks this box and all descendants as clean after display list generation.
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
        for child in &mut self.children {
            child.mark_clean();
        }
    }

    /// Returns true if this box is visible according to CSS `visibility`.
    /// Note: boxes with `visibility: hidden` occupy layout space but do not receive pointer hits or paint.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.style
            .as_ref()
            .is_none_or(|s| s.visibility == mango_css::values::Visibility::Visible)
    }

    /// Returns the intrinsic size `(width, height)` of this box if it is a replaced or media element.
    #[inline]
    pub fn intrinsic_size(&self) -> Option<(f32, f32)> {
        self.box_type.intrinsic_size()
    }

    /// Returns the intrinsic aspect ratio `(width / height)` of this box if applicable.
    #[inline]
    pub fn intrinsic_aspect_ratio(&self) -> Option<f32> {
        self.box_type.intrinsic_aspect_ratio()
    }

    pub fn new_anonymous_block(style: Option<Arc<ComputedStyle>>) -> Self {
        let style = style.map(|s| {
            let mut s = (*s).clone();
            // Anonymous blocks only inherit inheritable properties (CSS 2.1 §9.2.1.1).
            // Reset all non-inheritable box/positioning/visual props to initial.
            if !matches!(
                s.display,
                Display::TableRow
                    | Display::TableCell
                    | Display::TableRowGroup
                    | Display::TableHeaderGroup
                    | Display::TableFooterGroup
                    | Display::TableColumn
                    | Display::TableColumnGroup
                    | Display::TableCaption
            ) {
                s.display = Display::Block;
            }
            s.clear = mango_css::values::Clear::None;
            s.float = mango_css::values::Float::None;
            s.position = mango_css::values::Position::Static;
            s.overflow_x = mango_css::values::Overflow::Visible;
            s.overflow_y = mango_css::values::Overflow::Visible;
            s.z_index = None;
            s.transform = mango_css::values::Transform::default();
            s.opacity = 1.0;
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
            s.border_top_style = mango_css::values::BorderStyle::None;
            s.border_right_style = mango_css::values::BorderStyle::None;
            s.border_bottom_style = mango_css::values::BorderStyle::None;
            s.border_left_style = mango_css::values::BorderStyle::None;
            s.border_top_color = mango_core::Color::BLACK;
            s.border_right_color = mango_core::Color::BLACK;
            s.border_bottom_color = mango_core::Color::BLACK;
            s.border_left_color = mango_core::Color::BLACK;
            s.border_top_left_radius = 0.0;
            s.border_top_right_radius = 0.0;
            s.border_bottom_right_radius = 0.0;
            s.border_bottom_left_radius = 0.0;
            s.width = mango_css::values::Length::Auto;
            s.height = mango_css::values::Length::Auto;
            s.min_width = mango_css::values::Length::Auto;
            s.max_width = mango_css::values::Length::Auto;
            s.min_height = mango_css::values::Length::Auto;
            s.max_height = mango_css::values::Length::Auto;
            s.background_color = mango_core::Color::TRANSPARENT;
            s.background_image = None;
            s.background_layers = Vec::new();
            s.background_gradient = None;
            s.box_shadow = None;
            s.outline_width = 0.0;
            s.outline_style = mango_css::values::BorderStyle::None;
            s.outline_color = mango_core::Color::TRANSPARENT;
            s.outline_offset = 0.0;
            s.top = mango_css::values::Length::Auto;
            s.right = mango_css::values::Length::Auto;
            s.bottom = mango_css::values::Length::Auto;
            s.left = mango_css::values::Length::Auto;
            s.filter = Vec::new();
            s.backdrop_filter = Vec::new();
            s.mix_blend_mode = mango_css::values::BlendMode::Normal;
            s.background_blend_mode = mango_css::values::BlendMode::Normal;
            s.clip_path = mango_css::values::ClipPath::None;
            s.mask_image = None;
            s.isolation = mango_css::values::Isolation::Auto;
            s.contain = mango_css::values::Contain::None;
            s.content_visibility = mango_css::values::ContentVisibility::Visible;
            s.will_change = false;
            s.will_change_properties = Vec::new();
            Arc::new(s)
        });
        let mut dimensions = Dimensions::default();
        if let Some(ref s) = style {
            dimensions.writing_mode = s.writing_mode;
        }
        Self {
            box_type: BoxType::AnonymousBlock,
            dimensions,
            style,
            children: Vec::new(),
            link_target: None,
            link_frame: None,
            node_id: None,
            tag_name: None,
            attributes: Vec::new(),
            map_areas: Vec::new(),
            is_dirty: true,
            raw_children: None,
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
            is_independent_formatting_context: false,
            positioned_descendant_cache: std::sync::OnceLock::new(),
            custom_containing_block: None,
            fixed_containing_block: None,
            containing_inline_id: None,
            positioned_inline_style: None,
            static_position: None,
            is_static_bottom_anchored: false,
            containing_block_writing_mode: None,
        }
    }

    /// Returns whether any descendant (or this box) is positioned, memoized
    /// for the current tree generation.
    pub fn has_positioned_descendant(&self) -> bool {
        if let Some(value) = self.positioned_descendant_cache.get() {
            return *value;
        }
        let value = self.children.iter().any(|child| {
            child
                .style
                .as_ref()
                .is_some_and(|s| s.position != mango_css::values::Position::Static)
                || child.has_positioned_descendant()
        });
        let _ = self.positioned_descendant_cache.set(value);
        value
    }

    /// Looks up an attribute value by name (case-insensitive).
    #[inline]
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    #[inline]
    pub fn is_block(&self) -> bool {
        match &self.box_type {
            BoxType::BlockNode | BoxType::AnonymousBlock => true,
            BoxType::InlineNode | BoxType::InlineBlock | BoxType::TextNode(_) => false,
            _ => {
                if let Some(style) = &self.style {
                    match style.display {
                        Display::Block | Display::Flex | Display::Grid => true,
                        Display::Inline
                        | Display::InlineBlock
                        | Display::InlineFlex
                        | Display::InlineGrid => false,
                        _ => self.box_type.is_block(),
                    }
                } else {
                    self.box_type.is_block()
                }
            }
        }
    }

    #[inline]
    pub fn is_out_of_flow(&self) -> bool {
        self.style
            .as_ref()
            .map(|s| {
                matches!(
                    s.position,
                    mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                )
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_floating(&self) -> bool {
        self.style
            .as_ref()
            .map(|s| s.float != mango_css::values::Float::None)
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_in_flow_block(&self) -> bool {
        self.is_block() && !self.is_out_of_flow() && !self.is_floating()
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
}
