//! Cascade priority resolution and style aggregation.

use std::collections::HashMap;

use mango_html::dom::{Document, NodeData, NodeId};

use crate::parser::{Stylesheet, parse_declaration_list};
use crate::specificity::Specificity;
use crate::values::{Direction, Value, WritingMode};

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

/// Resolves all specified property values with author index, optional shadow index, multiple outer part indexes,
/// and parent writing-mode / direction context for accurate logical property canonicalization.
pub fn resolve_cascade_with_shadow_and_part_indexes_and_parent_details(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
    part_indexes: &[&RuleIndex],
    parent_wm: Option<WritingMode>,
    parent_dir: Option<Direction>,
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

    if matched_decls.is_empty() {
        return (HashMap::new(), std::collections::HashSet::new());
    }
    matched_decls.sort_by(|a, b| a.compare_cascade(b));
    let (wm, dir) =
        resolve_writing_mode_and_direction(node_id, doc, &matched_decls, parent_wm, parent_dir);
    resolve_matched_declarations_with_importance(&matched_decls, wm, dir)
}

/// Resolves all specified property values with author index, optional shadow index, and multiple outer part indexes.
pub fn resolve_cascade_with_shadow_and_part_indexes_details(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    shadow_index: Option<&RuleIndex>,
    part_indexes: &[&RuleIndex],
) -> (HashMap<String, Value>, std::collections::HashSet<String>) {
    resolve_cascade_with_shadow_and_part_indexes_and_parent_details(
        node_id,
        doc,
        author_index,
        shadow_index,
        part_indexes,
        None,
        None,
    )
}

fn resolve_writing_mode_and_direction(
    node_id: NodeId,
    doc: &Document,
    matched_decls: &[MatchedDeclaration],
    parent_wm: Option<WritingMode>,
    parent_dir: Option<Direction>,
) -> (WritingMode, Direction) {
    let mut wm = parent_wm.unwrap_or(WritingMode::HorizontalTb);
    let mut dir = parent_dir.unwrap_or(Direction::Ltr);

    // If parent_wm was not provided, check ancestors for inline style or dir attribute
    if parent_wm.is_none() {
        let mut curr = doc.get(node_id).and_then(|n| n.parent);
        while let Some(pid) = curr {
            if let Some(pnode) = doc.get(pid) {
                if let NodeData::Element(pelem) = &pnode.data {
                    if let Some(style_attr) = pelem.get_attribute("style") {
                        for decl in parse_declaration_list(style_attr) {
                            if decl.name == "writing-mode" {
                                if let Value::Keyword(ref k) = decl.value {
                                    match k.to_ascii_lowercase().as_str() {
                                        "vertical-rl" => wm = WritingMode::VerticalRl,
                                        "vertical-lr" => wm = WritingMode::VerticalLr,
                                        "sideways-rl" => wm = WritingMode::SidewaysRl,
                                        "sideways-lr" => wm = WritingMode::SidewaysLr,
                                        "horizontal-tb" => wm = WritingMode::HorizontalTb,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    if let Some(dir_attr) = pelem.get_attribute("dir") {
                        if dir_attr.eq_ignore_ascii_case("rtl") {
                            dir = Direction::Rtl;
                        } else if dir_attr.eq_ignore_ascii_case("ltr") {
                            dir = Direction::Ltr;
                        }
                    }
                }
                curr = pnode.parent;
            } else {
                break;
            }
        }
    }

    // Check HTML dir attribute on the element itself
    if let Some(node) = doc.get(node_id) {
        if let NodeData::Element(elem) = &node.data {
            if let Some(dir_attr) = elem.get_attribute("dir") {
                if dir_attr.eq_ignore_ascii_case("rtl") {
                    dir = Direction::Rtl;
                } else if dir_attr.eq_ignore_ascii_case("ltr") {
                    dir = Direction::Ltr;
                }
            }
        }
    }

    // Check matched declarations on this element (applied in cascade order)
    for matched in matched_decls {
        if matched.declaration.name == "writing-mode" {
            if let Value::Keyword(ref k) = matched.declaration.value {
                match k.to_ascii_lowercase().as_str() {
                    "vertical-rl" => wm = WritingMode::VerticalRl,
                    "vertical-lr" => wm = WritingMode::VerticalLr,
                    "sideways-rl" => wm = WritingMode::SidewaysRl,
                    "sideways-lr" => wm = WritingMode::SidewaysLr,
                    "horizontal-tb" => wm = WritingMode::HorizontalTb,
                    _ => {}
                }
            }
        } else if matched.declaration.name == "direction" {
            if let Value::Keyword(ref k) = matched.declaration.value {
                if k.eq_ignore_ascii_case("rtl") {
                    dir = Direction::Rtl;
                } else if k.eq_ignore_ascii_case("ltr") {
                    dir = Direction::Ltr;
                }
            }
        }
    }

    (wm, dir)
}

fn resolve_matched_declarations(matched_decls: &[MatchedDeclaration]) -> HashMap<String, Value> {
    resolve_matched_declarations_with_importance(
        matched_decls,
        WritingMode::HorizontalTb,
        Direction::Ltr,
    )
    .0
}

fn map_logical_side(side: &str, wm: WritingMode, dir: Direction) -> &'static str {
    match side {
        "block-start" => match wm {
            WritingMode::HorizontalTb => "top",
            WritingMode::VerticalRl | WritingMode::SidewaysRl => "right",
            WritingMode::VerticalLr | WritingMode::SidewaysLr => "left",
        },
        "block-end" => match wm {
            WritingMode::HorizontalTb => "bottom",
            WritingMode::VerticalRl | WritingMode::SidewaysRl => "left",
            WritingMode::VerticalLr | WritingMode::SidewaysLr => "right",
        },
        "inline-start" => {
            if wm.is_vertical() {
                if dir == Direction::Rtl {
                    "bottom"
                } else {
                    "top"
                }
            } else if dir == Direction::Rtl {
                "right"
            } else {
                "left"
            }
        }
        "inline-end" => {
            if wm.is_vertical() {
                if dir == Direction::Rtl {
                    "top"
                } else {
                    "bottom"
                }
            } else if dir == Direction::Rtl {
                "left"
            } else {
                "right"
            }
        }
        _ => "top",
    }
}

fn canonical_property_name(name: &str, wm: WritingMode, dir: Direction) -> &str {
    match name {
        "margin-block-start" => match map_logical_side("block-start", wm, dir) {
            "top" => "margin-top",
            "bottom" => "margin-bottom",
            "left" => "margin-left",
            _ => "margin-right",
        },
        "margin-block-end" => match map_logical_side("block-end", wm, dir) {
            "top" => "margin-top",
            "bottom" => "margin-bottom",
            "left" => "margin-left",
            _ => "margin-right",
        },
        "margin-inline-start" => match map_logical_side("inline-start", wm, dir) {
            "top" => "margin-top",
            "bottom" => "margin-bottom",
            "left" => "margin-left",
            _ => "margin-right",
        },
        "margin-inline-end" => match map_logical_side("inline-end", wm, dir) {
            "top" => "margin-top",
            "bottom" => "margin-bottom",
            "left" => "margin-left",
            _ => "margin-right",
        },
        "padding-block-start" => match map_logical_side("block-start", wm, dir) {
            "top" => "padding-top",
            "bottom" => "padding-bottom",
            "left" => "padding-left",
            _ => "padding-right",
        },
        "padding-block-end" => match map_logical_side("block-end", wm, dir) {
            "top" => "padding-top",
            "bottom" => "padding-bottom",
            "left" => "padding-left",
            _ => "padding-right",
        },
        "padding-inline-start" => match map_logical_side("inline-start", wm, dir) {
            "top" => "padding-top",
            "bottom" => "padding-bottom",
            "left" => "padding-left",
            _ => "padding-right",
        },
        "padding-inline-end" => match map_logical_side("inline-end", wm, dir) {
            "top" => "padding-top",
            "bottom" => "padding-bottom",
            "left" => "padding-left",
            _ => "padding-right",
        },
        "inset-block-start" => map_logical_side("block-start", wm, dir),
        "inset-block-end" => map_logical_side("block-end", wm, dir),
        "inset-inline-start" => map_logical_side("inline-start", wm, dir),
        "inset-inline-end" => map_logical_side("inline-end", wm, dir),
        "block-size" => {
            if wm.is_vertical() {
                "width"
            } else {
                "height"
            }
        }
        "inline-size" => {
            if wm.is_vertical() {
                "height"
            } else {
                "width"
            }
        }
        "min-block-size" => {
            if wm.is_vertical() {
                "min-width"
            } else {
                "min-height"
            }
        }
        "min-inline-size" => {
            if wm.is_vertical() {
                "min-height"
            } else {
                "min-width"
            }
        }
        "max-block-size" => {
            if wm.is_vertical() {
                "max-width"
            } else {
                "max-height"
            }
        }
        "max-inline-size" => {
            if wm.is_vertical() {
                "max-height"
            } else {
                "max-width"
            }
        }
        other => other,
    }
}

fn resolve_matched_declarations_with_importance(
    matched_decls: &[MatchedDeclaration],
    wm: WritingMode,
    dir: Direction,
) -> (HashMap<String, Value>, std::collections::HashSet<String>) {
    if matched_decls.is_empty() {
        return (HashMap::new(), std::collections::HashSet::new());
    }
    let mut decls_by_prop: HashMap<&str, Vec<&MatchedDeclaration>> = HashMap::new();
    for matched in matched_decls {
        let prop = canonical_property_name(&matched.declaration.name, wm, dir);
        decls_by_prop.entry(prop).or_default().push(matched);
    }

    let mut resolved = HashMap::with_capacity(decls_by_prop.len());
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
                important_props.insert(prop_name.to_string());
            }
            resolved.insert(prop_name.to_string(), val);
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

    if matched_decls.is_empty() {
        return HashMap::new();
    }

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
