use crate::parser::declarations::functions::parse_url_fn;
use crate::parser::declarations::gradient::token_to_string;
use crate::properties::shorthands::typography::parse_text_decoration_shorthand;
use crate::tokenizer::Token;
use crate::values::{
    ContentItem, CounterAction, FontFeatureSettings, FontVariationSettings, Hyphens, Length,
    LineClamp, OverflowWrap, TextDecorationLine, TextDecorationStyle, TextDecorationThickness,
    TextEmphasisStyle, Value, WhiteSpace, WordBreak,
};

pub fn try_parse_word_break(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    WordBreak::parse(&text).map(Value::WordBreak)
}

pub fn try_parse_overflow_wrap(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    OverflowWrap::parse(&text).map(Value::OverflowWrap)
}

pub fn try_parse_hyphens(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    Hyphens::parse(&text).map(Value::Hyphens)
}

pub fn try_parse_line_clamp(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    LineClamp::parse(&text).map(Value::LineClamp)
}

pub fn try_parse_font_variation_settings(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    FontVariationSettings::parse(&text).map(Value::FontVariationSettings)
}

pub fn try_parse_font_feature_settings(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    FontFeatureSettings::parse(&text).map(Value::FontFeatureSettings)
}

pub fn try_parse_text_emphasis_style(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    TextEmphasisStyle::parse(&text).map(Value::TextEmphasisStyle)
}

pub fn try_parse_text_decoration_thickness(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("initial")
        || trimmed.eq_ignore_ascii_case("inherit")
        || trimmed.eq_ignore_ascii_case("unset")
        || trimmed.eq_ignore_ascii_case("revert")
        || trimmed.eq_ignore_ascii_case("revert-layer")
    {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    TextDecorationThickness::parse(trimmed).map(Value::TextDecorationThickness)
}

pub fn try_parse_text_decoration_line(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("initial")
        || trimmed.eq_ignore_ascii_case("inherit")
        || trimmed.eq_ignore_ascii_case("unset")
        || trimmed.eq_ignore_ascii_case("revert")
        || trimmed.eq_ignore_ascii_case("revert-layer")
    {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    TextDecorationLine::parse(trimmed).map(Value::TextDecoration)
}

pub fn try_parse_text_decoration_style(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("initial")
        || trimmed.eq_ignore_ascii_case("inherit")
        || trimmed.eq_ignore_ascii_case("unset")
        || trimmed.eq_ignore_ascii_case("revert")
        || trimmed.eq_ignore_ascii_case("revert-layer")
    {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    TextDecorationStyle::parse(trimmed).map(Value::TextDecorationStyle)
}

pub fn try_parse_text_decoration_color(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("initial")
        || trimmed.eq_ignore_ascii_case("inherit")
        || trimmed.eq_ignore_ascii_case("unset")
        || trimmed.eq_ignore_ascii_case("revert")
        || trimmed.eq_ignore_ascii_case("revert-layer")
    {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    if trimmed.eq_ignore_ascii_case("currentcolor") {
        Some(Value::CurrentColor)
    } else {
        Value::parse_color(trimmed).map(Value::Color)
    }
}

pub fn try_parse_text_underline_offset(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("initial")
        || trimmed.eq_ignore_ascii_case("inherit")
        || trimmed.eq_ignore_ascii_case("unset")
        || trimmed.eq_ignore_ascii_case("revert")
        || trimmed.eq_ignore_ascii_case("revert-layer")
    {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    if trimmed.eq_ignore_ascii_case("auto") {
        Some(Value::Length(Length::Auto))
    } else {
        Length::parse(trimmed).map(Value::Length)
    }
}

pub fn try_parse_text_decoration(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("initial")
        || trimmed.eq_ignore_ascii_case("inherit")
        || trimmed.eq_ignore_ascii_case("unset")
        || trimmed.eq_ignore_ascii_case("revert")
        || trimmed.eq_ignore_ascii_case("revert-layer")
    {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    if trimmed.starts_with("var(") {
        return Some(Value::Keyword(trimmed.to_string()));
    }
    parse_text_decoration_shorthand(trimmed).map(|_| Value::String(trimmed.to_string()))
}

/// Tries to parse `white-space` keywords (normal, nowrap, pre, pre-wrap, pre-line, break-spaces).
pub fn try_parse_white_space(tokens: &[Token]) -> Option<Value> {
    let first = tokens
        .iter()
        .find(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))?;
    if let Token::Ident(ident) = first {
        match ident.to_ascii_lowercase().as_str() {
            "normal" => Some(Value::WhiteSpace(WhiteSpace::Normal)),
            "nowrap" => Some(Value::WhiteSpace(WhiteSpace::Nowrap)),
            "pre" => Some(Value::WhiteSpace(WhiteSpace::Pre)),
            "pre-wrap" => Some(Value::WhiteSpace(WhiteSpace::PreWrap)),
            "pre-line" => Some(Value::WhiteSpace(WhiteSpace::PreLine)),
            "break-spaces" => Some(Value::WhiteSpace(WhiteSpace::BreakSpaces)),
            _ => None,
        }
    } else {
        None
    }
}

/// Tries to parse `content` property for generated content (`::before` / `::after`).
pub fn try_parse_content(tokens: &[Token]) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    if filtered.is_empty() {
        return None;
    }

    if filtered.len() == 1
        && let Token::Ident(kw) = filtered[0]
        && (kw.eq_ignore_ascii_case("none") || kw.eq_ignore_ascii_case("normal"))
    {
        return Some(Value::Content(Vec::new()));
    }

    let mut items = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if matches!(tokens[i], Token::Whitespace | Token::Comment(_)) {
            i += 1;
            continue;
        }

        match &tokens[i] {
            Token::String(s) => {
                items.push(ContentItem::String(s.clone()));
                i += 1;
            }
            Token::Ident(id) if id.eq_ignore_ascii_case("open-quote") => {
                items.push(ContentItem::OpenQuote);
                i += 1;
            }
            Token::Ident(id) if id.eq_ignore_ascii_case("close-quote") => {
                items.push(ContentItem::CloseQuote);
                i += 1;
            }
            Token::Ident(id) if id.eq_ignore_ascii_case("no-open-quote") => {
                items.push(ContentItem::NoOpenQuote);
                i += 1;
            }
            Token::Ident(id) if id.eq_ignore_ascii_case("no-close-quote") => {
                items.push(ContentItem::NoCloseQuote);
                i += 1;
            }
            Token::Ident(id)
                if id.eq_ignore_ascii_case("none") || id.eq_ignore_ascii_case("normal") =>
            {
                i += 1;
            }
            Token::Ident(fn_name) if fn_name.eq_ignore_ascii_case("attr") => {
                let mut j = i + 1;
                while j < tokens.len() && matches!(tokens[j], Token::Whitespace | Token::Comment(_))
                {
                    j += 1;
                }
                if j < tokens.len() && tokens[j] == Token::OpenParen {
                    j += 1;
                    let mut attr_name = String::new();
                    while j < tokens.len() && tokens[j] != Token::CloseParen {
                        match &tokens[j] {
                            Token::Ident(s) | Token::String(s) => attr_name.push_str(s),
                            Token::Delim(c) => attr_name.push(*c),
                            _ => {}
                        }
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j] == Token::CloseParen {
                        j += 1;
                    }
                    items.push(ContentItem::Attr(attr_name.trim().to_string()));
                    i = j;
                } else {
                    i += 1;
                }
            }
            Token::Ident(fn_name) if fn_name.eq_ignore_ascii_case("counter") => {
                let mut j = i + 1;
                while j < tokens.len() && matches!(tokens[j], Token::Whitespace | Token::Comment(_))
                {
                    j += 1;
                }
                if j < tokens.len() && tokens[j] == Token::OpenParen {
                    j += 1;
                    let mut name = String::new();
                    let mut style = None;
                    let mut is_style = false;
                    while j < tokens.len() && tokens[j] != Token::CloseParen {
                        match &tokens[j] {
                            Token::Comma => is_style = true,
                            Token::Ident(s) => {
                                if is_style {
                                    style = Some(s.to_ascii_lowercase());
                                } else {
                                    name = s.clone();
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j] == Token::CloseParen {
                        j += 1;
                    }
                    items.push(ContentItem::Counter {
                        name: name.trim().to_string(),
                        style,
                    });
                    i = j;
                } else {
                    i += 1;
                }
            }
            Token::Ident(fn_name) if fn_name.eq_ignore_ascii_case("counters") => {
                let mut j = i + 1;
                while j < tokens.len() && matches!(tokens[j], Token::Whitespace | Token::Comment(_))
                {
                    j += 1;
                }
                if j < tokens.len() && tokens[j] == Token::OpenParen {
                    j += 1;
                    let mut name = String::new();
                    let mut separator = String::new();
                    let mut style = None;
                    let mut param_idx = 0;
                    while j < tokens.len() && tokens[j] != Token::CloseParen {
                        match &tokens[j] {
                            Token::Comma => param_idx += 1,
                            Token::Ident(s) => {
                                if param_idx == 0 {
                                    name = s.clone();
                                } else if param_idx >= 2 {
                                    style = Some(s.to_ascii_lowercase());
                                }
                            }
                            Token::String(s) if param_idx == 1 => {
                                separator = s.clone();
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j] == Token::CloseParen {
                        j += 1;
                    }
                    items.push(ContentItem::Counters {
                        name: name.trim().to_string(),
                        separator,
                        style,
                    });
                    i = j;
                } else {
                    i += 1;
                }
            }
            _ => {
                if let Some((Value::Url(url), consumed)) = parse_url_fn(&tokens[i..]) {
                    items.push(ContentItem::Url(url));
                    i += consumed;
                } else {
                    i += 1;
                }
            }
        }
    }

    Some(Value::Content(items))
}

/// Tries to parse `counter-reset` or `counter-increment` property values.
pub fn try_parse_counter_actions(tokens: &[Token], default_val: i32) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    if filtered.is_empty() {
        return None;
    }

    if filtered.len() == 1
        && let Token::Ident(kw) = filtered[0]
        && kw.eq_ignore_ascii_case("none")
    {
        return Some(Value::CounterActions(Vec::new()));
    }

    let mut actions = Vec::new();
    let mut i = 0;
    while i < filtered.len() {
        if let Token::Ident(name) = filtered[i] {
            i += 1;
            let mut val = default_val;
            if i < filtered.len() {
                if let Token::Number(n) = filtered[i] {
                    val = *n as i32;
                    i += 1;
                } else if i + 1 < filtered.len()
                    && matches!(filtered[i], Token::Delim('-'))
                    && matches!(filtered[i + 1], Token::Number(_))
                    && let Token::Number(n) = filtered[i + 1]
                {
                    val = -(*n as i32);
                    i += 2;
                }
            }
            actions.push(CounterAction {
                name: name.clone(),
                value: val,
            });
        } else {
            i += 1;
        }
    }

    Some(Value::CounterActions(actions))
}

/// Tries to parse `quotes` property value.
pub fn try_parse_quotes(tokens: &[Token]) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    if filtered.is_empty() {
        return None;
    }

    if filtered.len() == 1
        && let Token::Ident(kw) = filtered[0]
    {
        if kw.eq_ignore_ascii_case("none") {
            return Some(Value::Quotes(Vec::new()));
        }
        if kw.eq_ignore_ascii_case("auto") {
            return Some(Value::Keyword("auto".to_string()));
        }
    }

    let mut strings = Vec::new();
    for tok in filtered {
        if let Token::String(s) = tok {
            strings.push(s.clone());
        }
    }

    let mut pairs = Vec::new();
    let (chunks, _) = strings.as_chunks::<2>();
    for chunk in chunks {
        pairs.push((chunk[0].clone(), chunk[1].clone()));
    }
    Some(Value::Quotes(pairs))
}
