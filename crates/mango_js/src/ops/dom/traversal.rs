use deno_core::{OpState, op2};
use mango_html::dom::{Document, NodeData, QuirksMode, node_id_from_js, node_id_to_js};

use crate::dom_bindings::{DomDirtyFlag, SharedDocument};

// ── DOM Metadata & Traversal ─────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_get_tag_name(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .map(|n| match &n.data {
            NodeData::Element(el) => el.tag_name.to_ascii_uppercase(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

#[op2(fast)]
pub fn op_mango_get_node_type(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .map(|n| match &n.data {
            NodeData::Element(_) => 1,
            NodeData::Text(_) => 3,
            NodeData::Comment(_) => 8,
            NodeData::Document => 9,
            NodeData::DocumentType { .. } => 10,
            NodeData::DocumentFragment => 11,
        })
        .unwrap_or(0)
}

#[op2]
#[string]
pub fn op_mango_get_node_name(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .map(|n| match &n.data {
            NodeData::Element(el) => el.tag_name.to_ascii_uppercase(),
            NodeData::Text(_) => "#text".to_string(),
            NodeData::Comment(_) => "#comment".to_string(),
            NodeData::Document => "#document".to_string(),
            NodeData::DocumentType { name, .. } => name.clone(),
            NodeData::DocumentFragment => "#document-fragment".to_string(),
        })
        .unwrap_or_default()
}

#[op2]
#[string]
pub fn op_mango_get_doctype_public_id(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| match &n.data {
            NodeData::DocumentType { public_id, .. } => Some(public_id.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[op2]
#[string]
pub fn op_mango_get_doctype_system_id(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| match &n.data {
            NodeData::DocumentType { system_id, .. } => Some(system_id.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[op2]
#[string]
pub fn op_mango_get_node_value(state: &mut OpState, node_id: u32) -> Option<String> {
    let doc = state.try_borrow::<SharedDocument>()?;
    let d = doc.borrow();
    d.get(node_id_from_js(node_id)).and_then(|n| match &n.data {
        NodeData::Text(t) => Some(t.clone()),
        NodeData::Comment(c) => Some(c.clone()),
        _ => None,
    })
}

#[op2(fast)]
pub fn op_mango_set_node_value(state: &mut OpState, node_id: u32, #[string] val: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    if let Some(node) = d.get_mut(node_id_from_js(node_id)) {
        match &mut node.data {
            NodeData::Text(t) => *t = val,
            NodeData::Comment(c) => *c = val,
            _ => {}
        }
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
    }
}

#[op2(fast)]
pub fn op_mango_get_parent(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| n.parent)
        .and_then(|p| {
            let p_node = d.get(p)?;
            if matches!(p_node.data, NodeData::Element(_)) {
                Some(node_id_to_js(p))
            } else {
                None
            }
        })
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_parent_node(state: &mut OpState, node_id: u32) -> i32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return -1,
    };
    let d = doc.borrow();
    match d.get(node_id_from_js(node_id)).and_then(|n| n.parent) {
        Some(p) => node_id_to_js(p) as i32,
        None => -1,
    }
}

#[op2]
#[serde]
pub fn op_mango_get_children(state: &mut OpState, node_id: u32) -> Vec<u32> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    d.children(node_id_from_js(node_id))
        .filter_map(|c| {
            if matches!(c.data, NodeData::Element(_)) {
                Some(node_id_to_js(c.id))
            } else {
                None
            }
        })
        .collect()
}

#[op2]
#[serde]
pub fn op_mango_get_child_node_ids(state: &mut OpState, node_id: u32) -> Vec<u32> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    d.children(node_id_from_js(node_id))
        .map(|c| node_id_to_js(c.id))
        .collect()
}

#[op2(fast)]
pub fn op_mango_get_first_element_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.children(node_id_from_js(node_id))
        .find(|c| matches!(c.data, NodeData::Element(_)))
        .map(|c| node_id_to_js(c.id))
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_last_element_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.children(node_id_from_js(node_id))
        .filter(|c| matches!(c.data, NodeData::Element(_)))
        .last()
        .map(|c| node_id_to_js(c.id))
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_next_element_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    let nid = node_id_from_js(node_id);
    if let Some(node) = d.get(nid)
        && let Some(parent) = node.parent
    {
        let mut passed = false;
        for c in d.children(parent) {
            if passed {
                if matches!(c.data, NodeData::Element(_)) {
                    return node_id_to_js(c.id);
                }
            } else if c.id.matches(nid) {
                passed = true;
            }
        }
    }
    0
}

#[op2(fast)]
pub fn op_mango_get_previous_element_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    let nid = node_id_from_js(node_id);
    if let Some(node) = d.get(nid)
        && let Some(parent) = node.parent
    {
        let mut prev = 0;
        for c in d.children(parent) {
            if c.id.matches(nid) {
                return prev;
            }
            if matches!(c.data, NodeData::Element(_)) {
                prev = node_id_to_js(c.id);
            }
        }
    }
    0
}

#[op2(fast)]
pub fn op_mango_get_first_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| n.first_child)
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_last_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| n.last_child)
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_next_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| n.next_sibling)
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_previous_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(node_id_from_js(node_id))
        .and_then(|n| n.prev_sibling)
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2]
#[string]
pub fn op_mango_get_compat_mode(state: &mut OpState) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return "CSS1Compat".to_string(),
    };
    let d = doc.borrow();
    match d.quirks_mode {
        QuirksMode::Quirks => "BackCompat".to_string(),
        _ => "CSS1Compat".to_string(),
    }
}

#[op2]
#[string]
pub fn op_mango_get_character_set(state: &mut OpState) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return "UTF-8".to_string(),
    };
    doc.borrow().character_set.clone()
}

#[op2]
#[string]
pub fn op_mango_get_content_type(state: &mut OpState) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return "text/html".to_string(),
    };
    doc.borrow().content_type.clone()
}

#[op2(fast)]
pub fn op_mango_element_contains(state: &mut OpState, parent: u32, child: u32) -> bool {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return false,
    };
    let d = doc.borrow();
    let parent_id = node_id_from_js(parent);
    let child_id = node_id_from_js(child);
    if d.get(parent_id).is_none() || d.get(child_id).is_none() {
        return false;
    }
    d.is_ancestor(parent_id, child_id)
}

#[op2(fast)]
pub fn op_mango_compare_document_position(state: &mut OpState, node_a: u32, node_b: u32) -> u16 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => {
            return Document::DOCUMENT_POSITION_DISCONNECTED
                | Document::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC
                | 4;
        }
    };
    let d = doc.borrow();
    d.compare_document_position(node_id_from_js(node_a), node_id_from_js(node_b))
}
