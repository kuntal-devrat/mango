use std::collections::{HashMap, HashSet};

use mango_css::computed::{
    ComputedStyle, compute_pseudo_style_with_index, compute_pseudo_style_with_shadow_index,
    compute_style_with_index, compute_style_with_shadow_index,
};
use mango_css::parser::Stylesheet;
use mango_css::values::Display;
use mango_css::{Origin, RuleIndex};
use mango_html::dom::{Document, NodeData, NodeId};

use super::counters::CounterContext;
use super::extract::{
    ShadowContext, compute_slot_assignments, extract_shadow_stylesheets, extract_style_elements,
    scan_all_shadow_roots, scan_and_register_global_svg_symbols, scan_and_register_image_maps,
};
use super::node::StyledNode;
use super::pseudo::{collapse_whitespace, create_pseudo_styled_node};

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
    let mut env = mango_css::current_media_environment();
    env.viewport_width = viewport_width;
    env.viewport_height = viewport_height;
    build_style_tree_with_env(doc, author_styles, &env)
}

/// Builds the styled tree from a DOM [`Document`] with an explicit [`MediaEnvironment`].
pub fn build_style_tree_with_env(
    doc: &Document,
    author_styles: &[&Stylesheet],
    env: &mango_css::MediaEnvironment,
) -> Option<StyledNode> {
    mango_css::set_current_viewport(env.viewport_width, env.viewport_height);
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
    let author_index = RuleIndex::from_stylesheets_with_env(
        &all_author_sheets,
        Origin::Author,
        &mut order,
        env.clone(),
    );

    // 2.5 Extract stylesheets and build RuleIndexes for all shadow roots
    let all_shadow_roots = scan_all_shadow_roots(doc);
    let mut shadow_sheets: HashMap<NodeId, Vec<Stylesheet>> = HashMap::new();
    for &sr_id in all_shadow_roots.keys() {
        shadow_sheets.insert(sr_id, extract_shadow_stylesheets(doc, sr_id));
    }
    let mut shadow_indexes: HashMap<NodeId, RuleIndex> = HashMap::new();
    for (&sr_id, sheets) in &shadow_sheets {
        let refs: Vec<&Stylesheet> = sheets.iter().collect();
        let mut sr_order = 1000;
        let idx =
            RuleIndex::from_stylesheets_with_env(&refs, Origin::Author, &mut sr_order, env.clone());
        shadow_indexes.insert(sr_id, idx);
    }

    // 3. Recursively build style tree from root
    let mut counter_ctx = CounterContext::new();
    let mut visited = HashSet::new();
    build_styled_node(
        doc,
        doc.root(),
        &author_index,
        &shadow_indexes,
        None,
        None,
        &mut counter_ctx,
        &mut visited,
        0,
    )
}

pub fn is_node_inline(
    doc: &Document,
    node_id: NodeId,
    rule_index: &RuleIndex,
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
                || el.tag_name.eq_ignore_ascii_case("link")
                || el.tag_name.eq_ignore_ascii_case("meta")
            {
                return false;
            }
            let style = compute_style_with_index(node_id, doc, rule_index, parent_style);
            style.display.is_inline_level()
        }
        NodeData::Text(t) => !t.chars().all(|c| c.is_ascii_whitespace()),
        _ => false,
    }
}

pub fn build_styled_node<'a>(
    doc: &Document,
    node_id: NodeId,
    author_index: &'a RuleIndex<'a>,
    shadow_indexes: &'a HashMap<NodeId, RuleIndex<'a>>,
    shadow_ctx: Option<&ShadowContext<'a>>,
    parent_style: Option<&ComputedStyle>,
    counter_ctx: &mut CounterContext,
    visited: &mut HashSet<NodeId>,
    depth: usize,
) -> Option<StyledNode> {
    if depth >= mango_html::dom::MAX_DOM_DEPTH || !visited.insert(node_id) {
        return None;
    }
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
                        shadow_indexes,
                        shadow_ctx,
                        parent_style,
                        counter_ctx,
                        visited,
                        depth + 1,
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
                        shadow_indexes,
                        shadow_ctx,
                        parent_style,
                        counter_ctx,
                        visited,
                        depth + 1,
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

            let effective_index = if let Some(ctx) = shadow_ctx {
                ctx.rule_index
            } else {
                author_index
            };

            let host_shadow_index = el
                .shadow_root
                .and_then(|sr_id| shadow_indexes.get(&sr_id))
                .or_else(|| {
                    node.parent.and_then(|pid| doc.get(pid)).and_then(|pn| {
                        if let NodeData::Element(pe) = &pn.data {
                            pe.shadow_root.and_then(|sr_id| shadow_indexes.get(&sr_id))
                        } else {
                            None
                        }
                    })
                });

            let style = compute_style_with_shadow_index(
                node_id,
                doc,
                effective_index,
                host_shadow_index,
                parent_style,
            );
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
            let before_opt = compute_pseudo_style_with_shadow_index(
                node_id,
                doc,
                effective_index,
                host_shadow_index,
                &style,
                "before",
            );
            if let Some(before_style) = before_opt
                && before_style.display != Display::None
                && (before_style.content.is_some()
                    || before_style
                        .content_items
                        .as_ref()
                        .is_some_and(|items| !items.is_empty()))
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

            // Layout effective children according to Shadow DOM V1 tree projection
            let active_ctx = if let Some(sr_id) = el.shadow_root {
                let slot_assignments = compute_slot_assignments(doc, node_id, sr_id);
                let sr_rule_index = host_shadow_index.unwrap_or(author_index);
                Some(ShadowContext {
                    shadow_root: sr_id,
                    host_id: node_id,
                    rule_index: sr_rule_index,
                    slot_assignments,
                })
            } else {
                None
            };

            let raw_children: Vec<NodeId> = if let Some(ref act_ctx) = active_ctx {
                doc.children(act_ctx.shadow_root).map(|c| c.id).collect()
            } else {
                doc.children(node_id).map(|c| c.id).collect()
            };

            let current_ctx = active_ctx.as_ref().or(shadow_ctx);

            for child_id in raw_children {
                let is_slot = doc.get(child_id).is_some_and(|n| {
                    if let NodeData::Element(e) = &n.data {
                        e.tag_name.eq_ignore_ascii_case("slot")
                    } else {
                        false
                    }
                });

                if is_slot {
                    if let Some(ctx) = current_ctx
                        && let Some(assigned) = ctx.slot_assignments.get(&child_id)
                        && !assigned.is_empty()
                    {
                        for &assigned_id in assigned {
                            if let Some(styled_child) = build_styled_node(
                                doc,
                                assigned_id,
                                author_index,
                                shadow_indexes,
                                shadow_ctx,
                                Some(&style),
                                counter_ctx,
                                visited,
                                depth + 1,
                            ) {
                                styled_children.push(styled_child);
                            }
                        }
                    } else {
                        // Slot fallback content
                        for fallback in doc.children(child_id) {
                            if let Some(styled_child) = build_styled_node(
                                doc,
                                fallback.id,
                                author_index,
                                shadow_indexes,
                                current_ctx,
                                Some(&style),
                                counter_ctx,
                                visited,
                                depth + 1,
                            ) {
                                styled_children.push(styled_child);
                            }
                        }
                    }
                } else if let Some(styled_child) = build_styled_node(
                    doc,
                    child_id,
                    author_index,
                    shadow_indexes,
                    current_ctx,
                    Some(&style),
                    counter_ctx,
                    visited,
                    depth + 1,
                ) {
                    styled_children.push(styled_child);
                }
            }

            // Synthesize ::after pseudo-element if declared with content
            let after_opt = compute_pseudo_style_with_shadow_index(
                node_id,
                doc,
                effective_index,
                host_shadow_index,
                &style,
                "after",
            );
            if let Some(after_style) = after_opt
                && after_style.display != Display::None
                && (after_style.content.is_some()
                    || after_style
                        .content_items
                        .as_ref()
                        .is_some_and(|items| !items.is_empty()))
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
                styled_node.canvas_key = doc.canvas_key(node_id);
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
                        effective_index,
                        &styled_node.style,
                        "placeholder",
                    )
                {
                    let effective_alpha = ((ph_style.color.a as f32) * ph_style.opacity)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    styled_node.attributes.push((
                        "_mango_placeholder_color".to_string(),
                        format!(
                            "rgba({},{},{},{})",
                            ph_style.color.r, ph_style.color.g, ph_style.color.b, effective_alpha
                        ),
                    ));
                    styled_node.attributes.push((
                        "_mango_placeholder_opacity".to_string(),
                        ph_style.opacity.to_string(),
                    ));
                }

                // Capture ::marker styling for list items
                if (el.tag_name.eq_ignore_ascii_case("li")
                    || styled_node.style.display == Display::ListItem)
                    && let Some(m_style) = compute_pseudo_style_with_index(
                        node_id,
                        doc,
                        effective_index,
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

                // Capture parent <picture> <source> elements for responsive image resolution
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
                } else if parent_style
                    .map(|s| s.display == Display::Inline)
                    .unwrap_or(false)
                {
                    true
                } else if let Some(parent_id) = node.parent {
                    let siblings: Vec<_> = doc.children(parent_id).collect();
                    if let Some(pos) = siblings.iter().position(|c| c.id == node_id) {
                        let has_preceding_whitespace = siblings[..pos]
                            .iter()
                            .rev()
                            .take_while(|s| match &s.data {
                                NodeData::Comment(_) => true,
                                NodeData::Text(t) => t.chars().all(|c| c.is_ascii_whitespace()),
                                _ => false,
                            })
                            .any(|s| match &s.data {
                                NodeData::Text(t) => t.chars().all(|c| c.is_ascii_whitespace()),
                                _ => false,
                            });

                        if has_preceding_whitespace {
                            false
                        } else {
                            let is_meaningful = |s: &&&mango_html::dom::Node| match &s.data {
                                NodeData::Comment(_) => false,
                                NodeData::Text(t) => !t.chars().all(|c| c.is_ascii_whitespace()),
                                NodeData::Element(el) => {
                                    el.get_attribute("hidden").is_none()
                                        && !el.tag_name.eq_ignore_ascii_case("template")
                                        && !el.tag_name.eq_ignore_ascii_case("head")
                                        && !el.tag_name.eq_ignore_ascii_case("style")
                                        && !el.tag_name.eq_ignore_ascii_case("script")
                                        && !el.tag_name.eq_ignore_ascii_case("link")
                                        && !el.tag_name.eq_ignore_ascii_case("meta")
                                }
                                _ => true,
                            };
                            let prev_meaningful = siblings[..pos].iter().rev().find(is_meaningful);
                            let next_meaningful = siblings[pos + 1..].iter().find(is_meaningful);
                            let effective_index = if let Some(ctx) = shadow_ctx {
                                ctx.rule_index
                            } else {
                                author_index
                            };
                            let prev_inline = prev_meaningful.is_some_and(|s| {
                                is_node_inline(doc, s.id, effective_index, parent_style)
                            });
                            let next_inline = next_meaningful.is_some_and(|s| {
                                is_node_inline(doc, s.id, effective_index, parent_style)
                            });
                            prev_inline && next_inline
                        }
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

        NodeData::DocumentType { .. } | NodeData::Comment(_) | NodeData::DocumentFragment => None,
    }
}
