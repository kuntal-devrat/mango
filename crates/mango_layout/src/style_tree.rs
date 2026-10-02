//! Style tree construction: associates DOM nodes with their computed CSS styles.
//!
//! Elements with `display: none` and non-rendered nodes (`<head>`, `<style>`, `<script>`)
//! are filtered out, producing a clean tree of rendered styled nodes.

use mango_css::computed::{
    ComputedStyle, compute_pseudo_style_with_index, compute_style_with_index,
};
use mango_css::parser::{Stylesheet, parse_stylesheet};
use mango_css::values::{ContentItem, Display};
use mango_css::{Origin, RuleIndex};
use mango_html::dom::{Document, NodeData, NodeId};

/// A node in the style tree, holding computed style, optional DOM ID, and optional text payload.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledNode {
    pub node_id: Option<NodeId>,
    pub style: ComputedStyle,
    pub text: Option<String>,
    /// The element's tag name (e.g. "div", "img"), or None for text/document nodes.
    pub tag_name: Option<String>,
    /// Element attributes (e.g. "src", "width", "height").
    pub attributes: Vec<(String, String)>,
    pub children: Vec<StyledNode>,
}

impl StyledNode {
    pub fn new(node_id: Option<NodeId>, style: ComputedStyle) -> Self {
        Self {
            node_id,
            style,
            text: None,
            tag_name: None,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_text(node_id: Option<NodeId>, style: ComputedStyle, text: String) -> Self {
        Self {
            node_id,
            style,
            text: Some(text),
            tag_name: None,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Returns `true` if this styled node represents a text fragment.
    pub fn is_text(&self) -> bool {
        self.text.is_some()
    }
}

/// Extracts all embedded `<style>` elements from the document and parses them into stylesheets.
pub fn extract_style_elements(doc: &Document) -> Vec<Stylesheet> {
    let mut stylesheets = Vec::new();
    collect_styles_recursive(doc, doc.root(), &mut stylesheets);
    stylesheets
}

fn collect_styles_recursive(doc: &Document, node_id: NodeId, out: &mut Vec<Stylesheet>) {
    for child in doc.children(node_id) {
        if let NodeData::Element(el) = &child.data
            && el.tag_name.eq_ignore_ascii_case("style")
        {
            let css_text = doc.text_content(child.id);
            if !css_text.trim().is_empty() {
                out.push(parse_stylesheet(&css_text));
            }
        }
        collect_styles_recursive(doc, child.id, out);
    }
}

/// Scans the entire DOM for all `<svg>` elements (including hidden or defs) and registers their symbols.
fn scan_and_register_global_svg_symbols(doc: &Document) {
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

fn serialize_svg_dom_node(doc: &Document, node_id: NodeId) -> String {
    let mut out = String::new();
    serialize_svg_dom_recursive(doc, node_id, &mut out);
    out
}

fn serialize_svg_dom_recursive(doc: &Document, node_id: NodeId, out: &mut String) {
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

/// Scans the entire DOM for all `<map>` and `<area>` elements and registers them.
fn scan_and_register_image_maps(doc: &Document) {
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
                    crate::box_tree::register_image_map(map_name, areas);
                }
            }
            for child in doc.children(nid) {
                stack.push(child.id);
            }
        }
    }
}

/// Context for tracking scoped CSS counters and quotation nesting depths during style tree construction.
#[derive(Debug, Clone, Default)]
pub struct CounterContext {
    counters: std::collections::HashMap<String, Vec<i32>>,
    pub quote_depth: usize,
}

impl CounterContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> i32 {
        self.counters
            .get(name)
            .and_then(|stack| stack.last().copied())
            .unwrap_or(0)
    }

    pub fn get_all(&self, name: &str) -> Vec<i32> {
        self.counters.get(name).cloned().unwrap_or_else(|| vec![0])
    }

    pub fn reset(&mut self, name: &str, value: i32) {
        self.counters
            .entry(name.to_string())
            .or_default()
            .push(value);
    }

    pub fn increment(&mut self, name: &str, value: i32) {
        let stack = self.counters.entry(name.to_string()).or_default();
        if let Some(top) = stack.last_mut() {
            *top += value;
        } else {
            stack.push(value);
        }
    }

    pub fn pop(&mut self, name: &str) {
        if let Some(stack) = self.counters.get_mut(name) {
            stack.pop();
        }
    }
}

fn format_counter_value(val: i32, style: &str) -> String {
    match style {
        "decimal" => val.to_string(),
        "decimal-leading-zero" => {
            if val >= 0 && val < 10 {
                format!("0{}", val)
            } else if val < 0 && val > -10 {
                format!("-0{}", -val)
            } else {
                val.to_string()
            }
        }
        "none" => String::new(),
        "lower-alpha" | "lower-latin" => {
            if val <= 0 {
                return val.to_string();
            }
            int_to_alpha(val as u32, false)
        }
        "upper-alpha" | "upper-latin" => {
            if val <= 0 {
                return val.to_string();
            }
            int_to_alpha(val as u32, true)
        }
        "lower-roman" => {
            if val <= 0 || val > 3999 {
                return val.to_string();
            }
            int_to_roman(val as u32).to_ascii_lowercase()
        }
        "upper-roman" => {
            if val <= 0 || val > 3999 {
                return val.to_string();
            }
            int_to_roman(val as u32)
        }
        _ => val.to_string(),
    }
}

fn int_to_alpha(mut n: u32, upper: bool) -> String {
    let mut res = Vec::new();
    while n > 0 {
        n -= 1;
        let rem = (n % 26) as u8;
        let base = if upper { b'A' } else { b'a' };
        res.push((base + rem) as char);
        n /= 26;
    }
    res.reverse();
    res.into_iter().collect()
}

fn int_to_roman(mut n: u32) -> String {
    const ROMANS: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for &(val, name) in ROMANS {
        while n >= val {
            s.push_str(name);
            n -= val;
        }
    }
    s
}

/// Builds the styled tree from a DOM [`Document`], applying both external author
/// stylesheets and any inline `<style>` sheets found in the document.
pub fn build_style_tree(doc: &Document, author_styles: &[&Stylesheet]) -> Option<StyledNode> {
    build_style_tree_with_size(doc, author_styles, 1280.0, 900.0)
}

/// Builds the styled tree from a DOM [`Document`] with explicit viewport width and height
/// for accurate media query matching.
pub fn build_style_tree_with_size(
    doc: &Document,
    author_styles: &[&Stylesheet],
    viewport_width: f32,
    viewport_height: f32,
) -> Option<StyledNode> {
    mango_css::set_current_viewport(viewport_width, viewport_height);
    // 0. Pre-register all global SVG symbols and HTML image maps
    scan_and_register_global_svg_symbols(doc);
    scan_and_register_image_maps(doc);

    // 1. Extract any embedded <style> tags from the document
    let embedded_sheets = extract_style_elements(doc);
    let mut all_author_sheets: Vec<&Stylesheet> = Vec::new();
    all_author_sheets.extend(author_styles);
    for sheet in &embedded_sheets {
        all_author_sheets.push(sheet);
    }

    // 2. Build RuleIndex ONCE for all author stylesheets
    let mut order = 1000;
    let author_index = RuleIndex::from_stylesheets_with_size(
        &all_author_sheets,
        Origin::Author,
        &mut order,
        viewport_width,
        viewport_height,
    );

    // 3. Recursively build style tree from root
    let mut counter_ctx = CounterContext::new();
    build_styled_node(doc, doc.root(), &author_index, None, &mut counter_ctx)
}

fn is_node_inline(
    doc: &Document,
    node_id: NodeId,
    author_index: &RuleIndex,
    parent_style: Option<&ComputedStyle>,
) -> bool {
    let Some(node) = doc.get(node_id) else {
        return false;
    };
    match &node.data {
        NodeData::Element(el) => {
            if el.get_attribute("hidden").is_some()
                || el.tag_name.eq_ignore_ascii_case("template")
                || el.tag_name.eq_ignore_ascii_case("head")
                || el.tag_name.eq_ignore_ascii_case("style")
                || el.tag_name.eq_ignore_ascii_case("script")
            {
                return false;
            }
            let style = compute_style_with_index(node_id, doc, author_index, parent_style);
            style.display.is_inline_level()
        }
        NodeData::Text(t) => !t.chars().all(|c| c.is_ascii_whitespace()),
        _ => false,
    }
}

fn build_styled_node(
    doc: &Document,
    node_id: NodeId,
    author_index: &RuleIndex,
    parent_style: Option<&ComputedStyle>,
    counter_ctx: &mut CounterContext,
) -> Option<StyledNode> {
    let node = doc.get(node_id)?;

    match &node.data {
        NodeData::Document => {
            // Find the <html> child element
            for child in doc.children(node_id) {
                if let NodeData::Element(el) = &child.data
                    && el.tag_name.eq_ignore_ascii_case("html")
                {
                    return build_styled_node(
                        doc,
                        child.id,
                        author_index,
                        parent_style,
                        counter_ctx,
                    );
                }
            }
            // Fallback: search for any element child
            for child in doc.children(node_id) {
                if matches!(child.data, NodeData::Element(_)) {
                    return build_styled_node(
                        doc,
                        child.id,
                        author_index,
                        parent_style,
                        counter_ctx,
                    );
                }
            }
            None
        }

        NodeData::Element(el) => {
            if el.get_attribute("hidden").is_some() {
                return None;
            }

            // HTML5 Details disclosure: non-summary children of closed details are suppressed.
            // Under HTML §4.11.1, only the first <summary> child represents the disclosure summary;
            // any subsequent <summary> elements are normal children and suppressed when closed.
            if let Some(parent_id) = node.parent
                && let Some(parent_node) = doc.get(parent_id)
                && let NodeData::Element(parent_elem) = &parent_node.data
                && parent_elem.tag_name.eq_ignore_ascii_case("details")
                && parent_elem.get_attribute("open").is_none()
            {
                let is_first_summary = el.tag_name.eq_ignore_ascii_case("summary")
                    && doc.children(parent_id).find(|c| {
                        matches!(&c.data, NodeData::Element(e) if e.tag_name.eq_ignore_ascii_case("summary"))
                    }).map(|c| c.id) == Some(node_id);
                if !is_first_summary {
                    return None;
                }
            }

            if el.tag_name.eq_ignore_ascii_case("template") {
                return None;
            }

            let style = compute_style_with_index(node_id, doc, author_index, parent_style);
            if style.display == Display::None {
                return None;
            }

            // Track counter resets and increments on this element
            let mut created_counters: Vec<String> = Vec::new();
            for action in &style.counter_reset {
                counter_ctx.reset(&action.name, action.value);
                created_counters.push(action.name.clone());
            }
            for action in &style.counter_increment {
                counter_ctx.increment(&action.name, action.value);
            }

            let mut styled_children = Vec::new();

            // Synthesize ::before pseudo-element if declared with content
            if let Some(before_style) =
                compute_pseudo_style_with_index(node_id, doc, author_index, &style, "before")
                && before_style.display != Display::None
                && (before_style.content.is_some() || before_style.content_items.is_some())
            {
                for action in &before_style.counter_reset {
                    counter_ctx.reset(&action.name, action.value);
                }
                for action in &before_style.counter_increment {
                    counter_ctx.increment(&action.name, action.value);
                }
                if let Some(pseudo_node) =
                    create_pseudo_styled_node(node_id, doc, before_style, counter_ctx)
                {
                    styled_children.push(pseudo_node);
                }
            }

            // If the element has an attached shadow root (GAP-012/GAP-013),
            // render the shadow tree's children, projecting light DOM children into <slot> elements.
            let effective_children: Vec<mango_html::NodeId> = if let Some(sr_id) = el.shadow_root {
                let mut res = Vec::new();
                for sr_child in doc.children(sr_id) {
                    if let NodeData::Element(child_el) = &sr_child.data
                        && child_el.tag_name.eq_ignore_ascii_case("slot")
                    {
                        let light_children: Vec<mango_html::NodeId> =
                            doc.children(node_id).map(|c| c.id).collect();
                        if !light_children.is_empty() {
                            res.extend(light_children);
                        } else {
                            res.extend(doc.children(sr_child.id).map(|c| c.id));
                        }
                    } else {
                        res.push(sr_child.id);
                    }
                }
                res
            } else {
                doc.children(node_id).map(|c| c.id).collect()
            };

            for child_id in effective_children {
                if let Some(styled_child) =
                    build_styled_node(doc, child_id, author_index, Some(&style), counter_ctx)
                {
                    styled_children.push(styled_child);
                }
            }

            // Synthesize ::after pseudo-element if declared with content
            if let Some(after_style) =
                compute_pseudo_style_with_index(node_id, doc, author_index, &style, "after")
                && after_style.display != Display::None
                && (after_style.content.is_some() || after_style.content_items.is_some())
            {
                for action in &after_style.counter_reset {
                    counter_ctx.reset(&action.name, action.value);
                }
                for action in &after_style.counter_increment {
                    counter_ctx.increment(&action.name, action.value);
                }
                if let Some(pseudo_node) =
                    create_pseudo_styled_node(node_id, doc, after_style, counter_ctx)
                {
                    styled_children.push(pseudo_node);
                }
            }

            let mut styled_node = StyledNode::new(Some(node_id), style);
            // Capture tag name and attributes for downstream use (e.g. <img> detection)
            if let NodeData::Element(el) = &node.data {
                styled_node.tag_name = Some(el.tag_name.to_lowercase());
                styled_node.attributes = el
                    .attributes
                    .iter()
                    .map(|(name, value)| (name.to_lowercase(), value.clone()))
                    .collect();

                // Capture ::placeholder styling for form inputs
                if (el.tag_name.eq_ignore_ascii_case("input")
                    || el.tag_name.eq_ignore_ascii_case("textarea"))
                    && let Some(ph_style) = compute_pseudo_style_with_index(
                        node_id,
                        doc,
                        author_index,
                        &styled_node.style,
                        "placeholder",
                    )
                {
                    styled_node.attributes.push((
                        "_mango_placeholder_color".to_string(),
                        format!(
                            "rgba({},{},{},{})",
                            ph_style.color.r, ph_style.color.g, ph_style.color.b, ph_style.color.a
                        ),
                    ));
                }

                // Capture ::marker styling for list items
                if (el.tag_name.eq_ignore_ascii_case("li")
                    || styled_node.style.display == Display::ListItem)
                    && let Some(m_style) = compute_pseudo_style_with_index(
                        node_id,
                        doc,
                        author_index,
                        &styled_node.style,
                        "marker",
                    )
                {
                    styled_node.attributes.push((
                        "_mango_marker_color".to_string(),
                        format!(
                            "rgba({},{},{},{})",
                            m_style.color.r, m_style.color.g, m_style.color.b, m_style.color.a
                        ),
                    ));
                    if let Some(c) = &m_style.content {
                        styled_node
                            .attributes
                            .push(("_mango_marker_content".to_string(), c.clone()));
                    }
                }

                // Capture details disclosure state for summary elements
                if el.tag_name.eq_ignore_ascii_case("summary")
                    && let Some(parent_id) = node.parent
                    && let Some(parent_node) = doc.get(parent_id)
                    && let NodeData::Element(parent_elem) = &parent_node.data
                    && parent_elem.tag_name.eq_ignore_ascii_case("details")
                {
                    let is_open = parent_elem.get_attribute("open").is_some();
                    styled_node.attributes.push((
                        "_mango_details_open".to_string(),
                        if is_open {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        },
                    ));
                }

                // Capture parent <picture> <source> elements for responsive image resolution (GAP-019)
                if el.tag_name.eq_ignore_ascii_case("img")
                    && let Some(parent_id) = node.parent
                    && let Some(parent_node) = doc.get(parent_id)
                    && let NodeData::Element(parent_elem) = &parent_node.data
                    && parent_elem.tag_name.eq_ignore_ascii_case("picture")
                {
                    let mut sources_data = Vec::new();
                    for sib in doc.children(parent_id) {
                        if let NodeData::Element(sib_el) = &sib.data
                            && sib_el.tag_name.eq_ignore_ascii_case("source")
                        {
                            let srcset = sib_el.get_attribute("srcset").unwrap_or("").to_string();
                            let media = sib_el.get_attribute("media").unwrap_or("").to_string();
                            let mime = sib_el.get_attribute("type").unwrap_or("").to_string();
                            sources_data.push(format!("{srcset}\t{media}\t{mime}"));
                        }
                    }
                    if !sources_data.is_empty() {
                        styled_node.attributes.push((
                            "_mango_picture_sources".to_string(),
                            sources_data.join("\n"),
                        ));
                    }
                }
            }
            styled_node.children = styled_children;
            for name in created_counters {
                counter_ctx.pop(&name);
            }
            Some(styled_node)
        }

        NodeData::Text(raw_text) => {
            let parent_ws = parent_style
                .map(|s| s.white_space)
                .unwrap_or(mango_css::values::WhiteSpace::Normal);
            let preserves_all_ws = matches!(
                parent_ws,
                mango_css::values::WhiteSpace::Pre
                    | mango_css::values::WhiteSpace::PreWrap
                    | mango_css::values::WhiteSpace::BreakSpaces
            );
            let text_to_use = if preserves_all_ws {
                raw_text.clone()
            } else if parent_ws == mango_css::values::WhiteSpace::PreLine {
                let mut res = String::new();
                for (idx, line) in raw_text.split('\n').enumerate() {
                    if idx > 0 {
                        res.push('\n');
                    }
                    res.push_str(&collapse_whitespace(line));
                }
                res
            } else {
                collapse_whitespace(raw_text)
            };

            // In CSS, only ASCII whitespace (' ', '\t', '\n', '\r') collapses.
            // Non-breaking spaces (&nbsp; / U+00A0) and other Unicode spaces are content and must be preserved.
            let is_whitespace_only = text_to_use.chars().all(|c| c.is_ascii_whitespace());
            if !preserves_all_ws && is_whitespace_only {
                let parent_display = parent_style.map(|s| s.display);
                let is_non_inline_container = matches!(
                    parent_display,
                    Some(
                        Display::Flex
                            | Display::InlineFlex
                            | Display::Grid
                            | Display::InlineGrid
                            | Display::Table
                            | Display::TableRow
                            | Display::TableRowGroup
                            | Display::TableHeaderGroup
                            | Display::TableFooterGroup
                    )
                );
                let should_keep = if text_to_use.is_empty() || is_non_inline_container {
                    false
                } else if parent_style.map(|s| s.display == Display::Inline).unwrap_or(false) {
                    true
                } else if let Some(parent_id) = node.parent {
                    let siblings: Vec<_> = doc.children(parent_id).collect();
                    if let Some(pos) = siblings.iter().position(|c| c.id == node_id) {
                        let prev_meaningful = siblings[..pos].iter().rev().find(|s| match &s.data {
                            NodeData::Comment(_) => false,
                            NodeData::Element(el) => {
                                el.get_attribute("hidden").is_none()
                                    && !el.tag_name.eq_ignore_ascii_case("template")
                                    && !el.tag_name.eq_ignore_ascii_case("head")
                                    && !el.tag_name.eq_ignore_ascii_case("style")
                                    && !el.tag_name.eq_ignore_ascii_case("script")
                            }
                            _ => true,
                        });
                        let next_meaningful = siblings[pos + 1..].iter().find(|s| match &s.data {
                            NodeData::Comment(_) => false,
                            NodeData::Element(el) => {
                                el.get_attribute("hidden").is_none()
                                    && !el.tag_name.eq_ignore_ascii_case("template")
                                    && !el.tag_name.eq_ignore_ascii_case("head")
                                    && !el.tag_name.eq_ignore_ascii_case("style")
                                    && !el.tag_name.eq_ignore_ascii_case("script")
                            }
                            _ => true,
                        });
                        let prev_inline = prev_meaningful
                            .map_or(false, |s| is_node_inline(doc, s.id, author_index, parent_style));
                        let next_inline = next_meaningful
                            .map_or(false, |s| is_node_inline(doc, s.id, author_index, parent_style));
                        prev_inline && next_inline
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !should_keep {
                    return None;
                }
            }

            // Inherit parent style, forced to inline display
            let mut style = parent_style.cloned().unwrap_or_default();
            style.display = Display::Inline;
            // CSS rule: Raw text nodes do not have backgrounds, borders, or box-shadows
            style.background_image = None;
            style.background_color = mango_core::Color::TRANSPARENT;
            style.box_shadow = None;
            style.border_top_width = 0.0;
            style.border_right_width = 0.0;
            style.border_bottom_width = 0.0;
            style.border_left_width = 0.0;

            Some(StyledNode::with_text(Some(node_id), style, text_to_use))
        }

        // Fragments are transparent containers: inserting one into the tree moves its
        // children into the parent, so a fragment node should never reach layout.
        // If one is reached anyway, it is not rendered.
        NodeData::DocumentType { .. } | NodeData::Comment(_) | NodeData::DocumentFragment => None,
    }
}

enum PseudoPart {
    Text(String),
    Image(String),
}

fn create_pseudo_styled_node(
    host_node_id: NodeId,
    doc: &Document,
    mut style: ComputedStyle,
    counter_ctx: &mut CounterContext,
) -> Option<StyledNode> {
    let mut parts = Vec::new();
    let mut text_acc = String::new();
    let quotes = style.effective_quotes().to_vec();

    if let Some(items) = style.content_items.take() {
        for item in items {
            match item {
                ContentItem::String(s) => text_acc.push_str(&s),
                ContentItem::Attr(attr_name) => {
                    if let Some(host_node) = doc.get(host_node_id)
                        && let NodeData::Element(host_elem) = &host_node.data
                        && let Some(val) = host_elem.get_attribute(&attr_name)
                    {
                        text_acc.push_str(val);
                    }
                }
                ContentItem::Counter {
                    name,
                    style: cstyle,
                } => {
                    let val = counter_ctx.get(&name);
                    let style_name = cstyle.as_deref().unwrap_or("decimal");
                    let formatted = format_counter_value(val, style_name);
                    text_acc.push_str(&formatted);
                }
                ContentItem::Counters {
                    name,
                    separator,
                    style: cstyle,
                } => {
                    let vals = counter_ctx.get_all(&name);
                    let style_name = cstyle.as_deref().unwrap_or("decimal");
                    let formatted: Vec<String> = vals
                        .iter()
                        .map(|&v| format_counter_value(v, style_name))
                        .collect();
                    text_acc.push_str(&formatted.join(&separator));
                }
                ContentItem::OpenQuote => {
                    if !quotes.is_empty() {
                        let idx = counter_ctx.quote_depth.min(quotes.len() - 1);
                        text_acc.push_str(&quotes[idx].0);
                    }
                    counter_ctx.quote_depth += 1;
                }
                ContentItem::CloseQuote => {
                    if counter_ctx.quote_depth > 0 {
                        counter_ctx.quote_depth -= 1;
                    }
                    if !quotes.is_empty() {
                        let idx = counter_ctx.quote_depth.min(quotes.len() - 1);
                        text_acc.push_str(&quotes[idx].1);
                    }
                }
                ContentItem::NoOpenQuote => {
                    counter_ctx.quote_depth += 1;
                }
                ContentItem::NoCloseQuote => {
                    if counter_ctx.quote_depth > 0 {
                        counter_ctx.quote_depth -= 1;
                    }
                }
                ContentItem::Url(url) => {
                    if !text_acc.is_empty() {
                        parts.push(PseudoPart::Text(std::mem::take(&mut text_acc)));
                    }
                    parts.push(PseudoPart::Image(url));
                }
            }
        }
    } else if let Some(legacy_text) = style.content.take() {
        text_acc.push_str(&legacy_text);
    }

    if !text_acc.is_empty() {
        parts.push(PseudoPart::Text(text_acc));
    } else if parts.is_empty() {
        // Synthesize empty text for pseudo-elements with empty content (e.g. content: ""; clearfix)
        parts.push(PseudoPart::Text(String::new()));
    }

    if parts.is_empty() {
        return None;
    }

    if parts.len() == 1 {
        match parts.into_iter().next().unwrap() {
            PseudoPart::Text(text) => {
                if style.display == Display::Inline {
                    Some(StyledNode::with_text(None, style, text))
                } else {
                    let mut pseudo_node = StyledNode::new(None, style.clone());
                    let mut inline_style = style;
                    inline_style.display = Display::Inline;
                    pseudo_node
                        .children
                        .push(StyledNode::with_text(None, inline_style, text));
                    Some(pseudo_node)
                }
            }
            PseudoPart::Image(url) => {
                let mut img_style = style;
                if img_style.display == Display::Inline {
                    img_style.display = Display::InlineBlock;
                }
                let mut img_node = StyledNode::new(None, img_style);
                img_node.tag_name = Some("img".to_string());
                img_node.attributes = vec![("src".to_string(), url)];
                Some(img_node)
            }
        }
    } else {
        let mut pseudo_node = StyledNode::new(None, style.clone());
        for part in parts {
            match part {
                PseudoPart::Text(t) => {
                    let mut inline_style = style.clone();
                    inline_style.display = Display::Inline;
                    pseudo_node
                        .children
                        .push(StyledNode::with_text(None, inline_style, t));
                }
                PseudoPart::Image(url) => {
                    let mut img_style = style.clone();
                    if img_style.display == Display::Inline {
                        img_style.display = Display::InlineBlock;
                    }
                    let mut img_node = StyledNode::new(None, img_style);
                    img_node.tag_name = Some("img".to_string());
                    img_node.attributes = vec![("src".to_string(), url)];
                    pseudo_node.children.push(img_node);
                }
            }
        }
        Some(pseudo_node)
    }
}

/// Collapses runs of whitespace characters into a single space, per CSS white-space: normal.
pub fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_whitespace = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !in_whitespace {
                result.push(' ');
                in_whitespace = true;
            }
        } else {
            result.push(ch);
            in_whitespace = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Color;
    use mango_html::parse_html;

    #[test]
    fn test_build_style_tree_filters_head_and_style() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Test</title>
                    <style>h1 { color: #ff0000; }</style>
                </head>
                <body>
                    <h1>Hello Mango</h1>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");

        // The root should be <html>, child should be <body>, child of body is <h1>
        assert_eq!(style_tree.style.display, Display::Block);

        // Verify embedded <style> applied to <h1>
        let body = &style_tree.children[0];
        let h1 = &body.children[0];
        assert_eq!(h1.style.color, Color::RED);

        // Verify <h1> has text child "Hello Mango"
        assert_eq!(h1.children.len(), 1);
        assert_eq!(h1.children[0].text.as_deref(), Some("Hello Mango"));
    }

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(collapse_whitespace("hello   world\n\t!"), "hello world !");
        assert_eq!(collapse_whitespace("   "), " ");
    }

    #[test]
    fn test_pseudo_element_synthesis() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <style>
                        .quote::before { content: "“"; color: #0000ff; }
                        .quote::after { content: "”"; color: #ff0000; }
                    </style>
                </head>
                <body>
                    <p class="quote">Hello World</p>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
        let body = &style_tree.children[0];
        let p = &body.children[0];

        assert_eq!(p.children.len(), 3);
        assert_eq!(p.children[0].text.as_deref(), Some("“"));
        assert_eq!(p.children[0].style.color, Color::BLUE);
        assert_eq!(p.children[1].text.as_deref(), Some("Hello World"));
        assert_eq!(p.children[2].text.as_deref(), Some("”"));
        assert_eq!(p.children[2].style.color, Color::RED);
    }

    #[test]
    fn test_placeholder_and_marker_styling() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <style>
                        input::placeholder { color: #ff0000; }
                        li::marker { color: #00ff00; content: "» "; }
                    </style>
                </head>
                <body>
                    <input type="text" placeholder="Search..." />
                    <ul>
                        <li>Item 1</li>
                    </ul>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
        let body = &style_tree.children[0];
        let input = &body.children[0];
        let ul = &body.children[1];
        let li = &ul.children[0];

        let ph_color = input
            .attributes
            .iter()
            .find(|(k, _)| k == "_mango_placeholder_color");
        assert!(ph_color.is_some(), "Expected placeholder color attribute");
        assert!(ph_color.unwrap().1.contains("255,0,0"));

        let marker_color = li
            .attributes
            .iter()
            .find(|(k, _)| k == "_mango_marker_color");
        assert!(marker_color.is_some(), "Expected marker color attribute");
        assert!(marker_color.unwrap().1.contains("0,255,0"));

        let marker_content = li
            .attributes
            .iter()
            .find(|(k, _)| k == "_mango_marker_content");
        assert_eq!(marker_content.map(|(_, v)| v.as_str()), Some("» "));
    }

    #[test]
    fn test_template_suppression_and_shadow_dom_slot_projection() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <div id="host"><span>Light text</span></div>
                    <template id="tmpl"><span>Hidden template text</span></template>
                </body>
            </html>
        "#;
        let mut doc = parse_html(html);

        // 1. Verify template element is suppressed from the style tree
        let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
        let body = &style_tree.children[0];
        assert_eq!(
            body.children.len(),
            1,
            "Template element should not produce a styled node"
        );
        assert_eq!(
            body.children[0]
                .attributes
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.as_str()),
            Some("host")
        );

        // 2. Attach a shadow root containing a slot to the host element
        let host_id = doc
            .find_element_by_id(doc.root(), "host")
            .expect("host element exists");
        let shadow_root = doc.attach_shadow(host_id).expect("shadow root attached");

        // Add a slot inside the shadow root
        let slot = doc.create_element("slot", vec![]);
        doc.append_child(shadow_root, slot);

        // Build the style tree again — light DOM children should now be projected through the slot in shadow DOM!
        let tree_with_shadow = build_style_tree(&doc, &[]).expect("style tree exists");
        let body_shadow = &tree_with_shadow.children[0];
        let host_node = &body_shadow.children[0];
        assert_eq!(host_node.children.len(), 1);
        assert_eq!(
            host_node.children[0].children[0].text.as_deref(),
            Some("Light text")
        );
    }

    #[test]
    fn test_incremental_style_invalidation() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <style>
                        .active { color: #ff0000; }
                        .hidden { display: none; }
                        .muted { opacity: 0.5; }
                    </style>
                </head>
                <body>
                    <div id="c1" class="muted">Card 1</div>
                    <div id="c2">Card 2</div>
                    <div id="c3">Card 3</div>
                </body>
            </html>
        "#;
        let mut doc = parse_html(html);
        let mut style_tree = build_style_tree(&doc, &[]).expect("style tree exists");

        let c2_id = doc
            .find_element_by_id(doc.root(), "c2")
            .expect("c2 element exists");

        // Mutate only c2 by adding the .active class
        doc.add_class(c2_id, "active");
        assert!(doc.has_dirty_nodes());
        assert_eq!(doc.dirty_nodes().len(), 1);

        // Run incremental invalidation
        let mut invalidator = StyleInvalidator::new();
        invalidator.mark_all_dirty(doc.take_dirty_nodes());

        let recalc_count = invalidator.invalidate_and_update(&mut style_tree, &doc, &[]);
        // Only 2 nodes recalculated: c2 and its text child (c1 and c3 subtrees completely skipped!)
        assert_eq!(recalc_count, 2);

        // Verify c2 now has the active color red (rgb(255, 0, 0))
        let body = &style_tree.children[0];
        let c2_styled = &body.children[1];
        assert_eq!(c2_styled.style.color, mango_core::Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_generated_content_and_counters() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <style>
                        body {
                            counter-reset: chapter 0;
                        }
                        .chap {
                            counter-increment: chapter 1;
                        }
                        .chap::before {
                            content: "Chapter " counter(chapter, upper-roman) ": " attr(data-title) " ";
                        }
                        .quote {
                            quotes: "«" "»" "“" "”";
                        }
                        .quote::before {
                            content: open-quote;
                        }
                        .quote::after {
                            content: close-quote;
                        }
                        .icon::after {
                            content: url("icon.png");
                        }
                    </style>
                </head>
                <body>
                    <div id="c1" class="chap" data-title="Introduction">Content 1</div>
                    <div id="c2" class="chap" data-title="Deep Dive">
                        <span id="q1" class="quote">Quote 1</span>
                    </div>
                    <div id="btn" class="icon">Button</div>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
        let body = &style_tree.children[0];

        // 1. Check chapter 1: "Chapter I: Introduction "
        let c1 = &body.children[0];
        assert_eq!(c1.children.len(), 2);
        assert_eq!(
            c1.children[0].text.as_deref(),
            Some("Chapter I: Introduction ")
        );
        assert_eq!(c1.children[1].text.as_deref(), Some("Content 1"));

        // 2. Check chapter 2: "Chapter II: Deep Dive "
        let c2 = &body.children[1];
        assert_eq!(
            c2.children[0].text.as_deref(),
            Some("Chapter II: Deep Dive ")
        );

        // 3. Check quotes inside chapter 2
        let q1 = &c2.children[1];
        // q1 has ::before ("«"), "Quote 1", ::after ("»")
        assert_eq!(q1.children.len(), 3);
        assert_eq!(q1.children[0].text.as_deref(), Some("«"));
        assert_eq!(q1.children[1].text.as_deref(), Some("Quote 1"));
        assert_eq!(q1.children[2].text.as_deref(), Some("»"));

        // 4. Check image generated content
        let btn = &body.children[2];
        // btn has "Button" and ::after (image node)
        assert_eq!(btn.children.len(), 2);
        assert_eq!(btn.children[0].text.as_deref(), Some("Button"));
        let img_node = &btn.children[1];
        assert_eq!(img_node.tag_name.as_deref(), Some("img"));
        assert!(
            img_node
                .attributes
                .iter()
                .any(|(k, v)| k == "src" && v == "icon.png")
        );
    }

    #[test]
    fn test_whitespace_preserved_between_inline_elements() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <p><span>Hello</span> <span>World</span></p>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let style_tree = build_style_tree(&doc, &[]).expect("style tree exists");
        let body = &style_tree.children[0];
        let p = &body.children[0];

        // Should have 3 children: <span>Hello</span>, " ", <span>World</span>
        assert_eq!(p.children.len(), 3, "Inter-inline whitespace must be preserved");
        assert_eq!(p.children[0].children[0].text.as_deref(), Some("Hello"));
        assert_eq!(p.children[1].text.as_deref(), Some(" "));
        assert_eq!(p.children[2].children[0].text.as_deref(), Some("World"));
    }

    #[test]
    fn test_details_multiple_summaries_closed() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <details>
                        <summary>Summary 1</summary>
                        <summary>Summary 2</summary>
                        <p>Hidden body</p>
                    </details>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let style_tree = build_style_tree(&doc, &[]).expect("style tree exists");
        let body = &style_tree.children[0];
        let details = &body.children[0];

        // When closed, only the FIRST summary is shown
        assert_eq!(details.children.len(), 1);
        assert_eq!(details.children[0].children[0].text.as_deref(), Some("Summary 1"));
    }

    #[test]
    fn test_format_counter_leading_zero_and_none() {
        assert_eq!(format_counter_value(5, "decimal-leading-zero"), "05");
        assert_eq!(format_counter_value(12, "decimal-leading-zero"), "12");
        assert_eq!(format_counter_value(5, "none"), "");
    }
}

use std::collections::HashSet;

/// Incremental style invalidator tracking dirty DOM nodes (ARCH-002).
///
/// Instead of recalculating style for all $N$ nodes in the document on every change,
/// `StyleInvalidator` tracks which DOM nodes mutated (via attribute changes or class toggles)
/// and recalculates only those subtrees in $O(\text{changed\_nodes})$.
#[derive(Debug, Clone, Default)]
pub struct StyleInvalidator {
    dirty_nodes: HashSet<NodeId>,
}

impl StyleInvalidator {
    /// Creates a new empty `StyleInvalidator`.
    pub fn new() -> Self {
        Self {
            dirty_nodes: HashSet::new(),
        }
    }

    /// Marks a specific DOM node as dirty.
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        self.dirty_nodes.insert(node_id);
    }

    /// Marks a collection of DOM nodes as dirty.
    pub fn mark_all_dirty(&mut self, nodes: impl IntoIterator<Item = NodeId>) {
        self.dirty_nodes.extend(nodes);
    }

    /// Checks if a node is marked dirty.
    pub fn is_dirty(&self, node_id: NodeId) -> bool {
        self.dirty_nodes.contains(&node_id)
    }

    /// Clears all dirty flags.
    pub fn clear(&mut self) {
        self.dirty_nodes.clear();
    }

    /// Returns the number of currently tracked dirty nodes.
    pub fn len(&self) -> usize {
        self.dirty_nodes.len()
    }

    /// Returns `true` if there are no dirty nodes.
    pub fn is_empty(&self) -> bool {
        self.dirty_nodes.is_empty()
    }

    /// Updates only the dirty nodes in an existing style tree in-place (ARCH-002).
    ///
    /// Returns the number of nodes whose styles were recalculated.
    pub fn invalidate_and_update(
        &mut self,
        root: &mut StyledNode,
        doc: &Document,
        author_styles: &[&Stylesheet],
    ) -> usize {
        if self.dirty_nodes.is_empty() {
            return 0;
        }

        let embedded_sheets = extract_style_elements(doc);
        let mut all_author_sheets: Vec<&Stylesheet> = Vec::new();
        all_author_sheets.extend(author_styles);
        for sheet in &embedded_sheets {
            all_author_sheets.push(sheet);
        }

        let mut order = 1000;
        let author_index =
            RuleIndex::from_stylesheets(&all_author_sheets, Origin::Author, &mut order);

        let recalculated =
            update_styled_subtree(root, doc, &self.dirty_nodes, &author_index, None, false);

        self.clear();
        recalculated
    }
}

fn has_inherited_property_changed(old: &ComputedStyle, new: &ComputedStyle) -> bool {
    old.color != new.color
        || old.font_size != new.font_size
        || old.font_family != new.font_family
        || old.font_weight != new.font_weight
        || old.font_style != new.font_style
        || old.line_height != new.line_height
        || old.text_align != new.text_align
        || old.text_transform != new.text_transform
        || old.letter_spacing != new.letter_spacing
        || old.word_spacing != new.word_spacing
        || old.text_indent != new.text_indent
        || old.white_space != new.white_space
        || old.visibility != new.visibility
        || old.direction != new.direction
        || old.writing_mode != new.writing_mode
        || old.cursor != new.cursor
        || old.word_break != new.word_break
        || old.overflow_wrap != new.overflow_wrap
        || old.hyphens != new.hyphens
        || old.list_style_type != new.list_style_type
        || old.list_style_position != new.list_style_position
        || old.border_collapse != new.border_collapse
        || old.border_spacing != new.border_spacing
}

fn update_styled_subtree(
    node: &mut StyledNode,
    doc: &Document,
    dirty_nodes: &HashSet<NodeId>,
    author_index: &RuleIndex,
    parent_style: Option<&ComputedStyle>,
    parent_inherited_changed: bool,
) -> usize {
    let mut recalculated_count = 0;
    let mut this_node_changed_inherited = false;

    let is_directly_dirty = node.node_id.is_some_and(|nid| dirty_nodes.contains(&nid));
    let needs_recalc = is_directly_dirty || parent_inherited_changed;

    if needs_recalc
        && let Some(nid) = node.node_id
        && let Some(dom_node) = doc.get(nid)
    {
        match &dom_node.data {
            NodeData::Element(elem) => {
                let old_style = node.style.clone();
                node.style = compute_style_with_index(nid, doc, author_index, parent_style);
                node.attributes = elem
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.to_lowercase(), v.clone()))
                    .collect();

                // Check if any inherited property changed
                if has_inherited_property_changed(&old_style, &node.style) {
                    this_node_changed_inherited = true;
                }
                recalculated_count += 1;
            }
            NodeData::Text(_) => {
                if let Some(ps) = parent_style {
                    let mut style = ps.clone();
                    style.display = Display::Inline;
                    style.background_image = None;
                    style.background_color = mango_core::Color::TRANSPARENT;
                    style.box_shadow = None;
                    style.border_top_width = 0.0;
                    style.border_right_width = 0.0;
                    style.border_bottom_width = 0.0;
                    style.border_left_width = 0.0;
                    node.style = style;
                    recalculated_count += 1;
                }
            }
            _ => {}
        }
    }

    let propagate_to_children = this_node_changed_inherited;
    let current_style = node.style.clone();

    for child in &mut node.children {
        recalculated_count += update_styled_subtree(
            child,
            doc,
            dirty_nodes,
            author_index,
            Some(&current_style),
            propagate_to_children,
        );
    }

    recalculated_count
}
