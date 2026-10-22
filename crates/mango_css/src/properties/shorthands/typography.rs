//! Shorthand expansion for typography, columns, masks, and containers.

use crate::properties::Declaration;
use crate::values::{
    Length, TextDecorationLine, TextDecorationStyle, TextDecorationThickness, Value,
};

fn serialize_value_for_font(value: &Value) -> String {
    match value {
        Value::String(s) => {
            if s.contains(' ') || s.contains(',') {
                format!("\"{s}\"")
            } else {
                s.clone()
            }
        }
        Value::Keyword(s) => s.clone(),
        Value::Number(n) => format!("{n}"),
        Value::Percentage(p) => format!("{p}%"),
        Value::Length(len) => match len {
            crate::values::Length::Px(px) => format!("{px}px"),
            crate::values::Length::Em(em) => format!("{em}em"),
            crate::values::Length::Rem(rem) => format!("{rem}rem"),
            crate::values::Length::Percent(p) => format!("{p}%"),
            crate::values::Length::Vw(vw) => format!("{vw}vw"),
            crate::values::Length::Vh(vh) => format!("{vh}vh"),
            crate::values::Length::Vmin(vm) => format!("{vm}vmin"),
            crate::values::Length::Vmax(vm) => format!("{vm}vmax"),
            crate::values::Length::Auto => "auto".to_string(),
            _ => format!("{len:?}"),
        },
        Value::FontWeight(w) => match w {
            crate::values::FontWeight::Bold => "bold".to_string(),
            crate::values::FontWeight::Bolder => "bolder".to_string(),
            crate::values::FontWeight::Lighter => "lighter".to_string(),
            crate::values::FontWeight::Normal => "normal".to_string(),
            crate::values::FontWeight::Numeric(n) => format!("{n}"),
        },
        Value::FontStyle(s) => match s {
            crate::values::FontStyle::Normal => "normal".to_string(),
            crate::values::FontStyle::Italic => "italic".to_string(),
            crate::values::FontStyle::Oblique => "oblique".to_string(),
        },
        Value::List(items) => items
            .iter()
            .map(serialize_value_for_font)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Expands `font` shorthand into font-style, font-weight, font-size, line-height, font-family.
pub(crate) fn expand_font_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let text = match value {
        Value::String(s) | Value::Keyword(s) => s.clone(),
        Value::List(_) => serialize_value_for_font(value),
        _ => return vec![Declaration::new("font", value.clone(), important)],
    };
    let tokens = crate::tokenizer::CssTokenizer::tokenize(&text);
    let parser = crate::parser::CssParser::new(tokens.clone());
    let decls = parser.parse_font_shorthand(&tokens, important);
    if decls.is_empty() {
        vec![Declaration::new("font", value.clone(), important)]
    } else {
        decls
    }
}

/// Expands `container` shorthand into `container-name` and `container-type`.
pub(crate) fn expand_container_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    match value {
        Value::List(items) if items.len() >= 2 => {
            vec![
                Declaration::new("container-name", items[0].clone(), important),
                Declaration::new("container-type", items[1].clone(), important),
            ]
        }
        Value::ContainerType(ct) => {
            vec![
                Declaration::new(
                    "container-name",
                    Value::Keyword("none".to_string()),
                    important,
                ),
                Declaration::new("container-type", Value::ContainerType(*ct), important),
            ]
        }
        Value::String(s) | Value::Keyword(s) => {
            if let Some(ct) = crate::values::ContainerType::parse(s) {
                vec![
                    Declaration::new(
                        "container-name",
                        Value::Keyword("none".to_string()),
                        important,
                    ),
                    Declaration::new("container-type", Value::ContainerType(ct), important),
                ]
            } else {
                vec![
                    Declaration::new("container-name", Value::String(s.clone()), important),
                    Declaration::new(
                        "container-type",
                        Value::ContainerType(crate::values::ContainerType::Normal),
                        important,
                    ),
                ]
            }
        }
        _ => vec![Declaration::new("container", value.clone(), important)],
    }
}

/// Expands `contain-intrinsic-size` into `contain-intrinsic-width` and `contain-intrinsic-height`.
pub(crate) fn expand_contain_intrinsic_size_shorthand(
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    match value {
        Value::List(items) if items.len() >= 2 => {
            vec![
                Declaration::new("contain-intrinsic-width", items[0].clone(), important),
                Declaration::new("contain-intrinsic-height", items[1].clone(), important),
            ]
        }
        single => {
            vec![
                Declaration::new("contain-intrinsic-width", single.clone(), important),
                Declaration::new("contain-intrinsic-height", single.clone(), important),
            ]
        }
    }
}

/// Expands `text-emphasis: <style> || <color>` into `text-emphasis-style` and `text-emphasis-color`.
pub(crate) fn expand_text_emphasis_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    use crate::values::TextEmphasisStyle;
    let mut style_val = None;
    let mut color_val = None;

    let items = match value {
        Value::List(l) => l.clone(),
        single => vec![single.clone()],
    };

    let mut words = Vec::new();
    for item in &items {
        match item {
            Value::Color(c) => color_val = Some(Value::Color(*c)),
            Value::TextEmphasisStyle(tes) => {
                style_val = Some(Value::TextEmphasisStyle(tes.clone()))
            }
            Value::Keyword(k) => {
                if let Some(c) = Value::parse_color(k) {
                    color_val = Some(Value::Color(c));
                } else {
                    words.push(k.as_str());
                }
            }
            Value::String(s) => {
                if let Some(tes) = TextEmphasisStyle::parse(s) {
                    style_val = Some(Value::TextEmphasisStyle(tes));
                } else {
                    words.push(s.as_str());
                }
            }
            Value::ListStyleType(crate::values::ListStyleType::Circle) => {
                words.push("circle");
            }
            Value::ListStyleType(crate::values::ListStyleType::Disc) => {
                words.push("dot");
            }
            Value::ListStyleType(crate::values::ListStyleType::Square) => {
                words.push("square");
            }
            _ => {}
        }
    }

    if style_val.is_none() && !words.is_empty() {
        let combined = words.join(" ");
        if let Some(tes) = TextEmphasisStyle::parse(&combined) {
            style_val = Some(Value::TextEmphasisStyle(tes));
        } else {
            for word in &words {
                if let Some(tes) = TextEmphasisStyle::parse(word) {
                    style_val = Some(Value::TextEmphasisStyle(tes));
                    break;
                }
            }
        }
    }

    let mut decls = Vec::new();
    if let Some(sv) = style_val {
        decls.push(Declaration::new("text-emphasis-style", sv, important));
    }
    if let Some(cv) = color_val {
        decls.push(Declaration::new("text-emphasis-color", cv, important));
    }
    if decls.is_empty() {
        vec![Declaration::new(
            "text-emphasis-style",
            Value::Keyword("none".to_string()),
            important,
        )]
    } else {
        decls
    }
}

/// Expands `columns: <column-width> || <column-count>` (e.g. `columns: 2`, `columns: 20em`, `columns: 2 20em`, `columns: auto`).
pub(crate) fn expand_columns_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut col_width = None;
    let mut col_count = None;

    for item in items {
        match item {
            Value::Length(len) => {
                col_width = Some(Value::Length(*len));
            }
            Value::Number(n) => {
                col_count = Some(Value::Number(*n));
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                if col_width.is_none() {
                    col_width = Some(Value::Keyword("auto".to_string()));
                } else {
                    col_count = Some(Value::Keyword("auto".to_string()));
                }
            }
            _ => {}
        }
    }

    let width_val = col_width.unwrap_or(Value::Keyword("auto".to_string()));
    let count_val = col_count.unwrap_or(Value::Keyword("auto".to_string()));

    vec![
        Declaration::new("column-width", width_val, important),
        Declaration::new("column-count", count_val, important),
    ]
}

/// Expands `mask` or `-webkit-mask` shorthand into mask-image, mask-mode, mask-repeat, mask-position, mask-size.
pub(crate) fn expand_mask_shorthand(
    prefix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut decls = Vec::new();
    let mut image = None;
    let mut mode = None;
    let mut repeat = None;
    let mut size = None;
    let mut positions = Vec::new();

    let img_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-image"
    } else {
        "mask-image"
    };
    let mode_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-mode"
    } else {
        "mask-mode"
    };
    let repeat_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-repeat"
    } else {
        "mask-repeat"
    };
    let size_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-size"
    } else {
        "mask-size"
    };
    let pos_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-position"
    } else {
        "mask-position"
    };

    for item in items {
        match item {
            Value::Url(_) | Value::Gradient(_) => image = Some(item.clone()),
            Value::MaskMode(_) => mode = Some(item.clone()),
            Value::BackgroundRepeat(_) => repeat = Some(item.clone()),
            Value::BackgroundSize(_) => size = Some(item.clone()),
            Value::Length(_) | Value::Percentage(_) => positions.push(item.clone()),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" => image = Some(Value::Keyword("none".to_string())),
                "alpha" | "luminance" | "match-source" => {
                    if let Some(m) = crate::values::MaskMode::parse(k) {
                        mode = Some(Value::MaskMode(m));
                    }
                }
                "repeat" | "repeat-x" | "repeat-y" | "no-repeat" => repeat = Some(item.clone()),
                "cover" | "contain" => size = Some(item.clone()),
                _ => {}
            },
            _ => {}
        }
    }

    if let Some(img) = image {
        decls.push(Declaration::new(img_prop, img, important));
    }
    if let Some(m) = mode {
        decls.push(Declaration::new(mode_prop, m, important));
    }
    if let Some(r) = repeat {
        decls.push(Declaration::new(repeat_prop, r, important));
    }
    if let Some(s) = size {
        decls.push(Declaration::new(size_prop, s, important));
    }
    if !positions.is_empty() {
        if positions.len() == 1 {
            decls.push(Declaration::new(pos_prop, positions[0].clone(), important));
        } else {
            decls.push(Declaration::new(
                pos_prop,
                Value::List(positions),
                important,
            ));
        }
    }

    if decls.is_empty() {
        vec![Declaration::new(prefix, value.clone(), important)]
    } else {
        decls
    }
}

pub(crate) fn split_shorthand_tokens(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    for ch in text.chars() {
        if ch == '(' {
            depth += 1;
            current.push(ch);
        } else if ch == ')' {
            if depth > 0 {
                depth -= 1;
            }
            current.push(ch);
        } else if ch.is_whitespace() && depth == 0 {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                items.push(trimmed.to_string());
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        items.push(trimmed.to_string());
    }
    items
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTextDecorationShorthand {
    pub line: TextDecorationLine,
    pub thickness: TextDecorationThickness,
    pub style: TextDecorationStyle,
    pub color: Value,
}

pub fn parse_text_decoration_shorthand(text: &str) -> Option<ParsedTextDecorationShorthand> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let items = split_shorthand_tokens(trimmed);
    if items.is_empty() {
        return None;
    }

    let mut line_flags = 0u8;
    let mut line_special: Option<TextDecorationLine> = None;
    let mut has_line = false;

    let mut style: Option<TextDecorationStyle> = None;
    let mut thickness: Option<TextDecorationThickness> = None;
    let mut color: Option<Value> = None;

    for item in items {
        let lower = item.to_ascii_lowercase();
        // Check line keywords
        match lower.as_str() {
            "none" => {
                if has_line || line_flags != 0 || line_special.is_some() {
                    return None;
                }
                line_special = Some(TextDecorationLine::NONE);
                has_line = true;
                continue;
            }
            "spelling-error" => {
                if has_line || line_flags != 0 || line_special.is_some() {
                    return None;
                }
                line_special = Some(TextDecorationLine::SPELLING_ERROR);
                has_line = true;
                continue;
            }
            "grammar-error" => {
                if has_line || line_flags != 0 || line_special.is_some() {
                    return None;
                }
                line_special = Some(TextDecorationLine::GRAMMAR_ERROR);
                has_line = true;
                continue;
            }
            "underline" => {
                if line_special.is_some() || (line_flags & TextDecorationLine::UNDERLINE.0) != 0 {
                    return None;
                }
                line_flags |= TextDecorationLine::UNDERLINE.0;
                has_line = true;
                continue;
            }
            "overline" => {
                if line_special.is_some() || (line_flags & TextDecorationLine::OVERLINE.0) != 0 {
                    return None;
                }
                line_flags |= TextDecorationLine::OVERLINE.0;
                has_line = true;
                continue;
            }
            "line-through" => {
                if line_special.is_some() || (line_flags & TextDecorationLine::LINE_THROUGH.0) != 0
                {
                    return None;
                }
                line_flags |= TextDecorationLine::LINE_THROUGH.0;
                has_line = true;
                continue;
            }
            "blink" => {
                if line_special.is_some() || (line_flags & TextDecorationLine::BLINK.0) != 0 {
                    return None;
                }
                line_flags |= TextDecorationLine::BLINK.0;
                has_line = true;
                continue;
            }
            _ => {}
        }

        // Check style keywords
        if let Some(s) = TextDecorationStyle::parse(&item) {
            if style.is_some() {
                return None;
            }
            style = Some(s);
            continue;
        }

        // Check thickness
        if lower == "auto" {
            if thickness.is_some() {
                return None;
            }
            thickness = Some(TextDecorationThickness::Auto);
            continue;
        }
        if lower == "from-font" {
            if thickness.is_some() {
                return None;
            }
            thickness = Some(TextDecorationThickness::FromFont);
            continue;
        }
        if let Some(len) = Length::parse(&item) {
            if thickness.is_some() {
                return None;
            }
            thickness = Some(TextDecorationThickness::Length(len));
            continue;
        }

        // Check color
        if lower == "currentcolor" {
            if color.is_some() {
                return None;
            }
            color = Some(Value::CurrentColor);
            continue;
        }
        if Value::parse_color(&item).is_some() {
            if color.is_some() {
                return None;
            }
            color = Some(Value::Keyword(item.clone()));
            continue;
        }

        // Unrecognized token
        return None;
    }

    let line = if let Some(sp) = line_special {
        sp
    } else {
        TextDecorationLine(line_flags)
    };

    Some(ParsedTextDecorationShorthand {
        line,
        thickness: thickness.unwrap_or(TextDecorationThickness::Auto),
        style: style.unwrap_or(TextDecorationStyle::Solid),
        color: color.unwrap_or(Value::CurrentColor),
    })
}

/// Expands `text-decoration` shorthand into text-decoration-line, text-decoration-thickness, text-decoration-style, text-decoration-color.
pub(crate) fn expand_text_decoration_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    if crate::properties::validation::is_css_wide_keyword(value) {
        return vec![
            Declaration::new("text-decoration-line", value.clone(), important),
            Declaration::new("text-decoration-thickness", value.clone(), important),
            Declaration::new("text-decoration-style", value.clone(), important),
            Declaration::new("text-decoration-color", value.clone(), important),
        ];
    }
    if let Value::Var { .. } = value {
        return vec![Declaration::new(
            "text-decoration",
            value.clone(),
            important,
        )];
    }
    let text = match value {
        Value::String(s) | Value::Keyword(s) => s.clone(),
        Value::TextDecoration(td) => td.to_css_string(),
        Value::List(_) => value.to_css_string(),
        _ => return vec![],
    };
    if let Some(parsed) = parse_text_decoration_shorthand(&text) {
        vec![
            Declaration::new(
                "text-decoration-line",
                Value::TextDecoration(parsed.line),
                important,
            ),
            Declaration::new(
                "text-decoration-thickness",
                Value::TextDecorationThickness(parsed.thickness),
                important,
            ),
            Declaration::new(
                "text-decoration-style",
                Value::TextDecorationStyle(parsed.style),
                important,
            ),
            Declaration::new("text-decoration-color", parsed.color, important),
        ]
    } else {
        vec![]
    }
}
