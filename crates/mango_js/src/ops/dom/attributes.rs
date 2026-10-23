use deno_core::{OpState, op2};
use mango_html::dom::{NodeData, node_id_from_js};

use crate::dom_bindings::{DomDirtyFlag, SharedDocument};

// ── DOM Attributes Ops ───────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_get_attribute(
    state: &mut OpState,
    node_id: u32,
    #[string] name: String,
) -> Option<String> {
    let doc = state.try_borrow::<SharedDocument>()?;
    let d = doc.borrow();
    d.get(node_id_from_js(node_id)).and_then(|n| match &n.data {
        NodeData::Element(el) => el.get_attribute(&name).map(|s| s.to_string()),
        _ => None,
    })
}

#[op2(fast)]
pub fn op_mango_set_attribute(
    state: &mut OpState,
    node_id: u32,
    #[string] name: String,
    #[string] val: String,
) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    if let Some(node) = d.get_mut(node_id_from_js(node_id))
        && let NodeData::Element(ref mut elem) = node.data
    {
        if let Some(attr) = elem.attributes.iter_mut().find(|(k, _)| k == &name) {
            attr.1 = val;
        } else {
            elem.attributes.push((name, val));
        }
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
    }
}

#[op2(fast)]
pub fn op_mango_remove_attribute(state: &mut OpState, node_id: u32, #[string] name: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    if let Some(node) = d.get_mut(node_id_from_js(node_id))
        && let NodeData::Element(ref mut elem) = node.data
    {
        elem.attributes.retain(|(k, _)| k != &name);
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
    }
}

#[op2(fast)]
pub fn op_mango_has_attribute(state: &mut OpState, node_id: u32, #[string] name: String) -> bool {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return false,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .map(|n| match &n.data {
            NodeData::Element(el) => el.get_attribute(&name).is_some(),
            _ => false,
        })
        .unwrap_or(false)
}

#[op2]
#[serde]
pub fn op_mango_get_attribute_names(state: &mut OpState, node_id: u32) -> Vec<String> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .map(|n| match &n.data {
            NodeData::Element(el) => el.attributes.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

// ── DOM Dataset Ops ──────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_get_dataset_value(
    state: &mut OpState,
    node_id: u32,
    #[string] key: String,
) -> Option<String> {
    let doc = state.try_borrow::<SharedDocument>()?;
    let d = doc.borrow();
    let mut attr_name = String::from("data-");
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            attr_name.push('-');
            attr_name.push(ch.to_ascii_lowercase());
        } else {
            attr_name.push(ch);
        }
    }
    d.get(node_id_from_js(node_id)).and_then(|n| match &n.data {
        NodeData::Element(el) => el.get_attribute(&attr_name).map(|s| s.to_string()),
        _ => None,
    })
}

#[op2(fast)]
pub fn op_mango_set_dataset_value(
    state: &mut OpState,
    node_id: u32,
    #[string] key: String,
    #[string] val: String,
) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut attr_name = String::from("data-");
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            attr_name.push('-');
            attr_name.push(ch.to_ascii_lowercase());
        } else {
            attr_name.push(ch);
        }
    }
    let mut d = doc.borrow_mut();
    if let Some(node) = d.get_mut(node_id_from_js(node_id))
        && let NodeData::Element(ref mut elem) = node.data
    {
        if let Some(attr) = elem.attributes.iter_mut().find(|(k, _)| k == &attr_name) {
            attr.1 = val;
        } else {
            elem.attributes.push((attr_name, val));
        }
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
    }
}

#[op2(fast)]
pub fn op_mango_remove_dataset_value(state: &mut OpState, node_id: u32, #[string] key: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut attr_name = String::from("data-");
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            attr_name.push('-');
            attr_name.push(ch.to_ascii_lowercase());
        } else {
            attr_name.push(ch);
        }
    }
    let mut d = doc.borrow_mut();
    if let Some(node) = d.get_mut(node_id_from_js(node_id))
        && let NodeData::Element(ref mut elem) = node.data
    {
        elem.attributes.retain(|(k, _)| k != &attr_name);
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
    }
}

#[op2]
#[serde]
pub fn op_mango_get_dataset_keys(state: &mut OpState, node_id: u32) -> Vec<String> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    match d.get(node_id_from_js(node_id)).map(|n| &n.data) {
        Some(NodeData::Element(el)) => el
            .attributes
            .iter()
            .filter_map(|(k, _)| {
                k.strip_prefix("data-").map(|rest| {
                    let mut camel = String::new();
                    let mut cap_next = false;
                    for ch in rest.chars() {
                        if ch == '-' {
                            cap_next = true;
                        } else if cap_next {
                            camel.push(ch.to_ascii_uppercase());
                            cap_next = false;
                        } else {
                            camel.push(ch);
                        }
                    }
                    camel
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}
