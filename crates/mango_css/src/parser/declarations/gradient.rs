use crate::tokenizer::Token;
use crate::values::{ColorStop, Gradient, Value};

/// Tries to parse `linear-gradient(...)` or `radial-gradient(...)` or `conic-gradient(...)` or their
/// `-webkit-` prefixed variants starting at `tokens[0]`.
/// Returns `(Value::Gradient(...), tokens_consumed)` on success.
pub fn parse_gradient_fn(tokens: &[Token]) -> Option<(Value, usize)> {
    let mut idx = 0;
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    let fn_name = if let Token::Ident(name) = &tokens.get(idx)? {
        name.to_ascii_lowercase()
    } else {
        return None;
    };
    let is_conic = fn_name.contains("conic-gradient");
    let is_linear = fn_name.contains("linear-gradient");
    let is_radial = fn_name.contains("radial-gradient");
    let repeating = fn_name.contains("repeating-");
    if !is_linear && !is_radial && !is_conic {
        return None;
    }
    idx += 1; // consume function name
    // Consume whitespace then OpenParen
    while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
        idx += 1;
    }
    if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
        return None;
    }
    idx += 1;

    // Collect all tokens up to the matching CloseParen
    let mut depth = 1usize;
    let mut inner: Vec<Token> = Vec::new();
    while idx < tokens.len() {
        match &tokens[idx] {
            Token::OpenParen => {
                depth += 1;
                inner.push(tokens[idx].clone());
                idx += 1;
            }
            Token::CloseParen => {
                depth -= 1;
                idx += 1;
                if depth == 0 {
                    break;
                }
                inner.push(Token::CloseParen);
            }
            tok => {
                inner.push(tok.clone());
                idx += 1;
            }
        }
    }

    // Parse angle and color stops from inner tokens
    let inner_str: String = inner.iter().map(token_to_string).collect();
    let gradient = if is_conic {
        parse_conic_gradient_inner(&inner_str, repeating)
    } else if is_linear {
        parse_linear_gradient_inner_repeating(&inner_str, repeating)
    } else {
        parse_radial_gradient_inner_repeating(&inner_str, repeating)
    };
    Some((Value::Gradient(Box::new(gradient)), idx))
}

pub(crate) fn token_to_string(tok: &Token) -> String {
    match tok {
        Token::Ident(s) => s.clone(),
        Token::String(s) => format!("\"{s}\""),
        Token::Hash(h) => format!("#{h}"),
        Token::Number(n) => n.to_string(),
        Token::Percentage(p) => format!("{p}%"),
        Token::Dimension { value, unit } => format!("{value}{unit}"),
        Token::Colon => ":".to_string(),
        Token::Semicolon => ";".to_string(),
        Token::Comma => ",".to_string(),
        Token::OpenBracket => "[".to_string(),
        Token::CloseBracket => "]".to_string(),
        Token::OpenParen => "(".to_string(),
        Token::CloseParen => ")".to_string(),
        Token::Whitespace => " ".to_string(),
        Token::Delim(c) => c.to_string(),
        _ => String::new(),
    }
}

/// Parses the inner content of `linear-gradient(...)`.
fn parse_linear_gradient_inner_repeating(inner: &str, repeating: bool) -> Gradient {
    let mut angle_deg = 180.0f32; // default: to bottom
    let mut stops: Vec<ColorStop> = Vec::new();

    // Split on commas at depth 0 respecting nested parens
    let parts = split_gradient_parts(inner);

    let mut first = true;
    for part in &parts {
        let part = part.trim();
        if first {
            first = false;
            if part.to_ascii_lowercase().starts_with("to ") {
                let dir = part[3..].trim().to_ascii_lowercase();
                angle_deg = match dir.as_str() {
                    "top" => 0.0,
                    "right" => 90.0,
                    "bottom" => 180.0,
                    "left" => 270.0,
                    "top right" | "right top" => 45.0,
                    "bottom right" | "right bottom" => 135.0,
                    "bottom left" | "left bottom" => 225.0,
                    "top left" | "left top" => 315.0,
                    _ => 180.0,
                };
                continue;
            } else if let Some(deg) = parse_angle_deg(part) {
                angle_deg = deg;
                continue;
            }
            // Fall through: treat as a color stop
        }
        if let Some(stop) = parse_color_stop(part) {
            stops.push(stop);
        }
    }
    Gradient::Linear {
        angle_deg,
        stops,
        repeating,
    }
}

/// Parses the inner content of `radial-gradient(...)`.
fn parse_radial_gradient_inner_repeating(inner: &str, repeating: bool) -> Gradient {
    let parts = split_gradient_parts(inner);
    let mut stops: Vec<ColorStop> = Vec::new();
    let mut center_x = 0.5f32;
    let mut center_y = 0.5f32;
    let mut radius_x = None;
    let mut radius_y = None;

    let mut first = true;
    for part in &parts {
        let part = part.trim();
        if first {
            first = false;
            let lower = part.to_ascii_lowercase();
            let is_header = lower.starts_with("circle")
                || lower.starts_with("ellipse")
                || lower.starts_with("closest")
                || lower.starts_with("farthest")
                || lower.starts_with("at ")
                || lower.contains(" at ");
            if is_header {
                let (size_part, pos_part) = if let Some(at_idx) = lower.find(" at ") {
                    (part[..at_idx].trim(), Some(part[at_idx + 4..].trim()))
                } else if let Some(pos) = lower.strip_prefix("at ") {
                    ("", Some(pos.trim()))
                } else {
                    (part, None)
                };

                if let Some(pos_str) = pos_part {
                    let tokens: Vec<&str> = pos_str.split_whitespace().collect();
                    let (cx, cy) = parse_radial_position(&tokens);
                    center_x = cx;
                    center_y = cy;
                }

                let size_tokens: Vec<&str> = size_part
                    .split_whitespace()
                    .filter(|s| {
                        !s.eq_ignore_ascii_case("circle") && !s.eq_ignore_ascii_case("ellipse")
                    })
                    .collect();
                if let Some(rx_tok) = size_tokens.first() {
                    if let Some(pct) = rx_tok.strip_suffix('%') {
                        if let Ok(v) = pct.trim().parse::<f32>() {
                            radius_x = Some(v / 100.0);
                        }
                    }
                }
                if let Some(ry_tok) = size_tokens.get(1) {
                    if let Some(pct) = ry_tok.strip_suffix('%') {
                        if let Ok(v) = pct.trim().parse::<f32>() {
                            radius_y = Some(v / 100.0);
                        }
                    }
                }
                continue;
            }
        }
        if let Some(stop) = parse_color_stop(part) {
            stops.push(stop);
        }
    }
    Gradient::Radial {
        stops,
        repeating,
        center_x,
        center_y,
        radius_x,
        radius_y,
    }
}

fn parse_radial_position(tokens: &[&str]) -> (f32, f32) {
    let mut cx = 0.5f32;
    let mut cy = 0.5f32;
    let mut x_set = false;
    for &tok in tokens {
        match tok {
            "left" => {
                cx = 0.0;
                x_set = true;
            }
            "right" => {
                cx = 1.0;
                x_set = true;
            }
            "top" => {
                cy = 0.0;
            }
            "bottom" => {
                cy = 1.0;
            }
            "center" => {
                if !x_set {
                    cx = 0.5;
                    x_set = true;
                } else {
                    cy = 0.5;
                }
            }
            _ => {
                if let Some(pct) = tok.strip_suffix('%') {
                    if let Ok(v) = pct.trim().parse::<f32>() {
                        if !x_set {
                            cx = v / 100.0;
                            x_set = true;
                        } else {
                            cy = v / 100.0;
                        }
                    }
                } else if tok == "0" {
                    if !x_set {
                        cx = 0.0;
                        x_set = true;
                    } else {
                        cy = 0.0;
                    }
                }
            }
        }
    }
    (cx, cy)
}

/// Parses `conic-gradient([from <angle>] [at <position>], <stops>)`.
fn parse_conic_gradient_inner(inner: &str, repeating: bool) -> Gradient {
    let parts = split_gradient_parts(inner);
    let mut stops: Vec<ColorStop> = Vec::new();
    let mut angle_deg = 0.0f32;
    let mut first = true;
    for part in &parts {
        let part = part.trim();
        if first
            && (part.to_ascii_lowercase().starts_with("from ")
                || part.to_ascii_lowercase().starts_with("at ")
                || part.contains(" from "))
        {
            first = false;
            let lower = part.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("from ") {
                let angle_tok = rest.split_whitespace().next().unwrap_or("0deg");
                angle_deg = parse_angle_deg(angle_tok).unwrap_or(0.0);
            }
            continue;
        }
        first = false;
        if let Some(stop) = parse_color_stop(part) {
            stops.push(stop);
        }
    }
    Gradient::Conic {
        angle_deg,
        stops,
        repeating,
    }
}

fn split_gradient_parts(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

pub(crate) fn parse_angle_deg(s: &str) -> Option<f32> {
    let s = s.trim();
    if s == "0" {
        return Some(0.0);
    }
    if let Some(deg_str) = s.strip_suffix("deg") {
        return deg_str.trim().parse().ok();
    }
    if let Some(rad_str) = s.strip_suffix("rad") {
        return rad_str.trim().parse::<f32>().ok().map(|r| r.to_degrees());
    }
    if let Some(turn_str) = s.strip_suffix("turn") {
        return turn_str.trim().parse::<f32>().ok().map(|t| t * 360.0);
    }
    if let Some(grad_str) = s.strip_suffix("grad") {
        return grad_str.trim().parse::<f32>().ok().map(|g| g * 0.9);
    }
    None
}

/// Parses a single gradient color stop such as `red`, `red 25%`, or `red 10% 40%`.
fn parse_color_stop(s: &str) -> Option<ColorStop> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Find the trailing run of `<percentage>` / `<length>` position tokens.
    let mut position_parts: Vec<&str> = Vec::new();
    let mut color_end = s.len();
    let mut scan_end = s.len();
    let mut split_pos = 0usize;
    for (idx, ch) in s.char_indices().rev() {
        if ch.is_whitespace() {
            let token = s[idx..scan_end].trim();
            if !token.is_empty() && is_gradient_position(token) {
                position_parts.push(token);
                color_end = idx;
                split_pos = idx;
                scan_end = idx;
            } else {
                break;
            }
        }
    }
    let _ = split_pos;
    position_parts.reverse();

    let color_part = s[..color_end].trim();
    let is_current_color = color_part.eq_ignore_ascii_case("currentcolor");
    let (color, var_name) = if is_current_color {
        (mango_core::Color::BLACK, None)
    } else if color_part.starts_with("var(") && color_part.ends_with(')') {
        let var_inner = color_part[4..color_part.len() - 1].trim();
        let (name, fallback) = match var_inner.split_once(',') {
            Some((n, fb)) => (n.trim(), Some(fb.trim())),
            None => (var_inner, None),
        };
        let fallback_color = fallback
            .and_then(Value::parse_color)
            .unwrap_or(mango_core::Color::TRANSPARENT);
        (fallback_color, Some(name.to_string()))
    } else {
        let c = Value::parse_color(color_part)?;
        (c, None)
    };
    let to_pos = |p: &str| -> Option<crate::values::StopPosition> {
        let p = p.trim();
        if p == "0" {
            Some(crate::values::StopPosition::Fraction(0.0))
        } else if let Some(pct_str) = p.strip_suffix('%') {
            pct_str
                .trim()
                .parse::<f32>()
                .ok()
                .map(|v| crate::values::StopPosition::Fraction(v / 100.0))
        } else if let Some(px_str) = p.strip_suffix("px") {
            px_str
                .trim()
                .parse::<f32>()
                .ok()
                .map(crate::values::StopPosition::Px)
        } else {
            None
        }
    };
    let position = position_parts.first().copied().and_then(to_pos);
    let end_position = position_parts.get(1).copied().and_then(to_pos);
    Some(ColorStop {
        color,
        position,
        end_position,
        is_current_color,
        var_name,
    })
}

/// Returns true when a token is a gradient stop position (`50%`, `12px`, `0`).
fn is_gradient_position(token: &str) -> bool {
    let t = token.trim();
    if t == "0" {
        return true;
    }
    if let Some(pct) = t.strip_suffix('%') {
        return pct.trim().parse::<f32>().is_ok();
    }
    if let Some(px) = t.strip_suffix("px") {
        return px.trim().parse::<f32>().is_ok();
    }
    false
}
