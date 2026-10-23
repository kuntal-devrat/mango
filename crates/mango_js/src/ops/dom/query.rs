use deno_core::{OpState, op2};
use mango_html::dom::{NodeData, node_id_from_js, node_id_to_js};

use crate::dom_bindings::{
    LayoutBounds, SharedDocument, collect_elements_by_kind, collect_matching_elements,
    find_element_by_id_recursive, find_first_matching_element, query_selector_simple,
};

// ── DOM Query Ops ────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_get_element_by_id(state: &mut OpState, #[string] id: String) -> u32 {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return 0;
    };
    let d = doc.borrow();
    find_element_by_id_recursive(&d, d.root(), &id)
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_query_selector(
    state: &mut OpState,
    root_id: u32,
    #[string] selector: String,
) -> u32 {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return 0;
    };
    let d = doc.borrow();
    let root = if root_id == 0 {
        d.root()
    } else {
        node_id_from_js(root_id)
    };
    mango_css::selectors::set_current_scope_root(Some(root));
    let res = if let Some(selectors) = mango_css::parse_selectors(&selector)
        && let Some(found) = find_first_matching_element(&d, root, &selectors)
    {
        node_id_to_js(found)
    } else {
        query_selector_simple(&d, root, &selector)
            .map(node_id_to_js)
            .unwrap_or(0)
    };
    mango_css::selectors::set_current_scope_root(None);
    res
}

#[op2]
#[serde]
pub fn op_mango_query_selector_all(
    state: &mut OpState,
    root_id: u32,
    #[string] selector: String,
) -> Vec<u32> {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return Vec::new();
    };
    let d = doc.borrow();
    let root = if root_id == 0 {
        d.root()
    } else {
        node_id_from_js(root_id)
    };
    mango_css::selectors::set_current_scope_root(Some(root));
    let mut out = Vec::new();
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        collect_matching_elements(&d, root, &selectors, &mut out);
    }
    if out.is_empty() {
        collect_elements_by_kind(&d, root, &selector, &mut out);
    }
    mango_css::selectors::set_current_scope_root(None);
    out.into_iter().map(node_id_to_js).collect()
}

#[op2]
#[serde]
pub fn op_mango_get_document_elements(state: &mut OpState, #[string] tag: String) -> Vec<u32> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    let mut out = Vec::new();
    collect_elements_by_kind(&d, d.root(), &tag, &mut out);
    out.into_iter().map(node_id_to_js).collect()
}

#[op2(fast)]
pub fn op_mango_element_matches(
    state: &mut OpState,
    node_id: u32,
    #[string] selector: String,
) -> bool {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return false,
    };
    let d = doc.borrow();
    let nid = node_id_from_js(node_id);
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        return selectors.matches(nid, &d);
    }
    false
}

#[op2(fast)]
pub fn op_mango_element_closest(
    state: &mut OpState,
    node_id: u32,
    #[string] selector: String,
) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        let mut curr = Some(node_id_from_js(node_id));
        while let Some(id) = curr {
            if matches!(d.get(id).map(|n| &n.data), Some(NodeData::Element(_)))
                && selectors.matches(id, &d)
            {
                return node_id_to_js(id);
            }
            curr = d.get(id).and_then(|n| n.parent);
        }
    }
    0
}

// ── Layout Measurement Ops ───────────────────────────────────────────────────

#[op2]
#[serde]
pub fn op_mango_get_element_rect(state: &mut OpState, node_id: u32) -> Vec<f32> {
    let raw = node_id_from_js(node_id).raw();
    if let Some(bounds) = state.try_borrow::<LayoutBounds>()
        && let Some(&rect) = bounds.borrow().get(&raw)
    {
        return rect.to_vec();
    }
    vec![0.0, 0.0, 0.0, 0.0]
}

#[op2(fast)]
pub fn op_mango_element_from_point(state: &mut OpState, x: f32, y: f32) -> u32 {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return 0;
    };
    let d = doc.borrow();
    if let Some(bounds) = state.try_borrow::<LayoutBounds>() {
        let b = bounds.borrow();
        let mut best: Option<(u32, f32)> = None;
        for (&node_id, &[bx, by, bw, bh]) in b.iter() {
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                let area = bw * bh;
                if best.is_none_or(|(_, best_area)| area < best_area) {
                    best = Some((node_id, area));
                }
            }
        }
        if let Some((raw_id, _)) = best
            && let Some(id) = d.node_id_at_raw(raw_id)
        {
            return node_id_to_js(id);
        }
    }
    0
}

#[op2]
#[serde]
pub fn op_mango_elements_from_point(state: &mut OpState, x: f32, y: f32) -> Vec<u32> {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return Vec::new();
    };
    let d = doc.borrow();
    let mut matches: Vec<(u32, f32)> = Vec::new();
    if let Some(bounds) = state.try_borrow::<LayoutBounds>() {
        let b = bounds.borrow();
        for (&node_id, &[bx, by, bw, bh]) in b.iter() {
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                let area = bw * bh;
                matches.push((node_id, area));
            }
        }
    }
    matches.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    matches
        .into_iter()
        .filter_map(|(raw_id, _)| d.node_id_at_raw(raw_id).map(node_id_to_js))
        .collect()
}

#[op2(fast)]
pub fn op_mango_set_focus_state(_state: &mut OpState, _node_id: u32, _focused: bool) {}
