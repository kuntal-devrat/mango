//! Lightweight XML tokenizer and SVG attribute parser.

use mango_core::Color;
use std::collections::HashMap;
use tiny_skia::{FillRule, LineCap, LineJoin, Transform};

use super::ast::{
    SvgContainer, SvgDocument, SvgNode, SvgPaint, SvgRenderState, SvgSymbol, add_node_to_container,
    get_cur_state,
};
use super::cache::register_global_svg_symbol;
use super::path::parse_svg_path;

#[allow(clippy::enum_variant_names)]
pub(crate) enum XmlToken {
    StartTag(String, Vec<(String, String)>),
    EmptyTag(String, Vec<(String, String)>),
    EndTag(String),
}

pub(crate) struct XmlTokenizer<'a> {
    content: &'a str,
    pos: usize,
}

impl<'a> XmlTokenizer<'a> {
    pub(crate) fn new(content: &'a str) -> Self {
        Self { content, pos: 0 }
    }

    pub(crate) fn next_token(&mut self) -> Option<XmlToken> {
        let bytes = self.content.as_bytes();
        while self.pos < bytes.len() {
            // Find '<'
            if bytes[self.pos] != b'<' {
                self.pos += 1;
                continue;
            }

            // Check for comment <!--
            if self.pos + 4 <= bytes.len() && &bytes[self.pos..self.pos + 4] == b"<!--" {
                self.pos += 4;
                if let Some(end) = self.content[self.pos..].find("-->") {
                    self.pos += end + 3;
                } else {
                    self.pos = bytes.len();
                }
                continue;
            }

            // Check for processing instruction <? ... ?> or <! ... >
            if self.pos + 2 <= bytes.len()
                && (bytes[self.pos + 1] == b'?' || bytes[self.pos + 1] == b'!')
            {
                if let Some(end) = self.content[self.pos..].find('>') {
                    self.pos += end + 1;
                } else {
                    self.pos = bytes.len();
                }
                continue;
            }

            // End tag </name>
            if self.pos + 2 <= bytes.len() && bytes[self.pos + 1] == b'/' {
                self.pos += 2;
                let start = self.pos;
                while self.pos < bytes.len() && bytes[self.pos] != b'>' {
                    self.pos += 1;
                }
                let name = self.content[start..self.pos].trim().to_string();
                if self.pos < bytes.len() {
                    self.pos += 1; // skip '>'
                }
                return Some(XmlToken::EndTag(name));
            }

            // Start or Empty tag <name attrs...>
            self.pos += 1; // skip '<'
            let mut tag_content = String::new();
            let mut in_quote = None;

            while self.pos < bytes.len() {
                let c = bytes[self.pos] as char;
                self.pos += 1;

                if let Some(q) = in_quote {
                    if c == q {
                        in_quote = None;
                    }
                    tag_content.push(c);
                } else if c == '"' || c == '\'' {
                    in_quote = Some(c);
                    tag_content.push(c);
                } else if c == '>' {
                    break;
                } else {
                    tag_content.push(c);
                }
            }

            let trimmed = tag_content.trim();
            let is_self_closing = trimmed.ends_with('/');
            let inner = if is_self_closing {
                trimmed[..trimmed.len() - 1].trim()
            } else {
                trimmed
            };

            let mut parts = inner.split_whitespace();
            let tag_name = parts.next()?.to_string();

            let attrs = parse_attributes(&inner[tag_name.len()..]);

            if is_self_closing {
                return Some(XmlToken::EmptyTag(tag_name, attrs));
            } else {
                return Some(XmlToken::StartTag(tag_name, attrs));
            }
        }
        None
    }
}

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

// ---------------------------------------------------------------------------
// Attributes and CSS styles helpers
// ---------------------------------------------------------------------------

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
        "opacity" => {
            if let Ok(v) = val.parse::<f32>() {
                state.opacity = v.clamp(0.0, 1.0);
            }
        }
        _ => {}
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
    } else {
        parse_color_str(s).map(SvgPaint::Color)
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
    parse_length_with_ref(val, 0.0)
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

#[allow(dead_code)]
pub(crate) fn get_f32_attr(attrs: &[(String, String)], name: &str) -> Option<f32> {
    get_f32_attr_with_ref(attrs, name, 0.0)
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

pub(crate) fn parse_leaf_element(
    name: &str,
    attrs: &[(String, String)],
    cur_state: &SvgRenderState,
    ref_w: f32,
    ref_h: f32,
) -> Option<SvgNode> {
    let state = apply_attributes_to_state(cur_state.clone(), attrs);
    let tag_name = name.to_ascii_lowercase();
    match tag_name.as_str() {
        "path" => {
            if let Some((_, d)) = attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case("d")) {
                parse_svg_path(d).map(|path| SvgNode::Path { path, state })
            } else {
                None
            }
        }
        "rect" => {
            let x = get_f32_attr_with_ref(attrs, "x", ref_w).unwrap_or(0.0);
            let y = get_f32_attr_with_ref(attrs, "y", ref_h).unwrap_or(0.0);
            let w = get_f32_attr_with_ref(attrs, "width", ref_w).unwrap_or(0.0);
            let h = get_f32_attr_with_ref(attrs, "height", ref_h).unwrap_or(0.0);
            let rx = get_f32_attr_with_ref(attrs, "rx", ref_w).unwrap_or(0.0);
            let ry = get_f32_attr_with_ref(attrs, "ry", ref_h).unwrap_or(rx);
            if w > 0.0 && h > 0.0 {
                Some(SvgNode::Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                    rx,
                    ry,
                    state,
                })
            } else {
                None
            }
        }
        "circle" => {
            let cx = get_f32_attr_with_ref(attrs, "cx", ref_w).unwrap_or(0.0);
            let cy = get_f32_attr_with_ref(attrs, "cy", ref_h).unwrap_or(0.0);
            let ref_diag = ((ref_w * ref_w + ref_h * ref_h) / 2.0).sqrt();
            let r = get_f32_attr_with_ref(attrs, "r", ref_diag).unwrap_or(0.0);
            if r > 0.0 {
                Some(SvgNode::Circle { cx, cy, r, state })
            } else {
                None
            }
        }
        "ellipse" => {
            let cx = get_f32_attr_with_ref(attrs, "cx", ref_w).unwrap_or(0.0);
            let cy = get_f32_attr_with_ref(attrs, "cy", ref_h).unwrap_or(0.0);
            let rx = get_f32_attr_with_ref(attrs, "rx", ref_w).unwrap_or(0.0);
            let ry = get_f32_attr_with_ref(attrs, "ry", ref_h).unwrap_or(0.0);
            if rx > 0.0 && ry > 0.0 {
                Some(SvgNode::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    state,
                })
            } else {
                None
            }
        }
        "line" => {
            let x1 = get_f32_attr_with_ref(attrs, "x1", ref_w).unwrap_or(0.0);
            let y1 = get_f32_attr_with_ref(attrs, "y1", ref_h).unwrap_or(0.0);
            let x2 = get_f32_attr_with_ref(attrs, "x2", ref_w).unwrap_or(0.0);
            let y2 = get_f32_attr_with_ref(attrs, "y2", ref_h).unwrap_or(0.0);
            Some(SvgNode::Line {
                x1,
                y1,
                x2,
                y2,
                state,
            })
        }
        "polygon" => attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("points"))
            .map(|(_, pts)| SvgNode::Polygon {
                points: parse_points(pts),
                state,
            }),
        "polyline" => attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("points"))
            .map(|(_, pts)| SvgNode::Polyline {
                points: parse_points(pts),
                state,
            }),
        "use" => {
            let href = attrs
                .iter()
                .find(|(k, _)| {
                    k.eq_ignore_ascii_case("href") || k.eq_ignore_ascii_case("xlink:href")
                })
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let x = get_f32_attr_with_ref(attrs, "x", ref_w).unwrap_or(0.0);
            let y = get_f32_attr_with_ref(attrs, "y", ref_h).unwrap_or(0.0);
            let width = get_f32_attr_with_ref(attrs, "width", ref_w);
            let height = get_f32_attr_with_ref(attrs, "height", ref_h);
            let transform = attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("transform"))
                .and_then(|(_, v)| parse_transform(v));
            Some(SvgNode::Use {
                href,
                x,
                y,
                width,
                height,
                state,
                transform,
            })
        }
        _ => None,
    }
}

fn get_cur_ref_dims(
    container_stack: &[SvgContainer],
    root_view_box: Option<(f32, f32, f32, f32)>,
    root_width: Option<f32>,
    root_height: Option<f32>,
) -> (f32, f32) {
    for c in container_stack.iter().rev() {
        if let SvgContainer::Symbol {
            view_box: Some(vb), ..
        } = c
        {
            return (vb.2, vb.3);
        }
    }
    if let Some(vb) = root_view_box {
        (vb.2, vb.3)
    } else {
        (root_width.unwrap_or(300.0), root_height.unwrap_or(150.0))
    }
}

impl SvgDocument {
    pub fn parse(xml: &str) -> Option<Self> {
        let mut tokens = XmlTokenizer::new(xml);
        let mut width = None;
        let mut height = None;
        let mut view_box = None;
        let mut children = Vec::new();
        let mut symbols = HashMap::new();
        let mut container_stack: Vec<SvgContainer> = Vec::new();

        let mut base_state = SvgRenderState {
            fill: Some(SvgPaint::Color(Color::BLACK)),
            fill_rule: FillRule::Winding,
            fill_opacity: 1.0,
            stroke: None,
            stroke_width: 1.0,
            stroke_opacity: 1.0,
            stroke_linecap: LineCap::Butt,
            stroke_linejoin: LineJoin::Miter,
            opacity: 1.0,
        };

        while let Some(token) = tokens.next_token() {
            match token {
                XmlToken::StartTag(name, attrs) => {
                    let tag_name = name.to_ascii_lowercase();
                    if tag_name == "svg" {
                        for (k, v) in &attrs {
                            match k.to_ascii_lowercase().as_str() {
                                "width" => width = parse_length(v),
                                "height" => height = parse_length(v),
                                "viewbox" => view_box = parse_view_box(v),
                                _ => {}
                            }
                        }
                        base_state = apply_attributes_to_state(base_state, &attrs);
                    } else if tag_name == "g" {
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let state = apply_attributes_to_state(cur_state, &attrs);
                        let transform = attrs
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("transform"))
                            .and_then(|(_, v)| parse_transform(v));
                        let id = get_str_attr(&attrs, "id").map(|s| s.to_string());
                        container_stack.push(SvgContainer::Group {
                            children: Vec::new(),
                            state,
                            transform,
                            id,
                        });
                    } else if tag_name == "defs" {
                        container_stack.push(SvgContainer::Defs {
                            children: Vec::new(),
                        });
                    } else if tag_name == "symbol" {
                        let id = get_str_attr(&attrs, "id").unwrap_or("").to_string();
                        let vb = attrs
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("viewbox"))
                            .and_then(|(_, v)| parse_view_box(v));
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let state = apply_attributes_to_state(cur_state, &attrs);
                        container_stack.push(SvgContainer::Symbol {
                            id,
                            view_box: vb,
                            children: Vec::new(),
                            state,
                        });
                    } else {
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let (ref_w, ref_h) =
                            get_cur_ref_dims(&container_stack, view_box, width, height);
                        if let Some(node) =
                            parse_leaf_element(&tag_name, &attrs, &cur_state, ref_w, ref_h)
                        {
                            if let Some(id) = get_str_attr(&attrs, "id") {
                                let sym = SvgSymbol {
                                    view_box: None,
                                    children: vec![node.clone()],
                                    state: cur_state.clone(),
                                };
                                symbols.insert(id.to_string(), sym.clone());
                                register_global_svg_symbol(id.to_string(), sym);
                            }
                            add_node_to_container(&mut container_stack, &mut children, node);
                        }
                    }
                }
                XmlToken::EmptyTag(name, attrs) => {
                    let tag_name = name.to_ascii_lowercase();
                    if tag_name == "svg" {
                        for (k, v) in &attrs {
                            match k.to_ascii_lowercase().as_str() {
                                "width" => width = parse_length(v),
                                "height" => height = parse_length(v),
                                "viewbox" => view_box = parse_view_box(v),
                                _ => {}
                            }
                        }
                        base_state = apply_attributes_to_state(base_state, &attrs);
                    } else if tag_name == "g" || tag_name == "defs" {
                        // Empty container: nothing to push
                    } else if tag_name == "symbol" {
                        if let Some(id) = get_str_attr(&attrs, "id") {
                            let vb = attrs
                                .iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case("viewbox"))
                                .and_then(|(_, v)| parse_view_box(v));
                            let cur_state = get_cur_state(&container_stack, &base_state);
                            let state = apply_attributes_to_state(cur_state, &attrs);
                            let sym = SvgSymbol {
                                view_box: vb,
                                children: Vec::new(),
                                state,
                            };
                            symbols.insert(id.to_string(), sym.clone());
                            register_global_svg_symbol(id.to_string(), sym);
                        }
                    } else {
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let (ref_w, ref_h) =
                            get_cur_ref_dims(&container_stack, view_box, width, height);
                        if let Some(node) =
                            parse_leaf_element(&tag_name, &attrs, &cur_state, ref_w, ref_h)
                        {
                            if let Some(id) = get_str_attr(&attrs, "id") {
                                let sym = SvgSymbol {
                                    view_box: None,
                                    children: vec![node.clone()],
                                    state: cur_state.clone(),
                                };
                                symbols.insert(id.to_string(), sym.clone());
                                register_global_svg_symbol(id.to_string(), sym);
                            }
                            add_node_to_container(&mut container_stack, &mut children, node);
                        }
                    }
                }
                XmlToken::EndTag(name) => {
                    let tag_name = name.to_ascii_lowercase();
                    if tag_name == "g" {
                        if let Some(SvgContainer::Group {
                            children: grp_children,
                            state,
                            transform,
                            id,
                        }) = container_stack.pop()
                        {
                            let grp_node = SvgNode::Group {
                                children: grp_children,
                                state: state.clone(),
                                transform,
                            };
                            if let Some(id_str) = id {
                                let sym = SvgSymbol {
                                    view_box: None,
                                    children: vec![grp_node.clone()],
                                    state,
                                };
                                symbols.insert(id_str.clone(), sym.clone());
                                register_global_svg_symbol(id_str, sym);
                            }
                            add_node_to_container(&mut container_stack, &mut children, grp_node);
                        }
                    } else if tag_name == "defs" {
                        container_stack.pop();
                    } else if tag_name == "symbol"
                        && let Some(SvgContainer::Symbol {
                            id,
                            view_box,
                            children: sym_children,
                            state,
                        }) = container_stack.pop()
                    {
                        let sym = SvgSymbol {
                            view_box,
                            children: sym_children,
                            state,
                        };
                        symbols.insert(id.clone(), sym.clone());
                        register_global_svg_symbol(id, sym);
                    }
                }
            }
        }

        Some(SvgDocument {
            width,
            height,
            view_box,
            children,
            symbols,
            root_state: base_state,
        })
    }
}
