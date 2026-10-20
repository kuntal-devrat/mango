//! Transitions, animations, transforms, motion paths, and timeline properties.

use crate::computed::custom_props::is_current_color;
use crate::computed::style::ComputedStyle;
use crate::values::{
    Animation, AnimationDirection, AnimationFillMode, AnimationIterationCount, AnimationPlayState,
    Float, Isolation, Length, TextAlign, Transition, Value, VerticalAlign,
};

#[inline]
fn get_keyword_str(v: &Value) -> Option<&str> {
    match v {
        Value::Keyword(k) | Value::String(k) => Some(k.as_str()),
        Value::Float(Float::Left) => Some("left"),
        Value::Float(Float::Right) => Some("right"),
        Value::TextAlign(TextAlign::Center) => Some("center"),
        Value::VerticalAlign(VerticalAlign::Top) => Some("top"),
        Value::VerticalAlign(VerticalAlign::Bottom) => Some("bottom"),
        _ => None,
    }
}

/// Builds up `style.transitions` from one `transition-*` longhand declaration.
fn apply_transition_longhand(style: &mut ComputedStyle, prop: &str, val: &Value) {
    let values: Vec<Value> = match val {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    if values.is_empty() {
        return;
    }
    if style.transitions.len() < values.len() {
        style
            .transitions
            .resize(values.len(), Transition::default());
    }

    let time_ms = |v: &Value| -> Option<f32> {
        match v {
            Value::Time(ms) => Some(*ms),
            Value::Number(n) => Some(*n),
            _ => None,
        }
    };

    for (i, value) in values.iter().enumerate() {
        let idx = if i < style.transitions.len() {
            i
        } else {
            i % style.transitions.len()
        };
        let entry = &mut style.transitions[idx];
        match prop {
            "transition-property" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.property = k.to_ascii_lowercase();
                }
            }
            "transition-duration" => {
                if let Some(ms) = time_ms(value) {
                    entry.duration_ms = ms;
                }
            }
            "transition-delay" => {
                if let Some(ms) = time_ms(value) {
                    entry.delay_ms = ms;
                }
            }
            "transition-timing-function" => {
                if let Value::TimingFunction(tf) = value {
                    entry.timing = *tf;
                } else if let Value::Keyword(k) | Value::String(k) = value
                    && let Some(tf) = crate::values::parse_timing_function(k)
                {
                    entry.timing = tf;
                }
            }
            _ => {}
        }
    }
}

/// Builds up `style.animations` from one `animation-*` longhand declaration.
fn apply_animation_longhand(style: &mut ComputedStyle, prop: &str, val: &Value) {
    if prop == "animation-name"
        && let Value::Keyword(k) = val
        && k.eq_ignore_ascii_case("none")
    {
        style.animations.clear();
        return;
    }

    let values: Vec<Value> = match val {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };
    if values.is_empty() {
        return;
    }
    if style.animations.len() < values.len() {
        style.animations.resize(values.len(), Animation::default());
    }

    for (i, value) in values.iter().enumerate() {
        let idx = if i < style.animations.len() {
            i
        } else {
            i % style.animations.len()
        };
        let entry = &mut style.animations[idx];
        match prop {
            "animation-name" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.name = k.clone();
                }
            }
            "animation-duration" => match value {
                Value::Time(ms) => entry.duration_ms = *ms,
                Value::Number(n) => entry.duration_ms = *n,
                _ => {}
            },
            "animation-delay" => match value {
                Value::Time(ms) => entry.delay_ms = *ms,
                Value::Number(n) => entry.delay_ms = *n,
                _ => {}
            },
            "animation-timing-function" => {
                if let Value::TimingFunction(tf) = value {
                    entry.timing = *tf;
                } else if let Value::Keyword(k) | Value::String(k) = value
                    && let Some(tf) = crate::values::parse_timing_function(k)
                {
                    entry.timing = tf;
                }
            }
            "animation-iteration-count" => match value {
                Value::Number(n) => entry.iteration_count = AnimationIterationCount::Finite(*n),
                Value::Keyword(k) if k.eq_ignore_ascii_case("infinite") => {
                    entry.iteration_count = AnimationIterationCount::Infinite
                }
                _ => {}
            },
            "animation-direction" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.direction = match k.to_ascii_lowercase().as_str() {
                        "reverse" => AnimationDirection::Reverse,
                        "alternate" => AnimationDirection::Alternate,
                        "alternate-reverse" => AnimationDirection::AlternateReverse,
                        _ => AnimationDirection::Normal,
                    };
                }
            }
            "animation-fill-mode" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.fill_mode = match k.to_ascii_lowercase().as_str() {
                        "forwards" => AnimationFillMode::Forwards,
                        "backwards" => AnimationFillMode::Backwards,
                        "both" => AnimationFillMode::Both,
                        _ => AnimationFillMode::None,
                    };
                }
            }
            "animation-play-state" => {
                if let Value::Keyword(k) | Value::String(k) = value {
                    entry.play_state = if k.eq_ignore_ascii_case("paused") {
                        AnimationPlayState::Paused
                    } else {
                        AnimationPlayState::Running
                    };
                }
            }
            _ => {}
        }
    }
}

/// Applies animations, transitions, transforms, timelines, container query metadata, and related properties.
pub(crate) fn apply_animation_property(style: &mut ComputedStyle, prop: &str, val: &Value) -> bool {
    match prop {
        "isolation" => match val {
            Value::Isolation(iso) => style.isolation = *iso,
            Value::Keyword(k) => {
                if let Some(iso) = Isolation::parse(k) {
                    style.isolation = iso;
                }
            }
            _ => {}
        },
        "container-type" => match val {
            Value::ContainerType(ct) => style.container_type = *ct,
            Value::Keyword(k) | Value::String(k) => {
                if let Some(ct) = crate::values::ContainerType::parse(k) {
                    style.container_type = ct;
                }
            }
            _ => {}
        },
        "container-name" => match val {
            Value::String(s) | Value::Keyword(s) => {
                if s.eq_ignore_ascii_case("none") {
                    style.container_name = None;
                } else {
                    style.container_name = Some(s.clone());
                }
            }
            _ => {}
        },

        // Transform properties
        "transform" => match val {
            Value::Transform(t) => style.transform = t.clone(),
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                style.transform = crate::values::Transform::default();
            }
            _ => {}
        },
        "transform-origin-x" => match val {
            Value::Length(l) => style.transform_origin_x = *l,
            Value::Float(Float::Left) => style.transform_origin_x = Length::Percent(0.0),
            Value::Float(Float::Right) => style.transform_origin_x = Length::Percent(100.0),
            Value::TextAlign(TextAlign::Center) => style.transform_origin_x = Length::Percent(50.0),
            Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                "left" => style.transform_origin_x = Length::Percent(0.0),
                "center" => style.transform_origin_x = Length::Percent(50.0),
                "right" => style.transform_origin_x = Length::Percent(100.0),
                _ => {}
            },
            _ => {}
        },
        "transform-origin-y" => match val {
            Value::Length(l) => style.transform_origin_y = *l,
            Value::TextAlign(TextAlign::Center) => style.transform_origin_y = Length::Percent(50.0),
            Value::VerticalAlign(VerticalAlign::Top) => {
                style.transform_origin_y = Length::Percent(0.0)
            }
            Value::VerticalAlign(VerticalAlign::Bottom) => {
                style.transform_origin_y = Length::Percent(100.0)
            }
            Value::Keyword(k) | Value::String(k) => match k.to_ascii_lowercase().as_str() {
                "top" => style.transform_origin_y = Length::Percent(0.0),
                "center" => style.transform_origin_y = Length::Percent(50.0),
                "bottom" => style.transform_origin_y = Length::Percent(100.0),
                _ => {}
            },
            _ => {}
        },
        "transform-origin-z" => match val {
            Value::Length(l) => style.transform_origin_z = *l,
            Value::Number(n) if *n == 0.0 => style.transform_origin_z = Length::Px(0.0),
            _ => {}
        },
        "transform-origin" => {
            let items: Vec<Value> = match val {
                Value::List(items) => items.clone(),
                single => vec![single.clone()],
            };

            let parse_x = |v: &Value| -> Option<Length> {
                match v {
                    Value::Length(l) => Some(*l),
                    other => match get_keyword_str(other)?.to_ascii_lowercase().as_str() {
                        "left" => Some(Length::Percent(0.0)),
                        "center" => Some(Length::Percent(50.0)),
                        "right" => Some(Length::Percent(100.0)),
                        _ => None,
                    },
                }
            };

            let parse_y = |v: &Value| -> Option<Length> {
                match v {
                    Value::Length(l) => Some(*l),
                    other => match get_keyword_str(other)?.to_ascii_lowercase().as_str() {
                        "top" => Some(Length::Percent(0.0)),
                        "center" => Some(Length::Percent(50.0)),
                        "bottom" => Some(Length::Percent(100.0)),
                        _ => None,
                    },
                }
            };

            let is_y_keyword = |v: &Value| -> bool {
                if let Some(kw) = get_keyword_str(v) {
                    let s = kw.to_ascii_lowercase();
                    s == "top" || s == "bottom"
                } else {
                    false
                }
            };

            let is_x_keyword = |v: &Value| -> bool {
                if let Some(kw) = get_keyword_str(v) {
                    let s = kw.to_ascii_lowercase();
                    s == "left" || s == "right"
                } else {
                    false
                }
            };

            match items.len() {
                1 => {
                    let item = &items[0];
                    if is_y_keyword(item) {
                        style.transform_origin_x = Length::Percent(50.0);
                        if let Some(y) = parse_y(item) {
                            style.transform_origin_y = y;
                        }
                    } else {
                        if let Some(x) = parse_x(item) {
                            style.transform_origin_x = x;
                        }
                        style.transform_origin_y = Length::Percent(50.0);
                    }
                    style.transform_origin_z = Length::Px(0.0);
                }
                2 => {
                    let (item1, item2) = (&items[0], &items[1]);
                    if is_y_keyword(item1) || is_x_keyword(item2) {
                        if let Some(y) = parse_y(item1) {
                            style.transform_origin_y = y;
                        }
                        if let Some(x) = parse_x(item2) {
                            style.transform_origin_x = x;
                        }
                    } else {
                        if let Some(x) = parse_x(item1) {
                            style.transform_origin_x = x;
                        }
                        if let Some(y) = parse_y(item2) {
                            style.transform_origin_y = y;
                        }
                    }
                    style.transform_origin_z = Length::Px(0.0);
                }
                3.. => {
                    let (item1, item2, item3) = (&items[0], &items[1], &items[2]);
                    if is_y_keyword(item1) || is_x_keyword(item2) {
                        if let Some(y) = parse_y(item1) {
                            style.transform_origin_y = y;
                        }
                        if let Some(x) = parse_x(item2) {
                            style.transform_origin_x = x;
                        }
                    } else {
                        if let Some(x) = parse_x(item1) {
                            style.transform_origin_x = x;
                        }
                        if let Some(y) = parse_y(item2) {
                            style.transform_origin_y = y;
                        }
                    }
                    if let Value::Length(z) = item3 {
                        style.transform_origin_z = *z;
                    } else if let Value::Number(n) = item3
                        && *n == 0.0
                    {
                        style.transform_origin_z = Length::Px(0.0);
                    }
                }
                _ => {}
            }
        }

        "transition-property"
        | "transition-duration"
        | "transition-timing-function"
        | "transition-delay" => {
            apply_transition_longhand(style, prop, val);
        }
        "animation-name"
        | "animation-duration"
        | "animation-timing-function"
        | "animation-delay"
        | "animation-iteration-count"
        | "animation-direction"
        | "animation-fill-mode"
        | "animation-play-state" => {
            apply_animation_longhand(style, prop, val);
        }
        "will-change" => match val {
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                style.will_change = false;
                style.will_change_properties.clear();
            }
            Value::Keyword(k) => {
                style.will_change = true;
                style.will_change_properties = vec![k.clone()];
            }
            Value::List(items) => {
                let mut props = Vec::new();
                for item in items {
                    if let Value::Keyword(k) = item
                        && !k.eq_ignore_ascii_case("auto")
                    {
                        props.push(k.clone());
                    }
                }
                style.will_change = !props.is_empty();
                style.will_change_properties = props;
            }
            _ => {
                style.will_change = true;
            }
        },
        "aspect-ratio" => match val {
            Value::Number(n) if *n > 0.0 => style.aspect_ratio = Some(*n),
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => style.aspect_ratio = None,
            Value::List(items) if items.len() >= 2 => {
                if let (Value::Number(w), Value::Number(h)) = (&items[0], &items[items.len() - 1])
                    && *h > 0.0
                {
                    style.aspect_ratio = Some(*w / *h);
                }
            }
            _ => {}
        },
        "object-fit" => match val {
            Value::ObjectFit(of) => style.object_fit = *of,
            Value::BackgroundSize(crate::values::BackgroundSize::Cover) => {
                style.object_fit = crate::values::ObjectFit::Cover;
            }
            Value::BackgroundSize(crate::values::BackgroundSize::Contain) => {
                style.object_fit = crate::values::ObjectFit::Contain;
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "fill" => style.object_fit = crate::values::ObjectFit::Fill,
                "contain" => style.object_fit = crate::values::ObjectFit::Contain,
                "cover" => style.object_fit = crate::values::ObjectFit::Cover,
                "none" => style.object_fit = crate::values::ObjectFit::None,
                "scale-down" => style.object_fit = crate::values::ObjectFit::ScaleDown,
                _ => {}
            },
            _ => {}
        },
        "object-position" => match val {
            Value::List(items) if items.len() >= 2 => {
                if let (Value::Length(x), Value::Length(y)) = (&items[0], &items[1]) {
                    style.object_position = (*x, *y);
                }
            }
            Value::Length(l) => {
                style.object_position = (*l, *l);
            }
            _ => {}
        },
        "accent-color" => {
            if is_current_color(val) {
                style.accent_color = Some(style.color);
            } else if let Value::Color(c) = val {
                style.accent_color = Some(*c);
            } else if let Value::Keyword(k) | Value::String(k) = val {
                if k.eq_ignore_ascii_case("auto") {
                    style.accent_color = None;
                } else if let Some(c) = Value::parse_color(k) {
                    style.accent_color = Some(c);
                }
            }
        }
        "anchor-name" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("none") {
                    style.anchor_name = None;
                } else {
                    style.anchor_name = Some(k.clone());
                }
            }
            _ => {}
        },
        "position-anchor" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("auto") {
                    style.position_anchor = None;
                } else {
                    style.position_anchor = Some(k.clone());
                }
            }
            _ => {}
        },
        "position-try-fallbacks" | "position-try-options" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("none") {
                    style.position_try_fallbacks.clear();
                } else {
                    style.position_try_fallbacks = k
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
            Value::List(items) => {
                style.position_try_fallbacks = items
                    .iter()
                    .filter_map(|it| match it {
                        Value::Keyword(s) | Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
            }
            _ => {}
        },
        "scroll-timeline-name" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("none") {
                    style.scroll_timeline_name = None;
                } else {
                    style.scroll_timeline_name = Some(k.clone());
                }
            }
            _ => {}
        },
        "scroll-timeline-axis" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(axis) = crate::values::ScrollTimelineAxis::parse(k) {
                    style.scroll_timeline_axis = axis;
                }
            }
            _ => {}
        },
        "scroll-timeline" => match val {
            Value::Keyword(k) | Value::String(k) => {
                let parts: Vec<&str> = k.split_whitespace().collect();
                if let Some(&first) = parts.first() {
                    if !first.eq_ignore_ascii_case("none") {
                        style.scroll_timeline_name = Some(first.to_string());
                    }
                }
                if let Some(&second) = parts.get(1) {
                    if let Some(axis) = crate::values::ScrollTimelineAxis::parse(second) {
                        style.scroll_timeline_axis = axis;
                    }
                }
            }
            _ => {}
        },
        "view-timeline-name" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("none") {
                    style.view_timeline_name = None;
                } else {
                    style.view_timeline_name = Some(k.clone());
                }
            }
            _ => {}
        },
        "view-timeline-axis" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if let Some(axis) = crate::values::ScrollTimelineAxis::parse(k) {
                    style.view_timeline_axis = axis;
                }
            }
            _ => {}
        },
        "view-timeline" => match val {
            Value::Keyword(k) | Value::String(k) => {
                let parts: Vec<&str> = k.split_whitespace().collect();
                if let Some(&first) = parts.first() {
                    if !first.eq_ignore_ascii_case("none") {
                        style.view_timeline_name = Some(first.to_string());
                    }
                }
                if let Some(&second) = parts.get(1) {
                    if let Some(axis) = crate::values::ScrollTimelineAxis::parse(second) {
                        style.view_timeline_axis = axis;
                    }
                }
            }
            _ => {}
        },
        "animation-timeline" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("none") || k.eq_ignore_ascii_case("auto") {
                    style.animation_timeline = None;
                } else {
                    style.animation_timeline = Some(k.clone());
                }
            }
            _ => {}
        },
        "view-transition-name" => match val {
            Value::Keyword(k) | Value::String(k) => {
                if k.eq_ignore_ascii_case("none") {
                    style.view_transition_name = None;
                } else {
                    style.view_transition_name = Some(k.clone());
                }
            }
            _ => {}
        },
        _ => return false,
    }
    true
}
