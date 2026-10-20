//! Shorthand expansion for `background` (W3C CSS Backgrounds and Borders Module Level 3).

use crate::properties::Declaration;
use crate::values::{Display, Length, Value};

pub(crate) fn expand_background_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    // Check if there are comma separators indicating multiple background layers
    let layer_chunks: Vec<&[Value]> = items
        .split(|item| matches!(item, Value::Keyword(k) if k == ","))
        .collect();

    if layer_chunks.len() > 1 {
        let mut images = Vec::new();
        let mut repeats = Vec::new();
        let mut sizes = Vec::new();
        let mut positions = Vec::new();
        let mut attachments = Vec::new();
        let mut clips = Vec::new();
        let mut color = None;

        for chunk in layer_chunks {
            let mut layer_img = None;
            let mut layer_repeat = None;
            let mut layer_size = None;
            let mut layer_pos = Vec::new();
            let mut layer_sizes = Vec::new();
            let mut in_size = false;
            let mut layer_att = None;
            let mut layer_clip = None;

            for item in chunk {
                if let Value::Keyword(k) = item
                    && k == "/"
                {
                    in_size = true;
                    continue;
                }
                match item {
                    Value::Url(_) | Value::Gradient(_) => {
                        if layer_img.is_none() {
                            layer_img = Some(item.clone());
                        }
                    }
                    Value::BackgroundRepeat(_) => layer_repeat = Some(item.clone()),
                    Value::BackgroundSize(_) => layer_size = Some(item.clone()),
                    Value::BackgroundAttachment(_) => layer_att = Some(item.clone()),
                    Value::BackgroundClip(_) => layer_clip = Some(item.clone()),
                    Value::Color(_) | Value::Var { .. } | Value::CurrentColor => {
                        color = Some(item.clone())
                    }
                    Value::Length(_) | Value::Percentage(_) => {
                        if in_size {
                            layer_sizes.push(item.clone());
                        } else {
                            layer_pos.push(item.clone());
                        }
                    }
                    Value::TextAlign(crate::values::TextAlign::Center) => {
                        if !in_size {
                            layer_pos.push(Value::Keyword("center".to_string()));
                        }
                    }
                    Value::Float(crate::values::Float::Left) => {
                        if !in_size {
                            layer_pos.push(Value::Keyword("left".to_string()));
                        }
                    }
                    Value::Float(crate::values::Float::Right) => {
                        if !in_size {
                            layer_pos.push(Value::Keyword("right".to_string()));
                        }
                    }
                    Value::VerticalAlign(crate::values::VerticalAlign::Top) => {
                        if !in_size {
                            layer_pos.push(Value::Keyword("top".to_string()));
                        }
                    }
                    Value::VerticalAlign(crate::values::VerticalAlign::Bottom) => {
                        if !in_size {
                            layer_pos.push(Value::Keyword("bottom".to_string()));
                        }
                    }
                    Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                        "none" => {
                            layer_img = Some(Value::Keyword("none".to_string()));
                        }
                        "repeat" | "repeat-x" | "repeat-y" | "no-repeat" => {
                            layer_repeat = Some(item.clone())
                        }
                        "cover" | "contain" => layer_size = Some(item.clone()),
                        "auto" => {
                            if in_size {
                                layer_sizes.push(item.clone());
                            } else {
                                layer_pos.push(item.clone());
                            }
                        }
                        "top" | "bottom" | "left" | "right" | "center" => {
                            layer_pos.push(item.clone());
                        }
                        "scroll" | "fixed" | "local" => {
                            if let Some(a) = crate::values::BackgroundAttachment::parse(k) {
                                layer_att = Some(Value::BackgroundAttachment(a));
                            }
                        }
                        "border-box" | "padding-box" | "content-box" => {
                            if let Some(c) = crate::values::BackgroundClip::parse(k) {
                                layer_clip = Some(Value::BackgroundClip(c));
                            }
                        }
                        "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                        "currentcolor" => color = Some(Value::CurrentColor),
                        _ => {}
                    },
                    _ => {}
                }
            }

            images.push(layer_img.unwrap_or(Value::Keyword("none".to_string())));
            if let Some(r) = layer_repeat {
                repeats.push(r);
            }
            if let Some(s) = layer_size {
                sizes.push(s);
            } else if !layer_sizes.is_empty() {
                sizes.push(if layer_sizes.len() == 1 {
                    layer_sizes[0].clone()
                } else {
                    Value::List(layer_sizes)
                });
            }
            if !layer_pos.is_empty() {
                positions.push(if layer_pos.len() == 1 {
                    layer_pos[0].clone()
                } else {
                    Value::List(layer_pos)
                });
            }
            if let Some(a) = layer_att {
                attachments.push(a);
            }
            if let Some(c) = layer_clip {
                clips.push(c);
            }
        }

        let mut decls = Vec::new();
        if let Some(c) = color {
            decls.push(Declaration::new("background-color", c, important));
        }
        if !images.is_empty() {
            decls.push(Declaration::new(
                "background-image",
                Value::List(images),
                important,
            ));
        }
        if !repeats.is_empty() {
            decls.push(Declaration::new(
                "background-repeat",
                Value::List(repeats),
                important,
            ));
        }
        if !sizes.is_empty() {
            decls.push(Declaration::new(
                "background-size",
                Value::List(sizes),
                important,
            ));
        }
        if !positions.is_empty() {
            decls.push(Declaration::new(
                "background-position",
                Value::List(positions),
                important,
            ));
        }
        if !attachments.is_empty() {
            decls.push(Declaration::new(
                "background-attachment",
                Value::List(attachments),
                important,
            ));
        }
        if !clips.is_empty() {
            decls.push(Declaration::new(
                "background-clip",
                Value::List(clips),
                important,
            ));
        }
        return decls;
    }

    let mut decls = Vec::new();
    let mut image = None;
    let mut gradient = None;
    let mut repeat = None;
    let mut color = None;
    let mut size = None;
    let mut positions = Vec::new();
    let mut sizes = Vec::new();
    let mut in_size = false;
    let mut attachment = None;
    let mut clip = None;

    for item in items {
        if let Value::Keyword(k) = item
            && k == "/"
        {
            in_size = true;
            continue;
        }
        match item {
            Value::Url(_) => {
                if image.is_none() {
                    image = Some(item.clone());
                }
            }
            Value::Gradient(_) => {
                if gradient.is_none() {
                    gradient = Some(item.clone());
                }
            }
            Value::BackgroundRepeat(_) => repeat = Some(item.clone()),
            Value::BackgroundSize(_) => size = Some(item.clone()),
            Value::BackgroundAttachment(_) => attachment = Some(item.clone()),
            Value::BackgroundClip(_) => clip = Some(item.clone()),
            Value::Color(_) | Value::Var { .. } | Value::CurrentColor => color = Some(item.clone()),
            Value::Length(_) | Value::Percentage(_) => {
                if in_size {
                    sizes.push(item.clone());
                } else {
                    positions.push(item.clone());
                }
            }
            Value::TextAlign(crate::values::TextAlign::Center) => {
                if !in_size {
                    positions.push(Value::Keyword("center".to_string()));
                }
            }
            Value::Float(crate::values::Float::Left) => {
                if !in_size {
                    positions.push(Value::Keyword("left".to_string()));
                }
            }
            Value::Float(crate::values::Float::Right) => {
                if !in_size {
                    positions.push(Value::Keyword("right".to_string()));
                }
            }
            Value::VerticalAlign(crate::values::VerticalAlign::Top) => {
                if !in_size {
                    positions.push(Value::Keyword("top".to_string()));
                }
            }
            Value::VerticalAlign(crate::values::VerticalAlign::Bottom) => {
                if !in_size {
                    positions.push(Value::Keyword("bottom".to_string()));
                }
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" => {
                    image = Some(Value::Keyword("none".to_string()));
                    if color.is_none() {
                        color = Some(Value::Color(mango_core::Color::TRANSPARENT));
                    }
                }
                "repeat" | "repeat-x" | "repeat-y" | "no-repeat" => repeat = Some(item.clone()),
                "cover" | "contain" => size = Some(item.clone()),
                "auto" => {
                    if in_size {
                        sizes.push(item.clone());
                    } else {
                        positions.push(item.clone());
                    }
                }
                "top" | "bottom" | "left" | "right" | "center" => {
                    positions.push(item.clone());
                }
                "scroll" | "fixed" | "local" => {
                    if let Some(a) = crate::values::BackgroundAttachment::parse(k) {
                        attachment = Some(Value::BackgroundAttachment(a));
                    }
                }
                "border-box" | "padding-box" | "content-box" => {
                    if let Some(c) = crate::values::BackgroundClip::parse(k) {
                        clip = Some(Value::BackgroundClip(c));
                    }
                }
                "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                "currentcolor" => color = Some(Value::CurrentColor),
                _ => {}
            },
            Value::Display(Display::None) => {
                image = Some(Value::Keyword("none".to_string()));
                if color.is_none() {
                    color = Some(Value::Color(mango_core::Color::TRANSPARENT));
                }
            }
            _ => {}
        }
    }

    let found_any = color.is_some()
        || image.is_some()
        || gradient.is_some()
        || repeat.is_some()
        || size.is_some()
        || !sizes.is_empty()
        || !positions.is_empty()
        || attachment.is_some()
        || clip.is_some();

    if !found_any {
        return vec![Declaration::new("background", value.clone(), important)];
    }

    let c = color.unwrap_or(Value::Color(mango_core::Color::TRANSPARENT));
    decls.push(Declaration::new("background-color", c, important));

    if let Some(img) = image {
        decls.push(Declaration::new("background-image", img, important));
        if let Some(grad) = gradient {
            decls.push(Declaration::new("background-gradient", grad, important));
        }
    } else if let Some(grad) = gradient {
        decls.push(Declaration::new(
            "background-image",
            grad.clone(),
            important,
        ));
        decls.push(Declaration::new("background-gradient", grad, important));
    } else {
        decls.push(Declaration::new(
            "background-image",
            Value::Keyword("none".to_string()),
            important,
        ));
    }

    let r = repeat.unwrap_or(Value::BackgroundRepeat(
        crate::values::BackgroundRepeat::Repeat,
    ));
    decls.push(Declaration::new("background-repeat", r, important));

    if let Some(s) = size {
        decls.push(Declaration::new("background-size", s, important));
    } else if !sizes.is_empty() {
        if sizes.len() == 1 {
            decls.push(Declaration::new(
                "background-size",
                sizes[0].clone(),
                important,
            ));
        } else {
            decls.push(Declaration::new(
                "background-size",
                Value::List(sizes),
                important,
            ));
        }
    } else {
        decls.push(Declaration::new(
            "background-size",
            Value::BackgroundSize(crate::values::BackgroundSize::Auto),
            important,
        ));
    }

    if !positions.is_empty() {
        if positions.len() == 1 {
            decls.push(Declaration::new(
                "background-position",
                positions[0].clone(),
                important,
            ));
        } else {
            decls.push(Declaration::new(
                "background-position",
                Value::List(positions),
                important,
            ));
        }
    } else {
        decls.push(Declaration::new(
            "background-position",
            Value::List(vec![
                Value::Length(Length::Percent(0.0)),
                Value::Length(Length::Percent(0.0)),
            ]),
            important,
        ));
    }

    let a = attachment.unwrap_or(Value::BackgroundAttachment(
        crate::values::BackgroundAttachment::Scroll,
    ));
    decls.push(Declaration::new("background-attachment", a, important));

    let cl = clip.unwrap_or(Value::BackgroundClip(
        crate::values::BackgroundClip::BorderBox,
    ));
    decls.push(Declaration::new("background-clip", cl, important));

    decls
}
