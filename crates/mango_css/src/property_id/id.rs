//! Strongly-typed CSS property identifier enum.

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
    JustifyItems,
    JustifySelf,
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

    // ── Anchor Positioning ──
    AnchorName,
    PositionAnchor,
    PositionTryFallbacks,
    PositionTryOrder,

    // ── Scroll-Driven Animations ──
    ScrollTimelineName,
    ScrollTimelineAxis,
    ScrollTimeline,
    ViewTimelineName,
    ViewTimelineAxis,
    ViewTimelineInset,
    ViewTimeline,
    AnimationTimeline,
    AnimationRangeStart,
    AnimationRangeEnd,
    AnimationRange,

    // ── View Transitions ──
    ViewTransitionName,
    ViewTransitionClass,

    // ── Typography 2.0 ──
    TextWrap,

    // ── Container Queries & Containment ──
    ContainerType,
    ContainerName,
    Container,
    ContainIntrinsicSize,
    ContainIntrinsicWidth,
    ContainIntrinsicHeight,
}

/// Total number of supported CSS properties.
pub const CSS_PROPERTY_COUNT: usize = CssPropertyId::ContainIntrinsicHeight as usize + 1;

impl std::fmt::Display for CssPropertyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
