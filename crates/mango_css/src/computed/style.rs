//! Definition and default values for [`ComputedStyle`].

use std::collections::HashMap;
use std::sync::Arc;

use mango_core::Color;

use crate::values::{
    AlignContent, AlignItems, AlignSelf, Animation, Appearance, BackgroundAttachment,
    BackgroundClip, BackgroundLayer, BackgroundRepeat, BackgroundSize, BlendMode, BorderCollapse,
    BorderImage, BorderStyle, BoxShadow, BoxSizing, BreakInside, CaptionSide, Clear, ClipPath,
    ClipRect, ColumnSpan, Contain, ContainerType, ContentItem, ContentVisibility, CounterAction,
    Cursor, Direction, Display, FilterFunction, FlexDirection, FlexWrap, Float, FontDisplay,
    FontFeatureSettings, FontStyle, FontVariationSettings, FontWeight, Gradient, GridAutoFlow,
    GridPlacement, GridTrackSize, Hyphens, Isolation, JustifyContent, JustifyItems, JustifySelf,
    Length, LineClamp, ListStylePosition, ListStyleType, MaskComposite, MaskMode, ObjectFit,
    Overflow, OverflowWrap, PointerEvents, Position, Resize, ScrollBehavior, ScrollTimelineAxis,
    ScrollbarWidth, TableLayout, TextAlign, TextDecoration, TextDecorationStyle,
    TextDecorationThickness, TextEmphasisStyle, TextOverflow, TextShadow, TextTransform, TextWrap,
    Transform, Transition, UnicodeBidi, UserSelect, Value, VerticalAlign, Visibility, WhiteSpace,
    WordBreak, WritingMode,
};

/// Fully resolved, typed computed styles for a single DOM element.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    pub custom_properties: Arc<HashMap<String, Value>>,
    /// Custom properties declared on this element that must not be inherited.
    /// Keeping these out of the shared inherited map avoids cloning the full
    /// page variable table when a descendant overrides one registered value.
    pub local_custom_properties: HashMap<String, Value>,
    pub display: Display,
    pub original_display: Display,
    pub position: Position,
    pub float: Float,
    pub clear: Clear,
    pub color: Color,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub background_color: Color,
    pub background_image: Option<String>,
    pub background_repeat: BackgroundRepeat,
    pub background_size: BackgroundSize,
    pub background_position: (Length, Length),
    pub background_attachment: BackgroundAttachment,
    pub background_clip: BackgroundClip,
    pub background_layers: Vec<BackgroundLayer>,
    pub border_image: Option<BorderImage>,
    pub isolation: Isolation,
    pub mask_image: Option<String>,
    pub mask_size: Option<BackgroundSize>,
    pub mask_repeat: Option<BackgroundRepeat>,
    pub mask_position: Option<(Length, Length)>,
    pub mask_composite: MaskComposite,
    pub box_shadow: Option<BoxShadow>,
    pub box_shadows: Vec<BoxShadow>,
    pub content: Option<String>,
    pub content_items: Option<Vec<ContentItem>>,
    pub counter_reset: Vec<CounterAction>,
    pub counter_increment: Vec<CounterAction>,
    pub quotes: Option<Vec<(String, String)>>,
    pub container_type: ContainerType,
    pub container_name: Option<String>,
    pub font_size: f32,
    pub root_font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_family: String,
    pub line_height: Option<f32>,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
    pub text_decoration_style: TextDecorationStyle,
    pub text_decoration_color: Option<Color>,
    pub text_transform: TextTransform,
    pub letter_spacing: Length,
    pub word_spacing: Length,
    pub text_indent: Length,
    pub direction: Direction,
    pub unicode_bidi: UnicodeBidi,

    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub max_width: Length,
    pub min_height: Length,
    pub max_height: Length,

    pub margin_top: Length,
    pub margin_right: Length,
    pub margin_bottom: Length,
    pub margin_left: Length,
    pub margin_trim: String,

    pub padding_top: Length,
    pub padding_right: Length,
    pub padding_bottom: Length,
    pub padding_left: Length,

    pub border_top_width: f32,
    pub border_right_width: f32,
    pub border_bottom_width: f32,
    pub border_left_width: f32,

    pub border_top_color: Color,
    pub border_right_color: Color,
    pub border_bottom_color: Color,
    pub border_left_color: Color,

    pub border_top_style: BorderStyle,
    pub border_right_style: BorderStyle,
    pub border_bottom_style: BorderStyle,
    pub border_left_style: BorderStyle,

    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,

    pub box_sizing: BoxSizing,

    // Flexbox properties
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_line_count: Option<u32>,
    pub justify_content: JustifyContent,
    pub justify_items: JustifyItems,
    pub justify_self: JustifySelf,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub align_content: AlignContent,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub row_gap: Length,
    pub column_gap: Length,
    pub order: i32,
    pub z_index: Option<i32>,

    // CSS Grid properties
    pub grid_template_columns: Vec<GridTrackSize>,
    pub grid_template_rows: Vec<GridTrackSize>,
    pub grid_template_areas: Vec<Vec<String>>,
    pub grid_column_lines: Vec<(String, usize)>,
    pub grid_row_lines: Vec<(String, usize)>,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_column_start: GridPlacement,
    pub grid_column_end: GridPlacement,
    pub grid_row_start: GridPlacement,
    pub grid_row_end: GridPlacement,

    // Visual formatting and display properties
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub visibility: Visibility,
    pub opacity: f32,
    pub border_top_left_radius: f32,
    pub border_top_right_radius: f32,
    pub border_bottom_right_radius: f32,
    pub border_bottom_left_radius: f32,
    pub border_top_left_radius_len: Length,
    pub border_top_right_radius_len: Length,
    pub border_bottom_right_radius_len: Length,
    pub border_bottom_left_radius_len: Length,
    pub white_space: WhiteSpace,
    pub text_overflow: TextOverflow,
    pub list_style_type: ListStyleType,
    pub list_style_position: ListStylePosition,
    pub cursor: Cursor,
    pub vertical_align: VerticalAlign,
    pub border_collapse: BorderCollapse,
    pub border_spacing: f32,
    pub table_layout: TableLayout,
    pub caption_side: CaptionSide,

    pub outline_width: f32,
    pub outline_style: BorderStyle,
    pub outline_color: Color,
    pub outline_offset: f32,

    pub column_count: Option<usize>,
    pub column_width: Option<Length>,
    pub column_rule_width: Length,
    pub column_rule_style: BorderStyle,
    pub column_rule_color: Color,
    pub column_span: ColumnSpan,
    pub break_inside: BreakInside,
    pub contain: Contain,
    pub contain_intrinsic_width: Option<Length>,
    pub contain_intrinsic_height: Option<Length>,
    pub content_visibility: ContentVisibility,
    pub writing_mode: WritingMode,
    pub resize: Resize,
    pub scrollbar_width: ScrollbarWidth,
    pub scrollbar_color: Option<(Color, Color)>,
    pub appearance: Appearance,
    pub pointer_events: PointerEvents,
    pub user_select: UserSelect,
    pub scroll_behavior: ScrollBehavior,
    pub scroll_snap_type: Option<String>,

    // Transform, filter, and animation properties
    /// CSS `transform` property — list of transform functions.
    pub transform: Transform,
    /// CSS `transform-origin` in px (defaults to 50% 50% = center).
    pub transform_origin_x: Length,
    pub transform_origin_y: Length,
    /// CSS `text-shadow` property.
    pub text_shadow: Option<TextShadow>,
    /// All declared `text-shadow` layers (supports multiple comma-separated shadows).
    pub text_shadows: Vec<TextShadow>,
    /// CSS `filter` property — list of filter functions.
    pub filter: Vec<FilterFunction>,
    /// CSS `aspect-ratio` as a width/height ratio (e.g. 16/9 → 1.777…). `None` = auto.
    pub aspect_ratio: Option<f32>,
    /// CSS `object-fit` for replaced elements.
    pub object_fit: ObjectFit,
    /// CSS `object-position` (x%, y%) defaults to 50% 50%.
    pub object_position: (Length, Length),

    /// CSS `transition-*` longhands assembled into a list of transitions.
    pub transitions: Vec<Transition>,
    /// CSS `animation-*` longhands assembled into a list of animations.
    pub animations: Vec<Animation>,
    /// Resolved `background-image: <gradient>`, when the element has a gradient background.
    pub background_gradient: Option<Gradient>,
    /// CSS `transform-origin-z` length (defaults to 0px).
    pub transform_origin_z: Length,
    /// CSS `backdrop-filter` property — list of filter functions applied to backdrop.
    pub backdrop_filter: Vec<FilterFunction>,
    /// CSS `mix-blend-mode` property.
    pub mix_blend_mode: BlendMode,
    /// CSS `background-blend-mode` property.
    pub background_blend_mode: BlendMode,
    /// CSS `clip-path` property.
    pub clip_path: ClipPath,
    /// CSS 2.1 `clip` property.
    pub clip: ClipRect,
    /// CSS `mask-mode` property.
    pub mask_mode: MaskMode,
    /// CSS `transform-style` is not yet tracked; presence of `will-change` is captured here.
    pub will_change: bool,
    /// CSS `will-change` property names list.
    pub will_change_properties: Vec<String>,

    // Text and typography properties
    pub word_break: WordBreak,
    pub overflow_wrap: OverflowWrap,
    pub hyphens: Hyphens,
    pub text_underline_offset: Length,
    pub text_decoration_thickness: TextDecorationThickness,
    pub text_emphasis_style: TextEmphasisStyle,
    pub text_emphasis_color: Option<Color>,
    pub font_feature_settings: FontFeatureSettings,
    pub font_variation_settings: FontVariationSettings,
    pub line_clamp: LineClamp,
    pub font_display: FontDisplay,
    pub accent_color: Option<Color>,

    // Anchor positioning, motion, and timeline properties
    pub text_wrap: TextWrap,
    pub anchor_name: Option<String>,
    pub position_anchor: Option<String>,
    pub position_try_fallbacks: Vec<String>,
    pub scroll_timeline_name: Option<String>,
    pub scroll_timeline_axis: ScrollTimelineAxis,
    pub view_timeline_name: Option<String>,
    pub view_timeline_axis: ScrollTimelineAxis,
    pub animation_timeline: Option<String>,
    pub view_transition_name: Option<String>,
}

impl ComputedStyle {
    /// Returns the resolved text decoration color (`text_decoration_color` or `color` if unset).
    #[inline]
    pub fn resolved_text_decoration_color(&self) -> Color {
        self.text_decoration_color.unwrap_or(self.color)
    }

    /// Returns all declared text shadows. If `text_shadows` is populated, returns it;
    /// otherwise falls back to `text_shadow` if present.
    #[inline]
    pub fn all_text_shadows(&self) -> &[TextShadow] {
        if !self.text_shadows.is_empty() {
            &self.text_shadows
        } else if let Some(ts) = &self.text_shadow {
            std::slice::from_ref(ts)
        } else {
            &[]
        }
    }

    /// Returns all declared box shadows. If `box_shadows` is populated, returns it;
    /// otherwise falls back to `box_shadow` if present.
    #[inline]
    pub fn all_box_shadows(&self) -> &[BoxShadow] {
        if !self.box_shadows.is_empty() {
            &self.box_shadows
        } else if let Some(bs) = &self.box_shadow {
            std::slice::from_ref(bs)
        } else {
            &[]
        }
    }

    /// Returns the border radii as [top-left, top-right, bottom-right, bottom-left].
    #[inline]
    pub fn border_radius(&self) -> [f32; 4] {
        [
            self.border_top_left_radius,
            self.border_top_right_radius,
            self.border_bottom_right_radius,
            self.border_bottom_left_radius,
        ]
    }

    /// Returns the border radii resolved against an explicit box dimension `(width, height)`.
    /// Percentage radii are computed against `min(width, height)` per CSS Backgrounds and Borders 3.
    #[inline]
    pub fn border_radius_with_size(&self, width: f32, height: f32) -> [f32; 4] {
        let size = width.min(height).max(0.0);
        let resolve = |len: Length, fallback: f32| -> f32 {
            match len {
                Length::Percent(_) => len
                    .to_px_with_viewport(self.font_size, self.root_font_size, size, size)
                    .max(0.0),
                Length::Px(px) if px > 0.0 => px,
                _ => fallback.max(0.0),
            }
        };
        [
            resolve(self.border_top_left_radius_len, self.border_top_left_radius),
            resolve(
                self.border_top_right_radius_len,
                self.border_top_right_radius,
            ),
            resolve(
                self.border_bottom_right_radius_len,
                self.border_bottom_right_radius,
            ),
            resolve(
                self.border_bottom_left_radius_len,
                self.border_bottom_left_radius,
            ),
        ]
    }

    /// Returns true if any corner has a non-zero border radius.
    #[inline]
    pub fn has_border_radius(&self) -> bool {
        self.border_top_left_radius > 0.0
            || self.border_top_right_radius > 0.0
            || self.border_bottom_right_radius > 0.0
            || self.border_bottom_left_radius > 0.0
            || matches!(self.border_top_left_radius_len, Length::Percent(p) if p > 0.0)
            || matches!(self.border_top_right_radius_len, Length::Percent(p) if p > 0.0)
            || matches!(self.border_bottom_right_radius_len, Length::Percent(p) if p > 0.0)
            || matches!(self.border_bottom_left_radius_len, Length::Percent(p) if p > 0.0)
    }

    /// Returns true if this element should be visually rendered.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.visibility == Visibility::Visible && self.opacity > 0.0
    }

    /// Returns the active quote delimiters, falling back to Chromium's default `“ ” ‘ ’`.
    pub fn effective_quotes(&self) -> Vec<(String, String)> {
        self.quotes.clone().unwrap_or_else(|| {
            vec![
                ("“".to_string(), "”".to_string()),
                ("‘".to_string(), "’".to_string()),
            ]
        })
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            custom_properties: Arc::new(HashMap::new()),
            local_custom_properties: HashMap::new(),
            display: Display::Inline,
            original_display: Display::Inline,
            position: Position::Static,
            float: Float::None,
            clear: Clear::None,
            color: Color::BLACK,
            fill: None,
            stroke: None,
            background_color: Color::TRANSPARENT,
            background_image: None,
            background_repeat: BackgroundRepeat::Repeat,
            background_size: BackgroundSize::Auto,
            background_position: (Length::Px(0.0), Length::Px(0.0)),
            background_attachment: BackgroundAttachment::Scroll,
            background_clip: BackgroundClip::BorderBox,
            background_layers: Vec::new(),
            border_image: None,
            isolation: Isolation::Auto,
            mask_image: None,
            mask_size: None,
            mask_repeat: None,
            mask_position: None,
            mask_composite: MaskComposite::Add,
            box_shadow: None,
            box_shadows: Vec::new(),
            content: None,
            content_items: None,
            counter_reset: Vec::new(),
            counter_increment: Vec::new(),
            quotes: None,
            container_type: ContainerType::Normal,
            container_name: None,
            font_size: 16.0,
            root_font_size: 16.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            font_family: "serif".to_string(),
            line_height: None,
            text_align: TextAlign::Start,
            text_decoration: TextDecoration::None,
            text_decoration_style: TextDecorationStyle::Solid,
            text_decoration_color: None,
            text_transform: TextTransform::None,
            letter_spacing: Length::Px(0.0),
            word_spacing: Length::Px(0.0),
            text_indent: Length::Px(0.0),
            direction: Direction::Ltr,
            unicode_bidi: UnicodeBidi::Normal,

            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            max_width: Length::Auto,
            min_height: Length::Auto,
            max_height: Length::Auto,

            margin_top: Length::Px(0.0),
            margin_right: Length::Px(0.0),
            margin_bottom: Length::Px(0.0),
            margin_left: Length::Px(0.0),
            margin_trim: "none".to_string(),

            padding_top: Length::Px(0.0),
            padding_right: Length::Px(0.0),
            padding_bottom: Length::Px(0.0),
            padding_left: Length::Px(0.0),

            border_top_width: 0.0,
            border_right_width: 0.0,
            border_bottom_width: 0.0,
            border_left_width: 0.0,

            border_top_color: Color::BLACK,
            border_right_color: Color::BLACK,
            border_bottom_color: Color::BLACK,
            border_left_color: Color::BLACK,

            border_top_style: BorderStyle::None,
            border_right_style: BorderStyle::None,
            border_bottom_style: BorderStyle::None,
            border_left_style: BorderStyle::None,

            top: Length::Auto,
            right: Length::Auto,
            bottom: Length::Auto,
            left: Length::Auto,

            box_sizing: BoxSizing::ContentBox,

            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_line_count: None,
            justify_content: JustifyContent::Normal,
            justify_items: JustifyItems::Stretch,
            justify_self: JustifySelf::Auto,
            align_items: AlignItems::Normal,
            align_self: AlignSelf::Auto,
            align_content: AlignContent::Normal,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            row_gap: Length::Px(0.0),
            column_gap: Length::Px(0.0),
            order: 0,
            z_index: None,

            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_template_areas: Vec::new(),
            grid_column_lines: Vec::new(),
            grid_row_lines: Vec::new(),
            grid_auto_flow: GridAutoFlow::Row,
            grid_column_start: GridPlacement::Auto,
            grid_column_end: GridPlacement::Auto,
            grid_row_start: GridPlacement::Auto,
            grid_row_end: GridPlacement::Auto,

            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            visibility: Visibility::Visible,
            opacity: 1.0,
            border_top_left_radius: 0.0,
            border_top_right_radius: 0.0,
            border_bottom_right_radius: 0.0,
            border_bottom_left_radius: 0.0,
            border_top_left_radius_len: Length::Px(0.0),
            border_top_right_radius_len: Length::Px(0.0),
            border_bottom_right_radius_len: Length::Px(0.0),
            border_bottom_left_radius_len: Length::Px(0.0),
            white_space: WhiteSpace::Normal,
            text_overflow: TextOverflow::Clip,
            list_style_type: ListStyleType::Disc,
            list_style_position: ListStylePosition::Outside,
            cursor: Cursor::Auto,
            vertical_align: VerticalAlign::Baseline,
            border_collapse: BorderCollapse::Separate,
            border_spacing: 0.0,
            table_layout: TableLayout::Auto,
            caption_side: CaptionSide::Top,

            outline_width: 0.0,
            outline_style: BorderStyle::None,
            outline_color: Color::TRANSPARENT,
            outline_offset: 0.0,

            column_count: None,
            column_width: None,
            column_rule_width: Length::Px(1.0),
            column_rule_style: BorderStyle::None,
            column_rule_color: Color::BLACK,
            column_span: ColumnSpan::None,
            break_inside: BreakInside::Auto,
            contain: Contain::None,
            contain_intrinsic_width: None,
            contain_intrinsic_height: None,
            content_visibility: ContentVisibility::Visible,
            writing_mode: WritingMode::HorizontalTb,
            resize: Resize::None,
            scrollbar_width: ScrollbarWidth::Auto,
            scrollbar_color: None,
            appearance: Appearance::Auto,
            pointer_events: PointerEvents::Auto,
            user_select: UserSelect::Auto,
            scroll_behavior: ScrollBehavior::Auto,
            scroll_snap_type: None,

            transform: Transform::default(),
            transform_origin_x: Length::Percent(50.0),
            transform_origin_y: Length::Percent(50.0),
            text_shadow: None,
            text_shadows: Vec::new(),
            filter: Vec::new(),
            aspect_ratio: None,
            object_fit: ObjectFit::Fill,
            object_position: (Length::Percent(50.0), Length::Percent(50.0)),

            transitions: Vec::new(),
            animations: Vec::new(),
            background_gradient: None,
            transform_origin_z: Length::Px(0.0),
            backdrop_filter: Vec::new(),
            mix_blend_mode: BlendMode::Normal,
            background_blend_mode: BlendMode::Normal,
            clip_path: ClipPath::None,
            clip: ClipRect::Auto,
            mask_mode: MaskMode::MatchSource,
            will_change: false,
            will_change_properties: Vec::new(),
            word_break: WordBreak::Normal,
            overflow_wrap: OverflowWrap::Normal,
            hyphens: Hyphens::Manual,
            text_underline_offset: Length::Auto,
            text_decoration_thickness: TextDecorationThickness::Auto,
            text_emphasis_style: TextEmphasisStyle::None,
            text_emphasis_color: None,
            font_feature_settings: FontFeatureSettings::Normal,
            font_variation_settings: FontVariationSettings::Normal,
            line_clamp: LineClamp::None,
            font_display: FontDisplay::Auto,
            accent_color: None,

            text_wrap: TextWrap::default(),
            anchor_name: None,
            position_anchor: None,
            position_try_fallbacks: Vec::new(),
            scroll_timeline_name: None,
            scroll_timeline_axis: ScrollTimelineAxis::default(),
            view_timeline_name: None,
            view_timeline_axis: ScrollTimelineAxis::default(),
            animation_timeline: None,
            view_transition_name: None,
        }
    }
}
