use crate::parser::declarations::gradient::token_to_string;
use crate::parser::declarations::visual::split_commas;
use crate::properties::Declaration;
use crate::tokenizer::Token;
use crate::values::{ContainerType, TimingFunction, Value};

/// Tries to parse `will-change` (e.g. `transform, opacity`, `scroll-position`, `auto`).
pub fn try_parse_will_change(tokens: &[Token]) -> Option<Value> {
    let groups = split_commas(tokens);
    let mut props = Vec::new();
    for g in groups {
        let text: String = g.iter().map(token_to_string).collect();
        let clean = text.trim();
        if !clean.is_empty() {
            props.push(Value::Keyword(clean.to_string()));
        }
    }
    if props.is_empty() {
        None
    } else if props.len() == 1 {
        Some(props.remove(0))
    } else {
        Some(Value::List(props))
    }
}

/// Tries to parse `container-type` property value (`normal`, `size`, `inline-size`).
pub fn try_parse_container_type(tokens: &[Token]) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    let tok = filtered.first()?;
    if let Token::Ident(ident) = tok {
        ContainerType::parse(ident).map(Value::ContainerType)
    } else {
        None
    }
}

/// Tries to parse `container-name` property value (`none`, custom ident).
pub fn try_parse_container_name(tokens: &[Token]) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    let tok = filtered.first()?;
    if let Token::Ident(ident) = tok {
        if ident.eq_ignore_ascii_case("none") {
            Some(Value::Keyword("none".to_string()))
        } else {
            Some(Value::String(ident.clone()))
        }
    } else {
        None
    }
}

/// Tries to parse `container` shorthand property value (`<name>? / <type>`).
pub fn try_parse_container_shorthand(tokens: &[Token]) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();
    if filtered.is_empty() {
        return None;
    }
    let slash_pos = filtered.iter().position(|t| **t == Token::Delim('/'));
    if let Some(pos) = slash_pos {
        let name_tokens: Vec<Token> = filtered[..pos].iter().map(|t| (*t).clone()).collect();
        let type_tokens: Vec<Token> = filtered[pos + 1..].iter().map(|t| (*t).clone()).collect();
        let name_val = try_parse_container_name(&name_tokens)?;
        let type_val = try_parse_container_type(&type_tokens)?;
        Some(Value::List(vec![name_val, type_val]))
    } else if let Some(type_val) = try_parse_container_type(tokens) {
        Some(Value::List(vec![
            Value::Keyword("none".to_string()),
            type_val,
        ]))
    } else {
        try_parse_container_name(tokens).map(|name_val| {
            Value::List(vec![name_val, Value::ContainerType(ContainerType::Normal)])
        })
    }
}

/// Parses a `transition*` or `animation*` property into normalized longhand declarations.
///
/// Shorthands are expanded so that the computed-style layer only ever sees the
/// six `transition-*` and eight `animation-*` longhands, each holding a comma
/// separated [`Value::List`].
pub fn parse_transition_animation_property(
    prop: &str,
    tokens: &[Token],
    important: bool,
) -> Vec<Declaration> {
    let groups = split_commas(tokens);
    let is_animation = prop.starts_with("animation");

    let mut names: Vec<Value> = Vec::new();
    let mut durations: Vec<Value> = Vec::new();
    let mut timings: Vec<Value> = Vec::new();
    let mut delays: Vec<Value> = Vec::new();
    let mut iterations: Vec<Value> = Vec::new();
    let mut directions: Vec<Value> = Vec::new();
    let mut fills: Vec<Value> = Vec::new();
    let mut states: Vec<Value> = Vec::new();

    let longhand_prop = if is_animation {
        "animation-name"
    } else {
        "transition-property"
    };

    for group in &groups {
        let mut name: Option<Value> = None;
        let mut duration: Option<Value> = None;
        let mut timing: Option<Value> = None;
        let mut delay: Option<Value> = None;
        let mut iteration: Option<Value> = None;
        let mut direction: Option<Value> = None;
        let mut fill: Option<Value> = None;
        let mut state: Option<Value> = None;
        let mut time_count = 0;

        let mut i = 0;
        while i < group.len() {
            let tok = &group[i];
            match tok {
                Token::Dimension { value, unit }
                    if unit.eq_ignore_ascii_case("s") || unit.eq_ignore_ascii_case("ms") =>
                {
                    let ms = if unit.eq_ignore_ascii_case("s") {
                        value * 1000.0
                    } else {
                        *value
                    };
                    if time_count == 0 {
                        duration = Some(Value::Time(ms));
                    } else {
                        delay = Some(Value::Time(ms));
                    }
                    time_count += 1;
                    i += 1;
                }
                Token::Number(n) if *n == 0.0 && (time_count < 2) => {
                    if time_count == 0 {
                        duration = Some(Value::Time(0.0));
                    } else {
                        delay = Some(Value::Time(0.0));
                    }
                    time_count += 1;
                    i += 1;
                }
                Token::Number(n) if is_animation => {
                    iteration = Some(Value::Number(*n));
                    i += 1;
                }
                Token::Ident(word) => {
                    let lower = word.to_ascii_lowercase();
                    if (lower == "cubic-bezier" || lower == "steps")
                        && i + 1 < group.len()
                        && matches!(group[i + 1], Token::OpenParen)
                    {
                        let mut func_tokens = Vec::new();
                        func_tokens.push(tok.clone());
                        i += 1; // move to OpenParen
                        let mut depth = 0;
                        while i < group.len() {
                            let t = &group[i];
                            func_tokens.push(t.clone());
                            if matches!(t, Token::OpenParen) {
                                depth += 1;
                            } else if matches!(t, Token::CloseParen) {
                                depth -= 1;
                                if depth == 0 {
                                    i += 1;
                                    break;
                                }
                            }
                            i += 1;
                        }
                        let func_str: String = func_tokens.iter().map(token_to_string).collect();
                        if let Some(tf) = crate::values::parse_timing_function(&func_str) {
                            timing = Some(Value::TimingFunction(tf));
                        }
                    } else if let Some(tf) = crate::values::parse_timing_function(&lower) {
                        timing = Some(Value::TimingFunction(tf));
                        i += 1;
                    } else if is_animation {
                        match lower.as_str() {
                            "infinite" => iteration = Some(Value::Keyword("infinite".to_string())),
                            "normal" | "reverse" | "alternate" | "alternate-reverse" => {
                                direction = Some(Value::Keyword(lower.clone()))
                            }
                            "forwards" | "backwards" | "both" | "none" => {
                                fill = Some(Value::Keyword(lower.clone()))
                            }
                            "running" | "paused" => state = Some(Value::Keyword(lower.clone())),
                            _ => name = Some(Value::Keyword(lower.clone())),
                        }
                        i += 1;
                    } else {
                        name = Some(Value::Keyword(lower.clone()));
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Longhand-only properties carry a single value per group.
        match prop {
            "transition-property" | "animation-name" => {
                names.push(name.unwrap_or(Value::Keyword("all".to_string())));
                continue;
            }
            "transition-duration"
            | "transition-delay"
            | "animation-duration"
            | "animation-delay" => {
                let v = duration.or(delay).unwrap_or(Value::Time(0.0));
                if prop.ends_with("delay") {
                    delays.push(v);
                } else {
                    durations.push(v);
                }
                continue;
            }
            "transition-timing-function" | "animation-timing-function" => {
                timings.push(timing.unwrap_or(Value::TimingFunction(TimingFunction::Ease)));
                continue;
            }
            "animation-iteration-count" => {
                iterations.push(iteration.unwrap_or(Value::Number(1.0)));
                continue;
            }
            "animation-direction" => {
                directions.push(direction.unwrap_or(Value::Keyword("normal".to_string())));
                continue;
            }
            "animation-fill-mode" => {
                fills.push(fill.unwrap_or(Value::Keyword("none".to_string())));
                continue;
            }
            "animation-play-state" => {
                states.push(state.unwrap_or(Value::Keyword("running".to_string())));
                continue;
            }
            _ => {}
        }

        // Shorthand: fill defaults for anything omitted.
        names.push(name.unwrap_or_else(|| {
            if is_animation {
                Value::Keyword("none".to_string())
            } else {
                Value::Keyword("all".to_string())
            }
        }));
        durations.push(duration.unwrap_or(Value::Time(0.0)));
        timings.push(timing.unwrap_or(Value::TimingFunction(TimingFunction::Ease)));
        delays.push(delay.unwrap_or(Value::Time(0.0)));
        if is_animation {
            iterations.push(iteration.unwrap_or(Value::Number(1.0)));
            directions.push(direction.unwrap_or(Value::Keyword("normal".to_string())));
            fills.push(fill.unwrap_or(Value::Keyword("none".to_string())));
            states.push(state.unwrap_or(Value::Keyword("running".to_string())));
        }
    }

    let mut decls = Vec::new();
    let single = |v: &[Value]| -> Value {
        if v.len() == 1 {
            v[0].clone()
        } else {
            Value::List(v.to_vec())
        }
    };
    // If a specific longhand property was parsed, emit only that longhand
    match prop {
        "transition-property" | "animation-name" => {
            if !names.is_empty() {
                decls.push(Declaration::new(prop, single(&names), important));
            }
            return decls;
        }
        "transition-duration" | "animation-duration" => {
            if !durations.is_empty() {
                decls.push(Declaration::new(prop, single(&durations), important));
            }
            return decls;
        }
        "transition-delay" | "animation-delay" => {
            if !delays.is_empty() {
                decls.push(Declaration::new(prop, single(&delays), important));
            }
            return decls;
        }
        "transition-timing-function" | "animation-timing-function" => {
            if !timings.is_empty() {
                decls.push(Declaration::new(prop, single(&timings), important));
            }
            return decls;
        }
        "animation-iteration-count" => {
            if !iterations.is_empty() {
                decls.push(Declaration::new(prop, single(&iterations), important));
            }
            return decls;
        }
        "animation-direction" => {
            if !directions.is_empty() {
                decls.push(Declaration::new(prop, single(&directions), important));
            }
            return decls;
        }
        "animation-fill-mode" => {
            if !fills.is_empty() {
                decls.push(Declaration::new(prop, single(&fills), important));
            }
            return decls;
        }
        "animation-play-state" => {
            if !states.is_empty() {
                decls.push(Declaration::new(prop, single(&states), important));
            }
            return decls;
        }
        _ => {}
    }

    // Shorthand ("transition" or "animation"): emit all longhands
    if !names.is_empty() {
        decls.push(Declaration::new(longhand_prop, single(&names), important));
        decls.push(Declaration::new(
            if is_animation {
                "animation-duration"
            } else {
                "transition-duration"
            },
            single(&durations),
            important,
        ));
        decls.push(Declaration::new(
            if is_animation {
                "animation-timing-function"
            } else {
                "transition-timing-function"
            },
            single(&timings),
            important,
        ));
        decls.push(Declaration::new(
            if is_animation {
                "animation-delay"
            } else {
                "transition-delay"
            },
            single(&delays),
            important,
        ));
        if is_animation {
            decls.push(Declaration::new(
                "animation-iteration-count",
                single(&iterations),
                important,
            ));
            decls.push(Declaration::new(
                "animation-direction",
                single(&directions),
                important,
            ));
            decls.push(Declaration::new(
                "animation-fill-mode",
                single(&fills),
                important,
            ));
            decls.push(Declaration::new(
                "animation-play-state",
                single(&states),
                important,
            ));
        }
    }
    decls
}

/// Returns true for the `transition*` / `animation*` property family (including vendor prefixes).
pub fn is_transition_animation_property(prop: &str) -> bool {
    let p = prop
        .trim_start_matches("-webkit-")
        .trim_start_matches("-moz-")
        .trim_start_matches("-ms-")
        .trim_start_matches("-o-");
    p == "transition"
        || p == "animation"
        || p.starts_with("transition-")
        || p.starts_with("animation-")
}
