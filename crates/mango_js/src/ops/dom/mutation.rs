use deno_core::{OpState, op2};
use mango_html::dom::{NodeData, NodeId, node_id_from_js, node_id_to_js};

use crate::dom_bindings::{
    DomDirtyFlag, SharedDocument, adopt_children, adopt_children_before, clone_subtree,
};

#[op2(fast)]
pub fn op_mango_create_element(state: &mut OpState, #[string] tag_name: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    node_id_to_js(d.create_element(&tag_name, Vec::new()))
}

#[op2(fast)]
pub fn op_mango_create_text_node(state: &mut OpState, #[string] text: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    node_id_to_js(d.create_text(&text))
}

#[op2(fast)]
pub fn op_mango_create_comment(state: &mut OpState, #[string] comment: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    node_id_to_js(d.create_comment(&comment))
}

#[op2(fast)]
pub fn op_mango_create_fragment(state: &mut OpState) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    node_id_to_js(d.create_fragment())
}

#[op2(fast)]
pub fn op_mango_attach_shadow(state: &mut OpState, node_id: u32, #[string] mode: String) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    if let Some(sr_id) = d.attach_shadow_with_options(node_id_from_js(node_id), &mode, false, false)
    {
        if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
            *dirty.borrow_mut() = true;
        }
        node_id_to_js(sr_id)
    } else {
        0
    }
}

#[op2(fast)]
pub fn op_mango_get_shadow_root(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let d = doc.borrow();
    d.get_shadow_root(node_id_from_js(node_id))
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2]
#[string]
pub fn op_mango_get_shadow_root_mode(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return String::new(),
    };
    let d = doc.borrow();
    d.get_shadow_root_mode(node_id_from_js(node_id))
        .unwrap_or("")
        .to_string()
}

#[op2(fast)]
pub fn op_mango_get_shadow_host(state: &mut OpState, node_id: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let d = doc.borrow();
    let nid = node_id_from_js(node_id);
    d.get_enclosing_shadow_root(nid)
        .and_then(|sr_id| d.get_host_for_shadow_root(sr_id))
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_clone_node(state: &mut OpState, node_id: u32, deep: bool) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    let mut d = doc.borrow_mut();
    clone_subtree(&mut d, node_id_from_js(node_id), deep)
        .map(node_id_to_js)
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_append_child(state: &mut OpState, parent: u32, child: u32) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return 0,
    };
    doc.borrow_mut()
        .append_child(node_id_from_js(parent), node_id_from_js(child));
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
    doc.borrow_mut().detach(node_id_from_js(child));
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
            node_id_from_js(parent),
            node_id_from_js(new_child),
            node_id_from_js(ref_child),
        );
    } else {
        d.append_child(node_id_from_js(parent), node_id_from_js(new_child));
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
    let res = d.replace_child(
        node_id_from_js(parent),
        node_id_from_js(new_child),
        node_id_from_js(old_child),
    );
    match res {
        Ok(ret) => {
            if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
                *dirty.borrow_mut() = true;
            }
            node_id_to_js(ret)
        }
        Err(_) => 0,
    }
}

#[op2(fast)]
pub fn op_mango_retain_js_wrapper(state: &mut OpState, node_id: u32) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    doc.borrow_mut().retain_js_wrapper(node_id_from_js(node_id));
}

#[op2(fast)]
pub fn op_mango_release_js_wrapper(state: &mut OpState, node_id: u32) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let doc_id = doc.borrow().id();
    let freed = doc
        .borrow_mut()
        .release_js_wrapper(node_id_from_js(node_id));
    for id in freed {
        // Canvas backing stores are document-scoped. Remove them as soon as
        // the node is recycled; otherwise detached canvases retain their full
        // pixel buffers for the lifetime of the process.
        mango_render::remove_canvas((doc_id << 32) | u64::from(id.raw()));
    }
}

#[op2]
#[string]
pub fn op_mango_get_text_content(state: &mut OpState, node_id: u32) -> String {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return String::new(),
    };
    let d = doc.borrow();
    d.text_content(node_id_from_js(node_id))
}

#[op2(fast)]
pub fn op_mango_set_text_content(state: &mut OpState, node_id: u32, #[string] text: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    let nid = node_id_from_js(node_id);
    let children: Vec<NodeId> = d.children(nid).map(|n| n.id).collect();
    for child in children {
        let _ = d.detach_and_recycle_if_unreferenced(child);
    }
    if !text.is_empty() {
        let text_node = d.create_text(&text);
        d.append_child(nid, text_node);
    }
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
    let target = node_id_from_js(node_id);
    let target_src = if let Some(NodeData::Element(el)) = d.get(target).map(|n| &n.data) {
        if el.tag_name.eq_ignore_ascii_case("template") {
            d.template_contents(target).unwrap_or(target)
        } else {
            target
        }
    } else {
        target
    };
    d.serialize_children(target_src)
}

#[op2(fast)]
pub fn op_mango_set_inner_html(state: &mut OpState, node_id: u32, #[string] html: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    let target = node_id_from_js(node_id);
    let target_dest = if let Some(NodeData::Element(el)) = d.get(target).map(|n| &n.data) {
        if el.tag_name.eq_ignore_ascii_case("template") {
            d.get_or_create_template_contents(target)
        } else {
            target
        }
    } else {
        target
    };

    let children: Vec<NodeId> = d.children(target_dest).map(|n| n.id).collect();
    for c in children {
        let _ = d.detach_and_recycle_if_unreferenced(c);
    }

    if !html.is_empty() {
        let fragment = mango_html::parse_html(&format!("<html><body>{html}</body></html>"));
        if let Some(body) = fragment.find_element_by_tag(fragment.root(), "body") {
            adopt_children(&fragment, body, &mut d, target_dest);
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
    d.serialize_html(node_id_from_js(node_id))
}

#[op2(fast)]
pub fn op_mango_set_outer_html(state: &mut OpState, node_id: u32, #[string] html: String) {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut d = doc.borrow_mut();
    let target = node_id_from_js(node_id);
    let Some(parent) = d.get(target).and_then(|n| n.parent) else {
        return;
    };
    let fragment = mango_html::parse_html(&format!("<html><body>{html}</body></html>"));
    if let Some(body) = fragment.find_element_by_tag(fragment.root(), "body") {
        adopt_children_before(&fragment, body, &mut d, parent, target);
    }
    let _ = d.detach_and_recycle_if_unreferenced(target);
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
    d.template_contents(node_id_from_js(node_id))
        .map(node_id_to_js)
        .unwrap_or(0)
}
