//! Computed style resolution and CSS property inheritance.
//!
//! Converts cascaded property values into concrete, typed [`ComputedStyle`] structs
//! ready for the layout engine.

use std::collections::{HashMap, HashSet};

use mango_core::Color;
use mango_html::dom::{Document, NodeId};

use crate::parser::Stylesheet;
use crate::values::{
    AlignContent, AlignItems, AlignSelf, Appearance, BackgroundAttachment, BackgroundClip,
    BackgroundLayer, BackgroundRepeat, BackgroundSize, BorderCollapse, BorderImage, BorderStyle,
    BoxShadow, BoxSizing, CaptionSide, Clear, Cursor, Direction, Display, FlexDirection, FlexWrap,
    Float, FontStyle, FontWeight, GridAutoFlow, GridPlacement, GridTrackSize, Isolation,
    JustifyContent, Length, ListStylePosition, ListStyleType, MaskComposite, Overflow, Position,
    TableLayout, TextAlign, TextDecoration, TextOverflow, TextTransform, UnicodeBidi, Value,
    VerticalAlign, Visibility, WhiteSpace,
};
/// Fully resolved, typed computed styles for a single DOM element.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    pub custom_properties: HashMap<String, Value>,
    pub display: Display,
    pub position: Position,
    pub float: Float,
    pub clear: Clear,
    pub color: Color,
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
    pub content: Option<String>,
    pub content_items: Option<Vec<crate::values::ContentItem>>,
    pub counter_reset: Vec<crate::values::CounterAction>,
    pub counter_increment: Vec<crate::values::CounterAction>,
    pub quotes: Option<Vec<(String, String)>>,
    pub container_type: crate::values::ContainerType,
    pub container_name: Option<String>,
    pub font_size: f32,
    pub root_font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_family: String,
    pub line_height: Option<f32>,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
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
    pub justify_content: JustifyContent,
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

    // Phase 6 CSS properties
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub visibility: Visibility,
    pub opacity: f32,
    pub border_top_left_radius: f32,
    pub border_top_right_radius: f32,
    pub border_bottom_right_radius: f32,
    pub border_bottom_left_radius: f32,
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
    pub column_span: crate::values::ColumnSpan,
    pub break_inside: crate::values::BreakInside,
    pub contain: crate::values::Contain,
    pub content_visibility: crate::values::ContentVisibility,
    pub writing_mode: crate::values::WritingMode,
    pub resize: crate::values::Resize,
    pub scrollbar_width: crate::values::ScrollbarWidth,
    pub scrollbar_color: Option<(Color, Color)>,
    pub appearance: Appearance,

    // Phase 7 CSS properties
    /// CSS `transform` property — list of transform functions.
    pub transform: crate::values::Transform,
    /// CSS `transform-origin` in px (defaults to 50% 50% = center).
    pub transform_origin_x: Length,
    pub transform_origin_y: Length,
    /// CSS `text-shadow` property.
    pub text_shadow: Option<crate::values::TextShadow>,
    /// CSS `filter` property — list of filter functions.
    pub filter: Vec<crate::values::FilterFunction>,
    /// CSS `aspect-ratio` as a width/height ratio (e.g. 16/9 → 1.777…). `None` = auto.
    pub aspect_ratio: Option<f32>,
    /// CSS `object-fit` for replaced elements.
    pub object_fit: crate::values::ObjectFit,
    /// CSS `object-position` (x%, y%) defaults to 50% 50%.
    pub object_position: (Length, Length),

    /// CSS `transition-*` longhands assembled into a list of transitions.
    pub transitions: Vec<crate::values::Transition>,
    /// CSS `animation-*` longhands assembled into a list of animations.
    pub animations: Vec<crate::values::Animation>,
    /// Resolved `background-image: <gradient>`, when the element has a gradient background.
    pub background_gradient: Option<crate::values::Gradient>,
    /// CSS `transform-origin-z` length (defaults to 0px).
    pub transform_origin_z: Length,
    /// CSS `backdrop-filter` property — list of filter functions applied to backdrop.
    pub backdrop_filter: Vec<crate::values::FilterFunction>,
    /// CSS `mix-blend-mode` property.
    pub mix_blend_mode: crate::values::BlendMode,
    /// CSS `background-blend-mode` property.
    pub background_blend_mode: crate::values::BlendMode,
    /// CSS `clip-path` property.
    pub clip_path: crate::values::ClipPath,
    /// CSS `mask-mode` property.
    pub mask_mode: crate::values::MaskMode,
    /// CSS `transform-style` is not yet tracked; presence of `will-change` is captured here.
    pub will_change: bool,
    /// CSS `will-change` property names list.
    pub will_change_properties: Vec<String>,

    // Phase 2 / Section 6.4 Text & Font properties
    pub word_break: crate::values::WordBreak,
    pub overflow_wrap: crate::values::OverflowWrap,
    pub hyphens: crate::values::Hyphens,
    pub text_underline_offset: Length,
    pub text_decoration_thickness: crate::values::TextDecorationThickness,
    pub text_emphasis_style: crate::values::TextEmphasisStyle,
    pub text_emphasis_color: Option<Color>,
    pub font_feature_settings: crate::values::FontFeatureSettings,
    pub font_variation_settings: crate::values::FontVariationSettings,
    pub line_clamp: crate::values::LineClamp,
    pub font_display: crate::values::FontDisplay,
    pub accent_color: Option<Color>,
}

impl ComputedStyle {
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

    /// Returns true if any corner has a non-zero border radius.
    #[inline]
    pub fn has_border_radius(&self) -> bool {
        self.border_top_left_radius > 0.0
            || self.border_top_right_radius > 0.0
            || self.border_bottom_right_radius > 0.0
            || self.border_bottom_left_radius > 0.0
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
            custom_properties: HashMap::new(),
            display: Display::Inline,
            position: Position::Static,
            float: Float::None,
            clear: Clear::None,
            color: Color::BLACK,
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
            content: None,
            content_items: None,
            counter_reset: Vec::new(),
            counter_increment: Vec::new(),
            quotes: None,
            container_type: crate::values::ContainerType::Normal,
            container_name: None,
            font_size: 16.0,
            root_font_size: 16.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            font_family: "sans-serif".to_string(),
            line_height: None,
            text_align: TextAlign::Left,
            text_decoration: TextDecoration::None,
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
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            align_self: AlignSelf::Auto,
            align_content: AlignContent::Stretch,
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
            white_space: WhiteSpace::Normal,
            text_overflow: TextOverflow::Clip,
            list_style_type: ListStyleType::Disc,
            list_style_position: ListStylePosition::Outside,
            cursor: Cursor::Auto,
            vertical_align: VerticalAlign::Baseline,
            border_collapse: BorderCollapse::Separate,
            border_spacing: 2.0,
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
            column_span: crate::values::ColumnSpan::None,
            break_inside: crate::values::BreakInside::Auto,
            contain: crate::values::Contain::None,
            content_visibility: crate::values::ContentVisibility::Visible,
            writing_mode: crate::values::WritingMode::HorizontalTb,
            resize: crate::values::Resize::None,
            scrollbar_width: crate::values::ScrollbarWidth::Auto,
            scrollbar_color: None,
            appearance: Appearance::Auto,

            transform: crate::values::Transform::default(),
            transform_origin_x: Length::Percent(50.0),
            transform_origin_y: Length::Percent(50.0),
            text_shadow: None,
            filter: Vec::new(),
            aspect_ratio: None,
            object_fit: crate::values::ObjectFit::Fill,
            object_position: (Length::Percent(50.0), Length::Percent(50.0)),

            transitions: Vec::new(),
            animations: Vec::new(),
            background_gradient: None,
            transform_origin_z: Length::Px(0.0),
            backdrop_filter: Vec::new(),
            mix_blend_mode: crate::values::BlendMode::Normal,
            background_blend_mode: crate::values::BlendMode::Normal,
            clip_path: crate::values::ClipPath::None,
            mask_mode: crate::values::MaskMode::MatchSource,
            will_change: false,
            will_change_properties: Vec::new(),
            word_break: crate::values::WordBreak::Normal,
            overflow_wrap: crate::values::OverflowWrap::Normal,
            hyphens: crate::values::Hyphens::Manual,
            text_underline_offset: Length::Auto,
            text_decoration_thickness: crate::values::TextDecorationThickness::Auto,
            text_emphasis_style: crate::values::TextEmphasisStyle::None,
            text_emphasis_color: None,
            font_feature_settings: crate::values::FontFeatureSettings::Normal,
            font_variation_settings: crate::values::FontVariationSettings::Normal,
            line_clamp: crate::values::LineClamp::None,
            font_display: crate::values::FontDisplay::Auto,
            accent_color: None,
        }
    }
}

#[inline]
fn is_current_color(val: &Value) -> bool {
    match val {
        Value::CurrentColor => true,
        Value::Keyword(k) => k.eq_ignore_ascii_case("currentcolor"),
        _ => false,
    }
}

/// Recursively resolves `var(--name, fallback)` references using the element's custom properties.
fn resolve_value_vars(
    val: &Value,
    custom_props: &HashMap<String, Value>,
    visited: &mut HashSet<String>,
) -> Option<Value> {
    match val {
        Value::Var { name, fallback } => {
            if visited.contains(name) {
                // Cycle detected: fallback or invalid
                return fallback
                    .as_ref()
                    .and_then(|fb| resolve_value_vars(fb, custom_props, visited));
            }
            visited.insert(name.clone());
            let res = if let Some(target_val) = custom_props.get(name) {
                resolve_value_vars(target_val, custom_props, visited).or_else(|| {
                    fallback
                        .as_ref()
                        .and_then(|fb| resolve_value_vars(fb, custom_props, visited))
                })
            } else {
                fallback
                    .as_ref()
                    .and_then(|fb| resolve_value_vars(fb, custom_props, visited))
            };
            visited.remove(name);
            res
        }
        Value::List(items) => {
            let mut has_var = false;
            for item in items {
                if matches!(item, Value::Var { .. }) {
                    has_var = true;
                    break;
                }
            }
            if has_var {
                let mut new_items = Vec::with_capacity(items.len());
                for item in items {
                    let resolved_item = resolve_value_vars(item, custom_props, visited)
                        .unwrap_or_else(|| item.clone());
                    new_items.push(resolved_item);
                }
                Some(Value::List(new_items))
            } else {
                Some(val.clone())
            }
        }
        _ => Some(val.clone()),
    }
}

/// Computes the final style for a DOM node using a pre-indexed author stylesheet rule index.
pub fn compute_style_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    let cascaded = crate::cascade::resolve_cascade_with_index(node_id, doc, author_index);
    let mut style = ComputedStyle::default();

    // ── 1. Inherited properties: start with parent values if available ──
    if let Some(parent) = parent_style {
        style.root_font_size = parent.root_font_size;
        style.custom_properties = parent.custom_properties.clone();
        style.color = parent.color;
        style.font_size = parent.font_size;
        style.font_weight = parent.font_weight;
        style.font_style = parent.font_style;
        style.font_family = parent.font_family.clone();
        style.line_height = parent.line_height;
        style.text_align = parent.text_align;
        style.text_transform = parent.text_transform;
        style.letter_spacing = parent.letter_spacing;
        style.word_spacing = parent.word_spacing;
        style.text_indent = parent.text_indent;
        style.direction = parent.direction;
        style.visibility = parent.visibility;
        style.white_space = parent.white_space;
        style.list_style_type = parent.list_style_type;
        style.list_style_position = parent.list_style_position;
        style.cursor = parent.cursor;
        style.border_collapse = parent.border_collapse;
        style.border_spacing = parent.border_spacing;
        style.caption_side = parent.caption_side;
        style.writing_mode = parent.writing_mode;
        style.scrollbar_width = parent.scrollbar_width;
        style.scrollbar_color = parent.scrollbar_color;
        style.word_break = parent.word_break;
        style.overflow_wrap = parent.overflow_wrap;
        style.hyphens = parent.hyphens;
        style.text_emphasis_style = parent.text_emphasis_style.clone();
        style.text_emphasis_color = parent.text_emphasis_color;
        style.font_feature_settings = parent.font_feature_settings.clone();
        style.font_variation_settings = parent.font_variation_settings.clone();
        style.accent_color = parent.accent_color;
        style.quotes = parent.quotes.clone();
    }

    // Collect custom properties defined directly on this element
    for (prop, val) in &cascaded {
        if prop.starts_with("--") {
            style.custom_properties.insert(prop.clone(), val.clone());
        }
    }

    // Pre-resolve custom property var(--*) references in cascaded declarations
    let mut resolved_cascaded: HashMap<String, Value> = HashMap::with_capacity(cascaded.len());
    for (prop, val) in &cascaded {
        if prop.starts_with("--") {
            continue;
        }
        let mut visited = HashSet::new();
        let resolved_val = resolve_value_vars(val, &style.custom_properties, &mut visited)
            .unwrap_or_else(|| val.clone());
        resolved_cascaded.insert(prop.clone(), resolved_val);
    }

    apply_cascaded_properties(&mut style, &resolved_cascaded, parent_style);
    if parent_style.is_none() {
        style.root_font_size = style.font_size;
    }
    style
}

/// Computes the style for a pseudo-element (`before` or `after`) of an element using a pre-indexed author stylesheet rule index.
pub fn compute_pseudo_style_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    host_style: &ComputedStyle,
    pseudo: &str,
) -> Option<ComputedStyle> {
    let cascaded = crate::cascade::resolve_pseudo_element_cascade_with_index(
        node_id,
        doc,
        author_index,
        pseudo,
    );
    if cascaded.is_empty() {
        return None;
    }

    let mut style = ComputedStyle {
        color: host_style.color,
        font_size: host_style.font_size,
        root_font_size: host_style.root_font_size,
        font_weight: host_style.font_weight,
        font_style: host_style.font_style,
        font_family: host_style.font_family.clone(),
        line_height: host_style.line_height,
        text_align: host_style.text_align,
        text_transform: host_style.text_transform,
        letter_spacing: host_style.letter_spacing,
        word_spacing: host_style.word_spacing,
        text_indent: host_style.text_indent,
        visibility: host_style.visibility,
        white_space: host_style.white_space,
        cursor: host_style.cursor,
        word_break: host_style.word_break,
        overflow_wrap: host_style.overflow_wrap,
        hyphens: host_style.hyphens,
        text_emphasis_style: host_style.text_emphasis_style.clone(),
        text_emphasis_color: host_style.text_emphasis_color,
        font_feature_settings: host_style.font_feature_settings.clone(),
        font_variation_settings: host_style.font_variation_settings.clone(),
        accent_color: host_style.accent_color,
        quotes: host_style.quotes.clone(),
        custom_properties: host_style.custom_properties.clone(),
        display: Display::Inline,
        ..Default::default()
    };

    let mut resolved_cascaded: HashMap<String, Value> = HashMap::with_capacity(cascaded.len());
    for (prop, val) in &cascaded {
        if prop.starts_with("--") {
            continue;
        }
        let mut visited = HashSet::new();
        let resolved_val = resolve_value_vars(val, &style.custom_properties, &mut visited)
            .unwrap_or_else(|| val.clone());
        resolved_cascaded.insert(prop.clone(), resolved_val);
    }

    apply_cascaded_properties(&mut style, &resolved_cascaded, Some(host_style));
    Some(style)
}

/// Computes the final style for a DOM node given the document and author stylesheets.
pub fn compute_style(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    let mut order = 1000;
    let author_index = crate::cascade::RuleIndex::from_stylesheets(
        author_stylesheets,
        crate::cascade::Origin::Author,
        &mut order,
    );
    compute_style_with_index(node_id, doc, &author_index, parent_style)
}

/// Computes the style for a pseudo-element (`before` or `after`) of an element.
pub fn compute_pseudo_style(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
    host_style: &ComputedStyle,
    pseudo: &str,
) -> Option<ComputedStyle> {
    let mut order = 1000;
    let author_index = crate::cascade::RuleIndex::from_stylesheets(
        author_stylesheets,
        crate::cascade::Origin::Author,
        &mut order,
    );
    compute_pseudo_style_with_index(node_id, doc, &author_index, host_style, pseudo)
}

fn parse_position_len(val: &Value) -> Option<Length> {
    match val {
        Value::Length(l) => Some(*l),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "left" | "top" => Some(Length::Percent(0.0)),
            "center" => Some(Length::Percent(50.0)),
            "right" | "bottom" => Some(Length::Percent(100.0)),
            _ => None,
        },
        _ => None,
    }
}

fn parse_position_val(val: &Value) -> Option<(Length, Length)> {
    match val {
        Value::Length(l) => Some((*l, *l)),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "center" => Some((Length::Percent(50.0), Length::Percent(50.0))),
            "top" => Some((Length::Percent(50.0), Length::Percent(0.0))),
            "bottom" => Some((Length::Percent(50.0), Length::Percent(100.0))),
            "left" => Some((Length::Percent(0.0), Length::Percent(50.0))),
            "right" => Some((Length::Percent(100.0), Length::Percent(50.0))),
            _ => None,
        },
        Value::List(items) if items.len() >= 2 => {
            let x = parse_position_len(&items[0])?;
            let y = parse_position_len(&items[1])?;
            Some((x, y))
        }
        _ => None,
    }
}

#[inline]
fn get_keyword_str(v: &Value) -> Option<&str> {
    match v {
        Value::Keyword(k) | Value::String(k) => Some(k.as_str()),
        Value::Float(Float::Left) => Some("left"),
        Value::Float(Float::Right) => Some("right"),
        Value::TextAlign(TextAlign::Center) => Some("center"),
        Value::VerticalAlign(VerticalAlign::Top) => Some("top"),
        Value::VerticalAlign(VerticalAlign::Bottom) => Some("bottom"),
        _ => None,
    }
}

/// Applies parsed, variable-resolved declarations to a ComputedStyle.
pub fn apply_cascaded_properties(
    style: &mut ComputedStyle,
    resolved_cascaded: &HashMap<String, Value>,
    parent_style: Option<&ComputedStyle>,
) {
    // ── 2. Font Size (must be resolved first so em lengths can use it) ──
    if let Some(val) = resolved_cascaded.get("font-size") {
        let parent_fs = parent_style.map(|p| p.font_size).unwrap_or(16.0);
        match val {
            Value::Length(Length::Px(px)) => style.font_size = *px,
            Value::Length(Length::Em(em)) => style.font_size = *em * parent_fs,
            Value::Length(Length::Rem(rem)) => style.font_size = *rem * style.root_font_size,
            Value::Length(Length::Percent(pct)) => style.font_size = (*pct / 100.0) * parent_fs,
            Value::Number(n) => style.font_size = *n,
            _ => {}
        }
    }

    // ── 2b. Apply color early so currentColor resolves accurately for all other properties ──
    if let Some(val) = resolved_cascaded.get("color") {
        if let Value::Color(c) = val {
            style.color = *c;
        } else if let Value::Keyword(s) | Value::String(s) = val {
            if let Some(c) = Value::parse_color(s) {
                style.color = c;
            }
        } else if is_current_color(val)
            && let Some(p) = parent_style
        {
            style.color = p.color;
        }
    }

    // ── 3. Apply all remaining cascaded properties ──
    for (prop, val) in resolved_cascaded {
        match prop.as_str() {
            "display" => match val {
                Value::Display(d) => style.display = *d,
                Value::Keyword(k) => match k.as_str() {
                    "block" => style.display = Display::Block,
                    "inline" => style.display = Display::Inline,
                    "inline-block" => style.display = Display::InlineBlock,
                    "flex" => style.display = Display::Flex,
                    "inline-flex" => style.display = Display::InlineFlex,
                    "grid" => style.display = Display::Grid,
                    "inline-grid" => style.display = Display::InlineGrid,
                    "flow-root" => style.display = Display::FlowRoot,
                    "contents" => style.display = Display::Contents,
                    "table" => style.display = Display::Table,
                    "table-row" => style.display = Display::TableRow,
                    "table-cell" => style.display = Display::TableCell,
                    "table-caption" => style.display = Display::TableCaption,
                    "list-item" => style.display = Display::ListItem,
                    "ruby" => style.display = Display::Ruby,
                    "ruby-base" => style.display = Display::RubyBase,
                    "ruby-text" => style.display = Display::RubyText,
                    "none" => style.display = Display::None,
                    _ => {}
                },
                _ => {}
            },
            "position" => {
                if let Value::Position(p) = val {
                    style.position = *p;
                }
            }
            "float" => {
                if let Value::Float(f) = val {
                    style.float = *f;
                }
            }
            "clear" => match val {
                Value::Clear(c) => style.clear = *c,
                Value::Float(Float::Left) => style.clear = Clear::Left,
                Value::Float(Float::Right) => style.clear = Clear::Right,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "left" => style.clear = Clear::Left,
                    "right" => style.clear = Clear::Right,
                    "both" => style.clear = Clear::Both,
                    "none" => style.clear = Clear::None,
                    _ => {}
                },
                _ => {}
            },
            "color" => {
                if let Value::Color(c) = val {
                    style.color = *c;
                } else if let Value::Keyword(s) | Value::String(s) = val {
                    if let Some(c) = Value::parse_color(s) {
                        style.color = c;
                    }
                } else if is_current_color(val)
                    && let Some(p) = parent_style
                {
                    style.color = p.color;
                }
            }
            "background-color" => {
                if let Value::Color(c) = val {
                    style.background_color = *c;
                } else if let Value::Keyword(s) | Value::String(s) = val {
                    if let Some(c) = Value::parse_color(s) {
                        style.background_color = c;
                    }
                } else if is_current_color(val) {
                    style.background_color = style.color;
                }
            }
            "background-image" => match val {
                Value::Url(url) => {
                    style.background_image = Some(url.clone());
                    style.background_layers = vec![BackgroundLayer {
                        image: Some(url.clone()),
                        repeat: style.background_repeat,
                        size: style.background_size,
                        position: style.background_position,
                        attachment: style.background_attachment,
                        clip: style.background_clip,
                        ..Default::default()
                    }];
                }
                Value::String(url) => {
                    style.background_image = Some(url.clone());
                    style.background_layers = vec![BackgroundLayer {
                        image: Some(url.clone()),
                        repeat: style.background_repeat,
                        size: style.background_size,
                        position: style.background_position,
                        attachment: style.background_attachment,
                        clip: style.background_clip,
                        ..Default::default()
                    }];
                }
                Value::Gradient(g) => {
                    let mut grad = (**g).clone();
                    grad.resolve_current_color(style.color);
                    style.background_gradient = Some(grad.clone());
                    style.background_layers = vec![BackgroundLayer {
                        gradient: Some(Box::new(grad)),
                        repeat: style.background_repeat,
                        size: style.background_size,
                        position: style.background_position,
                        attachment: style.background_attachment,
                        clip: style.background_clip,
                        ..Default::default()
                    }];
                }
                Value::List(items) => {
                    let mut layers = Vec::new();
                    for it in items {
                        let mut layer = BackgroundLayer {
                            repeat: style.background_repeat,
                            size: style.background_size,
                            position: style.background_position,
                            attachment: style.background_attachment,
                            clip: style.background_clip,
                            ..Default::default()
                        };
                        match it {
                            Value::Url(u) | Value::String(u) => layer.image = Some(u.clone()),
                            Value::Gradient(g) => {
                                let mut grad = (**g).clone();
                                grad.resolve_current_color(style.color);
                                layer.gradient = Some(Box::new(grad));
                            }
                            _ => {}
                        }
                        layers.push(layer);
                    }
                    if let Some(first) = layers.first() {
                        style.background_image = first.image.clone();
                        style.background_gradient = first.gradient.clone().map(|g| *g);
                    }
                    style.background_layers = layers;
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.background_image = None;
                    style.background_gradient = None;
                    style.background_layers.clear();
                }
                _ => {}
            },
            "background-gradient" => match val {
                Value::Gradient(g) => {
                    let mut grad = (**g).clone();
                    grad.resolve_current_color(style.color);
                    style.background_gradient = Some(grad);
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.background_gradient = None;
                }
                _ => {}
            },
            "background-repeat" => match val {
                Value::BackgroundRepeat(r) => {
                    style.background_repeat = *r;
                    for layer in &mut style.background_layers {
                        layer.repeat = *r;
                    }
                }
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "repeat" => {
                        style.background_repeat = BackgroundRepeat::Repeat;
                        for layer in &mut style.background_layers {
                            layer.repeat = BackgroundRepeat::Repeat;
                        }
                    }
                    "repeat-x" => {
                        style.background_repeat = BackgroundRepeat::RepeatX;
                        for layer in &mut style.background_layers {
                            layer.repeat = BackgroundRepeat::RepeatX;
                        }
                    }
                    "repeat-y" => {
                        style.background_repeat = BackgroundRepeat::RepeatY;
                        for layer in &mut style.background_layers {
                            layer.repeat = BackgroundRepeat::RepeatY;
                        }
                    }
                    "no-repeat" => {
                        style.background_repeat = BackgroundRepeat::NoRepeat;
                        for layer in &mut style.background_layers {
                            layer.repeat = BackgroundRepeat::NoRepeat;
                        }
                    }
                    _ => {}
                },
                Value::List(items) => {
                    for (i, it) in items.iter().enumerate() {
                        let rep = match it {
                            Value::BackgroundRepeat(r) => Some(*r),
                            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                                "repeat" => Some(BackgroundRepeat::Repeat),
                                "repeat-x" => Some(BackgroundRepeat::RepeatX),
                                "repeat-y" => Some(BackgroundRepeat::RepeatY),
                                "no-repeat" => Some(BackgroundRepeat::NoRepeat),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(r) = rep
                            && i < style.background_layers.len()
                        {
                            style.background_layers[i].repeat = r;
                        }
                    }
                }
                _ => {}
            },
            "background-size" => match val {
                Value::BackgroundSize(s) => {
                    style.background_size = *s;
                    for layer in &mut style.background_layers {
                        layer.size = *s;
                    }
                }
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => {
                        style.background_size = BackgroundSize::Auto;
                        for layer in &mut style.background_layers {
                            layer.size = BackgroundSize::Auto;
                        }
                    }
                    "cover" => {
                        style.background_size = BackgroundSize::Cover;
                        for layer in &mut style.background_layers {
                            layer.size = BackgroundSize::Cover;
                        }
                    }
                    "contain" => {
                        style.background_size = BackgroundSize::Contain;
                        for layer in &mut style.background_layers {
                            layer.size = BackgroundSize::Contain;
                        }
                    }
                    _ => {}
                },
                Value::Length(len) => {
                    let s = BackgroundSize::Explicit(*len, Length::Auto);
                    style.background_size = s;
                    for layer in &mut style.background_layers {
                        layer.size = s;
                    }
                }
                Value::List(items) if items.len() >= 2 => {
                    let parse_dim = |v: &Value| match v {
                        Value::Length(len) => Some(*len),
                        Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => Some(Length::Auto),
                        _ => None,
                    };
                    if let (Some(w), Some(h)) = (parse_dim(&items[0]), parse_dim(&items[1])) {
                        let s = BackgroundSize::Explicit(w, h);
                        style.background_size = s;
                        for layer in &mut style.background_layers {
                            layer.size = s;
                        }
                    }
                }
                _ => {}
            },
            "background-position" => {
                if let Some(pos) = parse_position_val(val) {
                    style.background_position = pos;
                    for layer in &mut style.background_layers {
                        layer.position = pos;
                    }
                }
            }
            "background-attachment" => match val {
                Value::BackgroundAttachment(a) => {
                    style.background_attachment = *a;
                    for layer in &mut style.background_layers {
                        layer.attachment = *a;
                    }
                }
                Value::Keyword(k) => {
                    if let Some(a) = BackgroundAttachment::parse(k) {
                        style.background_attachment = a;
                        for layer in &mut style.background_layers {
                            layer.attachment = a;
                        }
                    }
                }
                Value::List(items) => {
                    for (i, it) in items.iter().enumerate() {
                        let att = match it {
                            Value::BackgroundAttachment(a) => Some(*a),
                            Value::Keyword(k) => BackgroundAttachment::parse(k),
                            _ => None,
                        };
                        if let Some(a) = att
                            && i < style.background_layers.len()
                        {
                            style.background_layers[i].attachment = a;
                        }
                    }
                    if let Some(first) = style.background_layers.first() {
                        style.background_attachment = first.attachment;
                    }
                }
                _ => {}
            },
            "background-clip" | "-webkit-background-clip" => match val {
                Value::BackgroundClip(c) => {
                    style.background_clip = *c;
                    for layer in &mut style.background_layers {
                        layer.clip = *c;
                    }
                }
                Value::Keyword(k) => {
                    if let Some(c) = BackgroundClip::parse(k) {
                        style.background_clip = c;
                        for layer in &mut style.background_layers {
                            layer.clip = c;
                        }
                    }
                }
                Value::List(items) => {
                    for (i, it) in items.iter().enumerate() {
                        let cl = match it {
                            Value::BackgroundClip(c) => Some(*c),
                            Value::Keyword(k) => BackgroundClip::parse(k),
                            _ => None,
                        };
                        if let Some(c) = cl
                            && i < style.background_layers.len()
                        {
                            style.background_layers[i].clip = c;
                        }
                    }
                    if let Some(first) = style.background_layers.first() {
                        style.background_clip = first.clip;
                    }
                }
                _ => {}
            },
            "border-image" => {
                if let Value::BorderImage(bi) = val {
                    style.border_image = Some(bi.clone())
                }
            }
            "border-image-source" => match val {
                Value::Url(url) => {
                    let bi = style.border_image.get_or_insert_with(BorderImage::default);
                    bi.source = Some(url.clone());
                    bi.gradient = None;
                }
                Value::Gradient(g) => {
                    let bi = style.border_image.get_or_insert_with(BorderImage::default);
                    bi.gradient = Some(g.clone());
                    bi.source = None;
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    if let Some(bi) = &mut style.border_image {
                        bi.source = None;
                        bi.gradient = None;
                    }
                }
                _ => {}
            },
            "mask-composite" | "-webkit-mask-composite" => match val {
                Value::MaskComposite(mc) => style.mask_composite = *mc,
                Value::Keyword(k) => {
                    if let Some(mc) = MaskComposite::parse(k) {
                        style.mask_composite = mc;
                    }
                }
                _ => {}
            },
            "isolation" => match val {
                Value::Isolation(iso) => style.isolation = *iso,
                Value::Keyword(k) => {
                    if let Some(iso) = Isolation::parse(k) {
                        style.isolation = iso;
                    }
                }
                _ => {}
            },
            "mask-image" | "-webkit-mask-image" => match val {
                Value::Url(url) => style.mask_image = Some(url.clone()),
                Value::String(url) => style.mask_image = Some(url.clone()),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.mask_image = None;
                }
                _ => {}
            },
            "mask-size" | "-webkit-mask-size" => match val {
                Value::BackgroundSize(s) => style.mask_size = Some(*s),
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.mask_size = Some(BackgroundSize::Auto),
                    "cover" => style.mask_size = Some(BackgroundSize::Cover),
                    "contain" => style.mask_size = Some(BackgroundSize::Contain),
                    _ => {}
                },
                Value::Length(len) => {
                    style.mask_size = Some(BackgroundSize::Explicit(*len, Length::Auto))
                }
                Value::List(items) if items.len() >= 2 => {
                    let w = match &items[0] {
                        Value::Length(len) => *len,
                        _ => Length::Auto,
                    };
                    let h = match &items[1] {
                        Value::Length(len) => *len,
                        _ => Length::Auto,
                    };
                    style.mask_size = Some(BackgroundSize::Explicit(w, h));
                }
                _ => {}
            },
            "mask-repeat" | "-webkit-mask-repeat" => match val {
                Value::BackgroundRepeat(r) => style.mask_repeat = Some(*r),
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "repeat" => style.mask_repeat = Some(BackgroundRepeat::Repeat),
                    "repeat-x" => style.mask_repeat = Some(BackgroundRepeat::RepeatX),
                    "repeat-y" => style.mask_repeat = Some(BackgroundRepeat::RepeatY),
                    "no-repeat" => style.mask_repeat = Some(BackgroundRepeat::NoRepeat),
                    _ => {}
                },
                _ => {}
            },
            "mask-position" | "-webkit-mask-position" => {
                if let Some(pos) = parse_position_val(val) {
                    style.mask_position = Some(pos);
                }
            }
            "font-weight" => match val {
                Value::FontWeight(w) => style.font_weight = *w,
                Value::Number(n) => {
                    let w = *n as u16;
                    style.font_weight = match w {
                        100..=900 => FontWeight::Numeric(w),
                        _ => FontWeight::Normal,
                    };
                }
                Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                    "bold" => style.font_weight = FontWeight::Bold,
                    "normal" => style.font_weight = FontWeight::Normal,
                    "bolder" => style.font_weight = FontWeight::Bolder,
                    "lighter" => style.font_weight = FontWeight::Lighter,
                    s => {
                        if let Ok(n) = s.parse::<u16>() {
                            style.font_weight = FontWeight::Numeric(n);
                        }
                    }
                },
                _ => {}
            },
            "font-style" => match val {
                Value::FontStyle(s) => style.font_style = *s,
                Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                    "italic" => style.font_style = FontStyle::Italic,
                    "oblique" => style.font_style = FontStyle::Oblique,
                    "normal" => style.font_style = FontStyle::Normal,
                    _ => {}
                },
                _ => {}
            },
            "appearance" | "-webkit-appearance" | "-moz-appearance" | "-ms-appearance"
            | "-o-appearance" => match val {
                Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                    "none" => style.appearance = Appearance::None,
                    "auto" => style.appearance = Appearance::Auto,
                    _ => {}
                },
                _ => {}
            },
            "font-family" => match val {
                Value::String(s) | Value::Keyword(s) => style.font_family = s.clone(),
                Value::List(items) => {
                    let mut names = Vec::new();
                    for item in items {
                        match item {
                            Value::String(s) | Value::Keyword(s) => names.push(s.clone()),
                            _ => {}
                        }
                    }
                    if !names.is_empty() {
                        style.font_family = names.join(", ");
                    }
                }
                _ => {}
            },
            "line-height" => match val {
                Value::Number(n) => style.line_height = Some(*n * style.font_size),
                Value::Length(len) => {
                    style.line_height = Some(len.to_px(style.font_size, 16.0, 100.0))
                }
                _ => {}
            },
            "text-align" => match val {
                Value::TextAlign(ta) => style.text_align = *ta,
                Value::Float(Float::Left) => style.text_align = TextAlign::Left,
                Value::Float(Float::Right) => style.text_align = TextAlign::Right,
                Value::Clear(Clear::Left) => style.text_align = TextAlign::Left,
                Value::Clear(Clear::Right) => style.text_align = TextAlign::Right,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "left" => style.text_align = TextAlign::Left,
                    "right" => style.text_align = TextAlign::Right,
                    "center" => style.text_align = TextAlign::Center,
                    "justify" => style.text_align = TextAlign::Justify,
                    _ => {}
                },
                _ => {}
            },
            "text-decoration" => {
                if let Value::TextDecoration(td) = val {
                    style.text_decoration = *td;
                }
            }
            "text-transform" => match val {
                Value::TextTransform(tt) => style.text_transform = *tt,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "uppercase" => style.text_transform = TextTransform::Uppercase,
                    "lowercase" => style.text_transform = TextTransform::Lowercase,
                    "capitalize" => style.text_transform = TextTransform::Capitalize,
                    "none" => style.text_transform = TextTransform::None,
                    _ => {}
                },
                _ => {}
            },
            "letter-spacing" => match val {
                Value::Length(l) => style.letter_spacing = *l,
                Value::Number(n) if *n == 0.0 => style.letter_spacing = Length::Px(0.0),
                Value::Keyword(k) if k.eq_ignore_ascii_case("normal") => {
                    style.letter_spacing = Length::Px(0.0)
                }
                _ => {}
            },
            "word-spacing" => match val {
                Value::Length(l) => style.word_spacing = *l,
                Value::Number(n) if *n == 0.0 => style.word_spacing = Length::Px(0.0),
                Value::Keyword(k) if k.eq_ignore_ascii_case("normal") => {
                    style.word_spacing = Length::Px(0.0)
                }
                _ => {}
            },
            "text-indent" => match val {
                Value::Length(l) => style.text_indent = *l,
                Value::Number(n) if *n == 0.0 => style.text_indent = Length::Px(0.0),
                _ => {}
            },
            "direction" => match val {
                Value::Direction(d) => style.direction = *d,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "rtl" => style.direction = Direction::Rtl,
                    _ => style.direction = Direction::Ltr,
                },
                _ => {}
            },
            "unicode-bidi" => match val {
                Value::UnicodeBidi(b) => style.unicode_bidi = *b,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "embed" => style.unicode_bidi = UnicodeBidi::Embed,
                    "isolate" => style.unicode_bidi = UnicodeBidi::Isolate,
                    "bidi-override" => style.unicode_bidi = UnicodeBidi::BidiOverride,
                    "isolate-override" => style.unicode_bidi = UnicodeBidi::IsolateOverride,
                    "plaintext" => style.unicode_bidi = UnicodeBidi::Plaintext,
                    _ => style.unicode_bidi = UnicodeBidi::Normal,
                },
                _ => {}
            },

            // Dimensions
            "width" => match val {
                Value::Length(l) => style.width = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.width = Length::Auto,
                    "content" => style.width = Length::Content,
                    "min-content" => style.width = Length::MinContent,
                    "max-content" => style.width = Length::MaxContent,
                    "fit-content" => style.width = Length::FitContent,
                    _ => {}
                },
                _ => {}
            },
            "height" => match val {
                Value::Length(l) => style.height = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.height = Length::Auto,
                    "content" => style.height = Length::Content,
                    "min-content" => style.height = Length::MinContent,
                    "max-content" => style.height = Length::MaxContent,
                    "fit-content" => style.height = Length::FitContent,
                    _ => {}
                },
                _ => {}
            },
            "min-width" => match val {
                Value::Length(l) => style.min_width = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.min_width = Length::Auto,
                    "content" => style.min_width = Length::Content,
                    "min-content" => style.min_width = Length::MinContent,
                    "max-content" => style.min_width = Length::MaxContent,
                    "fit-content" => style.min_width = Length::FitContent,
                    _ => {}
                },
                _ => {}
            },
            "max-width" => match val {
                Value::Length(l) => style.max_width = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.max_width = Length::Auto,
                    "content" => style.max_width = Length::Content,
                    "min-content" => style.max_width = Length::MinContent,
                    "max-content" => style.max_width = Length::MaxContent,
                    "fit-content" => style.max_width = Length::FitContent,
                    _ => {}
                },
                _ => {}
            },
            "min-height" => match val {
                Value::Length(l) => style.min_height = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.min_height = Length::Auto,
                    "content" => style.min_height = Length::Content,
                    "min-content" => style.min_height = Length::MinContent,
                    "max-content" => style.min_height = Length::MaxContent,
                    "fit-content" => style.min_height = Length::FitContent,
                    _ => {}
                },
                _ => {}
            },
            "max-height" => match val {
                Value::Length(l) => style.max_height = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "auto" => style.max_height = Length::Auto,
                    "content" => style.max_height = Length::Content,
                    "min-content" => style.max_height = Length::MinContent,
                    "max-content" => style.max_height = Length::MaxContent,
                    "fit-content" => style.max_height = Length::FitContent,
                    _ => {}
                },
                _ => {}
            },

            // Margins
            "margin-top" => {
                if let Value::Length(l) = val {
                    style.margin_top = *l;
                }
            }
            "margin-right" => {
                if let Value::Length(l) = val {
                    style.margin_right = *l;
                }
            }
            "margin-bottom" => {
                if let Value::Length(l) = val {
                    style.margin_bottom = *l;
                }
            }
            "margin-left" => {
                if let Value::Length(l) = val {
                    style.margin_left = *l;
                }
            }

            // Padding
            "padding-top" => {
                if let Value::Length(l) = val {
                    style.padding_top = *l;
                }
            }
            "padding-right" => {
                if let Value::Length(l) = val {
                    style.padding_right = *l;
                }
            }
            "padding-bottom" => {
                if let Value::Length(l) = val {
                    style.padding_bottom = *l;
                }
            }
            "padding-left" => {
                if let Value::Length(l) = val {
                    style.padding_left = *l;
                }
            }

            // Border widths
            "border-top-width" => {
                if let Value::Length(l) = val {
                    style.border_top_width = l.to_px(style.font_size, 16.0, 100.0);
                }
            }
            "border-right-width" => {
                if let Value::Length(l) = val {
                    style.border_right_width = l.to_px(style.font_size, 16.0, 100.0);
                }
            }
            "border-bottom-width" => {
                if let Value::Length(l) = val {
                    style.border_bottom_width = l.to_px(style.font_size, 16.0, 100.0);
                }
            }
            "border-left-width" => {
                if let Value::Length(l) = val {
                    style.border_left_width = l.to_px(style.font_size, 16.0, 100.0);
                }
            }

            // Border colors
            "border-top-color" => {
                if let Value::Color(c) = val {
                    style.border_top_color = *c;
                } else if let Value::Keyword(s) | Value::String(s) = val {
                    if let Some(c) = Value::parse_color(s) {
                        style.border_top_color = c;
                    }
                } else if is_current_color(val) {
                    style.border_top_color = style.color;
                }
            }
            "border-right-color" => {
                if let Value::Color(c) = val {
                    style.border_right_color = *c;
                } else if let Value::Keyword(s) | Value::String(s) = val {
                    if let Some(c) = Value::parse_color(s) {
                        style.border_right_color = c;
                    }
                } else if is_current_color(val) {
                    style.border_right_color = style.color;
                }
            }
            "border-bottom-color" => {
                if let Value::Color(c) = val {
                    style.border_bottom_color = *c;
                } else if let Value::Keyword(s) | Value::String(s) = val {
                    if let Some(c) = Value::parse_color(s) {
                        style.border_bottom_color = c;
                    }
                } else if is_current_color(val) {
                    style.border_bottom_color = style.color;
                }
            }
            "border-left-color" => {
                if let Value::Color(c) = val {
                    style.border_left_color = *c;
                } else if let Value::Keyword(s) | Value::String(s) = val {
                    if let Some(c) = Value::parse_color(s) {
                        style.border_left_color = c;
                    }
                } else if is_current_color(val) {
                    style.border_left_color = style.color;
                }
            }

            // Border styles
            "border-top-style" => {
                if let Some(s) = parse_border_style_value(val) {
                    style.border_top_style = s;
                }
            }
            "border-right-style" => {
                if let Some(s) = parse_border_style_value(val) {
                    style.border_right_style = s;
                }
            }
            "border-bottom-style" => {
                if let Some(s) = parse_border_style_value(val) {
                    style.border_bottom_style = s;
                }
            }
            "border-left-style" => {
                if let Some(s) = parse_border_style_value(val) {
                    style.border_left_style = s;
                }
            }

            // Positioning offsets
            "top" => {
                if let Value::Length(l) = val {
                    style.top = *l;
                }
            }
            "right" => {
                if let Value::Length(l) = val {
                    style.right = *l;
                }
            }
            "bottom" => {
                if let Value::Length(l) = val {
                    style.bottom = *l;
                }
            }
            "left" => {
                if let Value::Length(l) = val {
                    style.left = *l;
                }
            }

            "box-sizing" => {
                if let Value::BoxSizing(bs) = val {
                    style.box_sizing = *bs;
                }
            }

            // Flexbox & positioning
            "flex-direction" => {
                if let Value::FlexDirection(fd) = val {
                    style.flex_direction = *fd;
                }
            }
            "flex-wrap" => {
                if let Value::FlexWrap(fw) = val {
                    style.flex_wrap = *fw;
                }
            }
            "justify-content" => match val {
                Value::JustifyContent(jc) => style.justify_content = *jc,
                Value::TextAlign(TextAlign::Center) => {
                    style.justify_content = JustifyContent::Center
                }
                Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                    "center" => style.justify_content = JustifyContent::Center,
                    "flex-start" => style.justify_content = JustifyContent::FlexStart,
                    "flex-end" => style.justify_content = JustifyContent::FlexEnd,
                    "space-between" => style.justify_content = JustifyContent::SpaceBetween,
                    "space-around" => style.justify_content = JustifyContent::SpaceAround,
                    "space-evenly" => style.justify_content = JustifyContent::SpaceEvenly,
                    _ => {}
                },
                _ => {}
            },
            "align-items" => match val {
                Value::AlignItems(ai) => style.align_items = *ai,
                Value::TextAlign(TextAlign::Center) => style.align_items = AlignItems::Center,
                Value::JustifyContent(JustifyContent::FlexStart) => {
                    style.align_items = AlignItems::FlexStart
                }
                Value::JustifyContent(JustifyContent::FlexEnd) => {
                    style.align_items = AlignItems::FlexEnd
                }
                Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                    "center" => style.align_items = AlignItems::Center,
                    "flex-start" => style.align_items = AlignItems::FlexStart,
                    "flex-end" => style.align_items = AlignItems::FlexEnd,
                    "stretch" => style.align_items = AlignItems::Stretch,
                    "baseline" => style.align_items = AlignItems::Baseline,
                    _ => {}
                },
                _ => {}
            },
            "align-self" => match val {
                Value::AlignSelf(as_val) => style.align_self = *as_val,
                Value::AlignItems(ai) => {
                    style.align_self = match ai {
                        AlignItems::Stretch => AlignSelf::Stretch,
                        AlignItems::FlexStart => AlignSelf::FlexStart,
                        AlignItems::FlexEnd => AlignSelf::FlexEnd,
                        AlignItems::Center => AlignSelf::Center,
                        AlignItems::Baseline => AlignSelf::Baseline,
                    };
                }
                Value::TextAlign(TextAlign::Center) => style.align_self = AlignSelf::Center,
                Value::JustifyContent(JustifyContent::FlexStart) => {
                    style.align_self = AlignSelf::FlexStart
                }
                Value::JustifyContent(JustifyContent::FlexEnd) => {
                    style.align_self = AlignSelf::FlexEnd
                }
                Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                    "auto" => style.align_self = AlignSelf::Auto,
                    "center" => style.align_self = AlignSelf::Center,
                    "flex-start" => style.align_self = AlignSelf::FlexStart,
                    "flex-end" => style.align_self = AlignSelf::FlexEnd,
                    "stretch" => style.align_self = AlignSelf::Stretch,
                    "baseline" => style.align_self = AlignSelf::Baseline,
                    _ => {}
                },
                _ => {}
            },
            "align-content" => match val {
                Value::AlignContent(ac) => style.align_content = *ac,
                Value::JustifyContent(jc) => {
                    style.align_content = match jc {
                        JustifyContent::FlexStart => AlignContent::FlexStart,
                        JustifyContent::FlexEnd => AlignContent::FlexEnd,
                        JustifyContent::Center => AlignContent::Center,
                        JustifyContent::SpaceBetween => AlignContent::SpaceBetween,
                        JustifyContent::SpaceAround => AlignContent::SpaceAround,
                        JustifyContent::SpaceEvenly => AlignContent::SpaceEvenly,
                    };
                }
                Value::AlignItems(ai) => {
                    style.align_content = match ai {
                        AlignItems::Stretch => AlignContent::Stretch,
                        AlignItems::FlexStart => AlignContent::FlexStart,
                        AlignItems::FlexEnd => AlignContent::FlexEnd,
                        AlignItems::Center => AlignContent::Center,
                        _ => AlignContent::Stretch,
                    };
                }
                Value::TextAlign(TextAlign::Center) => style.align_content = AlignContent::Center,
                Value::Keyword(kw) => match kw.to_ascii_lowercase().as_str() {
                    "stretch" | "normal" => style.align_content = AlignContent::Stretch,
                    "flex-start" | "start" => style.align_content = AlignContent::FlexStart,
                    "flex-end" | "end" => style.align_content = AlignContent::FlexEnd,
                    "center" => style.align_content = AlignContent::Center,
                    "space-between" => style.align_content = AlignContent::SpaceBetween,
                    "space-around" => style.align_content = AlignContent::SpaceAround,
                    "space-evenly" => style.align_content = AlignContent::SpaceEvenly,
                    _ => {}
                },
                _ => {}
            },
            "flex-grow" => {
                if let Value::Number(n) = val {
                    style.flex_grow = (*n).max(0.0);
                } else if let Value::Length(Length::Px(px)) = val {
                    style.flex_grow = (*px).max(0.0);
                }
            }
            "flex-shrink" => {
                if let Value::Number(n) = val {
                    style.flex_shrink = (*n).max(0.0);
                } else if let Value::Length(Length::Px(px)) = val {
                    style.flex_shrink = (*px).max(0.0);
                }
            }
            "flex-basis" => match val {
                Value::Length(l) => style.flex_basis = *l,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "content" => style.flex_basis = Length::Content,
                    "min-content" => style.flex_basis = Length::MinContent,
                    "max-content" => style.flex_basis = Length::MaxContent,
                    "fit-content" => style.flex_basis = Length::FitContent,
                    "auto" => style.flex_basis = Length::Auto,
                    _ => {}
                },
                _ => {}
            },
            "row-gap" => {
                if let Value::Length(l) = val {
                    style.row_gap = *l;
                } else if let Value::Number(n) = val {
                    style.row_gap = Length::Px(*n);
                } else if let Value::Keyword(k) = val
                    && k.eq_ignore_ascii_case("normal")
                {
                    style.row_gap = Length::Px(0.0);
                }
            }
            "column-gap" => {
                if let Value::Length(l) = val {
                    style.column_gap = *l;
                } else if let Value::Number(n) = val {
                    style.column_gap = Length::Px(*n);
                } else if let Value::Keyword(k) = val
                    && k.eq_ignore_ascii_case("normal")
                {
                    style.column_gap = Length::Px(16.0);
                }
            }
            "order" => {
                if let Value::Number(n) = val {
                    style.order = *n as i32;
                } else if let Value::Length(Length::Px(px)) = val {
                    style.order = *px as i32;
                }
            }
            "z-index" => match val {
                Value::Number(n) => style.z_index = Some(*n as i32),
                Value::Length(Length::Px(px)) => style.z_index = Some(*px as i32),
                Value::Keyword(k) if k == "auto" => style.z_index = None,
                _ => {}
            },

            // Phase 6 CSS properties
            "overflow" => match val {
                Value::Overflow(o) => {
                    style.overflow_x = *o;
                    style.overflow_y = *o;
                }
                Value::Keyword(k) => match k.as_str() {
                    "visible" => {
                        style.overflow_x = Overflow::Visible;
                        style.overflow_y = Overflow::Visible;
                    }
                    "hidden" => {
                        style.overflow_x = Overflow::Hidden;
                        style.overflow_y = Overflow::Hidden;
                    }
                    "scroll" => {
                        style.overflow_x = Overflow::Scroll;
                        style.overflow_y = Overflow::Scroll;
                    }
                    "auto" => {
                        style.overflow_x = Overflow::Auto;
                        style.overflow_y = Overflow::Auto;
                    }
                    _ => {}
                },
                _ => {}
            },
            "overflow-x" => match val {
                Value::Overflow(o) => style.overflow_x = *o,
                Value::Visibility(Visibility::Hidden) => style.overflow_x = Overflow::Hidden,
                Value::Visibility(Visibility::Visible) => style.overflow_x = Overflow::Visible,
                Value::Keyword(k) => match k.as_str() {
                    "visible" => style.overflow_x = Overflow::Visible,
                    "hidden" => style.overflow_x = Overflow::Hidden,
                    "scroll" => style.overflow_x = Overflow::Scroll,
                    "auto" => style.overflow_x = Overflow::Auto,
                    _ => {}
                },
                _ => {}
            },
            "overflow-y" => match val {
                Value::Overflow(o) => style.overflow_y = *o,
                Value::Visibility(Visibility::Hidden) => style.overflow_y = Overflow::Hidden,
                Value::Visibility(Visibility::Visible) => style.overflow_y = Overflow::Visible,
                Value::Keyword(k) => match k.as_str() {
                    "visible" => style.overflow_y = Overflow::Visible,
                    "hidden" => style.overflow_y = Overflow::Hidden,
                    "scroll" => style.overflow_y = Overflow::Scroll,
                    "auto" => style.overflow_y = Overflow::Auto,
                    _ => {}
                },
                _ => {}
            },
            "visibility" => match val {
                Value::Visibility(v) => style.visibility = *v,
                Value::Overflow(Overflow::Hidden) => style.visibility = Visibility::Hidden,
                Value::Overflow(Overflow::Visible) => style.visibility = Visibility::Visible,
                Value::Keyword(k) => match k.as_str() {
                    "visible" => style.visibility = Visibility::Visible,
                    "hidden" => style.visibility = Visibility::Hidden,
                    "collapse" => style.visibility = Visibility::Collapse,
                    _ => {}
                },
                _ => {}
            },
            "opacity" => match val {
                Value::Number(n) => style.opacity = (*n).clamp(0.0, 1.0),
                Value::Percentage(pct) => style.opacity = (*pct / 100.0).clamp(0.0, 1.0),
                Value::Length(Length::Px(px)) => style.opacity = (*px).clamp(0.0, 1.0),
                Value::Length(Length::Percent(pct)) => {
                    style.opacity = (*pct / 100.0).clamp(0.0, 1.0)
                }
                _ => {}
            },
            "border-top-left-radius" => match val {
                Value::Length(len) => {
                    style.border_top_left_radius = len
                        .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                        .max(0.0)
                }
                Value::Number(n) => style.border_top_left_radius = (*n).max(0.0),
                _ => {}
            },
            "border-top-right-radius" => match val {
                Value::Length(len) => {
                    style.border_top_right_radius = len
                        .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                        .max(0.0)
                }
                Value::Number(n) => style.border_top_right_radius = (*n).max(0.0),
                _ => {}
            },
            "border-bottom-right-radius" => match val {
                Value::Length(len) => {
                    style.border_bottom_right_radius = len
                        .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                        .max(0.0)
                }
                Value::Number(n) => style.border_bottom_right_radius = (*n).max(0.0),
                _ => {}
            },
            "border-bottom-left-radius" => match val {
                Value::Length(len) => {
                    style.border_bottom_left_radius = len
                        .to_px_with_viewport(style.font_size, style.root_font_size, 0.0, 0.0)
                        .max(0.0)
                }
                Value::Number(n) => style.border_bottom_left_radius = (*n).max(0.0),
                _ => {}
            },
            "white-space" => match val {
                Value::WhiteSpace(ws) => style.white_space = *ws,
                Value::FlexWrap(FlexWrap::NoWrap) => style.white_space = WhiteSpace::Nowrap,
                Value::FlexWrap(FlexWrap::Wrap) => style.white_space = WhiteSpace::Normal,
                Value::Keyword(k) => match k.as_str() {
                    "normal" => style.white_space = WhiteSpace::Normal,
                    "nowrap" => style.white_space = WhiteSpace::Nowrap,
                    "pre" => style.white_space = WhiteSpace::Pre,
                    "pre-wrap" => style.white_space = WhiteSpace::PreWrap,
                    "pre-line" => style.white_space = WhiteSpace::PreLine,
                    "break-spaces" => style.white_space = WhiteSpace::BreakSpaces,
                    _ => {}
                },
                _ => {}
            },
            "text-overflow" => match val {
                Value::TextOverflow(to) => style.text_overflow = *to,
                Value::Keyword(k) => match k.as_str() {
                    "clip" => style.text_overflow = TextOverflow::Clip,
                    "ellipsis" => style.text_overflow = TextOverflow::Ellipsis,
                    _ => {}
                },
                _ => {}
            },
            "list-style-type" => match val {
                Value::ListStyleType(lst) => style.list_style_type = *lst,
                Value::Keyword(k) => match k.as_str() {
                    "disc" => style.list_style_type = ListStyleType::Disc,
                    "circle" => style.list_style_type = ListStyleType::Circle,
                    "square" => style.list_style_type = ListStyleType::Square,
                    "decimal" => style.list_style_type = ListStyleType::Decimal,
                    "lower-alpha" => style.list_style_type = ListStyleType::LowerAlpha,
                    "upper-alpha" => style.list_style_type = ListStyleType::UpperAlpha,
                    "lower-roman" => style.list_style_type = ListStyleType::LowerRoman,
                    "upper-roman" => style.list_style_type = ListStyleType::UpperRoman,
                    "none" => style.list_style_type = ListStyleType::None,
                    _ => {}
                },
                _ => {}
            },
            "list-style-position" => match val {
                Value::ListStylePosition(lsp) => style.list_style_position = *lsp,
                Value::Keyword(k) => match k.as_str() {
                    "outside" => style.list_style_position = ListStylePosition::Outside,
                    "inside" => style.list_style_position = ListStylePosition::Inside,
                    _ => {}
                },
                _ => {}
            },
            "cursor" => match val {
                Value::Cursor(c) => style.cursor = *c,
                Value::Keyword(k) => match k.as_str() {
                    "default" => style.cursor = Cursor::Default,
                    "pointer" => style.cursor = Cursor::Pointer,
                    "text" => style.cursor = Cursor::Text,
                    "not-allowed" => style.cursor = Cursor::NotAllowed,
                    "move" => style.cursor = Cursor::Move,
                    "wait" => style.cursor = Cursor::Wait,
                    "help" => style.cursor = Cursor::Help,
                    _ => {}
                },
                _ => {}
            },
            "vertical-align" => match val {
                Value::VerticalAlign(va) => style.vertical_align = *va,
                Value::Keyword(k) => match k.as_str() {
                    "baseline" => style.vertical_align = VerticalAlign::Baseline,
                    "top" => style.vertical_align = VerticalAlign::Top,
                    "middle" => style.vertical_align = VerticalAlign::Middle,
                    "bottom" => style.vertical_align = VerticalAlign::Bottom,
                    "text-top" => style.vertical_align = VerticalAlign::TextTop,
                    "text-bottom" => style.vertical_align = VerticalAlign::TextBottom,
                    "sub" => style.vertical_align = VerticalAlign::Sub,
                    "super" => style.vertical_align = VerticalAlign::Super,
                    _ => {}
                },
                _ => {}
            },
            "border-collapse" => match val {
                Value::BorderCollapse(bc) => style.border_collapse = *bc,
                Value::Keyword(k) => match k.as_str() {
                    "collapse" => style.border_collapse = BorderCollapse::Collapse,
                    "separate" => style.border_collapse = BorderCollapse::Separate,
                    _ => {}
                },
                _ => {}
            },
            "border-spacing" => match val {
                Value::Length(Length::Px(px)) if *px >= 0.0 => style.border_spacing = *px,
                Value::Number(n) if *n >= 0.0 => style.border_spacing = *n,
                _ => {}
            },
            "table-layout" => match val {
                Value::TableLayout(tl) => style.table_layout = *tl,
                Value::Position(Position::Fixed) => style.table_layout = TableLayout::Fixed,
                Value::Keyword(k) => match k.as_str() {
                    "auto" => style.table_layout = TableLayout::Auto,
                    "fixed" => style.table_layout = TableLayout::Fixed,
                    _ => {}
                },
                _ => {}
            },
            "caption-side" => match val {
                Value::CaptionSide(cs) => style.caption_side = *cs,
                Value::VerticalAlign(VerticalAlign::Top) => style.caption_side = CaptionSide::Top,
                Value::VerticalAlign(VerticalAlign::Bottom) => {
                    style.caption_side = CaptionSide::Bottom
                }
                Value::Keyword(k) => match k.as_str() {
                    "top" => style.caption_side = CaptionSide::Top,
                    "bottom" => style.caption_side = CaptionSide::Bottom,
                    _ => {}
                },
                _ => {}
            },

            "column-count" => match val {
                Value::Number(n) => {
                    if *n >= 1.0 {
                        style.column_count = Some(*n as usize);
                    } else {
                        style.column_count = None;
                    }
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.column_count = None;
                }
                _ => {}
            },
            "column-width" => match val {
                Value::Length(len) => {
                    style.column_width = Some(*len);
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.column_width = None;
                }
                _ => {}
            },
            "column-rule-width" => match val {
                Value::Length(len) => style.column_rule_width = *len,
                Value::Number(n) => style.column_rule_width = Length::Px(*n),
                Value::Keyword(k) => match k.as_str() {
                    "thin" => style.column_rule_width = Length::Px(1.0),
                    "medium" => style.column_rule_width = Length::Px(3.0),
                    "thick" => style.column_rule_width = Length::Px(5.0),
                    _ => {}
                },
                _ => {}
            },
            "column-rule-style" => {
                if let Some(bs) = parse_border_style_value(val) {
                    style.column_rule_style = bs;
                }
            }
            "column-rule-color" => {
                if is_current_color(val) {
                    style.column_rule_color = style.color;
                } else if let Value::Color(c) = val {
                    style.column_rule_color = *c;
                } else if let Value::Keyword(k) = val
                    && let Some(c) = Value::parse_color(k)
                {
                    style.column_rule_color = c;
                }
            }
            "column-span" => match val {
                Value::ColumnSpan(cs) => style.column_span = *cs,
                Value::Keyword(k) if k.eq_ignore_ascii_case("all") => {
                    style.column_span = crate::values::ColumnSpan::All;
                }
                _ => style.column_span = crate::values::ColumnSpan::None,
            },
            "break-inside" | "page-break-inside" => match val {
                Value::BreakInside(bi) => style.break_inside = *bi,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "avoid" => style.break_inside = crate::values::BreakInside::Avoid,
                    "avoid-column" => style.break_inside = crate::values::BreakInside::AvoidColumn,
                    "avoid-page" => style.break_inside = crate::values::BreakInside::AvoidPage,
                    _ => style.break_inside = crate::values::BreakInside::Auto,
                },
                _ => {}
            },
            "contain" => {
                if let Value::Keyword(k) = val {
                    style.contain = match k.to_ascii_lowercase().as_str() {
                        "strict" => crate::values::Contain::Strict,
                        "content" => crate::values::Contain::Content,
                        "paint" => crate::values::Contain::Paint,
                        "layout" => crate::values::Contain::Layout,
                        "size" => crate::values::Contain::Size,
                        "style" => crate::values::Contain::Style,
                        _ => crate::values::Contain::None,
                    };
                }
            }
            "content-visibility" => {
                if let Value::Keyword(k) = val {
                    style.content_visibility = match k.to_ascii_lowercase().as_str() {
                        "auto" => crate::values::ContentVisibility::Auto,
                        "hidden" => crate::values::ContentVisibility::Hidden,
                        _ => crate::values::ContentVisibility::Visible,
                    };
                }
            }
            "writing-mode" => {
                if let Value::Keyword(k) = val {
                    style.writing_mode = match k.to_ascii_lowercase().as_str() {
                        "vertical-rl" => crate::values::WritingMode::VerticalRl,
                        "vertical-lr" => crate::values::WritingMode::VerticalLr,
                        _ => crate::values::WritingMode::HorizontalTb,
                    };
                }
            }
            "resize" => {
                if let Value::Keyword(k) = val {
                    style.resize = match k.to_ascii_lowercase().as_str() {
                        "both" => crate::values::Resize::Both,
                        "horizontal" => crate::values::Resize::Horizontal,
                        "vertical" => crate::values::Resize::Vertical,
                        _ => crate::values::Resize::None,
                    };
                }
            }
            "scrollbar-width" => {
                if let Value::Keyword(k) = val {
                    style.scrollbar_width = match k.to_ascii_lowercase().as_str() {
                        "thin" => crate::values::ScrollbarWidth::Thin,
                        "none" => crate::values::ScrollbarWidth::None,
                        _ => crate::values::ScrollbarWidth::Auto,
                    };
                }
            }
            "scrollbar-color" => match val {
                Value::List(list) if list.len() >= 2 => {
                    let c1 = match &list[0] {
                        Value::Color(c) => Some(*c),
                        Value::CurrentColor => Some(style.color),
                        Value::Keyword(k) if k.eq_ignore_ascii_case("currentcolor") => {
                            Some(style.color)
                        }
                        Value::Keyword(k) => Value::parse_color(k),
                        _ => None,
                    };
                    let c2 = match &list[1] {
                        Value::Color(c) => Some(*c),
                        Value::CurrentColor => Some(style.color),
                        Value::Keyword(k) if k.eq_ignore_ascii_case("currentcolor") => {
                            Some(style.color)
                        }
                        Value::Keyword(k) => Value::parse_color(k),
                        _ => None,
                    };
                    if let (Some(thumb), Some(track)) = (c1, c2) {
                        style.scrollbar_color = Some((thumb, track));
                    }
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.scrollbar_color = None;
                }
                _ => {}
            },

            "outline-width" => {
                let px = match val {
                    Value::Length(len) => len.to_px(style.font_size, 16.0, 0.0),
                    Value::Number(n) => *n,
                    Value::Keyword(k) => match k.as_str() {
                        "thin" => 1.0,
                        "medium" => 3.0,
                        "thick" => 5.0,
                        _ => 0.0,
                    },
                    _ => 0.0,
                };
                style.outline_width = px.max(0.0);
            }
            "outline-style" => {
                if let Some(bs) = parse_border_style_value(val) {
                    style.outline_style = bs;
                }
            }
            "outline-color" => match val {
                Value::Color(c) => style.outline_color = *c,
                Value::CurrentColor => style.outline_color = style.color,
                Value::Keyword(k) => {
                    if k.eq_ignore_ascii_case("invert") {
                        style.outline_color = Color::BLACK;
                    } else if let Some(c) = Value::parse_color(k) {
                        style.outline_color = c;
                    }
                }
                _ => {}
            },
            "outline-offset" => {
                if let Value::Length(len) = val {
                    style.outline_offset = len.to_px(style.font_size, 16.0, 0.0);
                } else if let Value::Number(n) = val {
                    style.outline_offset = *n;
                }
            }

            // CSS Grid properties
            "grid-template-columns" => {
                if let Value::Keyword(k) = val
                    && k.eq_ignore_ascii_case("none")
                {
                    style.grid_template_columns.clear();
                    style.grid_column_lines.clear();
                } else if let Value::GridTrackListWithLines { tracks, lines } = val {
                    style.grid_template_columns = tracks.clone();
                    style.grid_column_lines = lines.clone();
                } else {
                    style.grid_template_columns = value_to_track_list(val);
                }
            }
            "grid-template-rows" => {
                if let Value::Keyword(k) = val
                    && k.eq_ignore_ascii_case("none")
                {
                    style.grid_template_rows.clear();
                    style.grid_row_lines.clear();
                } else if let Value::GridTrackListWithLines { tracks, lines } = val {
                    style.grid_template_rows = tracks.clone();
                    style.grid_row_lines = lines.clone();
                } else {
                    style.grid_template_rows = value_to_track_list(val);
                }
            }
            "grid-auto-flow" => match val {
                Value::GridAutoFlow(flow) => style.grid_auto_flow = *flow,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "column" => style.grid_auto_flow = GridAutoFlow::Column,
                    "column dense" | "dense column" => {
                        style.grid_auto_flow = GridAutoFlow::ColumnDense
                    }
                    "row dense" | "dense row" | "dense" => {
                        style.grid_auto_flow = GridAutoFlow::RowDense
                    }
                    _ => style.grid_auto_flow = GridAutoFlow::Row,
                },
                _ => {}
            },
            "grid-template-areas" => {
                if let Value::Keyword(k) = val
                    && k.eq_ignore_ascii_case("none")
                {
                    style.grid_template_areas.clear();
                } else {
                    style.grid_template_areas = value_to_template_areas(val);
                }
            }
            "grid-column-start" => {
                style.grid_column_start = value_to_placement(val);
            }
            "grid-column-end" => {
                style.grid_column_end = value_to_placement(val);
            }
            "grid-row-start" => {
                style.grid_row_start = value_to_placement(val);
            }
            "grid-row-end" => {
                style.grid_row_end = value_to_placement(val);
            }
            "box-shadow" => match val {
                Value::BoxShadow(bs) => style.box_shadow = Some(*bs),
                Value::List(items) => style.box_shadow = parse_box_shadow_from_values(items),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => style.box_shadow = None,
                _ => {}
            },
            "content" => match val {
                Value::Content(items) => {
                    if items.is_empty() {
                        style.content = None;
                        style.content_items = Some(Vec::new());
                    } else {
                        let mut static_text = String::new();
                        for it in items {
                            if let crate::values::ContentItem::String(s) = it {
                                static_text.push_str(s);
                            }
                        }
                        style.content = if static_text.is_empty() {
                            None
                        } else {
                            Some(static_text)
                        };
                        style.content_items = Some(items.clone());
                    }
                }
                Value::String(s) => {
                    style.content = Some(s.clone());
                    style.content_items = Some(vec![crate::values::ContentItem::String(s.clone())]);
                }
                Value::Keyword(k)
                    if k.eq_ignore_ascii_case("none") || k.eq_ignore_ascii_case("normal") =>
                {
                    style.content = None;
                    style.content_items = Some(Vec::new());
                }
                _ => {}
            },
            "counter-reset" => {
                if let Value::CounterActions(actions) = val {
                    style.counter_reset = actions.clone();
                }
            }
            "counter-increment" => {
                if let Value::CounterActions(actions) = val {
                    style.counter_increment = actions.clone();
                }
            }
            "quotes" => match val {
                Value::Quotes(pairs) => {
                    style.quotes = Some(pairs.clone());
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.quotes = Some(Vec::new());
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.quotes = None;
                }
                _ => {}
            },
            "container-type" => match val {
                Value::ContainerType(ct) => style.container_type = *ct,
                Value::Keyword(k) | Value::String(k) => {
                    if let Some(ct) = crate::values::ContainerType::parse(k) {
                        style.container_type = ct;
                    }
                }
                _ => {}
            },
            "container-name" => match val {
                Value::String(s) | Value::Keyword(s) => {
                    if s.eq_ignore_ascii_case("none") {
                        style.container_name = None;
                    } else {
                        style.container_name = Some(s.clone());
                    }
                }
                _ => {}
            },

            // Phase 7 CSS properties
            "transform" => match val {
                Value::Transform(t) => style.transform = t.clone(),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.transform = crate::values::Transform::default();
                }
                _ => {}
            },
            "transform-origin-x" => match val {
                Value::Length(l) => style.transform_origin_x = *l,
                Value::Float(Float::Left) => style.transform_origin_x = Length::Percent(0.0),
                Value::Float(Float::Right) => style.transform_origin_x = Length::Percent(100.0),
                Value::TextAlign(TextAlign::Center) => {
                    style.transform_origin_x = Length::Percent(50.0)
                }
                Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                    "left" => style.transform_origin_x = Length::Percent(0.0),
                    "center" => style.transform_origin_x = Length::Percent(50.0),
                    "right" => style.transform_origin_x = Length::Percent(100.0),
                    _ => {}
                },
                _ => {}
            },
            "transform-origin-y" => match val {
                Value::Length(l) => style.transform_origin_y = *l,
                Value::TextAlign(TextAlign::Center) => {
                    style.transform_origin_y = Length::Percent(50.0)
                }
                Value::VerticalAlign(VerticalAlign::Top) => {
                    style.transform_origin_y = Length::Percent(0.0)
                }
                Value::VerticalAlign(VerticalAlign::Bottom) => {
                    style.transform_origin_y = Length::Percent(100.0)
                }
                Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                    "top" => style.transform_origin_y = Length::Percent(0.0),
                    "center" => style.transform_origin_y = Length::Percent(50.0),
                    "bottom" => style.transform_origin_y = Length::Percent(100.0),
                    _ => {}
                },
                _ => {}
            },
            "transform-origin-z" => match val {
                Value::Length(l) => style.transform_origin_z = *l,
                Value::Number(n) if *n == 0.0 => style.transform_origin_z = Length::Px(0.0),
                _ => {}
            },
            "transform-origin" => {
                let items: Vec<Value> = match val {
                    Value::List(items) => items.clone(),
                    single => vec![single.clone()],
                };

                let parse_x = |v: &Value| -> Option<Length> {
                    match v {
                        Value::Length(l) => Some(*l),
                        other => match get_keyword_str(other)?.to_ascii_lowercase().as_str() {
                            "left" => Some(Length::Percent(0.0)),
                            "center" => Some(Length::Percent(50.0)),
                            "right" => Some(Length::Percent(100.0)),
                            _ => None,
                        },
                    }
                };

                let parse_y = |v: &Value| -> Option<Length> {
                    match v {
                        Value::Length(l) => Some(*l),
                        other => match get_keyword_str(other)?.to_ascii_lowercase().as_str() {
                            "top" => Some(Length::Percent(0.0)),
                            "center" => Some(Length::Percent(50.0)),
                            "bottom" => Some(Length::Percent(100.0)),
                            _ => None,
                        },
                    }
                };

                let is_y_keyword = |v: &Value| -> bool {
                    if let Some(kw) = get_keyword_str(v) {
                        let s = kw.to_ascii_lowercase();
                        s == "top" || s == "bottom"
                    } else {
                        false
                    }
                };

                let is_x_keyword = |v: &Value| -> bool {
                    if let Some(kw) = get_keyword_str(v) {
                        let s = kw.to_ascii_lowercase();
                        s == "left" || s == "right"
                    } else {
                        false
                    }
                };

                match items.len() {
                    1 => {
                        let item = &items[0];
                        if is_y_keyword(item) {
                            style.transform_origin_x = Length::Percent(50.0);
                            if let Some(y) = parse_y(item) {
                                style.transform_origin_y = y;
                            }
                        } else {
                            if let Some(x) = parse_x(item) {
                                style.transform_origin_x = x;
                            }
                            style.transform_origin_y = Length::Percent(50.0);
                        }
                        style.transform_origin_z = Length::Px(0.0);
                    }
                    2 => {
                        let (item1, item2) = (&items[0], &items[1]);
                        if is_y_keyword(item1) || is_x_keyword(item2) {
                            if let Some(y) = parse_y(item1) {
                                style.transform_origin_y = y;
                            }
                            if let Some(x) = parse_x(item2) {
                                style.transform_origin_x = x;
                            }
                        } else {
                            if let Some(x) = parse_x(item1) {
                                style.transform_origin_x = x;
                            }
                            if let Some(y) = parse_y(item2) {
                                style.transform_origin_y = y;
                            }
                        }
                        style.transform_origin_z = Length::Px(0.0);
                    }
                    3.. => {
                        let (item1, item2, item3) = (&items[0], &items[1], &items[2]);
                        if is_y_keyword(item1) || is_x_keyword(item2) {
                            if let Some(y) = parse_y(item1) {
                                style.transform_origin_y = y;
                            }
                            if let Some(x) = parse_x(item2) {
                                style.transform_origin_x = x;
                            }
                        } else {
                            if let Some(x) = parse_x(item1) {
                                style.transform_origin_x = x;
                            }
                            if let Some(y) = parse_y(item2) {
                                style.transform_origin_y = y;
                            }
                        }
                        if let Value::Length(z) = item3 {
                            style.transform_origin_z = *z;
                        } else if let Value::Number(n) = item3
                            && *n == 0.0
                        {
                            style.transform_origin_z = Length::Px(0.0);
                        }
                    }
                    _ => {}
                }
            }
            "transition-property"
            | "transition-duration"
            | "transition-timing-function"
            | "transition-delay" => {
                apply_transition_longhand(style, prop, val);
            }
            "animation-name"
            | "animation-duration"
            | "animation-timing-function"
            | "animation-delay"
            | "animation-iteration-count"
            | "animation-direction"
            | "animation-fill-mode"
            | "animation-play-state" => {
                apply_animation_longhand(style, prop, val);
            }
            "will-change" => match val {
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.will_change = false;
                    style.will_change_properties.clear();
                }
                Value::Keyword(k) => {
                    style.will_change = true;
                    style.will_change_properties = vec![k.clone()];
                }
                Value::List(items) => {
                    let mut props = Vec::new();
                    for item in items {
                        if let Value::Keyword(k) = item
                            && !k.eq_ignore_ascii_case("auto")
                        {
                            props.push(k.clone());
                        }
                    }
                    style.will_change = !props.is_empty();
                    style.will_change_properties = props;
                }
                _ => {
                    style.will_change = true;
                }
            },
            "text-shadow" => match val {
                Value::TextShadow(ts) => style.text_shadow = Some(*ts),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => style.text_shadow = None,
                _ => {}
            },
            "filter" | "-webkit-filter" => match val {
                Value::Filter(fns) => style.filter = fns.clone(),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => style.filter.clear(),
                _ => {}
            },
            "backdrop-filter" | "-webkit-backdrop-filter" => match val {
                Value::Filter(fns) => style.backdrop_filter = fns.clone(),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.backdrop_filter.clear()
                }
                _ => {}
            },
            "mix-blend-mode" => match val {
                Value::BlendMode(bm) => style.mix_blend_mode = *bm,
                Value::Keyword(k) => {
                    if let Some(bm) = crate::values::BlendMode::parse(k) {
                        style.mix_blend_mode = bm;
                    }
                }
                _ => {}
            },
            "background-blend-mode" => match val {
                Value::BlendMode(bm) => style.background_blend_mode = *bm,
                Value::Keyword(k) => {
                    if let Some(bm) = crate::values::BlendMode::parse(k) {
                        style.background_blend_mode = bm;
                    }
                }
                _ => {}
            },
            "clip-path" | "-webkit-clip-path" => match val {
                Value::ClipPath(cp) => style.clip_path = cp.clone(),
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.clip_path = crate::values::ClipPath::None
                }
                _ => {}
            },
            "mask-mode" | "-webkit-mask-mode" => match val {
                Value::MaskMode(mm) => style.mask_mode = *mm,
                Value::Keyword(k) => {
                    if let Some(mm) = crate::values::MaskMode::parse(k) {
                        style.mask_mode = mm;
                    }
                }
                _ => {}
            },
            "aspect-ratio" => match val {
                Value::Number(n) if *n > 0.0 => style.aspect_ratio = Some(*n),
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => style.aspect_ratio = None,
                Value::List(items) if items.len() >= 2 => {
                    // e.g. "16 / 9" → items [Number(16), Number(9)]
                    if let (Value::Number(w), Value::Number(h)) =
                        (&items[0], &items[items.len() - 1])
                        && *h > 0.0
                    {
                        style.aspect_ratio = Some(*w / *h);
                    }
                }
                _ => {}
            },
            "object-fit" => match val {
                Value::ObjectFit(of) => style.object_fit = *of,
                Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                    "fill" => style.object_fit = crate::values::ObjectFit::Fill,
                    "contain" => style.object_fit = crate::values::ObjectFit::Contain,
                    "cover" => style.object_fit = crate::values::ObjectFit::Cover,
                    "none" => style.object_fit = crate::values::ObjectFit::None,
                    "scale-down" => style.object_fit = crate::values::ObjectFit::ScaleDown,
                    _ => {}
                },
                _ => {}
            },
            "object-position" => match val {
                Value::List(items) if items.len() >= 2 => {
                    if let (Value::Length(x), Value::Length(y)) = (&items[0], &items[1]) {
                        style.object_position = (*x, *y);
                    }
                }
                Value::Length(l) => {
                    style.object_position = (*l, *l);
                }
                _ => {}
            },
            "word-break" => match val {
                Value::WordBreak(wb) => style.word_break = *wb,
                Value::Keyword(k) => {
                    if let Some(wb) = crate::values::WordBreak::parse(k) {
                        style.word_break = wb;
                    }
                }
                _ => {}
            },
            "overflow-wrap" | "word-wrap" => match val {
                Value::OverflowWrap(ow) => style.overflow_wrap = *ow,
                Value::Keyword(k) => {
                    if let Some(ow) = crate::values::OverflowWrap::parse(k) {
                        style.overflow_wrap = ow;
                    }
                }
                _ => {}
            },
            "hyphens" => match val {
                Value::Hyphens(h) => style.hyphens = *h,
                Value::Keyword(k) => {
                    if let Some(h) = crate::values::Hyphens::parse(k) {
                        style.hyphens = h;
                    }
                }
                _ => {}
            },
            "line-clamp" | "-webkit-line-clamp" => match val {
                Value::LineClamp(lc) => style.line_clamp = *lc,
                Value::Number(n) if *n >= 1.0 => {
                    style.line_clamp = crate::values::LineClamp::Lines(*n as u32)
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                    style.line_clamp = crate::values::LineClamp::None
                }
                Value::Keyword(k) => {
                    if let Some(lc) = crate::values::LineClamp::parse(k) {
                        style.line_clamp = lc;
                    }
                }
                _ => {}
            },
            "text-underline-offset" => match val {
                Value::Length(l) => style.text_underline_offset = *l,
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.text_underline_offset = Length::Auto
                }
                _ => {}
            },
            "text-decoration-thickness" => match val {
                Value::TextDecorationThickness(tdt) => {
                    style.text_decoration_thickness = tdt.clone()
                }
                Value::Length(l) => {
                    style.text_decoration_thickness =
                        crate::values::TextDecorationThickness::Length(*l)
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                    style.text_decoration_thickness = crate::values::TextDecorationThickness::Auto
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("from-font") => {
                    style.text_decoration_thickness =
                        crate::values::TextDecorationThickness::FromFont
                }
                _ => {}
            },
            "text-emphasis-style" => match val {
                Value::TextEmphasisStyle(tes) => style.text_emphasis_style = tes.clone(),
                Value::Keyword(k) | Value::String(k) => {
                    if let Some(tes) = crate::values::TextEmphasisStyle::parse(k) {
                        style.text_emphasis_style = tes;
                    }
                }
                _ => {}
            },
            "text-emphasis-color" => match val {
                Value::Color(c) => style.text_emphasis_color = Some(*c),
                Value::Keyword(k) if k.eq_ignore_ascii_case("currentcolor") => {
                    style.text_emphasis_color = None
                }
                Value::Keyword(k) => {
                    if let Some(c) = Value::parse_color(k) {
                        style.text_emphasis_color = Some(c);
                    }
                }
                _ => {}
            },
            "font-feature-settings" => match val {
                Value::FontFeatureSettings(ffs) => style.font_feature_settings = ffs.clone(),
                Value::Keyword(k) if k.eq_ignore_ascii_case("normal") => {
                    style.font_feature_settings = crate::values::FontFeatureSettings::Normal
                }
                Value::Keyword(k) | Value::String(k) => {
                    if let Some(ffs) = crate::values::FontFeatureSettings::parse(k) {
                        style.font_feature_settings = ffs;
                    }
                }
                _ => {}
            },
            "font-variation-settings" => match val {
                Value::FontVariationSettings(fvs) => style.font_variation_settings = fvs.clone(),
                Value::Keyword(k) if k.eq_ignore_ascii_case("normal") => {
                    style.font_variation_settings = crate::values::FontVariationSettings::Normal
                }
                Value::Keyword(k) | Value::String(k) => {
                    if let Some(fvs) = crate::values::FontVariationSettings::parse(k) {
                        style.font_variation_settings = fvs;
                    }
                }
                _ => {}
            },
            "font-display" => match val {
                Value::FontDisplay(fd) => style.font_display = *fd,
                Value::Keyword(k) => {
                    if let Some(fd) = crate::values::FontDisplay::parse(k) {
                        style.font_display = fd;
                    }
                }
                _ => {}
            },
            "accent-color" => {
                if is_current_color(val) {
                    style.accent_color = Some(style.color);
                } else if let Value::Color(c) = val {
                    style.accent_color = Some(*c);
                } else if let Value::Keyword(k) | Value::String(k) = val {
                    if k.eq_ignore_ascii_case("auto") {
                        style.accent_color = None;
                    } else if let Some(c) = Value::parse_color(k) {
                        style.accent_color = Some(c);
                    }
                }
            }
            // Silently accept but ignore vendor-prefixed and future properties
            _ => {}
        }
    }

    // CSS 2.1 § 9.7: If position is absolute/fixed, or float is not none, display is blockified.
    let is_out_of_flow = matches!(style.position, Position::Absolute | Position::Fixed);
    let is_floating = style.float != Float::None;
    if is_out_of_flow || is_floating {
        style.display = match style.display {
            Display::Inline | Display::InlineBlock => Display::Block,
            Display::InlineFlex => Display::Flex,
            Display::InlineGrid => Display::Grid,
            other => other,
        };
    }

    // If border width was explicitly given but border style was not specified, default to Solid
    if !resolved_cascaded.contains_key("border-top-style") && style.border_top_width > 0.0 {
        style.border_top_style = BorderStyle::Solid;
    }
    if !resolved_cascaded.contains_key("border-right-style") && style.border_right_width > 0.0 {
        style.border_right_style = BorderStyle::Solid;
    }
    if !resolved_cascaded.contains_key("border-bottom-style") && style.border_bottom_width > 0.0 {
        style.border_bottom_style = BorderStyle::Solid;
    }
    if !resolved_cascaded.contains_key("border-left-style") && style.border_left_width > 0.0 {
        style.border_left_style = BorderStyle::Solid;
    }

    // CSS 2.1 § 8.5.1: If the border style is 'none' or 'hidden', the computed border width is zero.
    if matches!(
        style.border_top_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_top_width = 0.0;
    }
    if matches!(
        style.border_right_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_right_width = 0.0;
    }
    if matches!(
        style.border_bottom_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_bottom_width = 0.0;
    }
    if matches!(
        style.border_left_style,
        BorderStyle::None | BorderStyle::Hidden
    ) {
        style.border_left_width = 0.0;
    }
}

/// Builds up `style.transitions` from one `transition-*` longhand declaration.
///
/// Longhands may be applied in any order, so each one independently grows the
/// transition list as needed and only writes its own field.
fn apply_transition_longhand(style: &mut ComputedStyle, prop: &str, val: &Value) {
    use crate::values::Transition;

    let values: Vec<Value> = match val {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    if values.is_empty() {
        return;
    }
    if style.transitions.len() < values.len() {
        style
            .transitions
            .resize(values.len(), Transition::default());
    }

    let time_ms = |v: &Value| -> Option<f32> {
        match v {
            Value::Time(ms) => Some(*ms),
            Value::Number(n) => Some(*n),
            _ => None,
        }
    };

    for (i, value) in values.iter().enumerate() {
        // CSS repeats the shorter lists to match the longest one.
        let idx = if i < style.transitions.len() {
            i
        } else {
            i % style.transitions.len()
        };
        let entry = &mut style.transitions[idx];
        match prop {
            "transition-property" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.property = k.to_ascii_lowercase();
                }
            }
            "transition-duration" => {
                if let Some(ms) = time_ms(value) {
                    entry.duration_ms = ms;
                }
            }
            "transition-delay" => {
                if let Some(ms) = time_ms(value) {
                    entry.delay_ms = ms;
                }
            }
            "transition-timing-function" => {
                if let Value::TimingFunction(tf) = value {
                    entry.timing = *tf;
                } else if let Value::Keyword(k) | Value::String(k) = value
                    && let Some(tf) = crate::values::parse_timing_function(k)
                {
                    entry.timing = tf;
                }
            }
            _ => {}
        }
    }
}

/// Builds up `style.animations` from one `animation-*` longhand declaration.
fn apply_animation_longhand(style: &mut ComputedStyle, prop: &str, val: &Value) {
    use crate::values::{
        Animation, AnimationDirection, AnimationFillMode, AnimationIterationCount,
        AnimationPlayState,
    };

    // `animation: none` clears the animation list entirely.
    if prop == "animation-name"
        && let Value::Keyword(k) = val
        && k.eq_ignore_ascii_case("none")
    {
        style.animations.clear();
        return;
    }

    let values: Vec<Value> = match val {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    if values.is_empty() {
        return;
    }
    if style.animations.len() < values.len() {
        style.animations.resize(values.len(), Animation::default());
    }

    for (i, value) in values.iter().enumerate() {
        let idx = if i < style.animations.len() {
            i
        } else {
            i % style.animations.len()
        };
        let entry = &mut style.animations[idx];
        match prop {
            "animation-name" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.name = k.clone();
                }
            }
            "animation-duration" => match value {
                Value::Time(ms) => entry.duration_ms = *ms,
                Value::Number(n) => entry.duration_ms = *n,
                _ => {}
            },
            "animation-delay" => match value {
                Value::Time(ms) => entry.delay_ms = *ms,
                Value::Number(n) => entry.delay_ms = *n,
                _ => {}
            },
            "animation-timing-function" => {
                if let Value::TimingFunction(tf) = value {
                    entry.timing = *tf;
                } else if let Value::Keyword(k) | Value::String(k) = value
                    && let Some(tf) = crate::values::parse_timing_function(k)
                {
                    entry.timing = tf;
                }
            }
            "animation-iteration-count" => match value {
                Value::Number(n) => entry.iteration_count = AnimationIterationCount::Finite(*n),
                Value::Keyword(k) if k.eq_ignore_ascii_case("infinite") => {
                    entry.iteration_count = AnimationIterationCount::Infinite
                }
                _ => {}
            },
            "animation-direction" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.direction = match k.to_ascii_lowercase().as_str() {
                        "reverse" => AnimationDirection::Reverse,
                        "alternate" => AnimationDirection::Alternate,
                        "alternate-reverse" => AnimationDirection::AlternateReverse,
                        _ => AnimationDirection::Normal,
                    };
                }
            }
            "animation-fill-mode" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.fill_mode = match k.to_ascii_lowercase().as_str() {
                        "forwards" => AnimationFillMode::Forwards,
                        "backwards" => AnimationFillMode::Backwards,
                        "both" => AnimationFillMode::Both,
                        _ => AnimationFillMode::None,
                    };
                }
            }
            "animation-play-state" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.play_state = if k.eq_ignore_ascii_case("paused") {
                        AnimationPlayState::Paused
                    } else {
                        AnimationPlayState::Running
                    };
                }
            }
            _ => {}
        }
    }
}

fn parse_border_style_value(val: &Value) -> Option<BorderStyle> {
    match val {
        Value::BorderStyle(s) => Some(*s),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "none" => Some(BorderStyle::None),
            "hidden" => Some(BorderStyle::Hidden),
            "solid" | "groove" | "ridge" | "inset" | "outset" => Some(BorderStyle::Solid),
            "dashed" => Some(BorderStyle::Dashed),
            "dotted" => Some(BorderStyle::Dotted),
            "double" => Some(BorderStyle::Double),
            _ => None,
        },
        Value::Display(Display::None) => Some(BorderStyle::None),
        _ => None,
    }
}

fn parse_box_shadow_from_values(items: &[Value]) -> Option<BoxShadow> {
    let mut offset_x = None;
    let mut offset_y = None;
    let mut blur_radius = 0.0f32;
    let mut spread_radius = 0.0f32;
    let mut color = Color::rgba(0, 0, 0, 255);
    let mut inset = false;
    let mut len_count = 0;

    for item in items {
        match item {
            Value::Keyword(k) if k.eq_ignore_ascii_case("inset") => inset = true,
            Value::Length(len) => {
                let px = match len {
                    Length::Px(p) => *p,
                    Length::Em(e) => *e * 16.0,
                    Length::Rem(r) => *r * 16.0,
                    Length::Calc(c) => c.px,
                    _ => 0.0,
                };
                match len_count {
                    0 => offset_x = Some(px),
                    1 => offset_y = Some(px),
                    2 => blur_radius = px,
                    3 => spread_radius = px,
                    _ => {}
                }
                len_count += 1;
            }
            Value::Number(n) => {
                let px = *n;
                match len_count {
                    0 => offset_x = Some(px),
                    1 => offset_y = Some(px),
                    2 => blur_radius = px,
                    3 => spread_radius = px,
                    _ => {}
                }
                len_count += 1;
            }
            Value::Color(c) => color = *c,
            Value::Var { fallback, .. } => {
                if let Some(fb) = fallback
                    && let Value::Color(c) = &**fb
                {
                    color = *c;
                }
            }
            _ => {}
        }
    }

    if let (Some(ox), Some(oy)) = (offset_x, offset_y) {
        Some(BoxShadow {
            offset_x: ox,
            offset_y: oy,
            blur_radius,
            spread_radius,
            color,
            inset,
        })
    } else {
        None
    }
}

fn value_to_track_size(v: &Value) -> Option<GridTrackSize> {
    match v {
        Value::Length(l) => Some(GridTrackSize::Length(*l)),
        Value::Fr(f) => Some(GridTrackSize::Fr(*f)),
        Value::GridTrackList(list) if list.len() == 1 => Some(list[0].clone()),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "auto" => Some(GridTrackSize::Auto),
            "min-content" => Some(GridTrackSize::MinContent),
            "max-content" => Some(GridTrackSize::MaxContent),
            "subgrid" => Some(GridTrackSize::Subgrid),
            _ => None,
        },
        _ => None,
    }
}

fn value_to_track_list(v: &Value) -> Vec<GridTrackSize> {
    match v {
        Value::GridTrackList(list) => list.clone(),
        Value::GridTrackListWithLines { tracks, .. } => tracks.clone(),
        Value::List(items) => items.iter().filter_map(value_to_track_size).collect(),
        single => value_to_track_size(single).into_iter().collect(),
    }
}

fn value_to_template_areas(v: &Value) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    match v {
        Value::List(items) => {
            for item in items {
                if let Value::String(s) = item {
                    let cells: Vec<String> = s
                        .split_whitespace()
                        .map(|w| w.to_ascii_lowercase())
                        .collect();
                    if !cells.is_empty() {
                        rows.push(cells);
                    }
                }
            }
        }
        Value::String(s) => {
            for line in s.lines() {
                let cells: Vec<String> = line
                    .split_whitespace()
                    .map(|w| w.to_ascii_lowercase())
                    .collect();
                if !cells.is_empty() {
                    rows.push(cells);
                }
            }
        }
        _ => {}
    }
    rows
}

fn value_to_placement(v: &Value) -> GridPlacement {
    match v {
        Value::GridPlacement(gp) => gp.clone(),
        Value::Number(n) => GridPlacement::Line(*n as i32),
        Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => GridPlacement::Auto,
        Value::Keyword(k) => GridPlacement::Area(k.to_ascii_lowercase()),
        Value::String(s) => GridPlacement::Area(s.to_ascii_lowercase()),
        _ => GridPlacement::Auto,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_html::parse_html;

    #[test]
    fn test_computed_style_inheritance() {
        let html = r#"<div style="color: red; font-size: 20px;"><p>Inherited</p></div>"#;
        let doc = parse_html(html);
        let root = doc.root();

        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.color, Color::RED);
        assert_eq!(div_style.font_size, 20.0);

        // p should inherit color: red and font_size: 20px from div
        let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
        assert_eq!(p_style.color, Color::RED);
        assert_eq!(p_style.font_size, 20.0);
        // But margin is not inherited (p gets UA margin, not div's)
        assert_ne!(p_style.margin_top, Length::Auto);
    }

    #[test]
    fn test_computed_style_em_resolution() {
        let html = r#"<div style="font-size: 20px;"><h1 style="font-size: 2em;">Title</h1></div>"#;
        let doc = parse_html(html);
        let root = doc.root();

        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let h1_id = doc.find_element_by_tag(root, "h1").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.font_size, 20.0);

        let h1_style = compute_style(h1_id, &doc, &[], Some(&div_style));
        // 2em * 20px = 40px
        assert_eq!(h1_style.font_size, 40.0);
    }

    #[test]
    fn test_custom_property_basic_var_resolution() {
        let html = r#"<div style="--theme-color: #336699; --base-size: 24px;"><p style="color: var(--theme-color); font-size: var(--base-size);">Hello</p></div>"#;
        let doc = parse_html(html);
        let root = doc.root();

        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        let p_style = compute_style(p_id, &doc, &[], Some(&div_style));

        assert_eq!(p_style.color, Color::rgb(0x33, 0x66, 0x99));
        assert_eq!(p_style.font_size, 24.0);
    }

    #[test]
    fn test_custom_property_fallback() {
        let html = r#"<p style="color: var(--non-existent, #00ff00); font-size: var(--missing-size, 18px);">Fallback</p>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let p_style = compute_style(p_id, &doc, &[], None);
        assert_eq!(p_style.color, Color::rgb(0, 255, 0));
        assert_eq!(p_style.font_size, 18.0);
    }

    #[test]
    fn test_custom_property_nested_var() {
        let html = r#"<div style="--level2: #ffa136;"><p style="color: var(--level1, var(--level2, #000000));">Nested</p></div>"#;
        let doc = parse_html(html);
        let root = doc.root();

        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        let p_style = compute_style(p_id, &doc, &[], Some(&div_style));

        assert_eq!(p_style.color, Color::rgb(255, 161, 54));
    }

    #[test]
    fn test_custom_property_cycle_detection() {
        let html =
            r#"<p style="--a: var(--b); --b: var(--a); color: var(--a, #ff0000);">Cycle</p>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let p_style = compute_style(p_id, &doc, &[], None);
        assert_eq!(p_style.color, Color::RED);
    }

    #[test]
    fn test_custom_property_in_border_shorthand() {
        let html = r#"<div style="--border-color: #202122; border: 2px solid var(--border-color);">Box</div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.border_top_width, 2.0);
        assert_eq!(div_style.border_top_style, BorderStyle::Solid);
        assert_eq!(div_style.border_top_color, Color::rgb(0x20, 0x21, 0x22));
    }

    #[test]
    fn test_background_image_and_repeat_and_size() {
        let html = r#"<div style="background-image: url('logo.png'); background-repeat: no-repeat; background-size: cover;">Logo</div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.background_image.as_deref(), Some("logo.png"));
        assert_eq!(div_style.background_repeat, BackgroundRepeat::NoRepeat);
        assert_eq!(div_style.background_size, BackgroundSize::Cover);
    }

    #[test]
    fn test_background_shorthand_expansion() {
        let html = r#"<div style="background: #ffffff url('/static/images/wiki.png') no-repeat;">Wiki</div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.background_color, Color::WHITE);
        assert_eq!(
            div_style.background_image.as_deref(),
            Some("/static/images/wiki.png")
        );
        assert_eq!(div_style.background_repeat, BackgroundRepeat::NoRepeat);
    }

    #[test]
    fn test_letter_spacing_word_spacing_text_indent() {
        let html =
            r#"<div style="letter-spacing: 2px; word-spacing: 4px; text-indent: 20px;">Text</div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.letter_spacing, Length::Px(2.0));
        assert_eq!(div_style.word_spacing, Length::Px(4.0));
        assert_eq!(div_style.text_indent, Length::Px(20.0));
    }

    #[test]
    fn test_spacing_and_indent_inheritance() {
        let html = r#"<div style="letter-spacing: 3px; word-spacing: 5px; text-indent: 1.5em;"><p>Child</p></div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
        assert_eq!(p_style.letter_spacing, Length::Px(3.0));
        assert_eq!(p_style.word_spacing, Length::Px(5.0));
        assert_eq!(p_style.text_indent, Length::Em(1.5));
    }

    #[test]
    fn test_accent_color_and_current_color() {
        let html = r#"<div style="color: #ff5500; accent-color: currentcolor; background-color: currentcolor; border-top-color: currentcolor; outline-color: currentcolor;"><p style="accent-color: #00aaee;">Child</p></div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        let div_style = compute_style(div_id, &doc, &[], None);
        assert_eq!(div_style.color, Color::rgb(255, 85, 0));
        assert_eq!(div_style.accent_color, Some(Color::rgb(255, 85, 0)));
        assert_eq!(div_style.background_color, Color::rgb(255, 85, 0));
        assert_eq!(div_style.border_top_color, Color::rgb(255, 85, 0));
        assert_eq!(div_style.outline_color, Color::rgb(255, 85, 0));

        let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
        assert_eq!(p_style.accent_color, Some(Color::rgb(0, 170, 238)));

        // Test auto resets to None
        let auto_html = r#"<div style="accent-color: auto;">Auto</div>"#;
        let auto_doc = parse_html(auto_html);
        let auto_div = auto_doc
            .find_element_by_tag(auto_doc.root(), "div")
            .unwrap();
        let auto_style = compute_style(auto_div, &auto_doc, &[], None);
        assert_eq!(auto_style.accent_color, None);
    }
}
