use super::animation::TimingFunction;
use super::enums::*;
use super::gradient::Gradient;
use super::length::Length;
use super::transform::Transform;
use mango_core::Color;

/// A parsed CSS value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Length(Length),
    Fr(f32),
    GridTrackList(Vec<GridTrackSize>),
    GridTrackListWithLines {
        tracks: Vec<GridTrackSize>,
        lines: Vec<(String, usize)>,
    },
    GridPlacement(GridPlacement),
    Color(Color),
    Keyword(String),
    String(String),
    Url(String),
    BackgroundRepeat(BackgroundRepeat),
    BackgroundSize(BackgroundSize),
    BoxShadow(BoxShadow),
    Number(f32),
    Percentage(f32),
    Display(Display),
    Position(Position),
    Float(Float),
    Clear(Clear),
    TextAlign(TextAlign),
    BorderStyle(BorderStyle),
    FontWeight(FontWeight),
    FontStyle(FontStyle),
    BoxSizing(BoxSizing),
    TextDecoration(TextDecoration),
    TextTransform(TextTransform),
    FlexDirection(FlexDirection),
    FlexWrap(FlexWrap),
    JustifyContent(JustifyContent),
    JustifyItems(JustifyItems),
    JustifySelf(JustifySelf),
    AlignItems(AlignItems),
    AlignSelf(AlignSelf),
    AlignContent(AlignContent),
    Overflow(Overflow),
    Visibility(Visibility),
    WhiteSpace(WhiteSpace),
    TextOverflow(TextOverflow),
    ListStyleType(ListStyleType),
    ListStylePosition(ListStylePosition),
    Cursor(Cursor),
    ColumnSpan(ColumnSpan),
    BreakInside(BreakInside),
    VerticalAlign(VerticalAlign),
    BorderCollapse(BorderCollapse),
    TableLayout(TableLayout),
    CaptionSide(CaptionSide),
    GridAutoFlow(GridAutoFlow),
    CurrentColor,
    List(Vec<Value>),
    Var {
        name: String,
        fallback: Option<Box<Value>>,
    },
    /// CSS transform list.
    Transform(Transform),
    /// A CSS `<time>` value, stored in milliseconds (e.g. `0.3s` → 300.0).
    Time(f32),
    /// A CSS `<angle>` value, stored in degrees (e.g. `90deg`, `0.5turn`).
    Angle(f32),
    /// CSS `<easing-function>`.
    TimingFunction(TimingFunction),
    /// CSS gradient (linear or radial).
    Gradient(Box<Gradient>),
    /// CSS text-shadow.
    TextShadow(TextShadow),
    /// CSS filter function list.
    Filter(Vec<FilterFunction>),
    /// CSS object-fit.
    ObjectFit(ObjectFit),
    /// CSS direction.
    Direction(Direction),
    /// CSS unicode-bidi.
    UnicodeBidi(UnicodeBidi),
    /// CSS blend mode (mix-blend-mode / background-blend-mode).
    BlendMode(BlendMode),
    /// CSS clip-path shape.
    ClipPath(ClipPath),
    /// CSS mask-mode.
    MaskMode(MaskMode),
    /// CSS word-break.
    WordBreak(WordBreak),
    /// CSS overflow-wrap (word-wrap).
    OverflowWrap(OverflowWrap),
    /// CSS hyphens.
    Hyphens(Hyphens),
    /// CSS line-clamp.
    LineClamp(LineClamp),
    /// CSS font-display.
    FontDisplay(FontDisplay),
    /// CSS font-variation-settings.
    FontVariationSettings(FontVariationSettings),
    /// CSS font-feature-settings.
    FontFeatureSettings(FontFeatureSettings),
    /// CSS text-emphasis-style.
    TextEmphasisStyle(TextEmphasisStyle),
    /// CSS text-decoration-thickness.
    TextDecorationThickness(TextDecorationThickness),
    /// CSS content property item list for generated content.
    Content(Vec<ContentItem>),
    /// CSS counter-reset or counter-increment actions.
    CounterActions(Vec<CounterAction>),
    /// CSS quotes list (pairs of open and close quote strings).
    Quotes(Vec<(String, String)>),
    /// CSS container-type.
    ContainerType(ContainerType),
    /// CSS background-attachment.
    BackgroundAttachment(BackgroundAttachment),
    /// CSS background-clip.
    BackgroundClip(BackgroundClip),
    /// CSS border-image.
    BorderImage(Box<BorderImage>),
    /// CSS mask-composite.
    MaskComposite(MaskComposite),
    /// CSS isolation.
    Isolation(Isolation),
}

impl Value {
    /// Attempts to parse a color from a hex string, named color, `rgb()`/`rgba()`,
    /// `hsl()`/`hsla()`, `hwb()`, `lab()`, `lch()`, `light-dark()`, `color()`,
    /// `oklab()`/`oklch()`, or `color-mix()` string.
    pub fn parse_color(s: &str) -> Option<Color> {
        super::color::parse_color(s)
    }
}
