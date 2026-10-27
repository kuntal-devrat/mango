//! Visual effects, borders, backgrounds, shadows, masks, filters, and blend mode enums.

use mango_core::Color;

use crate::values::gradient::Gradient;
use crate::values::length::Length;

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

/// CSS `background-size` property values.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BackgroundSize {
    #[default]
    Auto,
    Cover,
    Contain,
    Explicit(Length, Length),
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

/// CSS `text-shadow` property value.
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
    DropShadow {
        offset_x: f32,
        offset_y: f32,
        blur: f32,
        color: Color,
    },
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

/// CSS `clip-path` shapes per CSS Masking Module Level 1.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ClipPath {
    #[default]
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

/// CSS `mask-mode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskMode {
    #[default]
    MatchSource,
    Alpha,
    Luminance,
}

/// CSS 2.1 `clip` property per CSS 2.1 § 11.1.2.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ClipRect {
    #[default]
    Auto,
    Rect {
        top: Length,
        right: Length,
        bottom: Length,
        left: Length,
    },
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
