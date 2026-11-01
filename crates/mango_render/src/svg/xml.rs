//! Lightweight XML tokenizer and SVG attribute parser.

use mango_core::Color;
use std::collections::HashMap;
use tiny_skia::{FillRule, LineCap, LineJoin, Transform};

use super::ast::{
    SvgClipPath, SvgContainer, SvgDocument, SvgGradient, SvgNode, SvgPaint, SvgRenderState,
    SvgSpreadMethod, SvgStop, SvgSymbol, SvgTextAnchor, SvgTspan, add_node_to_container,
    get_cur_state,
};
use super::cache::register_global_svg_symbol;
use super::xml_attrs::{
    apply_attributes_to_state, get_f32_attr_with_ref, get_str_attr, parse_attributes,
    parse_color_str, parse_length, parse_preserve_aspect_ratio, parse_spread_method,
    parse_transform, parse_view_box,
};
use super::xml_leaf::parse_leaf_element;

#[allow(clippy::enum_variant_names)]
pub(crate) enum XmlToken {
    StartTag(String, Vec<(String, String)>),
    EmptyTag(String, Vec<(String, String)>),
    EndTag(String),
    Text(String),
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
            // Check for text content before '<'
            if bytes[self.pos] != b'<' {
                let start = self.pos;
                while self.pos < bytes.len() && bytes[self.pos] != b'<' {
                    self.pos += 1;
                }
                let text_slice = self.content[start..self.pos].trim();
                if !text_slice.is_empty() {
                    return Some(XmlToken::Text(text_slice.to_string()));
                }
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

enum PendingGradient {
    Linear {
        id: String,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: Vec<SvgStop>,
        transform: Option<Transform>,
        spread_method: SvgSpreadMethod,
        user_space_on_use: bool,
    },
    Radial {
        id: String,
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
        stops: Vec<SvgStop>,
        transform: Option<Transform>,
        spread_method: SvgSpreadMethod,
        user_space_on_use: bool,
    },
}

impl SvgDocument {
    pub fn parse(xml: &str) -> Option<Self> {
        let mut tokens = XmlTokenizer::new(xml);
        let mut width = None;
        let mut height = None;
        let mut view_box = None;
        let mut preserve_aspect_ratio = None;
        let mut children = Vec::new();
        let mut symbols = HashMap::new();
        let mut gradients = HashMap::new();
        let mut clip_paths = HashMap::new();
        let mut container_stack: Vec<SvgContainer> = Vec::new();
        let mut cur_gradient: Option<PendingGradient> = None;
        let mut cur_tspan: Option<SvgTspan> = None;

        let mut base_state = SvgRenderState {
            fill: Some(SvgPaint::Color(Color::BLACK)),
            fill_rule: FillRule::Winding,
            fill_opacity: 1.0,
            stroke: None,
            stroke_width: 1.0,
            stroke_opacity: 1.0,
            stroke_linecap: LineCap::Butt,
            stroke_linejoin: LineJoin::Miter,
            stroke_dasharray: None,
            stroke_dashoffset: 0.0,
            clip_path: None,
            opacity: 1.0,
            font_size: 16.0,
            font_family: "sans-serif".to_string(),
            font_weight: "normal".to_string(),
            font_style: "normal".to_string(),
            text_anchor: SvgTextAnchor::Start,
        };

        while let Some(token) = tokens.next_token() {
            match token {
                XmlToken::Text(text_str) => {
                    if let Some(SvgContainer::Text { text, tspans, .. }) =
                        container_stack.last_mut()
                    {
                        if let Some(ref mut tspan) = cur_tspan {
                            tspan.text.push_str(&text_str);
                        } else if !tspans.is_empty() {
                            if let Some(last_span) = tspans.last_mut() {
                                last_span.text.push_str(&text_str);
                            }
                        } else {
                            text.push_str(&text_str);
                        }
                    }
                }
                XmlToken::StartTag(name, attrs) => {
                    let tag_name = name.to_ascii_lowercase();
                    if tag_name == "svg" {
                        for (k, v) in &attrs {
                            match k.to_ascii_lowercase().as_str() {
                                "width" => width = parse_length(v),
                                "height" => height = parse_length(v),
                                "viewbox" => view_box = parse_view_box(v),
                                "preserveaspectratio" => {
                                    preserve_aspect_ratio = parse_preserve_aspect_ratio(v)
                                }
                                _ => {}
                            }
                        }
                        base_state = apply_attributes_to_state(base_state, &attrs);
                    } else if tag_name == "lineargradient" {
                        let id = get_str_attr(&attrs, "id").unwrap_or("").to_string();
                        let x1 = parse_gradient_coord(get_str_attr(&attrs, "x1").unwrap_or("0%"));
                        let y1 = parse_gradient_coord(get_str_attr(&attrs, "y1").unwrap_or("0%"));
                        let x2 = parse_gradient_coord(get_str_attr(&attrs, "x2").unwrap_or("100%"));
                        let y2 = parse_gradient_coord(get_str_attr(&attrs, "y2").unwrap_or("0%"));
                        let transform =
                            get_str_attr(&attrs, "gradientTransform").and_then(parse_transform);
                        let spread_method = get_str_attr(&attrs, "spreadMethod")
                            .map(parse_spread_method)
                            .unwrap_or(SvgSpreadMethod::Pad);
                        let user_space = get_str_attr(&attrs, "gradientUnits")
                            .map(|u| u.eq_ignore_ascii_case("userSpaceOnUse"))
                            .unwrap_or(false);

                        cur_gradient = Some(PendingGradient::Linear {
                            id,
                            x1,
                            y1,
                            x2,
                            y2,
                            stops: Vec::new(),
                            transform,
                            spread_method,
                            user_space_on_use: user_space,
                        });
                    } else if tag_name == "radialgradient" {
                        let id = get_str_attr(&attrs, "id").unwrap_or("").to_string();
                        let cx = parse_gradient_coord(get_str_attr(&attrs, "cx").unwrap_or("50%"));
                        let cy = parse_gradient_coord(get_str_attr(&attrs, "cy").unwrap_or("50%"));
                        let r = parse_gradient_coord(get_str_attr(&attrs, "r").unwrap_or("50%"));
                        let fx = get_str_attr(&attrs, "fx")
                            .map(parse_gradient_coord)
                            .unwrap_or(cx);
                        let fy = get_str_attr(&attrs, "fy")
                            .map(parse_gradient_coord)
                            .unwrap_or(cy);
                        let transform =
                            get_str_attr(&attrs, "gradientTransform").and_then(parse_transform);
                        let spread_method = get_str_attr(&attrs, "spreadMethod")
                            .map(parse_spread_method)
                            .unwrap_or(SvgSpreadMethod::Pad);
                        let user_space = get_str_attr(&attrs, "gradientUnits")
                            .map(|u| u.eq_ignore_ascii_case("userSpaceOnUse"))
                            .unwrap_or(false);

                        cur_gradient = Some(PendingGradient::Radial {
                            id,
                            cx,
                            cy,
                            r,
                            fx,
                            fy,
                            stops: Vec::new(),
                            transform,
                            spread_method,
                            user_space_on_use: user_space,
                        });
                    } else if tag_name == "stop" {
                        if let Some(ref mut grad) = cur_gradient {
                            let stop = parse_gradient_stop(&attrs);
                            match grad {
                                PendingGradient::Linear { stops, .. } => stops.push(stop),
                                PendingGradient::Radial { stops, .. } => stops.push(stop),
                            }
                        }
                    } else if tag_name == "clippath" {
                        let id = get_str_attr(&attrs, "id").unwrap_or("").to_string();
                        let transform = attrs
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("transform"))
                            .and_then(|(_, v)| parse_transform(v));
                        container_stack.push(SvgContainer::ClipPath {
                            id,
                            children: Vec::new(),
                            transform,
                        });
                    } else if tag_name == "text" {
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let state = apply_attributes_to_state(cur_state, &attrs);
                        let (ref_w, ref_h) =
                            get_cur_ref_dims(&container_stack, view_box, width, height);
                        let x = get_f32_attr_with_ref(&attrs, "x", ref_w).unwrap_or(0.0);
                        let y = get_f32_attr_with_ref(&attrs, "y", ref_h).unwrap_or(0.0);
                        let dx = get_f32_attr_with_ref(&attrs, "dx", ref_w).unwrap_or(0.0);
                        let dy = get_f32_attr_with_ref(&attrs, "dy", ref_h).unwrap_or(0.0);
                        let transform = attrs
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("transform"))
                            .and_then(|(_, v)| parse_transform(v));

                        container_stack.push(SvgContainer::Text {
                            x,
                            y,
                            dx,
                            dy,
                            text: String::new(),
                            font_size: state.font_size,
                            font_family: state.font_family.clone(),
                            font_weight: state.font_weight.clone(),
                            font_style: state.font_style.clone(),
                            text_anchor: state.text_anchor,
                            tspans: Vec::new(),
                            state,
                            transform,
                        });
                    } else if tag_name == "tspan" {
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let state = apply_attributes_to_state(cur_state, &attrs);
                        let (ref_w, ref_h) =
                            get_cur_ref_dims(&container_stack, view_box, width, height);
                        let x = get_f32_attr_with_ref(&attrs, "x", ref_w);
                        let y = get_f32_attr_with_ref(&attrs, "y", ref_h);
                        let dx = get_f32_attr_with_ref(&attrs, "dx", ref_w);
                        let dy = get_f32_attr_with_ref(&attrs, "dy", ref_h);

                        cur_tspan = Some(SvgTspan {
                            x,
                            y,
                            dx,
                            dy,
                            text: String::new(),
                            state,
                        });
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
                        let par = attrs
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("preserveaspectratio"))
                            .and_then(|(_, v)| parse_preserve_aspect_ratio(v));
                        let cur_state = get_cur_state(&container_stack, &base_state);
                        let state = apply_attributes_to_state(cur_state, &attrs);
                        container_stack.push(SvgContainer::Symbol {
                            id,
                            view_box: vb,
                            children: Vec::new(),
                            state,
                            preserve_aspect_ratio: par,
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
                                    preserve_aspect_ratio: None,
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
                    if tag_name == "stop" {
                        if let Some(ref mut grad) = cur_gradient {
                            let stop = parse_gradient_stop(&attrs);
                            match grad {
                                PendingGradient::Linear { stops, .. } => stops.push(stop),
                                PendingGradient::Radial { stops, .. } => stops.push(stop),
                            }
                        }
                    } else if tag_name == "svg" {
                        for (k, v) in &attrs {
                            match k.to_ascii_lowercase().as_str() {
                                "width" => width = parse_length(v),
                                "height" => height = parse_length(v),
                                "viewbox" => view_box = parse_view_box(v),
                                "preserveaspectratio" => {
                                    preserve_aspect_ratio = parse_preserve_aspect_ratio(v)
                                }
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
                            let par = attrs
                                .iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case("preserveaspectratio"))
                                .and_then(|(_, v)| parse_preserve_aspect_ratio(v));
                            let cur_state = get_cur_state(&container_stack, &base_state);
                            let state = apply_attributes_to_state(cur_state, &attrs);
                            let sym = SvgSymbol {
                                view_box: vb,
                                children: Vec::new(),
                                state,
                                preserve_aspect_ratio: par,
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
                                    preserve_aspect_ratio: None,
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
                    if tag_name == "lineargradient" || tag_name == "radialgradient" {
                        if let Some(pending) = cur_gradient.take() {
                            match pending {
                                PendingGradient::Linear {
                                    id,
                                    x1,
                                    y1,
                                    x2,
                                    y2,
                                    stops,
                                    transform,
                                    spread_method,
                                    user_space_on_use,
                                } => {
                                    if !id.is_empty() {
                                        gradients.insert(
                                            id.clone(),
                                            SvgGradient::Linear {
                                                id,
                                                x1,
                                                y1,
                                                x2,
                                                y2,
                                                stops,
                                                transform,
                                                spread_method,
                                                user_space_on_use,
                                            },
                                        );
                                    }
                                }
                                PendingGradient::Radial {
                                    id,
                                    cx,
                                    cy,
                                    r,
                                    fx,
                                    fy,
                                    stops,
                                    transform,
                                    spread_method,
                                    user_space_on_use,
                                } => {
                                    if !id.is_empty() {
                                        gradients.insert(
                                            id.clone(),
                                            SvgGradient::Radial {
                                                id,
                                                cx,
                                                cy,
                                                r,
                                                fx,
                                                fy,
                                                stops,
                                                transform,
                                                spread_method,
                                                user_space_on_use,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    } else if tag_name == "clippath" {
                        if let Some(SvgContainer::ClipPath {
                            id,
                            children: cp_children,
                            transform,
                        }) = container_stack.pop()
                        {
                            if !id.is_empty() {
                                clip_paths.insert(
                                    id.clone(),
                                    SvgClipPath {
                                        id,
                                        children: cp_children,
                                        transform,
                                    },
                                );
                            }
                        }
                    } else if tag_name == "tspan" {
                        if let Some(span) = cur_tspan.take()
                            && let Some(SvgContainer::Text { tspans, .. }) =
                                container_stack.last_mut()
                        {
                            tspans.push(span);
                        }
                    } else if tag_name == "text" {
                        if let Some(SvgContainer::Text {
                            x,
                            y,
                            dx,
                            dy,
                            text,
                            font_size,
                            font_family,
                            font_weight,
                            font_style,
                            text_anchor,
                            tspans,
                            state,
                            transform,
                        }) = container_stack.pop()
                        {
                            let text_node = SvgNode::Text {
                                x,
                                y,
                                dx,
                                dy,
                                text,
                                font_size,
                                font_family,
                                font_weight,
                                font_style,
                                text_anchor,
                                tspans,
                                state,
                                transform,
                            };
                            add_node_to_container(&mut container_stack, &mut children, text_node);
                        }
                    } else if tag_name == "g" {
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
                                    preserve_aspect_ratio: None,
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
                            preserve_aspect_ratio: par,
                        }) = container_stack.pop()
                    {
                        let sym = SvgSymbol {
                            view_box,
                            children: sym_children,
                            state,
                            preserve_aspect_ratio: par,
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
            preserve_aspect_ratio,
            children,
            symbols,
            gradients,
            clip_paths,
            root_state: base_state,
        })
    }
}

fn parse_gradient_coord(val: &str) -> f32 {
    let s = val.trim();
    if let Some(pct) = s.strip_suffix('%') {
        pct.trim().parse::<f32>().unwrap_or(0.0) / 100.0
    } else {
        s.parse::<f32>().unwrap_or(0.0)
    }
}

fn parse_gradient_stop(attrs: &[(String, String)]) -> SvgStop {
    let mut offset = 0.0f32;
    let mut color = Color::BLACK;
    let mut opacity = 1.0f32;

    for (k, v) in attrs {
        match k.to_ascii_lowercase().as_str() {
            "offset" => {
                offset = parse_gradient_coord(v).clamp(0.0, 1.0);
            }
            "stop-color" => {
                if let Some(c) = parse_color_str(v) {
                    color = c;
                }
            }
            "stop-opacity" => {
                if let Ok(op) = v.trim().parse::<f32>() {
                    opacity = op.clamp(0.0, 1.0);
                }
            }
            "style" => {
                for declaration in v.split(';') {
                    if let Some((prop, val)) = declaration.split_once(':') {
                        match prop.trim().to_ascii_lowercase().as_str() {
                            "stop-color" => {
                                if let Some(c) = parse_color_str(val.trim()) {
                                    color = c;
                                }
                            }
                            "stop-opacity" => {
                                if let Ok(op) = val.trim().parse::<f32>() {
                                    opacity = op.clamp(0.0, 1.0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    SvgStop {
        offset,
        color,
        opacity,
    }
}
