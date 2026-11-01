//! SVG attribute parsing, color conversion, length resolution, and transform parsing.

use mango_core::Color;
use tiny_skia::{FillRule, LineCap, LineJoin, Transform};

use super::ast::{
    AspectRatioAlign, MeetOrSlice, PreserveAspectRatio, SvgPaint, SvgRenderState, SvgSpreadMethod,
    SvgTextAnchor,
};

pub(crate) fn parse_attributes(s: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        // Attribute name
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
            && bytes[i] != b'>'
        {
            i += 1;
        }
        let attr_name = match std::str::from_utf8(&bytes[name_start..i]) {
            Ok(n) => n.trim().to_string(),
            Err(_) => break,
        };

        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        let mut attr_val = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1; // skip '='
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let val_start = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                if let Ok(v) = std::str::from_utf8(&bytes[val_start..i]) {
                    attr_val = v.to_string();
                }
                if i < bytes.len() {
                    i += 1; // skip quote
                }
            } else {
                let val_start = i;
                while i < bytes.len()
                    && !bytes[i].is_ascii_whitespace()
                    && bytes[i] != b'/'
                    && bytes[i] != b'>'
                {
                    i += 1;
                }
                if let Ok(v) = std::str::from_utf8(&bytes[val_start..i]) {
                    attr_val = v.to_string();
                }
            }
        }

        if !attr_name.is_empty() {
            attrs.push((attr_name, attr_val));
        }
    }

    attrs
}

pub(crate) fn apply_attributes_to_state(
    mut state: SvgRenderState,
    attrs: &[(String, String)],
) -> SvgRenderState {
    for (key, val) in attrs {
        apply_single_attribute(&mut state, key, val);
    }
    // Also parse `style="..."`
    if let Some((_, style_val)) = attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case("style")) {
        for declaration in style_val.split(';') {
            let declaration = declaration.trim();
            if declaration.is_empty() {
                continue;
            }
            if let Some((prop, v)) = declaration.split_once(':') {
                apply_single_attribute(&mut state, prop.trim(), v.trim());
            }
        }
    }
    state
}

pub(crate) fn apply_single_attribute(state: &mut SvgRenderState, key: &str, val: &str) {
    match key.to_ascii_lowercase().as_str() {
        "fill" => {
            state.fill = parse_paint(val);
        }
        "fill-rule" => {
            state.fill_rule = if val.eq_ignore_ascii_case("evenodd") {
                FillRule::EvenOdd
            } else {
                FillRule::Winding
            };
        }
        "fill-opacity" => {
            if let Ok(v) = val.parse::<f32>() {
                state.fill_opacity = v.clamp(0.0, 1.0);
            }
        }
        "stroke" => {
            state.stroke = parse_paint(val);
        }
        "stroke-width" => {
            if let Some(w) = parse_length(val) {
                state.stroke_width = w;
            }
        }
        "stroke-opacity" => {
            if let Ok(v) = val.parse::<f32>() {
                state.stroke_opacity = v.clamp(0.0, 1.0);
            }
        }
        "stroke-linecap" => {
            state.stroke_linecap = match val.to_ascii_lowercase().as_str() {
                "round" => LineCap::Round,
                "square" => LineCap::Square,
                _ => LineCap::Butt,
            };
        }
        "stroke-linejoin" => {
            state.stroke_linejoin = match val.to_ascii_lowercase().as_str() {
                "round" => LineJoin::Round,
                "bevel" => LineJoin::Bevel,
                _ => LineJoin::Miter,
            };
        }
        "stroke-dasharray" => {
            state.stroke_dasharray = parse_dasharray(val);
        }
        "stroke-dashoffset" => {
            if let Some(o) = parse_length(val) {
                state.stroke_dashoffset = o;
            }
        }
        "clip-path" => {
            state.clip_path = parse_url_id(val);
        }
        "opacity" => {
            if let Ok(v) = val.parse::<f32>() {
                state.opacity = v.clamp(0.0, 1.0);
            }
        }
        "font-size" => {
            if let Some(s) = parse_length(val) {
                state.font_size = s;
            }
        }
        "font-family" => {
            state.font_family = val.trim().to_string();
        }
        "font-weight" => {
            state.font_weight = val.trim().to_string();
        }
        "font-style" => {
            state.font_style = val.trim().to_string();
        }
        "text-anchor" => {
            state.text_anchor = match val.trim().to_ascii_lowercase().as_str() {
                "middle" => SvgTextAnchor::Middle,
                "end" => SvgTextAnchor::End,
                _ => SvgTextAnchor::Start,
            };
        }
        _ => {}
    }
}

pub(crate) fn parse_url_id(val: &str) -> Option<String> {
    let s = val.trim();
    if s.starts_with("url(") && s.ends_with(')') {
        let inner = s[4..s.len() - 1].trim();
        let stripped = inner.trim_matches(|c| c == '\'' || c == '"');
        let id = stripped.trim_start_matches('#').trim();
        if !id.is_empty() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

pub(crate) fn parse_paint(val: &str) -> Option<SvgPaint> {
    let s = val.trim();
    if s.is_empty() {
        None
    } else if s.eq_ignore_ascii_case("none") {
        Some(SvgPaint::None)
    } else if s.eq_ignore_ascii_case("currentcolor") {
        Some(SvgPaint::CurrentColor)
    } else if let Some(id) = parse_url_id(s) {
        Some(SvgPaint::Url(id))
    } else {
        parse_color_str(s).map(SvgPaint::Color)
    }
}

pub(crate) fn parse_dasharray(val: &str) -> Option<Vec<f32>> {
    let s = val.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return None;
    }
    let mut nums: Vec<f32> = s
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse::<f32>().ok())
        .filter(|&v| v >= 0.0)
        .collect();

    if nums.is_empty() {
        return None;
    }
    // SVG 1.1 §11.4: If an odd number of values is provided, repeat the list once
    if !nums.len().is_multiple_of(2) {
        let cloned = nums.clone();
        nums.extend(cloned);
    }
    Some(nums)
}

pub(crate) fn parse_preserve_aspect_ratio(val: &str) -> Option<PreserveAspectRatio> {
    let s = val.trim();
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let mut align_str = parts[0];
    let mut meet_slice_str = parts.get(1).copied();

    if align_str.eq_ignore_ascii_case("defer") {
        align_str = parts.get(1).copied().unwrap_or("xMidYMid");
        meet_slice_str = parts.get(2).copied();
    }

    let align = match align_str.to_ascii_lowercase().as_str() {
        "none" => AspectRatioAlign::None,
        "xminymin" => AspectRatioAlign::XMinYMin,
        "xmidymin" => AspectRatioAlign::XMidYMin,
        "xmaxymin" => AspectRatioAlign::XMaxYMin,
        "xminymid" => AspectRatioAlign::XMinYMid,
        "xmidymid" => AspectRatioAlign::XMidYMid,
        "xmaxymid" => AspectRatioAlign::XMaxYMid,
        "xminymax" => AspectRatioAlign::XMinYMax,
        "xmidymax" => AspectRatioAlign::XMidYMax,
        "xmaxymax" => AspectRatioAlign::XMaxYMax,
        _ => AspectRatioAlign::XMidYMid,
    };

    let meet_or_slice = match meet_slice_str.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("slice") => MeetOrSlice::Slice,
        _ => MeetOrSlice::Meet,
    };

    Some(PreserveAspectRatio {
        align,
        meet_or_slice,
    })
}

pub(crate) fn parse_spread_method(val: &str) -> SvgSpreadMethod {
    match val.trim().to_ascii_lowercase().as_str() {
        "reflect" => SvgSpreadMethod::Reflect,
        "repeat" => SvgSpreadMethod::Repeat,
        _ => SvgSpreadMethod::Pad,
    }
}

pub(crate) fn parse_color_str(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Color::rgba(r, g, b, a))
            }
            _ => None,
        }
    } else if s.starts_with("rgb(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            Some(Color::rgb(r, g, b))
        } else {
            None
        }
    } else if s.starts_with("rgba(") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 4 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            let a_f = parts[3].trim().parse::<f32>().ok()?;
            let a = (a_f * 255.0).clamp(0.0, 255.0) as u8;
            Some(Color::rgba(r, g, b, a))
        } else {
            None
        }
    } else {
        match s.to_ascii_lowercase().as_str() {
            "black" => Some(Color::BLACK),
            "white" => Some(Color::WHITE),
            "red" => Some(Color::RED),
            "green" => Some(Color::GREEN),
            "blue" => Some(Color::BLUE),
            "yellow" => Some(Color::rgb(255, 255, 0)),
            "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
            "lightgray" | "lightgrey" => Some(Color::rgb(211, 211, 211)),
            "darkgray" | "darkgrey" => Some(Color::rgb(169, 169, 169)),
            "orange" => Some(Color::rgb(255, 165, 0)),
            "purple" => Some(Color::rgb(128, 0, 128)),
            _ => None,
        }
    }
}

pub(crate) fn parse_length(val: &str) -> Option<f32> {
    let s = val.trim();
    if s.ends_with('%') {
        None
    } else {
        parse_length_with_ref(val, 0.0)
    }
}

pub(crate) fn parse_length_with_ref(val: &str, ref_size: f32) -> Option<f32> {
    let s = val.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let p = pct.trim().parse::<f32>().ok()?;
        Some((p / 100.0) * ref_size)
    } else {
        let s = s.trim_end_matches("px");
        s.parse::<f32>().ok()
    }
}

pub(crate) fn parse_view_box(val: &str) -> Option<(f32, f32, f32, f32)> {
    let nums: Vec<f32> = val
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();

    if nums.len() == 4 {
        Some((nums[0], nums[1], nums[2], nums[3]))
    } else {
        None
    }
}

pub(crate) fn parse_points(val: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = val
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();

    let mut pts = Vec::with_capacity(nums.len() / 2);
    let (chunks, _) = nums.as_chunks::<2>();
    for chunk in chunks {
        pts.push((chunk[0], chunk[1]));
    }
    pts
}

pub(crate) fn parse_transform(val: &str) -> Option<Transform> {
    let s = val.trim();
    if s.starts_with("translate(") && s.ends_with(')') {
        let inner = &s[10..s.len() - 1];
        let nums: Vec<f32> = inner
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse::<f32>().ok())
            .collect();
        if nums.len() == 1 {
            Some(Transform::from_translate(nums[0], 0.0))
        } else if nums.len() >= 2 {
            Some(Transform::from_translate(nums[0], nums[1]))
        } else {
            None
        }
    } else if s.starts_with("scale(") && s.ends_with(')') {
        let inner = &s[6..s.len() - 1];
        let nums: Vec<f32> = inner
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse::<f32>().ok())
            .collect();
        if nums.len() == 1 {
            Some(Transform::from_scale(nums[0], nums[0]))
        } else if nums.len() >= 2 {
            Some(Transform::from_scale(nums[0], nums[1]))
        } else {
            None
        }
    } else {
        None
    }
}

pub(crate) fn get_f32_attr_with_ref(
    attrs: &[(String, String)],
    name: &str,
    ref_size: f32,
) -> Option<f32> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .and_then(|(_, v)| parse_length_with_ref(v, ref_size))
}

pub(crate) fn get_str_attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim())
}
