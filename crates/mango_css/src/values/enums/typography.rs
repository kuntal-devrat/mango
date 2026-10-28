//! Typography, font, text formatting, and internationalization enums.

use crate::values::length::Length;

/// CSS `text-align` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Left,
    Right,
    Center,
    Justify,
    End,
}

/// CSS `text-wrap` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    #[default]
    Wrap,
    Nowrap,
    Balance,
    Pretty,
}

impl TextWrap {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "wrap" => Some(Self::Wrap),
            "nowrap" => Some(Self::Nowrap),
            "balance" => Some(Self::Balance),
            "pretty" => Some(Self::Pretty),
            _ => None,
        }
    }
}

/// CSS `font-weight` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// CSS `text-decoration-line` bitflags value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextDecorationLine(pub u8);

/// Backwards compatibility alias for `TextDecorationLine`.
pub type TextDecoration = TextDecorationLine;

impl TextDecorationLine {
    pub const NONE: Self = Self(0);
    pub const UNDERLINE: Self = Self(1 << 0);
    pub const OVERLINE: Self = Self(1 << 1);
    pub const LINE_THROUGH: Self = Self(1 << 2);
    pub const BLINK: Self = Self(1 << 3);
    pub const SPELLING_ERROR: Self = Self(1 << 4);
    pub const GRAMMAR_ERROR: Self = Self(1 << 5);

    #[allow(non_upper_case_globals)]
    pub const None: Self = Self::NONE;
    #[allow(non_upper_case_globals)]
    pub const Underline: Self = Self::UNDERLINE;
    #[allow(non_upper_case_globals)]
    pub const Overline: Self = Self::OVERLINE;
    #[allow(non_upper_case_globals)]
    pub const LineThrough: Self = Self::LINE_THROUGH;
    #[allow(non_upper_case_globals)]
    pub const Blink: Self = Self::BLINK;
    #[allow(non_upper_case_globals)]
    pub const SpellingError: Self = Self::SPELLING_ERROR;
    #[allow(non_upper_case_globals)]
    pub const GrammarError: Self = Self::GRAMMAR_ERROR;

    #[inline]
    pub fn is_none(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn has_underline(&self) -> bool {
        (self.0 & Self::UNDERLINE.0) != 0
    }

    #[inline]
    pub fn has_overline(&self) -> bool {
        (self.0 & Self::OVERLINE.0) != 0
    }

    #[inline]
    pub fn has_line_through(&self) -> bool {
        (self.0 & Self::LINE_THROUGH.0) != 0
    }

    #[inline]
    pub fn has_blink(&self) -> bool {
        (self.0 & Self::BLINK.0) != 0
    }

    #[inline]
    pub fn has_spelling_error(&self) -> bool {
        (self.0 & Self::SPELLING_ERROR.0) != 0
    }

    #[inline]
    pub fn has_grammar_error(&self) -> bool {
        (self.0 & Self::GRAMMAR_ERROR.0) != 0
    }

    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }
        if tokens.len() == 1 {
            let lower = tokens[0].to_ascii_lowercase();
            match lower.as_str() {
                "none" => return Some(Self::NONE),
                "spelling-error" => return Some(Self::SPELLING_ERROR),
                "grammar-error" => return Some(Self::GRAMMAR_ERROR),
                _ => {}
            }
        }
        let mut flags = 0u8;
        for token in tokens {
            let lower = token.to_ascii_lowercase();
            match lower.as_str() {
                "underline" => {
                    if (flags & Self::UNDERLINE.0) != 0 {
                        return None;
                    }
                    flags |= Self::UNDERLINE.0;
                }
                "overline" => {
                    if (flags & Self::OVERLINE.0) != 0 {
                        return None;
                    }
                    flags |= Self::OVERLINE.0;
                }
                "line-through" => {
                    if (flags & Self::LINE_THROUGH.0) != 0 {
                        return None;
                    }
                    flags |= Self::LINE_THROUGH.0;
                }
                "blink" => {
                    if (flags & Self::BLINK.0) != 0 {
                        return None;
                    }
                    flags |= Self::BLINK.0;
                }
                _ => return None,
            }
        }
        if flags == 0 { None } else { Some(Self(flags)) }
    }

    pub fn to_css_string(&self) -> String {
        if self.0 == Self::NONE.0 {
            return "none".to_string();
        }
        if self.0 == Self::SPELLING_ERROR.0 {
            return "spelling-error".to_string();
        }
        if self.0 == Self::GRAMMAR_ERROR.0 {
            return "grammar-error".to_string();
        }
        let mut parts = Vec::new();
        if self.has_underline() {
            parts.push("underline");
        }
        if self.has_overline() {
            parts.push("overline");
        }
        if self.has_line_through() {
            parts.push("line-through");
        }
        if self.has_blink() {
            parts.push("blink");
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" ")
        }
    }
}

/// CSS `text-decoration-style` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecorationStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

impl TextDecorationStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "solid" => Some(Self::Solid),
            "double" => Some(Self::Double),
            "dotted" => Some(Self::Dotted),
            "dashed" => Some(Self::Dashed),
            "wavy" => Some(Self::Wavy),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Double => "double",
            Self::Dotted => "dotted",
            Self::Dashed => "dashed",
            Self::Wavy => "wavy",
        }
    }
}

/// CSS `text-transform` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

impl TextTransform {
    /// Applies this transformation to a string slice, returning a new `String`.
    pub fn apply(&self, text: &str) -> String {
        match self {
            TextTransform::None => text.to_string(),
            TextTransform::Uppercase => text.to_uppercase(),
            TextTransform::Lowercase => text.to_lowercase(),
            TextTransform::Capitalize => {
                let mut result = String::with_capacity(text.len());
                let mut capitalize_next = true;
                for ch in text.chars() {
                    if ch.is_whitespace() || ch.is_ascii_punctuation() {
                        capitalize_next = true;
                        result.push(ch);
                    } else if capitalize_next {
                        result.extend(ch.to_uppercase());
                        capitalize_next = false;
                    } else {
                        result.push(ch);
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
        } else {
            Length::parse(trimmed).map(Self::Length)
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
