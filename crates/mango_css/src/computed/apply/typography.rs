//! Font, text, and typography property application.

use std::collections::HashMap;

use crate::computed::style::ComputedStyle;
use crate::values::{
    Clear, Direction, Float, FontStyle, FontWeight, JustifyContent, Length, TextAlign,
    TextDecorationLine, TextDecorationStyle, TextOverflow, TextTransform, UnicodeBidi, Value,
    WhiteSpace,
};

/// Pre-applies `font-size` early so subsequent length values (like `em`) can resolve against it.
pub(crate) fn pre_apply_font_size(
    style: &mut ComputedStyle,
    resolved_cascaded: &HashMap<String, Value>,
    parent_style: Option<&ComputedStyle>,
) {
    if let Some(val) = resolved_cascaded.get("font-size") {
        if super::css_wide::apply_css_wide_keyword(style, "font-size", val, parent_style) {
            return;
        }
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
}

/// Applies typography, text formatting, and font properties.
pub(crate) fn apply_typography_property(
    style: &mut ComputedStyle,
    prop: &str,
    val: &Value,
) -> bool {
    match prop {
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
            Value::Percentage(pct) => style.line_height = Some((*pct / 100.0) * style.font_size),
            Value::Length(len) => {
                style.line_height = Some(len.to_px_with_viewport(
                    style.font_size,
                    style.root_font_size,
                    style.font_size,
                    style.font_size,
                ))
            }
            _ => {}
        },
        "text-align" => match val {
            Value::TextAlign(ta) => style.text_align = *ta,
            Value::Float(Float::Left)
            | Value::Clear(Clear::Left)
            | Value::JustifyContent(JustifyContent::Left) => {
                style.text_align = TextAlign::Left;
            }
            Value::Float(Float::Right)
            | Value::Clear(Clear::Right)
            | Value::JustifyContent(JustifyContent::Right) => {
                style.text_align = TextAlign::Right;
            }
            Value::JustifyContent(JustifyContent::Start) => {
                style.text_align = if style.direction == Direction::Rtl {
                    TextAlign::Right
                } else {
                    TextAlign::Left
                };
            }
            Value::JustifyContent(JustifyContent::End) => {
                style.text_align = if style.direction == Direction::Rtl {
                    TextAlign::Left
                } else {
                    TextAlign::Right
                };
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "left" => style.text_align = TextAlign::Left,
                "right" => style.text_align = TextAlign::Right,
                "start" => {
                    style.text_align = if style.direction == Direction::Rtl {
                        TextAlign::Right
                    } else {
                        TextAlign::Left
                    };
                }
                "end" => {
                    style.text_align = if style.direction == Direction::Rtl {
                        TextAlign::Left
                    } else {
                        TextAlign::Right
                    };
                }
                "center" => style.text_align = TextAlign::Center,
                "justify" => style.text_align = TextAlign::Justify,
                _ => {}
            },
            _ => {}
        },
        "text-decoration" | "text-decoration-line" => match val {
            Value::TextDecoration(td) => style.text_decoration = *td,
            Value::Keyword(k) => {
                if let Some(line) = TextDecorationLine::parse(k) {
                    style.text_decoration = line;
                }
            }
            _ => {}
        },
        "text-decoration-style" => match val {
            Value::TextDecorationStyle(tds) => style.text_decoration_style = *tds,
            Value::Keyword(k) => {
                if let Some(s) = TextDecorationStyle::parse(k) {
                    style.text_decoration_style = s;
                }
            }
            _ => {}
        },
        "text-decoration-color" => match val {
            Value::Color(c) => style.text_decoration_color = Some(*c),
            Value::CurrentColor => style.text_decoration_color = None,
            Value::Keyword(k) => {
                if k.eq_ignore_ascii_case("currentcolor") {
                    style.text_decoration_color = None;
                } else if let Some(c) = Value::parse_color(k) {
                    style.text_decoration_color = Some(c);
                }
            }
            _ => {}
        },
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
        "white-space" => match val {
            Value::WhiteSpace(ws) => style.white_space = *ws,
            Value::FlexWrap(crate::values::FlexWrap::NoWrap) => {
                style.white_space = WhiteSpace::Nowrap
            }
            Value::FlexWrap(crate::values::FlexWrap::Wrap) => {
                style.white_space = WhiteSpace::Normal
            }
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
        "content" => match val {
            Value::Content(items) => {
                if items.is_empty() {
                    style.content = None;
                    style.content_items = None;
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
                style.content_items = None;
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
        "text-shadow" => match val {
            Value::TextShadow(ts) => {
                style.text_shadow = Some(*ts);
                style.text_shadows = vec![*ts];
            }
            Value::List(items) => {
                let list: Vec<crate::values::TextShadow> = items
                    .iter()
                    .filter_map(|it| match it {
                        Value::TextShadow(ts) => Some(*ts),
                        _ => None,
                    })
                    .collect();
                if !list.is_empty() {
                    style.text_shadow = list.first().copied();
                    style.text_shadows = list;
                }
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                style.text_shadow = None;
                style.text_shadows.clear();
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
            Value::TextDecorationThickness(tdt) => style.text_decoration_thickness = tdt.clone(),
            Value::Length(l) => {
                style.text_decoration_thickness = crate::values::TextDecorationThickness::Length(*l)
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                style.text_decoration_thickness = crate::values::TextDecorationThickness::Auto
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("from-font") => {
                style.text_decoration_thickness = crate::values::TextDecorationThickness::FromFont
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
        "text-wrap" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(tw) = crate::values::TextWrap::parse(k) {
                    style.text_wrap = tw;
                }
            }
            Value::FlexWrap(crate::values::FlexWrap::Balance) => {
                style.text_wrap = crate::values::TextWrap::Balance;
            }
            _ => {}
        },
        _ => return false,
    }
    true
}
