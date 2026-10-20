use crate::parser::declarations::gradient::token_to_string;
use crate::tokenizer::Token;
use crate::values::{Length, Transform, TransformFunction, Value};

/// Tries to parse `transform: func(...) func(...) ...` from the token slice.
pub fn try_parse_transform_list(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let text_lower = text.trim().to_ascii_lowercase();
    if text_lower == "none" {
        return Some(Value::Transform(Transform::default()));
    }
    let has_transform = [
        "translate(",
        "translatex(",
        "translatey(",
        "translatez(",
        "translate3d(",
        "rotate(",
        "rotatex(",
        "rotatey(",
        "rotatez(",
        "rotate3d(",
        "scale(",
        "scalex(",
        "scaley(",
        "scalez(",
        "scale3d(",
        "skew(",
        "skewx(",
        "skewy(",
        "matrix(",
        "matrix3d(",
        "perspective(",
    ]
    .iter()
    .any(|f| text_lower.contains(f));
    if !has_transform {
        return None;
    }

    let mut funcs: Vec<TransformFunction> = Vec::new();
    let raw = text.trim();
    let mut remaining = raw;
    loop {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }
        let Some(paren_pos) = remaining.find('(') else {
            break;
        };
        let fn_name = remaining[..paren_pos].trim().to_ascii_lowercase();
        let args_start = paren_pos + 1;
        let mut depth = 1;
        let mut end = args_start;
        for (i, ch) in remaining[args_start..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = args_start + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let args_str = remaining[args_start..end].trim();
        remaining = remaining[end + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());

        let parse_num = |s: &str| -> f32 {
            let trimmed = s.trim();
            let clean = trimmed.trim_end_matches("px").trim_end_matches('%');
            clean.parse::<f32>().unwrap_or(0.0)
        };

        let parse_len = |s: &str| -> Length {
            let trimmed = s.trim();
            if let Some(p) = trimmed.strip_suffix('%') {
                Length::Percent(p.trim().parse::<f32>().unwrap_or(0.0))
            } else if let Some(px) = trimmed.strip_suffix("px") {
                Length::Px(px.trim().parse::<f32>().unwrap_or(0.0))
            } else if let Some(em) = trimmed.strip_suffix("em") {
                Length::Em(em.trim().parse::<f32>().unwrap_or(0.0))
            } else if let Some(rem) = trimmed.strip_suffix("rem") {
                Length::Rem(rem.trim().parse::<f32>().unwrap_or(0.0))
            } else {
                Length::Px(trimmed.parse::<f32>().unwrap_or(0.0))
            }
        };
        let is_len = |s: &str| -> bool {
            let trimmed = s.trim();
            trimmed.ends_with('%') || trimmed.ends_with("em") || trimmed.ends_with("rem")
        };

        let parse_angle = |s: &str| -> f32 {
            let trimmed = s.trim().to_ascii_lowercase();
            if let Some(deg_str) = trimmed.strip_suffix("deg") {
                deg_str.trim().parse::<f32>().unwrap_or(0.0)
            } else if let Some(rad_str) = trimmed.strip_suffix("rad") {
                rad_str.trim().parse::<f32>().unwrap_or(0.0) * (180.0 / std::f32::consts::PI)
            } else if let Some(grad_str) = trimmed.strip_suffix("grad") {
                grad_str.trim().parse::<f32>().unwrap_or(0.0) * 0.9
            } else if let Some(turn_str) = trimmed.strip_suffix("turn") {
                turn_str.trim().parse::<f32>().unwrap_or(0.0) * 360.0
            } else {
                trimmed.parse::<f32>().unwrap_or(0.0)
            }
        };

        let parts: Vec<&str> = if args_str.contains(',') {
            args_str.split(',').collect()
        } else {
            args_str.split_whitespace().collect()
        };
        let get_num = |idx: usize| -> f32 { parts.get(idx).map(|s| parse_num(s)).unwrap_or(0.0) };
        let get_ang = |idx: usize| -> f32 { parts.get(idx).map(|s| parse_angle(s)).unwrap_or(0.0) };

        let f = match fn_name.as_str() {
            "translate" => {
                let s0 = parts.first().copied().unwrap_or("");
                let s1 = parts.get(1).copied().unwrap_or("");
                if is_len(s0) || is_len(s1) {
                    TransformFunction::TranslateLen(parse_len(s0), parse_len(s1))
                } else {
                    let tx = get_num(0);
                    let ty = if parts.len() > 1 { get_num(1) } else { 0.0 };
                    TransformFunction::Translate(tx, ty)
                }
            }
            "translatex" => {
                let s0 = parts.first().copied().unwrap_or("");
                if is_len(s0) {
                    TransformFunction::TranslateXLen(parse_len(s0))
                } else {
                    TransformFunction::TranslateX(get_num(0))
                }
            }
            "translatey" => {
                let s0 = parts.first().copied().unwrap_or("");
                if is_len(s0) {
                    TransformFunction::TranslateYLen(parse_len(s0))
                } else {
                    TransformFunction::TranslateY(get_num(0))
                }
            }
            "translatez" => TransformFunction::TranslateZ(get_num(0)),
            "translate3d" => TransformFunction::Translate3d(get_num(0), get_num(1), get_num(2)),
            "rotate" => TransformFunction::Rotate(get_ang(0)),
            "rotatex" => TransformFunction::RotateX(get_ang(0)),
            "rotatey" => TransformFunction::RotateY(get_ang(0)),
            "rotatez" => TransformFunction::RotateZ(get_ang(0)),
            "rotate3d" => {
                TransformFunction::Rotate3d(get_num(0), get_num(1), get_num(2), get_ang(3))
            }
            "scale" => {
                let sx = get_num(0);
                let sy = if parts.len() > 1 { get_num(1) } else { sx };
                TransformFunction::Scale(sx, sy)
            }
            "scalex" => TransformFunction::ScaleX(get_num(0)),
            "scaley" => TransformFunction::ScaleY(get_num(0)),
            "scalez" => TransformFunction::ScaleZ(get_num(0)),
            "scale3d" => TransformFunction::Scale3d(get_num(0), get_num(1), get_num(2)),
            "skew" => {
                let ax = get_ang(0);
                let ay = if parts.len() > 1 { get_ang(1) } else { 0.0 };
                TransformFunction::Skew(ax, ay)
            }
            "skewx" => TransformFunction::Skew(get_ang(0), 0.0),
            "skewy" => TransformFunction::Skew(0.0, get_ang(0)),
            "matrix" => TransformFunction::Matrix(
                get_num(0),
                get_num(1),
                get_num(2),
                get_num(3),
                get_num(4),
                get_num(5),
            ),
            "matrix3d" => {
                let mut m = [0.0f32; 16];
                for (i, p) in parts.iter().enumerate().take(16) {
                    m[i] = parse_num(p);
                }
                TransformFunction::Matrix3d(m)
            }
            "perspective" => TransformFunction::Perspective(get_num(0)),
            _ => break,
        };
        funcs.push(f);
    }
    if funcs.is_empty() {
        return None;
    }
    Some(Value::Transform(Transform(funcs)))
}

/// Splits a token slice on top-level commas into sub-slices without dropping whitespace.
pub(crate) fn split_tokens_by_top_level_comma(tokens: &[Token]) -> Vec<&[Token]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    for (i, tok) in tokens.iter().enumerate() {
        match tok {
            Token::OpenParen | Token::OpenBracket | Token::OpenCurly => depth += 1,
            Token::CloseParen | Token::CloseBracket | Token::CloseCurly => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            Token::Comma if depth == 0 => {
                chunks.push(&tokens[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= tokens.len() {
        chunks.push(&tokens[start..]);
    }
    chunks
}
