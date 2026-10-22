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
    TextDecorationStyle(TextDecorationStyle),
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

    /// Serializes this CSS value into canonical CSS text representation.
    pub fn to_css_string(&self) -> String {
        match self {
            Value::Length(len) => match len {
                Length::Px(px) => {
                    let rounded = (*px * 1000.0).round() / 1000.0;
                    if (rounded - rounded.round()).abs() < 1e-4 {
                        format!("{:.0}px", rounded.round())
                    } else {
                        format!("{rounded}px")
                    }
                }
                Length::Percent(pct) => {
                    let rounded = (*pct * 1000.0).round() / 1000.0;
                    if (rounded - rounded.round()).abs() < 1e-4 {
                        format!("{:.0}%", rounded.round())
                    } else {
                        format!("{rounded}%")
                    }
                }
                Length::Em(em) => {
                    let rounded = (*em * 1000.0).round() / 1000.0;
                    if (rounded - rounded.round()).abs() < 1e-4 {
                        format!("{:.0}em", rounded.round())
                    } else {
                        format!("{rounded}em")
                    }
                }
                Length::Ex(ex) => {
                    let rounded = (*ex * 1000.0).round() / 1000.0;
                    if (rounded - rounded.round()).abs() < 1e-4 {
                        format!("{:.0}ex", rounded.round())
                    } else {
                        format!("{rounded}ex")
                    }
                }
                Length::Rem(rem) => {
                    let rounded = (*rem * 1000.0).round() / 1000.0;
                    if (rounded - rounded.round()).abs() < 1e-4 {
                        format!("{:.0}rem", rounded.round())
                    } else {
                        format!("{rounded}rem")
                    }
                }
                Length::Auto => "auto".to_string(),
                Length::Content => "content".to_string(),
                Length::MinContent => "min-content".to_string(),
                Length::MaxContent => "max-content".to_string(),
                Length::FitContent => "fit-content".to_string(),
                Length::Calc(calc) => {
                    let mut parts = Vec::new();
                    if calc.has_percent || calc.percent != 0.0 {
                        let rounded = (calc.percent * 1000.0).round() / 1000.0;
                        if (rounded - rounded.round()).abs() < 1e-4 {
                            parts.push(format!("{:.0}%", rounded.round()));
                        } else {
                            parts.push(format!("{rounded}%"));
                        }
                    }
                    if calc.em != 0.0 {
                        let rounded = (calc.em * 1000.0).round() / 1000.0;
                        if (rounded - rounded.round()).abs() < 1e-4 {
                            parts.push(format!("{:.0}em", rounded.round()));
                        } else {
                            parts.push(format!("{rounded}em"));
                        }
                    }
                    if calc.ex != 0.0 {
                        let rounded = (calc.ex * 1000.0).round() / 1000.0;
                        if (rounded - rounded.round()).abs() < 1e-4 {
                            parts.push(format!("{:.0}ex", rounded.round()));
                        } else {
                            parts.push(format!("{rounded}ex"));
                        }
                    }
                    if calc.px != 0.0 {
                        let rounded = (calc.px * 1000.0).round() / 1000.0;
                        if (rounded - rounded.round()).abs() < 1e-4 {
                            parts.push(format!("{:.0}px", rounded.round()));
                        } else {
                            parts.push(format!("{rounded}px"));
                        }
                    }
                    if calc.rem != 0.0 {
                        let rounded = (calc.rem * 1000.0).round() / 1000.0;
                        if (rounded - rounded.round()).abs() < 1e-4 {
                            parts.push(format!("{:.0}rem", rounded.round()));
                        } else {
                            parts.push(format!("{rounded}rem"));
                        }
                    }
                    if calc.vh != 0.0 {
                        parts.push(format!("{}vh", calc.vh));
                    }
                    if calc.vw != 0.0 {
                        parts.push(format!("{}vw", calc.vw));
                    }
                    if parts.is_empty() {
                        "calc(0px)".to_string()
                    } else if parts.len() == 1 {
                        format!("calc({})", parts[0])
                    } else {
                        format!("calc({})", parts.join(" + "))
                    }
                }
                Length::Math(mid) => {
                    if let Some(expr) = super::math::get_math(*mid) {
                        super::math::format_math_toplevel(&expr)
                    } else {
                        "0px".to_string()
                    }
                }
                Length::Anchor { .. } => "auto".to_string(),
                Length::Vw(v) => format!("{v}vw"),
                Length::Vh(v) => format!("{v}vh"),
                Length::Vmin(v) => format!("{v}vmin"),
                Length::Vmax(v) => format!("{v}vmax"),
                Length::Cqw(v) => format!("{v}cqw"),
                Length::Cqh(v) => format!("{v}cqh"),
                Length::Cqi(v) => format!("{v}cqi"),
                Length::Cqb(v) => format!("{v}cqb"),
                Length::Cqmin(v) => format!("{v}cqmin"),
                Length::Cqmax(v) => format!("{v}cqmax"),
            },
            Value::Color(c) => {
                if c.a == 255 {
                    format!("rgb({}, {}, {})", c.r, c.g, c.b)
                } else if c.a == 0 {
                    "rgba(0, 0, 0, 0)".to_string()
                } else {
                    format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, c.a as f32 / 255.0)
                }
            }
            Value::Number(n) => {
                let rounded = (*n * 1000.0).round() / 1000.0;
                if (rounded - rounded.round()).abs() < 1e-4 {
                    format!("{:.0}", rounded.round())
                } else {
                    format!("{rounded}")
                }
            }
            Value::Percentage(p) => {
                let rounded = (*p * 1000.0).round() / 1000.0;
                if (rounded - rounded.round()).abs() < 1e-4 {
                    format!("{:.0}%", rounded.round())
                } else {
                    format!("{rounded}%")
                }
            }
            Value::Keyword(k) | Value::String(k) => k.clone(),
            Value::Url(u) => format!("url(\"{u}\")"),
            Value::Display(d) => match d {
                Display::None => "none",
                Display::Block => "block",
                Display::Inline => "inline",
                Display::InlineBlock => "inline-block",
                Display::Flex => "flex",
                Display::InlineFlex => "inline-flex",
                Display::Grid => "grid",
                Display::InlineGrid => "inline-grid",
                Display::FlowRoot => "flow-root",
                Display::Contents => "contents",
                Display::Table => "table",
                Display::TableRow => "table-row",
                Display::TableCell => "table-cell",
                Display::TableCaption => "table-caption",
                Display::TableRowGroup => "table-row-group",
                Display::TableHeaderGroup => "table-header-group",
                Display::TableFooterGroup => "table-footer-group",
                Display::TableColumn => "table-column",
                Display::TableColumnGroup => "table-column-group",
                _ => "block",
            }
            .to_string(),
            Value::Position(p) => match p {
                Position::Static => "static",
                Position::Relative => "relative",
                Position::Absolute => "absolute",
                Position::Fixed => "fixed",
                Position::Sticky => "sticky",
            }
            .to_string(),
            Value::Clear(c) => match c {
                Clear::None => "none",
                Clear::Left => "left",
                Clear::Right => "right",
                Clear::Both => "both",
                Clear::InlineStart => "inline-start",
                Clear::InlineEnd => "inline-end",
            }
            .to_string(),
            Value::Float(f) => match f {
                Float::None => "none",
                Float::Left => "left",
                Float::Right => "right",
                Float::InlineStart => "inline-start",
                Float::InlineEnd => "inline-end",
            }
            .to_string(),
            Value::Visibility(v) => match v {
                Visibility::Visible => "visible",
                Visibility::Hidden => "hidden",
                Visibility::Collapse => "collapse",
            }
            .to_string(),
            Value::Overflow(o) => match o {
                Overflow::Visible => "visible",
                Overflow::Hidden => "hidden",
                Overflow::Scroll => "scroll",
                Overflow::Auto => "auto",
            }
            .to_string(),
            Value::FlexDirection(fd) => match fd {
                FlexDirection::Row => "row",
                FlexDirection::RowReverse => "row-reverse",
                FlexDirection::Column => "column",
                FlexDirection::ColumnReverse => "column-reverse",
            }
            .to_string(),
            Value::FlexWrap(fw) => match fw {
                FlexWrap::NoWrap => "nowrap",
                FlexWrap::Wrap => "wrap",
                FlexWrap::WrapReverse => "wrap-reverse",
            }
            .to_string(),
            Value::BoxSizing(bs) => match bs {
                BoxSizing::ContentBox => "content-box",
                BoxSizing::BorderBox => "border-box",
            }
            .to_string(),
            Value::CurrentColor => "currentcolor".to_string(),
            Value::TextDecoration(td) => td.to_css_string(),
            Value::TextDecorationStyle(tds) => tds.as_str().to_string(),
            Value::TextDecorationThickness(tdt) => match tdt {
                TextDecorationThickness::Auto => "auto".to_string(),
                TextDecorationThickness::FromFont => "from-font".to_string(),
                TextDecorationThickness::Length(l) => Value::Length(*l).to_css_string(),
            },
            Value::List(list) => list
                .iter()
                .map(|v| v.to_css_string())
                .collect::<Vec<_>>()
                .join(" "),
            Value::Var { name, fallback } => {
                if let Some(fb) = fallback {
                    format!("var({name}, {})", fb.to_css_string())
                } else {
                    format!("var({name})")
                }
            }
            _ => format!("{self:?}"),
        }
    }
}
