//! Color, background, border color, shadow, mask, and filter properties.

use std::collections::HashMap;

use mango_core::Color;

use crate::computed::custom_props::is_current_color;
use crate::computed::style::ComputedStyle;
use crate::values::{
    BackgroundAttachment, BackgroundClip, BackgroundLayer, BackgroundRepeat, BackgroundSize,
    BorderImage, BoxShadow, Length, MaskComposite, Value,
};

pub(crate) fn parse_position_len(val: &Value) -> Option<Length> {
    match val {
        Value::Length(l) => Some(*l),
        Value::Percentage(p) => Some(Length::Percent(*p)),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "left" | "top" => Some(Length::Percent(0.0)),
            "center" => Some(Length::Percent(50.0)),
            "right" | "bottom" => Some(Length::Percent(100.0)),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn parse_position_val(val: &Value) -> Option<(Length, Length)> {
    match val {
        Value::Length(l) => Some((*l, Length::Percent(50.0))),
        Value::Percentage(p) => Some((Length::Percent(*p), Length::Percent(50.0))),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "center" => Some((Length::Percent(50.0), Length::Percent(50.0))),
            "top" => Some((Length::Percent(50.0), Length::Percent(0.0))),
            "bottom" => Some((Length::Percent(50.0), Length::Percent(100.0))),
            "left" => Some((Length::Percent(0.0), Length::Percent(50.0))),
            "right" => Some((Length::Percent(100.0), Length::Percent(50.0))),
            _ => None,
        },
        Value::List(items) => {
            if items.is_empty() {
                return None;
            }
            if let Some(Value::List(sub)) = items.first() {
                return parse_position_val(&Value::List(sub.clone()));
            }
            if items.len() == 1 {
                let x = parse_position_len(&items[0])?;
                Some((x, Length::Percent(50.0)))
            } else {
                let x = parse_position_len(&items[0])?;
                let y = parse_position_len(&items[1])?;
                Some((x, y))
            }
        }
        _ => None,
    }
}

pub(crate) fn parse_background_size(val: &Value) -> Option<BackgroundSize> {
    match val {
        Value::BackgroundSize(s) => Some(*s),
        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
            "auto" => Some(BackgroundSize::Auto),
            "cover" => Some(BackgroundSize::Cover),
            "contain" => Some(BackgroundSize::Contain),
            _ => None,
        },
        Value::Length(len) => Some(BackgroundSize::Explicit(*len, Length::Auto)),
        Value::Percentage(p) => Some(BackgroundSize::Explicit(Length::Percent(*p), Length::Auto)),
        Value::List(items) => {
            if items.is_empty() {
                return None;
            }
            if let Some(Value::List(sub)) = items.first() {
                return parse_background_size(&Value::List(sub.clone()));
            }
            let parse_dim = |v: &Value| match v {
                Value::Length(len) => Some(*len),
                Value::Percentage(p) => Some(Length::Percent(*p)),
                Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => Some(Length::Auto),
                _ => None,
            };
            if items.len() == 1 {
                let w = parse_dim(&items[0])?;
                Some(BackgroundSize::Explicit(w, Length::Auto))
            } else {
                let w = parse_dim(&items[0])?;
                let h = parse_dim(&items[1])?;
                Some(BackgroundSize::Explicit(w, h))
            }
        }
        _ => None,
    }
}

pub(crate) fn apply_background_image(style: &mut ComputedStyle, val: &Value) {
    match val {
        Value::Url(url) => {
            style.background_image = Some(url.clone());
            style.background_layers = vec![BackgroundLayer {
                image: Some(url.clone()),
                repeat: style.background_repeat,
                size: style.background_size,
                position: style.background_position,
                attachment: style.background_attachment,
                clip: style.background_clip,
                ..Default::default()
            }];
        }
        Value::String(url) => {
            style.background_image = Some(url.clone());
            style.background_layers = vec![BackgroundLayer {
                image: Some(url.clone()),
                repeat: style.background_repeat,
                size: style.background_size,
                position: style.background_position,
                attachment: style.background_attachment,
                clip: style.background_clip,
                ..Default::default()
            }];
        }
        Value::Gradient(g) => {
            let mut grad = (**g).clone();
            grad.resolve_current_color(style.color);
            grad.resolve_var_colors(&style.custom_properties);
            style.background_gradient = Some(grad.clone());
            style.background_layers = vec![BackgroundLayer {
                gradient: Some(Box::new(grad)),
                repeat: style.background_repeat,
                size: style.background_size,
                position: style.background_position,
                attachment: style.background_attachment,
                clip: style.background_clip,
                ..Default::default()
            }];
        }
        Value::List(items) => {
            let mut layers = Vec::new();
            for it in items {
                let mut layer = BackgroundLayer {
                    repeat: style.background_repeat,
                    size: style.background_size,
                    position: style.background_position,
                    attachment: style.background_attachment,
                    clip: style.background_clip,
                    ..Default::default()
                };
                match it {
                    Value::Url(u) | Value::String(u) => layer.image = Some(u.clone()),
                    Value::Gradient(g) => {
                        let mut grad = (**g).clone();
                        grad.resolve_current_color(style.color);
                        grad.resolve_var_colors(&style.custom_properties);
                        layer.gradient = Some(Box::new(grad));
                    }
                    _ => {}
                }
                layers.push(layer);
            }
            if let Some(first) = layers.first() {
                style.background_image = first.image.clone();
                style.background_gradient = first.gradient.as_ref().map(|g| (**g).clone());
            }
            style.background_layers = layers;
        }
        Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
            style.background_image = None;
            style.background_gradient = None;
            style.background_layers.clear();
        }
        _ => {}
    }
}

pub(crate) fn parse_box_shadow_from_values(items: &[Value]) -> Option<BoxShadow> {
    let mut offset_x = None;
    let mut offset_y = None;
    let mut blur_radius = 0.0f32;
    let mut spread_radius = 0.0f32;
    let mut color = Color::rgba(0, 0, 0, 255);
    let mut inset = false;
    let mut len_count = 0;

    for item in items {
        match item {
            Value::Keyword(k) if k.eq_ignore_ascii_case("inset") => inset = true,
            Value::Length(len) => {
                let px = match len {
                    Length::Px(p) => *p,
                    Length::Em(e) => *e * 16.0,
                    Length::Rem(r) => *r * 16.0,
                    Length::Calc(c) => c.px,
                    _ => 0.0,
                };
                match len_count {
                    0 => offset_x = Some(px),
                    1 => offset_y = Some(px),
                    2 => blur_radius = px,
                    3 => spread_radius = px,
                    _ => {}
                }
                len_count += 1;
            }
            Value::Number(n) => {
                let px = *n;
                match len_count {
                    0 => offset_x = Some(px),
                    1 => offset_y = Some(px),
                    2 => blur_radius = px,
                    3 => spread_radius = px,
                    _ => {}
                }
                len_count += 1;
            }
            Value::Color(c) => color = *c,
            Value::Var { fallback, .. } => {
                if let Some(fb) = fallback
                    && let Value::Color(c) = &**fb
                {
                    color = *c;
                }
            }
            _ => {}
        }
    }

    if let (Some(ox), Some(oy)) = (offset_x, offset_y) {
        Some(BoxShadow {
            offset_x: ox,
            offset_y: oy,
            blur_radius,
            spread_radius,
            color,
            inset,
        })
    } else {
        None
    }
}

/// Pre-applies early color so `currentColor` resolves accurately for subsequent properties.
pub(crate) fn pre_apply_color(
    style: &mut ComputedStyle,
    resolved_cascaded: &HashMap<String, Value>,
    parent_style: Option<&ComputedStyle>,
) {
    if let Some(val) = resolved_cascaded.get("color") {
        if super::css_wide::apply_css_wide_keyword(style, "color", val, parent_style) {
            return;
        }
        if let Value::Color(c) = val {
            style.color = *c;
        } else if let Value::Keyword(s) | Value::String(s) = val {
            if let Some(c) = Value::parse_color(s) {
                style.color = c;
            }
        } else if is_current_color(val)
            && let Some(p) = parent_style
        {
            style.color = p.color;
        }
    }
}

/// Pre-applies background base defaults & background-image deterministically.
pub(crate) fn pre_apply_background_defaults(
    style: &mut ComputedStyle,
    resolved_cascaded: &HashMap<String, Value>,
) {
    if let Some(val) = resolved_cascaded.get("background-repeat") {
        match val {
            Value::BackgroundRepeat(r) => style.background_repeat = *r,
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "repeat" => style.background_repeat = BackgroundRepeat::Repeat,
                "repeat-x" => style.background_repeat = BackgroundRepeat::RepeatX,
                "repeat-y" => style.background_repeat = BackgroundRepeat::RepeatY,
                "no-repeat" => style.background_repeat = BackgroundRepeat::NoRepeat,
                _ => {}
            },
            _ => {}
        }
    }
    if let Some(val) = resolved_cascaded.get("background-attachment") {
        match val {
            Value::BackgroundAttachment(a) => style.background_attachment = *a,
            Value::Keyword(k) => {
                if let Some(a) = BackgroundAttachment::parse(k) {
                    style.background_attachment = a;
                }
            }
            _ => {}
        }
    }
    if let Some(val) = resolved_cascaded.get("background-clip") {
        match val {
            Value::BackgroundClip(c) => style.background_clip = *c,
            Value::Keyword(k) => {
                if let Some(c) = BackgroundClip::parse(k) {
                    style.background_clip = c;
                }
            }
            _ => {}
        }
    }
    if let Some(pos) = resolved_cascaded
        .get("background-position")
        .and_then(parse_position_val)
    {
        style.background_position = pos;
    }
    if let Some(s) = resolved_cascaded
        .get("background-size")
        .and_then(parse_background_size)
    {
        style.background_size = s;
    }
    if let Some(val) = resolved_cascaded.get("background-image") {
        apply_background_image(style, val);
    }
}

/// Applies color, background, border-color, shadow, mask, and filter properties.
pub(crate) fn apply_colors_bg_property(
    style: &mut ComputedStyle,
    prop: &str,
    val: &Value,
    parent_style: Option<&ComputedStyle>,
) -> bool {
    match prop {
        "color" => {
            if let Value::Color(c) = val {
                style.color = *c;
            } else if let Value::Keyword(s) | Value::String(s) = val {
                if let Some(c) = Value::parse_color(s) {
                    style.color = c;
                }
            } else if is_current_color(val)
                && let Some(p) = parent_style
            {
                style.color = p.color;
            }
        }
        "background-color" => {
            if let Value::Color(c) = val {
                style.background_color = *c;
            } else if let Value::Keyword(s) | Value::String(s) = val {
                if let Some(c) = Value::parse_color(s) {
                    style.background_color = c;
                }
            } else if is_current_color(val) {
                style.background_color = style.color;
            }
        }
        "background-image" => {}
        "background-gradient" => match val {
            Value::Gradient(g) => {
                let mut grad = (**g).clone();
                grad.resolve_current_color(style.color);
                grad.resolve_var_colors(&style.custom_properties);
                style.background_gradient = Some(grad);
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                style.background_gradient = None;
            }
            _ => {}
        },
        "background-repeat" => match val {
            Value::BackgroundRepeat(r) => {
                style.background_repeat = *r;
                for layer in &mut style.background_layers {
                    layer.repeat = *r;
                }
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "repeat" => {
                    style.background_repeat = BackgroundRepeat::Repeat;
                    for layer in &mut style.background_layers {
                        layer.repeat = BackgroundRepeat::Repeat;
                    }
                }
                "repeat-x" => {
                    style.background_repeat = BackgroundRepeat::RepeatX;
                    for layer in &mut style.background_layers {
                        layer.repeat = BackgroundRepeat::RepeatX;
                    }
                }
                "repeat-y" => {
                    style.background_repeat = BackgroundRepeat::RepeatY;
                    for layer in &mut style.background_layers {
                        layer.repeat = BackgroundRepeat::RepeatY;
                    }
                }
                "no-repeat" => {
                    style.background_repeat = BackgroundRepeat::NoRepeat;
                    for layer in &mut style.background_layers {
                        layer.repeat = BackgroundRepeat::NoRepeat;
                    }
                }
                _ => {}
            },
            Value::List(items) => {
                for (i, it) in items.iter().enumerate() {
                    let rep = match it {
                        Value::BackgroundRepeat(r) => Some(*r),
                        Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                            "repeat" => Some(BackgroundRepeat::Repeat),
                            "repeat-x" => Some(BackgroundRepeat::RepeatX),
                            "repeat-y" => Some(BackgroundRepeat::RepeatY),
                            "no-repeat" => Some(BackgroundRepeat::NoRepeat),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(r) = rep
                        && i < style.background_layers.len()
                    {
                        style.background_layers[i].repeat = r;
                    }
                }
            }
            _ => {}
        },
        "background-size" => {
            if let Some(s) = parse_background_size(val) {
                style.background_size = s;
                for layer in &mut style.background_layers {
                    layer.size = s;
                }
            }
        }
        "background-position" => {
            if let Some(pos) = parse_position_val(val) {
                style.background_position = pos;
                for layer in &mut style.background_layers {
                    layer.position = pos;
                }
            }
        }
        "background-attachment" => match val {
            Value::BackgroundAttachment(a) => {
                style.background_attachment = *a;
                for layer in &mut style.background_layers {
                    layer.attachment = *a;
                }
            }
            Value::Keyword(k) => {
                if let Some(a) = BackgroundAttachment::parse(k) {
                    style.background_attachment = a;
                    for layer in &mut style.background_layers {
                        layer.attachment = a;
                    }
                }
            }
            Value::List(items) => {
                for (i, it) in items.iter().enumerate() {
                    let att = match it {
                        Value::BackgroundAttachment(a) => Some(*a),
                        Value::Keyword(k) => BackgroundAttachment::parse(k),
                        _ => None,
                    };
                    if let Some(a) = att
                        && i < style.background_layers.len()
                    {
                        style.background_layers[i].attachment = a;
                    }
                }
                if let Some(first) = style.background_layers.first() {
                    style.background_attachment = first.attachment;
                }
            }
            _ => {}
        },
        "background-clip" | "-webkit-background-clip" => match val {
            Value::BackgroundClip(c) => {
                style.background_clip = *c;
                for layer in &mut style.background_layers {
                    layer.clip = *c;
                }
            }
            Value::Keyword(k) => {
                if let Some(c) = BackgroundClip::parse(k) {
                    style.background_clip = c;
                    for layer in &mut style.background_layers {
                        layer.clip = c;
                    }
                }
            }
            Value::List(items) => {
                for (i, it) in items.iter().enumerate() {
                    let cl = match it {
                        Value::BackgroundClip(c) => Some(*c),
                        Value::Keyword(k) => BackgroundClip::parse(k),
                        _ => None,
                    };
                    if let Some(c) = cl
                        && i < style.background_layers.len()
                    {
                        style.background_layers[i].clip = c;
                    }
                }
                if let Some(first) = style.background_layers.first() {
                    style.background_clip = first.clip;
                }
            }
            _ => {}
        },
        "border-image" => {
            if let Value::BorderImage(bi) = val {
                style.border_image = Some((**bi).clone())
            }
        }
        "border-image-source" => match val {
            Value::Url(url) => {
                let bi = style.border_image.get_or_insert_with(BorderImage::default);
                bi.source = Some(url.clone());
                bi.gradient = None;
            }
            Value::Gradient(g) => {
                let bi = style.border_image.get_or_insert_with(BorderImage::default);
                bi.gradient = Some(g.clone());
                bi.source = None;
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                if let Some(bi) = &mut style.border_image {
                    bi.source = None;
                    bi.gradient = None;
                }
            }
            _ => {}
        },
        "mask-composite" | "-webkit-mask-composite" => match val {
            Value::MaskComposite(mc) => style.mask_composite = *mc,
            Value::Keyword(k) => {
                if let Some(mc) = MaskComposite::parse(k) {
                    style.mask_composite = mc;
                }
            }
            _ => {}
        },
        "mask-image" | "-webkit-mask-image" => match val {
            Value::Url(url) => style.mask_image = Some(url.clone()),
            Value::String(url) => style.mask_image = Some(url.clone()),
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                style.mask_image = None;
            }
            _ => {}
        },
        "mask-size" | "-webkit-mask-size" => match val {
            Value::BackgroundSize(s) => style.mask_size = Some(*s),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "auto" => style.mask_size = Some(BackgroundSize::Auto),
                "cover" => style.mask_size = Some(BackgroundSize::Cover),
                "contain" => style.mask_size = Some(BackgroundSize::Contain),
                _ => {}
            },
            Value::Length(len) => {
                style.mask_size = Some(BackgroundSize::Explicit(*len, Length::Auto))
            }
            Value::List(items) if items.len() >= 2 => {
                let w = match &items[0] {
                    Value::Length(len) => *len,
                    _ => Length::Auto,
                };
                let h = match &items[1] {
                    Value::Length(len) => *len,
                    _ => Length::Auto,
                };
                style.mask_size = Some(BackgroundSize::Explicit(w, h));
            }
            _ => {}
        },
        "mask-repeat" | "-webkit-mask-repeat" => match val {
            Value::BackgroundRepeat(r) => style.mask_repeat = Some(*r),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "repeat" => style.mask_repeat = Some(BackgroundRepeat::Repeat),
                "repeat-x" => style.mask_repeat = Some(BackgroundRepeat::RepeatX),
                "repeat-y" => style.mask_repeat = Some(BackgroundRepeat::RepeatY),
                "no-repeat" => style.mask_repeat = Some(BackgroundRepeat::NoRepeat),
                _ => {}
            },
            _ => {}
        },
        "mask-position" | "-webkit-mask-position" => {
            if let Some(pos) = parse_position_val(val) {
                style.mask_position = Some(pos);
            }
        }
        "mask-mode" | "-webkit-mask-mode" => match val {
            Value::MaskMode(mm) => style.mask_mode = *mm,
            Value::Keyword(k) => {
                if let Some(mm) = crate::values::MaskMode::parse(k) {
                    style.mask_mode = mm;
                }
            }
            _ => {}
        },

        // Border colors
        "border-top-color" => {
            if let Value::Color(c) = val {
                style.border_top_color = *c;
            } else if let Value::Keyword(s) | Value::String(s) = val {
                if let Some(c) = Value::parse_color(s) {
                    style.border_top_color = c;
                }
            } else if is_current_color(val) {
                style.border_top_color = style.color;
            }
        }
        "border-right-color" => {
            if let Value::Color(c) = val {
                style.border_right_color = *c;
            } else if let Value::Keyword(s) | Value::String(s) = val {
                if let Some(c) = Value::parse_color(s) {
                    style.border_right_color = c;
                }
            } else if is_current_color(val) {
                style.border_right_color = style.color;
            }
        }
        "border-bottom-color" => {
            if let Value::Color(c) = val {
                style.border_bottom_color = *c;
            } else if let Value::Keyword(s) | Value::String(s) = val {
                if let Some(c) = Value::parse_color(s) {
                    style.border_bottom_color = c;
                }
            } else if is_current_color(val) {
                style.border_bottom_color = style.color;
            }
        }
        "border-left-color" => {
            if let Value::Color(c) = val {
                style.border_left_color = *c;
            } else if let Value::Keyword(s) | Value::String(s) = val {
                if let Some(c) = Value::parse_color(s) {
                    style.border_left_color = c;
                }
            } else if is_current_color(val) {
                style.border_left_color = style.color;
            }
        }

        "box-shadow" => match val {
            Value::BoxShadow(bs) => {
                style.box_shadow = Some(*bs);
                style.box_shadows = vec![*bs];
            }
            Value::List(items) => {
                let mut shadows = Vec::new();
                for item in items {
                    if let Value::BoxShadow(bs) = item {
                        shadows.push(*bs);
                    }
                }
                if !shadows.is_empty() {
                    style.box_shadow = shadows.first().copied();
                    style.box_shadows = shadows;
                } else if let Some(bs) = parse_box_shadow_from_values(items) {
                    style.box_shadow = Some(bs);
                    style.box_shadows = vec![bs];
                }
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                style.box_shadow = None;
                style.box_shadows.clear();
            }
            _ => {}
        },
        "filter" | "-webkit-filter" => match val {
            Value::Filter(fns) => style.filter = fns.clone(),
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => style.filter.clear(),
            _ => {}
        },
        "backdrop-filter" | "-webkit-backdrop-filter" => match val {
            Value::Filter(fns) => style.backdrop_filter = fns.clone(),
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => style.backdrop_filter.clear(),
            _ => {}
        },
        "mix-blend-mode" => match val {
            Value::BlendMode(bm) => style.mix_blend_mode = *bm,
            Value::Keyword(k) => {
                if let Some(bm) = crate::values::BlendMode::parse(k) {
                    style.mix_blend_mode = bm;
                }
            }
            _ => {}
        },
        "background-blend-mode" => match val {
            Value::BlendMode(bm) => style.background_blend_mode = *bm,
            Value::Keyword(k) => {
                if let Some(bm) = crate::values::BlendMode::parse(k) {
                    style.background_blend_mode = bm;
                }
            }
            _ => {}
        },
        "clip-path" | "-webkit-clip-path" => match val {
            Value::ClipPath(cp) => style.clip_path = cp.clone(),
            Value::Keyword(k) if k.eq_ignore_ascii_case("none") => {
                style.clip_path = crate::values::ClipPath::None
            }
            _ => {}
        },

        "outline-color" => match val {
            Value::Color(c) => style.outline_color = *c,
            Value::CurrentColor => style.outline_color = style.color,
            Value::Keyword(k) => {
                if k.eq_ignore_ascii_case("invert") {
                    style.outline_color = Color::BLACK;
                } else if let Some(c) = Value::parse_color(k) {
                    style.outline_color = c;
                }
            }
            _ => {}
        },
        _ => return false,
    }
    true
}
