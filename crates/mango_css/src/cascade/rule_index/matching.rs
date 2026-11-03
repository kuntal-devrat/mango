//! Rule matching, container sizing, and cascade candidate collection.

use std::collections::{HashMap, HashSet};

use mango_html::dom::{Document, ElementData, NodeData, NodeId};

use crate::cascade::media::matches_container_query;
use crate::cascade::types::{IndexedRule, MatchedDeclaration, Origin, ScopeDef};
use crate::parser::StyleRule;
use crate::specificity::Specificity;

pub(crate) use super::container::find_container_size_for_node;
use super::index::RuleIndex;
pub(crate) use super::scope::*;

static EMPTY_LAYER_PATH: std::sync::LazyLock<std::sync::Arc<[u32]>> =
    std::sync::LazyLock::new(|| std::sync::Arc::from([]));

pub(crate) fn apply_style_rule(
    style_rule: &StyleRule,
    node_id: NodeId,
    doc: &Document,
    origin: Origin,
    layer_path: &std::sync::Arc<[u32]>,
    scope_chain: &[ScopeDef],
    rule_source_order: usize,
    bloom_filter: Option<&selectors::bloom::BloomFilter>,
    out: &mut Vec<MatchedDeclaration>,
) {
    let winning_root = if !scope_chain.is_empty() {
        if let Some(root_node) = match_scope_chain(scope_chain, node_id, doc, style_rule, None) {
            Some(root_node)
        } else {
            return;
        }
    } else {
        None
    };

    let scope_proximity = winning_root.map(|root| {
        let mut hops = 0u32;
        let mut cur = node_id;
        while cur.raw() != root.raw() {
            if let Some(parent) = doc
                .get(cur)
                .and_then(|n| n.parent)
                .or_else(|| doc.get_host_for_shadow_root(cur))
            {
                hops += 1;
                cur = parent;
            } else {
                break;
            }
        }
        hops
    });

    if let Some(root_node) = winning_root {
        crate::selectors::set_current_scope_root(Some(root_node));
    }

    let mut max_specificity: Option<Specificity> = None;
    let mut non_visited_match = false;
    for complex_sel in &style_rule.selectors.selectors {
        // Enforce shadow boundary:
        // If matching against a shadow host with shadow stylesheets, only :host selectors match.
        if let Some(host_id) = crate::selectors::current_host_element() {
            if host_id.raw() == node_id.raw()
                && !selector_targets_host(complex_sel)
                && winning_root.map(|r| r.raw()) != Some(host_id.raw())
            {
                continue;
            }
        }
        if winning_root.map(|r| r.raw()) == Some(node_id.raw()) {
            let subject = if let Some((_, compound)) = complex_sel.tail.last() {
                compound
            } else {
                &complex_sel.head
            };
            let has_scope = subject.simple_selectors.iter().any(|s| match s {
                crate::selectors::SimpleSelector::Nesting => true,
                crate::selectors::SimpleSelector::PseudoClass(p) => {
                    let lower = p.to_ascii_lowercase();
                    lower == "scope"
                        || lower == "host"
                        || lower.starts_with("host(")
                        || lower.starts_with("host-context(")
                        || ((lower.starts_with("is(") || lower.starts_with("where("))
                            && (lower.contains("scope")
                                || lower.contains('&')
                                || lower.contains("host")))
                }
                _ => false,
            });
            if !has_scope {
                continue;
            }
        }
        let m = complex_sel.matches_with_filter(node_id, doc, bloom_filter);
        if m {
            if !is_visited_selector(complex_sel) {
                non_visited_match = true;
            }
            let spec = Specificity::of(complex_sel);
            max_specificity = Some(match max_specificity {
                Some(current) => current.max(spec),
                None => spec,
            });
        }
    }

    if winning_root.is_some() {
        crate::selectors::set_current_scope_root(None);
    }

    if let Some(specificity) = max_specificity {
        let is_visited_only = !non_visited_match;
        let mut decl_idx = 0;
        for decl in &style_rule.declarations {
            if decl.is_shorthand() {
                for expanded in decl.expand_shorthand_ref() {
                    // CSS Selectors 4 § 4.1: Styling of :visited is restricted to color properties only.
                    if is_visited_only && !is_allowed_visited_property(&expanded.name) {
                        continue;
                    }
                    decl_idx += 1;
                    out.push(MatchedDeclaration {
                        declaration: expanded,
                        specificity,
                        origin,
                        source_order: rule_source_order * 1000 + decl_idx,
                        layer_path: std::sync::Arc::clone(layer_path),
                        rule_id: rule_source_order,
                        scope_proximity,
                    });
                }
            } else {
                if is_visited_only && !is_allowed_visited_property(&decl.name) {
                    continue;
                }
                decl_idx += 1;
                out.push(MatchedDeclaration {
                    declaration: decl.clone(),
                    specificity,
                    origin,
                    source_order: rule_source_order * 1000 + decl_idx,
                    layer_path: std::sync::Arc::clone(layer_path),
                    rule_id: rule_source_order,
                    scope_proximity,
                });
            }
        }
    }
}

fn is_visited_selector(complex_sel: &crate::selectors::ComplexSelector) -> bool {
    let subject = if let Some((_, compound)) = complex_sel.tail.last() {
        compound
    } else {
        &complex_sel.head
    };
    subject.simple_selectors.iter().any(|s| match s {
        crate::selectors::SimpleSelector::PseudoClass(p) => p.eq_ignore_ascii_case("visited"),
        _ => false,
    })
}

fn is_allowed_visited_property(prop: &str) -> bool {
    matches!(
        prop,
        "color"
            | "background-color"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
            | "outline-color"
            | "column-rule-color"
            | "fill"
            | "stroke"
    )
}

pub(crate) fn collect_matches_for_node<'a>(
    index: &RuleIndex<'a>,
    node_id: NodeId,
    doc: &Document,
    elem: &ElementData,
    _source_order: &mut usize,
    out: &mut Vec<MatchedDeclaration>,
) {
    let mut seen_rules: HashSet<usize> = HashSet::with_capacity(32);

    // Fast BloomFilter of ancestors for O(1) early rejection of ancestor selectors
    let mut bloom = selectors::bloom::BloomFilter::default();
    let mut cur = doc.get(node_id).and_then(|n| n.parent);
    while let Some(pid) = cur {
        let p_elem = crate::selectors::servo::MangoElement { node_id: pid, doc };
        selectors::Element::add_element_unique_hashes(&p_elem, &mut bloom);
        cur = doc
            .get(pid)
            .and_then(|n| n.parent)
            .or_else(|| doc.get_host_for_shadow_root(pid));
    }
    let filter = Some(&bloom);

    // 0. Root rules (only for <html>)
    if elem.tag_name.eq_ignore_ascii_case("html") {
        for cand in &index.root_rules {
            if seen_rules.insert(cand.source_order) {
                apply_style_rule(
                    cand.style_rule,
                    node_id,
                    doc,
                    cand.origin,
                    &cand.layer_path,
                    &cand.scope_chain,
                    cand.source_order,
                    filter,
                    out,
                );
            }
        }
    }

    // 1. Universal rules (always candidates)
    for cand in &index.universal_rules {
        if seen_rules.insert(cand.source_order) {
            apply_style_rule(
                cand.style_rule,
                node_id,
                doc,
                cand.origin,
                &cand.layer_path,
                &cand.scope_chain,
                cand.source_order,
                filter,
                out,
            );
        }
    }

    // 2. Tag name bucket
    let tag_lower = elem.tag_name.to_ascii_lowercase();
    if let Some(rules) = index.tag_rules.get(&tag_lower) {
        for cand in rules {
            if seen_rules.insert(cand.source_order) {
                apply_style_rule(
                    cand.style_rule,
                    node_id,
                    doc,
                    cand.origin,
                    &cand.layer_path,
                    &cand.scope_chain,
                    cand.source_order,
                    filter,
                    out,
                );
            }
        }
    }

    // 3. ID bucket
    if let Some(id) = elem.id()
        && let Some(rules) = index.id_rules.get(id)
    {
        for cand in rules {
            if seen_rules.insert(cand.source_order) {
                apply_style_rule(
                    cand.style_rule,
                    node_id,
                    doc,
                    cand.origin,
                    &cand.layer_path,
                    &cand.scope_chain,
                    cand.source_order,
                    filter,
                    out,
                );
            }
        }
    }

    // 4. Class buckets
    for cls in elem.classes() {
        if let Some(rules) = index.class_rules.get(cls) {
            for cand in rules {
                if seen_rules.insert(cand.source_order) {
                    apply_style_rule(
                        cand.style_rule,
                        node_id,
                        doc,
                        cand.origin,
                        &cand.layer_path,
                        &cand.scope_chain,
                        cand.source_order,
                        filter,
                        out,
                    );
                }
            }
        }
    }

    // 4.5 Attribute buckets
    for (attr_name, _) in &elem.attributes {
        let attr_lower = attr_name.to_ascii_lowercase();
        if let Some(rules) = index.attr_rules.get(&attr_lower) {
            for cand in rules {
                if seen_rules.insert(cand.source_order) {
                    apply_style_rule(
                        cand.style_rule,
                        node_id,
                        doc,
                        cand.origin,
                        &cand.layer_path,
                        &cand.scope_chain,
                        cand.source_order,
                        filter,
                        out,
                    );
                }
            }
        }
    }

    // 5. Container rules
    let mut cached_container_sizes: HashMap<Option<String>, Option<(f32, f32)>> = HashMap::new();
    for cand in &index.container_rules {
        if seen_rules.insert(cand.source_order) {
            let size = *cached_container_sizes
                .entry(cand.name.clone())
                .or_insert_with(|| {
                    index.find_container_size_for_node(node_id, doc, cand.name.as_deref())
                });
            if let Some((cw, ch)) = size
                && matches_container_query(&cand.query, cw, ch)
            {
                apply_style_rule(
                    cand.style_rule,
                    node_id,
                    doc,
                    cand.origin,
                    &EMPTY_LAYER_PATH,
                    &[],
                    cand.source_order,
                    filter,
                    out,
                );
            }
        }
    }
}

pub(crate) fn collect_part_matches_for_node<'a>(
    index: &RuleIndex<'a>,
    node_id: NodeId,
    doc: &Document,
    source_order: &mut usize,
    out: &mut Vec<MatchedDeclaration>,
) {
    let mut seen_rules = HashSet::with_capacity(16);
    for cand in &index.universal_rules {
        let is_part_rule = cand.style_rule.selectors.selectors.iter().any(|s| {
            let subject = if let Some((_, compound)) = s.tail.last() {
                compound
            } else {
                &s.head
            };
            subject.has_part_pseudo()
        });
        if !is_part_rule {
            continue;
        }
        if !seen_rules.insert(cand.source_order) {
            continue;
        }
        apply_style_rule(
            cand.style_rule,
            node_id,
            doc,
            cand.origin,
            &cand.layer_path,
            &cand.scope_chain,
            *source_order + cand.source_order,
            None,
            out,
        );
    }
}

pub(crate) fn collect_pseudo_matches_for_node<'a>(
    index: &RuleIndex<'a>,
    node_id: NodeId,
    doc: &Document,
    pseudo: &str,
    _source_order: &mut usize,
    out: &mut Vec<MatchedDeclaration>,
) {
    let pseudo_lower = pseudo.to_ascii_lowercase();
    let Some(bucket) = index.pseudo_rules.get(&pseudo_lower) else {
        return;
    };

    let Some(node) = doc.get(node_id) else {
        return;
    };
    let NodeData::Element(elem) = &node.data else {
        return;
    };

    let mut seen_rules: HashSet<usize> = HashSet::new();

    let mut check_cand = |cand: &IndexedRule<'a>, out: &mut Vec<MatchedDeclaration>| {
        if !seen_rules.insert(cand.source_order) {
            return;
        }
        if let Some(host_id) = crate::selectors::current_host_element() {
            if host_id.raw() == node_id.raw()
                && !cand
                    .style_rule
                    .selectors
                    .selectors
                    .iter()
                    .any(selector_targets_host)
            {
                return;
            }
        }
        let winning_root = if !cand.scope_chain.is_empty() {
            if let Some(root_node) = match_scope_chain(
                &cand.scope_chain,
                node_id,
                doc,
                cand.style_rule,
                Some(&pseudo_lower),
            ) {
                Some(root_node)
            } else {
                return;
            }
        } else {
            None
        };
        if let Some(root) = winning_root {
            crate::selectors::set_current_scope_root(Some(root));
        }
        let mut max_specificity: Option<Specificity> = None;
        for complex_sel in &cand.style_rule.selectors.selectors {
            if complex_sel.matches_pseudo_element(node_id, doc, pseudo) {
                let spec = Specificity::of(complex_sel);
                max_specificity = Some(match max_specificity {
                    Some(current) => current.max(spec),
                    None => spec,
                });
            }
        }
        if winning_root.is_some() {
            crate::selectors::set_current_scope_root(None);
        }
        if let Some(specificity) = max_specificity {
            let scope_proximity = winning_root.map(|root| {
                let mut depth = 0u32;
                let mut cur = Some(node_id);
                while let Some(elem) = cur {
                    if elem.raw() == root.raw() {
                        break;
                    }
                    depth += 1;
                    cur = doc.get(elem).and_then(|n| n.parent);
                }
                depth
            });
            let mut decl_idx = 0;
            for decl in &cand.style_rule.declarations {
                if decl.is_shorthand() {
                    for expanded in decl.expand_shorthand_ref() {
                        decl_idx += 1;
                        out.push(MatchedDeclaration {
                            declaration: expanded,
                            specificity,
                            origin: cand.origin,
                            source_order: cand.source_order * 1000 + decl_idx,
                            layer_path: cand.layer_path.clone(),
                            rule_id: cand.source_order,
                            scope_proximity,
                        });
                    }
                } else {
                    decl_idx += 1;
                    out.push(MatchedDeclaration {
                        declaration: decl.clone(),
                        specificity,
                        origin: cand.origin,
                        source_order: cand.source_order * 1000 + decl_idx,
                        layer_path: cand.layer_path.clone(),
                        rule_id: cand.source_order,
                        scope_proximity,
                    });
                }
            }
        }
    };

    // 0. Root pseudo rules (only for <html>)
    if elem.tag_name.eq_ignore_ascii_case("html") {
        for cand in &bucket.root_rules {
            check_cand(cand, out);
        }
    }

    // 1. Universal pseudo rules
    for cand in &bucket.universal_rules {
        check_cand(cand, out);
    }

    // 2. Tag bucket
    let tag_lower = elem.tag_name.to_ascii_lowercase();
    if let Some(rules) = bucket.tag_rules.get(&tag_lower) {
        for cand in rules {
            check_cand(cand, out);
        }
    }

    // 3. ID bucket
    if let Some(id) = elem.id()
        && let Some(rules) = bucket.id_rules.get(id)
    {
        for cand in rules {
            check_cand(cand, out);
        }
    }

    // 4. Class buckets
    for cls in elem.classes() {
        if let Some(rules) = bucket.class_rules.get(cls) {
            for cand in rules {
                check_cand(cand, out);
            }
        }
    }

    // 5. Attribute buckets
    for (attr_name, _) in &elem.attributes {
        let attr_lower = attr_name.to_ascii_lowercase();
        if let Some(rules) = bucket.attr_rules.get(&attr_lower) {
            for cand in rules {
                check_cand(cand, out);
            }
        }
    }

    // Container rules targeting pseudo-elements
    let mut cached_container_sizes: HashMap<Option<String>, Option<(f32, f32)>> = HashMap::new();
    for cand in &index.container_rules {
        if !seen_rules.insert(cand.source_order) {
            continue;
        }
        let has_pseudo = cand
            .style_rule
            .selectors
            .selectors
            .iter()
            .any(|s| s.matches_pseudo_element(node_id, doc, pseudo));
        if !has_pseudo {
            continue;
        }
        let size = *cached_container_sizes
            .entry(cand.name.clone())
            .or_insert_with(|| {
                index.find_container_size_for_node(node_id, doc, cand.name.as_deref())
            });
        if let Some((cw, ch)) = size
            && matches_container_query(&cand.query, cw, ch)
        {
            let mut max_specificity: Option<Specificity> = None;
            for complex_sel in &cand.style_rule.selectors.selectors {
                if complex_sel.matches_pseudo_element(node_id, doc, pseudo) {
                    let spec = Specificity::of(complex_sel);
                    max_specificity = Some(match max_specificity {
                        Some(current) => current.max(spec),
                        None => spec,
                    });
                }
            }
            if let Some(specificity) = max_specificity {
                let mut decl_idx = 0;
                for decl in &cand.style_rule.declarations {
                    if decl.is_shorthand() {
                        for expanded in decl.expand_shorthand_ref() {
                            decl_idx += 1;
                            out.push(MatchedDeclaration {
                                declaration: expanded,
                                specificity,
                                origin: cand.origin,
                                source_order: cand.source_order * 1000 + decl_idx,
                                layer_path: std::sync::Arc::from([]),
                                rule_id: cand.source_order,
                                scope_proximity: None,
                            });
                        }
                    } else {
                        decl_idx += 1;
                        out.push(MatchedDeclaration {
                            declaration: decl.clone(),
                            specificity,
                            origin: cand.origin,
                            source_order: cand.source_order * 1000 + decl_idx,
                            layer_path: std::sync::Arc::from([]),
                            rule_id: cand.source_order,
                            scope_proximity: None,
                        });
                    }
                }
            }
        }
    }
}
