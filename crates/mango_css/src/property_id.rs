//! Typed CSS property identifier system (ARCH-007).
//!
//! Provides a strongly-typed `CssPropertyId` enum covering all CSS properties
//! supported by the Mango engine. Using enum discriminants instead of string
//! matching enables:
//! - Compile-time exhaustiveness checking in `match` arms.
//! - Faster O(1) property lookups via integer discriminants.
//! - Reduced heap allocations (no string comparisons in hot paths).
//!
//! # Example
//! ```
//! use mango_css::property_id::CssPropertyId;
//!
//! let id = CssPropertyId::from_name("background-color");
//! assert_eq!(id, Some(CssPropertyId::BackgroundColor));
//! assert_eq!(CssPropertyId::BackgroundColor.name(), "background-color");
//! assert!(CssPropertyId::BackgroundColor.is_inherited() == false);
//! ```

/// Strongly-typed identifier for every CSS property supported by the Mango engine.
///
/// Each variant maps 1:1 to a field on [`super::computed::ComputedStyle`] and can
/// be converted to/from the canonical CSS property name string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum CssPropertyId {
    // ── Layout & Display ──
    Display = 0,
    Position,
    Float,
    Clear,
    BoxSizing,
    Width,
    Height,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    Top,
    Right,
    Bottom,
    Left,
    ZIndex,
    Overflow,
    OverflowX,
    OverflowY,
    Visibility,
    Opacity,
    VerticalAlign,
    Order,

    // ── Border ──
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderTopColor,
    BorderRightColor,
    BorderBottomColor,
    BorderLeftColor,
    BorderTopStyle,
    BorderRightStyle,
    BorderBottomStyle,
    BorderLeftStyle,
    BorderTopLeftRadius,
    BorderTopRightRadius,
    BorderBottomRightRadius,
    BorderBottomLeftRadius,
    BorderCollapse,
    BorderSpacing,
    TableLayout,
    CaptionSide,

    // ── Outline ──
    OutlineWidth,
    OutlineStyle,
    OutlineColor,
    OutlineOffset,

    // ── Color & Background ──
    Color,
    BackgroundColor,
    BackgroundImage,
    BackgroundRepeat,
    BackgroundSize,
    BackgroundPosition,
    BackgroundGradient,
    BoxShadow,
    MaskImage,
    WebkitMaskImage,
    MaskSize,
    WebkitMaskSize,
    MaskRepeat,
    WebkitMaskRepeat,
    MaskPosition,
    WebkitMaskPosition,
    BackgroundAttachment,
    BackgroundClip,
    WebkitBackgroundClip,
    BorderImage,
    BorderImageSource,
    BorderImageSlice,
    BorderImageWidth,
    BorderImageOutset,
    BorderImageRepeat,
    MaskComposite,
    WebkitMaskComposite,
    Isolation,

    // ── Typography ──
    FontSize,
    FontWeight,
    FontStyle,
    FontFamily,
    LineHeight,
    TextAlign,
    TextDecoration,
    TextTransform,
    LetterSpacing,
    WordSpacing,
    TextIndent,
    WhiteSpace,
    TextOverflow,
    TextShadow,
    Direction,
    UnicodeBidi,
    WordBreak,
    OverflowWrap,
    Hyphens,
    TextUnderlineOffset,
    TextDecorationThickness,
    TextEmphasis,
    TextEmphasisStyle,
    TextEmphasisColor,
    FontFeatureSettings,
    FontVariationSettings,
    LineClamp,
    FontDisplay,

    // ── List ──
    ListStyleType,
    ListStylePosition,
    Content,
    Cursor,

    // ── Flexbox ──
    FlexDirection,
    FlexWrap,
    JustifyContent,
    AlignItems,
    AlignSelf,
    AlignContent,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    RowGap,
    ColumnGap,

    // ── Grid ──
    GridTemplateColumns,
    GridTemplateRows,
    GridTemplateAreas,
    GridColumnStart,
    GridColumnEnd,
    GridRowStart,
    GridRowEnd,
    GridAutoFlow,

    // ── Transforms & Animations ──
    Transform,
    TransformOriginX,
    TransformOriginY,
    TransformOriginZ,
    Filter,
    BackdropFilter,
    WebkitBackdropFilter,
    MixBlendMode,
    BackgroundBlendMode,
    ClipPath,
    WebkitClipPath,
    MaskMode,
    WebkitMaskMode,
    AspectRatio,
    ObjectFit,
    ObjectPosition,
    WillChange,

    // ── Multi-column ──
    ColumnCount,
    ColumnWidth,
    ColumnRuleWidth,
    ColumnRuleStyle,
    ColumnRuleColor,

    // ── Layout Features (Section 6.3) ──
    Contain,
    ContentVisibility,
    WritingMode,
    Resize,
    ScrollbarWidth,
    ScrollbarColor,

    // ── UI ──
    Appearance,
    WebkitAppearance,
    MozAppearance,
    AccentColor,
}

/// Total number of supported CSS properties.
pub const CSS_PROPERTY_COUNT: usize = CssPropertyId::AccentColor as usize + 1;

impl CssPropertyId {
    /// Converts a canonical CSS property name to its typed ID.
    ///
    /// Returns `None` for unknown or unsupported property names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "display" => Some(Self::Display),
            "position" => Some(Self::Position),
            "float" => Some(Self::Float),
            "clear" => Some(Self::Clear),
            "box-sizing" => Some(Self::BoxSizing),
            "width" => Some(Self::Width),
            "height" => Some(Self::Height),
            "min-width" => Some(Self::MinWidth),
            "max-width" => Some(Self::MaxWidth),
            "min-height" => Some(Self::MinHeight),
            "max-height" => Some(Self::MaxHeight),
            "margin-top" => Some(Self::MarginTop),
            "margin-right" => Some(Self::MarginRight),
            "margin-bottom" => Some(Self::MarginBottom),
            "margin-left" => Some(Self::MarginLeft),
            "padding-top" => Some(Self::PaddingTop),
            "padding-right" => Some(Self::PaddingRight),
            "padding-bottom" => Some(Self::PaddingBottom),
            "padding-left" => Some(Self::PaddingLeft),
            "top" => Some(Self::Top),
            "right" => Some(Self::Right),
            "bottom" => Some(Self::Bottom),
            "left" => Some(Self::Left),
            "z-index" => Some(Self::ZIndex),
            "overflow" => Some(Self::Overflow),
            "overflow-x" => Some(Self::OverflowX),
            "overflow-y" => Some(Self::OverflowY),
            "visibility" => Some(Self::Visibility),
            "opacity" => Some(Self::Opacity),
            "vertical-align" => Some(Self::VerticalAlign),
            "order" => Some(Self::Order),
            "border-top-width" => Some(Self::BorderTopWidth),
            "border-right-width" => Some(Self::BorderRightWidth),
            "border-bottom-width" => Some(Self::BorderBottomWidth),
            "border-left-width" => Some(Self::BorderLeftWidth),
            "border-top-color" => Some(Self::BorderTopColor),
            "border-right-color" => Some(Self::BorderRightColor),
            "border-bottom-color" => Some(Self::BorderBottomColor),
            "border-left-color" => Some(Self::BorderLeftColor),
            "border-top-style" => Some(Self::BorderTopStyle),
            "border-right-style" => Some(Self::BorderRightStyle),
            "border-bottom-style" => Some(Self::BorderBottomStyle),
            "border-left-style" => Some(Self::BorderLeftStyle),
            "border-top-left-radius" => Some(Self::BorderTopLeftRadius),
            "border-top-right-radius" => Some(Self::BorderTopRightRadius),
            "border-bottom-right-radius" => Some(Self::BorderBottomRightRadius),
            "border-bottom-left-radius" => Some(Self::BorderBottomLeftRadius),
            "border-collapse" => Some(Self::BorderCollapse),
            "border-spacing" => Some(Self::BorderSpacing),
            "table-layout" => Some(Self::TableLayout),
            "caption-side" => Some(Self::CaptionSide),
            "outline-width" => Some(Self::OutlineWidth),
            "outline-style" => Some(Self::OutlineStyle),
            "outline-color" => Some(Self::OutlineColor),
            "outline-offset" => Some(Self::OutlineOffset),
            "color" => Some(Self::Color),
            "background-color" => Some(Self::BackgroundColor),
            "background-image" => Some(Self::BackgroundImage),
            "background-repeat" => Some(Self::BackgroundRepeat),
            "background-size" => Some(Self::BackgroundSize),
            "background-position" => Some(Self::BackgroundPosition),
            "background-gradient" => Some(Self::BackgroundGradient),
            "box-shadow" => Some(Self::BoxShadow),
            "mask-image" => Some(Self::MaskImage),
            "-webkit-mask-image" => Some(Self::WebkitMaskImage),
            "mask-size" => Some(Self::MaskSize),
            "-webkit-mask-size" => Some(Self::WebkitMaskSize),
            "mask-repeat" => Some(Self::MaskRepeat),
            "-webkit-mask-repeat" => Some(Self::WebkitMaskRepeat),
            "mask-position" => Some(Self::MaskPosition),
            "-webkit-mask-position" => Some(Self::WebkitMaskPosition),
            "background-attachment" => Some(Self::BackgroundAttachment),
            "background-clip" => Some(Self::BackgroundClip),
            "-webkit-background-clip" => Some(Self::WebkitBackgroundClip),
            "border-image" => Some(Self::BorderImage),
            "border-image-source" => Some(Self::BorderImageSource),
            "border-image-slice" => Some(Self::BorderImageSlice),
            "border-image-width" => Some(Self::BorderImageWidth),
            "border-image-outset" => Some(Self::BorderImageOutset),
            "border-image-repeat" => Some(Self::BorderImageRepeat),
            "mask-composite" => Some(Self::MaskComposite),
            "-webkit-mask-composite" => Some(Self::WebkitMaskComposite),
            "isolation" => Some(Self::Isolation),
            "font-size" => Some(Self::FontSize),
            "font-weight" => Some(Self::FontWeight),
            "font-style" => Some(Self::FontStyle),
            "font-family" => Some(Self::FontFamily),
            "line-height" => Some(Self::LineHeight),
            "text-align" => Some(Self::TextAlign),
            "text-decoration" => Some(Self::TextDecoration),
            "text-transform" => Some(Self::TextTransform),
            "letter-spacing" => Some(Self::LetterSpacing),
            "word-spacing" => Some(Self::WordSpacing),
            "text-indent" => Some(Self::TextIndent),
            "white-space" => Some(Self::WhiteSpace),
            "text-overflow" => Some(Self::TextOverflow),
            "text-shadow" => Some(Self::TextShadow),
            "direction" => Some(Self::Direction),
            "unicode-bidi" => Some(Self::UnicodeBidi),
            "word-break" => Some(Self::WordBreak),
            "overflow-wrap" | "word-wrap" => Some(Self::OverflowWrap),
            "hyphens" => Some(Self::Hyphens),
            "text-underline-offset" => Some(Self::TextUnderlineOffset),
            "text-decoration-thickness" => Some(Self::TextDecorationThickness),
            "text-emphasis" => Some(Self::TextEmphasis),
            "text-emphasis-style" => Some(Self::TextEmphasisStyle),
            "text-emphasis-color" => Some(Self::TextEmphasisColor),
            "font-feature-settings" => Some(Self::FontFeatureSettings),
            "font-variation-settings" => Some(Self::FontVariationSettings),
            "line-clamp" | "-webkit-line-clamp" => Some(Self::LineClamp),
            "font-display" => Some(Self::FontDisplay),
            "list-style-type" => Some(Self::ListStyleType),
            "list-style-position" => Some(Self::ListStylePosition),
            "content" => Some(Self::Content),
            "cursor" => Some(Self::Cursor),
            "flex-direction" => Some(Self::FlexDirection),
            "flex-wrap" => Some(Self::FlexWrap),
            "justify-content" => Some(Self::JustifyContent),
            "align-items" => Some(Self::AlignItems),
            "align-self" => Some(Self::AlignSelf),
            "align-content" => Some(Self::AlignContent),
            "flex-grow" => Some(Self::FlexGrow),
            "flex-shrink" => Some(Self::FlexShrink),
            "flex-basis" => Some(Self::FlexBasis),
            "row-gap" => Some(Self::RowGap),
            "column-gap" => Some(Self::ColumnGap),
            "grid-template-columns" => Some(Self::GridTemplateColumns),
            "grid-template-rows" => Some(Self::GridTemplateRows),
            "grid-template-areas" => Some(Self::GridTemplateAreas),
            "grid-column-start" => Some(Self::GridColumnStart),
            "grid-column-end" => Some(Self::GridColumnEnd),
            "grid-row-start" => Some(Self::GridRowStart),
            "grid-row-end" => Some(Self::GridRowEnd),
            "grid-auto-flow" => Some(Self::GridAutoFlow),
            "transform" => Some(Self::Transform),
            "transform-origin-x" => Some(Self::TransformOriginX),
            "transform-origin-y" => Some(Self::TransformOriginY),
            "transform-origin-z" => Some(Self::TransformOriginZ),
            "filter" | "-webkit-filter" => Some(Self::Filter),
            "backdrop-filter" => Some(Self::BackdropFilter),
            "-webkit-backdrop-filter" => Some(Self::WebkitBackdropFilter),
            "mix-blend-mode" => Some(Self::MixBlendMode),
            "background-blend-mode" => Some(Self::BackgroundBlendMode),
            "clip-path" => Some(Self::ClipPath),
            "-webkit-clip-path" => Some(Self::WebkitClipPath),
            "mask-mode" => Some(Self::MaskMode),
            "-webkit-mask-mode" => Some(Self::WebkitMaskMode),
            "aspect-ratio" => Some(Self::AspectRatio),
            "object-fit" => Some(Self::ObjectFit),
            "object-position" => Some(Self::ObjectPosition),
            "will-change" => Some(Self::WillChange),
            "column-count" => Some(Self::ColumnCount),
            "column-width" => Some(Self::ColumnWidth),
            "column-rule-width" => Some(Self::ColumnRuleWidth),
            "column-rule-style" => Some(Self::ColumnRuleStyle),
            "column-rule-color" => Some(Self::ColumnRuleColor),
            "contain" => Some(Self::Contain),
            "content-visibility" => Some(Self::ContentVisibility),
            "writing-mode" => Some(Self::WritingMode),
            "resize" => Some(Self::Resize),
            "scrollbar-width" => Some(Self::ScrollbarWidth),
            "scrollbar-color" => Some(Self::ScrollbarColor),
            "appearance" => Some(Self::Appearance),
            "-webkit-appearance" => Some(Self::WebkitAppearance),
            "-moz-appearance" => Some(Self::MozAppearance),
            "accent-color" => Some(Self::AccentColor),
            _ => None,
        }
    }

    /// Returns the canonical CSS property name string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Position => "position",
            Self::Float => "float",
            Self::Clear => "clear",
            Self::BoxSizing => "box-sizing",
            Self::Width => "width",
            Self::Height => "height",
            Self::MinWidth => "min-width",
            Self::MaxWidth => "max-width",
            Self::MinHeight => "min-height",
            Self::MaxHeight => "max-height",
            Self::MarginTop => "margin-top",
            Self::MarginRight => "margin-right",
            Self::MarginBottom => "margin-bottom",
            Self::MarginLeft => "margin-left",
            Self::PaddingTop => "padding-top",
            Self::PaddingRight => "padding-right",
            Self::PaddingBottom => "padding-bottom",
            Self::PaddingLeft => "padding-left",
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::ZIndex => "z-index",
            Self::Overflow => "overflow",
            Self::OverflowX => "overflow-x",
            Self::OverflowY => "overflow-y",
            Self::Visibility => "visibility",
            Self::Opacity => "opacity",
            Self::VerticalAlign => "vertical-align",
            Self::Order => "order",
            Self::BorderTopWidth => "border-top-width",
            Self::BorderRightWidth => "border-right-width",
            Self::BorderBottomWidth => "border-bottom-width",
            Self::BorderLeftWidth => "border-left-width",
            Self::BorderTopColor => "border-top-color",
            Self::BorderRightColor => "border-right-color",
            Self::BorderBottomColor => "border-bottom-color",
            Self::BorderLeftColor => "border-left-color",
            Self::BorderTopStyle => "border-top-style",
            Self::BorderRightStyle => "border-right-style",
            Self::BorderBottomStyle => "border-bottom-style",
            Self::BorderLeftStyle => "border-left-style",
            Self::BorderTopLeftRadius => "border-top-left-radius",
            Self::BorderTopRightRadius => "border-top-right-radius",
            Self::BorderBottomRightRadius => "border-bottom-right-radius",
            Self::BorderBottomLeftRadius => "border-bottom-left-radius",
            Self::BorderCollapse => "border-collapse",
            Self::BorderSpacing => "border-spacing",
            Self::TableLayout => "table-layout",
            Self::CaptionSide => "caption-side",
            Self::OutlineWidth => "outline-width",
            Self::OutlineStyle => "outline-style",
            Self::OutlineColor => "outline-color",
            Self::OutlineOffset => "outline-offset",
            Self::Color => "color",
            Self::BackgroundColor => "background-color",
            Self::BackgroundImage => "background-image",
            Self::BackgroundRepeat => "background-repeat",
            Self::BackgroundSize => "background-size",
            Self::BackgroundPosition => "background-position",
            Self::BackgroundGradient => "background-gradient",
            Self::BoxShadow => "box-shadow",
            Self::MaskImage => "mask-image",
            Self::WebkitMaskImage => "-webkit-mask-image",
            Self::MaskSize => "mask-size",
            Self::WebkitMaskSize => "-webkit-mask-size",
            Self::MaskRepeat => "mask-repeat",
            Self::WebkitMaskRepeat => "-webkit-mask-repeat",
            Self::MaskPosition => "mask-position",
            Self::WebkitMaskPosition => "-webkit-mask-position",
            Self::BackgroundAttachment => "background-attachment",
            Self::BackgroundClip => "background-clip",
            Self::WebkitBackgroundClip => "-webkit-background-clip",
            Self::BorderImage => "border-image",
            Self::BorderImageSource => "border-image-source",
            Self::BorderImageSlice => "border-image-slice",
            Self::BorderImageWidth => "border-image-width",
            Self::BorderImageOutset => "border-image-outset",
            Self::BorderImageRepeat => "border-image-repeat",
            Self::MaskComposite => "mask-composite",
            Self::WebkitMaskComposite => "-webkit-mask-composite",
            Self::Isolation => "isolation",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::FontStyle => "font-style",
            Self::FontFamily => "font-family",
            Self::LineHeight => "line-height",
            Self::TextAlign => "text-align",
            Self::TextDecoration => "text-decoration",
            Self::TextTransform => "text-transform",
            Self::LetterSpacing => "letter-spacing",
            Self::WordSpacing => "word-spacing",
            Self::TextIndent => "text-indent",
            Self::WhiteSpace => "white-space",
            Self::TextOverflow => "text-overflow",
            Self::TextShadow => "text-shadow",
            Self::Direction => "direction",
            Self::UnicodeBidi => "unicode-bidi",
            Self::WordBreak => "word-break",
            Self::OverflowWrap => "overflow-wrap",
            Self::Hyphens => "hyphens",
            Self::TextUnderlineOffset => "text-underline-offset",
            Self::TextDecorationThickness => "text-decoration-thickness",
            Self::TextEmphasis => "text-emphasis",
            Self::TextEmphasisStyle => "text-emphasis-style",
            Self::TextEmphasisColor => "text-emphasis-color",
            Self::FontFeatureSettings => "font-feature-settings",
            Self::FontVariationSettings => "font-variation-settings",
            Self::LineClamp => "line-clamp",
            Self::FontDisplay => "font-display",
            Self::ListStyleType => "list-style-type",
            Self::ListStylePosition => "list-style-position",
            Self::Content => "content",
            Self::Cursor => "cursor",
            Self::FlexDirection => "flex-direction",
            Self::FlexWrap => "flex-wrap",
            Self::JustifyContent => "justify-content",
            Self::AlignItems => "align-items",
            Self::AlignSelf => "align-self",
            Self::AlignContent => "align-content",
            Self::FlexGrow => "flex-grow",
            Self::FlexShrink => "flex-shrink",
            Self::FlexBasis => "flex-basis",
            Self::RowGap => "row-gap",
            Self::ColumnGap => "column-gap",
            Self::GridTemplateColumns => "grid-template-columns",
            Self::GridTemplateRows => "grid-template-rows",
            Self::GridTemplateAreas => "grid-template-areas",
            Self::GridColumnStart => "grid-column-start",
            Self::GridColumnEnd => "grid-column-end",
            Self::GridRowStart => "grid-row-start",
            Self::GridRowEnd => "grid-row-end",
            Self::GridAutoFlow => "grid-auto-flow",
            Self::Transform => "transform",
            Self::TransformOriginX => "transform-origin-x",
            Self::TransformOriginY => "transform-origin-y",
            Self::TransformOriginZ => "transform-origin-z",
            Self::Filter => "filter",
            Self::BackdropFilter => "backdrop-filter",
            Self::WebkitBackdropFilter => "-webkit-backdrop-filter",
            Self::MixBlendMode => "mix-blend-mode",
            Self::BackgroundBlendMode => "background-blend-mode",
            Self::ClipPath => "clip-path",
            Self::WebkitClipPath => "-webkit-clip-path",
            Self::MaskMode => "mask-mode",
            Self::WebkitMaskMode => "-webkit-mask-mode",
            Self::AspectRatio => "aspect-ratio",
            Self::ObjectFit => "object-fit",
            Self::ObjectPosition => "object-position",
            Self::WillChange => "will-change",
            Self::ColumnCount => "column-count",
            Self::ColumnWidth => "column-width",
            Self::ColumnRuleWidth => "column-rule-width",
            Self::ColumnRuleStyle => "column-rule-style",
            Self::ColumnRuleColor => "column-rule-color",
            Self::Contain => "contain",
            Self::ContentVisibility => "content-visibility",
            Self::WritingMode => "writing-mode",
            Self::Resize => "resize",
            Self::ScrollbarWidth => "scrollbar-width",
            Self::ScrollbarColor => "scrollbar-color",
            Self::Appearance => "appearance",
            Self::WebkitAppearance => "-webkit-appearance",
            Self::MozAppearance => "-moz-appearance",
            Self::AccentColor => "accent-color",
        }
    }

    /// Returns `true` if this property is inherited per the CSS specification.
    ///
    /// Inherited properties flow from parent to child unless explicitly overridden.
    pub fn is_inherited(self) -> bool {
        matches!(
            self,
            Self::Color
                | Self::FontSize
                | Self::FontWeight
                | Self::FontStyle
                | Self::FontFamily
                | Self::LineHeight
                | Self::TextAlign
                | Self::TextDecoration
                | Self::TextTransform
                | Self::LetterSpacing
                | Self::WordSpacing
                | Self::TextIndent
                | Self::WhiteSpace
                | Self::TextOverflow
                | Self::ListStyleType
                | Self::ListStylePosition
                | Self::Cursor
                | Self::Visibility
                | Self::BorderCollapse
                | Self::BorderSpacing
                | Self::CaptionSide
                | Self::TextShadow
                | Self::WritingMode
                | Self::ScrollbarWidth
                | Self::ScrollbarColor
                | Self::WordBreak
                | Self::OverflowWrap
                | Self::Hyphens
                | Self::TextEmphasis
                | Self::TextEmphasisStyle
                | Self::TextEmphasisColor
                | Self::FontFeatureSettings
                | Self::FontVariationSettings
                | Self::AccentColor
                // CSS spec: `direction` is an inherited property.
                | Self::Direction
        )
    }

    /// Returns `true` if this property can be animated/transitioned.
    pub fn is_animatable(self) -> bool {
        matches!(
            self,
            Self::Width
                | Self::Height
                | Self::MarginTop
                | Self::MarginRight
                | Self::MarginBottom
                | Self::MarginLeft
                | Self::PaddingTop
                | Self::PaddingRight
                | Self::PaddingBottom
                | Self::PaddingLeft
                | Self::Top
                | Self::Right
                | Self::Bottom
                | Self::Left
                | Self::Opacity
                | Self::Color
                | Self::BackgroundColor
                | Self::BorderTopWidth
                | Self::BorderRightWidth
                | Self::BorderBottomWidth
                | Self::BorderLeftWidth
                | Self::BorderTopColor
                | Self::BorderRightColor
                | Self::BorderBottomColor
                | Self::BorderLeftColor
                | Self::FontSize
                | Self::BorderTopLeftRadius
                | Self::BorderTopRightRadius
                | Self::BorderBottomRightRadius
                | Self::BorderBottomLeftRadius
                | Self::Transform
                | Self::Filter
                | Self::BackdropFilter
                | Self::WebkitBackdropFilter
                | Self::ClipPath
                | Self::WebkitClipPath
                | Self::FlexGrow
                | Self::FlexShrink
                | Self::OutlineWidth
                | Self::OutlineColor
                | Self::OutlineOffset
                | Self::AccentColor
        )
    }

    /// Returns all property IDs as a static slice.
    pub fn all() -> &'static [CssPropertyId] {
        use CssPropertyId::*;
        &[
            Display,
            Position,
            Float,
            Clear,
            BoxSizing,
            Width,
            Height,
            MinWidth,
            MaxWidth,
            MinHeight,
            MaxHeight,
            MarginTop,
            MarginRight,
            MarginBottom,
            MarginLeft,
            PaddingTop,
            PaddingRight,
            PaddingBottom,
            PaddingLeft,
            Top,
            Right,
            Bottom,
            Left,
            ZIndex,
            Overflow,
            OverflowX,
            OverflowY,
            Visibility,
            Opacity,
            VerticalAlign,
            Order,
            BorderTopWidth,
            BorderRightWidth,
            BorderBottomWidth,
            BorderLeftWidth,
            BorderTopColor,
            BorderRightColor,
            BorderBottomColor,
            BorderLeftColor,
            BorderTopStyle,
            BorderRightStyle,
            BorderBottomStyle,
            BorderLeftStyle,
            BorderTopLeftRadius,
            BorderTopRightRadius,
            BorderBottomRightRadius,
            BorderBottomLeftRadius,
            BorderCollapse,
            BorderSpacing,
            TableLayout,
            CaptionSide,
            OutlineWidth,
            OutlineStyle,
            OutlineColor,
            OutlineOffset,
            Color,
            BackgroundColor,
            BackgroundImage,
            BackgroundRepeat,
            BackgroundSize,
            BackgroundPosition,
            BackgroundGradient,
            BoxShadow,
            MaskImage,
            WebkitMaskImage,
            MaskSize,
            WebkitMaskSize,
            MaskRepeat,
            WebkitMaskRepeat,
            MaskPosition,
            WebkitMaskPosition,
            BackgroundAttachment,
            BackgroundClip,
            WebkitBackgroundClip,
            BorderImage,
            BorderImageSource,
            BorderImageSlice,
            BorderImageWidth,
            BorderImageOutset,
            BorderImageRepeat,
            MaskComposite,
            WebkitMaskComposite,
            Isolation,
            FontSize,
            FontWeight,
            FontStyle,
            FontFamily,
            LineHeight,
            TextAlign,
            TextDecoration,
            TextTransform,
            LetterSpacing,
            WordSpacing,
            TextIndent,
            WhiteSpace,
            TextOverflow,
            TextShadow,
            Direction,
            UnicodeBidi,
            WordBreak,
            OverflowWrap,
            Hyphens,
            TextUnderlineOffset,
            TextDecorationThickness,
            TextEmphasis,
            TextEmphasisStyle,
            TextEmphasisColor,
            FontFeatureSettings,
            FontVariationSettings,
            LineClamp,
            FontDisplay,
            ListStyleType,
            ListStylePosition,
            Content,
            Cursor,
            FlexDirection,
            FlexWrap,
            JustifyContent,
            AlignItems,
            AlignSelf,
            AlignContent,
            FlexGrow,
            FlexShrink,
            FlexBasis,
            RowGap,
            ColumnGap,
            GridTemplateColumns,
            GridTemplateRows,
            GridTemplateAreas,
            GridColumnStart,
            GridColumnEnd,
            GridRowStart,
            GridRowEnd,
            GridAutoFlow,
            Transform,
            TransformOriginX,
            TransformOriginY,
            TransformOriginZ,
            Filter,
            BackdropFilter,
            WebkitBackdropFilter,
            MixBlendMode,
            BackgroundBlendMode,
            ClipPath,
            WebkitClipPath,
            MaskMode,
            WebkitMaskMode,
            AspectRatio,
            ObjectFit,
            ObjectPosition,
            WillChange,
            ColumnCount,
            ColumnWidth,
            ColumnRuleWidth,
            ColumnRuleStyle,
            ColumnRuleColor,
            Contain,
            ContentVisibility,
            WritingMode,
            Resize,
            ScrollbarWidth,
            ScrollbarColor,
            Appearance,
            WebkitAppearance,
            MozAppearance,
            AccentColor,
        ]
    }
}

impl std::fmt::Display for CssPropertyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

use crate::values::Value;
use std::collections::HashMap;

/// An enum-indexed map of CSS properties (ARCH-007).
///
/// Backed by a fixed-size `[Option<Value>; CSS_PROPERTY_COUNT]` array indexed by
/// `CssPropertyId` discriminant, providing true O(1) access with zero hashing
/// overhead and no heap allocation per entry.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyMap {
    entries: Box<[Option<Value>; CSS_PROPERTY_COUNT]>,
    len: usize,
}

impl Default for PropertyMap {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertyMap {
    /// Creates an empty `PropertyMap`.
    pub fn new() -> Self {
        Self {
            entries: Box::new(std::array::from_fn(|_| None)),
            len: 0,
        }
    }

    /// Creates an empty `PropertyMap` with pre-allocated capacity (no-op for array-backed).
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    /// Inserts a property and its value into the map.
    pub fn insert(&mut self, property: CssPropertyId, value: Value) -> Option<Value> {
        let idx = property as usize;
        let old = self.entries[idx].take();
        self.entries[idx] = Some(value);
        if old.is_none() {
            self.len += 1;
        }
        old
    }

    /// Gets a reference to the value for a property, if present.
    pub fn get(&self, property: CssPropertyId) -> Option<&Value> {
        self.entries[property as usize].as_ref()
    }

    /// Gets a mutable reference to the value for a property, if present.
    pub fn get_mut(&mut self, property: CssPropertyId) -> Option<&mut Value> {
        self.entries[property as usize].as_mut()
    }

    /// Removes a property from the map and returns its value.
    pub fn remove(&mut self, property: CssPropertyId) -> Option<Value> {
        let old = self.entries[property as usize].take();
        if old.is_some() {
            self.len -= 1;
        }
        old
    }

    /// Returns `true` if the map contains the specified property.
    pub fn contains(&self, property: CssPropertyId) -> bool {
        self.entries[property as usize].is_some()
    }

    /// Returns the number of properties in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the map contains no properties.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clears all properties from the map.
    pub fn clear(&mut self) {
        for slot in self.entries.iter_mut() {
            *slot = None;
        }
        self.len = 0;
    }

    /// Returns an iterator over property-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&CssPropertyId, &Value)> {
        static ALL_IDS: std::sync::OnceLock<Vec<CssPropertyId>> = std::sync::OnceLock::new();
        let all = ALL_IDS.get_or_init(|| CssPropertyId::all().to_vec());
        all.iter()
            .filter_map(move |id| self.entries[*id as usize].as_ref().map(|v| (id, v)))
    }

    /// Returns a mutable iterator over property-value pairs.
    pub fn iter_mut(&mut self) -> PropertyMapIterMut<'_> {
        PropertyMapIterMut {
            entries: self.entries.as_mut(),
            index: 0,
        }
    }

    /// Converts a string-based declaration map into a strongly typed `PropertyMap`.
    /// Properties that are not recognized by `CssPropertyId` are omitted.
    pub fn from_string_map(map: &HashMap<String, Value>) -> Self {
        let mut prop_map = Self::new();
        for (k, v) in map {
            if let Some(prop_id) = CssPropertyId::from_name(k) {
                prop_map.insert(prop_id, v.clone());
            }
        }
        prop_map
    }

    /// Converts this `PropertyMap` into a standard string-keyed map for backward compatibility.
    pub fn to_string_map(&self) -> HashMap<String, Value> {
        let mut map = HashMap::with_capacity(self.len);
        for (id, val) in self.iter() {
            map.insert(id.name().to_string(), val.clone());
        }
        map
    }
}

/// Mutable iterator for `PropertyMap`.
pub struct PropertyMapIterMut<'a> {
    entries: &'a mut [Option<Value>; CSS_PROPERTY_COUNT],
    index: usize,
}

impl<'a> Iterator for PropertyMapIterMut<'a> {
    type Item = (CssPropertyId, &'a mut Value);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < CSS_PROPERTY_COUNT {
            let idx = self.index;
            self.index += 1;
            // SAFETY: We guarantee unique access via the mutable borrow on `entries`,
            // and each index is visited at most once.
            let slot = unsafe { &mut *(self.entries.as_mut_ptr().add(idx)) };
            if let Some(val) = slot {
                // Reconstruct the CssPropertyId from the index.
                // This is safe because all indices in 0..CSS_PROPERTY_COUNT are valid discriminants.
                let id = CssPropertyId::all()[idx];
                return Some((id, val));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_id_roundtrip() {
        for &id in CssPropertyId::all() {
            let name = id.name();
            let resolved = CssPropertyId::from_name(name);
            assert_eq!(resolved, Some(id), "roundtrip failed for {name}");
        }
    }

    #[test]
    fn test_unknown_property_returns_none() {
        assert_eq!(CssPropertyId::from_name("made-up-property"), None);
        assert_eq!(CssPropertyId::from_name(""), None);
    }

    #[test]
    fn test_inherited_properties() {
        assert!(CssPropertyId::Color.is_inherited());
        assert!(CssPropertyId::FontSize.is_inherited());
        assert!(CssPropertyId::Visibility.is_inherited());
        assert!(!CssPropertyId::Display.is_inherited());
        assert!(!CssPropertyId::Width.is_inherited());
        assert!(!CssPropertyId::Opacity.is_inherited());
    }

    #[test]
    fn test_animatable_properties() {
        assert!(CssPropertyId::Opacity.is_animatable());
        assert!(CssPropertyId::Transform.is_animatable());
        assert!(CssPropertyId::Width.is_animatable());
        assert!(!CssPropertyId::Display.is_animatable());
        assert!(!CssPropertyId::Position.is_animatable());
    }

    #[test]
    fn test_property_count() {
        assert_eq!(CssPropertyId::all().len(), CSS_PROPERTY_COUNT);
    }

    #[test]
    fn test_property_map_basic_operations() {
        let mut map = PropertyMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        map.insert(CssPropertyId::Display, Value::Keyword("block".to_string()));
        map.insert(CssPropertyId::Color, Value::Keyword("red".to_string()));

        assert_eq!(map.len(), 2);
        assert!(map.contains(CssPropertyId::Display));
        assert!(map.contains(CssPropertyId::Color));
        assert!(!map.contains(CssPropertyId::Width));

        assert_eq!(
            map.get(CssPropertyId::Display),
            Some(&Value::Keyword("block".to_string()))
        );

        let removed = map.remove(CssPropertyId::Display);
        assert_eq!(removed, Some(Value::Keyword("block".to_string())));
        assert_eq!(map.len(), 1);
        assert!(!map.contains(CssPropertyId::Display));
    }

    #[test]
    fn test_property_map_string_conversion() {
        let mut str_map = HashMap::new();
        str_map.insert("font-size".to_string(), Value::Keyword("16px".to_string()));
        str_map.insert("color".to_string(), Value::Keyword("blue".to_string()));
        str_map.insert(
            "unknown-custom-prop".to_string(),
            Value::Keyword("test".to_string()),
        );

        let prop_map = PropertyMap::from_string_map(&str_map);
        assert_eq!(prop_map.len(), 2);
        assert!(prop_map.contains(CssPropertyId::FontSize));
        assert!(prop_map.contains(CssPropertyId::Color));

        let roundtrip = prop_map.to_string_map();
        assert_eq!(roundtrip.len(), 2);
        assert_eq!(
            roundtrip.get("font-size"),
            Some(&Value::Keyword("16px".to_string()))
        );
        assert_eq!(
            roundtrip.get("color"),
            Some(&Value::Keyword("blue".to_string()))
        );
    }
}
