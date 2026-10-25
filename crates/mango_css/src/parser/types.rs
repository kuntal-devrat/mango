use crate::properties::Declaration;
use crate::selectors::SelectorList;
use crate::values::{FontDisplay, FontStyle, FontWeight, Value};

/// A parsed CSS stylesheet containing style rules and at-rules.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
    pub scoping_element: Option<mango_html::dom::NodeId>,
}

impl Stylesheet {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            scoping_element: None,
        }
    }
}

/// A top-level rule inside a stylesheet.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    Style(StyleRule),
    Media(MediaRule),
    Container(ContainerRule),
    Layer(LayerRule),
    Scope(ScopeRule),
    Import(ImportRule),
    FontFace(FontFaceRule),
    /// `@keyframes name { ... }` animation rule.
    Keyframes(KeyframesRule),
    /// `@position-try --name { ... }` anchor fallback option rule.
    PositionTry(PositionTryRule),
    /// `@view-transition { navigation: auto; }` cross-document transition rule.
    ViewTransition(ViewTransitionRule),
    /// `@property --name { syntax: ...; inherits: ...; initial-value: ...; }` CSS Properties and Values API rule.
    Property(Box<PropertyRule>),
}

/// A single keyframe rule inside `@keyframes`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeRule {
    /// Keyframe offsets as percentages (0.0 = from, 100.0 = to).
    pub offsets: Vec<f32>,
    pub declarations: Vec<Declaration>,
}

/// A complete `@keyframes` at-rule.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframesRule {
    pub name: String,
    pub keyframes: Vec<KeyframeRule>,
}

/// A parsed `@font-face` at-rule describing a downloadable web font.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFaceRule {
    pub font_family: String,
    pub src_url: String,
    pub font_weight: FontWeight,
    pub font_weight_range: Option<(u16, u16)>,
    pub font_style: FontStyle,
    pub font_display: FontDisplay,
}

/// A CSS style rule consisting of a selector list and property declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
    pub selectors: SelectorList,
    pub declarations: Vec<Declaration>,
}

/// An `@media` at-rule wrapping nested style rules.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaRule {
    pub query: String,
    pub rules: Vec<StyleRule>,
}

/// An `@container` at-rule wrapping nested style rules.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerRule {
    pub name: Option<String>,
    pub query: String,
    pub rules: Vec<StyleRule>,
}

/// An `@layer` at-rule wrapping nested style or at-rules.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerRule {
    pub name: Option<String>,
    pub rules: Vec<Rule>,
}

/// An `@scope [(<scope-start>)] [to (<scope-end>)]` at-rule wrapping nested style rules.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeRule {
    pub root: Option<SelectorList>,
    pub limit: Option<SelectorList>,
    pub rules: Vec<Rule>,
}

/// An `@position-try <dashed-ident>` at-rule for CSS Anchor Positioning fallbacks.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionTryRule {
    pub name: String,
    pub declarations: Vec<Declaration>,
}

/// An `@view-transition` at-rule for cross-document / single-page view transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewTransitionRule {
    pub navigation: String,
    pub types: Vec<String>,
}

/// An `@property <custom-property-name>` at-rule (CSS Properties and Values API Level 1).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRule {
    pub name: String,
    pub syntax: Option<String>,
    pub inherits: bool,
    pub initial_value: Option<Value>,
}

/// An `@import` at-rule with optional layer, supports, and media conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportRule {
    pub url: String,
    pub layer: Option<String>,
    pub supports: Option<String>,
    pub media: Option<String>,
    pub supports_matches: bool,
}

impl ImportRule {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            layer: None,
            supports: None,
            media: None,
            supports_matches: true,
        }
    }
}
