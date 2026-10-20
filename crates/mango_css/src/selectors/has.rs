//! Implementation of the `:has()` relational pseudo-class and selector pattern caching.

use mango_html::dom::{Document, ElementData, NodeData, NodeId};

use super::nav::{
    is_descendant_of, next_element_sibling, parent_element, prev_element_sibling,
    subsequent_element_siblings,
};
use super::types::{Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector};

pub(crate) fn get_or_parse_complex_selector(inner: &str) -> Option<SelectorList> {
    use std::sync::{OnceLock, RwLock};
    static CACHE: OnceLock<RwLock<std::collections::HashMap<String, Option<SelectorList>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()));

    // Fast path: read lock for cache hits.
    if let Ok(guard) = cache.read()
        && let Some(res) = guard.get(inner)
    {
        return res.clone();
    }

    let parsed = crate::parser::parse_selectors(inner);
    if let Ok(mut guard) = cache.write() {
        // Evict half the cache to avoid thundering-herd stampede that a full
        // clear() would cause — all threads would re-parse simultaneously.
        if guard.len() >= 2048 {
            let keys_to_remove: Vec<String> = guard.keys().take(guard.len() / 2).cloned().collect();
            for key in keys_to_remove {
                guard.remove(&key);
            }
        }
        guard.insert(inner.to_string(), parsed.clone());
    }
    parsed
}

pub(crate) fn matches_simple_pattern(
    pattern: &str,
    el: &ElementData,
    node_id: NodeId,
    doc: &Document,
) -> bool {
    let p = pattern.trim();
    if p.is_empty() {
        return false;
    }
    if p == "*" {
        return true;
    }
    if let Some(class) = p.strip_prefix('.') {
        el.has_class(class)
    } else if let Some(id) = p.strip_prefix('#') {
        el.id() == Some(id)
    } else if let Some(pseudo) = p.strip_prefix(':') {
        SimpleSelector::PseudoClass(pseudo.to_string()).matches(node_id, doc)
    } else if p.starts_with('[') && p.ends_with(']') {
        let attr_inner = &p[1..p.len() - 1];
        if let Some((k, v)) = attr_inner.split_once('=') {
            let key = k.trim();
            let val = v.trim().trim_matches('"').trim_matches('\'');
            el.get_attribute(key) == Some(val)
        } else {
            el.get_attribute(attr_inner.trim()).is_some()
        }
    } else {
        el.tag_name.eq_ignore_ascii_case(p)
    }
}

pub(crate) fn matches_has(node_id: NodeId, selectors: &[ComplexSelector], doc: &Document) -> bool {
    for sel in selectors {
        // Check if selector starts with a relative combinator (e.g. > img, + p, ~ p)
        let is_rel =
            sel.head.simple_selectors == vec![SimpleSelector::Universal] && !sel.tail.is_empty();

        if is_rel {
            let first_comb = sel.tail[0].0;
            match first_comb {
                Combinator::NextSibling => {
                    if let Some(next) = next_element_sibling(node_id, doc) {
                        if sel.tail.len() == 1 {
                            if sel.tail[0].1.matches(next, doc) {
                                return true;
                            }
                        } else if sel.matches_relative(next, Some(node_id), doc) {
                            return true;
                        }
                    }
                }
                Combinator::SubsequentSibling => {
                    for sib in subsequent_element_siblings(node_id, doc) {
                        if sel.tail.len() == 1 {
                            if sel.tail[0].1.matches(sib, doc) {
                                return true;
                            }
                        } else if sel.matches_relative(sib, Some(node_id), doc) {
                            return true;
                        }
                    }
                }
                Combinator::Child => {
                    for child in doc.children(node_id) {
                        if matches!(child.data, NodeData::Element(_)) {
                            if sel.tail.len() == 1 {
                                if sel.tail[0].1.matches(child.id, doc) {
                                    return true;
                                }
                            } else if sel.matches_relative(child.id, Some(node_id), doc) {
                                return true;
                            }
                        }
                    }
                }
                Combinator::Descendant => {
                    let mut stack: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
                    while let Some(curr) = stack.pop() {
                        if matches!(doc.get(curr).map(|n| &n.data), Some(NodeData::Element(_))) {
                            if sel.matches_relative(curr, Some(node_id), doc) {
                                return true;
                            }
                            stack.extend(doc.children(curr).map(|c| c.id));
                        }
                    }
                }
            }
        } else {
            // Implicit descendant combinator: any descendant in node_id's subtree
            let mut stack: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            while let Some(curr) = stack.pop() {
                if matches!(doc.get(curr).map(|n| &n.data), Some(NodeData::Element(_))) {
                    if sel.tail.is_empty() {
                        if sel.head.matches(curr, doc) {
                            return true;
                        }
                    } else if sel.matches(curr, doc)
                        && leftmost_origin_within(curr, sel, node_id, doc)
                    {
                        return true;
                    }
                    stack.extend(doc.children(curr).map(|c| c.id));
                }
            }
        }
    }
    false
}

pub(crate) fn leftmost_origin_within(
    curr: NodeId,
    sel: &ComplexSelector,
    scope_id: NodeId,
    doc: &Document,
) -> bool {
    can_reach_origin_within(curr, &sel.tail, scope_id, doc)
}

pub(crate) fn can_reach_origin_within(
    curr: NodeId,
    tail_slice: &[(Combinator, CompoundSelector)],
    scope_id: NodeId,
    doc: &Document,
) -> bool {
    let Some(((comb, _), remaining)) = tail_slice.split_last() else {
        return curr == scope_id || is_descendant_of(curr, scope_id, doc);
    };

    match comb {
        Combinator::Child => {
            let Some(parent) = parent_element(curr, doc) else {
                return false;
            };
            can_reach_origin_within(parent, remaining, scope_id, doc)
        }
        Combinator::Descendant => {
            let mut p = curr;
            while let Some(parent) = parent_element(p, doc) {
                if can_reach_origin_within(parent, remaining, scope_id, doc) {
                    return true;
                }
                p = parent;
            }
            false
        }
        Combinator::NextSibling => {
            let Some(sib) = prev_element_sibling(curr, doc) else {
                return false;
            };
            can_reach_origin_within(sib, remaining, scope_id, doc)
        }
        Combinator::SubsequentSibling => {
            let mut sib = curr;
            while let Some(prev) = prev_element_sibling(sib, doc) {
                if can_reach_origin_within(prev, remaining, scope_id, doc) {
                    return true;
                }
                sib = prev;
            }
            false
        }
    }
}
