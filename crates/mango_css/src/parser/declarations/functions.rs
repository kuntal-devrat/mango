use crate::parser::declarations::gradient::token_to_string;
use crate::parser::declarations::value::parse_value_from_tokens;
use crate::tokenizer::Token;
use crate::values::Value;

pub fn parse_var_call(tokens: &[Token]) -> Option<(Value, usize)> {
    if tokens.is_empty() {
        return None;
    }
    let mut idx = 0;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() {
        return None;
    }
    match &tokens[idx] {
        Token::Ident(name) if name.eq_ignore_ascii_case("var") => {}
        _ => return None,
    }
    idx += 1;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
        return None;
    }
    idx += 1; // consume OpenParen

    // Find matching CloseParen, handling nested parens
    let mut depth = 1;
    let mut close_idx = None;
    for (j, tok) in tokens.iter().enumerate().skip(idx) {
        match tok {
            Token::OpenParen => depth += 1,
            Token::CloseParen => {
                depth -= 1;
                if depth == 0 {
                    close_idx = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_idx = close_idx?;
    let inner_tokens = &tokens[idx..close_idx];
    let total_consumed = close_idx + 1;

    // Find first comma at depth 0
    let mut inner_depth = 0;
    let mut comma_idx = None;
    for (k, tok) in inner_tokens.iter().enumerate() {
        match tok {
            Token::OpenParen => inner_depth += 1,
            Token::CloseParen => {
                if inner_depth > 0 {
                    inner_depth -= 1;
                }
            }
            Token::Comma if inner_depth == 0 => {
                comma_idx = Some(k);
                break;
            }
            _ => {}
        }
    }

    let (var_name_tokens, fallback_tokens) = match comma_idx {
        Some(k) => (&inner_tokens[..k], Some(&inner_tokens[k + 1..])),
        None => (inner_tokens, None),
    };

    let mut var_name = String::new();
    for tok in var_name_tokens {
        match tok {
            Token::Whitespace | Token::Comment(_) => {}
            Token::Ident(name) => {
                var_name = name.clone();
                break;
            }
            _ => {}
        }
    }

    if var_name.is_empty() {
        return None;
    }

    let fallback = if let Some(fb_tokens) = fallback_tokens {
        parse_value_from_tokens(fb_tokens)
            .or_else(|| {
                let mut raw = String::new();
                for tok in fb_tokens {
                    raw.push_str(&token_to_string(tok));
                }
                let trimmed = raw.trim().to_string();
                if !trimmed.is_empty() {
                    Some(Value::String(trimmed))
                } else {
                    None
                }
            })
            .map(Box::new)
    } else {
        None
    };

    Some((
        Value::Var {
            name: var_name,
            fallback,
        },
        total_consumed,
    ))
}

pub fn parse_color_fn(tokens: &[Token]) -> Option<(Value, usize)> {
    if tokens.is_empty() {
        return None;
    }
    let mut idx = 0;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() {
        return None;
    }
    let fn_name = match &tokens[idx] {
        Token::Ident(name)
            if name.eq_ignore_ascii_case("rgb")
                || name.eq_ignore_ascii_case("rgba")
                || name.eq_ignore_ascii_case("hsl")
                || name.eq_ignore_ascii_case("hsla")
                || name.eq_ignore_ascii_case("color")
                || name.eq_ignore_ascii_case("oklab")
                || name.eq_ignore_ascii_case("oklch")
                || name.eq_ignore_ascii_case("color-mix") =>
        {
            name.clone()
        }
        _ => return None,
    };
    idx += 1;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
        return None;
    }
    idx += 1; // consume OpenParen

    let mut depth = 1;
    let mut close_idx = None;
    for (j, tok) in tokens.iter().enumerate().skip(idx) {
        match tok {
            Token::OpenParen => depth += 1,
            Token::CloseParen => {
                depth -= 1;
                if depth == 0 {
                    close_idx = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_idx = close_idx?;
    let mut call_str = fn_name;
    for t in &tokens[idx - 1..=close_idx] {
        call_str.push_str(&token_to_string(t));
    }
    let col = Value::parse_color(&call_str)?;
    Some((Value::Color(col), close_idx + 1))
}

pub fn parse_url_fn(tokens: &[Token]) -> Option<(Value, usize)> {
    let mut idx = 0;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() {
        return None;
    }
    match &tokens[idx] {
        Token::Ident(name) if name.eq_ignore_ascii_case("url") => {}
        _ => return None,
    }
    idx += 1;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
        return None;
    }
    idx += 1; // consume OpenParen

    let mut depth = 1;
    let mut close_idx = None;
    for (j, tok) in tokens.iter().enumerate().skip(idx) {
        match tok {
            Token::OpenParen => depth += 1,
            Token::CloseParen => {
                depth -= 1;
                if depth == 0 {
                    close_idx = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_idx = close_idx?;
    let inner_tokens = &tokens[idx..close_idx];
    let mut url_str = String::new();
    for t in inner_tokens {
        match t {
            Token::Whitespace | Token::Comment(_) => {}
            Token::String(s) => url_str.push_str(s),
            _ => url_str.push_str(&token_to_string(t)),
        }
    }
    let url_str = url_str
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    Some((Value::Url(url_str), close_idx + 1))
}

/// Parses CSS `aspect-ratio: auto || <ratio>` where ratio is `<number [0,∞]> [ / <number [0,∞]> ]?`.
pub fn try_parse_aspect_ratio(tokens: &[Token]) -> Option<Value> {
    if tokens.is_empty() {
        return None;
    }
    if let Some((var_val, consumed)) = parse_var_call(tokens) {
        let remaining = &tokens[consumed..];
        if remaining
            .iter()
            .all(|t| matches!(t, Token::Whitespace | Token::Comment(_)))
        {
            return Some(var_val);
        }
    }

    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    if filtered.is_empty() {
        return None;
    }

    let mut numbers = Vec::new();
    let mut has_auto = false;
    let mut has_slash = false;

    for tok in &filtered {
        match tok {
            Token::Ident(s) if s.eq_ignore_ascii_case("auto") => has_auto = true,
            Token::Number(n) => numbers.push(*n),
            Token::Delim('/') => has_slash = true,
            _ => return None,
        }
    }

    if numbers.is_empty() {
        if has_auto {
            return Some(Value::Keyword("auto".to_string()));
        }
        return None;
    }

    if numbers.len() == 1 && !has_slash {
        let n = numbers[0];
        if n > 0.0 {
            return Some(Value::Number(n));
        }
        return None;
    }

    if numbers.len() == 2 && has_slash {
        let w = numbers[0];
        let h = numbers[1];
        if w > 0.0 && h > 0.0 {
            return Some(Value::Number(w / h));
        }
        return None;
    }

    None
}
