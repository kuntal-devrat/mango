use crate::parser::declarations::functions::parse_url_fn;
use crate::parser::declarations::gradient::{parse_gradient_fn, token_to_string};
use crate::parser::declarations::value::parse_value_from_tokens;
use crate::tokenizer::Token;
use crate::values::{
    BackgroundAttachment, BackgroundClip, BlendMode, BorderImage, BorderImageRepeat, ClipPath,
    FilterFunction, Isolation, Length, MaskComposite, MaskMode, TextShadow, Value,
};

/// Tries to parse `text-shadow: offset-x offset-y blur? color?` from token slice,
/// supporting multiple comma-separated shadow layers.
pub fn try_parse_text_shadow(tokens: &[Token]) -> Option<Value> {
    let mut groups: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    for tok in tokens {
        match tok {
            Token::OpenParen => {
                depth += 1;
                current.push(tok.clone());
            }
            Token::CloseParen => {
                depth -= 1;
                current.push(tok.clone());
            }
            Token::Comma if depth == 0 => {
                if !current.is_empty() {
                    groups.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(tok.clone()),
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }

    let mut shadows = Vec::new();

    for group in groups {
        let text: String = group.iter().map(token_to_string).collect();
        let lower = text.trim().to_ascii_lowercase();
        if lower == "none" {
            return Some(Value::TextShadow(TextShadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur_radius: 0.0,
                color: mango_core::Color::TRANSPARENT,
            }));
        }
        let parts = split_shadow_args(text.trim());
        let parse_px = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(num) = s.strip_suffix("px") {
                num.trim().parse::<f32>().ok()
            } else if let Some(num) = s.strip_suffix("em") {
                num.trim().parse::<f32>().ok().map(|e| e * 16.0)
            } else if let Some(num) = s.strip_suffix("rem") {
                num.trim().parse::<f32>().ok().map(|r| r * 16.0)
            } else if s == "0" {
                Some(0.0)
            } else {
                s.parse::<f32>().ok()
            }
        };

        let mut offset_x = None;
        let mut offset_y = None;
        let mut blur = 0.0f32;
        let mut color = mango_core::Color::rgba(0, 0, 0, 180);

        for part in &parts {
            if let Some(c) = Value::parse_color(part) {
                color = c;
            } else if let Some(px) = parse_px(part) {
                if offset_x.is_none() {
                    offset_x = Some(px);
                } else if offset_y.is_none() {
                    offset_y = Some(px);
                } else {
                    blur = px;
                }
            }
        }

        if let (Some(ox), Some(oy)) = (offset_x, offset_y) {
            shadows.push(Value::TextShadow(TextShadow {
                offset_x: ox,
                offset_y: oy,
                blur_radius: blur,
                color,
            }));
        }
    }

    if shadows.is_empty() {
        None
    } else if shadows.len() == 1 {
        Some(shadows.remove(0))
    } else {
        Some(Value::List(shadows))
    }
}

/// Tries to parse `filter: blur(4px) brightness(0.8) ...` from token slice.
pub fn try_parse_filter_list(tokens: &[Token]) -> Option<Value> {
    let raw: String = tokens.iter().map(token_to_string).collect();
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some(Value::Filter(vec![]));
    }
    if !raw.contains('(') {
        return None;
    }
    let mut funcs: Vec<FilterFunction> = Vec::new();
    let mut remaining = raw;
    loop {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }
        let paren = match remaining.find('(') {
            Some(p) => p,
            None => break,
        };
        let fn_name = remaining[..paren].trim().to_ascii_lowercase();
        let args_start = paren + 1;
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
        let arg = remaining[args_start..end]
            .trim()
            .trim_end_matches("px")
            .trim_end_matches('%')
            .trim_end_matches("deg");
        let val: f32 = arg.parse().unwrap_or(0.0);
        let norm = |v: f32, is_pct: bool| -> f32 {
            if is_pct || remaining[args_start..end].contains('%') {
                v / 100.0
            } else {
                v
            }
        };
        if let Some(f) = match fn_name.as_str() {
            "blur" => Some(FilterFunction::Blur(val)),
            "brightness" => Some(FilterFunction::Brightness(norm(val, false))),
            "contrast" => Some(FilterFunction::Contrast(norm(val, false))),
            "grayscale" => Some(FilterFunction::Grayscale(norm(val, false))),
            "opacity" => Some(FilterFunction::Opacity(norm(val, false))),
            "saturate" => Some(FilterFunction::Saturate(norm(val, false))),
            "sepia" => Some(FilterFunction::Sepia(norm(val, false))),
            "hue-rotate" => Some(FilterFunction::HueRotate(val)),
            "invert" => Some(FilterFunction::Invert(norm(val, false))),
            "drop-shadow" => {
                let parts = split_shadow_args(remaining[args_start..end].trim());
                let parse_px = |s: &str| -> Option<f32> {
                    let s = s.trim();
                    if let Some(num) = s.strip_suffix("px") {
                        num.trim().parse::<f32>().ok()
                    } else if let Some(num) = s.strip_suffix("em") {
                        num.trim().parse::<f32>().ok().map(|e| e * 16.0)
                    } else if let Some(num) = s.strip_suffix("rem") {
                        num.trim().parse::<f32>().ok().map(|r| r * 16.0)
                    } else if s == "0" {
                        Some(0.0)
                    } else {
                        s.parse::<f32>().ok()
                    }
                };

                let mut ox = 0.0;
                let mut oy = 0.0;
                let mut blur = 0.0;
                let mut color = mango_core::Color::rgba(0, 0, 0, 128);
                let mut len_count = 0;

                for part in &parts {
                    if let Some(c) = Value::parse_color(part) {
                        color = c;
                    } else if let Some(px) = parse_px(part) {
                        match len_count {
                            0 => ox = px,
                            1 => oy = px,
                            2 => blur = px,
                            _ => {}
                        }
                        len_count += 1;
                    }
                }

                Some(FilterFunction::DropShadow {
                    offset_x: ox,
                    offset_y: oy,
                    blur,
                    color,
                })
            }
            _ => None,
        } {
            funcs.push(f);
        }
        remaining = remaining[end + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());
    }
    if funcs.is_empty() {
        return None;
    }
    Some(Value::Filter(funcs))
}

/// Tries to parse `clip-path` (e.g. `none`, `url(...)`, `circle(...)`, `ellipse(...)`, `inset(...)`, `polygon(...)`).
pub fn try_parse_clip_path(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower == "none" {
        return Some(Value::ClipPath(ClipPath::None));
    }
    if lower.starts_with("url(") && lower.ends_with(')') {
        let inner = trimmed[4..trimmed.len() - 1]
            .trim()
            .trim_matches(|c| c == '\'' || c == '"');
        return Some(Value::ClipPath(ClipPath::Url(inner.to_string())));
    }

    let parse_len = |t: &str| -> Length {
        let t = t.trim();
        if let Some(pct) = t.strip_suffix('%') {
            Length::Percent(pct.parse().unwrap_or(0.0))
        } else if let Some(px) = t.strip_suffix("px") {
            Length::Px(px.parse().unwrap_or(0.0))
        } else if let Some(em) = t.strip_suffix("em") {
            Length::Em(em.parse().unwrap_or(0.0))
        } else if let Some(rem) = t.strip_suffix("rem") {
            Length::Rem(rem.parse().unwrap_or(0.0))
        } else if t.eq_ignore_ascii_case("center") {
            Length::Percent(50.0)
        } else if t.eq_ignore_ascii_case("left") || t.eq_ignore_ascii_case("top") {
            Length::Percent(0.0)
        } else if t.eq_ignore_ascii_case("right") || t.eq_ignore_ascii_case("bottom") {
            Length::Percent(100.0)
        } else {
            Length::Px(t.parse().unwrap_or(0.0))
        }
    };

    if lower.starts_with("circle(") && lower.ends_with(')') {
        let inner = trimmed[7..trimmed.len() - 1].trim();
        let (radius_str, pos_str) = if let Some(at_idx) = inner.to_ascii_lowercase().find(" at ") {
            (&inner[..at_idx], Some(&inner[at_idx + 4..]))
        } else {
            (inner, None)
        };
        let radius = if radius_str.is_empty() {
            Length::Percent(50.0)
        } else {
            parse_len(radius_str)
        };
        let (cx, cy) = if let Some(p) = pos_str {
            let parts: Vec<&str> = p.split_whitespace().collect();
            let x = parts
                .first()
                .map(|s| parse_len(s))
                .unwrap_or(Length::Percent(50.0));
            let y = parts
                .get(1)
                .map(|s| parse_len(s))
                .unwrap_or(Length::Percent(50.0));
            (x, y)
        } else {
            (Length::Percent(50.0), Length::Percent(50.0))
        };
        return Some(Value::ClipPath(ClipPath::Circle {
            radius,
            center_x: cx,
            center_y: cy,
        }));
    }

    if lower.starts_with("ellipse(") && lower.ends_with(')') {
        let inner = trimmed[8..trimmed.len() - 1].trim();
        let (radii_str, pos_str) = if let Some(at_idx) = inner.to_ascii_lowercase().find(" at ") {
            (&inner[..at_idx], Some(&inner[at_idx + 4..]))
        } else {
            (inner, None)
        };
        let radii_parts: Vec<&str> = radii_str.split_whitespace().collect();
        let rx = radii_parts
            .first()
            .map(|s| parse_len(s))
            .unwrap_or(Length::Percent(50.0));
        let ry = radii_parts
            .get(1)
            .map(|s| parse_len(s))
            .unwrap_or(Length::Percent(50.0));
        let (cx, cy) = if let Some(p) = pos_str {
            let parts: Vec<&str> = p.split_whitespace().collect();
            let x = parts
                .first()
                .map(|s| parse_len(s))
                .unwrap_or(Length::Percent(50.0));
            let y = parts
                .get(1)
                .map(|s| parse_len(s))
                .unwrap_or(Length::Percent(50.0));
            (x, y)
        } else {
            (Length::Percent(50.0), Length::Percent(50.0))
        };
        return Some(Value::ClipPath(ClipPath::Ellipse {
            radius_x: rx,
            radius_y: ry,
            center_x: cx,
            center_y: cy,
        }));
    }

    if lower.starts_with("inset(") && lower.ends_with(')') {
        let inner = trimmed[6..trimmed.len() - 1].trim();
        let (insets_str, round_str) =
            if let Some(r_idx) = inner.to_ascii_lowercase().find(" round ") {
                (&inner[..r_idx], Some(&inner[r_idx + 7..]))
            } else {
                (inner, None)
            };
        let parts: Vec<&str> = insets_str.split_whitespace().collect();
        let (t, r, b, l) = match parts.len() {
            1 => {
                let v = parse_len(parts[0]);
                (v, v, v, v)
            }
            2 => {
                let tb = parse_len(parts[0]);
                let rl = parse_len(parts[1]);
                (tb, rl, tb, rl)
            }
            3 => {
                let top = parse_len(parts[0]);
                let rl = parse_len(parts[1]);
                let bot = parse_len(parts[2]);
                (top, rl, bot, rl)
            }
            4.. => (
                parse_len(parts[0]),
                parse_len(parts[1]),
                parse_len(parts[2]),
                parse_len(parts[3]),
            ),
            _ => (
                Length::Px(0.0),
                Length::Px(0.0),
                Length::Px(0.0),
                Length::Px(0.0),
            ),
        };
        let round = round_str.map(|r_text| {
            let r_parts: Vec<&str> = r_text.split_whitespace().collect();
            match r_parts.len() {
                1 => {
                    let rad = parse_len(r_parts[0]);
                    [rad, rad, rad, rad]
                }
                2 => {
                    let r1 = parse_len(r_parts[0]);
                    let r2 = parse_len(r_parts[1]);
                    [r1, r2, r1, r2]
                }
                3 => {
                    let r1 = parse_len(r_parts[0]);
                    let r2 = parse_len(r_parts[1]);
                    let r3 = parse_len(r_parts[2]);
                    [r1, r2, r3, r2]
                }
                4.. => [
                    parse_len(r_parts[0]),
                    parse_len(r_parts[1]),
                    parse_len(r_parts[2]),
                    parse_len(r_parts[3]),
                ],
                _ => [Length::Px(0.0); 4],
            }
        });
        return Some(Value::ClipPath(ClipPath::Inset {
            top: t,
            right: r,
            bottom: b,
            left: l,
            round,
        }));
    }

    if lower.starts_with("polygon(") && lower.ends_with(')') {
        let inner = trimmed[8..trimmed.len() - 1].trim();
        let mut pairs = Vec::new();
        for item in inner.split(',') {
            let item = item.trim();
            if item.eq_ignore_ascii_case("evenodd") || item.eq_ignore_ascii_case("nonzero") {
                continue;
            }
            let coords: Vec<&str> = item.split_whitespace().collect();
            if coords.len() >= 2 {
                pairs.push((parse_len(coords[0]), parse_len(coords[1])));
            }
        }
        if !pairs.is_empty() {
            return Some(Value::ClipPath(ClipPath::Polygon(pairs)));
        }
    }

    None
}

/// Tries to parse `mix-blend-mode` (e.g. `normal`, `multiply`, `screen`, etc.).
pub fn try_parse_blend_mode(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    BlendMode::parse(&text).map(Value::BlendMode)
}

/// Tries to parse `background-blend-mode` (single or comma-separated list).
pub fn try_parse_background_blend_mode(tokens: &[Token]) -> Option<Value> {
    let groups = split_commas(tokens);
    let mut list = Vec::new();
    for g in groups {
        let text: String = g.iter().map(token_to_string).collect();
        if let Some(bm) = BlendMode::parse(&text) {
            list.push(Value::BlendMode(bm));
        }
    }
    if list.is_empty() {
        None
    } else if list.len() == 1 {
        Some(list.remove(0))
    } else {
        Some(Value::List(list))
    }
}

/// Tries to parse `background-attachment` (single or comma-separated list).
pub fn try_parse_background_attachment(tokens: &[Token]) -> Option<Value> {
    let groups = split_commas(tokens);
    let mut list = Vec::new();
    for g in groups {
        let text: String = g.iter().map(token_to_string).collect();
        if let Some(a) = BackgroundAttachment::parse(&text) {
            list.push(Value::BackgroundAttachment(a));
        }
    }
    if list.is_empty() {
        None
    } else if list.len() == 1 {
        Some(list.remove(0))
    } else {
        Some(Value::List(list))
    }
}

/// Tries to parse `background-clip` (single or comma-separated list).
pub fn try_parse_background_clip(tokens: &[Token]) -> Option<Value> {
    let groups = split_commas(tokens);
    let mut list = Vec::new();
    for g in groups {
        let text: String = g.iter().map(token_to_string).collect();
        if let Some(c) = BackgroundClip::parse(&text) {
            list.push(Value::BackgroundClip(c));
        }
    }
    if list.is_empty() {
        None
    } else if list.len() == 1 {
        Some(list.remove(0))
    } else {
        Some(Value::List(list))
    }
}

/// Tries to parse `mask-composite` (single or comma-separated list).
pub fn try_parse_mask_composite(tokens: &[Token]) -> Option<Value> {
    let groups = split_commas(tokens);
    let mut list = Vec::new();
    for g in groups {
        let text: String = g.iter().map(token_to_string).collect();
        if let Some(m) = MaskComposite::parse(&text) {
            list.push(Value::MaskComposite(m));
        }
    }
    if list.is_empty() {
        None
    } else if list.len() == 1 {
        Some(list.remove(0))
    } else {
        Some(Value::List(list))
    }
}

/// Tries to parse `isolation` (auto | isolate).
pub fn try_parse_isolation(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    Isolation::parse(&text).map(Value::Isolation)
}

/// Tries to parse comma-separated `background-image` list (url, gradient, none).
pub fn try_parse_layered_background_images(tokens: &[Token]) -> Option<Value> {
    let groups = split_commas(tokens);
    if groups.len() <= 1 {
        return None;
    }
    let mut images = Vec::new();
    for g in groups {
        if let Some(val) = parse_value_from_tokens(&g) {
            images.push(val);
        }
    }
    if images.is_empty() {
        None
    } else {
        Some(Value::List(images))
    }
}

/// Tries to parse `border-image` shorthand or longhand.
pub fn try_parse_border_image(tokens: &[Token]) -> Option<Value> {
    let mut bi = BorderImage::default();
    let mut i = 0;
    let mut found_any = false;

    while i < tokens.len() {
        if matches!(tokens[i], Token::Whitespace | Token::Comment(_)) {
            i += 1;
            continue;
        }
        if let Some((Value::Url(url), consumed)) = parse_url_fn(&tokens[i..]) {
            bi.source = Some(url);
            found_any = true;
            i += consumed;
            continue;
        }
        if let Some((Value::Gradient(g), consumed)) = parse_gradient_fn(&tokens[i..]) {
            bi.gradient = Some(g);
            found_any = true;
            i += consumed;
            continue;
        }
        match &tokens[i] {
            Token::String(s) => {
                bi.source = Some(s.clone());
                found_any = true;
                i += 1;
            }
            Token::Ident(id) if id.eq_ignore_ascii_case("none") => {
                bi.source = None;
                found_any = true;
                i += 1;
            }
            Token::Ident(id) if id.eq_ignore_ascii_case("fill") => {
                bi.fill = true;
                found_any = true;
                i += 1;
            }
            Token::Ident(id)
                if matches!(
                    id.to_ascii_lowercase().as_str(),
                    "stretch" | "repeat" | "round" | "space"
                ) =>
            {
                if let Some(r) = BorderImageRepeat::parse(id) {
                    bi.repeat_h = r;
                    bi.repeat_v = r;
                    found_any = true;
                }
                i += 1;
            }
            Token::Number(n) => {
                let val = Length::Px(*n);
                bi.slice = [val, val, val, val];
                found_any = true;
                i += 1;
            }
            Token::Percentage(p) => {
                let val = Length::Percent(*p);
                bi.slice = [val, val, val, val];
                found_any = true;
                i += 1;
            }
            Token::Delim('/') => {
                i += 1;
                while i < tokens.len() && matches!(tokens[i], Token::Whitespace | Token::Comment(_))
                {
                    i += 1;
                }
                if i < tokens.len() {
                    if let Token::Number(w) = &tokens[i] {
                        let len = Length::Px(*w);
                        bi.width = [len, len, len, len];
                        i += 1;
                    } else if let Token::Dimension { value, unit } = &tokens[i]
                        && unit.eq_ignore_ascii_case("px")
                    {
                        let len = Length::Px(*value);
                        bi.width = [len, len, len, len];
                        i += 1;
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    if found_any {
        Some(Value::BorderImage(Box::new(bi)))
    } else {
        None
    }
}

/// Tries to parse `mask-mode` (e.g. `alpha`, `luminance`, `match-source`).
pub fn try_parse_mask_mode(tokens: &[Token]) -> Option<Value> {
    let text: String = tokens.iter().map(token_to_string).collect();
    MaskMode::parse(&text).map(Value::MaskMode)
}

pub(crate) fn split_shadow_args(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            c if c.is_ascii_whitespace() && depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                    current = String::new();
                }
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    parts
}

/// Splits a token slice on top-level commas, respecting nested parentheses.
pub(crate) fn split_commas(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut groups: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    for tok in tokens {
        match tok {
            Token::OpenParen => {
                depth += 1;
                current.push(tok.clone());
            }
            Token::CloseParen => {
                depth -= 1;
                current.push(tok.clone());
            }
            Token::Comma if depth == 0 => {
                groups.push(std::mem::take(&mut current));
            }
            Token::Whitespace | Token::Comment(_) => {}
            _ => current.push(tok.clone()),
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    if groups.is_empty() {
        groups.push(Vec::new());
    }
    groups
}
