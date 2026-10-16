use deno_core::{op2, OpState};
use mango_html::dom::{NodeData, NodeId, QuirksMode};
use crate::dom_bindings::{
    adopt_children, clone_subtree, collect_elements_by_kind, collect_matching_elements,
    find_element_by_id_recursive, find_first_matching_element, jar_lock, query_selector_simple,
    CookieStore, DomDirtyFlag, LayoutBounds, SharedDocument,
};

// ── Cookies ──────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct PageUrl(pub String);

#[op2]
#[string]
pub fn op_mango_cookie_get(state: &mut OpState) -> String {
    let page_url = state
        .try_borrow::<PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_else(|| "about:blank".to_string());
    if let Some(cookie_jar) = state.try_borrow::<CookieStore>() {
        let jar = jar_lock(cookie_jar);
        if let Ok(url) = mango_net::Url::parse(&page_url) {
            return jar.script_cookie_string(&url);
        }
    }
    String::new()
}

#[op2(fast)]
pub fn op_mango_cookie_set(state: &mut OpState, #[string] cookie_str: String) {
    let page_url = state
        .try_borrow::<PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_else(|| "about:blank".to_string());
    if let Some(cookie_jar) = state.try_borrow::<CookieStore>() {
        let mut jar = jar_lock(cookie_jar);
        if let Ok(url) = mango_net::Url::parse(&page_url) {
            jar.set_from_script(&cookie_str, &url);
            jar.prune_expired();
        }
    }
}

// ── DOM Creation Ops ─────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_create_element(state: &mut OpState, #[string] tag_name: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    d.create_element(&tag_name, Vec::new()).raw() as u32
}

#[op2(fast)]
pub fn op_mango_create_text_node(state: &mut OpState, #[string] text: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    d.create_text(&text).raw() as u32
}

#[op2(fast)]
pub fn op_mango_create_comment(state: &mut OpState, #[string] comment: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    d.create_comment(&comment).raw() as u32
}

#[op2(fast)]
pub fn op_mango_create_fragment(state: &mut OpState) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    d.create_fragment().raw() as u32
}

#[op2(fast)]
pub fn op_mango_attach_shadow(state: &mut OpState, node_id: u32, #[string] _mode: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    if let Some(sr_id) = d.attach_shadow(NodeId::from_raw(node_id)) {
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
        sr_id.raw()
    } else {
        0
    }
}

#[op2(fast)]
pub fn op_mango_clone_node(state: &mut OpState, node_id: u32, deep: bool) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    clone_subtree(&mut d, NodeId::from_raw(node_id), deep)
        .map(|id| id.raw() as u32)
        .unwrap_or(0)
}

// ── DOM Query Ops ────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_get_element_by_id(state: &mut OpState, #[string] id: String) -> u32 {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return 0;
    };
    let d = doc.borrow();
    find_element_by_id_recursive(&d, d.root(), &id).map(|id| id.raw()).unwrap_or(0)
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
        NodeId::from_raw(root_id)
    };
    if let Some(selectors) = mango_css::parse_selectors(&selector)
        && let Some(found) = find_first_matching_element(&d, root, &selectors)
    {
        return found.raw();
    }
    query_selector_simple(&d, root, &selector).map(|id| id.raw()).unwrap_or(0)
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
        NodeId::from_raw(root_id)
    };
    let mut out = Vec::new();
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        collect_matching_elements(&d, root, &selectors, &mut out);
    }
    if out.is_empty() {
        collect_elements_by_kind(&d, root, &selector, &mut out);
    }
    out.into_iter().map(|id| id.raw()).collect()
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
    out.into_iter().map(|id| id.raw() as u32).collect()
}

// ── DOM Mutation Ops ─────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_append_child(state: &mut OpState, parent: u32, child: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    doc.borrow_mut()
        .append_child(NodeId::from_raw(parent), NodeId::from_raw(child));
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
    child
}

#[op2(fast)]
pub fn op_mango_remove_child(state: &mut OpState, _parent: u32, child: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    doc.borrow_mut()
        .remove_child_and_recycle(NodeId::from_raw(child));
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
    child
}

#[op2(fast)]
pub fn op_mango_insert_before(
    state: &mut OpState,
    parent: u32,
    new_child: u32,
    ref_child: u32,
) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    if ref_child > 0 {
        let _ = d.insert_before(
            NodeId::from_raw(parent),
            NodeId::from_raw(new_child),
            NodeId::from_raw(ref_child),
        );
    } else {
        d.append_child(NodeId::from_raw(parent), NodeId::from_raw(new_child));
    }
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
    new_child
}

#[op2(fast)]
pub fn op_mango_replace_child(
    state: &mut OpState,
    parent: u32,
    new_child: u32,
    old_child: u32,
) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    d.replace_child(
        NodeId::from_raw(parent),
        NodeId::from_raw(new_child),
        NodeId::from_raw(old_child),
    );
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
    old_child
}

// ── DOM Text & HTML Ops ──────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_get_text_content(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.text_content(NodeId::from_raw(node_id))
}

#[op2(fast)]
pub fn op_mango_set_text_content(state: &mut OpState, node_id: u32, #[string] text: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    let nid = NodeId::from_raw(node_id);
    let children: Vec<NodeId> = d.children(nid).map(|n| n.id).collect();
    for child in children {
        d.detach(child);
    }
    let text_node = d.create_text(&text);
    d.append_child(nid, text_node);
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2]
#[string]
pub fn op_mango_get_inner_html(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.serialize_children(NodeId::from_raw(node_id))
}

#[op2(fast)]
pub fn op_mango_set_inner_html(state: &mut OpState, node_id: u32, #[string] html: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    let target = NodeId::from_raw(node_id);

    let children: Vec<NodeId> = d.children(target).map(|n| n.id).collect();
    for c in children {
        d.remove_child_and_recycle(c);
    }

    if !html.is_empty() {
        let fragment = mango_html::parse_html(&format!("<html><body>{html}</body></html>"));
        if let Some(body) = fragment.find_element_by_tag(fragment.root(), "body") {
            adopt_children(&fragment, body, &mut d, target);
        }
    }

    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2]
#[string]
pub fn op_mango_get_outer_html(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.serialize_html(NodeId::from_raw(node_id))
}

#[op2(fast)]
pub fn op_mango_set_outer_html(state: &mut OpState, node_id: u32, #[string] html: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    let target = NodeId::from_raw(node_id);
    let Some(parent) = d.get(target).and_then(|n| n.parent) else {
        return;
    };
    let fragment = mango_html::parse_html(&format!("<html><body>{html}</body></html>"));
    if let Some(body) = fragment.find_element_by_tag(fragment.root(), "body") {
        crate::dom_bindings::adopt_children_before(&fragment, body, &mut d, parent, target);
    }
    d.remove_child_and_recycle(target);
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_get_template_content(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.template_contents(NodeId::from_raw(node_id))
        .map(|id| id.raw() as u32)
        .unwrap_or(0)
}

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
    d.get(NodeId::from_raw(node_id)).and_then(|n| match &n.data {
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
    if let Some(node) = d.get_mut(NodeId::from_raw(node_id))
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
    if let Some(node) = d.get_mut(NodeId::from_raw(node_id))
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
    d.get(NodeId::from_raw(node_id))
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
    d.get(NodeId::from_raw(node_id))
        .map(|n| match &n.data {
            NodeData::Element(el) => el.attributes.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

// ── DOM Metadata & Traversal ─────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_get_tag_name(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.get(NodeId::from_raw(node_id))
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
    d.get(NodeId::from_raw(node_id))
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
    d.get(NodeId::from_raw(node_id))
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
pub fn op_mango_get_node_value(state: &mut OpState, node_id: u32) -> Option<String> {
    let doc = state.try_borrow::<SharedDocument>()?;
    let d = doc.borrow();
    d.get(NodeId::from_raw(node_id)).and_then(|n| match &n.data {
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
    if let Some(node) = d.get_mut(NodeId::from_raw(node_id)) {
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
    d.get(NodeId::from_raw(node_id))
        .and_then(|n| n.parent)
        .and_then(|p| {
            let p_node = d.get(p)?;
            if matches!(p_node.data, NodeData::Element(_)) {
                Some(p.raw() as u32)
            } else {
                None
            }
        })
        .unwrap_or(0)
}

#[op2]
#[serde]
pub fn op_mango_get_children(state: &mut OpState, node_id: u32) -> Vec<u32> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    d.children(NodeId::from_raw(node_id))
        .filter_map(|c| {
            if matches!(c.data, NodeData::Element(_)) {
                Some(c.id.raw() as u32)
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
    d.children(NodeId::from_raw(node_id))
        .map(|c| c.id.raw() as u32)
        .collect()
}

#[op2(fast)]
pub fn op_mango_get_first_element_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.children(NodeId::from_raw(node_id))
        .find(|c| matches!(c.data, NodeData::Element(_)))
        .map(|c| c.id.raw() as u32)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_last_element_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.children(NodeId::from_raw(node_id))
        .filter(|c| matches!(c.data, NodeData::Element(_)))
        .last()
        .map(|c| c.id.raw() as u32)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_next_element_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    let nid = NodeId::from_raw(node_id);
    if let Some(node) = d.get(nid)
        && let Some(parent) = node.parent
    {
        let mut passed = false;
        for c in d.children(parent) {
            if passed {
                if matches!(c.data, NodeData::Element(_)) {
                    return c.id.raw() as u32;
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
    let nid = NodeId::from_raw(node_id);
    if let Some(node) = d.get(nid)
        && let Some(parent) = node.parent
    {
        let mut prev = 0;
        for c in d.children(parent) {
            if c.id.matches(nid) {
                return prev;
            }
            if matches!(c.data, NodeData::Element(_)) {
                prev = c.id.raw() as u32;
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
    d.get(NodeId::from_raw(node_id))
        .and_then(|n| n.first_child)
        .map(|c| c.raw() as u32)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_last_child(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(NodeId::from_raw(node_id))
        .and_then(|n| n.last_child)
        .map(|c| c.raw() as u32)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_next_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(NodeId::from_raw(node_id))
        .and_then(|n| n.next_sibling)
        .map(|c| c.raw() as u32)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_get_previous_sibling(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    d.get(NodeId::from_raw(node_id))
        .and_then(|n| n.prev_sibling)
        .map(|c| c.raw() as u32)
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
    d.get(NodeId::from_raw(node_id)).and_then(|n| match &n.data {
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
    if let Some(node) = d.get_mut(NodeId::from_raw(node_id))
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
pub fn op_mango_remove_dataset_value(
    state: &mut OpState,
    node_id: u32,
    #[string] key: String,
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
    if let Some(node) = d.get_mut(NodeId::from_raw(node_id))
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
    match d.get(NodeId::from_raw(node_id)).map(|n| &n.data) {
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

// ── Element Matches / Contains / Closest ─────────────────────────────────────

#[op2(fast)]
pub fn op_mango_element_contains(state: &mut OpState, parent: u32, child: u32) -> bool {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return false,
    };
    let d = doc.borrow();
    if d.get(NodeId::from_raw(parent)).is_none() {
        return false;
    }
    let mut cur = Some(NodeId::from_raw(child));
    while let Some(id) = cur {
        if id.raw() == parent {
            return true;
        }
        cur = d.get(id).and_then(|n| n.parent);
    }
    false
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
    let nid = NodeId::from_raw(node_id);
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
        let mut curr = Some(NodeId::from_raw(node_id));
        while let Some(id) = curr {
            if matches!(d.get(id).map(|n| &n.data), Some(NodeData::Element(_)))
                && selectors.matches(id, &d)
            {
                return id.raw() as u32;
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
    if let Some(bounds) = state.try_borrow::<LayoutBounds>()
        && let Some(&rect) = bounds.borrow().get(&node_id)
    {
        return rect.to_vec();
    }
    vec![0.0, 0.0, 0.0, 0.0]
}

#[op2(fast)]
pub fn op_mango_element_from_point(state: &mut OpState, x: f32, y: f32) -> u32 {
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
        if let Some((id, _)) = best {
            return id;
        }
    }
    0
}

#[op2]
#[serde]
pub fn op_mango_elements_from_point(state: &mut OpState, x: f32, y: f32) -> Vec<u32> {
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
    matches.into_iter().map(|(id, _)| id).collect()
}

#[op2(fast)]
pub fn op_mango_set_focus_state(_state: &mut OpState, _node_id: u32, _focused: bool) {}

// ── Canvas 2D Ops ────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_canvas_resize(state: &mut OpState, node_id: u32, width: u32, height: u32) {
    mango_render::resize_canvas(node_id as usize, width, height);
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_set_fill_style(_state: &mut OpState, node_id: u32, #[string] color_str: String) {
    let color = mango_render::color::parse_color(&color_str).unwrap_or(mango_core::Color::BLACK);
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_fill_style(color);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_stroke_style(_state: &mut OpState, node_id: u32, #[string] color_str: String) {
    let color = mango_render::color::parse_color(&color_str).unwrap_or(mango_core::Color::BLACK);
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_stroke_style(color);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_line_width(_state: &mut OpState, node_id: u32, width: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_line_width(width);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_line_cap(_state: &mut OpState, node_id: u32, #[string] cap: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_line_cap(&cap);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_line_join(_state: &mut OpState, node_id: u32, #[string] join: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_line_join(&join);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_miter_limit(_state: &mut OpState, node_id: u32, limit: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_miter_limit(limit);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_global_alpha(_state: &mut OpState, node_id: u32, alpha: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_global_alpha(alpha);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_font(_state: &mut OpState, node_id: u32, #[string] font: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_font(&font);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_text_align(_state: &mut OpState, node_id: u32, #[string] align: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_text_align(&align);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_text_baseline(_state: &mut OpState, node_id: u32, #[string] baseline: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_text_baseline(&baseline);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_fill_rect(state: &mut OpState, node_id: u32, x: f32, y: f32, w: f32, h: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.fill_rect(x, y, w, h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_stroke_rect(state: &mut OpState, node_id: u32, x: f32, y: f32, w: f32, h: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.stroke_rect(x, y, w, h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_clear_rect(state: &mut OpState, node_id: u32, x: f32, y: f32, w: f32, h: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.clear_rect(x, y, w, h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_begin_path(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.begin_path();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_close_path(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.close_path();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_move_to(_state: &mut OpState, node_id: u32, x: f32, y: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.move_to(x, y);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_line_to(_state: &mut OpState, node_id: u32, x: f32, y: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.line_to(x, y);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_arc(
    _state: &mut OpState,
    node_id: u32,
    x: f32,
    y: f32,
    r: f32,
    sa: f32,
    ea: f32,
    ccw: bool,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.arc(x, y, r, sa, ea, ccw);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_fill(state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.fill();
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_stroke(state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.stroke();
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_fill_text(
    state: &mut OpState,
    node_id: u32,
    #[string] text: String,
    x: f32,
    y: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.fill_text(&text, x, y, None);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_stroke_text(
    state: &mut OpState,
    node_id: u32,
    #[string] text: String,
    x: f32,
    y: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.stroke_text(&text, x, y, None);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_measure_text(_state: &mut OpState, node_id: u32, #[string] text: String) -> f32 {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| c.measure_text(&text)).unwrap_or(0.0)
}

#[op2(fast)]
pub fn op_mango_canvas_save(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.save();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_restore(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.restore();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_translate(_state: &mut OpState, node_id: u32, x: f32, y: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.translate(x, y);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_rotate(_state: &mut OpState, node_id: u32, angle: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.rotate(angle);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_scale(_state: &mut OpState, node_id: u32, sx: f32, sy: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.scale(sx, sy);
    });
}

#[op2]
#[serde]
pub fn op_mango_canvas_get_image_data(
    _state: &mut OpState,
    node_id: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.get_image_data(x as i32, y as i32, w, h)
    })
    .unwrap_or_else(|| vec![0u8; (w * h * 4) as usize])
}

#[op2]
pub fn op_mango_canvas_put_image_data(
    state: &mut OpState,
    node_id: u32,
    #[serde] data: Vec<u8>,
    dx: i32,
    dy: i32,
    dirty_x: i32,
    dirty_y: i32,
    dirty_w: u32,
    dirty_h: u32,
    src_width: u32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.put_image_data_with_stride(
            &data, src_width, dx, dy, dirty_x, dirty_y, dirty_w, dirty_h,
        );
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2]
#[string]
pub fn op_mango_canvas_to_data_url(_state: &mut OpState, node_id: u32) -> String {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.to_data_url("image/png")
    })
    .unwrap_or_default()
}
