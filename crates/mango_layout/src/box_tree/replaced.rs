use std::collections::HashMap;

use mango_css::values::Length;

use crate::box_model::BoxType;
use crate::style_tree::StyledNode;

use super::types::{LayoutBox, MapArea, get_image_map, parse_dimension_attr};

pub(crate) fn resolve_map_areas(
    usemap: &str,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) -> Option<Vec<MapArea>> {
    let clean_name = usemap.trim().trim_start_matches('#').to_ascii_lowercase();
    if let Some(m) = maps
        && let Some(areas) = m.get(&clean_name)
    {
        return Some(areas.clone());
    }
    get_image_map(usemap)
}

/// Splits a `srcset` attribute into candidates without incorrectly splitting on commas inside `data:` URIs.
pub fn parse_srcset_candidates(srcset: &str) -> Vec<&str> {
    let mut list = Vec::new();
    let s = srcset.trim();
    if s.is_empty() {
        return list;
    }
    let mut start = 0;
    let mut in_data_header = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if s[i..].starts_with("data:") {
            in_data_header = true;
        }
        if bytes[i] == b',' {
            if in_data_header {
                in_data_header = false;
            } else {
                let candidate = s[start..i].trim();
                if !candidate.is_empty() {
                    list.push(candidate);
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        list.push(tail);
    }
    list
}

/// Builds a [`ReplacedElement`](BoxType::ReplacedElement) for `<img>` elements.
///
/// Reads `src`, `width`, and `height` attributes. Supports `data:` URIs for inline images.
#[allow(dead_code)]
pub fn build_replaced_element(styled_node: &StyledNode) -> LayoutBox {
    build_replaced_element_with_maps(styled_node, None)
}

pub fn build_replaced_element_with_maps(
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

    // Parse explicit width/height: prefer CSS style if px, otherwise HTML attributes
    let explicit_w: Option<f32> = match styled_node.style.width {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(parse_dimension_attr),
    };
    let explicit_h: Option<f32> = match styled_node.style.height {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(parse_dimension_attr),
    };

    // Helper to try decoding data URI or getting from cache (including with https: prefix)
    let lookup_image = |url: &str| -> Option<mango_render::image_decode::DecodedImage> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(img) = mango_render::image_decode::decode_data_uri(trimmed) {
            return Some(img);
        }
        if let Some(img) = mango_render::image_decode::get_cached_image(trimmed) {
            return Some(img);
        }
        if trimmed.starts_with("//") {
            let https_url = format!("https:{trimmed}");
            if let Some(img) = mango_render::image_decode::get_cached_image(&https_url) {
                return Some(img);
            }
        }
        None
    };

    // 1. First priority: check <picture> / <source> responsive image candidates
    let mut decoded = None;
    if let Some(pic_sources) = get_attr("_mango_picture_sources") {
        let (vp_w, vp_h) = mango_css::get_current_viewport();
        let (vp_w, vp_h) = if vp_w > 0.0 && vp_h > 0.0 {
            (vp_w, vp_h)
        } else {
            (800.0, 600.0)
        };

        for line in pic_sources.lines() {
            let mut parts = line.split('\t');
            let srcset = parts.next().unwrap_or("");
            let media = parts.next().unwrap_or("");
            let mime = parts.next().unwrap_or("");

            // Check media query against active viewport
            if !media.is_empty() && !mango_css::matches_media_query_size(media, vp_w, vp_h) {
                continue;
            }
            // Check MIME type
            if !mime.is_empty() {
                let lower_mime = mime.to_ascii_lowercase();
                if !lower_mime.starts_with("image/") {
                    continue;
                }
            }
            // Candidate matched! Try each candidate in srcset
            for candidate in parse_srcset_candidates(srcset) {
                let url = candidate.split_whitespace().next().unwrap_or("");
                if let Some(img) = lookup_image(url) {
                    decoded = Some(img);
                    break;
                }
            }
            if decoded.is_some() {
                break;
            }
        }
    }

    let src = get_attr("src").unwrap_or("");
    if decoded.is_none() {
        decoded = lookup_image(src);
    }

    // Fallback: check srcset (candidates separated by commas)
    if decoded.is_none()
        && let Some(srcset) = get_attr("srcset")
    {
        for candidate in parse_srcset_candidates(srcset) {
            let url = candidate.split_whitespace().next().unwrap_or("");
            if let Some(img) = lookup_image(url) {
                decoded = Some(img);
                break;
            }
        }
    }

    // Fallback: check data-src
    if decoded.is_none()
        && let Some(data_src) = get_attr("data-src")
    {
        decoded = lookup_image(data_src);
    }

    let (intrinsic_width, intrinsic_height, pixels) = if let Some(img) = decoded {
        let natural_w = img.width as f32;
        let natural_h = img.height as f32;
        let aspect_ratio =
            if let Some(css_ratio) = styled_node.style.aspect_ratio.filter(|&r| r > 0.0) {
                css_ratio
            } else if natural_h > 0.0 {
                natural_w / natural_h
            } else {
                1.0
            };

        let (w, h) = match (explicit_w, explicit_h) {
            (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
            (Some(ew), None) => {
                let ew = ew.max(1.0);
                (ew, (ew / aspect_ratio).max(1.0))
            }
            (None, Some(eh)) => {
                let eh = eh.max(1.0);
                (((eh * aspect_ratio).max(1.0)), eh)
            }
            (None, None) => (natural_w.max(1.0), natural_h.max(1.0)),
        };

        let scaled = if !src.is_empty() {
            mango_render::image_decode::get_or_resize_cached(src, &img, w as u32, h as u32)
        } else {
            img.resize(w as u32, h as u32)
        };
        (w, h, scaled.pixels)
    } else {
        // Use placeholder for missing/failed images
        let aspect_ratio = styled_node
            .style
            .aspect_ratio
            .filter(|&r| r > 0.0)
            .unwrap_or(4.0 / 3.0);
        let (w, h) = match (explicit_w, explicit_h) {
            (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
            (Some(ew), None) => (ew.max(1.0), (ew / aspect_ratio).max(1.0)),
            (None, Some(eh)) => (((eh * aspect_ratio).max(1.0)), eh.max(1.0)),
            (None, None) => (64.0, 64.0),
        };
        let placeholder = mango_render::image_decode::create_placeholder(w as u32, h as u32);
        (w, h, placeholder.pixels)
    };

    let box_type = BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    if let Some(usemap) = get_attr("usemap")
        && let Some(areas) = resolve_map_areas(usemap, maps)
    {
        b.map_areas = areas;
    }

    b
}

/// Builds a layout box for `<embed>` elements.
pub fn build_embed_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let explicit_w: Option<f32> = match styled_node.style.width {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(parse_dimension_attr),
    };
    let explicit_h: Option<f32> = match styled_node.style.height {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(parse_dimension_attr),
    };

    let w = explicit_w.unwrap_or(300.0).max(1.0);
    let h = explicit_h.unwrap_or(150.0).max(1.0);

    let src = get_attr("src").unwrap_or("").trim();
    let decoded = if !src.is_empty() {
        if let Some(img) = mango_render::image_decode::decode_data_uri(src) {
            Some(img)
        } else {
            mango_render::image_decode::get_cached_image(src)
        }
    } else {
        None
    };

    let pixels = if let Some(img) = decoded {
        img.resize(w as u32, h as u32).pixels
    } else {
        mango_render::image_decode::create_placeholder(w as u32, h as u32).pixels
    };

    let box_type = BoxType::ReplacedElement {
        intrinsic_width: w,
        intrinsic_height: h,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();
    b
}

/// Attempts to build a [`BoxType::ReplacedElement`] for `<object>` elements if media data is valid.
#[allow(dead_code)]
pub fn try_build_object_element(styled_node: &StyledNode) -> Option<LayoutBox> {
    try_build_object_element_with_maps(styled_node, None)
}

pub fn try_build_object_element_with_maps(
    styled_node: &StyledNode,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) -> Option<LayoutBox> {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let data = get_attr("data").or_else(|| get_attr("src"))?.trim();
    if data.is_empty() {
        return None;
    }

    let decoded = if let Some(img) = mango_render::image_decode::decode_data_uri(data) {
        Some(img)
    } else {
        mango_render::image_decode::get_cached_image(data)
    };

    let img = decoded?;

    let explicit_w: Option<f32> = match styled_node.style.width {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(parse_dimension_attr),
    };
    let explicit_h: Option<f32> = match styled_node.style.height {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(parse_dimension_attr),
    };

    let natural_w = img.width as f32;
    let natural_h = img.height as f32;
    let aspect_ratio = if let Some(css_ratio) = styled_node.style.aspect_ratio.filter(|&r| r > 0.0)
    {
        css_ratio
    } else if natural_h > 0.0 {
        natural_w / natural_h
    } else {
        1.0
    };

    let (w, h) = match (explicit_w, explicit_h) {
        (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
        (Some(ew), None) => (ew.max(1.0), (ew / aspect_ratio).max(1.0)),
        (None, Some(eh)) => (((eh * aspect_ratio).max(1.0)), eh.max(1.0)),
        (None, None) => (natural_w.max(1.0), natural_h.max(1.0)),
    };

    let scaled = img.resize(w as u32, h as u32);
    let box_type = BoxType::ReplacedElement {
        intrinsic_width: w,
        intrinsic_height: h,
        pixels: scaled.pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();

    if let Some(usemap) = get_attr("usemap")
        && let Some(areas) = resolve_map_areas(usemap, maps)
    {
        b.map_areas = areas;
    }

    Some(b)
}

/// Builds a [`ReplacedElement`](BoxType::ReplacedElement) for `<svg>` elements.
pub fn build_svg_replaced_element(styled_node: &StyledNode) -> LayoutBox {
    let get_attr = |name: &str| -> Option<&str> {
        styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    // Serialize styled node to SVG XML
    let svg_xml = serialize_svg_styled_node(styled_node);

    // Dimensions: check CSS style.width/height, then SVG width/height attributes, then viewBox
    let (intrinsic_w_opt, intrinsic_h_opt) = mango_render::get_svg_intrinsic_dimensions(&svg_xml);

    let explicit_w: Option<f32> = match styled_node.style.width {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("width").and_then(parse_dimension_attr),
    };

    let explicit_h: Option<f32> = match styled_node.style.height {
        Length::Px(px) if px > 0.0 => Some(px),
        _ => get_attr("height").and_then(parse_dimension_attr),
    };

    let aspect_ratio = if let Some(css_ratio) = styled_node.style.aspect_ratio.filter(|&r| r > 0.0)
    {
        Some(css_ratio)
    } else if let (Some(iw), Some(ih)) = (intrinsic_w_opt, intrinsic_h_opt) {
        if ih > 0.0 { Some(iw / ih) } else { None }
    } else {
        None
    };

    let (w, h) = match (explicit_w, explicit_h) {
        (Some(ew), Some(eh)) => (ew.max(1.0), eh.max(1.0)),
        (Some(ew), None) => {
            let ew = ew.max(1.0);
            if let Some(ratio) = aspect_ratio {
                (ew, (ew / ratio).max(1.0))
            } else if let Some(ih) = intrinsic_h_opt {
                (ew, ih.max(1.0))
            } else {
                (ew, ew)
            }
        }
        (None, Some(eh)) => {
            let eh = eh.max(1.0);
            if let Some(ratio) = aspect_ratio {
                (((eh * ratio).max(1.0)), eh)
            } else if let Some(iw) = intrinsic_w_opt {
                (iw.max(1.0), eh)
            } else {
                (eh, eh)
            }
        }
        (None, None) => {
            let def_w = intrinsic_w_opt.unwrap_or(300.0);
            let def_h = intrinsic_h_opt.unwrap_or_else(|| {
                if let Some(ratio) = aspect_ratio {
                    def_w / ratio
                } else {
                    150.0
                }
            });
            (def_w.max(1.0), def_h.max(1.0))
        }
    };

    let current_color = styled_node.style.color;
    let decoded = mango_render::render_svg(&svg_xml, w as u32, h as u32, current_color);

    let pixels = if let Some(img) = decoded {
        img.pixels
    } else {
        mango_render::image_decode::create_placeholder(w as u32, h as u32).pixels
    };

    let intrinsic_width = intrinsic_w_opt.unwrap_or(w);
    let intrinsic_height = intrinsic_h_opt.unwrap_or(h);

    let box_type = BoxType::ReplacedElement {
        intrinsic_width,
        intrinsic_height,
        pixels,
    };

    let mut b = LayoutBox::new(box_type, Some(styled_node.style.clone()));
    b.node_id = styled_node.node_id;
    b.tag_name = styled_node.tag_name.clone();
    b.attributes = styled_node.attributes.clone();
    b
}

pub(crate) fn serialize_svg_styled_node(node: &StyledNode) -> String {
    let mut out = String::new();
    serialize_svg_recursive(node, &mut out);
    out
}

fn serialize_svg_recursive(node: &StyledNode, out: &mut String) {
    if let Some(ref tag) = node.tag_name {
        out.push('<');
        out.push_str(tag);
        for (k, v) in &node.attributes {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            for ch in v.chars() {
                match ch {
                    '"' => out.push_str("&quot;"),
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    _ => out.push(ch),
                }
            }
            out.push('"');
        }

        if node.children.is_empty() && node.text.is_none() {
            out.push_str("/>");
        } else {
            out.push('>');
            if let Some(ref t) = node.text {
                for ch in t.chars() {
                    match ch {
                        '<' => out.push_str("&lt;"),
                        '>' => out.push_str("&gt;"),
                        '&' => out.push_str("&amp;"),
                        _ => out.push(ch),
                    }
                }
            }
            for child in &node.children {
                serialize_svg_recursive(child, out);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
    }
}
