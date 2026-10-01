//! Pure-Rust SVG parser and vector rasterizer using `tiny-skia`.
//!
//! Provides crisp, anti-aliased rendering of SVG icons, shapes, and paths
//! with support for `viewBox`, `fill`, `stroke`, `currentColor`, opacity,
//! and standard SVG element hierarchies.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{OnceLock, RwLock};

use mango_core::Color;
use tiny_skia::{
    Color as SkiaColor, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke,
    Transform,
};

use crate::image_decode::DecodedImage;

static SVG_DOC_CACHE: OnceLock<RwLock<HashMap<String, std::sync::Arc<SvgDocument>>>> =
    OnceLock::new();
static SVG_RENDER_CACHE: OnceLock<RwLock<HashMap<(String, u32, u32, u32), DecodedImage>>> =
    OnceLock::new();

fn svg_doc_cache() -> &'static RwLock<HashMap<String, std::sync::Arc<SvgDocument>>> {
    SVG_DOC_CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(256)))
}

fn svg_render_cache() -> &'static RwLock<HashMap<(String, u32, u32, u32), DecodedImage>> {
    SVG_RENDER_CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(256)))
}

/// Render an SVG XML string into a [`DecodedImage`] of `(target_w, target_h)` pixels.
/// Caches both parsed SVG DOMs and rendered pixmaps to avoid re-parsing on every paint (OPT-008).
///
/// When `currentColor` is used in SVG attributes (`fill="currentColor"` or `stroke="currentColor"`),
/// it resolves to the provided `current_color`.
pub fn render_svg(
    svg_xml: &str,
    target_w: u32,
    target_h: u32,
    current_color: Color,
) -> Option<DecodedImage> {
    if target_w == 0 || target_h == 0 {
        return None;
    }

    let color_key = current_color.to_argb_u32();
    let cache_key = (svg_xml.to_string(), target_w, target_h, color_key);

    // 1. Check rendered pixmap cache (OPT-008)
    if let Ok(cache) = svg_render_cache().read()
        && let Some(img) = cache.get(&cache_key)
    {
        return Some(img.clone());
    }

    // 2. Check parsed SVG DOM cache (OPT-008)
    let doc: std::sync::Arc<SvgDocument> = {
        let parsed_opt = if let Ok(cache) = svg_doc_cache().read() {
            cache.get(svg_xml).cloned()
        } else {
            None
        };

        if let Some(d) = parsed_opt {
            d
        } else {
            let parsed = SvgDocument::parse(svg_xml)?;
            let arc_doc = std::sync::Arc::new(parsed);
            if let Ok(mut cache) = svg_doc_cache().write() {
                if cache.len() >= 512 {
                    let keys_to_remove: Vec<_> = cache.keys().take(256).cloned().collect();
                    for k in keys_to_remove {
                        cache.remove(&k);
                    }
                }
                cache.insert(svg_xml.to_string(), arc_doc.clone());
            }
            arc_doc
        }
    };

    let mut pixmap = Pixmap::new(target_w, target_h)?;
    pixmap.fill(SkiaColor::TRANSPARENT);

    // Determine coordinate transformation
    let (vb_x, vb_y, vb_w, vb_h) = if let Some(vb) = doc.view_box {
        vb
    } else {
        let w = doc.width.unwrap_or(target_w as f32);
        let h = doc.height.unwrap_or(target_h as f32);
        (0.0, 0.0, w, h)
    };

    if vb_w <= 0.0 || vb_h <= 0.0 {
        return None;
    }

    // Uniform fit (meet) preserving aspect ratio, centered in viewport
    let scale_x = target_w as f32 / vb_w;
    let scale_y = target_h as f32 / vb_h;
    let scale = scale_x.min(scale_y);

    let offset_x = (target_w as f32 - vb_w * scale) / 2.0;
    let offset_y = (target_h as f32 - vb_h * scale) / 2.0;

    let transform = Transform::from_translate(offset_x, offset_y)
        .pre_scale(scale, scale)
        .pre_translate(-vb_x, -vb_y);

    let initial_state = SvgRenderState {
        fill: Some(SvgPaint::Color(Color::BLACK)), // SVG default fill is black
        fill_rule: FillRule::Winding,
        fill_opacity: 1.0,
        stroke: None,
        stroke_width: 1.0,
        stroke_opacity: 1.0,
        stroke_linecap: LineCap::Butt,
        stroke_linejoin: LineJoin::Miter,
        opacity: 1.0,
    };

    doc.render(&mut pixmap, transform, &initial_state, current_color);

    // Convert pixmap to DecodedImage
    let mut pixels = Vec::with_capacity((target_w * target_h) as usize);
    for pixel in pixmap.pixels() {
        let a = pixel.alpha();
        let r = pixel.red();
        let g = pixel.green();
        let b = pixel.blue();
        pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
    }

    let decoded = DecodedImage {
        width: target_w,
        height: target_h,
        pixels,
    };

    // Store in render cache
    if let Ok(mut cache) = svg_render_cache().write() {
        if cache.len() >= 512 {
            let keys_to_remove: Vec<_> = cache.keys().take(256).cloned().collect();
            for k in keys_to_remove {
                cache.remove(&k);
            }
        }
        cache.insert(cache_key, decoded.clone());
    }

    Some(decoded)
}

/// Extracts intrinsic dimensions (width, height) from an SVG document if specified.
static GLOBAL_SVG_SYMBOLS: OnceLock<RwLock<HashMap<String, SvgSymbol>>> = OnceLock::new();

fn global_svg_symbols() -> &'static RwLock<HashMap<String, SvgSymbol>> {
    GLOBAL_SVG_SYMBOLS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Registers an SVG symbol in the global cross-SVG symbol registry.
pub fn register_global_svg_symbol(id: String, symbol: SvgSymbol) {
    if !id.is_empty()
        && let Ok(mut lock) = global_svg_symbols().write()
    {
        lock.insert(id, symbol);
    }
}

/// Looks up an SVG symbol in the global cross-SVG symbol registry by ID.
pub fn get_global_svg_symbol(id: &str) -> Option<SvgSymbol> {
    let clean = id.trim_start_matches('#');
    let lock = global_svg_symbols().read().ok()?;
    lock.get(clean).cloned()
}

/// Clears all globally registered SVG symbols.
pub fn clear_global_svg_symbols() {
    if let Ok(mut lock) = global_svg_symbols().write() {
        lock.clear();
    }
}

/// Extracts intrinsic dimensions (width, height) from an SVG document if specified.
pub fn get_svg_intrinsic_dimensions(svg_xml: &str) -> (Option<f32>, Option<f32>) {
    let doc: std::sync::Arc<SvgDocument> = {
        let cached = if let Ok(cache) = svg_doc_cache().read() {
            cache.get(svg_xml).cloned()
        } else {
            None
        };
        if let Some(d) = cached {
            d
        } else if let Some(parsed) = SvgDocument::parse(svg_xml) {
            let arc_doc = std::sync::Arc::new(parsed);
            if let Ok(mut cache) = svg_doc_cache().write() {
                if cache.len() >= 512 {
                    cache.clear();
                }
                cache.insert(svg_xml.to_string(), arc_doc.clone());
            }
            arc_doc
        } else {
            return (None, None);
        }
    };

    match (doc.width, doc.height, doc.view_box) {
        (Some(w), Some(h), _) => return (Some(w), Some(h)),
        (Some(w), None, Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
            return (Some(w), Some(w / (vb_w / vb_h)));
        }
        (None, Some(h), Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
            return (Some(h * (vb_w / vb_h)), Some(h));
        }
        (Some(w), None, _) => return (Some(w), None),
        (None, Some(h), _) => return (None, Some(h)),
        (None, None, Some((_, _, vb_w, vb_h))) => return (Some(vb_w), Some(vb_h)),
        (None, None, None) => {}
    }
    for child in &doc.children {
        if let SvgNode::Use {
            href,
            width,
            height,
            ..
        } = child
        {
            let sym_id = href.split('#').next_back().unwrap_or(href).trim();
            let sym = doc
                .symbols
                .get(sym_id)
                .cloned()
                .or_else(|| get_global_svg_symbol(sym_id));
            let sym_vb = sym.and_then(|s| s.view_box);
            match (*width, *height, sym_vb) {
                (Some(w), Some(h), _) => return (Some(w), Some(h)),
                (Some(w), None, Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
                    return (Some(w), Some(w / (vb_w / vb_h)));
                }
                (None, Some(h), Some((_, _, vb_w, vb_h))) if vb_h > 0.0 => {
                    return (Some(h * (vb_w / vb_h)), Some(h));
                }
                (Some(w), None, _) => return (Some(w), None),
                (None, Some(h), _) => return (None, Some(h)),
                (None, None, Some((_, _, vb_w, vb_h))) => return (Some(vb_w), Some(vb_h)),
                _ => {}
            }
        }
    }
    (None, None)
}

// ---------------------------------------------------------------------------
// SVG Parsing and Document Tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum SvgPaint {
    None,
    Color(Color),
    CurrentColor,
}

#[derive(Debug, Clone)]
pub struct SvgRenderState {
    fill: Option<SvgPaint>,
    fill_rule: FillRule,
    fill_opacity: f32,
    stroke: Option<SvgPaint>,
    stroke_width: f32,
    stroke_opacity: f32,
    stroke_linecap: LineCap,
    stroke_linejoin: LineJoin,
    opacity: f32,
}

/// A parsed SVG `<symbol>` template or reusable graphic definition.
#[derive(Debug, Clone)]
pub struct SvgSymbol {
    pub view_box: Option<(f32, f32, f32, f32)>,
    pub children: Vec<SvgNode>,
    pub state: SvgRenderState,
}

#[derive(Debug, Clone)]
pub enum SvgNode {
    Path {
        path: Path,
        state: SvgRenderState,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rx: f32,
        ry: f32,
        state: SvgRenderState,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        state: SvgRenderState,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        state: SvgRenderState,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        state: SvgRenderState,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        state: SvgRenderState,
    },
    Polyline {
        points: Vec<(f32, f32)>,
        state: SvgRenderState,
    },
    Group {
        children: Vec<SvgNode>,
        state: SvgRenderState,
        transform: Option<Transform>,
    },
    Use {
        href: String,
        x: f32,
        y: f32,
        width: Option<f32>,
        height: Option<f32>,
        state: SvgRenderState,
        transform: Option<Transform>,
    },
}

#[derive(Debug)]
pub struct SvgDocument {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub view_box: Option<(f32, f32, f32, f32)>,
    pub children: Vec<SvgNode>,
    pub symbols: HashMap<String, SvgSymbol>,
}

#[derive(Debug)]
enum SvgContainer {
    Group {
        children: Vec<SvgNode>,
        state: SvgRenderState,
        transform: Option<Transform>,
        id: Option<String>,
    },
    Defs {
        children: Vec<SvgNode>,
    },
    Symbol {
        id: String,
        view_box: Option<(f32, f32, f32, f32)>,
        children: Vec<SvgNode>,
        state: SvgRenderState,
    },
}

fn get_cur_state(container_stack: &[SvgContainer], base_state: &SvgRenderState) -> SvgRenderState {
    container_stack
        .last()
        .map(|c| match c {
            SvgContainer::Group { state, .. } => state.clone(),
            SvgContainer::Symbol { state, .. } => state.clone(),
            SvgContainer::Defs { .. } => SvgRenderState {
                fill: None,
                stroke: None,
                ..base_state.clone()
            },
        })
        .unwrap_or_else(|| base_state.clone())
}

fn add_node_to_container(
    container_stack: &mut [SvgContainer],
    root_children: &mut Vec<SvgNode>,
    node: SvgNode,
) {
    if let Some(top) = container_stack.last_mut() {
        match top {
            SvgContainer::Group { children, .. } => children.push(node),
            SvgContainer::Defs { children } => children.push(node),
            SvgContainer::Symbol { children, .. } => children.push(node),
        }
    } else {
        root_children.push(node);
    }
}

fn parse_leaf_element(
    name: &str,
    attrs: &[(String, String)],
    cur_state: &SvgRenderState,
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
            let x = get_f32_attr(attrs, "x").unwrap_or(0.0);
            let y = get_f32_attr(attrs, "y").unwrap_or(0.0);
            let w = get_f32_attr(attrs, "width").unwrap_or(0.0);
            let h = get_f32_attr(attrs, "height").unwrap_or(0.0);
            let rx = get_f32_attr(attrs, "rx").unwrap_or(0.0);
            let ry = get_f32_attr(attrs, "ry").unwrap_or(rx);
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
            let cx = get_f32_attr(attrs, "cx").unwrap_or(0.0);
            let cy = get_f32_attr(attrs, "cy").unwrap_or(0.0);
            let r = get_f32_attr(attrs, "r").unwrap_or(0.0);
            if r > 0.0 {
                Some(SvgNode::Circle { cx, cy, r, state })
            } else {
                None
            }
        }
        "ellipse" => {
            let cx = get_f32_attr(attrs, "cx").unwrap_or(0.0);
            let cy = get_f32_attr(attrs, "cy").unwrap_or(0.0);
            let rx = get_f32_attr(attrs, "rx").unwrap_or(0.0);
            let ry = get_f32_attr(attrs, "ry").unwrap_or(0.0);
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
            let x1 = get_f32_attr(attrs, "x1").unwrap_or(0.0);
            let y1 = get_f32_attr(attrs, "y1").unwrap_or(0.0);
            let x2 = get_f32_attr(attrs, "x2").unwrap_or(0.0);
            let y2 = get_f32_attr(attrs, "y2").unwrap_or(0.0);
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
            let x = get_f32_attr(attrs, "x").unwrap_or(0.0);
            let y = get_f32_attr(attrs, "y").unwrap_or(0.0);
            let width = get_f32_attr(attrs, "width");
            let height = get_f32_attr(attrs, "height");
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

impl SvgDocument {
    pub fn parse(xml: &str) -> Option<Self> {
        let mut tokens = XmlTokenizer::new(xml);
        let mut width = None;
        let mut height = None;
        let mut view_box = None;
        let mut children = Vec::new();
        let mut symbols = HashMap::new();
        let mut container_stack: Vec<SvgContainer> = Vec::new();

        let base_state = SvgRenderState {
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
                        if let Some(node) = parse_leaf_element(&tag_name, &attrs, &cur_state) {
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
                        if let Some(node) = parse_leaf_element(&tag_name, &attrs, &cur_state) {
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
        })
    }

    fn render(
        &self,
        pixmap: &mut Pixmap,
        transform: Transform,
        parent_state: &SvgRenderState,
        current_color: Color,
    ) {
        for child in &self.children {
            child.render(
                pixmap,
                transform,
                parent_state,
                current_color,
                &self.symbols,
                0,
            );
        }
    }
}

fn compose_state(node_state: &SvgRenderState, parent_state: &SvgRenderState) -> SvgRenderState {
    SvgRenderState {
        fill: node_state
            .fill
            .clone()
            .or_else(|| parent_state.fill.clone()),
        fill_rule: node_state.fill_rule,
        fill_opacity: node_state.fill_opacity * parent_state.fill_opacity,
        stroke: node_state
            .stroke
            .clone()
            .or_else(|| parent_state.stroke.clone()),
        stroke_width: if node_state.stroke.is_some() {
            node_state.stroke_width
        } else {
            parent_state.stroke_width
        },
        stroke_opacity: node_state.stroke_opacity * parent_state.stroke_opacity,
        stroke_linecap: node_state.stroke_linecap,
        stroke_linejoin: node_state.stroke_linejoin,
        opacity: node_state.opacity * parent_state.opacity,
    }
}

impl SvgNode {
    fn render(
        &self,
        pixmap: &mut Pixmap,
        transform: Transform,
        parent_state: &SvgRenderState,
        current_color: Color,
        symbols: &HashMap<String, SvgSymbol>,
        depth: usize,
    ) {
        match self {
            SvgNode::Path { path, state } => {
                let effective = compose_state(state, parent_state);
                draw_path_with_state(pixmap, path, &effective, transform, current_color);
            }
            SvgNode::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
                state,
            } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                if *rx > 0.0 || *ry > 0.0 {
                    let r = (*rx).max(*ry).min(*width / 2.0).min(*height / 2.0);
                    pb.move_to(x + r, *y);
                    pb.line_to(x + width - r, *y);
                    pb.quad_to(x + width, *y, x + width, y + r);
                    pb.line_to(x + width, y + height - r);
                    pb.quad_to(x + width, y + height, x + width - r, y + height);
                    pb.line_to(x + r, y + height);
                    pb.quad_to(*x, y + height, *x, y + height - r);
                    pb.line_to(*x, y + r);
                    pb.quad_to(*x, *y, x + r, *y);
                    pb.close();
                } else {
                    pb.move_to(*x, *y);
                    pb.line_to(x + width, *y);
                    pb.line_to(x + width, y + height);
                    pb.line_to(*x, y + height);
                    pb.close();
                }
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Circle { cx, cy, r, state } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                add_ellipse_to_path(&mut pb, *cx, *cy, *r, *r);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Ellipse {
                cx,
                cy,
                rx,
                ry,
                state,
            } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                add_ellipse_to_path(&mut pb, *cx, *cy, *rx, *ry);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Line {
                x1,
                y1,
                x2,
                y2,
                state,
            } => {
                let effective = compose_state(state, parent_state);
                let mut pb = PathBuilder::new();
                pb.move_to(*x1, *y1);
                pb.line_to(*x2, *y2);
                if let Some(path) = pb.finish() {
                    draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                }
            }
            SvgNode::Polygon { points, state } => {
                let effective = compose_state(state, parent_state);
                if points.len() >= 2 {
                    let mut pb = PathBuilder::new();
                    pb.move_to(points[0].0, points[0].1);
                    for pt in &points[1..] {
                        pb.line_to(pt.0, pt.1);
                    }
                    pb.close();
                    if let Some(path) = pb.finish() {
                        draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                    }
                }
            }
            SvgNode::Polyline { points, state } => {
                let effective = compose_state(state, parent_state);
                if points.len() >= 2 {
                    let mut pb = PathBuilder::new();
                    pb.move_to(points[0].0, points[0].1);
                    for pt in &points[1..] {
                        pb.line_to(pt.0, pt.1);
                    }
                    if let Some(path) = pb.finish() {
                        draw_path_with_state(pixmap, &path, &effective, transform, current_color);
                    }
                }
            }
            SvgNode::Group {
                children,
                state,
                transform: grp_transform,
            } => {
                let effective = compose_state(state, parent_state);
                let mut cur_tf = transform;
                if let Some(gt) = grp_transform {
                    cur_tf = cur_tf.pre_concat(*gt);
                }
                for child in children {
                    child.render(pixmap, cur_tf, &effective, current_color, symbols, depth);
                }
            }
            SvgNode::Use {
                href,
                x,
                y,
                width,
                height,
                state,
                transform: use_transform,
            } => {
                if depth > 16 {
                    return;
                }
                let sym_id = href.split('#').next_back().unwrap_or(href).trim();
                if sym_id.is_empty() {
                    return;
                }
                let symbol = symbols
                    .get(sym_id)
                    .cloned()
                    .or_else(|| get_global_svg_symbol(sym_id));

                if let Some(sym) = symbol {
                    let mut cur_tf = transform;
                    if let Some(ut) = use_transform {
                        cur_tf = cur_tf.pre_concat(*ut);
                    }

                    if let Some((vb_x, vb_y, vb_w, vb_h)) = sym.view_box {
                        if vb_w > 0.0 && vb_h > 0.0 {
                            let target_w = width.unwrap_or(vb_w);
                            let target_h = height.unwrap_or(vb_h);
                            let scale_x = target_w / vb_w;
                            let scale_y = target_h / vb_h;
                            let scale = scale_x.min(scale_y);
                            let offset_x = *x + (target_w - vb_w * scale) / 2.0;
                            let offset_y = *y + (target_h - vb_h * scale) / 2.0;
                            cur_tf = cur_tf
                                .pre_translate(offset_x, offset_y)
                                .pre_scale(scale, scale)
                                .pre_translate(-vb_x, -vb_y);
                        }
                    } else {
                        cur_tf = cur_tf.pre_translate(*x, *y);
                    }

                    let effective_use_state = compose_state(state, parent_state);
                    for child in &sym.children {
                        child.render(
                            pixmap,
                            cur_tf,
                            &effective_use_state,
                            current_color,
                            symbols,
                            depth + 1,
                        );
                    }
                }
            }
        }
    }
}

fn add_ellipse_to_path(pb: &mut PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32) {
    let k = 0.552_284_8;
    let ox = rx * k;
    let oy = ry * k;

    pb.move_to(cx - rx, cy);
    pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
    pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
    pb.close();
}

fn draw_path_with_state(
    pixmap: &mut Pixmap,
    path: &Path,
    state: &SvgRenderState,
    transform: Transform,
    current_color: Color,
) {
    // 1. Fill
    if let Some(ref fill_paint_type) = state.fill {
        let fill_color = match fill_paint_type {
            SvgPaint::None => None,
            SvgPaint::Color(c) => Some(*c),
            SvgPaint::CurrentColor => Some(current_color),
        };

        if let Some(fill_color) = fill_color {
            let alpha = (fill_color.a as f32 / 255.0) * state.fill_opacity * state.opacity;
            if alpha > 0.0 {
                let mut paint = Paint::default();
                let skia_color = SkiaColor::from_rgba(
                    fill_color.r as f32 / 255.0,
                    fill_color.g as f32 / 255.0,
                    fill_color.b as f32 / 255.0,
                    alpha,
                );
                if let Some(sc) = skia_color {
                    let b = path.bounds();
                    if b.width() > 0.0 && b.height() > 0.0 {
                        paint.set_color(sc);
                        paint.anti_alias = true;
                        pixmap.fill_path(path, &paint, state.fill_rule, transform, None);
                    }
                }
            }
        }
    }

    // 2. Stroke
    if let Some(ref stroke_paint_type) = state.stroke {
        let stroke_color = match stroke_paint_type {
            SvgPaint::None => None,
            SvgPaint::Color(c) => Some(*c),
            SvgPaint::CurrentColor => Some(current_color),
        };

        if let Some(stroke_color) = stroke_color {
            let alpha = (stroke_color.a as f32 / 255.0) * state.stroke_opacity * state.opacity;
            if alpha > 0.0 && state.stroke_width > 0.0 {
                let mut paint = Paint::default();
                let skia_color = SkiaColor::from_rgba(
                    stroke_color.r as f32 / 255.0,
                    stroke_color.g as f32 / 255.0,
                    stroke_color.b as f32 / 255.0,
                    alpha,
                );
                if let Some(sc) = skia_color {
                    paint.set_color(sc);
                    paint.anti_alias = true;
                    let stroke = Stroke {
                        width: state.stroke_width,
                        line_cap: state.stroke_linecap,
                        line_join: state.stroke_linejoin,
                        ..Default::default()
                    };
                    pixmap.stroke_path(path, &paint, &stroke, transform, None);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Attributes and CSS styles helpers
// ---------------------------------------------------------------------------

fn apply_attributes_to_state(
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

fn apply_single_attribute(state: &mut SvgRenderState, key: &str, val: &str) {
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

fn parse_paint(val: &str) -> Option<SvgPaint> {
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

fn parse_color_str(s: &str) -> Option<Color> {
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

fn parse_length(val: &str) -> Option<f32> {
    let s = val.trim().trim_end_matches("px");
    s.parse::<f32>().ok()
}

fn parse_view_box(val: &str) -> Option<(f32, f32, f32, f32)> {
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

fn parse_points(val: &str) -> Vec<(f32, f32)> {
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

fn parse_transform(val: &str) -> Option<Transform> {
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

fn get_f32_attr(attrs: &[(String, String)], name: &str) -> Option<f32> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .and_then(|(_, v)| parse_length(v))
}

fn get_str_attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim())
}

// ---------------------------------------------------------------------------
// SVG Path Parser (M, m, L, l, H, h, V, v, C, c, S, s, Q, q, T, t, A, a, Z, z)
// ---------------------------------------------------------------------------

pub fn parse_svg_path(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mut tokens = PathTokenizer::new(d);

    let mut cur_x = 0.0f32;
    let mut cur_y = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;
    let mut last_cp_x = 0.0f32;
    let mut last_cp_y = 0.0f32;
    let mut last_cmd = ' ';

    while let Some(cmd) = tokens.next_cmd() {
        match cmd {
            'M' | 'm' => {
                let is_rel = cmd == 'm';
                let mut first = true;
                while let (Some(x), Some(y)) = (tokens.next_f32(), tokens.next_f32()) {
                    let nx = if is_rel { cur_x + x } else { x };
                    let ny = if is_rel { cur_y + y } else { y };
                    if first {
                        pb.move_to(nx, ny);
                        start_x = nx;
                        start_y = ny;
                        first = false;
                    } else {
                        pb.line_to(nx, ny);
                    }
                    cur_x = nx;
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'L' | 'l' => {
                let is_rel = cmd == 'l';
                while let (Some(x), Some(y)) = (tokens.next_f32(), tokens.next_f32()) {
                    let nx = if is_rel { cur_x + x } else { x };
                    let ny = if is_rel { cur_y + y } else { y };
                    pb.line_to(nx, ny);
                    cur_x = nx;
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'H' | 'h' => {
                let is_rel = cmd == 'h';
                while let Some(x) = tokens.next_f32() {
                    let nx = if is_rel { cur_x + x } else { x };
                    pb.line_to(nx, cur_y);
                    cur_x = nx;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'V' | 'v' => {
                let is_rel = cmd == 'v';
                while let Some(y) = tokens.next_f32() {
                    let ny = if is_rel { cur_y + y } else { y };
                    pb.line_to(cur_x, ny);
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'C' | 'c' => {
                let is_rel = cmd == 'c';
                while let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let (nx1, ny1) = if is_rel {
                        (cur_x + x1, cur_y + y1)
                    } else {
                        (x1, y1)
                    };
                    let (nx2, ny2) = if is_rel {
                        (cur_x + x2, cur_y + y2)
                    } else {
                        (x2, y2)
                    };
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };

                    pb.cubic_to(nx1, ny1, nx2, ny2, nx, ny);
                    last_cp_x = nx2;
                    last_cp_y = ny2;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'S' | 's' => {
                let is_rel = cmd == 's';
                while let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let (nx2, ny2) = if is_rel {
                        (cur_x + x2, cur_y + y2)
                    } else {
                        (x2, y2)
                    };
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };

                    let (nx1, ny1) = if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                        (2.0 * cur_x - last_cp_x, 2.0 * cur_y - last_cp_y)
                    } else {
                        (cur_x, cur_y)
                    };

                    pb.cubic_to(nx1, ny1, nx2, ny2, nx, ny);
                    last_cp_x = nx2;
                    last_cp_y = ny2;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'Q' | 'q' => {
                let is_rel = cmd == 'q';
                while let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let (nx1, ny1) = if is_rel {
                        (cur_x + x1, cur_y + y1)
                    } else {
                        (x1, y1)
                    };
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };

                    pb.quad_to(nx1, ny1, nx, ny);
                    last_cp_x = nx1;
                    last_cp_y = ny1;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'T' | 't' => {
                let is_rel = cmd == 't';
                while let (Some(x), Some(y)) = (tokens.next_f32(), tokens.next_f32()) {
                    let (nx, ny) = if is_rel {
                        (cur_x + x, cur_y + y)
                    } else {
                        (x, y)
                    };
                    let (nx1, ny1) = if matches!(last_cmd, 'Q' | 'q' | 'T' | 't') {
                        (2.0 * cur_x - last_cp_x, 2.0 * cur_y - last_cp_y)
                    } else {
                        (cur_x, cur_y)
                    };

                    pb.quad_to(nx1, ny1, nx, ny);
                    last_cp_x = nx1;
                    last_cp_y = ny1;
                    cur_x = nx;
                    cur_y = ny;
                }
            }
            'A' | 'a' => {
                let is_rel = cmd == 'a';
                while let (
                    Some(rx),
                    Some(ry),
                    Some(x_axis_rotation),
                    Some(large_arc_flag),
                    Some(sweep_flag),
                    Some(x),
                    Some(y),
                ) = (
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                    tokens.next_flag(),
                    tokens.next_flag(),
                    tokens.next_f32(),
                    tokens.next_f32(),
                ) {
                    let nx = if is_rel { cur_x + x } else { x };
                    let ny = if is_rel { cur_y + y } else { y };

                    add_svg_arc_to_path(
                        &mut pb,
                        cur_x,
                        cur_y,
                        rx.abs(),
                        ry.abs(),
                        x_axis_rotation,
                        large_arc_flag != 0,
                        sweep_flag != 0,
                        nx,
                        ny,
                    );

                    cur_x = nx;
                    cur_y = ny;
                    last_cp_x = cur_x;
                    last_cp_y = cur_y;
                }
            }
            'Z' | 'z' => {
                pb.close();
                cur_x = start_x;
                cur_y = start_y;
                last_cp_x = cur_x;
                last_cp_y = cur_y;
            }
            _ => {}
        }
        last_cmd = cmd;
    }

    pb.finish()
}

/// Convert an SVG arc segment into cubic Bézier curves and add to the builder.
#[allow(clippy::too_many_arguments)]
fn add_svg_arc_to_path(
    pb: &mut PathBuilder,
    x0: f32,
    y0: f32,
    mut rx: f32,
    mut ry: f32,
    x_axis_rotation: f32,
    large_arc_flag: bool,
    sweep_flag: bool,
    x1: f32,
    y1: f32,
) {
    if (x0 - x1).abs() < 1e-6 && (y0 - y1).abs() < 1e-6 {
        return;
    }
    if rx < 1e-6 || ry < 1e-6 {
        pb.line_to(x1, y1);
        return;
    }

    let phi = x_axis_rotation * PI / 180.0;
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    let dx = (x0 - x1) / 2.0;
    let dy = (y0 - y1) / 2.0;

    let x1_p = cos_phi * dx + sin_phi * dy;
    let y1_p = -sin_phi * dx + cos_phi * dy;

    // Radii correction
    let prx = rx * rx;
    let pry = ry * ry;
    let px1 = x1_p * x1_p;
    let py1 = y1_p * y1_p;

    let radii_check = px1 / prx + py1 / pry;
    if radii_check > 1.0 {
        let factor = radii_check.sqrt();
        rx *= factor;
        ry *= factor;
    }

    let sign = if large_arc_flag != sweep_flag {
        1.0
    } else {
        -1.0
    };
    let numer = (rx * rx * ry * ry) - (rx * rx * y1_p * y1_p) - (ry * ry * x1_p * x1_p);
    let denom = (rx * rx * y1_p * y1_p) + (ry * ry * x1_p * x1_p);
    let sq = if denom > 0.0 {
        (numer.max(0.0) / denom).sqrt()
    } else {
        0.0
    };

    let cx_p = sign * sq * (rx * y1_p / ry);
    let cy_p = sign * sq * -(ry * x1_p / rx);

    let cx = cos_phi * cx_p - sin_phi * cy_p + (x0 + x1) / 2.0;
    let cy = sin_phi * cx_p + cos_phi * cy_p + (y0 + y1) / 2.0;

    let theta1 = angle_between(1.0, 0.0, (x1_p - cx_p) / rx, (y1_p - cy_p) / ry);
    let mut dtheta = angle_between(
        (x1_p - cx_p) / rx,
        (y1_p - cy_p) / ry,
        (-x1_p - cx_p) / rx,
        (-y1_p - cy_p) / ry,
    );

    if !sweep_flag && dtheta > 0.0 {
        dtheta -= 2.0 * PI;
    } else if sweep_flag && dtheta < 0.0 {
        dtheta += 2.0 * PI;
    }

    // Split into segments of at most PI/2
    let num_segments = (dtheta.abs() / (PI / 2.0)).ceil() as usize;
    let num_segments = num_segments.max(1);
    let seg_angle = dtheta / num_segments as f32;

    let mut current_theta = theta1;
    for _ in 0..num_segments {
        let next_theta = current_theta + seg_angle;
        let alpha = seg_angle / 2.0;
        let t = (4.0 / 3.0) * (alpha / 2.0).tan();

        let cos_curr = current_theta.cos();
        let sin_curr = current_theta.sin();
        let cos_next = next_theta.cos();
        let sin_next = next_theta.sin();

        // Control points on unit circle
        let p1x = cos_curr - t * sin_curr;
        let p1y = sin_curr + t * cos_curr;
        let p2x = cos_next + t * sin_next;
        let p2y = sin_next - t * cos_next;

        // Transform to ellipse and rotate
        let cp1_x = cos_phi * (rx * p1x) - sin_phi * (ry * p1y) + cx;
        let cp1_y = sin_phi * (rx * p1x) + cos_phi * (ry * p1y) + cy;
        let cp2_x = cos_phi * (rx * p2x) - sin_phi * (ry * p2y) + cx;
        let cp2_y = sin_phi * (rx * p2x) + cos_phi * (ry * p2y) + cy;
        let end_x = cos_phi * (rx * cos_next) - sin_phi * (ry * sin_next) + cx;
        let end_y = sin_phi * (rx * cos_next) + cos_phi * (ry * sin_next) + cy;

        pb.cubic_to(cp1_x, cp1_y, cp2_x, cp2_y, end_x, end_y);
        current_theta = next_theta;
    }
}

fn angle_between(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
    let dot = ux * vx + uy * vy;
    let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
    let cos_val = (dot / len.max(1e-6)).clamp(-1.0, 1.0);
    let sign = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 };
    sign * cos_val.acos()
}

// ---------------------------------------------------------------------------
// Path Tokenizer
// ---------------------------------------------------------------------------

struct PathTokenizer<'a> {
    chars: &'a [u8],
    idx: usize,
}

impl<'a> PathTokenizer<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.as_bytes(),
            idx: 0,
        }
    }

    fn skip_whitespace_and_commas(&mut self) {
        while self.idx < self.chars.len() {
            let c = self.chars[self.idx];
            if c.is_ascii_whitespace() || c == b',' {
                self.idx += 1;
            } else {
                break;
            }
        }
    }

    fn next_cmd(&mut self) -> Option<char> {
        self.skip_whitespace_and_commas();
        if self.idx >= self.chars.len() {
            return None;
        }
        let c = self.chars[self.idx] as char;
        if c.is_ascii_alphabetic() {
            self.idx += 1;
            Some(c)
        } else {
            None
        }
    }

    fn next_f32(&mut self) -> Option<f32> {
        self.skip_whitespace_and_commas();
        if self.idx >= self.chars.len() {
            return None;
        }

        let start = self.idx;
        let mut has_digit = false;
        let mut has_dot = false;

        if self.chars[self.idx] == b'+' || self.chars[self.idx] == b'-' {
            self.idx += 1;
        }

        while self.idx < self.chars.len() {
            let c = self.chars[self.idx];
            if c.is_ascii_digit() {
                has_digit = true;
                self.idx += 1;
            } else if c == b'.' && !has_dot {
                has_dot = true;
                self.idx += 1;
            } else if (c == b'e' || c == b'E') && has_digit {
                self.idx += 1;
                if self.idx < self.chars.len()
                    && (self.chars[self.idx] == b'+' || self.chars[self.idx] == b'-')
                {
                    self.idx += 1;
                }
            } else {
                break;
            }
        }

        if !has_digit {
            self.idx = start;
            return None;
        }

        let slice = std::str::from_utf8(&self.chars[start..self.idx]).ok()?;
        slice.parse::<f32>().ok()
    }

    fn next_flag(&mut self) -> Option<u8> {
        self.skip_whitespace_and_commas();
        if self.idx < self.chars.len() {
            let c = self.chars[self.idx];
            if c == b'0' || c == b'1' {
                self.idx += 1;
                return Some(c - b'0');
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Lightweight XML Tokenizer for SVG
// ---------------------------------------------------------------------------

#[allow(clippy::enum_variant_names)]
enum XmlToken {
    StartTag(String, Vec<(String, String)>),
    EmptyTag(String, Vec<(String, String)>),
    EndTag(String),
}

struct XmlTokenizer<'a> {
    content: &'a str,
    pos: usize,
}

impl<'a> XmlTokenizer<'a> {
    fn new(content: &'a str) -> Self {
        Self { content, pos: 0 }
    }

    fn next_token(&mut self) -> Option<XmlToken> {
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

fn parse_attributes(s: &str) -> Vec<(String, String)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_simple_svg_rect() {
        let svg = r##"<svg viewBox="0 0 100 100"><rect x="10" y="10" width="80" height="80" fill="#FF0000"/></svg>"##;
        let img = render_svg(svg, 100, 100, Color::BLACK).expect("SVG should render");
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);

        // Center pixel should be red (alpha = 255, red = 255)
        let center_idx = 50 * 100 + 50;
        let center_pixel = img.pixels[center_idx];
        let alpha = (center_pixel >> 24) & 0xFF;
        let red = (center_pixel >> 16) & 0xFF;
        assert_eq!(alpha, 255);
        assert_eq!(red, 255);
    }

    #[test]
    fn test_render_svg_path_current_color() {
        let svg =
            r#"<svg viewBox="0 0 24 24"><path d="M0 0 H24 V24 H0 Z" fill="currentColor"/></svg>"#;
        let custom_color = Color::rgb(40, 120, 200);
        let img = render_svg(svg, 24, 24, custom_color).expect("SVG should render");
        let p = img.pixels[12 * 24 + 12];
        let r = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let b = p & 0xFF;
        assert_eq!(r, 40);
        assert_eq!(g, 120);
        assert_eq!(b, 200);
    }

    #[test]
    fn test_parse_svg_path_arcs() {
        let d = "M10 80 Q 52.5 10, 95 80 T 180 80";
        let path = parse_svg_path(d);
        assert!(path.is_some());
    }

    #[test]
    fn test_svg_defs_symbol_and_use_intra_document() {
        let svg = r##"
        <svg viewBox="0 0 100 100">
            <defs>
                <symbol id="box-sym" viewBox="0 0 50 50">
                    <rect x="0" y="0" width="50" height="50" fill="#00FF00"/>
                </symbol>
            </defs>
            <use xlink:href="#box-sym" x="10" y="10" width="80" height="80"/>
        </svg>
        "##;
        let img = render_svg(svg, 100, 100, Color::BLACK).expect("SVG with use should render");
        let center_idx = 50 * 100 + 50;
        let center_pixel = img.pixels[center_idx];
        let alpha = (center_pixel >> 24) & 0xFF;
        let green = (center_pixel >> 8) & 0xFF;
        assert_eq!(alpha, 255);
        assert_eq!(green, 255);
    }

    #[test]
    fn test_svg_symbol_cross_document_sharing() {
        clear_global_svg_symbols();

        // 1. First SVG defines a symbol in a hidden sprite (like Wikipedia)
        let sprite_svg = r##"
        <svg style="display:none">
            <defs>
                <symbol id="global-star" viewBox="0 0 20 20">
                    <rect x="0" y="0" width="20" height="20" fill="#0000FF"/>
                </symbol>
            </defs>
        </svg>
        "##;
        let _ = render_svg(sprite_svg, 20, 20, Color::BLACK);

        // 2. Second SVG references #global-star without defining it
        let consumer_svg = r##"
        <svg viewBox="0 0 40 40">
            <use href="#global-star" x="0" y="0" width="40" height="40"/>
        </svg>
        "##;
        let img = render_svg(consumer_svg, 40, 40, Color::BLACK)
            .expect("Cross-SVG symbol should resolve");
        let center_idx = 20 * 40 + 20;
        let center_pixel = img.pixels[center_idx];
        let blue = center_pixel & 0xFF;
        assert_eq!(blue, 255);
    }

    #[test]
    fn test_svg_use_current_color_propagation() {
        let svg = r##"
        <svg viewBox="0 0 20 20">
            <defs>
                <symbol id="icon-fill">
                    <rect x="0" y="0" width="20" height="20"/>
                </symbol>
            </defs>
            <use href="#icon-fill" fill="currentColor"/>
        </svg>
        "##;
        let orange = Color::rgb(255, 165, 0);
        let img = render_svg(svg, 20, 20, orange).expect("SVG use currentColor should render");
        let p = img.pixels[10 * 20 + 10];
        let r = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let b = p & 0xFF;
        assert_eq!(r, 255);
        assert_eq!(g, 165);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_svg_use_intrinsic_dimensions_fallback() {
        let svg = r##"
        <svg>
            <defs>
                <symbol id="dim-icon" viewBox="0 0 64 32">
                    <rect x="0" y="0" width="64" height="32"/>
                </symbol>
            </defs>
            <use href="#dim-icon"/>
        </svg>
        "##;
        let (w, h) = get_svg_intrinsic_dimensions(svg);
        assert_eq!(w, Some(64.0));
        assert_eq!(h, Some(32.0));
    }

    #[test]
    fn test_svg_rendering_and_dom_caching() {
        let svg = r##"<svg viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="#123456"/></svg>"##;
        let img1 = render_svg(svg, 10, 10, Color::BLACK).expect("should render SVG");
        let img2 =
            render_svg(svg, 10, 10, Color::BLACK).expect("should return cached rendered SVG");

        assert_eq!(img1.width, img2.width);
        assert_eq!(img1.height, img2.height);
        assert_eq!(img1.pixels, img2.pixels);

        // Verify that the parsed DOM cache contains this SVG XML
        let cache = svg_doc_cache().read().unwrap();
        assert!(cache.contains_key(svg));

        // Verify that the render cache contains this key
        let rcache = svg_render_cache().read().unwrap();
        assert!(rcache.contains_key(&(svg.to_string(), 10, 10, Color::BLACK.to_argb_u32())));
    }
}
