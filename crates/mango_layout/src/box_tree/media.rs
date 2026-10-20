use std::collections::HashMap;

use mango_css::values::Length;

use crate::box_model::{BoxType, IFrameSandbox};
use crate::style_tree::StyledNode;

use super::builder::build_box_tree_with_maps;
use super::types::{LayoutBox, MapArea, parse_dimension_attr};

/// Builds a [`BoxType::IFrame`] layout box for `<iframe>` elements.
///
/// Implements HTML5 §4.8.5 `<iframe>`:
/// - Resolves intrinsic dimensions (default 300×150 per HTML5 spec, overridden by width/height attributes or CSS)
/// - Extracts security sandbox settings ([`IFrameSandbox`])
/// - Parses and lays out inner document from `srcdoc` or `data:text/html` into child boxes
/// - Sets navigation target for interactive click-through
pub fn build_iframe_element(styled_node: &StyledNode) -> LayoutBox {
    build_iframe_element_with_maps(styled_node, None)
}

pub fn build_iframe_element_with_maps(
    styled_node: &StyledNode,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let src = get_attr("src").unwrap_or("").trim().to_string();
    let srcdoc = get_attr("srcdoc").map(|s| s.to_string());
    let sandbox = IFrameSandbox::from_attribute(get_attr("sandbox"));

    // Standard HTML5 iframe defaults to 300x150
    let mut intrinsic_width = 300.0f32;
    let mut intrinsic_height = 150.0f32;

    if let Some(w_attr) = get_attr("width")
        && let Some(w) = parse_dimension_attr(w_attr)
        && w > 0.0
    {
        intrinsic_width = w;
    }
    if let Some(h_attr) = get_attr("height")
        && let Some(h) = parse_dimension_attr(h_attr)
        && h > 0.0
    {
        intrinsic_height = h;
    }

    // Prefer CSS styled dimensions if specified in pixels
    match styled_node.style.width {
        Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::IFrame {
        src: src.clone(),
        srcdoc: srcdoc.clone(),
        sandbox,
        intrinsic_width,
        intrinsic_height,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    // If src is a link/URL, set link target for navigation when clicked if no interactive children
    if !src.is_empty() && !src.starts_with("data:") && src != "about:blank" {
        b.link_target = Some(src.clone());
    }

    // Build nested browsing context content:
    // 1. If `srcdoc` is provided, parse and lay out inner document
    // 2. If `src` is `data:text/html,...`, decode and lay out
    let inner_html = if let Some(ref doc_html) = srcdoc {
        Some(doc_html.clone())
    } else if src.starts_with("data:text/html") {
        decode_data_uri_html(&src)
    } else {
        None
    };

    if let Some(html) = inner_html {
        let inner_doc = mango_html::parse_html(&html);
        if let Some(inner_styled_root) = crate::style_tree::build_style_tree(&inner_doc, &[]) {
            let inner_box = build_box_tree_with_maps(&inner_styled_root, maps);
            b.children.push(inner_box);
        }
    } else {
        // Fallback content inside <iframe>...</iframe> for older/unsupported frame markup
        for child in &styled_node.children {
            b.children.push(build_box_tree_with_maps(child, maps));
        }
    }

    b
}

/// Decodes an inline HTML data URI (`data:text/html,...` or `data:text/html;base64,...`).
fn decode_data_uri_html(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("data:")?;
    let comma_pos = rest.find(',')?;
    let metadata = &rest[..comma_pos];
    let payload = &rest[comma_pos + 1..];

    if metadata.contains(";base64") {
        let clean: Vec<u8> = payload
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        base64_decode_bytes(&clean).and_then(|bytes| String::from_utf8(bytes).ok())
    } else {
        Some(percent_decode_str(payload))
    }
}

fn percent_decode_str(input: &str) -> String {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(b);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn base64_decode_bytes(input: &[u8]) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let filtered: Vec<u8> = input.iter().copied().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);
    for chunk in filtered.chunks(4) {
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push((b2 << 6) | b3);
        }
    }
    Some(out)
}

/// Builds a [`BoxType::Video`] layout box for `<video>` elements.
///
/// Implements HTML5 §4.8.9 `<video>`:
/// - Resolves `src` from element attributes or child `<source>` elements
/// - Resolves and decodes `poster` image (supports `data:` URIs and cached HTTP URLs)
/// - Extracts playback controls (`controls`, `autoplay`, `loop`, `muted`) and runtime state
/// - Resolves intrinsic dimensions (defaults to 300×150 per HTML5 spec, or poster dimensions, overridden by CSS/attributes)
pub fn build_video_element(styled_node: &StyledNode) -> LayoutBox {
    build_video_element_with_maps(styled_node, None)
}

pub fn build_video_element_with_maps(
    styled_node: &StyledNode,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    // 1. Resolve source: check element attribute first, then child <source> elements
    let mut src = get_attr("src").unwrap_or("").trim().to_string();
    if src.is_empty() {
        for child in &styled_node.children {
            if child.tag_name.as_deref() == Some("source")
                && let Some((_, s)) = child
                    .attributes
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("src"))
            {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    src = trimmed.to_string();
                    break;
                }
            }
        }
    }

    // 2. Resolve poster image
    let poster = get_attr("poster").map(|s| s.trim().to_string());
    let mut poster_pixels = None;
    let mut poster_width = 0u32;
    let mut poster_height = 0u32;

    if let Some(ref poster_url) = poster {
        if poster_url.starts_with("data:image/") {
            if let Some(decoded) = mango_render::decode_data_uri(poster_url) {
                poster_width = decoded.width;
                poster_height = decoded.height;
                poster_pixels = Some(decoded.pixels);
            }
        } else if let Some(cached) = mango_render::get_cached_image(poster_url) {
            poster_width = cached.width;
            poster_height = cached.height;
            poster_pixels = Some(cached.pixels);
        }
    }

    // 3. Flags and runtime state
    let has_controls = get_attr("controls").is_some()
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("controls"));
    let autoplay = get_attr("autoplay").is_some()
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("autoplay"));
    let is_loop = get_attr("loop").is_some()
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("loop"));
    let is_muted = get_attr("muted").is_some()
        || get_attr("data-mango-muted") == Some("true")
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("muted"));

    let is_playing = get_attr("data-mango-playing") == Some("true")
        || (autoplay && get_attr("data-mango-playing") != Some("false"));
    let current_time = get_attr("data-mango-time")
        .and_then(|t| t.parse().ok())
        .unwrap_or(0.0f32);
    let duration = get_attr("data-mango-duration")
        .and_then(|d| d.parse().ok())
        .unwrap_or(if !src.is_empty() { 180.0f32 } else { 0.0f32 });

    // 4. Default dimensions (300x150 per HTML5 spec, or poster dimensions)
    let mut intrinsic_width = if poster_width > 0 {
        poster_width as f32
    } else {
        300.0f32
    };
    let mut intrinsic_height = if poster_height > 0 {
        poster_height as f32
    } else {
        150.0f32
    };

    if let Some(w_attr) = get_attr("width")
        && let Some(w) = parse_dimension_attr(w_attr)
        && w > 0.0
    {
        intrinsic_width = w;
    }
    if let Some(h_attr) = get_attr("height")
        && let Some(h) = parse_dimension_attr(h_attr)
        && h > 0.0
    {
        intrinsic_height = h;
    }

    match styled_node.style.width {
        Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::Video {
        src,
        poster,
        has_controls,
        autoplay,
        is_loop,
        is_muted,
        is_playing,
        current_time,
        duration,
        intrinsic_width,
        intrinsic_height,
        poster_pixels,
        poster_width,
        poster_height,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    // Preserve child nodes (e.g. <track>, fallback text)
    for child in &styled_node.children {
        b.children.push(build_box_tree_with_maps(child, maps));
    }
    b
}

/// Builds a [`BoxType::Audio`] layout box for `<audio>` elements.
///
/// Implements HTML5 §4.8.10 `<audio>`:
/// - Resolves `src` from element attributes or child `<source>` elements
/// - Extracts controls and playback state
/// - Sized to standard 300×36 bar when controls are enabled, or 0×0 / hidden when without controls
pub fn build_audio_element(styled_node: &StyledNode) -> LayoutBox {
    build_audio_element_with_maps(styled_node, None)
}

pub fn build_audio_element_with_maps(
    styled_node: &StyledNode,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let mut src = get_attr("src").unwrap_or("").trim().to_string();
    if src.is_empty() {
        for child in &styled_node.children {
            if child.tag_name.as_deref() == Some("source")
                && let Some((_, s)) = child
                    .attributes
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("src"))
            {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    src = trimmed.to_string();
                    break;
                }
            }
        }
    }

    let has_controls = get_attr("controls").is_some()
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("controls"));
    let autoplay = get_attr("autoplay").is_some()
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("autoplay"));
    let is_loop = get_attr("loop").is_some()
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("loop"));
    let is_muted = get_attr("muted").is_some()
        || get_attr("data-mango-muted") == Some("true")
        || styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("muted"));

    let is_playing = get_attr("data-mango-playing") == Some("true")
        || (autoplay && get_attr("data-mango-playing") != Some("false"));
    let current_time = get_attr("data-mango-time")
        .and_then(|t| t.parse().ok())
        .unwrap_or(0.0f32);
    let duration = get_attr("data-mango-duration")
        .and_then(|d| d.parse().ok())
        .unwrap_or(if !src.is_empty() { 210.0f32 } else { 0.0f32 });

    let mut intrinsic_width = if has_controls { 300.0f32 } else { 0.0f32 };
    let mut intrinsic_height = if has_controls { 36.0f32 } else { 0.0f32 };

    match styled_node.style.width {
        Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::Audio {
        src,
        has_controls,
        autoplay,
        is_loop,
        is_muted,
        is_playing,
        current_time,
        duration,
        intrinsic_width,
        intrinsic_height,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    for child in &styled_node.children {
        b.children.push(build_box_tree_with_maps(child, maps));
    }
    b
}

/// Builds a [`BoxType::Canvas`] layout box for `<canvas>` elements.
///
/// Implements HTML5 §4.12.5 `<canvas>`:
/// - Resolves pixel dimensions from width/height attributes or CSS styles (defaulting to 300×150)
/// - Queries the global canvas store for backing pixel data
/// - Fallback child content is NOT rendered to page layout per specification
pub fn build_canvas_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let mut width = 300u32;
    let mut height = 150u32;

    if let Some(w_attr) = get_attr("width")
        && let Some(w) = parse_dimension_attr(w_attr)
        && w > 0.0
    {
        width = w as u32;
    }
    if let Some(h_attr) = get_attr("height")
        && let Some(h) = parse_dimension_attr(h_attr)
        && h > 0.0
    {
        height = h as u32;
    }

    let node_idx = styled_node.node_id.map(|id| id.raw() as usize);

    // If canvas is already instantiated in CANVAS_REGISTRY, retrieve its actual buffer & dimensions
    let mut pixels = None;
    if let Some(nid) = node_idx
        && let Some((cw, ch, c_pixels)) = mango_render::get_canvas_pixels(nid)
    {
        width = cw;
        height = ch;
        pixels = Some(c_pixels);
    }

    let mut intrinsic_width = width as f32;
    let mut intrinsic_height = height as f32;

    match styled_node.style.width {
        Length::Px(px) if px > 0.0 => intrinsic_width = px,
        _ => {}
    }
    match styled_node.style.height {
        Length::Px(px) if px > 0.0 => intrinsic_height = px,
        _ => {}
    }

    let box_type = BoxType::Canvas {
        node_id: node_idx,
        width,
        height,
        intrinsic_width,
        intrinsic_height,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();
    // Do NOT push children: canvas child nodes are fallback content only!
    b
}
