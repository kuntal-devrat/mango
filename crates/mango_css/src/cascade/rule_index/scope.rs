//! Scoping chain matching for CSS `@scope` rules.

use mango_html::dom::{Document, NodeId};

use crate::cascade::types::ScopeDef;
use crate::parser::StyleRule;

pub(crate) fn selector_targets_scope(sel_list: Option<&crate::selectors::SelectorList>) -> bool {
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
        })
    })
}

pub(crate) fn subject_targets_scope(style_rule: &StyleRule) -> bool {
    selector_targets_scope(Some(&style_rule.selectors))
}

pub(crate) fn selector_targets_host(sel: &crate::selectors::ComplexSelector) -> bool {
    let subject = if let Some((_, compound)) = sel.tail.last() {
        compound
    } else {
        &sel.head
    };
    subject.simple_selectors.iter().any(|s| match s {
        crate::selectors::SimpleSelector::Nesting => true,
        crate::selectors::SimpleSelector::PseudoClass(p) => {
            let lower = p.to_ascii_lowercase();
            lower == "host"
                || lower.starts_with("host(")
                || lower.starts_with("host-context(")
                || lower == "scope"
                || ((lower.starts_with("is(") || lower.starts_with("where("))
                    && (lower.contains("host") || lower.contains("scope") || lower.contains('&')))
        }
        _ => false,
    })
}

pub(crate) fn match_scope_chain(
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
                if target_node.raw() == root_cand.raw() && !selector_targets_host(sel) {
                    return false;
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
