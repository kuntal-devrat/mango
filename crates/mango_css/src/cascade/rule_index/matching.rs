//! Rule matching, container sizing, and cascade candidate collection.

use std::collections::{HashMap, HashSet};

use mango_html::dom::{Document, ElementData, NodeData, NodeId};

use crate::cascade::media::matches_container_query;
use crate::cascade::types::{IndexedRule, MatchedDeclaration, Origin, ScopeDef};
use crate::parser::{StyleRule, parse_declaration_list};
use crate::specificity::Specificity;
use crate::values::{Length, Value};

use super::index::RuleIndex;

fn selector_targets_scope(sel_list: Option<&crate::selectors::SelectorList>) -> bool {
    let Some(list) = sel_list else { return false };
    list.selectors.iter().any(|sel| {
        let subject = if let Some((_, compound)) = sel.tail.last() {
            compound
        } else {
            &sel.head
        };
        subject.simple_selectors.iter().any(|s| match s {
            crate::selectors::SimpleSelector::Nesting => true,
            crate::selectors::SimpleSelector::PseudoClass(p) => {
                p.eq_ignore_ascii_case("scope")
                    || ((p.starts_with("is(") || p.starts_with("where("))
                        && (p.contains("scope") || p.contains('&')))
            }
            _ => false,
        })
    })
}

fn subject_targets_scope(style_rule: &StyleRule) -> bool {
    selector_targets_scope(Some(&style_rule.selectors))
}

pub(crate) fn selector_targets_host(sel: &crate::selectors::ComplexSelector) -> bool {
    let subject = if let Some((_, compound)) = sel.tail.last() {
        compound
    } else {
        &sel.head
    };
    subject.simple_selectors.iter().any(|s| match s {
        crate::selectors::SimpleSelector::PseudoClass(p) => {
            let lower = p.to_ascii_lowercase();
            lower == "host" || lower.starts_with("host(") || lower.starts_with("host-context(")
        }
        _ => false,
    })
}

fn match_scope_chain(
    chain: &[ScopeDef],
    node_id: NodeId,
    doc: &Document,
    style_rule: &StyleRule,
    pseudo: Option<&str>,
) -> Option<NodeId> {
    if chain.is_empty() {
        return Some(node_id);
    }
    match_scope_level(
        chain,
        chain.len() - 1,
        node_id,
        doc,
        style_rule,
        true,
        pseudo,
    )
}

fn match_scope_level(
    chain: &[ScopeDef],
    level: usize,
    target_node: NodeId,
    doc: &Document,
    style_rule: &StyleRule,
    is_innermost: bool,
    pseudo: Option<&str>,
) -> Option<NodeId> {
    let scope_def = &chain[level];
    let root_sel = scope_def.root;
    let limit_sel = scope_def.limit;
    let scoping_element = scope_def.scoping_element;

    let can_target_self = if is_innermost {
        subject_targets_scope(style_rule)
    } else {
        selector_targets_scope(chain[level + 1].root)
    };

    let mut candidates = Vec::new();
    if root_sel.is_some() {
        if can_target_self {
            candidates.push(target_node);
        }
        let mut cur = Some(target_node);
        while let Some(curr_id) = cur {
            let parent = doc
                .get(curr_id)
                .and_then(|n| n.parent)
                .or_else(|| doc.get_host_for_shadow_root(curr_id));
            if let Some(pid) = parent {
                candidates.push(pid);
                cur = Some(pid);
            } else {
                break;
            }
        }
    } else if let Some(scop_id) = scoping_element {
        let mut is_in_scope = false;
        if target_node.raw() == scop_id.raw() {
            if can_target_self {
                is_in_scope = true;
            }
        } else {
            let mut cur = Some(target_node);
            while let Some(curr_id) = cur {
                if curr_id.raw() == scop_id.raw() {
                    is_in_scope = true;
                    break;
                }
                cur = doc
                    .get(curr_id)
                    .and_then(|n| n.parent)
                    .or_else(|| doc.get_host_for_shadow_root(curr_id));
            }
        }
        if is_in_scope {
            candidates.push(scop_id);
        }
    }

    for root_cand in candidates {
        let mut outer_root_opt = None;
        if level > 0 {
            if let Some(outer_root) =
                match_scope_level(chain, level - 1, root_cand, doc, style_rule, false, None)
            {
                outer_root_opt = Some(outer_root);
            } else {
                continue;
            }
        }

        if let Some(rsel) = root_sel {
            if let Some(outer_root) = outer_root_opt {
                crate::selectors::set_current_scope_root(Some(outer_root));
            }
            let matches_root = rsel.matches(root_cand, doc);
            if outer_root_opt.is_some() {
                crate::selectors::set_current_scope_root(None);
            }
            if !matches_root {
                continue;
            }
        }

        crate::selectors::set_current_scope_root(Some(root_cand));
        let is_limited = if let Some(lsel) = limit_sel {
            if selector_targets_scope(Some(lsel)) && lsel.matches(root_cand, doc) {
                true
            } else if target_node.raw() == root_cand.raw() {
                false
            } else {
                let mut hit = false;
                let mut curr_opt = Some(target_node);
                while let Some(elem) = curr_opt {
                    if elem.raw() == root_cand.raw() {
                        break;
                    }
                    if lsel.matches(elem, doc) {
                        hit = true;
                        break;
                    }
                    curr_opt = doc
                        .get(elem)
                        .and_then(|n| n.parent)
                        .or_else(|| doc.get_host_for_shadow_root(elem));
                }
                hit
            }
        } else {
            false
        };
        crate::selectors::set_current_scope_root(None);

        if is_limited {
            continue;
        }

        if is_innermost {
            crate::selectors::set_current_scope_root(Some(root_cand));
            let matches_rule = style_rule.selectors.selectors.iter().any(|sel| {
                if target_node.raw() == root_cand.raw() {
                    let subject = if let Some((_, compound)) = sel.tail.last() {
                        compound
                    } else {
                        &sel.head
                    };
                    let has_scope = subject.simple_selectors.iter().any(|s| match s {
                        crate::selectors::SimpleSelector::Nesting => true,
                        crate::selectors::SimpleSelector::PseudoClass(p) => {
                            p.eq_ignore_ascii_case("scope")
                                || ((p.starts_with("is(") || p.starts_with("where("))
                                    && (p.contains("scope") || p.contains('&')))
                        }
                        _ => false,
                    });
                    if !has_scope {
                        return false;
                    }
                }
                if let Some(p) = pseudo {
                    sel.matches_pseudo_element(target_node, doc, p)
                } else {
                    sel.matches(target_node, doc)
                }
            });
            crate::selectors::set_current_scope_root(None);

            if !matches_rule {
                continue;
            }
        }

        return Some(root_cand);
    }

    None
}

pub(crate) fn apply_style_rule(
    style_rule: &StyleRule,
    node_id: NodeId,
    doc: &Document,
    origin: Origin,
    layer_path: std::sync::Arc<[u32]>,
    scope_chain: &[ScopeDef],
    rule_source_order: usize,
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
            if host_id.raw() == node_id.raw() && !selector_targets_host(complex_sel) {
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
                    p.eq_ignore_ascii_case("scope")
                        || ((p.starts_with("is(") || p.starts_with("where("))
                            && (p.contains("scope") || p.contains('&')))
                }
                _ => false,
            });
            if !has_scope {
                continue;
            }
        }
        let m = complex_sel.matches(node_id, doc);
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
                        layer_path: layer_path.clone(),
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
                    layer_path: layer_path.clone(),
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
    let mut seen_rules: HashSet<usize> = HashSet::new();

    // 0. Root rules (only for <html>)
    if elem.tag_name.eq_ignore_ascii_case("html") {
        for cand in &index.root_rules {
            if seen_rules.insert(cand.source_order) {
                apply_style_rule(
                    cand.style_rule,
                    node_id,
                    doc,
                    cand.origin,
                    cand.layer_path.clone(),
                    &cand.scope_chain,
                    cand.source_order,
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
                cand.layer_path.clone(),
                &cand.scope_chain,
                cand.source_order,
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
                    cand.layer_path.clone(),
                    &cand.scope_chain,
                    cand.source_order,
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
                    cand.layer_path.clone(),
                    &cand.scope_chain,
                    cand.source_order,
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
                        cand.layer_path.clone(),
                        &cand.scope_chain,
                        cand.source_order,
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
                        cand.layer_path.clone(),
                        &cand.scope_chain,
                        cand.source_order,
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
                    std::sync::Arc::from([]),
                    &[],
                    cand.source_order,
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
    let mut seen_rules = HashSet::new();
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
            cand.layer_path.clone(),
            &cand.scope_chain,
            *source_order + cand.source_order,
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

pub(crate) fn find_container_size_for_node(
    index: &RuleIndex,
    node_id: NodeId,
    doc: &Document,
    name_filter: Option<&str>,
) -> Option<(f32, f32)> {
    let mut current = doc.get(node_id).and_then(|n| n.parent);
    while let Some(ancestor_id) = current {
        let Some(ancestor_node) = doc.get(ancestor_id) else {
            break;
        };
        if let NodeData::Element(elem) = &ancestor_node.data {
            let mut container_type = None;
            let mut container_name = None;
            let mut explicit_width = None;
            let mut explicit_height = None;

            // 1. Inspect element's inline style
            if let Some(style_attr) = elem.get_attribute("style") {
                let decls = parse_declaration_list(style_attr);
                for d in decls {
                    let name = d.name.to_ascii_lowercase();
                    if name == "container-type" {
                        container_type = Some(d.value);
                    } else if name == "contain" {
                        let val_str = format!("{:?}", d.value).to_ascii_lowercase();
                        if val_str.contains("size")
                            || val_str.contains("inline-size")
                            || val_str.contains("strict")
                        {
                            if container_type.is_none() {
                                container_type = Some(Value::Keyword("inline-size".into()));
                            }
                        }
                    } else if name == "container-name" {
                        if let Value::String(s) = d.value {
                            container_name = Some(s);
                        } else if let Value::Keyword(s) = d.value {
                            container_name = Some(s);
                        }
                    } else if name == "container" {
                        for exp in d.expand_shorthand() {
                            if exp.name == "container-name" {
                                if let Value::String(s) = exp.value {
                                    container_name = Some(s);
                                } else if let Value::Keyword(s) = exp.value {
                                    container_name = Some(s);
                                }
                            } else if exp.name == "container-type" {
                                container_type = Some(exp.value);
                            }
                        }
                    } else if name == "width" {
                        if let Value::Length(Length::Px(px)) = d.value {
                            explicit_width = Some(px);
                        }
                    } else if name == "height"
                        && let Value::Length(Length::Px(px)) = d.value
                    {
                        explicit_height = Some(px);
                    }
                }
            }

            // 2. Inspect author stylesheet rules defining container properties
            for cand in &index.container_def_rules {
                for complex_sel in &cand.style_rule.selectors.selectors {
                    if complex_sel.matches(ancestor_id, doc) {
                        for decl in &cand.style_rule.declarations {
                            let mut check_decl = |dname: &str, dval: &Value| {
                                if dname == "container-type" && container_type.is_none() {
                                    container_type = Some(dval.clone());
                                } else if dname == "contain" && container_type.is_none() {
                                    let val_str = format!("{:?}", dval).to_ascii_lowercase();
                                    if val_str.contains("size")
                                        || val_str.contains("inline-size")
                                        || val_str.contains("strict")
                                    {
                                        container_type = Some(Value::Keyword("inline-size".into()));
                                    }
                                } else if dname == "container-name" && container_name.is_none() {
                                    if let Value::String(s) = dval {
                                        container_name = Some(s.clone());
                                    } else if let Value::Keyword(s) = dval {
                                        container_name = Some(s.clone());
                                    }
                                } else if dname == "width" && explicit_width.is_none() {
                                    if let Value::Length(Length::Px(px)) = dval {
                                        explicit_width = Some(*px);
                                    }
                                } else if dname == "height"
                                    && explicit_height.is_none()
                                    && let Value::Length(Length::Px(px)) = dval
                                {
                                    explicit_height = Some(*px);
                                }
                            };
                            if decl.is_shorthand() {
                                for exp in decl.expand_shorthand_ref() {
                                    check_decl(&exp.name.to_ascii_lowercase(), &exp.value);
                                }
                            } else {
                                check_decl(&decl.name.to_ascii_lowercase(), &decl.value);
                            }
                        }
                        break;
                    }
                }
            }

            // 3. HTML attributes
            if explicit_width.is_none()
                && let Some(w_str) = elem.get_attribute("width")
                && let Ok(w) = w_str.trim_end_matches("px").parse::<f32>()
            {
                explicit_width = Some(w);
            }
            if explicit_height.is_none()
                && let Some(h_str) = elem.get_attribute("height")
                && let Ok(h) = h_str.trim_end_matches("px").parse::<f32>()
            {
                explicit_height = Some(h);
            }
            if container_name.is_none()
                && let Some(cn) = elem.get_attribute("data-container-name")
            {
                container_name = Some(cn.to_string());
            }
            if container_type.is_none()
                && let Some(ct) = elem.get_attribute("data-container-type")
                && let Some(parsed_ct) = crate::values::ContainerType::parse(ct)
            {
                container_type = Some(Value::ContainerType(parsed_ct));
            }
            if container_type.is_none()
                && let Some(c) = elem.get_attribute("contain")
                && (c.contains("size") || c.contains("inline-size") || c.contains("strict"))
            {
                container_type = Some(Value::Keyword("inline-size".into()));
            }

            // Check if this element establishes a container
            let is_container = match &container_type {
                Some(Value::ContainerType(ct)) => *ct != crate::values::ContainerType::Normal,
                Some(Value::Keyword(k)) => k != "normal",
                _ => container_name.is_some(),
            };

            if is_container {
                let matches_name = match name_filter {
                    Some(filter) => {
                        if let Some(ref cn) = container_name {
                            cn.split_whitespace()
                                .any(|part| part.eq_ignore_ascii_case(filter))
                        } else {
                            false
                        }
                    }
                    None => true,
                };

                if matches_name {
                    let w = explicit_width.unwrap_or(index.viewport_width);
                    let h = explicit_height.unwrap_or(index.viewport_height);
                    return Some((w, h));
                }
            }
        }
        current = ancestor_node.parent;
    }
    None
}
