//! Cascade priority resolution and style aggregation.

use std::collections::HashMap;

use mango_html::dom::{Document, NodeData, NodeId};

use crate::parser::{Stylesheet, parse_declaration_list};
use crate::specificity::Specificity;
use crate::values::Value;

use super::hints::collect_presentational_hints;
use super::rule_index::RuleIndex;
use super::types::{MatchedDeclaration, Origin};
use super::user_agent::user_agent_stylesheet;

/// The cached global User-Agent RuleIndex.
pub fn user_agent_rule_index() -> &'static RuleIndex<'static> {
    static UA_RULE_INDEX: std::sync::OnceLock<RuleIndex<'static>> = std::sync::OnceLock::new();
    UA_RULE_INDEX.get_or_init(|| {
        let sheet = user_agent_stylesheet();
        let mut order = 0;
        let mut index = RuleIndex::new();
        index.add_stylesheet(sheet, Origin::UserAgent, &mut order);
        index
    })
}

/// Resolves all specified property values for a given DOM node by applying
/// user-agent styles, optional shadow host styles (via `shadow_index`),
/// author stylesheets (via `RuleIndex`), inline styles, and cascade priority sorting.
pub fn resolve_cascade_with_shadow_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
) -> HashMap<String, Value> {
    let mut matched_decls: Vec<MatchedDeclaration> = Vec::new();
    let mut source_order = 0;

    let Some(node) = doc.get(node_id) else {
        return HashMap::new();
    };

    if let NodeData::Element(elem) = &node.data {
        // 1. User Agent Stylesheet (via cached UA index)
        user_agent_rule_index().collect_matches_for_node(
            node_id,
            doc,
            elem,
            &mut source_order,
            &mut matched_decls,
        );

        // 1.5 Presentational hints from HTML attributes
        collect_presentational_hints(node_id, doc, elem, &mut source_order, &mut matched_decls);

        // 1.8 Shadow Root :host & ::slotted styles
        if let Some(s_index) = shadow_index {
            let host_id = if elem.shadow_root.is_some() {
                Some(node_id)
            } else {
                node.parent
            };
            crate::selectors::set_current_host_element(host_id);
            s_index.collect_matches_for_node(
                node_id,
                doc,
                elem,
                &mut source_order,
                &mut matched_decls,
            );
            crate::selectors::set_current_host_element(None);
        }

        // 2. Author Stylesheets (via author index)
        author_index.collect_matches_for_node(
            node_id,
            doc,
            elem,
            &mut source_order,
            &mut matched_decls,
        );

        // 3. Inline style="..." attribute on the element itself
        if let Some(style_attr) = elem.get_attribute("style") {
            let inline_decls = parse_declaration_list(style_attr);
            for decl in inline_decls {
                if decl.is_shorthand() {
                    for expanded in decl.expand_shorthand() {
                        source_order += 1;
                        matched_decls.push(MatchedDeclaration {
                            declaration: expanded,
                            specificity: Specificity(1, 0, 0),
                            origin: Origin::Inline,
                            source_order,
                            layer_order: None,
                        });
                    }
                } else {
                    source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: decl,
                        specificity: Specificity(1, 0, 0),
                        origin: Origin::Inline,
                        source_order,
                        layer_order: None,
                    });
                }
            }
        }
    }

    matched_decls.sort_by_key(|a| a.cascade_weight());
    let mut resolved = HashMap::new();
    for matched in matched_decls {
        resolved.insert(matched.declaration.name, matched.declaration.value);
    }
    resolved
}

/// Resolves all specified property values for a given DOM node by applying
/// user-agent styles, author stylesheets (via `RuleIndex`), inline styles, and cascade priority sorting.
pub fn resolve_cascade_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
) -> HashMap<String, Value> {
    resolve_cascade_with_shadow_index(node_id, doc, author_index, None)
}

/// Resolves pseudo-element cascade matching using the indexed author stylesheet rules and optional shadow index.
pub fn resolve_pseudo_element_cascade_with_shadow_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
    pseudo: &str,
) -> HashMap<String, Value> {
    let mut matched_decls: Vec<MatchedDeclaration> = Vec::new();
    let mut source_order = 0;

    if let Some(s_index) = shadow_index {
        let is_host = doc.get(node_id).is_some_and(|n| {
            if let NodeData::Element(el) = &n.data {
                el.shadow_root.is_some()
            } else {
                false
            }
        });
        let host_id = if is_host {
            Some(node_id)
        } else {
            doc.get(node_id).and_then(|n| n.parent)
        };
        crate::selectors::set_current_host_element(host_id);
        s_index.collect_pseudo_matches_for_node(
            node_id,
            doc,
            pseudo,
            &mut source_order,
            &mut matched_decls,
        );
        crate::selectors::set_current_host_element(None);
    }

    author_index.collect_pseudo_matches_for_node(
        node_id,
        doc,
        pseudo,
        &mut source_order,
        &mut matched_decls,
    );

    matched_decls.sort_by_key(|a| a.cascade_weight());
    let mut resolved = HashMap::new();
    for matched in matched_decls {
        resolved.insert(matched.declaration.name, matched.declaration.value);
    }
    resolved
}

/// Resolves pseudo-element cascade matching using the indexed author stylesheet rules.
pub fn resolve_pseudo_element_cascade_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    pseudo: &str,
) -> HashMap<String, Value> {
    resolve_pseudo_element_cascade_with_shadow_index(node_id, doc, author_index, None, pseudo)
}

/// Resolves all specified property values for a given DOM node by applying
/// user-agent styles, author stylesheets, inline styles, and cascade priority sorting.
pub fn resolve_cascade(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
) -> HashMap<String, Value> {
    let mut order = 1000;
    let author_index = RuleIndex::from_stylesheets(author_stylesheets, Origin::Author, &mut order);
    resolve_cascade_with_index(node_id, doc, &author_index)
}

/// Resolves all specified property values for a given DOM node's pseudo-element (`before`, `after`, etc.)
/// by applying author stylesheets matching that pseudo-element.
pub fn resolve_pseudo_element_cascade(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
    pseudo: &str,
) -> HashMap<String, Value> {
    let mut order = 1000;
    let author_index = RuleIndex::from_stylesheets(author_stylesheets, Origin::Author, &mut order);
    resolve_pseudo_element_cascade_with_index(node_id, doc, &author_index, pseudo)
}

/// Helper function to find container size for a node using a default or custom RuleIndex.
pub fn find_container_size_for_node(
    node_id: NodeId,
    doc: &Document,
    name_filter: Option<&str>,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(f32, f32)> {
    let index = RuleIndex::new_with_size(viewport_width, viewport_height);
    index.find_container_size_for_node(node_id, doc, name_filter)
}
