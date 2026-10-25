//! Cascade priority resolution and style aggregation.

use std::collections::HashMap;

use mango_html::dom::{Document, NodeData, NodeId};

use crate::parser::{Stylesheet, parse_declaration_list};
use crate::specificity::Specificity;
use crate::values::Value;

use super::hints::collect_presentational_hints;
use super::rule_index::RuleIndex;
use super::types::{MatchedDeclaration, Origin, compare_layer_paths};
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
    resolve_cascade_with_shadow_index_details(node_id, doc, author_index, shadow_index).0
}

/// Resolves all specified property values along with the set of properties whose
/// winning declarations were marked `!important`.
pub fn resolve_cascade_with_shadow_index_details(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
) -> (HashMap<String, Value>, std::collections::HashSet<String>) {
    resolve_cascade_with_shadow_and_part_index_details(
        node_id,
        doc,
        author_index,
        shadow_index,
        None,
    )
}

/// Resolves all specified property values with author index, optional shadow index, and optional outer part index.
pub fn resolve_cascade_with_shadow_and_part_index_details(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
    part_index: Option<&RuleIndex>,
) -> (HashMap<String, Value>, std::collections::HashSet<String>) {
    let part_indexes = match part_index {
        Some(pi) => vec![pi],
        None => Vec::new(),
    };
    resolve_cascade_with_shadow_and_part_indexes_details(
        node_id,
        doc,
        author_index,
        shadow_index,
        &part_indexes,
    )
}

/// Resolves all specified property values with author index, optional shadow index, and multiple outer part indexes.
pub fn resolve_cascade_with_shadow_and_part_indexes_details(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
    part_indexes: &[&RuleIndex],
) -> (HashMap<String, Value>, std::collections::HashSet<String>) {
    let mut matched_decls: Vec<MatchedDeclaration> = Vec::new();
    let mut source_order = 0;

    let Some(node) = doc.get(node_id) else {
        return (HashMap::new(), std::collections::HashSet::new());
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
            } else if let Some(sr_id) = doc.get_enclosing_shadow_root(node_id) {
                doc.get_host_for_shadow_root(sr_id)
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

        // 2.5 Outer part rules (CSS Shadow Parts)
        for p_index in part_indexes {
            p_index.collect_part_matches_for_node(
                node_id,
                doc,
                &mut source_order,
                &mut matched_decls,
            );
        }

        // 3. Inline style="..." attribute on the element itself
        if let Some(style_attr) = elem.get_attribute("style") {
            let inline_decls = parse_declaration_list(style_attr);
            let inline_rule_id = source_order + 10_000_000;
            for decl in inline_decls {
                if decl.is_shorthand() {
                    for expanded in decl.expand_shorthand() {
                        source_order += 1;
                        matched_decls.push(MatchedDeclaration {
                            declaration: expanded,
                            specificity: Specificity(1, 0, 0),
                            origin: Origin::Inline,
                            source_order,
                            layer_path: std::sync::Arc::from([]),
                            rule_id: inline_rule_id,
                            scope_proximity: None,
                        });
                    }
                } else {
                    source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: decl,
                        specificity: Specificity(1, 0, 0),
                        origin: Origin::Inline,
                        source_order,
                        layer_path: std::sync::Arc::from([]),
                        rule_id: inline_rule_id,
                        scope_proximity: None,
                    });
                }
            }
        }
    }

    matched_decls.sort_by(|a, b| a.compare_cascade(b));
    resolve_matched_declarations_with_importance(&matched_decls)
}

fn resolve_matched_declarations(matched_decls: &[MatchedDeclaration]) -> HashMap<String, Value> {
    resolve_matched_declarations_with_importance(matched_decls).0
}

fn canonical_property_name(name: &str) -> &str {
    match name {
        "margin-block-start" => "margin-top",
        "margin-block-end" => "margin-bottom",
        "margin-inline-start" => "margin-left",
        "margin-inline-end" => "margin-right",
        "padding-block-start" => "padding-top",
        "padding-block-end" => "padding-bottom",
        "padding-inline-start" => "padding-left",
        "padding-inline-end" => "padding-right",
        "inset-block-start" => "top",
        "inset-block-end" => "bottom",
        "inset-inline-start" => "left",
        "inset-inline-end" => "right",
        other => other,
    }
}

fn resolve_matched_declarations_with_importance(
    matched_decls: &[MatchedDeclaration],
) -> (HashMap<String, Value>, std::collections::HashSet<String>) {
    let mut decls_by_prop: HashMap<String, Vec<&MatchedDeclaration>> = HashMap::new();
    for matched in matched_decls {
        let prop = canonical_property_name(&matched.declaration.name);
        decls_by_prop
            .entry(prop.to_string())
            .or_default()
            .push(matched);
    }

    let mut resolved = HashMap::new();
    let mut important_props = std::collections::HashSet::new();
    for (prop_name, decls) in decls_by_prop {
        let mut chosen_val = None;
        let mut is_important = false;
        let mut idx = decls.len();
        let mut skipping_origins: std::collections::HashSet<Origin> =
            std::collections::HashSet::new();
        let mut skipping_layers: Vec<(Origin, std::sync::Arc<[u32]>)> = Vec::new();
        let mut skipping_rule_id: Option<usize> = None;
        let mut reverted_from_important = false;

        while idx > 0 {
            idx -= 1;
            let decl = decls[idx];

            if let Some(target_rule) = skipping_rule_id {
                if decl.rule_id == target_rule {
                    continue;
                }
                skipping_rule_id = None;
            }

            if skipping_origins.contains(&decl.origin) {
                continue;
            }
            if skipping_layers.iter().any(|(orig, layer)| {
                *orig == decl.origin
                    && compare_layer_paths(&decl.layer_path, layer) != std::cmp::Ordering::Less
            }) {
                continue;
            }

            match &decl.declaration.value {
                Value::Keyword(k) if k.eq_ignore_ascii_case("revert-rule") => {
                    if decl.declaration.important {
                        reverted_from_important = true;
                    }
                    skipping_rule_id = Some(decl.rule_id);
                    continue;
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("revert-layer") => {
                    if reverted_from_important {
                        // Cycle between revert-rule !important and revert-layer resolves to unset
                        chosen_val = None;
                        break;
                    }
                    if decl.origin == Origin::Inline {
                        skipping_origins.insert(Origin::Inline);
                    } else if decl.origin == Origin::Author || decl.origin == Origin::Shadow {
                        let has_earlier_layer = decls.iter().take(idx).any(|d| {
                            d.origin == decl.origin
                                && compare_layer_paths(&d.layer_path, &decl.layer_path)
                                    == std::cmp::Ordering::Less
                        });
                        if has_earlier_layer {
                            skipping_layers.push((decl.origin, decl.layer_path.clone()));
                        } else {
                            skipping_origins.insert(decl.origin);
                        }
                    } else {
                        skipping_origins.insert(decl.origin);
                    }
                    continue;
                }
                Value::Keyword(k) if k.eq_ignore_ascii_case("revert") => {
                    skipping_origins.insert(Origin::Inline);
                    skipping_origins.insert(Origin::Author);
                    skipping_origins.insert(Origin::Shadow);
                    continue;
                }
                val => {
                    chosen_val = Some(val.clone());
                    is_important = decl.declaration.important;
                    break;
                }
            }
        }

        if let Some(val) = chosen_val {
            if is_important {
                important_props.insert(prop_name.clone());
            }
            resolved.insert(prop_name, val);
        }
    }
    (resolved, important_props)
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

    matched_decls.sort_by(|a, b| a.compare_cascade(b));
    resolve_matched_declarations(&matched_decls)
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
