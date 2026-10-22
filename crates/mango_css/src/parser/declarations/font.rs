//! Parsing for the CSS `font` shorthand and zero-length normalization.

use crate::parser::declarations::value::parse_single_value;
use crate::properties::Declaration;
use crate::tokenizer::Token;
use crate::values::{FontStyle, FontWeight, Length, Value};

/// Parses a CSS `font` shorthand declaration into constituent longhands:
/// `font-style`, `font-weight`, `font-size`, `line-height`, and `font-family`.
pub fn parse_font_shorthand(tokens: &[Token], important: bool) -> Vec<Declaration> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();

    if filtered.is_empty() {
        return Vec::new();
    }

    // Avoid breaking global keywords like inherit/initial/unset
    if filtered.len() == 1
        && let Token::Ident(kw) = filtered[0]
    {
        let lower = kw.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "inherit" | "initial" | "unset" | "revert" | "revert-layer"
        ) {
            return vec![Declaration::new("font", Value::Keyword(lower), important)];
        }
    }

    let mut font_style = None;
    let mut font_weight = None;
    let mut font_size = None;
    let mut line_height = None;
    let mut idx = 0;

    // 1. Consume optional prefix keywords: font-style, font-weight, font-variant, font-stretch
    while idx < filtered.len() {
        match filtered[idx] {
            Token::Ident(s) => {
                let lower = s.to_ascii_lowercase();
                if font_style.is_none() && matches!(lower.as_str(), "italic" | "oblique") {
                    font_style = Some(match lower.as_str() {
                        "italic" => FontStyle::Italic,
                        _ => FontStyle::Oblique,
                    });
                    idx += 1;
                } else if font_weight.is_none() && matches!(lower.as_str(), "bold" | "bolder" | "lighter") {
                    font_weight = Some(match lower.as_str() {
                        "bold" => FontWeight::Bold,
                        "bolder" => FontWeight::Bolder,
                        _ => FontWeight::Lighter,
                    });
                    idx += 1;
                } else if lower == "normal" {
                    idx += 1;
                } else if font_size.is_none()
                    && matches!(
                        lower.as_str(),
                        "xx-small"
                            | "x-small"
                            | "small"
                            | "medium"
                            | "large"
                            | "x-large"
                            | "xx-large"
                            | "xxx-large"
                            | "larger"
                            | "smaller"
                    )
                {
                    font_size = Some(Value::Keyword(lower));
                    idx += 1;
                    break;
                } else {
                    break;
                }
            }
            Token::Number(n) if font_weight.is_none() && (1.0..=1000.0).contains(n) => {
                font_weight = Some(FontWeight::Numeric(*n as u16));
                idx += 1;
            }
            Token::Dimension { .. } | Token::Percentage(_) if font_size.is_none() => {
                font_size = parse_single_value(filtered[idx]);
                idx += 1;
                break;
            }
            _ => break,
        }
    }

    // Must have found a font-size
    let Some(fs) = font_size else {
        return Vec::new();
    };

    // 2. Check for optional `/ <line-height>`
    if idx < filtered.len() && matches!(filtered[idx], Token::Delim('/')) {
        idx += 1;
        if idx < filtered.len() {
            match filtered[idx] {
                Token::Number(n) => {
                    line_height = Some(Value::Number(*n));
                    idx += 1;
                }
                Token::Dimension { .. } | Token::Percentage(_) => {
                    line_height = parse_single_value(filtered[idx]);
                    idx += 1;
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("normal") => {
                    idx += 1;
                }
                _ => {}
            }
        }
    }

    // 3. All remaining tokens form the font-family
    if idx >= filtered.len() {
        return Vec::new();
    }

    let mut family_names = Vec::new();
    let mut cur_name = String::new();

    for &tok in &filtered[idx..] {
        match tok {
            Token::Comma => {
                let trimmed = cur_name.trim().to_string();
                if !trimmed.is_empty() {
                    family_names.push(trimmed);
                    cur_name.clear();
                }
            }
            Token::String(s) => {
                if !cur_name.is_empty() {
                    cur_name.push(' ');
                }
                cur_name.push_str(s);
            }
            Token::Ident(s) => {
                if !cur_name.is_empty() {
                    cur_name.push(' ');
                }
                cur_name.push_str(s);
            }
            _ => {}
        }
    }
    let trimmed = cur_name.trim().to_string();
    if !trimmed.is_empty() {
        family_names.push(trimmed);
    }

    if family_names.is_empty() {
        return Vec::new();
    }

    let family_str = family_names.join(", ");

    vec![
        Declaration::new(
            "font-style",
            Value::FontStyle(font_style.unwrap_or(FontStyle::Normal)),
            important,
        ),
        Declaration::new(
            "font-weight",
            Value::FontWeight(font_weight.unwrap_or(FontWeight::Normal)),
            important,
        ),
        Declaration::new("font-size", fs, important),
        Declaration::new(
            "line-height",
            line_height.unwrap_or_else(|| Value::Keyword("normal".to_string())),
            important,
        ),
        Declaration::new("font-family", Value::String(family_str), important),
    ]
}

/// Normalizes unitless zero numbers to Length::Px(0.0) for length-accepting properties.
pub fn normalize_unitless_zero_lengths(prop: &str, val: Value) -> Value {
    let is_length_prop = matches!(
        prop,
        "margin"
            | "margin-top"
            | "margin-right"
            | "margin-bottom"
            | "margin-left"
            | "margin-block"
            | "margin-inline"
            | "padding"
            | "padding-top"
            | "padding-right"
            | "padding-bottom"
            | "padding-left"
            | "padding-block"
            | "padding-inline"
            | "inset"
            | "inset-block"
            | "inset-inline"
            | "inset-block-start"
            | "inset-block-end"
            | "inset-inline-start"
            | "inset-inline-end"
            | "top"
            | "right"
            | "bottom"
            | "left"
            | "width"
            | "min-width"
            | "max-width"
            | "height"
            | "min-height"
            | "max-height"
            | "flex-basis"
    );
    if !is_length_prop {
        return val;
    }
    match val {
        Value::Number(n) if n == 0.0 => Value::Length(Length::Px(0.0)),
        Value::List(list) => Value::List(
            list.into_iter()
                .map(|item| match item {
                    Value::Number(n) if n == 0.0 => Value::Length(Length::Px(0.0)),
                    other => other,
                })
                .collect(),
        ),
        other => other,
    }
}
