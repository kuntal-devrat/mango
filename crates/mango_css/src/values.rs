//! CSS value types, units, lengths, colors, and layout keywords.

use mango_core::Color;

/// A linear combination of dimensional units in a CSS `calc(...)` expression.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcLength {
    pub px: f32,
    pub percent: f32,
    pub em: f32,
    pub rem: f32,
    pub vw: f32,
    pub vh: f32,
}

impl CalcLength {
    pub const ZERO: CalcLength = CalcLength {
        px: 0.0,
        percent: 0.0,
        em: 0.0,
        rem: 0.0,
        vw: 0.0,
        vh: 0.0,
    };

    pub fn is_zero(&self) -> bool {
        self.px == 0.0
            && self.percent == 0.0
            && self.em == 0.0
            && self.rem == 0.0
            && self.vw == 0.0
            && self.vh == 0.0
    }
}

/// A CSS length or dimension value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum Length {
    /// Exact length in pixels (e.g. `16px`).
    Px(f32),
    /// Relative to current element's font size (e.g. `1.5em`).
    Em(f32),
    /// Relative to root element's font size (e.g. `2rem`).
    Rem(f32),
    /// Relative to containing block dimension (e.g. `50%`).
    Percent(f32),
    /// Relative to viewport width (e.g. `60vw` = 60% of viewport width).
    Vw(f32),
    /// Relative to viewport height (e.g. `15vh` = 15% of viewport height).
    Vh(f32),
    /// Relative to 1% of viewport minimum dimension (min(vw, vh)).
    Vmin(f32),
    /// Relative to 1% of viewport maximum dimension (max(vw, vh)).
    Vmax(f32),
    /// A computed mathematical length resulting from a `calc(...)` expression.
    Calc(CalcLength),
    /// Keyword: content (e.g. `flex-basis: content`).
    Content,
    /// Keyword: min-content.
    MinContent,
    /// Keyword: max-content.
    MaxContent,
    /// Keyword: fit-content.
    FitContent,
    /// Automatic sizing by the layout engine.
    #[default]
    Auto,
}

thread_local! {
    static CURRENT_VIEWPORT: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((800.0, 600.0)) };
}

/// Sets the active viewport width and height used for resolving viewport units (`vw`, `vh`, etc.).
pub fn set_current_viewport(width: f32, height: f32) {
    CURRENT_VIEWPORT.with(|cell| {
        cell.set((width, height));
    });
}

/// Returns the active viewport width and height.
pub fn get_current_viewport() -> (f32, f32) {
    CURRENT_VIEWPORT.with(|cell| cell.get())
}

impl Length {
    pub const ZERO: Length = Length::Px(0.0);

    /// Parses a CSS length string (e.g. `12px`, `1.5em`, `2rem`, `50%`, `auto`).
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("auto") {
            return Some(Length::Auto);
        }
        if trimmed.eq_ignore_ascii_case("content") {
            return Some(Length::Content);
        }
        if trimmed.eq_ignore_ascii_case("min-content") {
            return Some(Length::MinContent);
        }
        if trimmed.eq_ignore_ascii_case("max-content") {
            return Some(Length::MaxContent);
        }
        if trimmed.eq_ignore_ascii_case("fit-content") {
            return Some(Length::FitContent);
        }
        if let Some(num) = trimmed.strip_suffix("px") {
            num.trim().parse::<f32>().ok().map(Length::Px)
        } else if let Some(num) = trimmed.strip_suffix("em") {
            num.trim().parse::<f32>().ok().map(Length::Em)
        } else if let Some(num) = trimmed.strip_suffix("rem") {
            num.trim().parse::<f32>().ok().map(Length::Rem)
        } else if let Some(num) = trimmed.strip_suffix('%') {
            num.trim().parse::<f32>().ok().map(Length::Percent)
        } else if let Some(num) = trimmed.strip_suffix("vw") {
            num.trim().parse::<f32>().ok().map(Length::Vw)
        } else if let Some(num) = trimmed.strip_suffix("vh") {
            num.trim().parse::<f32>().ok().map(Length::Vh)
        } else if let Some(num) = trimmed.strip_suffix("vmin") {
            num.trim().parse::<f32>().ok().map(Length::Vmin)
        } else if let Some(num) = trimmed.strip_suffix("vmax") {
            num.trim().parse::<f32>().ok().map(Length::Vmax)
        } else if let Ok(n) = trimmed.parse::<f32>() {
            Some(Length::Px(n))
        } else {
            None
        }
    }

    /// Resolves this length to concrete pixels given contextual metrics.
    pub fn to_px(&self, font_size: f32, root_font_size: f32, container_size: f32) -> f32 {
        let (vp_w, vp_h) = get_current_viewport();
        self.to_px_with_viewports(font_size, root_font_size, container_size, vp_w, vp_h)
    }

    /// Resolves this length to concrete pixels given contextual metrics and explicit viewport height.
    pub fn to_px_with_viewport(
        &self,
        font_size: f32,
        root_font_size: f32,
        container_size: f32,
        viewport_height: f32,
    ) -> f32 {
        let (vp_w, vp_h) = get_current_viewport();
        let actual_h = if viewport_height > 0.0 && viewport_height != 600.0 {
            viewport_height
        } else {
            vp_h
        };
        self.to_px_with_viewports(font_size, root_font_size, container_size, vp_w, actual_h)
    }

    /// Resolves this length to concrete pixels given contextual metrics and explicit viewport dimensions.
    pub fn to_px_with_viewports(
        &self,
        font_size: f32,
        root_font_size: f32,
        container_size: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> f32 {
        match *self {
            Length::Px(px) => px,
            Length::Em(em) => em * font_size,
            Length::Rem(rem) => rem * root_font_size,
            Length::Percent(pct) => (pct / 100.0) * container_size,
            Length::Vw(vw) => (vw / 100.0) * viewport_width,
            Length::Vh(vh) => (vh / 100.0) * viewport_height,
            Length::Vmin(vmin) => (vmin / 100.0) * viewport_width.min(viewport_height),
            Length::Vmax(vmax) => (vmax / 100.0) * viewport_width.max(viewport_height),
            Length::Calc(calc) => {
                calc.px
                    + (calc.percent / 100.0) * container_size
                    + calc.em * font_size
                    + calc.rem * root_font_size
                    + (calc.vw / 100.0) * viewport_width
                    + (calc.vh / 100.0) * viewport_height
            }
            Length::Auto
            | Length::Content
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent => 0.0,
        }
    }

    /// Returns `true` if this length is `Auto` or `Content`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Length::Auto | Length::Content)
    }

    /// Returns `true` if this length represents intrinsic content sizing.
    pub fn is_content(&self) -> bool {
        matches!(
            self,
            Length::Content | Length::MinContent | Length::MaxContent | Length::FitContent
        )
    }
}


/// CSS `display` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Inline,
    Block,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    FlowRoot,
    Contents,
    Table,
    TableRow,
    TableCell,
    TableCaption,
    ListItem,
    Ruby,
    RubyBase,
    RubyText,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableColumn,
    TableColumnGroup,
    None,
}

impl Display {
    /// Returns `true` if this display value represents an inline-level box.
    #[inline]
    pub fn is_inline_level(&self) -> bool {
        matches!(
            self,
            Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid | Display::Ruby | Display::RubyBase | Display::RubyText
        )
    }
}

/// CSS `position` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// CSS `float` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Float {
    #[default]
    None,
    Left,
    Right,
}

/// CSS `clear` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clear {
    #[default]
    None,
    Left,
    Right,
    Both,
}

/// CSS `text-align` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
    Justify,
}

/// CSS `border-style` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Hidden,
    Groove,
    Ridge,
    Inset,
    Outset,
}

/// CSS `border-collapse` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderCollapse {
    #[default]
    Separate,
    Collapse,
}

/// CSS `table-layout` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableLayout {
    #[default]
    Auto,
    Fixed,
}

/// CSS `caption-side` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptionSide {
    #[default]
    Top,
    Bottom,
}

/// CSS `font-weight` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
    Bolder,
    Lighter,
    Numeric(u16),
}


impl FontWeight {
    /// Returns the numeric weight representation (400 for normal, 700 for bold).
    pub fn to_number(&self) -> u16 {
        match self {
            FontWeight::Normal => 400,
            FontWeight::Bold => 700,
            FontWeight::Bolder => 900,
            FontWeight::Lighter => 100,
            FontWeight::Numeric(n) => *n,
        }
    }
}

/// CSS `font-style` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// CSS `box-sizing` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

/// CSS `appearance` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Auto,
    None,
}

/// CSS `text-decoration` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecoration {
    #[default]
    None,
    Underline,
    LineThrough,
    Overline,
}

/// CSS `text-transform` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Capitalize,
    Uppercase,
    Lowercase,
}

impl TextTransform {
    /// Applies this text transformation to a string according to CSS text-transform rules.
    pub fn apply(&self, text: &str) -> String {
        match self {
            TextTransform::None => text.to_string(),
            TextTransform::Uppercase => text.to_uppercase(),
            TextTransform::Lowercase => text.to_lowercase(),
            TextTransform::Capitalize => {
                let mut result = String::with_capacity(text.len());
                let mut capitalize_next = true;
                for c in text.chars() {
                    if c.is_whitespace() || c.is_ascii_punctuation() {
                        result.push(c);
                        capitalize_next = true;
                    } else if capitalize_next {
                        for uc in c.to_uppercase() {
                            result.push(uc);
                        }
                        capitalize_next = false;
                    } else {
                        result.push(c);
                    }
                }
                result
            }
        }
    }
}

/// CSS `direction` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// CSS `unicode-bidi` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnicodeBidi {
    #[default]
    Normal,
    Embed,
    Isolate,
    BidiOverride,
    IsolateOverride,
    Plaintext,
}

/// CSS `flex-direction` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// CSS `flex-wrap` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// CSS `justify-content` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// CSS `align-items` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `align-self` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignSelf {
    #[default]
    Auto,
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `align-content` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignContent {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// CSS `overflow` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
}

/// CSS `visibility` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

/// CSS `white-space` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

/// CSS `text-overflow` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
}

/// CSS `list-style-type` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListStyleType {
    #[default]
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    None,
}

/// CSS `list-style-position` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListStylePosition {
    #[default]
    Outside,
    Inside,
}

/// CSS `cursor` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Auto,
    Default,
    Pointer,
    Text,
    Move,
    NotAllowed,
    Crosshair,
    Wait,
    Help,
}

/// CSS `vertical-align` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    #[default]
    Baseline,
    Top,
    Middle,
    Bottom,
    TextTop,
    TextBottom,
    Sub,
    Super,
}

/// A sizing specification for a CSS Grid track (column or row).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum GridTrackSize {
    /// Exact or relative length (px, em, rem, %, vw, vh).
    Length(Length),
    /// Flexible fraction of remaining space (e.g. `1fr`, `2.5fr`).
    Fr(f32),
    /// Auto sizing based on content.
    #[default]
    Auto,
    /// Minimum content size.
    MinContent,
    /// Maximum content size.
    MaxContent,
    /// minmax(min, max) sizing constraint.
    MinMax(Box<GridTrackSize>, Box<GridTrackSize>),
    /// Subgrid: adopts tracks from parent grid.
    Subgrid,
    /// `repeat(auto-fill, ...)`
    RepeatAutoFill(Box<GridTrackSize>),
    /// `repeat(auto-fit, ...)`
    RepeatAutoFit(Box<GridTrackSize>),
}

/// CSS `grid-auto-flow` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl GridAutoFlow {
    pub fn is_row(self) -> bool {
        matches!(self, GridAutoFlow::Row | GridAutoFlow::RowDense)
    }

    pub fn is_column(self) -> bool {
        matches!(self, GridAutoFlow::Column | GridAutoFlow::ColumnDense)
    }

    pub fn is_dense(self) -> bool {
        matches!(self, GridAutoFlow::RowDense | GridAutoFlow::ColumnDense)
    }
}

/// Placement coordinate for a grid item along a grid axis (start or end).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GridPlacement {
    #[default]
    Auto,
    /// 1-based grid line index (e.g. 1, 2, -1).
    Line(i32),
    /// Span a number of tracks (e.g. `span 2`).
    Span(u16),
    /// Named grid area (e.g. `titlebar`, `content`, `columnStart`).
    Area(String),
}

/// CSS `background-repeat` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundRepeat {
    #[default]
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
}

/// CSS `background-attachment` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundAttachment {
    #[default]
    Scroll,
    Fixed,
    Local,
}

impl BackgroundAttachment {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "scroll" => Some(Self::Scroll),
            "fixed" => Some(Self::Fixed),
            "local" => Some(Self::Local),
            _ => None,
        }
    }
}

/// CSS `background-clip` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundClip {
    #[default]
    BorderBox,
    PaddingBox,
    ContentBox,
    Text,
}

impl BackgroundClip {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "border-box" => Some(Self::BorderBox),
            "padding-box" => Some(Self::PaddingBox),
            "content-box" => Some(Self::ContentBox),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

/// A single CSS background layer in a multiple background declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundLayer {
    pub image: Option<String>,
    pub gradient: Option<Box<Gradient>>,
    pub repeat: BackgroundRepeat,
    pub size: BackgroundSize,
    pub position: (Length, Length),
    pub attachment: BackgroundAttachment,
    pub clip: BackgroundClip,
    pub blend_mode: BlendMode,
}

impl Default for BackgroundLayer {
    fn default() -> Self {
        Self {
            image: None,
            gradient: None,
            repeat: BackgroundRepeat::Repeat,
            size: BackgroundSize::Auto,
            position: (Length::Px(0.0), Length::Px(0.0)),
            attachment: BackgroundAttachment::Scroll,
            clip: BackgroundClip::BorderBox,
            blend_mode: BlendMode::Normal,
        }
    }
}

/// CSS `border-image-repeat` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderImageRepeat {
    #[default]
    Stretch,
    Repeat,
    Round,
    Space,
}

impl BorderImageRepeat {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stretch" => Some(Self::Stretch),
            "repeat" => Some(Self::Repeat),
            "round" => Some(Self::Round),
            "space" => Some(Self::Space),
            _ => None,
        }
    }
}

/// CSS `border-image` properties.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderImage {
    pub source: Option<String>,
    pub gradient: Option<Box<Gradient>>,
    pub slice: [Length; 4],
    pub fill: bool,
    pub width: [Length; 4],
    pub outset: [Length; 4],
    pub repeat_h: BorderImageRepeat,
    pub repeat_v: BorderImageRepeat,
}

impl Default for BorderImage {
    fn default() -> Self {
        Self {
            source: None,
            gradient: None,
            slice: [Length::Percent(100.0); 4],
            fill: false,
            width: [Length::Px(1.0); 4],
            outset: [Length::Px(0.0); 4],
            repeat_h: BorderImageRepeat::Stretch,
            repeat_v: BorderImageRepeat::Stretch,
        }
    }
}

/// CSS `mask-composite` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskComposite {
    #[default]
    Add,
    Subtract,
    Intersect,
    Exclude,
}

impl MaskComposite {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "add" => Some(Self::Add),
            "subtract" => Some(Self::Subtract),
            "intersect" => Some(Self::Intersect),
            "exclude" => Some(Self::Exclude),
            _ => None,
        }
    }
}

/// CSS `isolation` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Isolation {
    #[default]
    Auto,
    Isolate,
}

impl Isolation {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "isolate" => Some(Self::Isolate),
            _ => None,
        }
    }
}

/// CSS `background-size` property values.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BackgroundSize {
    #[default]
    Auto,
    Cover,
    Contain,
    Explicit(Length, Length),
}

/// CSS `box-shadow` property value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
    pub inset: bool,
}

/// A single CSS 2D/3D transform function.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformFunction {
    /// `translateX(tx)` / `translateY(ty)` / `translate(tx, ty)`
    Translate(f32, f32),
    /// `translateX(tx)`
    TranslateX(f32),
    /// `translateY(ty)`
    TranslateY(f32),
    /// `translate(tx, ty)` with Length units (supports px, %, em, etc.)
    TranslateLen(Length, Length),
    /// `translateX(tx)` with Length units
    TranslateXLen(Length),
    /// `translateY(ty)` with Length units
    TranslateYLen(Length),
    /// `translateZ(tz)` (3D)
    TranslateZ(f32),
    /// `translate3d(tx, ty, tz)` (3D)
    Translate3d(f32, f32, f32),
    /// `rotate(angle_deg)`
    Rotate(f32),
    /// `rotateX(angle_deg)` (3D)
    RotateX(f32),
    /// `rotateY(angle_deg)` (3D)
    RotateY(f32),
    /// `rotateZ(angle_deg)` (3D - equivalent to 2D rotate)
    RotateZ(f32),
    /// `rotate3d(x, y, z, angle_deg)` (3D)
    Rotate3d(f32, f32, f32, f32),
    /// `scaleX(sx)` / `scaleY(sy)` / `scale(sx, sy)`
    Scale(f32, f32),
    /// `scaleX(sx)`
    ScaleX(f32),
    /// `scaleY(sy)`
    ScaleY(f32),
    /// `scaleZ(sz)` (3D)
    ScaleZ(f32),
    /// `scale3d(sx, sy, sz)` (3D)
    Scale3d(f32, f32, f32),
    /// `skewX(angle_deg)` / `skewY(angle_deg)` / `skew(ax, ay)`
    Skew(f32, f32),
    /// `matrix(a, b, c, d, e, f)`
    Matrix(f32, f32, f32, f32, f32, f32),
    /// `matrix3d(m11, m12, ... m44)` (3D)
    Matrix3d([f32; 16]),
    /// `perspective(d)` — 3D perspective
    Perspective(f32),
}

/// Resolved CSS transform: a list of transform functions applied left to right.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transform(pub Vec<TransformFunction>);

impl Transform {
    pub fn is_identity(&self) -> bool {
        self.0.is_empty()
    }

    /// Collapses the transform list into a 2D affine matrix [a, b, c, d, e, f]
    /// (same as SVG/Canvas `matrix(a, b, c, d, e, f)`), projecting 3D functions down per CSS Transforms 2.
    pub fn to_matrix(&self) -> [f32; 6] {
        self.to_matrix_with_size(0.0, 0.0)
    }

    /// Collapses the transform list into a 2D affine matrix, resolving percentage translations against box size.
    pub fn to_matrix_with_size(&self, width: f32, height: f32) -> [f32; 6] {
        let mul4 = |a: [f32; 16], b: [f32; 16]| -> [f32; 16] {
            let mut out = [0.0; 16];
            for r in 0..4 {
                for c in 0..4 {
                    out[r * 4 + c] = a[r * 4] * b[c]
                        + a[r * 4 + 1] * b[4 + c]
                        + a[r * 4 + 2] * b[8 + c]
                        + a[r * 4 + 3] * b[12 + c];
                }
            }
            out
        };
        let mut mat4 = [
            1.0f32, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        for func in &self.0 {
            let n = match func {
                TransformFunction::Translate(tx, ty) => [1.0, 0.0, 0.0, 1.0, *tx, *ty],
                TransformFunction::TranslateX(tx) => [1.0, 0.0, 0.0, 1.0, *tx, 0.0],
                TransformFunction::TranslateY(ty) => [1.0, 0.0, 0.0, 1.0, 0.0, *ty],
                TransformFunction::TranslateLen(tx, ty) => {
                    let rx = match tx {
                        Length::Px(px) => *px,
                        Length::Percent(p) => width * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    let ry = match ty {
                        Length::Px(px) => *px,
                        Length::Percent(p) => height * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    [1.0, 0.0, 0.0, 1.0, rx, ry]
                }
                TransformFunction::TranslateXLen(tx) => {
                    let rx = match tx {
                        Length::Px(px) => *px,
                        Length::Percent(p) => width * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    [1.0, 0.0, 0.0, 1.0, rx, 0.0]
                }
                TransformFunction::TranslateYLen(ty) => {
                    let ry = match ty {
                        Length::Px(px) => *px,
                        Length::Percent(p) => height * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    [1.0, 0.0, 0.0, 1.0, 0.0, ry]
                }
                TransformFunction::TranslateZ(tz) => {
                    // 3D translation: z translation is stored for perspective calculation
                    let mat = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, *tz, 1.0];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Translate3d(tx, ty, tz) => {
                    let mat = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, *tx, *ty, *tz, 1.0];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Rotate(deg) | TransformFunction::RotateZ(deg) => {
                    let r = deg.to_radians();
                    let (s, c) = r.sin_cos();
                    [c, s, -s, c, 0.0, 0.0]
                }
                TransformFunction::RotateX(deg) => {
                    let r = deg.to_radians();
                    let (s, c) = r.sin_cos();
                    let mat = [1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::RotateY(deg) => {
                    let r = deg.to_radians();
                    let (s, c) = r.sin_cos();
                    let mat = [c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Rotate3d(x, y, z, deg) => {
                    let len = (x * x + y * y + z * z).sqrt();
                    if len < 1e-6 {
                        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
                    } else {
                        let nx = x / len;
                        let ny = y / len;
                        let nz = z / len;
                        let r = deg.to_radians();
                        let (s, c) = r.sin_cos();
                        let omc = 1.0 - c;
                        let a = c + nx * nx * omc;
                        let b = ny * nx * omc + nz * s;
                        let c_elem = nx * ny * omc - nz * s;
                        let d = c + ny * ny * omc;
                        [a, b, c_elem, d, 0.0, 0.0]
                    }
                }
                TransformFunction::Scale(sx, sy) => [*sx, 0.0, 0.0, *sy, 0.0, 0.0],
                TransformFunction::ScaleX(sx) => [*sx, 0.0, 0.0, 1.0, 0.0, 0.0],
                TransformFunction::ScaleY(sy) => [1.0, 0.0, 0.0, *sy, 0.0, 0.0],
                TransformFunction::ScaleZ(sz) => {
                    let mat = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, *sz, 0.0, 0.0, 0.0, 0.0, 1.0];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Scale3d(sx, sy, sz) => {
                    let mat = [*sx, 0.0, 0.0, 0.0, 0.0, *sy, 0.0, 0.0, 0.0, 0.0, *sz, 0.0, 0.0, 0.0, 0.0, 1.0];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Skew(ax, ay) => {
                    let tx = ax.to_radians().tan();
                    let ty = ay.to_radians().tan();
                    [1.0, ty, tx, 1.0, 0.0, 0.0]
                }
                TransformFunction::Matrix(a, b, c, d, e, f) => [*a, *b, *c, *d, *e, *f],
                TransformFunction::Matrix3d(m) => {
                    let mut mat = [0.0f32; 16];
                    for i in 0..16 {
                        mat[i] = *m.get(i).unwrap_or(if i % 5 == 0 { &1.0 } else { &0.0 });
                    }
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Perspective(d) => {
                    if *d > 0.0 {
                        let mat = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, -1.0 / *d, 0.0, 0.0, 0.0, 1.0];
                        mat4 = mul4(mat4, mat);
                    }
                    continue;
                }
            };
            // Embed 2D affine [a, b, c, d, e, f] into 4x4 and multiply
            let mat = [
                n[0], n[1], 0.0, 0.0,
                n[2], n[3], 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                n[4], n[5], 0.0, 1.0,
            ];
            mat4 = mul4(mat4, mat);
        }

        // Project 4x4 homogeneous matrix to 2D affine [a, b, c, d, e, f]
        let w = mat4[15];
        let scale = if w.abs() > 1e-6 { 1.0 / w } else { 1.0 };
        [
            mat4[0] * scale,
            mat4[1] * scale,
            mat4[4] * scale,
            mat4[5] * scale,
            mat4[12] * scale,
            mat4[13] * scale,
        ]
    }
}

/// A CSS color stop for gradients.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorStop {
    pub color: Color,
    /// Position as a fraction [0.0, 1.0]. `None` means evenly spaced.
    pub position: Option<f32>,
    /// An optional explicit second position (for `color 10% 20%` double stops).
    pub end_position: Option<f32>,
    /// True when the color stop specified `currentColor`.
    pub is_current_color: bool,
}

impl ColorStop {
    pub fn new(color: Color, position: Option<f32>, end_position: Option<f32>) -> Self {
        Self {
            color,
            position,
            end_position,
            is_current_color: false,
        }
    }
}

/// CSS gradient type.
#[derive(Debug, Clone, PartialEq)]
pub enum Gradient {
    /// `linear-gradient(angle_deg, stops)`.
    /// Angle: 0° = to top, 90° = to right, 180° = to bottom.
    Linear {
        angle_deg: f32,
        stops: Vec<ColorStop>,
        /// `repeating-linear-gradient()` when true.
        repeating: bool,
    },
    /// `radial-gradient(stops)` — simplified circular.
    Radial {
        stops: Vec<ColorStop>,
        /// `repeating-radial-gradient()` when true.
        repeating: bool,
    },
    /// `conic-gradient(from <angle> at <position>, stops)`.
    Conic {
        angle_deg: f32,
        stops: Vec<ColorStop>,
        /// `repeating-conic-gradient()` when true.
        repeating: bool,
    },
}

impl Gradient {
    /// Returns the color stops of this gradient.
    pub fn stops(&self) -> &[ColorStop] {
        match self {
            Gradient::Linear { stops, .. }
            | Gradient::Radial { stops, .. }
            | Gradient::Conic { stops, .. } => stops,
        }
    }

    /// Resolves any `currentColor` stops in this gradient to the specified color.
    pub fn resolve_current_color(&mut self, current_color: Color) {
        let stops = match self {
            Gradient::Linear { stops, .. }
            | Gradient::Radial { stops, .. }
            | Gradient::Conic { stops, .. } => stops,
        };
        for stop in stops {
            if stop.is_current_color {
                stop.color = current_color;
            }
        }
    }

    /// Rasterizes the gradient into a row-major `0xAARRGGBB` pixel buffer.
    pub fn rasterize(&self, width: u32, height: u32) -> Vec<u32> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let total = (width * height) as usize;
        let mut pixels = vec![0u32; total];
        let w = width as f32;
        let h = height as f32;
        match self {
            Gradient::Linear { angle_deg, stops, repeating } => {
                if stops.is_empty() { return pixels; }
                // Resolve positions
                let resolved = resolve_stops(stops);
                let (a_pos, _) = resolved[0];
                let (z_pos, _) = resolved[resolved.len() - 1];
                let span = (z_pos - a_pos).max(0.0001);
                let angle_rad = angle_deg.to_radians();
                let (sin_a, cos_a) = angle_rad.sin_cos();
                // Gradient line length: project corners onto gradient direction
                let grad_len = (w * sin_a.abs() + h * cos_a.abs()).max(1.0);
                for y in 0..height {
                    for x in 0..width {
                        // Normalised position along gradient line (0..1)
                        let cx = x as f32 + 0.5 - w * 0.5;
                        let cy = y as f32 + 0.5 - h * 0.5;
                        let proj = cx * sin_a + cy * (-cos_a);
                        let mut t = proj / grad_len + 0.5;
                        if *repeating {
                            t = a_pos + (t - a_pos).rem_euclid(span);
                        }
                        let color = sample_stops(&resolved, t.clamp(0.0, 1.0));
                        pixels[(y * width + x) as usize] = color;
                    }
                }
            }
            Gradient::Radial { stops, repeating } => {
                if stops.is_empty() { return pixels; }
                let resolved = resolve_stops(stops);
                let (a_pos, _) = resolved[0];
                let (z_pos, _) = resolved[resolved.len() - 1];
                let span = (z_pos - a_pos).max(0.0001);
                let cx = w * 0.5;
                let cy = h * 0.5;
                let radius = (cx * cx + cy * cy).sqrt().max(1.0);
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 + 0.5 - cx;
                        let dy = y as f32 + 0.5 - cy;
                        let mut t = (dx * dx + dy * dy).sqrt() / radius;
                        if *repeating {
                            t = a_pos + (t - a_pos).rem_euclid(span);
                        }
                        let color = sample_stops(&resolved, t.clamp(0.0, 1.0));
                        pixels[(y * width + x) as usize] = color;
                    }
                }
            }
            Gradient::Conic { angle_deg, stops, repeating } => {
                if stops.is_empty() { return pixels; }
                let resolved = resolve_stops(stops);
                let (a_pos, _) = resolved[0];
                let (z_pos, _) = resolved[resolved.len() - 1];
                let span = (z_pos - a_pos).max(0.0001);
                let cx = w * 0.5;
                let cy = h * 0.5;
                // CSS conic gradients sweep clockwise starting from the 12 o'clock position,
                // offset by `from <angle>`.
                let from = angle_deg.to_radians();
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 + 0.5 - cx;
                        let dy = y as f32 + 0.5 - cy;
                        // atan2 measures from +x axis; rotate so 0 is straight up (12 o'clock)
                        let mut theta = dy.atan2(dx) + std::f32::consts::FRAC_PI_2 - from;
                        theta = theta.rem_euclid(std::f32::consts::TAU);
                        let mut t = theta / std::f32::consts::TAU;
                        if *repeating {
                            t = a_pos + (t - a_pos).rem_euclid(span);
                        }
                        let color = sample_stops(&resolved, t.clamp(0.0, 1.0));
                        pixels[(y * width + x) as usize] = color;
                    }
                }
            }
        }
        pixels
    }
}

/// CSS `<step-position>` values for `steps()` timing functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepPosition {
    #[default]
    JumpEnd,
    JumpStart,
    JumpNone,
    JumpBoth,
}

/// CSS `<easing-function>`: how an animation/transition progresses over time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// `cubic-bezier(x1, y1, x2, y2)`
    CubicBezier(f32, f32, f32, f32),
    /// `steps(n, position)`
    Steps(u32, StepPosition),
}

impl Default for TimingFunction {
    fn default() -> Self {
        TimingFunction::Ease
    }
}

impl TimingFunction {
    /// Maps an `input` progress in `[0, 1]` to an eased output progress in `[0, 1]`.
    pub fn sample(&self, input: f32) -> f32 {
        let t = input.clamp(0.0, 1.0);
        match self {
            TimingFunction::Linear => t,
            TimingFunction::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            TimingFunction::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
            TimingFunction::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
            TimingFunction::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
            TimingFunction::CubicBezier(x1, y1, x2, y2) => cubic_bezier(*x1, *y1, *x2, *y2, t),
            TimingFunction::Steps(n, pos) => {
                let steps = (*n).max(1) as f32;
                match pos {
                    StepPosition::JumpStart => ((t * steps).ceil() / steps).clamp(0.0, 1.0),
                    StepPosition::JumpEnd => ((t * steps).floor() / steps).clamp(0.0, 1.0),
                    StepPosition::JumpNone => {
                        if t <= 0.0 || t >= 1.0 {
                            t
                        } else {
                            (((t * steps).floor()) / (steps - 1.0).max(1.0)).clamp(0.0, 1.0)
                        }
                    }
                    StepPosition::JumpBoth => {
                        ((t * (steps + 1.0)).floor() / steps).clamp(0.0, 1.0)
                    }
                }
            }
        }
    }
}

/// Evaluates a cubic-bezier easing curve at progress `t` using Newton-Raphson.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    // Solve for the bezier parameter `u` such that x(u) == t, then return y(u).
    let bezier_axis = |a: f32, b: f32, u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * u * a + 3.0 * inv * u * u * b + u * u * u
    };
    let bezier_axis_deriv = |a: f32, b: f32, u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * a + 6.0 * inv * u * (b - a) + 3.0 * u * u * (1.0 - b)
    };

    let mut u = t;
    for _ in 0..8 {
        let x = bezier_axis(x1, x2, u) - t;
        if x.abs() < 1e-5 {
            break;
        }
        let d = bezier_axis_deriv(x1, x2, u);
        if d.abs() < 1e-6 {
            break;
        }
        u = (u - x / d).clamp(0.0, 1.0);
    }
    bezier_axis(y1, y2, u)
}

/// A CSS `transition` (one comma-separated entry of the `transition` shorthand).
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// `all`, or a property name such as `opacity` or `transform`.
    pub property: String,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    pub timing: TimingFunction,
    /// Delay in milliseconds.
    pub delay_ms: f32,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            property: "all".to_string(),
            duration_ms: 0.0,
            timing: TimingFunction::Ease,
            delay_ms: 0.0,
        }
    }
}

/// CSS `animation-direction` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationDirection {
    #[default]
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// CSS `animation-fill-mode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationFillMode {
    #[default]
    None,
    Forwards,
    Backwards,
    Both,
}

/// CSS `animation-iteration-count` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationIterationCount {
    Finite(f32),
    Infinite,
}

impl Default for AnimationIterationCount {
    fn default() -> Self {
        AnimationIterationCount::Finite(1.0)
    }
}

/// CSS `animation-play-state` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationPlayState {
    #[default]
    Running,
    Paused,
}

/// A CSS `animation` declaration (one comma-separated entry of the shorthand).
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// Keyframes name (`@keyframes <name>`), or empty when unspecified.
    pub name: String,
    pub duration_ms: f32,
    pub timing: TimingFunction,
    pub delay_ms: f32,
    pub iteration_count: AnimationIterationCount,
    pub direction: AnimationDirection,
    pub fill_mode: AnimationFillMode,
    pub play_state: AnimationPlayState,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: String::new(),
            duration_ms: 0.0,
            timing: TimingFunction::Ease,
            delay_ms: 0.0,
            iteration_count: AnimationIterationCount::default(),
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
            play_state: AnimationPlayState::Running,
        }
    }
}

/// Parses a CSS `<easing-function>` keyword or function call (e.g. `ease-in`, `steps(4, end)`).
pub fn parse_timing_function(s: &str) -> Option<TimingFunction> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "linear" => return Some(TimingFunction::Linear),
        "ease" => return Some(TimingFunction::Ease),
        "ease-in" => return Some(TimingFunction::EaseIn),
        "ease-out" => return Some(TimingFunction::EaseOut),
        "ease-in-out" => return Some(TimingFunction::EaseInOut),
        "step-start" => return Some(TimingFunction::Steps(1, StepPosition::JumpStart)),
        "step-end" => return Some(TimingFunction::Steps(1, StepPosition::JumpEnd)),
        _ => {}
    }
    if let Some(inner) = t.strip_prefix("cubic-bezier(").and_then(|s| s.strip_suffix(')')) {
        let nums: Vec<f32> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            return Some(TimingFunction::CubicBezier(nums[0], nums[1], nums[2], nums[3]));
        }
        return None;
    }
    if let Some(inner) = t.strip_prefix("steps(").and_then(|s| s.strip_suffix(')')) {
        let mut parts = inner.split(',');
        let n = parts.next()?.trim().parse::<f32>().ok()?;
        let pos = match parts.next().map(|p| p.trim()) {
            Some("start") | Some("jump-start") => StepPosition::JumpStart,
            Some("jump-none") => StepPosition::JumpNone,
            Some("jump-both") => StepPosition::JumpBoth,
            _ => StepPosition::JumpEnd,
        };
        return Some(TimingFunction::Steps(n.max(1.0) as u32, pos));
    }
    None
}

fn resolve_stops(stops: &[ColorStop]) -> Vec<(f32, Color)> {
    let n = stops.len();
    let mut out: Vec<(f32, Color)> = Vec::with_capacity(n);
    for (i, stop) in stops.iter().enumerate() {
        let default_pos = if n == 1 { 0.5 } else { i as f32 / (n - 1) as f32 };
        let start = stop.position.unwrap_or(default_pos);
        out.push((start, stop.color));
        if let Some(end) = stop.end_position {
            out.push((end, stop.color));
        }
    }
    // Enforce CSS monotonicity: each stop is at least its predecessor's position.
    let mut prev = 0.0f32;
    for entry in out.iter_mut() {
        if entry.0 < prev {
            entry.0 = prev;
        }
        prev = entry.0;
    }
    out
}

fn sample_stops(stops: &[(f32, Color)], t: f32) -> u32 {
    if stops.is_empty() { return 0; }
    if stops.len() == 1 {
        let c = stops[0].1;
        return ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    }
    // Find the bracketing stops
    let (a_pos, a_col) = stops[0];
    let (z_pos, z_col) = stops[stops.len() - 1];
    if t <= a_pos {
        let c = a_col;
        return ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    }
    if t >= z_pos {
        let c = z_col;
        return ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    }
    for i in 1..stops.len() {
        let (p1, c1) = stops[i - 1];
        let (p2, c2) = stops[i];
        if t >= p1 && t <= p2 {
            let span = (p2 - p1).max(0.0001);
            let f = ((t - p1) / span).clamp(0.0, 1.0);
            let r = lerp_u8(c1.r, c2.r, f);
            let g = lerp_u8(c1.g, c2.g, f);
            let b = lerp_u8(c1.b, c2.b, f);
            let a = lerp_u8(c1.a, c2.a, f);
            return ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
    }
    0
}

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

/// CSS `text-shadow` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub color: Color,
}

/// CSS filter function list (simplified subset).
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFunction {
    Blur(f32),
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    Opacity(f32),
    Saturate(f32),
    Sepia(f32),
    HueRotate(f32),
    Invert(f32),
    DropShadow { offset_x: f32, offset_y: f32, blur: f32, color: Color },
}

/// CSS `object-fit` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectFit {
    #[default]
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

/// CSS `contain` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Contain {
    #[default]
    None,
    Strict,
    Content,
    Paint,
    Layout,
    Size,
    Style,
}

/// CSS `content-visibility` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentVisibility {
    #[default]
    Visible,
    Auto,
    Hidden,
}

/// CSS `writing-mode` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// CSS `resize` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Resize {
    #[default]
    None,
    Both,
    Horizontal,
    Vertical,
}

/// CSS `scrollbar-width` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarWidth {
    #[default]
    Auto,
    Thin,
    None,
}

/// CSS `column-span` property values (PRD 8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnSpan {
    #[default]
    None,
    All,
}

/// CSS `break-inside` property values (PRD 8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakInside {
    #[default]
    Auto,
    Avoid,
    AvoidColumn,
    AvoidPage,
}

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
    BorderImage(BorderImage),
    /// CSS mask-composite.
    MaskComposite(MaskComposite),
    /// CSS isolation.
    Isolation(Isolation),
}

/// An item in a CSS `content` property value list for generated content (`::before` / `::after`).
#[derive(Debug, Clone, PartialEq)]
pub enum ContentItem {
    /// Literal string text.
    String(String),
    /// `attr(attribute-name)` resolved from the originating element.
    Attr(String),
    /// `counter(name, style?)` formatted from the current counter value.
    Counter {
        name: String,
        style: Option<String>,
    },
    /// `counters(name, separator, style?)` formatted from all counter scopes.
    Counters {
        name: String,
        separator: String,
        style: Option<String>,
    },
    /// `open-quote`: emits an opening quote at current quote depth and increases depth.
    OpenQuote,
    /// `close-quote`: decreases quote depth and emits a closing quote.
    CloseQuote,
    /// `no-open-quote`: increases quote depth without emitting text.
    NoOpenQuote,
    /// `no-close-quote`: decreases quote depth without emitting text.
    NoCloseQuote,
    /// `url(...)` replaced image content.
    Url(String),
}

/// An action for `counter-reset` or `counter-increment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterAction {
    pub name: String,
    pub value: i32,
}

/// CSS `container-type` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerType {
    #[default]
    Normal,
    Size,
    InlineSize,
    ScrollState,
}

impl ContainerType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "size" => Some(Self::Size),
            "inline-size" => Some(Self::InlineSize),
            "scroll-state" => Some(Self::ScrollState),
            _ => None,
        }
    }

    pub fn affects_inline(&self) -> bool {
        matches!(self, Self::Size | Self::InlineSize)
    }

    pub fn affects_block(&self) -> bool {
        matches!(self, Self::Size)
    }
}

/// CSS `font-display` property for `@font-face` and font loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontDisplay {
    #[default]
    Auto,
    Block,
    Swap,
    Fallback,
    Optional,
}

impl FontDisplay {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "block" => Some(Self::Block),
            "swap" => Some(Self::Swap),
            "fallback" => Some(Self::Fallback),
            "optional" => Some(Self::Optional),
            _ => None,
        }
    }
}

/// CSS `word-break` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WordBreak {
    #[default]
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

impl WordBreak {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "break-all" => Some(Self::BreakAll),
            "keep-all" => Some(Self::KeepAll),
            "break-word" => Some(Self::BreakWord),
            _ => None,
        }
    }
}

/// CSS `overflow-wrap` (or `word-wrap`) property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowWrap {
    #[default]
    Normal,
    BreakWord,
    Anywhere,
}

impl OverflowWrap {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "break-word" => Some(Self::BreakWord),
            "anywhere" => Some(Self::Anywhere),
            _ => None,
        }
    }
}

/// CSS `hyphens` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hyphens {
    None,
    #[default]
    Manual,
    Auto,
}

impl Hyphens {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "manual" => Some(Self::Manual),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// CSS `line-clamp` (and `-webkit-line-clamp`) property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineClamp {
    #[default]
    None,
    Lines(u32),
}

impl LineClamp {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("none") {
            Some(Self::None)
        } else if let Ok(n) = trimmed.parse::<u32>() {
            if n > 0 {
                Some(Self::Lines(n))
            } else {
                Some(Self::None)
            }
        } else {
            None
        }
    }
}

/// CSS `text-emphasis-style` property values.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TextEmphasisStyle {
    #[default]
    None,
    FilledDot,
    OpenDot,
    FilledCircle,
    OpenCircle,
    FilledDoubleCircle,
    OpenDoubleCircle,
    FilledTriangle,
    OpenTriangle,
    FilledSesame,
    OpenSesame,
    String(String),
}

impl TextEmphasisStyle {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let lower = trimmed.to_ascii_lowercase();
        match lower.as_str() {
            "none" => Some(Self::None),
            "dot" | "filled dot" => Some(Self::FilledDot),
            "open dot" => Some(Self::OpenDot),
            "circle" | "filled circle" => Some(Self::FilledCircle),
            "open circle" => Some(Self::OpenCircle),
            "double-circle" | "filled double-circle" => Some(Self::FilledDoubleCircle),
            "open double-circle" => Some(Self::OpenDoubleCircle),
            "triangle" | "filled triangle" => Some(Self::FilledTriangle),
            "open triangle" => Some(Self::OpenTriangle),
            "sesame" | "filled sesame" => Some(Self::FilledSesame),
            "open sesame" => Some(Self::OpenSesame),
            _ => {
                if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
                    || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
                {
                    let inner = &trimmed[1..trimmed.len() - 1];
                    Some(Self::String(inner.to_string()))
                } else if !trimmed.is_empty() && trimmed.chars().count() == 1 {
                    Some(Self::String(trimmed.to_string()))
                } else {
                    None
                }
            }
        }
    }

    /// Returns the character used for emphasis mark rendering.
    pub fn mark_char(&self) -> Option<char> {
        match self {
            Self::None => None,
            Self::FilledDot => Some('•'),
            Self::OpenDot => Some('◦'),
            Self::FilledCircle => Some('●'),
            Self::OpenCircle => Some('○'),
            Self::FilledDoubleCircle | Self::OpenDoubleCircle => Some('◎'),
            Self::FilledTriangle => Some('▲'),
            Self::OpenTriangle => Some('△'),
            Self::FilledSesame => Some('﹅'),
            Self::OpenSesame => Some('﹆'),
            Self::String(s) => s.chars().next(),
        }
    }
}

/// CSS `text-decoration-thickness` property values.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TextDecorationThickness {
    #[default]
    Auto,
    FromFont,
    Length(Length),
}

impl TextDecorationThickness {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("auto") {
            Some(Self::Auto)
        } else if trimmed.eq_ignore_ascii_case("from-font") {
            Some(Self::FromFont)
        } else if let Some(l) = Length::parse(trimmed) {
            Some(Self::Length(l))
        } else {
            None
        }
    }
}

/// CSS `font-variation-settings` property values.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FontVariationSettings {
    #[default]
    Normal,
    Settings(Vec<(String, f32)>),
}

impl FontVariationSettings {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("normal") {
            return Some(Self::Normal);
        }
        let mut list = Vec::new();
        for part in trimmed.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let mut words = p.split_whitespace();
            if let Some(tag_raw) = words.next() {
                let tag = tag_raw.trim_matches('\'').trim_matches('"').to_string();
                let val: f32 = words.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                list.push((tag, val));
            }
        }
        if list.is_empty() {
            None
        } else {
            Some(Self::Settings(list))
        }
    }
}

/// CSS `font-feature-settings` property values.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FontFeatureSettings {
    #[default]
    Normal,
    Features(Vec<(String, u32)>),
}

impl FontFeatureSettings {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("normal") {
            return Some(Self::Normal);
        }
        let mut list = Vec::new();
        for part in trimmed.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let mut words = p.split_whitespace();
            if let Some(tag_raw) = words.next() {
                let tag = tag_raw.trim_matches('\'').trim_matches('"').to_string();
                let val = match words.next() {
                    Some(v) if v.eq_ignore_ascii_case("on") => 1,
                    Some(v) if v.eq_ignore_ascii_case("off") => 0,
                    Some(v) => v.parse::<u32>().unwrap_or(1),
                    None => 1,
                };
                list.push((tag, val));
            }
        }
        if list.is_empty() {
            None
        } else {
            Some(Self::Features(list))
        }
    }
}

/// CSS blend mode for `mix-blend-mode` and `background-blend-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "multiply" => Some(Self::Multiply),
            "screen" => Some(Self::Screen),
            "overlay" => Some(Self::Overlay),
            "darken" => Some(Self::Darken),
            "lighten" => Some(Self::Lighten),
            "color-dodge" => Some(Self::ColorDodge),
            "color-burn" => Some(Self::ColorBurn),
            "hard-light" => Some(Self::HardLight),
            "soft-light" => Some(Self::SoftLight),
            "difference" => Some(Self::Difference),
            "exclusion" => Some(Self::Exclusion),
            "hue" => Some(Self::Hue),
            "saturation" => Some(Self::Saturation),
            "color" => Some(Self::Color),
            "luminosity" => Some(Self::Luminosity),
            _ => None,
        }
    }
}

/// CSS `clip-path` shapes per CSS Masking Module Level 1.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipPath {
    None,
    Circle {
        radius: Length,
        center_x: Length,
        center_y: Length,
    },
    Ellipse {
        radius_x: Length,
        radius_y: Length,
        center_x: Length,
        center_y: Length,
    },
    Inset {
        top: Length,
        right: Length,
        bottom: Length,
        left: Length,
        round: Option<[Length; 4]>,
    },
    Polygon(Vec<(Length, Length)>),
    Url(String),
}

impl Default for ClipPath {
    fn default() -> Self {
        Self::None
    }
}

/// CSS `mask-mode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskMode {
    #[default]
    MatchSource,
    Alpha,
    Luminance,
}

impl MaskMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "alpha" => Some(Self::Alpha),
            "luminance" => Some(Self::Luminance),
            "match-source" => Some(Self::MatchSource),
            _ => None,
        }
    }
}

/// Converts HSL color values to RGB (0..255).
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r_prime, g_prime, b_prime) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r_prime + m) * 255.0).round() as u8,
        ((g_prime + m) * 255.0).round() as u8,
        ((b_prime + m) * 255.0).round() as u8,
    )
}

/// Converts an sRGB channel value in `0.0..=1.0` to linear light.
#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a linear light value in `0.0..=1.0` to sRGB gamma space.
#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts Display P3 coordinates `(r, g, b)` (in `0.0..=1.0`) to sRGB `(r, g, b)` bytes.
pub fn display_p3_to_srgb(r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let r_lin = srgb_to_linear(r);
    let g_lin = srgb_to_linear(g);
    let b_lin = srgb_to_linear(b);

    let r_srgb_lin = 1.224940179 * r_lin - 0.224940179 * g_lin + 0.0 * b_lin;
    let g_srgb_lin = -0.042056916 * r_lin + 1.042056916 * g_lin + 0.0 * b_lin;
    let b_srgb_lin = -0.019637554 * r_lin - 0.078636046 * g_lin + 1.098273600 * b_lin;

    let r_out = (linear_to_srgb(r_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g_out = (linear_to_srgb(g_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b_out = (linear_to_srgb(b_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r_out, g_out, b_out)
}

/// Converts Oklab coordinates `(L, a, b)` to standard sRGB `(r, g, b)` bytes.
pub fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (u8, u8, u8) {
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);

    let r_lin = 4.0767434036 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_;
    let g_lin = -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_;
    let b_lin = -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_;

    let r = (linear_to_srgb(r_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (linear_to_srgb(g_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (linear_to_srgb(b_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r, g, b)
}

/// Converts sRGB bytes `(r, g, b)` to Oklab coordinates `(L, a, b)`.
pub fn srgb_to_oklab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r_lin = srgb_to_linear(r as f32 / 255.0);
    let g_lin = srgb_to_linear(g as f32 / 255.0);
    let b_lin = srgb_to_linear(b as f32 / 255.0);

    let l = (0.4122214708 * r_lin + 0.5363325363 * g_lin + 0.0514459929 * b_lin).cbrt();
    let m = (0.2119034982 * r_lin + 0.6806995451 * g_lin + 0.1073969566 * b_lin).cbrt();
    let s = (0.0883024619 * r_lin + 0.2817188376 * g_lin + 0.6299787005 * b_lin).cbrt();

    let l_out = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    let a_out = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let b_out = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;

    (l_out, a_out, b_out)
}

/// Converts Oklch coordinates `(L, C, h_deg)` to Oklab `(L, a, b)`.
pub fn oklch_to_oklab(l: f32, c: f32, h_deg: f32) -> (f32, f32, f32) {
    let h_rad = h_deg.to_radians();
    let a = c * h_rad.cos();
    let b = c * h_rad.sin();
    (l, a, b)
}

/// Converts Oklab coordinates `(L, a, b)` to Oklch `(L, C, h_deg)`.
pub fn oklab_to_oklch(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let c = (a * a + b * b).sqrt();
    let mut h = b.atan2(a).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    (l, c, h)
}

fn split_commas_depth0(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => { depth += 1; cur.push(ch); }
            ')' => { depth -= 1; cur.push(ch); }
            ',' if depth == 0 => {
                parts.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

fn parse_color_mix_inner(inner: &str) -> Option<Color> {
    let parts = split_commas_depth0(inner);
    if parts.len() < 3 {
        return None;
    }
    let first = parts[0].trim();
    let space = if let Some(sp) = first.strip_prefix("in ") {
        sp.trim().to_ascii_lowercase()
    } else if let Some(sp) = first.strip_prefix("IN ") {
        sp.trim().to_ascii_lowercase()
    } else {
        first.to_ascii_lowercase()
    };

    let parse_color_and_pct = |s: &str| -> Option<(Color, Option<f32>)> {
        let s = s.trim();
        if let Some((col_str, pct_str)) = s.rsplit_once(char::is_whitespace) {
            if let Some(pct) = pct_str.strip_suffix('%') {
                if let Ok(val) = pct.trim().parse::<f32>() {
                    if let Some(c) = Value::parse_color(col_str) {
                        return Some((c, Some(val / 100.0)));
                    }
                }
            }
        }
        Value::parse_color(s).map(|c| (c, None))
    };

    let (c1, p1_opt) = parse_color_and_pct(&parts[1])?;
    let (c2, p2_opt) = parse_color_and_pct(&parts[2])?;

    let (w1, w2, a_mult) = match (p1_opt, p2_opt) {
        (Some(p1), Some(p2)) => {
            let sum = p1 + p2;
            if sum <= 0.0 { (0.5, 0.5, 1.0) } else { (p1 / sum, p2 / sum, sum.min(1.0)) }
        }
        (Some(p1), None) => {
            let w1 = p1.clamp(0.0, 1.0);
            (w1, 1.0 - w1, 1.0)
        }
        (None, Some(p2)) => {
            let w2 = p2.clamp(0.0, 1.0);
            (1.0 - w2, w2, 1.0)
        }
        (None, None) => (0.5, 0.5, 1.0),
    };

    let alpha = ((c1.a as f32 * w1 + c2.a as f32 * w2) * a_mult).clamp(0.0, 255.0).round() as u8;

    if space == "oklab" {
        let (l1, a1, b1) = srgb_to_oklab(c1.r, c1.g, c1.b);
        let (l2, a2, b2) = srgb_to_oklab(c2.r, c2.g, c2.b);
        let l = l1 * w1 + l2 * w2;
        let a_val = a1 * w1 + a2 * w2;
        let b_val = b1 * w1 + b2 * w2;
        let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
        Some(Color::rgba(r, g, b, alpha))
    } else if space == "oklch" {
        let (l1, a1, b1) = srgb_to_oklab(c1.r, c1.g, c1.b);
        let (l2, a2, b2) = srgb_to_oklab(c2.r, c2.g, c2.b);
        let (ok_l1, c_val1, h1) = oklab_to_oklch(l1, a1, b1);
        let (ok_l2, c_val2, h2) = oklab_to_oklch(l2, a2, b2);
        let l = ok_l1 * w1 + ok_l2 * w2;
        let c = c_val1 * w1 + c_val2 * w2;
        let mut diff = (h2 - h1).rem_euclid(360.0);
        if diff > 180.0 { diff -= 360.0; }
        let h = (h1 + diff * w2).rem_euclid(360.0);
        let (lab_l, lab_a, lab_b) = oklch_to_oklab(l, c, h);
        let (r, g, b) = oklab_to_srgb(lab_l, lab_a, lab_b);
        Some(Color::rgba(r, g, b, alpha))
    } else {
        // srgb, display-p3, linear srgb interpolation
        let r_lin = srgb_to_linear(c1.r as f32 / 255.0) * w1 + srgb_to_linear(c2.r as f32 / 255.0) * w2;
        let g_lin = srgb_to_linear(c1.g as f32 / 255.0) * w1 + srgb_to_linear(c2.g as f32 / 255.0) * w2;
        let b_lin = srgb_to_linear(c1.b as f32 / 255.0) * w1 + srgb_to_linear(c2.b as f32 / 255.0) * w2;
        let r = (linear_to_srgb(r_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (linear_to_srgb(g_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (linear_to_srgb(b_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
        Some(Color::rgba(r, g, b, alpha))
    }
}

impl Value {
    /// Attempts to parse a color from a hex string, named color, rgb()/rgba(), or hsl()/hsla() string.
    pub fn parse_color(s: &str) -> Option<Color> {
        let trimmed = s.trim();

        // Hex color: #rgb, #rgba, #rrggbb, #rrggbbaa
        if let Some(hex) = trimmed.strip_prefix('#') {
            return match hex.len() {
                3 => {
                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                    Some(Color::rgb(r, g, b))
                }
                4 => {
                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                    let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                    Some(Color::rgba(r, g, b, a))
                }
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some(Color::rgb(r, g, b))
                }
                8 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                    Some(Color::rgba(r, g, b, a))
                }
                _ => None,
            };
        }

        // rgb(r, g, b), rgba(r, g, b, a), or modern space-separated rgb(r g b / a)
        if let Some(inner) = trimmed
            .strip_prefix("rgb(")
            .or_else(|| trimmed.strip_prefix("rgba("))
            .and_then(|s| s.strip_suffix(')'))
        {
            let (rgb_part, alpha_part) = if let Some((rgb_s, a_s)) = inner.split_once('/') {
                (rgb_s.trim(), Some(a_s.trim()))
            } else {
                (inner.trim(), None)
            };

            let parts: Vec<&str> = if rgb_part.contains(',') {
                rgb_part.split(',').map(|p| p.trim()).collect()
            } else {
                rgb_part.split_whitespace().collect()
            };

            let parse_component = |comp: &str| -> Option<u8> {
                let comp = comp.trim();
                if let Some(pct) = comp.strip_suffix('%') {
                    let f = pct.parse::<f32>().ok()?;
                    Some(((f.clamp(0.0, 100.0) / 100.0) * 255.0).round() as u8)
                } else {
                    let f = comp.parse::<f32>().ok()?;
                    Some(f.clamp(0.0, 255.0).round() as u8)
                }
            };

            let parse_alpha = |a_str: &str| -> Option<u8> {
                let a_str = a_str.trim().trim_start_matches('/').trim();
                let alpha_f = if let Some(pct) = a_str.strip_suffix('%') {
                    pct.parse::<f32>().ok()? / 100.0
                } else {
                    a_str.parse::<f32>().ok()?
                };
                Some((alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8)
            };

            if parts.len() == 3 {
                let r = parse_component(parts[0])?;
                let g = parse_component(parts[1])?;
                let b = parse_component(parts[2])?;
                let a = if let Some(a_str) = alpha_part {
                    parse_alpha(a_str)?
                } else {
                    255
                };
                return Some(Color::rgba(r, g, b, a));
            } else if parts.len() == 4 && alpha_part.is_none() {
                let r = parse_component(parts[0])?;
                let g = parse_component(parts[1])?;
                let b = parse_component(parts[2])?;
                let a = parse_alpha(parts[3])?;
                return Some(Color::rgba(r, g, b, a));
            }
        }

        // hsl(h, s, l), hsla(h, s, l, a), or space-separated hsl(h s l / a)
        if let Some(inner) = trimmed
            .strip_prefix("hsl(")
            .or_else(|| trimmed.strip_prefix("hsla("))
            .and_then(|s| s.strip_suffix(')'))
        {
            let (hsl_part, alpha_part) = if let Some((hsl_s, a_s)) = inner.split_once('/') {
                (hsl_s.trim(), Some(a_s.trim()))
            } else {
                (inner.trim(), None)
            };

            let parts: Vec<&str> = if hsl_part.contains(',') {
                hsl_part.split(',').map(|p| p.trim()).collect()
            } else {
                hsl_part.split_whitespace().collect()
            };

            let parse_alpha = |a_str: &str| -> Option<u8> {
                let a_str = a_str.trim().trim_start_matches('/').trim();
                let alpha_f = if let Some(pct) = a_str.strip_suffix('%') {
                    pct.parse::<f32>().ok()? / 100.0
                } else {
                    a_str.parse::<f32>().ok()?
                };
                Some((alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8)
            };

            if parts.len() == 3 {
                let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
                let s = parts[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
                let l = parts[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
                let a = if let Some(a_str) = alpha_part {
                    parse_alpha(a_str)?
                } else {
                    255
                };
                let (r, g, b) = hsl_to_rgb(h, s, l);
                return Some(Color::rgba(r, g, b, a));
            } else if parts.len() == 4 && alpha_part.is_none() {
                let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
                let s = parts[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
                let l = parts[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
                let a = parse_alpha(parts[3])?;
                let (r, g, b) = hsl_to_rgb(h, s, l);
                return Some(Color::rgba(r, g, b, a));
            }
        }

        // color(display-p3 r g b [/ a]) or color(srgb r g b [/ a])
        if let Some(inner) = trimmed
            .strip_prefix("color(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let (col_part, alpha_part) = if let Some((c_s, a_s)) = inner.split_once('/') {
                (c_s.trim(), Some(a_s.trim()))
            } else {
                (inner.trim(), None)
            };

            let parts: Vec<&str> = if col_part.contains(',') {
                col_part.split(',').map(|p| p.trim()).collect()
            } else {
                col_part.split_whitespace().collect()
            };

            if parts.len() >= 4 {
                let space = parts[0].to_ascii_lowercase();
                let parse_f = |s: &str| -> Option<f32> {
                    let s = s.trim();
                    if let Some(pct) = s.strip_suffix('%') {
                        pct.parse::<f32>().ok().map(|p| p / 100.0)
                    } else {
                        s.parse::<f32>().ok()
                    }
                };

                if let (Some(c1), Some(c2), Some(c3)) = (parse_f(parts[1]), parse_f(parts[2]), parse_f(parts[3])) {
                    let alpha_f = if let Some(a_s) = alpha_part {
                        parse_f(a_s).unwrap_or(1.0)
                    } else if parts.len() >= 5 {
                        parse_f(parts[4]).unwrap_or(1.0)
                    } else {
                        1.0
                    };
                    let a = (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8;

                    if space == "display-p3" {
                        let (r, g, b) = display_p3_to_srgb(c1, c2, c3);
                        return Some(Color::rgba(r, g, b, a));
                    } else if space == "srgb" {
                        let r = (c1.clamp(0.0, 1.0) * 255.0).round() as u8;
                        let g = (c2.clamp(0.0, 1.0) * 255.0).round() as u8;
                        let b = (c3.clamp(0.0, 1.0) * 255.0).round() as u8;
                        return Some(Color::rgba(r, g, b, a));
                    } else if space == "srgb-linear" {
                        let r = (linear_to_srgb(c1).clamp(0.0, 1.0) * 255.0).round() as u8;
                        let g = (linear_to_srgb(c2).clamp(0.0, 1.0) * 255.0).round() as u8;
                        let b = (linear_to_srgb(c3).clamp(0.0, 1.0) * 255.0).round() as u8;
                        return Some(Color::rgba(r, g, b, a));
                    }
                }
            }
        }

        // oklab(L a b [/ alpha])
        if let Some(inner) = trimmed
            .strip_prefix("oklab(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let (lab_part, alpha_part) = if let Some((lab_s, a_s)) = inner.split_once('/') {
                (lab_s.trim(), Some(a_s.trim()))
            } else {
                (inner.trim(), None)
            };

            let parts: Vec<&str> = if lab_part.contains(',') {
                lab_part.split(',').map(|p| p.trim()).collect()
            } else {
                lab_part.split_whitespace().collect()
            };

            let parse_l = |s: &str| -> Option<f32> {
                let s = s.trim();
                if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|p| p / 100.0)
                } else {
                    s.parse::<f32>().ok()
                }
            };
            let parse_ab = |s: &str| -> Option<f32> {
                let s = s.trim();
                if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|p| (p / 100.0) * 0.4)
                } else {
                    s.parse::<f32>().ok()
                }
            };
            let parse_alpha = |s: &str| -> Option<u8> {
                let s = s.trim().trim_start_matches('/').trim();
                let f = if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok()? / 100.0
                } else {
                    s.parse::<f32>().ok()?
                };
                Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
            };

            if parts.len() == 3 {
                if let (Some(l), Some(a_val), Some(b_val)) = (parse_l(parts[0]), parse_ab(parts[1]), parse_ab(parts[2])) {
                    let a = if let Some(a_s) = alpha_part {
                        parse_alpha(a_s).unwrap_or(255)
                    } else {
                        255
                    };
                    let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
                    return Some(Color::rgba(r, g, b, a));
                }
            } else if parts.len() == 4 && alpha_part.is_none() {
                if let (Some(l), Some(a_val), Some(b_val), Some(a)) = (parse_l(parts[0]), parse_ab(parts[1]), parse_ab(parts[2]), parse_alpha(parts[3])) {
                    let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
                    return Some(Color::rgba(r, g, b, a));
                }
            }
        }

        // oklch(L C h [/ alpha])
        if let Some(inner) = trimmed
            .strip_prefix("oklch(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let (lch_part, alpha_part) = if let Some((lch_s, a_s)) = inner.split_once('/') {
                (lch_s.trim(), Some(a_s.trim()))
            } else {
                (inner.trim(), None)
            };

            let parts: Vec<&str> = if lch_part.contains(',') {
                lch_part.split(',').map(|p| p.trim()).collect()
            } else {
                lch_part.split_whitespace().collect()
            };

            let parse_l = |s: &str| -> Option<f32> {
                let s = s.trim();
                if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|p| p / 100.0)
                } else {
                    s.parse::<f32>().ok()
                }
            };
            let parse_c = |s: &str| -> Option<f32> {
                let s = s.trim();
                if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|p| (p / 100.0) * 0.4)
                } else {
                    s.parse::<f32>().ok()
                }
            };
            let parse_hue = |s: &str| -> Option<f32> {
                let s = s.trim();
                if s.eq_ignore_ascii_case("none") {
                    return Some(0.0);
                }
                if let Some(deg) = s.strip_suffix("deg") {
                    return deg.trim().parse::<f32>().ok();
                }
                if let Some(rad) = s.strip_suffix("rad") {
                    return rad.trim().parse::<f32>().ok().map(|r| r.to_degrees());
                }
                if let Some(turn) = s.strip_suffix("turn") {
                    return turn.trim().parse::<f32>().ok().map(|t| t * 360.0);
                }
                if let Some(grad) = s.strip_suffix("grad") {
                    return grad.trim().parse::<f32>().ok().map(|g| g * 0.9);
                }
                s.parse::<f32>().ok()
            };
            let parse_alpha = |s: &str| -> Option<u8> {
                let s = s.trim().trim_start_matches('/').trim();
                let f = if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok()? / 100.0
                } else {
                    s.parse::<f32>().ok()?
                };
                Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
            };

            if parts.len() == 3 {
                if let (Some(l), Some(c), Some(h)) = (parse_l(parts[0]), parse_c(parts[1]), parse_hue(parts[2])) {
                    let a = if let Some(a_s) = alpha_part {
                        parse_alpha(a_s).unwrap_or(255)
                    } else {
                        255
                    };
                    let (ok_l, ok_a, ok_b) = oklch_to_oklab(l, c, h);
                    let (r, g, b) = oklab_to_srgb(ok_l, ok_a, ok_b);
                    return Some(Color::rgba(r, g, b, a));
                }
            } else if parts.len() == 4 && alpha_part.is_none() {
                if let (Some(l), Some(c), Some(h), Some(a)) = (parse_l(parts[0]), parse_c(parts[1]), parse_hue(parts[2]), parse_alpha(parts[3])) {
                    let (ok_l, ok_a, ok_b) = oklch_to_oklab(l, c, h);
                    let (r, g, b) = oklab_to_srgb(ok_l, ok_a, ok_b);
                    return Some(Color::rgba(r, g, b, a));
                }
            }
        }

        // color-mix(in <color-space>, <color1> [p1], <color2> [p2])
        if let Some(inner) = trimmed
            .strip_prefix("color-mix(")
            .and_then(|s| s.strip_suffix(')'))
        {
            if let Some(col) = parse_color_mix_inner(inner) {
                return Some(col);
            }
        }

        // Full CSS 148 standard named colors (CSS Color Module Level 3/4)
        match trimmed.to_ascii_lowercase().as_str() {
            "aliceblue" => Some(Color::rgb(240, 248, 255)),
            "antiquewhite" => Some(Color::rgb(250, 235, 215)),
            "aqua" | "cyan" => Some(Color::rgb(0, 255, 255)),
            "aquamarine" => Some(Color::rgb(127, 255, 212)),
            "azure" => Some(Color::rgb(240, 255, 255)),
            "beige" => Some(Color::rgb(245, 245, 220)),
            "bisque" => Some(Color::rgb(255, 228, 196)),
            "black" => Some(Color::BLACK),
            "blanchedalmond" => Some(Color::rgb(255, 235, 205)),
            "blue" => Some(Color::BLUE),
            "blueviolet" => Some(Color::rgb(138, 43, 226)),
            "brown" => Some(Color::rgb(165, 42, 42)),
            "burlywood" => Some(Color::rgb(222, 184, 135)),
            "cadetblue" => Some(Color::rgb(95, 158, 160)),
            "chartreuse" => Some(Color::rgb(127, 255, 0)),
            "chocolate" => Some(Color::rgb(210, 105, 30)),
            "coral" => Some(Color::rgb(255, 127, 80)),
            "cornflowerblue" => Some(Color::rgb(100, 149, 237)),
            "cornsilk" => Some(Color::rgb(255, 248, 220)),
            "crimson" => Some(Color::rgb(220, 20, 60)),
            "dark" => Some(Color::MANGO_DARK),
            "darkblue" => Some(Color::rgb(0, 0, 139)),
            "darkcyan" => Some(Color::rgb(0, 139, 139)),
            "darkgoldenrod" => Some(Color::rgb(184, 134, 11)),
            "darkgray" | "darkgrey" => Some(Color::rgb(169, 169, 169)),
            "darkgreen" => Some(Color::rgb(0, 100, 0)),
            "darkkhaki" => Some(Color::rgb(189, 183, 107)),
            "darkmagenta" => Some(Color::rgb(139, 0, 139)),
            "darkolivegreen" => Some(Color::rgb(85, 107, 47)),
            "darkorange" => Some(Color::rgb(255, 140, 0)),
            "darkorchid" => Some(Color::rgb(153, 50, 204)),
            "darkred" => Some(Color::rgb(139, 0, 0)),
            "darksalmon" => Some(Color::rgb(233, 150, 122)),
            "darkseagreen" => Some(Color::rgb(143, 188, 143)),
            "darkslateblue" => Some(Color::rgb(72, 61, 139)),
            "darkslategray" | "darkslategrey" => Some(Color::rgb(47, 79, 79)),
            "darkturquoise" => Some(Color::rgb(0, 206, 209)),
            "darkviolet" => Some(Color::rgb(148, 0, 211)),
            "deeppink" => Some(Color::rgb(255, 20, 147)),
            "deepskyblue" => Some(Color::rgb(0, 191, 255)),
            "dimgray" | "dimgrey" => Some(Color::rgb(105, 105, 105)),
            "dodgerblue" => Some(Color::rgb(30, 144, 255)),
            "firebrick" => Some(Color::rgb(178, 34, 34)),
            "floralwhite" => Some(Color::rgb(255, 250, 240)),
            "forestgreen" => Some(Color::rgb(34, 139, 34)),
            "fuchsia" | "magenta" => Some(Color::rgb(255, 0, 255)),
            "gainsboro" => Some(Color::rgb(220, 220, 220)),
            "ghostwhite" => Some(Color::rgb(248, 248, 255)),
            "gold" => Some(Color::rgb(255, 215, 0)),
            "goldenrod" => Some(Color::rgb(218, 165, 32)),
            "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
            "green" => Some(Color::GREEN),
            "greenyellow" => Some(Color::rgb(173, 255, 47)),
            "honeydew" => Some(Color::rgb(240, 255, 240)),
            "hotpink" => Some(Color::rgb(255, 105, 180)),
            "indianred" => Some(Color::rgb(205, 92, 92)),
            "indigo" => Some(Color::rgb(75, 0, 130)),
            "ivory" => Some(Color::rgb(255, 255, 240)),
            "khaki" => Some(Color::rgb(240, 230, 140)),
            "lavender" => Some(Color::rgb(230, 230, 250)),
            "lavenderblush" => Some(Color::rgb(255, 240, 245)),
            "lawngreen" => Some(Color::rgb(124, 252, 0)),
            "lemonchiffon" => Some(Color::rgb(255, 250, 205)),
            "light" => Some(Color::MANGO_LIGHT),
            "lightblue" => Some(Color::rgb(173, 216, 230)),
            "lightcoral" => Some(Color::rgb(240, 128, 128)),
            "lightcyan" => Some(Color::rgb(224, 255, 255)),
            "lightgoldenrodyellow" => Some(Color::rgb(250, 250, 210)),
            "lightgray" | "lightgrey" => Some(Color::rgb(211, 211, 211)),
            "lightgreen" => Some(Color::rgb(144, 238, 144)),
            "lightpink" => Some(Color::rgb(255, 182, 193)),
            "lightsalmon" => Some(Color::rgb(255, 160, 122)),
            "lightseagreen" => Some(Color::rgb(32, 178, 170)),
            "lightskyblue" => Some(Color::rgb(135, 206, 250)),
            "lightslategray" | "lightslategrey" => Some(Color::rgb(119, 136, 153)),
            "lightsteelblue" => Some(Color::rgb(176, 196, 222)),
            "lightyellow" => Some(Color::rgb(255, 255, 224)),
            "lime" => Some(Color::rgb(0, 255, 0)),
            "limegreen" => Some(Color::rgb(50, 205, 50)),
            "linen" => Some(Color::rgb(250, 240, 230)),
            "mango" | "orange" => Some(Color::MANGO_ORANGE),
            "maroon" => Some(Color::rgb(128, 0, 0)),
            "mediumaquamarine" => Some(Color::rgb(102, 205, 170)),
            "mediumblue" => Some(Color::rgb(0, 0, 205)),
            "mediumorchid" => Some(Color::rgb(186, 85, 211)),
            "mediumpurple" => Some(Color::rgb(147, 112, 219)),
            "mediumseagreen" => Some(Color::rgb(60, 179, 113)),
            "mediumslateblue" => Some(Color::rgb(123, 104, 238)),
            "mediumspringgreen" => Some(Color::rgb(0, 250, 154)),
            "mediumturquoise" => Some(Color::rgb(72, 209, 204)),
            "mediumvioletred" => Some(Color::rgb(199, 21, 133)),
            "midnightblue" => Some(Color::rgb(25, 25, 112)),
            "mintcream" => Some(Color::rgb(245, 255, 250)),
            "mistyrose" => Some(Color::rgb(255, 228, 225)),
            "moccasin" => Some(Color::rgb(255, 228, 181)),
            "navajowhite" => Some(Color::rgb(255, 222, 173)),
            "navy" => Some(Color::rgb(0, 0, 128)),
            "oldlace" => Some(Color::rgb(253, 245, 230)),
            "olive" => Some(Color::rgb(128, 128, 0)),
            "olivedrab" => Some(Color::rgb(107, 142, 35)),
            "orangered" => Some(Color::rgb(255, 69, 0)),
            "orchid" => Some(Color::rgb(218, 112, 214)),
            "palegoldenrod" => Some(Color::rgb(238, 232, 170)),
            "palegreen" => Some(Color::rgb(152, 251, 152)),
            "paleturquoise" => Some(Color::rgb(175, 238, 238)),
            "palevioletred" => Some(Color::rgb(219, 112, 147)),
            "papayawhip" => Some(Color::rgb(255, 239, 213)),
            "peachpuff" => Some(Color::rgb(255, 218, 185)),
            "peru" => Some(Color::rgb(205, 133, 63)),
            "pink" => Some(Color::rgb(255, 192, 203)),
            "plum" => Some(Color::rgb(221, 160, 221)),
            "powderblue" => Some(Color::rgb(176, 224, 230)),
            "purple" => Some(Color::rgb(128, 0, 128)),
            "rebeccapurple" => Some(Color::rgb(102, 51, 153)),
            "red" => Some(Color::RED),
            "rosybrown" => Some(Color::rgb(188, 143, 143)),
            "royalblue" => Some(Color::rgb(65, 105, 225)),
            "saddlebrown" => Some(Color::rgb(139, 69, 19)),
            "salmon" => Some(Color::rgb(250, 128, 114)),
            "sandybrown" => Some(Color::rgb(244, 164, 96)),
            "seagreen" => Some(Color::rgb(46, 139, 87)),
            "seashell" => Some(Color::rgb(255, 245, 238)),
            "sienna" => Some(Color::rgb(160, 82, 45)),
            "silver" => Some(Color::rgb(192, 192, 192)),
            "skyblue" => Some(Color::rgb(135, 206, 235)),
            "slateblue" => Some(Color::rgb(106, 90, 205)),
            "slategray" | "slategrey" => Some(Color::rgb(112, 128, 144)),
            "snow" => Some(Color::rgb(255, 250, 250)),
            "springgreen" => Some(Color::rgb(0, 255, 127)),
            "steelblue" => Some(Color::rgb(70, 130, 180)),
            "tan" => Some(Color::rgb(210, 180, 140)),
            "teal" => Some(Color::rgb(0, 128, 128)),
            "thistle" => Some(Color::rgb(216, 191, 216)),
            "tomato" => Some(Color::rgb(255, 99, 71)),
            "transparent" => Some(Color::TRANSPARENT),
            "turquoise" => Some(Color::rgb(64, 224, 208)),
            "violet" => Some(Color::rgb(238, 130, 238)),
            "wheat" => Some(Color::rgb(245, 222, 179)),
            "white" => Some(Color::WHITE),
            "whitesmoke" => Some(Color::rgb(245, 245, 245)),
            "yellow" => Some(Color::rgb(255, 255, 0)),
            "yellowgreen" => Some(Color::rgb(154, 205, 50)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_transform() {
        assert_eq!(TextTransform::Uppercase.apply("hello world"), "HELLO WORLD");
        assert_eq!(TextTransform::Lowercase.apply("HELLO WORLD"), "hello world");
        assert_eq!(TextTransform::Capitalize.apply("hello world"), "Hello World");
        assert_eq!(TextTransform::Capitalize.apply("foo-bar_baz"), "Foo-Bar_Baz");
        assert_eq!(TextTransform::None.apply("Hello World"), "Hello World");
    }

    #[test]
    fn test_length_to_px() {
        assert_eq!(Length::Px(24.0).to_px(16.0, 16.0, 1000.0), 24.0);
        assert_eq!(Length::Em(1.5).to_px(16.0, 16.0, 1000.0), 24.0);
        assert_eq!(Length::Rem(2.0).to_px(16.0, 14.0, 1000.0), 28.0);
        assert_eq!(Length::Percent(50.0).to_px(16.0, 16.0, 800.0), 400.0);
        assert!((Length::Vw(60.0).to_px(16.0, 16.0, 800.0) - 480.0).abs() < 0.001);
        assert_eq!(Length::Vh(15.0).to_px_with_viewport(16.0, 16.0, 800.0, 600.0), 90.0);
        assert_eq!(Length::Vw(10.0).to_px_with_viewports(16.0, 16.0, 200.0, 800.0, 600.0), 80.0);
        assert_eq!(Length::Vw(10.0).to_px_with_viewport(16.0, 16.0, 200.0, 600.0), 80.0);
    }

    #[test]
    fn test_parse_color_hex() {
        assert_eq!(Value::parse_color("#fff"), Some(Color::WHITE));
        assert_eq!(Value::parse_color("#000000"), Some(Color::BLACK));
        assert_eq!(Value::parse_color("#ff0000"), Some(Color::RED));
        assert_eq!(
            Value::parse_color("#ffa136"),
            Some(Color::rgb(255, 161, 54))
        );
    }

    #[test]
    fn test_parse_color_rgb() {
        assert_eq!(
            Value::parse_color("rgb(255, 0, 0)"),
            Some(Color::RED)
        );
        assert_eq!(
            Value::parse_color("rgba(0, 0, 0, 0)"),
            Some(Color::TRANSPARENT)
        );
        assert_eq!(
            Value::parse_color("rgb(255 0 0)"),
            Some(Color::RED)
        );
        assert_eq!(
            Value::parse_color("rgb(255 0 0 / 0.5)"),
            Some(Color::rgba(255, 0, 0, 128))
        );
        assert_eq!(
            Value::parse_color("rgb(100% 0% 0% / 50%)"),
            Some(Color::rgba(255, 0, 0, 128))
        );
    }

    #[test]
    fn test_named_colors() {
        assert_eq!(Value::parse_color("blue"), Some(Color::BLUE));
        assert_eq!(Value::parse_color("orange"), Some(Color::MANGO_ORANGE));
        assert_eq!(Value::parse_color("transparent"), Some(Color::TRANSPARENT));
        assert_eq!(Value::parse_color("cornflowerblue"), Some(Color::rgb(100, 149, 237)));
        assert_eq!(Value::parse_color("rebeccapurple"), Some(Color::rgb(102, 51, 153)));
        assert_eq!(Value::parse_color("gainsboro"), Some(Color::rgb(220, 220, 220)));
    }

    #[test]
    fn test_parse_color_display_p3_and_srgb() {
        // sRGB
        assert_eq!(Value::parse_color("color(srgb 1 0 0)"), Some(Color::RED));
        assert_eq!(Value::parse_color("color(srgb 0 1 0 / 0.5)"), Some(Color::rgba(0, 255, 0, 128)));
        assert_eq!(Value::parse_color("color(srgb 100% 100% 0%)"), Some(Color::rgb(255, 255, 0)));

        // display-p3
        let p3_red = Value::parse_color("color(display-p3 1 0 0)").unwrap();
        assert_eq!(p3_red.r, 255);
        assert_eq!(p3_red.a, 255);

        let p3_white = Value::parse_color("color(display-p3 1 1 1 / 1)").unwrap();
        assert_eq!(p3_white, Color::WHITE);

        let p3_black = Value::parse_color("color(display-p3 0 0 0)").unwrap();
        assert_eq!(p3_black, Color::BLACK);
    }

    #[test]
    fn test_parse_color_oklab_and_oklch() {
        // oklab
        assert_eq!(Value::parse_color("oklab(0 0 0)"), Some(Color::BLACK));
        assert_eq!(Value::parse_color("oklab(1 0 0)"), Some(Color::WHITE));
        let ok_red = Value::parse_color("oklab(0.628 0.225 0.126)").unwrap();
        assert!(ok_red.r >= 250);
        assert!(ok_red.g <= 10);
        assert!(ok_red.b <= 10);

        // oklch
        assert_eq!(Value::parse_color("oklch(0 0 0)"), Some(Color::BLACK));
        assert_eq!(Value::parse_color("oklch(100% 0 0deg)"), Some(Color::WHITE));
        let oklch_red = Value::parse_color("oklch(0.628 0.257 29.23deg)").unwrap();
        assert!(oklch_red.r >= 240);

        // oklch with turns and alpha
        let oklch_alpha = Value::parse_color("oklch(0.5 0.2 0.5turn / 50%)").unwrap();
        assert_eq!(oklch_alpha.a, 128);
    }

    #[test]
    fn test_parse_color_mix() {
        // 50% / 50% in srgb
        let mixed = Value::parse_color("color-mix(in srgb, red, blue)").unwrap();
        assert!(mixed.r > 150);
        assert_eq!(mixed.g, 0);
        assert!(mixed.b > 150);
        assert_eq!(mixed.a, 255);

        // 100% red in srgb
        assert_eq!(Value::parse_color("color-mix(in srgb, red 100%, blue 0%)"), Some(Color::RED));

        // color-mix in oklab
        let ok_mixed = Value::parse_color("color-mix(in oklab, red 50%, blue 50%)").unwrap();
        assert!(ok_mixed.r > 100);
        assert!(ok_mixed.b > 100);
        assert_eq!(ok_mixed.a, 255);

        // color-mix with alpha
        let alpha_mix = Value::parse_color("color-mix(in srgb, rgba(255, 0, 0, 0.5) 50%, rgba(0, 0, 255, 0.5) 50%)").unwrap();
        assert_eq!(alpha_mix.a, 128);
    }

    #[test]
    fn test_repeating_gradients_rasterize() {
        use crate::values::{ColorStop, Gradient};
        let grad = Gradient::Linear {
            angle_deg: 90.0,
            stops: vec![
                ColorStop::new(Color::RED, Some(0.0), None),
                ColorStop::new(Color::BLUE, Some(0.1), None),
            ],
            repeating: true,
        };
        let pixels = grad.rasterize(100, 10);
        assert_eq!(pixels.len(), 1000);
        // At every 10px period (10%), the pixel values repeat identically
        let c0 = pixels[0];
        let c10 = pixels[10];
        let c20 = pixels[20];
        let c30 = pixels[30];
        assert_eq!(c0, c10);
        assert_eq!(c10, c20);
        assert_eq!(c20, c30);
    }

    #[test]
    fn test_colors_and_gradients() {
        use crate::values::{ColorStop, Gradient};

        // 1. color(display-p3)
        let p3_green = Value::parse_color("color(display-p3 0 1 0)").unwrap();
        assert!(p3_green.g > 240);

        // 2. oklab and oklch
        let oklab_blue = Value::parse_color("oklab(0.45 -0.03 -0.31)").unwrap();
        assert!(oklab_blue.b > 180);

        let oklch_green = Value::parse_color("oklch(0.86 0.29 142deg)").unwrap();
        assert!(oklch_green.g > 180);

        // 3. color-mix in oklch
        let mixed_lch = Value::parse_color("color-mix(in oklch, yellow 60%, green 40%)").unwrap();
        assert!(mixed_lch.g > 150);

        // 4. currentColor in gradient stops
        let mut grad = Gradient::Linear {
            angle_deg: 180.0,
            stops: vec![
                ColorStop {
                    color: Color::BLACK,
                    position: Some(0.0),
                    end_position: None,
                    is_current_color: true,
                },
                ColorStop::new(Color::WHITE, Some(1.0), None),
            ],
            repeating: false,
        };
        let custom_color = Color::rgb(255, 120, 0);
        grad.resolve_current_color(custom_color);
        assert_eq!(grad.stops()[0].color, custom_color);

        // 5. Conic gradient rasterization
        let conic = Gradient::Conic {
            angle_deg: 0.0,
            stops: vec![
                ColorStop::new(Color::RED, Some(0.0), None),
                ColorStop::new(Color::BLUE, Some(1.0), None),
            ],
            repeating: false,
        };
        let conic_pixels = conic.rasterize(50, 50);
        assert_eq!(conic_pixels.len(), 2500);
        assert!(conic_pixels.iter().any(|&p| p != 0));

        // 6. Radial gradient rasterization
        let radial = Gradient::Radial {
            stops: vec![
                ColorStop::new(Color::RED, Some(0.0), None),
                ColorStop::new(Color::BLUE, Some(1.0), None),
            ],
            repeating: false,
        };
        let radial_pixels = radial.rasterize(40, 40);
        assert_eq!(radial_pixels.len(), 1600);
        assert!(radial_pixels.iter().any(|&p| p != 0));
    }
}

