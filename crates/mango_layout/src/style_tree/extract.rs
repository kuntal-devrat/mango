use std::collections::HashMap;

use mango_css::RuleIndex;
use mango_css::parser::{Stylesheet, parse_stylesheet};
use mango_html::dom::{Document, NodeData, NodeId};

/// Context for rendering elements within a shadow root.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ShadowContext<'a> {
    pub shadow_root: NodeId,
    pub host_id: NodeId,
    pub rule_index: &'a RuleIndex<'a>,
    pub slot_assignments: HashMap<NodeId, Vec<NodeId>>,
}

/// Extracts all embedded `<style>` elements from the document and parses them into stylesheets.
pub fn extract_style_elements(doc: &Document) -> Vec<Stylesheet> {
    let mut stylesheets = Vec::new();
    collect_styles_recursive(doc, doc.root(), &mut stylesheets);
    stylesheets
}

pub fn collect_styles_recursive(doc: &Document, node_id: NodeId, out: &mut Vec<Stylesheet>) {
    for child in doc.children(node_id) {
        if let NodeData::Element(el) = &child.data {
            if el.tag_name.eq_ignore_ascii_case("noscript") {
                // When scripting is enabled, styles inside <noscript> must not be applied.
                continue;
            }
            if el.tag_name.eq_ignore_ascii_case("style") {
                let css_text = doc.text_content(child.id);
                if !css_text.trim().is_empty() {
                    out.push(parse_stylesheet(&css_text));
                }
            }
        }
        collect_styles_recursive(doc, child.id, out);
    }
}

/// Discovers all shadow roots in the document (including declarative and nested).
pub fn scan_all_shadow_roots(doc: &Document) -> HashMap<NodeId, NodeId> {
    let mut map = doc.shadow_hosts.clone();
    let mut stack = vec![doc.root()];
    while let Some(nid) = stack.pop() {
        if let Some(node) = doc.get(nid) {
            if let NodeData::Element(el) = &node.data {
                if let Some(sr_id) = el.shadow_root {
                    map.entry(sr_id).or_insert(nid);
                    stack.push(sr_id);
                }
            }
            for child in doc.children(nid) {
                stack.push(child.id);
            }
        }
    }
    map
}

/// Extracts embedded `<style>` stylesheets from within a shadow root.
pub fn extract_shadow_stylesheets(doc: &Document, sr_id: NodeId) -> Vec<Stylesheet> {
    let mut stylesheets = Vec::new();
    collect_styles_recursive(doc, sr_id, &mut stylesheets);
    stylesheets
}

/// Computes slot assignments for a given shadow root in tree order.
pub fn compute_slot_assignments(
    doc: &Document,
    host_id: NodeId,
    sr_id: NodeId,
) -> HashMap<NodeId, Vec<NodeId>> {
    let mut map: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut slots = Vec::new();

    let mut stack = vec![sr_id];
    while let Some(nid) = stack.pop() {
        if let Some(node) = doc.get(nid) {
            if let NodeData::Element(el) = &node.data {
                if el.tag_name.eq_ignore_ascii_case("template") {
                    continue;
                }
                if el.tag_name.eq_ignore_ascii_case("slot") {
                    slots.push(nid);
                }
            }
            let mut children: Vec<NodeId> = doc.children(nid).map(|c| c.id).collect();
            children.reverse();
            stack.extend(children);
        }
    }

    for &slot_id in &slots {
        map.insert(slot_id, Vec::new());
    }

    for light_child in doc.children(host_id) {
        let child_slot = if let NodeData::Element(el) = &light_child.data {
            el.get_attribute("slot").unwrap_or("")
        } else {
            ""
        };
        for &slot_id in &slots {
            if let Some(slot_node) = doc.get(slot_id)
                && let NodeData::Element(slot_el) = &slot_node.data
            {
                let slot_name = slot_el.get_attribute("name").unwrap_or("");
                if slot_name == child_slot {
                    map.get_mut(&slot_id).unwrap().push(light_child.id);
                    break;
                }
            }
        }
    }

    map
}

/// Scans the entire DOM for all `<svg>` elements (including hidden or defs) and registers their symbols.
pub fn scan_and_register_global_svg_symbols(doc: &Document) {
    let mut stack = vec![doc.root()];
    while let Some(nid) = stack.pop() {
        if let Some(node) = doc.get(nid) {
            if let NodeData::Element(elem) = &node.data
                && elem.tag_name.eq_ignore_ascii_case("svg")
            {
                let svg_xml = serialize_svg_dom_node(doc, nid);
                if !svg_xml.is_empty() {
                    let _ = mango_render::get_svg_intrinsic_dimensions(&svg_xml);
                }
            }
            for child in doc.children(nid) {
                stack.push(child.id);
            }
        }
    }
}

pub fn serialize_svg_dom_node(doc: &Document, node_id: NodeId) -> String {
    let mut out = String::new();
    serialize_svg_dom_recursive(doc, node_id, &mut out);
    out
}

pub fn serialize_svg_dom_recursive(doc: &Document, node_id: NodeId, out: &mut String) {
    let Some(node) = doc.get(node_id) else {
        return;
    };
    match &node.data {
        NodeData::Element(elem) => {
            out.push('<');
            out.push_str(&elem.tag_name);
            for (k, v) in &elem.attributes {
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
            let children: Vec<_> = doc.children(node_id).collect();
            if children.is_empty() {
                out.push_str("/>");
            } else {
                out.push('>');
                for child in children {
                    serialize_svg_dom_recursive(doc, child.id, out);
                }
                out.push_str("</");
                out.push_str(&elem.tag_name);
                out.push('>');
            }
        }
        NodeData::Text(t) => {
            for ch in t.chars() {
                match ch {
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '&' => out.push_str("&amp;"),
                    _ => out.push(ch),
                }
            }
        }
        _ => {}
    }
}

/// Scans the entire DOM for all `<map>` and `<area>` elements and collects them into a map.
pub fn collect_image_maps(doc: &Document) -> HashMap<String, Vec<crate::box_tree::MapArea>> {
    let mut map_registry = HashMap::new();
    let mut stack = vec![doc.root()];
    while let Some(nid) = stack.pop() {
        if let Some(node) = doc.get(nid) {
            if let NodeData::Element(elem) = &node.data
                && elem.tag_name.eq_ignore_ascii_case("map")
            {
                let map_name = elem
                    .get_attribute("name")
                    .or_else(|| elem.get_attribute("id"))
                    .unwrap_or("")
                    .trim();
                if !map_name.is_empty() {
                    let mut areas = Vec::new();
                    for child in doc.children(nid) {
                        if let NodeData::Element(area_elem) = &child.data
                            && area_elem.tag_name.eq_ignore_ascii_case("area")
                        {
                            let shape_str = area_elem.get_attribute("shape");
                            let coords_str = area_elem.get_attribute("coords").unwrap_or("");
                            let href = area_elem.get_attribute("href").map(|s| s.to_string());
                            let alt = area_elem.get_attribute("alt").map(|s| s.to_string());
                            let target = area_elem.get_attribute("target").map(|s| s.to_string());
                            let shape = crate::box_tree::AreaShape::parse(shape_str, coords_str);
                            areas.push(crate::box_tree::MapArea {
                                shape,
                                href,
                                alt,
                                target,
                            });
                        }
                    }
                    map_registry.insert(map_name.to_ascii_lowercase(), areas);
                }
            }
            for child in doc.children(nid) {
                stack.push(child.id);
            }
        }
    }
    map_registry
}

/// Scans the entire DOM for all `<map>` and `<area>` elements and registers them in the global fallback registry.
pub fn scan_and_register_image_maps(doc: &Document) {
    crate::box_tree::clear_image_maps();
    let maps = collect_image_maps(doc);
    for (name, areas) in maps {
        crate::box_tree::register_image_map(&name, areas);
    }
}
