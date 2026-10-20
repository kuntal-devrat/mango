//! Rule matching, container sizing, and cascade candidate collection.

use std::collections::{HashMap, HashSet};

use mango_html::dom::{Document, ElementData, NodeData, NodeId};

use crate::cascade::media::matches_container_query;
use crate::cascade::types::{IndexedRule, MatchedDeclaration, Origin};
use crate::parser::{StyleRule, parse_declaration_list};
use crate::specificity::Specificity;
use crate::values::{Length, Value};

use super::index::RuleIndex;

pub(crate) fn apply_style_rule(
    style_rule: &StyleRule,
    node_id: NodeId,
    doc: &Document,
    origin: Origin,
    layer_order: Option<u32>,
    scope_root: Option<&crate::selectors::SelectorList>,
    scope_limit: Option<&crate::selectors::SelectorList>,
    rule_source_order: usize,
    out: &mut Vec<MatchedDeclaration>,
) {
    let mut added_scope_spec = Specificity::ZERO;

    if scope_root.is_some() || scope_limit.is_some() {
        let mut matching_root: Option<(NodeId, Specificity)> = None;

        if let Some(root_sel) = scope_root {
            let is_root_self = root_sel.matches(node_id, doc);
            let limit_excludes_self = scope_limit.is_some_and(|lim| lim.matches(node_id, doc));

            if is_root_self && !limit_excludes_self {
                let root_spec = root_sel
                    .selectors
                    .iter()
                    .map(Specificity::of)
                    .max()
                    .unwrap_or(Specificity::ZERO);
                matching_root = Some((node_id, root_spec));
            }

            if matching_root.is_none() {
                if limit_excludes_self {
                    return;
                }
                let mut curr = doc.get(node_id).and_then(|n| n.parent);
                let mut hit_limit = false;
                while let Some(ancestor_id) = curr {
                    if root_sel.matches(ancestor_id, doc) {
                        if !hit_limit {
                            let root_spec = root_sel
                                .selectors
                                .iter()
                                .map(Specificity::of)
                                .max()
                                .unwrap_or(Specificity::ZERO);
                            matching_root = Some((ancestor_id, root_spec));
                        }
                        break;
                    }
                    if let Some(lim) = scope_limit {
                        if lim.matches(ancestor_id, doc) {
                            hit_limit = true;
                        }
                    }
                    curr = doc.get(ancestor_id).and_then(|n| n.parent);
                }
            }

            if let Some((root_node, root_spec)) = matching_root {
                crate::selectors::set_current_scope_root(Some(root_node));
                added_scope_spec = root_spec;
            } else {
                return;
            }
        } else if let Some(lim) = scope_limit {
            let mut curr = Some(node_id);
            while let Some(elem_id) = curr {
                if lim.matches(elem_id, doc) {
                    return;
                }
                curr = doc.get(elem_id).and_then(|n| n.parent);
            }
        }
    }

    let mut max_specificity: Option<Specificity> = None;
    for complex_sel in &style_rule.selectors.selectors {
        if complex_sel.matches(node_id, doc) {
            let spec = Specificity::of(complex_sel) + added_scope_spec;
            max_specificity = Some(match max_specificity {
                Some(current) => current.max(spec),
                None => spec,
            });
        }
    }

    if scope_root.is_some() {
        crate::selectors::set_current_scope_root(None);
    }

    if let Some(specificity) = max_specificity {
        let mut decl_idx = 0;
        for decl in &style_rule.declarations {
            if decl.is_shorthand() {
                for expanded in decl.expand_shorthand_ref() {
                    decl_idx += 1;
                    out.push(MatchedDeclaration {
                        declaration: expanded,
                        specificity,
                        origin,
                        source_order: rule_source_order * 1000 + decl_idx,
                        layer_order,
                    });
                }
            } else {
                decl_idx += 1;
                out.push(MatchedDeclaration {
                    declaration: decl.clone(),
                    specificity,
                    origin,
                    source_order: rule_source_order * 1000 + decl_idx,
                    layer_order,
                });
            }
        }
    }
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
                    cand.layer_order,
                    cand.scope_root,
                    cand.scope_limit,
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
                cand.layer_order,
                cand.scope_root,
                cand.scope_limit,
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
                    cand.layer_order,
                    cand.scope_root,
                    cand.scope_limit,
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
                    cand.layer_order,
                    cand.scope_root,
                    cand.scope_limit,
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
                        cand.layer_order,
                        cand.scope_root,
                        cand.scope_limit,
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
                        cand.layer_order,
                        cand.scope_root,
                        cand.scope_limit,
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
                    None,
                    None,
                    None,
                    cand.source_order,
                    out,
                );
            }
        }
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
                            layer_order: cand.layer_order,
                        });
                    }
                } else {
                    decl_idx += 1;
                    out.push(MatchedDeclaration {
                        declaration: decl.clone(),
                        specificity,
                        origin: cand.origin,
                        source_order: cand.source_order * 1000 + decl_idx,
                        layer_order: cand.layer_order,
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
                                layer_order: None,
                            });
                        }
                    } else {
                        decl_idx += 1;
                        out.push(MatchedDeclaration {
                            declaration: decl.clone(),
                            specificity,
                            origin: cand.origin,
                            source_order: cand.source_order * 1000 + decl_idx,
                            layer_order: None,
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
