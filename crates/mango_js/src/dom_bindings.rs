//! DOM API bindings that bridge Boa JS engine to Mango's arena-based DOM.
//!
//! Registers native functions on the Boa context so JavaScript can call
//! `document.getElementById()`, `document.createElement()`, `element.textContent`, etc.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsArgs, JsValue, NativeFunction};
use mango_html::dom::{Document, NodeData, NodeId, QuirksMode};

/// Shared document reference accessible from JS callbacks.
pub type SharedDocument = Rc<RefCell<Document>>;

/// Flag indicating the DOM was mutated and needs re-layout.
pub type DomDirtyFlag = Rc<RefCell<bool>>;

/// Published layout geometry: DOM node id → `[x, y, width, height]` in viewport pixels.
///
/// The browser refreshes this after every layout so that JS measurement APIs
/// (`getBoundingClientRect`, `offsetWidth`, `elementFromPoint`) return real values
/// without `mango_js` depending on `mango_layout`.
pub type LayoutBounds = Rc<RefCell<std::collections::HashMap<u32, [f32; 4]>>>;

/// Shared cookie jar backing `document.cookie`.
///
/// The browser owns the jar (it loads and saves it from the profile) and shares
/// that exact handle with the HTTP loader and this runtime, so a script write is
/// visible to the very next request with no copy step (GAP-016).
pub type CookieStore = std::sync::Arc<std::sync::Mutex<mango_net::CookieJar>>;

fn jar_lock(store: &CookieStore) -> std::sync::MutexGuard<'_, mango_net::CookieJar> {
    store.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Helper macro to create an unsafe NativeFunction::from_closure.
/// SAFETY: Mango is single-threaded; closures capture only Rc<RefCell<_>> values.
macro_rules! native_fn {
    ($closure:expr) => {
        unsafe { NativeFunction::from_closure($closure) }
    };
}

/// Registers the `document` object with DOM query and mutation methods.
pub fn register_document_api(
    context: &mut Context,
    doc: SharedDocument,
    dirty: DomDirtyFlag,
    layout_bounds: LayoutBounds,
    cookie_jar: CookieStore,
    page_url: String,
) {
    // document.getElementById(id)
    let doc_ref = doc.clone();
    let get_element_by_id = native_fn!(move |_this, args, ctx| {
        let id_str = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let document = doc_ref.borrow();
        let root = document.root();
        match find_element_by_id_recursive(&document, root, &id_str) {
            Some(node_id) => Ok(JsValue::from(node_id.raw() as i32)),
            None => Ok(JsValue::null()),
        }
    });

    // document.querySelector(selector) / element.querySelector(selector)
    let doc_ref = doc.clone();
    let query_selector = native_fn!(move |_this, args, ctx| {
        let (scope_idx, sel_arg_idx) = if args.len() > 1 {
            (args.get_or_undefined(0).to_u32(ctx).unwrap_or(0), 1)
        } else {
            (0, 0)
        };
        let sel_str = args
            .get_or_undefined(sel_arg_idx)
            .to_string(ctx)?
            .to_std_string_escaped();
        let document = doc_ref.borrow();
        let scope_id = if scope_idx == 0 {
            document.root()
        } else {
            NodeId::from_raw(scope_idx)
        };
        if let Some(selectors) = mango_css::parse_selectors(&sel_str) {
            if let Some(found) = find_first_matching_element(&document, scope_id, &selectors) {
                return Ok(JsValue::from(found.raw() as i32));
            }
        }
        match query_selector_simple(&document, scope_id, &sel_str) {
            Some(node_id) => Ok(JsValue::from(node_id.raw() as i32)),
            None => Ok(JsValue::null()),
        }
    });

    // document.querySelectorAll(selector) / element.querySelectorAll(selector)
    let doc_ref = doc.clone();
    let query_selector_all = native_fn!(move |_this, args, ctx| {
        let (scope_idx, sel_arg_idx) = if args.len() > 1 {
            (args.get_or_undefined(0).to_u32(ctx).unwrap_or(0), 1)
        } else {
            (0, 0)
        };
        let sel_str = args
            .get_or_undefined(sel_arg_idx)
            .to_string(ctx)?
            .to_std_string_escaped();
        let document = doc_ref.borrow();
        let scope_id = if scope_idx == 0 {
            document.root()
        } else {
            NodeId::from_raw(scope_idx)
        };
        let mut matched = Vec::new();
        if let Some(selectors) = mango_css::parse_selectors(&sel_str) {
            collect_matching_elements(&document, scope_id, &selectors, &mut matched);
        }
        let values: Vec<JsValue> = matched.into_iter().map(|id| JsValue::from(id.raw() as i32)).collect();
        let js_array = JsArray::from_iter(values, ctx);
        Ok(js_array.into())
    });

    // document.createElement(tagName)
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let create_element = native_fn!(move |_this, args, ctx| {
        let tag = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let mut document = doc_ref.borrow_mut();
        let node_id = document.create_element(&tag, vec![]);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::from(node_id.raw() as i32))
    });

    // document.createTextNode(text)
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let create_text_node = native_fn!(move |_this, args, ctx| {
        let text = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let mut document = doc_ref.borrow_mut();
        let node_id = document.create_text(&text);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::from(node_id.raw() as i32))
    });

    // Register document object with methods
    let doc_obj = boa_engine::object::ObjectInitializer::new(context)
        .function(get_element_by_id, boa_engine::js_string!("getElementById"), 1)
        .function(query_selector.clone(), boa_engine::js_string!("querySelector"), 1)
        .function(query_selector_all.clone(), boa_engine::js_string!("querySelectorAll"), 1)
        .function(create_element, boa_engine::js_string!("createElement"), 1)
        .function(create_text_node, boa_engine::js_string!("createTextNode"), 1)
        .build();

    context
        .register_global_property(
            boa_engine::js_string!("document"),
            doc_obj,
            boa_engine::property::Attribute::all(),
        )
        .expect("register document");

    // --- Helper functions for element manipulation ---

    let doc_ref = doc.clone();
    let get_text_content = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        let text = document.text_content(node_id);
        Ok(JsValue::from(boa_engine::js_string!(text)))
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_text_content = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let text = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let node_id = NodeId::from_raw(node_idx);
        let mut document = doc_ref.borrow_mut();
        let children: Vec<NodeId> = document.children(node_id).map(|n| n.id).collect();
        for child in children {
            document.detach(child);
        }
        let text_node = document.create_text(&text);
        document.append_child(node_id, text_node);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let append_child = native_fn!(move |_this, args, ctx| {
        let parent_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let child_idx = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0);
        let mut document = doc_ref.borrow_mut();
        document.append_child(NodeId::from_raw(parent_idx), NodeId::from_raw(child_idx));
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let remove_child = native_fn!(move |_this, args, ctx| {
        let child_idx = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0);
        let mut document = doc_ref.borrow_mut();
        document.detach(NodeId::from_raw(child_idx));
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_attribute = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let name = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let value = args.get_or_undefined(2).to_string(ctx)?.to_std_string_escaped();
        let node_id = NodeId::from_raw(node_idx);
        let mut document = doc_ref.borrow_mut();
        if let Some(node) = document.get_mut(node_id)
            && let NodeData::Element(ref mut elem) = node.data {
                if let Some(attr) = elem.attributes.iter_mut().find(|(k, _)| k == &name) {
                    attr.1 = value;
                } else {
                    elem.attributes.push((name, value));
                }
                *dirty_ref.borrow_mut() = true;
            }
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let remove_attribute = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let name = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let node_id = NodeId::from_raw(node_idx);
        let mut document = doc_ref.borrow_mut();
        if let Some(node) = document.get_mut(node_id)
            && let NodeData::Element(ref mut elem) = node.data {
                elem.attributes.retain(|(k, _)| k != &name);
                *dirty_ref.borrow_mut() = true;
            }
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let get_attribute = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let name = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let NodeData::Element(ref elem) = node.data
                && let Some(val) = elem.get_attribute(&name) {
                    return Ok(JsValue::from(boa_engine::js_string!(val)));
                }
        Ok(JsValue::null())
    });

    let doc_ref = doc.clone();
    let get_tag_name = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let NodeData::Element(ref elem) = node.data {
                return Ok(JsValue::from(boa_engine::js_string!(elem.tag_name.to_uppercase())));
            }
        Ok(JsValue::from(boa_engine::js_string!("")))
    });

    let doc_ref = doc.clone();
    let element_matches = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let sel_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(selectors) = mango_css::parse_selectors(&sel_str) {
            return Ok(JsValue::from(selectors.matches(node_id, &document)));
        }
        if let Some(node) = document.get(node_id)
            && let NodeData::Element(elem) = &node.data {
                let s = sel_str.trim();
                if let Some(id) = s.strip_prefix('#') {
                    return Ok(JsValue::from(elem.id() == Some(id)));
                }
                if let Some(cls) = s.strip_prefix('.') {
                    return Ok(JsValue::from(elem.has_class(cls)));
                }
                return Ok(JsValue::from(elem.tag_name.eq_ignore_ascii_case(s)));
            }
        Ok(JsValue::from(false))
    });

    let doc_ref = doc.clone();
    let element_closest = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let sel_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let mut curr_id = Some(NodeId::from_raw(node_idx));
        let document = doc_ref.borrow();
        let selectors = mango_css::parse_selectors(&sel_str);
        while let Some(id) = curr_id {
            if let Some(node) = document.get(id) {
                if let NodeData::Element(elem) = &node.data {
                    if let Some(ref sel) = selectors {
                        if sel.matches(id, &document) {
                            return Ok(JsValue::from(id.raw() as i32));
                        }
                    } else {
                        let s = sel_str.trim();
                        if let Some(target_id) = s.strip_prefix('#') {
                            if elem.id() == Some(target_id) {
                                return Ok(JsValue::from(id.raw() as i32));
                            }
                        } else if let Some(cls) = s.strip_prefix('.') {
                            if elem.has_class(cls) {
                                return Ok(JsValue::from(id.raw() as i32));
                            }
                        } else if elem.tag_name.eq_ignore_ascii_case(s) {
                            return Ok(JsValue::from(id.raw() as i32));
                        }
                    }
                }
                curr_id = node.parent;
            } else {
                break;
            }
        }
        Ok(JsValue::null())
    });

    let doc_ref = doc.clone();
    let get_compat_mode = native_fn!(move |_this, _args, _ctx| {
        let document = doc_ref.borrow();
        let mode = match document.quirks_mode {
            QuirksMode::Quirks => "BackCompat",
            QuirksMode::NoQuirks | QuirksMode::LimitedQuirks => "CSS1Compat",
        };
        Ok(JsValue::from(boa_engine::js_string!(mode)))
    });

    let doc_ref = doc.clone();
    let get_character_set = native_fn!(move |_this, _args, _ctx| {
        let document = doc_ref.borrow();
        Ok(JsValue::from(boa_engine::js_string!(document.character_set.clone())))
    });

    let doc_ref = doc.clone();
    let get_content_type = native_fn!(move |_this, _args, _ctx| {
        let document = doc_ref.borrow();
        Ok(JsValue::from(boa_engine::js_string!(document.content_type.clone())))
    });

    let doc_ref = doc.clone();
    let get_parent = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let Some(parent) = node.parent {
                if let Some(p_node) = document.get(parent) && matches!(p_node.data, NodeData::Element(_)) {
                    return Ok(JsValue::from(parent.raw() as i32));
                }
            }
        Ok(JsValue::null())
    });

    let doc_ref = doc.clone();
    let get_children = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        let mut elem_children = Vec::new();
        for child in document.children(node_id) {
            if matches!(child.data, NodeData::Element(_)) {
                elem_children.push(JsValue::from(child.id.raw() as i32));
            }
        }
        let js_array = JsArray::from_iter(elem_children, ctx);
        Ok(js_array.into())
    });

    // _getChildNodeIds(nodeId) → every child node including text and comment nodes
    let doc_ref = doc.clone();
    let get_child_node_ids = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        let ids: Vec<JsValue> = document
            .children(NodeId::from_raw(node_idx))
            .map(|n| JsValue::from(n.id.raw() as i32))
            .collect();
        Ok(JsArray::from_iter(ids, ctx).into())
    });

    let doc_ref = doc.clone();
    let get_first_element_child = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        for child in document.children(node_id) {
            if matches!(child.data, NodeData::Element(_)) {
                return Ok(JsValue::from(child.id.raw() as i32));
            }
        }
        Ok(JsValue::null())
    });

    let doc_ref = doc.clone();
    let get_last_element_child = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        let mut last = None;
        for child in document.children(node_id) {
            if matches!(child.data, NodeData::Element(_)) {
                last = Some(child.id);
            }
        }
        if let Some(id) = last {
            Ok(JsValue::from(id.raw() as i32))
        } else {
            Ok(JsValue::null())
        }
    });

    let doc_ref = doc.clone();
    let get_next_element_sibling = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        let mut curr = document.get(node_id).and_then(|n| n.next_sibling);
        while let Some(id) = curr {
            if let Some(node) = document.get(id) {
                if matches!(node.data, NodeData::Element(_)) {
                    return Ok(JsValue::from(id.raw() as i32));
                }
                curr = node.next_sibling;
            } else {
                break;
            }
        }
        Ok(JsValue::null())
    });

    let doc_ref = doc.clone();
    let get_previous_element_sibling = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        let mut curr = document.get(node_id).and_then(|n| n.prev_sibling);
        while let Some(id) = curr {
            if let Some(node) = document.get(id) {
                if matches!(node.data, NodeData::Element(_)) {
                    return Ok(JsValue::from(id.raw() as i32));
                }
                curr = node.prev_sibling;
            } else {
                break;
            }
        }
        Ok(JsValue::null())
    });

    let doc_ref = doc.clone();
    let get_first_child = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let Some(fc) = node.first_child {
            Ok(JsValue::from(fc.raw() as i32))
        } else {
            Ok(JsValue::null())
        }
    });

    let doc_ref = doc.clone();
    let get_last_child = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let Some(lc) = node.last_child {
            Ok(JsValue::from(lc.raw() as i32))
        } else {
            Ok(JsValue::null())
        }
    });

    let doc_ref = doc.clone();
    let get_next_sibling = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let Some(ns) = node.next_sibling {
            Ok(JsValue::from(ns.raw() as i32))
        } else {
            Ok(JsValue::null())
        }
    });

    let doc_ref = doc.clone();
    let get_previous_sibling = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let node_id = NodeId::from_raw(node_idx);
        let document = doc_ref.borrow();
        if let Some(node) = document.get(node_id)
            && let Some(ps) = node.prev_sibling {
            Ok(JsValue::from(ps.raw() as i32))
        } else {
            Ok(JsValue::null())
        }
    });

    // ── DOM API completeness bindings (Phase 1.5 / 7.4) ───────────────────────

    // _cloneNode(nodeId, deep) → new node id
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let clone_node = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let deep = args.get_or_undefined(1).to_boolean();
        let mut document = doc_ref.borrow_mut();
        let new_id = clone_subtree(&mut document, NodeId::from_raw(node_idx), deep);
        *dirty_ref.borrow_mut() = true;
        Ok(match new_id {
            Some(id) => JsValue::from(id.raw() as i32),
            None => JsValue::null(),
        })
    });

    // _createFragment() → document fragment node id
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let create_fragment = native_fn!(move |_this, _args, _ctx| {
        let mut document = doc_ref.borrow_mut();
        let node_id = document.create_fragment();
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::from(node_id.raw() as i32))
    });

    // _attachShadow(hostId) -> shadow root fragment node id (GAP-012)
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let attach_shadow_native = native_fn!(move |_this, args, ctx| {
        let host_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let mut document = doc_ref.borrow_mut();
        let host_id = NodeId::from_raw(host_idx);
        if let Some(sr_id) = document.attach_shadow(host_id) {
            *dirty_ref.borrow_mut() = true;
            Ok(JsValue::from(sr_id.raw() as i32))
        } else {
            Ok(JsValue::null())
        }
    });

    // _insertBefore(parentId, childId, refId) — refId 0 means "append"
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let insert_before_native = native_fn!(move |_this, args, ctx| {
        let parent_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let child_idx = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0);
        let ref_idx = args.get_or_undefined(2).to_u32(ctx).unwrap_or(0);
        let mut document = doc_ref.borrow_mut();
        let parent = NodeId::from_raw(parent_idx);
        let child = NodeId::from_raw(child_idx);
        if ref_idx == 0 {
            document.append_child(parent, child);
        } else {
            document.insert_before(parent, child, NodeId::from_raw(ref_idx));
        }
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    // _replaceChild(parentId, newChildId, oldChildId)
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let replace_child_native = native_fn!(move |_this, args, ctx| {
        let parent_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let new_idx = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0);
        let old_idx = args.get_or_undefined(2).to_u32(ctx).unwrap_or(0);
        let mut document = doc_ref.borrow_mut();
        let parent = NodeId::from_raw(parent_idx);
        let new_child = NodeId::from_raw(new_idx);
        let old_child = NodeId::from_raw(old_idx);
        document.replace_child(parent, new_child, old_child);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    // _elementContains(ancestorId, otherId) → bool
    let doc_ref = doc.clone();
    let element_contains = native_fn!(move |_this, args, ctx| {
        // Node id 0 is the document root, so "no node" must be signalled by an
        // out-of-range value rather than zero.
        let Some(ancestor) = args.get_or_undefined(0).to_u32(ctx).ok() else {
            return Ok(JsValue::from(false));
        };
        let Some(other) = args.get_or_undefined(1).to_u32(ctx).ok() else {
            return Ok(JsValue::from(false));
        };
        let document = doc_ref.borrow();
        if document.get(NodeId::from_raw(ancestor)).is_none() {
            return Ok(JsValue::from(false));
        }
        let mut cur = Some(NodeId::from_raw(other));
        while let Some(id) = cur {
            if id.raw() == ancestor {
                return Ok(JsValue::from(true));
            }
            cur = document.get(id).and_then(|n| n.parent);
        }
        Ok(JsValue::from(false))
    });

    // _getInnerHTML(nodeId) / _getOuterHTML(nodeId)
    let doc_ref = doc.clone();
    let get_inner_html = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        Ok(JsValue::from(boa_engine::js_string!(document.serialize_children(
            NodeId::from_raw(node_idx)
        ))))
    });

    let doc_ref = doc.clone();
    let get_outer_html = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        Ok(JsValue::from(boa_engine::js_string!(document.serialize_html(
            NodeId::from_raw(node_idx)
        ))))
    });

    let doc_ref = doc.clone();
    let get_template_content = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let mut document = doc_ref.borrow_mut();
        let frag_id = document.get_or_create_template_contents(NodeId::from_raw(node_idx));
        Ok(JsValue::from(frag_id.raw()))
    });

    // _setInnerHTML(nodeId, html) — parses an HTML fragment and adopts its children
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_inner_html = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let html = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let mut document = doc_ref.borrow_mut();
        let node_id = NodeId::from_raw(node_idx);
        let children: Vec<NodeId> = document.children(node_id).map(|n| n.id).collect();
        for child in children {
            document.detach(child);
        }
        let fragment = mango_html::parse_html(&format!("<html><body>{html}</body></html>"));
        if let Some(body) = fragment.find_element_by_tag(fragment.root(), "body") {
            adopt_children(&fragment, body, &mut document, node_id);
        }
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    // _setOuterHTML(nodeId, html) — replaces the node itself
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_outer_html = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let html = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let mut document = doc_ref.borrow_mut();
        let node_id = NodeId::from_raw(node_idx);
        let Some(parent) = document.get(node_id).and_then(|n| n.parent) else {
            return Ok(JsValue::undefined());
        };
        let fragment = mango_html::parse_html(&format!("<html><body>{html}</body></html>"));
        if let Some(body) = fragment.find_element_by_tag(fragment.root(), "body") {
            adopt_children_before(&fragment, body, &mut document, parent, node_id);
        }
        document.detach(node_id);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    // _getNodeType(nodeId) / _getNodeName(nodeId) / _getNodeValue(nodeId) / _setNodeValue
    let doc_ref = doc.clone();
    let get_node_type = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        let t = match document.get(NodeId::from_raw(node_idx)).map(|n| &n.data) {
            Some(NodeData::Element(_)) => 1,
            Some(NodeData::Text(_)) => 3,
            Some(NodeData::Comment(_)) => 8,
            Some(NodeData::Document) => 9,
            Some(NodeData::DocumentType { .. }) => 10,
            Some(NodeData::DocumentFragment) => 11,
            None => 0,
        };
        Ok(JsValue::from(t))
    });

    let doc_ref = doc.clone();
    let get_node_name = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        let name = match document.get(NodeId::from_raw(node_idx)).map(|n| &n.data) {
            Some(NodeData::Element(el)) => el.tag_name.to_uppercase(),
            Some(NodeData::Text(_)) => "#text".to_string(),
            Some(NodeData::Comment(_)) => "#comment".to_string(),
            Some(NodeData::Document) => "#document".to_string(),
            Some(NodeData::DocumentType { name, .. }) => name.clone(),
            Some(NodeData::DocumentFragment) => "#document-fragment".to_string(),
            None => String::new(),
        };
        Ok(JsValue::from(boa_engine::js_string!(name)))
    });

    let doc_ref = doc.clone();
    let get_node_value = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        Ok(match document.get(NodeId::from_raw(node_idx)).map(|n| &n.data) {
            Some(NodeData::Text(t)) | Some(NodeData::Comment(t)) => {
                JsValue::from(boa_engine::js_string!(t.clone()))
            }
            _ => JsValue::null(),
        })
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_node_value = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let value = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let mut document = doc_ref.borrow_mut();
        if let Some(node) = document.get_mut(NodeId::from_raw(node_idx)) {
            match &mut node.data {
                NodeData::Text(t) | NodeData::Comment(t) => {
                    *t = value;
                    *dirty_ref.borrow_mut() = true;
                }
                _ => {}
            }
        }
        Ok(JsValue::undefined())
    });

    // _getAttributeNames(nodeId) → array of attribute names
    let doc_ref = doc.clone();
    let get_attribute_names = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        let names: Vec<JsValue> = match document.get(NodeId::from_raw(node_idx)).map(|n| &n.data) {
            Some(NodeData::Element(el)) => el
                .attributes
                .iter()
                .map(|(k, _)| JsValue::from(boa_engine::js_string!(k.clone())))
                .collect(),
            _ => Vec::new(),
        };
        Ok(JsArray::from_iter(names, ctx).into())
    });

    // data-* attribute helpers for element.dataset
    let doc_ref = doc.clone();
    let get_dataset_value = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let prop = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let attr = dataset_prop_to_attribute(&prop);
        let document = doc_ref.borrow();
        Ok(match document.get(NodeId::from_raw(node_idx)).map(|n| &n.data) {
            Some(NodeData::Element(el)) => match el.get_attribute(&attr) {
                Some(v) => JsValue::from(boa_engine::js_string!(v.to_string())),
                None => JsValue::undefined(),
            },
            _ => JsValue::undefined(),
        })
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_dataset_value = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let prop = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let value = args.get_or_undefined(2).to_string(ctx)?.to_std_string_escaped();
        let attr = dataset_prop_to_attribute(&prop);
        let mut document = doc_ref.borrow_mut();
        if let Some(node) = document.get_mut(NodeId::from_raw(node_idx))
            && let NodeData::Element(el) = &mut node.data
        {
            set_element_attribute(el, &attr, &value);
            *dirty_ref.borrow_mut() = true;
        }
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let remove_dataset_value = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let prop = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let attr = dataset_prop_to_attribute(&prop);
        let mut document = doc_ref.borrow_mut();
        if let Some(node) = document.get_mut(NodeId::from_raw(node_idx))
            && let NodeData::Element(el) = &mut node.data
        {
            el.attributes.retain(|(k, _)| !k.eq_ignore_ascii_case(&attr));
            *dirty_ref.borrow_mut() = true;
        }
        Ok(JsValue::undefined())
    });

    let doc_ref = doc.clone();
    let get_dataset_keys = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let document = doc_ref.borrow();
        let keys: Vec<JsValue> = match document.get(NodeId::from_raw(node_idx)).map(|n| &n.data) {
            Some(NodeData::Element(el)) => el
                .attributes
                .iter()
                .filter_map(|(k, _)| k.strip_prefix("data-").map(|s| camel_case_key(s)))
                .map(|k| JsValue::from(boa_engine::js_string!(k)))
                .collect(),
            _ => Vec::new(),
        };
        Ok(JsArray::from_iter(keys, ctx).into())
    });

    // _getDocumentElements(kind) → array of node ids (forms/images/links/anchors/scripts)
    let doc_ref = doc.clone();
    let get_document_elements = native_fn!(move |_this, args, ctx| {
        let kind = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
        let document = doc_ref.borrow();
        let mut out = Vec::new();
        collect_elements_by_kind(&document, document.root(), &kind, &mut out);
        let values: Vec<JsValue> = out.into_iter().map(|id| JsValue::from(id.raw() as i32)).collect();
        Ok(JsArray::from_iter(values, ctx).into())
    });

    // _createComment(text) → comment node id
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let create_comment_native = native_fn!(move |_this, args, ctx| {
        let text = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
        let mut document = doc_ref.borrow_mut();
        let node_id = document.create_comment(&text);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::from(node_id.raw() as i32))
    });

    // _setFocusState(nodeId, focused) — drives the :focus / :focus-within pseudo-classes
    let doc_ref = doc.clone();
    let dirty_ref = dirty.clone();
    let set_focus_state = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let focused = args.get_or_undefined(1).to_boolean();
        let mut document = doc_ref.borrow_mut();
        if let Some(node) = document.get_mut(NodeId::from_raw(node_idx))
            && let NodeData::Element(el) = &mut node.data
        {
            if focused {
                set_element_attribute(el, "data-mango-focused", "true");
            } else {
                el.attributes.retain(|(k, _)| k != "data-mango-focused");
            }
            *dirty_ref.borrow_mut() = true;
        }
        Ok(JsValue::undefined())
    });

    // _getElementRect(nodeId) → [x, y, width, height] from the browser's layout tree
    let bounds_ref = layout_bounds.clone();
    let get_element_rect = native_fn!(move |_this, args, ctx| {
        let node_idx = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let bounds = bounds_ref.borrow();
        let rect = bounds.get(&node_idx).copied().unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let values: Vec<JsValue> = rect.iter().map(|v| JsValue::from(*v)).collect();
        Ok(JsArray::from_iter(values, ctx).into())
    });

    // _elementFromPoint(x, y) → topmost node id at the given viewport point
    let bounds_ref = layout_bounds.clone();
    let element_from_point = native_fn!(move |_this, args, ctx| {
        let x = args.get_or_undefined(0).to_number(ctx)? as f32;
        let y = args.get_or_undefined(1).to_number(ctx)? as f32;
        let bounds = bounds_ref.borrow();
        let mut best: Option<(u32, f32)> = None;
        for (id, rect) in bounds.iter() {
            let (rx, ry, rw, rh) = (rect[0], rect[1], rect[2], rect[3]);
            if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                let area = rw * rh;
                if best.is_none_or(|(_, best_area)| area < best_area) {
                    best = Some((*id, area));
                }
            }
        }
        Ok(match best {
            Some((id, _)) => JsValue::from(id as i32),
            None => JsValue::null(),
        })
    });

    // _elementsFromPoint(x, y) → array of node ids at the given viewport point (innermost first)
    let bounds_ref = layout_bounds.clone();
    let elements_from_point = native_fn!(move |_this, args, ctx| {
        let x = args.get_or_undefined(0).to_number(ctx)? as f32;
        let y = args.get_or_undefined(1).to_number(ctx)? as f32;
        let bounds = bounds_ref.borrow();
        let mut matches: Vec<(u32, f32)> = Vec::new();
        for (id, rect) in bounds.iter() {
            let (rx, ry, rw, rh) = (rect[0], rect[1], rect[2], rect[3]);
            if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                let area = rw * rh;
                matches.push((*id, area));
            }
        }
        matches.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let values: Vec<JsValue> = matches.into_iter().map(|(id, _)| JsValue::from(id as i32)).collect();
        Ok(JsArray::from_iter(values, ctx).into())
    });

    // _getDocumentCookie() → the `document.cookie` string for the current page.
    let cookie_jar_ref = cookie_jar.clone();
    let cookie_url = page_url.clone();
    let get_document_cookie = native_fn!(move |_this, _args, _ctx| {
        let jar = jar_lock(&cookie_jar_ref);
        let url = match mango_net::Url::parse(&cookie_url) {
            Ok(u) => u,
            Err(_) => return Ok(JsValue::from(boa_engine::js_string!(""))),
        };
        let value = jar.script_cookie_string(&url);
        Ok(JsValue::from(boa_engine::js_string!(value)))
    });

    // _setDocumentCookie(str) → applies a `document.cookie = ...` assignment.
    let cookie_jar_ref = cookie_jar.clone();
    let cookie_url = page_url.clone();
    let set_document_cookie = native_fn!(move |_this, args, ctx| {
        let raw = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let url = match mango_net::Url::parse(&cookie_url) {
            Ok(u) => u,
            Err(_) => return Ok(JsValue::undefined()),
        };
        let mut jar = jar_lock(&cookie_jar_ref);
        jar.set_from_script(&raw, &url);
        jar.prune_expired();
        drop(jar);
        Ok(JsValue::undefined())
    });

    // Register all helper functions as globals
    context.register_global_callable(boa_engine::js_string!("_getDocumentCookie"), 0, get_document_cookie).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setDocumentCookie"), 1, set_document_cookie).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getTextContent"), 1, get_text_content).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setTextContent"), 2, set_text_content).unwrap();
    context.register_global_callable(boa_engine::js_string!("_appendChild"), 2, append_child).unwrap();
    context.register_global_callable(boa_engine::js_string!("_removeChild"), 2, remove_child).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setAttribute"), 3, set_attribute).unwrap();
    context.register_global_callable(boa_engine::js_string!("_removeAttribute"), 2, remove_attribute).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getAttribute"), 2, get_attribute).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getTagName"), 1, get_tag_name).unwrap();
    context.register_global_callable(boa_engine::js_string!("_querySelector"), 2, query_selector).unwrap();
    context.register_global_callable(boa_engine::js_string!("_querySelectorAll"), 2, query_selector_all).unwrap();
    context.register_global_callable(boa_engine::js_string!("_elementMatches"), 2, element_matches).unwrap();
    context.register_global_callable(boa_engine::js_string!("_elementClosest"), 2, element_closest).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getCompatMode"), 0, get_compat_mode).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getCharacterSet"), 0, get_character_set).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getContentType"), 0, get_content_type).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getParent"), 1, get_parent).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getChildren"), 1, get_children).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getChildNodeIds"), 1, get_child_node_ids).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getFirstChild"), 1, get_first_child).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getLastChild"), 1, get_last_child).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getNextSibling"), 1, get_next_sibling).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getPreviousSibling"), 1, get_previous_sibling).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getFirstElementChild"), 1, get_first_element_child).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getLastElementChild"), 1, get_last_element_child).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getNextElementSibling"), 1, get_next_element_sibling).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getPreviousElementSibling"), 1, get_previous_element_sibling).unwrap();
    context.register_global_callable(boa_engine::js_string!("_cloneNode"), 2, clone_node).unwrap();
    context.register_global_callable(boa_engine::js_string!("_createFragment"), 0, create_fragment).unwrap();
    context.register_global_callable(boa_engine::js_string!("_attachShadow"), 1, attach_shadow_native).unwrap();
    context.register_global_callable(boa_engine::js_string!("_createComment"), 1, create_comment_native).unwrap();
    context.register_global_callable(boa_engine::js_string!("_insertBefore"), 3, insert_before_native).unwrap();
    context.register_global_callable(boa_engine::js_string!("_replaceChild"), 3, replace_child_native).unwrap();
    context.register_global_callable(boa_engine::js_string!("_elementContains"), 2, element_contains).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getInnerHTML"), 1, get_inner_html).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setInnerHTML"), 2, set_inner_html).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getOuterHTML"), 1, get_outer_html).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setOuterHTML"), 2, set_outer_html).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getTemplateContent"), 1, get_template_content).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getNodeType"), 1, get_node_type).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getNodeName"), 1, get_node_name).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getNodeValue"), 1, get_node_value).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setNodeValue"), 2, set_node_value).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getAttributeNames"), 1, get_attribute_names).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getDatasetValue"), 2, get_dataset_value).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setDatasetValue"), 3, set_dataset_value).unwrap();
    context.register_global_callable(boa_engine::js_string!("_removeDatasetValue"), 2, remove_dataset_value).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getDatasetKeys"), 1, get_dataset_keys).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getDocumentElements"), 1, get_document_elements).unwrap();
    context.register_global_callable(boa_engine::js_string!("_setFocusState"), 2, set_focus_state).unwrap();
    context.register_global_callable(boa_engine::js_string!("_getElementRect"), 1, get_element_rect).unwrap();
    context.register_global_callable(boa_engine::js_string!("_elementFromPoint"), 2, element_from_point).unwrap();
    context.register_global_callable(boa_engine::js_string!("_elementsFromPoint"), 2, elements_from_point).unwrap();

    // Canvas native bindings
    let dirty_ref = dirty.clone();
    let canvas_resize = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let width = args.get_or_undefined(1).to_u32(ctx).unwrap_or(300);
        let height = args.get_or_undefined(2).to_u32(ctx).unwrap_or(150);
        mango_render::resize_canvas(node_id, width, height);
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let canvas_set_fill_style = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let color_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let color = mango_render::color::parse_color(&color_str).unwrap_or(mango_core::Color::BLACK);
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_fill_style(color);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_stroke_style = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let color_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let color = mango_render::color::parse_color(&color_str).unwrap_or(mango_core::Color::BLACK);
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_stroke_style(color);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_line_width = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let width = args.get_or_undefined(1).to_number(ctx).unwrap_or(1.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_line_width(width);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_line_cap = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let cap = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_line_cap(&cap);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_line_join = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let join = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_line_join(&join);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_miter_limit = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let limit = args.get_or_undefined(1).to_number(ctx).unwrap_or(10.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_miter_limit(limit);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_global_alpha = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let alpha = args.get_or_undefined(1).to_number(ctx).unwrap_or(1.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_global_alpha(alpha);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_font = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let font_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_font(&font_str);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_text_align = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let align_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_text_align(&align_str);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_text_baseline = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let baseline_str = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.set_text_baseline(&baseline_str);
        });
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_fill_rect = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let w = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let h = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.fill_rect(x, y, w, h);
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_stroke_rect = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let w = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let h = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.stroke_rect(x, y, w, h);
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_clear_rect = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let w = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let h = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.clear_rect(x, y, w, h);
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let canvas_begin_path = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.begin_path();
        });
        Ok(JsValue::undefined())
    });

    let canvas_close_path = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.close_path();
        });
        Ok(JsValue::undefined())
    });

    let canvas_move_to = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.move_to(x, y);
        });
        Ok(JsValue::undefined())
    });

    let canvas_line_to = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.line_to(x, y);
        });
        Ok(JsValue::undefined())
    });

    let canvas_rect = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let w = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let h = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.rect(x, y, w, h);
        });
        Ok(JsValue::undefined())
    });

    let canvas_arc = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let cx_val = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let cy_val = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let r_val = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let sa_val = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        let ea_val = args.get_or_undefined(5).to_number(ctx).unwrap_or(0.0) as f32;
        let ccw = args.get_or_undefined(6).to_boolean();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.arc(cx_val, cy_val, r_val, sa_val, ea_val, ccw);
        });
        Ok(JsValue::undefined())
    });

    let canvas_arc_to = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let x1 = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let y1 = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let x2 = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let y2 = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        let r = args.get_or_undefined(5).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.arc_to(x1, y1, x2, y2, r);
        });
        Ok(JsValue::undefined())
    });

    let canvas_bezier_curve_to = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let cp1x = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let cp1y = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let cp2x = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let cp2y = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        let x = args.get_or_undefined(5).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(6).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y);
        });
        Ok(JsValue::undefined())
    });

    let canvas_quadratic_curve_to = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let cpx = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let cpy = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let x = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.quadratic_curve_to(cpx, cpy, x, y);
        });
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_fill = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let _rule = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.fill();
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_stroke = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.stroke();
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let canvas_save = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.save();
        });
        Ok(JsValue::undefined())
    });

    let canvas_restore = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.restore();
        });
        Ok(JsValue::undefined())
    });

    let canvas_scale = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let sx = args.get_or_undefined(1).to_number(ctx).unwrap_or(1.0) as f32;
        let sy = args.get_or_undefined(2).to_number(ctx).unwrap_or(1.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.scale(sx, sy);
        });
        Ok(JsValue::undefined())
    });

    let canvas_rotate = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let angle = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.rotate(angle);
        });
        Ok(JsValue::undefined())
    });

    let canvas_translate = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let dx = args.get_or_undefined(1).to_number(ctx).unwrap_or(0.0) as f32;
        let dy = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.translate(dx, dy);
        });
        Ok(JsValue::undefined())
    });

    let canvas_transform = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let a = args.get_or_undefined(1).to_number(ctx).unwrap_or(1.0) as f32;
        let b = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let c = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let d = args.get_or_undefined(4).to_number(ctx).unwrap_or(1.0) as f32;
        let e = args.get_or_undefined(5).to_number(ctx).unwrap_or(0.0) as f32;
        let f = args.get_or_undefined(6).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |cv| {
            cv.transform(a, b, c, d, e, f);
        });
        Ok(JsValue::undefined())
    });

    let canvas_set_transform = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let a = args.get_or_undefined(1).to_number(ctx).unwrap_or(1.0) as f32;
        let b = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let c = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let d = args.get_or_undefined(4).to_number(ctx).unwrap_or(1.0) as f32;
        let e = args.get_or_undefined(5).to_number(ctx).unwrap_or(0.0) as f32;
        let f = args.get_or_undefined(6).to_number(ctx).unwrap_or(0.0) as f32;
        mango_render::with_canvas_mut(node_id, 300, 150, |cv| {
            cv.set_transform(a, b, c, d, e, f);
        });
        Ok(JsValue::undefined())
    });

    let canvas_reset_transform = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.reset_transform();
        });
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_fill_text = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let text = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let x = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let max_w = if !args.get_or_undefined(4).is_undefined() && !args.get_or_undefined(4).is_null() {
            Some(args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32)
        } else {
            None
        };
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.fill_text(&text, x, y, max_w);
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let dirty_ref = dirty.clone();
    let canvas_stroke_text = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let text = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let x = args.get_or_undefined(2).to_number(ctx).unwrap_or(0.0) as f32;
        let y = args.get_or_undefined(3).to_number(ctx).unwrap_or(0.0) as f32;
        let max_w = if !args.get_or_undefined(4).is_undefined() && !args.get_or_undefined(4).is_null() {
            Some(args.get_or_undefined(4).to_number(ctx).unwrap_or(0.0) as f32)
        } else {
            None
        };
        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.stroke_text(&text, x, y, max_w);
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let canvas_measure_text = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let text = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let w = mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.measure_text(&text)
        }).unwrap_or(0.0);
        Ok(JsValue::from(w as f64))
    });

    let canvas_get_image_data = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let sx = args.get_or_undefined(1).to_i32(ctx).unwrap_or(0);
        let sy = args.get_or_undefined(2).to_i32(ctx).unwrap_or(0);
        let sw = args.get_or_undefined(3).to_u32(ctx).unwrap_or(1);
        let sh = args.get_or_undefined(4).to_u32(ctx).unwrap_or(1);
        let bytes = mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.get_image_data(sx, sy, sw, sh)
        }).unwrap_or_else(|| vec![0u8; (sw * sh * 4) as usize]);
        let values: Vec<JsValue> = bytes.into_iter().map(|b| JsValue::from(b as i32)).collect();
        let js_array = JsArray::from_iter(values, ctx);
        Ok(js_array.into())
    });

    let dirty_ref = dirty.clone();
    let canvas_put_image_data = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let raw_val = args.get_or_undefined(1);
        let dx = args.get_or_undefined(2).to_i32(ctx).unwrap_or(0);
        let dy = args.get_or_undefined(3).to_i32(ctx).unwrap_or(0);
        let dirty_x = args.get_or_undefined(4).to_i32(ctx).unwrap_or(0);
        let dirty_y = args.get_or_undefined(5).to_i32(ctx).unwrap_or(0);
        let dirty_w = args.get_or_undefined(6).to_u32(ctx).unwrap_or(1);
        let dirty_h = args.get_or_undefined(7).to_u32(ctx).unwrap_or(1);
        let src_width = args.get_or_undefined(8).to_u32(ctx).unwrap_or(0);

        let mut byte_vec = Vec::new();
        if let Some(obj) = raw_val.as_object() {
            let len = obj
                .get(boa_engine::js_string!("length"), ctx)
                .ok()
                .and_then(|v| v.to_u32(ctx).ok())
                .unwrap_or(0);
            byte_vec.reserve(len as usize);
            for i in 0..len {
                if let Ok(v) = obj.get(i, ctx) {
                    byte_vec.push(v.to_u32(ctx).unwrap_or(0) as u8);
                } else {
                    byte_vec.push(0);
                }
            }
        }

        mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.put_image_data_with_stride(&byte_vec, src_width, dx, dy, dirty_x, dirty_y, dirty_w, dirty_h);
        });
        *dirty_ref.borrow_mut() = true;
        Ok(JsValue::undefined())
    });

    let canvas_to_data_url = native_fn!(move |_this, args, ctx| {
        let node_id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let mime = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
        let url = mango_render::with_canvas_mut(node_id, 300, 150, |c| {
            c.to_data_url(&mime)
        }).unwrap_or_else(|| "data:image/png;base64,".to_string());
        Ok(JsValue::from(boa_engine::js_string!(url)))
    });

    context.register_global_callable(boa_engine::js_string!("_canvasResize"), 3, canvas_resize).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetFillStyle"), 2, canvas_set_fill_style).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetStrokeStyle"), 2, canvas_set_stroke_style).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetLineWidth"), 2, canvas_set_line_width).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetLineCap"), 2, canvas_set_line_cap).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetLineJoin"), 2, canvas_set_line_join).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetMiterLimit"), 2, canvas_set_miter_limit).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetGlobalAlpha"), 2, canvas_set_global_alpha).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetFont"), 2, canvas_set_font).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetTextAlign"), 2, canvas_set_text_align).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetTextBaseline"), 2, canvas_set_text_baseline).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasFillRect"), 5, canvas_fill_rect).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasStrokeRect"), 5, canvas_stroke_rect).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasClearRect"), 5, canvas_clear_rect).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasBeginPath"), 1, canvas_begin_path).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasClosePath"), 1, canvas_close_path).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasMoveTo"), 3, canvas_move_to).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasLineTo"), 3, canvas_line_to).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasRect"), 5, canvas_rect).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasArc"), 7, canvas_arc).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasArcTo"), 6, canvas_arc_to).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasBezierCurveTo"), 7, canvas_bezier_curve_to).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasQuadraticCurveTo"), 5, canvas_quadratic_curve_to).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasFill"), 2, canvas_fill).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasStroke"), 1, canvas_stroke).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSave"), 1, canvas_save).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasRestore"), 1, canvas_restore).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasScale"), 3, canvas_scale).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasRotate"), 2, canvas_rotate).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasTranslate"), 3, canvas_translate).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasTransform"), 7, canvas_transform).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasSetTransform"), 7, canvas_set_transform).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasResetTransform"), 1, canvas_reset_transform).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasFillText"), 5, canvas_fill_text).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasStrokeText"), 5, canvas_stroke_text).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasMeasureText"), 2, canvas_measure_text).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasGetImageData"), 5, canvas_get_image_data).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasPutImageData"), 9, canvas_put_image_data).unwrap();
    context.register_global_callable(boa_engine::js_string!("_canvasToDataUrl"), 2, canvas_to_data_url).unwrap();

    // Install JS shim that wraps raw node IDs into rich element-like objects
    let shim = r#"
    function _normalizeEventOptions(options) {
        var capture = false;
        var once = false;
        var passive = false;
        var signal = null;
        if (typeof options === 'boolean') {
            capture = options;
        } else if (options && typeof options === 'object') {
            capture = !!options.capture;
            once = !!options.once;
            passive = !!options.passive;
            signal = options.signal || null;
        }
        return { capture: capture, once: once, passive: passive, signal: signal };
    }

    function _addDOMEventListener(target, type, listener, options) {
        if (!listener || !target) return;
        type = String(type);
        if (!target._listeners) target._listeners = {};
        if (!target._listeners[type]) target._listeners[type] = [];

        var opts = _normalizeEventOptions(options);
        if (opts.signal && opts.signal.aborted) return;

        for (var i = 0; i < target._listeners[type].length; i++) {
            var item = target._listeners[type][i];
            if (item.fn === listener && item.capture === opts.capture) {
                return;
            }
        }

        var entry = {
            fn: listener,
            capture: opts.capture,
            once: opts.once,
            passive: opts.passive,
            signal: opts.signal
        };
        target._listeners[type].push(entry);

        if (opts.signal && typeof opts.signal.addEventListener === 'function') {
            var onAbort = function() {
                _removeDOMEventListener(target, type, listener, { capture: opts.capture });
                if (typeof opts.signal.removeEventListener === 'function') {
                    opts.signal.removeEventListener('abort', onAbort);
                }
            };
            opts.signal.addEventListener('abort', onAbort);
        }
    }

    function _removeDOMEventListener(target, type, listener, options) {
        if (!target || !target._listeners) return;
        type = String(type);
        if (!target._listeners[type]) return;
        var capture = false;
        if (typeof options === 'boolean') {
            capture = options;
        } else if (options && typeof options === 'object') {
            capture = !!options.capture;
        }
        target._listeners[type] = target._listeners[type].filter(function(item) {
            return !(item.fn === listener && item.capture === capture);
        });
    }

    function _dispatchDOMEvent(target, event) {
        if (!event || typeof event !== 'object') {
            if (typeof globalThis.Event === 'function') {
                event = new globalThis.Event(String(event));
            } else {
                event = { type: String(event), bubbles: false, cancelable: false };
            }
        }
        var type = String(event.type);
        event.target = target;
        event.isTrusted = !!event.isTrusted;
        event.defaultPrevented = !!event.defaultPrevented;
        event._stopped = false;
        event._stopImmediate = false;

        var path = [];
        if (target !== document && (typeof globalThis === 'undefined' || target !== globalThis.window) && (target._nodeId === undefined || target._nodeId === null)) {
            path = [target];
        } else {
            var cur = target;
            var guard = 0;
            while (cur && guard++ < 256) {
                path.push(cur);
                if (typeof globalThis !== 'undefined' && cur === globalThis.window) {
                    break;
                } else if (cur === document) {
                    if (typeof globalThis !== 'undefined' && globalThis.window && globalThis.window !== document) {
                        path.push(globalThis.window);
                    }
                    break;
                } else if (cur._nodeId !== undefined && cur._nodeId !== null) {
                    var pid = _getParent(cur._nodeId);
                    if (pid !== null && pid !== undefined) {
                        cur = _wrapElement(pid);
                    } else {
                        path.push(document);
                        if (typeof globalThis !== 'undefined' && globalThis.window && globalThis.window !== document) {
                            path.push(globalThis.window);
                        }
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        var cleanPath = [];
        for (var p = 0; p < path.length; p++) {
            if (cleanPath.indexOf(path[p]) === -1) {
                cleanPath.push(path[p]);
            }
        }
        path = cleanPath;

        event.composedPath = function() {
            return path.slice();
        };

        var onProp = 'on' + type;

        function invokeListenersOnNode(node, phaseIsCapture) {
            if (!node) return;
            event.currentTarget = node;

            if (node._listeners && node._listeners[type]) {
                var listeners = node._listeners[type].slice();
                for (var i = 0; i < listeners.length; i++) {
                    if (event._stopImmediate) break;
                    var entry = listeners[i];
                    if (event.eventPhase === 2 /* AT_TARGET */ || entry.capture === phaseIsCapture) {
                        if (entry.once) {
                            _removeDOMEventListener(node, type, entry.fn, { capture: entry.capture });
                        }
                        event._currentPassive = !!entry.passive;
                        try {
                            if (typeof entry.fn === 'function') {
                                entry.fn.call(node, event);
                            } else if (entry.fn && typeof entry.fn.handleEvent === 'function') {
                                entry.fn.handleEvent(event);
                            }
                        } catch (e) {
                            if (typeof console !== 'undefined' && console.error) console.error(e);
                        } finally {
                            event._currentPassive = false;
                        }
                    }
                }
            }

            if (!phaseIsCapture && !event._stopImmediate) {
                if (typeof node[onProp] === 'function') {
                    try {
                        node[onProp].call(node, event);
                    } catch (e) {
                        if (typeof console !== 'undefined' && console.error) console.error(e);
                    }
                }
            }
        }

        // Phase 1: Capturing Phase (window -> document -> ancestors down to target's parent)
        event.eventPhase = 1;
        for (var j = path.length - 1; j > 0; j--) {
            if (event._stopped || event._stopImmediate) break;
            invokeListenersOnNode(path[j], true);
        }

        // Phase 2: At Target Phase
        if (!event._stopped && !event._stopImmediate) {
            event.eventPhase = 2;
            invokeListenersOnNode(target, false);
        }

        // Phase 3: Bubbling Phase (target's parent -> document -> window)
        if (event.bubbles && !event._stopped && !event._stopImmediate) {
            event.eventPhase = 3;
            for (var k = 1; k < path.length; k++) {
                if (event._stopped || event._stopImmediate) break;
                invokeListenersOnNode(path[k], false);
            }
        }

        event.eventPhase = 0;
        event.currentTarget = null;
        return !event.defaultPrevented;
    }

    function _createStyleObject(nodeId) {
        var obj = {
            _props: {},
            getPropertyValue: function(p) {
                if (!p) return "";
                var val = this._props[p];
                if (val !== undefined) return val;
                var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                if (this[camel] !== undefined && typeof this[camel] !== 'function') return String(this[camel]);
                if (this[p] !== undefined && typeof this[p] !== 'function') return String(this[p]);
                return "";
            },
            setProperty: function(p, v) {
                if (!p) return;
                var sVal = String(v);
                this._props[p] = sVal;
                var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                this[camel] = sVal;
                this[p] = sVal;
            },
            removeProperty: function(p) {
                if (!p) return "";
                var prev = this.getPropertyValue(p);
                delete this._props[p];
                var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                delete this[camel];
                delete this[p];
                return prev;
            }
        };
        if (nodeId !== undefined && typeof _getAttribute === 'function') {
            var rawStyle = _getAttribute(nodeId, "style");
            if (rawStyle) {
                var decls = rawStyle.split(';');
                for (var i = 0; i < decls.length; i++) {
                    var d = decls[i].trim();
                    if (!d) continue;
                    var colon = d.indexOf(':');
                    if (colon !== -1) {
                        var prop = d.slice(0, colon).trim();
                        var val = d.slice(colon + 1).trim();
                        obj.setProperty(prop, val);
                    }
                }
            }
        }
        return obj;
    }

    function _createClassList(nodeId) {
        return {
            add: function() {
                var cur = _getAttribute(nodeId, "class") || "";
                var classes = cur.split(/\s+/).filter(Boolean);
                for (var i = 0; i < arguments.length; i++) {
                    if (classes.indexOf(arguments[i]) === -1) classes.push(arguments[i]);
                }
                _setAttribute(nodeId, "class", classes.join(" "));
            },
            remove: function() {
                var cur = _getAttribute(nodeId, "class") || "";
                var classes = cur.split(/\s+/).filter(Boolean);
                for (var i = 0; i < arguments.length; i++) {
                    var idx = classes.indexOf(arguments[i]);
                    if (idx !== -1) classes.splice(idx, 1);
                }
                _setAttribute(nodeId, "class", classes.join(" "));
            },
            contains: function(cls) {
                var cur = _getAttribute(nodeId, "class") || "";
                return cur.split(/\s+/).indexOf(cls) !== -1;
            },
            toggle: function(cls) {
                if (this.contains(cls)) { this.remove(cls); return false; }
                else { this.add(cls); return true; }
            }
        };
    }

    function _createHTMLCollection(getItemsFn) {
        var proto = (typeof globalThis.HTMLCollection !== 'undefined') ? globalThis.HTMLCollection.prototype : Object.prototype;
        var base = Object.create(proto);
        base.item = function(index) {
            var items = getItemsFn() || [];
            var i = Number(index);
            if (isNaN(i) || i < 0 || i >= items.length) return null;
            return items[i];
        };
        base.namedItem = function(name) {
            var items = getItemsFn() || [];
            var str = String(name);
            for (var i = 0; i < items.length; i++) {
                var it = items[i];
                if (it && (it.id === str || (it.getAttribute && it.getAttribute('name') === str))) {
                    return it;
                }
            }
            return null;
        };
        base[Symbol.iterator] = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: items[i++], done: false };
                    }
                    return { value: undefined, done: true };
                }
            };
        };
        base[Symbol.toStringTag] = 'HTMLCollection';

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                if (prop === 'length') {
                    return (getItemsFn() || []).length;
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var items = getItemsFn() || [];
                        return items[idx];
                    }
                }
                if (prop in target) {
                    return target[prop];
                }
                if (typeof prop === 'string') {
                    return target.namedItem(prop) || undefined;
                }
                return undefined;
            },
            has: function(target, prop) {
                if (prop === 'length') return true;
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return idx < (getItemsFn() || []).length;
                    }
                    if (target.namedItem(prop) !== null) return true;
                }
                return prop in target;
            },
            ownKeys: function(target) {
                var items = getItemsFn() || [];
                var keys = [];
                for (var i = 0; i < items.length; i++) {
                    keys.push(String(i));
                }
                keys.push('length');
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop === 'length') {
                    return { value: (getItemsFn() || []).length, writable: false, enumerable: false, configurable: true };
                }
                var idx = Number(prop);
                if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                    var items = getItemsFn() || [];
                    if (idx < items.length) {
                        return { value: items[idx], writable: false, enumerable: true, configurable: true };
                    }
                }
                return Object.getOwnPropertyDescriptor(target, prop);
            }
        });
    }

    function _createNodeList(getItemsFn) {
        var proto = (typeof globalThis.NodeList !== 'undefined') ? globalThis.NodeList.prototype : Object.prototype;
        var base = Object.create(proto);
        base.item = function(index) {
            var items = getItemsFn() || [];
            var i = Number(index);
            if (isNaN(i) || i < 0 || i >= items.length) return null;
            return items[i];
        };
        base.forEach = function(callback, thisArg) {
            var items = getItemsFn() || [];
            for (var i = 0; i < items.length; i++) {
                callback.call(thisArg, items[i], i, this);
            }
        };
        base.entries = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: [i, items[i++]], done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base.keys = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: i++, done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base.values = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: items[i++], done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base[Symbol.iterator] = function() {
            return this.values();
        };
        base[Symbol.toStringTag] = 'NodeList';

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                if (prop === 'length') {
                    return (getItemsFn() || []).length;
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var items = getItemsFn() || [];
                        return items[idx];
                    }
                }
                if (prop in target) {
                    return target[prop];
                }
                return undefined;
            },
            has: function(target, prop) {
                if (prop === 'length') return true;
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return idx < (getItemsFn() || []).length;
                    }
                }
                return prop in target;
            },
            ownKeys: function(target) {
                var items = getItemsFn() || [];
                var keys = [];
                for (var i = 0; i < items.length; i++) {
                    keys.push(String(i));
                }
                keys.push('length');
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop === 'length') {
                    return { value: (getItemsFn() || []).length, writable: false, enumerable: false, configurable: true };
                }
                var idx = Number(prop);
                if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                    var items = getItemsFn() || [];
                    if (idx < items.length) {
                        return { value: items[idx], writable: false, enumerable: true, configurable: true };
                    }
                }
                return Object.getOwnPropertyDescriptor(target, prop);
            }
        });
    }

    function _createCanvasRenderingContext2D(canvas) {
        var ctx = {
            _canvas: canvas,
            _fillStyle: '#000000',
            _strokeStyle: '#000000',
            _lineWidth: 1,
            _lineCap: 'butt',
            _lineJoin: 'miter',
            _miterLimit: 10,
            _globalAlpha: 1.0,
            _font: '10px sans-serif',
            _textAlign: 'start',
            _textBaseline: 'alphabetic',
            shadowBlur: 0,
            shadowColor: 'rgba(0, 0, 0, 0)',
            shadowOffsetX: 0,
            shadowOffsetY: 0,
            globalCompositeOperation: 'source-over',
            imageSmoothingEnabled: true,

            get canvas() { return this._canvas; },

            get fillStyle() { return this._fillStyle; },
            set fillStyle(val) {
                if (typeof val === 'string') {
                    this._fillStyle = val;
                    _canvasSetFillStyle(this._canvas._nodeId, val);
                } else if (val && typeof val === 'object') {
                    this._fillStyle = val;
                }
            },

            get strokeStyle() { return this._strokeStyle; },
            set strokeStyle(val) {
                if (typeof val === 'string') {
                    this._strokeStyle = val;
                    _canvasSetStrokeStyle(this._canvas._nodeId, val);
                } else if (val && typeof val === 'object') {
                    this._strokeStyle = val;
                }
            },

            get lineWidth() { return this._lineWidth; },
            set lineWidth(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num > 0) {
                    this._lineWidth = num;
                    _canvasSetLineWidth(this._canvas._nodeId, num);
                }
            },

            get lineCap() { return this._lineCap; },
            set lineCap(val) {
                this._lineCap = String(val);
                _canvasSetLineCap(this._canvas._nodeId, this._lineCap);
            },

            get lineJoin() { return this._lineJoin; },
            set lineJoin(val) {
                this._lineJoin = String(val);
                _canvasSetLineJoin(this._canvas._nodeId, this._lineJoin);
            },

            get miterLimit() { return this._miterLimit; },
            set miterLimit(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num > 0) {
                    this._miterLimit = num;
                    _canvasSetMiterLimit(this._canvas._nodeId, num);
                }
            },

            get globalAlpha() { return this._globalAlpha; },
            set globalAlpha(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num >= 0 && num <= 1) {
                    this._globalAlpha = num;
                    _canvasSetGlobalAlpha(this._canvas._nodeId, num);
                }
            },

            get font() { return this._font; },
            set font(val) {
                this._font = String(val);
                _canvasSetFont(this._canvas._nodeId, this._font);
            },

            get textAlign() { return this._textAlign; },
            set textAlign(val) {
                this._textAlign = String(val);
                _canvasSetTextAlign(this._canvas._nodeId, this._textAlign);
            },

            get textBaseline() { return this._textBaseline; },
            set textBaseline(val) {
                this._textBaseline = String(val);
                _canvasSetTextBaseline(this._canvas._nodeId, this._textBaseline);
            },

            fillRect: function(x, y, w, h) {
                _canvasFillRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            strokeRect: function(x, y, w, h) {
                _canvasStrokeRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            clearRect: function(x, y, w, h) {
                _canvasClearRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },

            beginPath: function() {
                _canvasBeginPath(this._canvas._nodeId);
            },
            closePath: function() {
                _canvasClosePath(this._canvas._nodeId);
            },
            moveTo: function(x, y) {
                _canvasMoveTo(this._canvas._nodeId, Number(x)||0, Number(y)||0);
            },
            lineTo: function(x, y) {
                _canvasLineTo(this._canvas._nodeId, Number(x)||0, Number(y)||0);
            },
            rect: function(x, y, w, h) {
                _canvasRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            arc: function(x, y, radius, startAngle, endAngle, counterclockwise) {
                _canvasArc(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(radius)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise);
            },
            arcTo: function(x1, y1, x2, y2, radius) {
                _canvasArcTo(this._canvas._nodeId, Number(x1)||0, Number(y1)||0, Number(x2)||0, Number(y2)||0, Number(radius)||0);
            },
            bezierCurveTo: function(cp1x, cp1y, cp2x, cp2y, x, y) {
                _canvasBezierCurveTo(this._canvas._nodeId, Number(cp1x)||0, Number(cp1y)||0, Number(cp2x)||0, Number(cp2y)||0, Number(x)||0, Number(y)||0);
            },
            quadraticCurveTo: function(cpx, cpy, x, y) {
                _canvasQuadraticCurveTo(this._canvas._nodeId, Number(cpx)||0, Number(cpy)||0, Number(x)||0, Number(y)||0);
            },

            fill: function(rule) {
                _canvasFill(this._canvas._nodeId, String(rule || 'nonzero'));
            },
            stroke: function() {
                _canvasStroke(this._canvas._nodeId);
            },
            clip: function(rule) {},
            isPointInPath: function(x, y) { return false; },

            save: function() {
                _canvasSave(this._canvas._nodeId);
            },
            restore: function() {
                _canvasRestore(this._canvas._nodeId);
            },
            scale: function(sx, sy) {
                _canvasScale(this._canvas._nodeId, Number(sx)||1, Number(sy)||1);
            },
            rotate: function(angle) {
                _canvasRotate(this._canvas._nodeId, Number(angle)||0);
            },
            translate: function(dx, dy) {
                _canvasTranslate(this._canvas._nodeId, Number(dx)||0, Number(dy)||0);
            },
            transform: function(a, b, c, d, e, f) {
                _canvasTransform(this._canvas._nodeId, Number(a)||1, Number(b)||0, Number(c)||0, Number(d)||1, Number(e)||0, Number(f)||0);
            },
            setTransform: function(a, b, c, d, e, f) {
                if (arguments.length === 0) {
                    _canvasResetTransform(this._canvas._nodeId);
                } else {
                    _canvasSetTransform(this._canvas._nodeId, Number(a)||1, Number(b)||0, Number(c)||0, Number(d)||1, Number(e)||0, Number(f)||0);
                }
            },
            resetTransform: function() {
                _canvasResetTransform(this._canvas._nodeId);
            },
            getTransform: function() {
                return { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
            },

            fillText: function(text, x, y, maxWidth) {
                var mw = maxWidth !== undefined ? Number(maxWidth) : null;
                _canvasFillText(this._canvas._nodeId, String(text), Number(x)||0, Number(y)||0, mw);
            },
            strokeText: function(text, x, y, maxWidth) {
                var mw = maxWidth !== undefined ? Number(maxWidth) : null;
                _canvasStrokeText(this._canvas._nodeId, String(text), Number(x)||0, Number(y)||0, mw);
            },
            measureText: function(text) {
                var str = String(text || '');
                var w = _canvasMeasureText(this._canvas._nodeId, str);
                return {
                    width: w,
                    actualBoundingBoxAscent: 10,
                    actualBoundingBoxDescent: 2,
                    fontBoundingBoxAscent: 10,
                    fontBoundingBoxDescent: 2
                };
            },

            getImageData: function(sx, sy, sw, sh) {
                var w = Math.max(1, Math.floor(sw || 1));
                var h = Math.max(1, Math.floor(sh || 1));
                var arr = _canvasGetImageData(this._canvas._nodeId, Math.floor(sx || 0), Math.floor(sy || 0), w, h);
                var u8 = new Uint8ClampedArray(arr);
                if (typeof globalThis.ImageData === 'function') {
                    return new globalThis.ImageData(u8, w, h);
                }
                return { data: u8, width: w, height: h };
            },
            putImageData: function(imgData, dx, dy, dirtyX, dirtyY, dirtyWidth, dirtyHeight) {
                if (!imgData || !imgData.data) return;
                var w = imgData.width || 0;
                var h = imgData.height || 0;
                var dX = dirtyX !== undefined ? Math.floor(dirtyX) : 0;
                var dY = dirtyY !== undefined ? Math.floor(dirtyY) : 0;
                var dW = dirtyWidth !== undefined ? Math.floor(dirtyWidth) : (w - dX);
                var dH = dirtyHeight !== undefined ? Math.floor(dirtyHeight) : (h - dY);
                var raw = [];
                for (var i = 0; i < imgData.data.length; i++) {
                    raw.push(imgData.data[i]);
                }
                _canvasPutImageData(this._canvas._nodeId, raw, Math.floor(dx || 0), Math.floor(dy || 0), dX, dY, dW, dH, w);
            },
            createImageData: function(w, h) {
                var width = Math.max(1, Math.floor(w || 1));
                var height = Math.max(1, Math.floor(h || 1));
                var u8 = new Uint8ClampedArray(width * height * 4);
                if (typeof globalThis.ImageData === 'function') {
                    return new globalThis.ImageData(u8, width, height);
                }
                return { data: u8, width: width, height: height };
            },
            createLinearGradient: function(x0, y0, x1, y1) {
                return {
                    addColorStop: function(offset, color) {}
                };
            },
            createRadialGradient: function(x0, y0, r0, x1, y1, r1) {
                return {
                    addColorStop: function(offset, color) {}
                };
            },
            createPattern: function(image, repetition) {
                return {};
            },
            drawImage: function() {}
        };

        // Initialize backing store with the canvas's width and height
        _canvasResize(canvas._nodeId, canvas.width, canvas.height);

        if (typeof globalThis.CanvasRenderingContext2D === 'function') {
            Object.setPrototypeOf(ctx, globalThis.CanvasRenderingContext2D.prototype);
        }
        return ctx;
    }

    // Per-node wrapper cache. DOM element identity requires that wrapping the
    // same node twice yields the same object (`document.querySelector('#a') ===
    // document.querySelector('#a')`), and listeners/styles registered through
    // one handle must stay reachable when that node is wrapped again — e.g.
    // when the chrome dispatches `input`/`change` events from Rust.
    var _wrapperCache = {};

    function _wrapElement(nodeId) {
        if (nodeId === null || nodeId === undefined) return null;
        if (_wrapperCache[nodeId]) return _wrapperCache[nodeId];
        var el = {
            _nodeId: nodeId,
            _listeners: {},
            get textContent() { return _getTextContent(this._nodeId); },
            set textContent(val) {
                var oldVal = _getTextContent(this._nodeId);
                _setTextContent(this._nodeId, String(val));
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'characterData',
                        target: this,
                        oldValue: oldVal
                    });
                }
            },
            get innerText() { return this.textContent; },
            set innerText(val) { this.textContent = val; },
            get innerHTML() { return _getInnerHTML(this._nodeId); },
            set innerHTML(val) { _setInnerHTML(this._nodeId, String(val)); },
            get outerHTML() { return _getOuterHTML(this._nodeId); },
            set outerHTML(val) { _setOuterHTML(this._nodeId, String(val)); },
            get nodeType() { return _getNodeType(this._nodeId); },
            get nodeName() { return _getNodeName(this._nodeId); },
            get nodeValue() { return _getNodeValue(this._nodeId); },
            set nodeValue(val) { _setNodeValue(this._nodeId, val === null ? '' : String(val)); },
            ELEMENT_NODE: 1,
            TEXT_NODE: 3,
            COMMENT_NODE: 8,
            DOCUMENT_NODE: 9,
            DOCUMENT_FRAGMENT_NODE: 11,
            get tagName() { return _getTagName(this._nodeId); },
            get localName() { var t = this.tagName; return t ? t.toLowerCase() : t; },
            get content() {
                var tag = (this.tagName || '').toUpperCase();
                if (tag === 'TEMPLATE') {
                    var fragId = _getTemplateContent(this._nodeId);
                    if (fragId !== null && fragId !== undefined) {
                        return _wrapElement(fragId);
                    }
                }
                return undefined;
            },
            get dataset() {
                if (!this._dataset) {
                    var self = this;
                    var target = {};
                    var keys = _getDatasetKeys(this._nodeId) || [];
                    for (var i = 0; i < keys.length; i++) {
                        (function(k) {
                            Object.defineProperty(target, k, {
                                enumerable: true,
                                configurable: true,
                                get: function() { return _getDatasetValue(self._nodeId, k); },
                                set: function(v) { _setDatasetValue(self._nodeId, k, String(v)); }
                            });
                        })(keys[i]);
                    }
                    this._dataset = new Proxy(target, {
                        get: function(t, prop) {
                            if (typeof prop !== 'string') return t[prop];
                            if (prop in t) return t[prop];
                            return _getDatasetValue(self._nodeId, prop);
                        },
                        set: function(t, prop, value) {
                            if (typeof prop !== 'string') { t[prop] = value; return true; }
                            _setDatasetValue(self._nodeId, prop, String(value));
                            Object.defineProperty(t, prop, {
                                enumerable: true,
                                configurable: true,
                                get: function() { return _getDatasetValue(self._nodeId, prop); },
                                set: function(v) { _setDatasetValue(self._nodeId, prop, String(v)); }
                            });
                            return true;
                        },
                        deleteProperty: function(t, prop) {
                            if (typeof prop === 'string') _removeDatasetValue(self._nodeId, prop);
                            delete t[prop];
                            return true;
                        },
                        has: function(t, prop) {
                            return (prop in t) || _getDatasetValue(self._nodeId, String(prop)) !== undefined;
                        },
                        ownKeys: function(t) {
                            return _getDatasetKeys(self._nodeId) || [];
                        },
                        getOwnPropertyDescriptor: function(t, prop) {
                            if (typeof prop === 'string') {
                                var v = _getDatasetValue(self._nodeId, prop);
                                if (v !== undefined) {
                                    return {
                                        value: v,
                                        writable: true,
                                        enumerable: true,
                                        configurable: true
                                    };
                                }
                            }
                            return Object.getOwnPropertyDescriptor(t, prop);
                        }
                    });
                }
                return this._dataset;
            },
            get id() { return this.getAttribute("id") || ""; },
            set id(val) { this.setAttribute("id", val); },
            get className() { return this.getAttribute("class") || ""; },
            set className(val) { this.setAttribute("class", val); },
            get classList() {
                if (!this._classList) this._classList = _createClassList(this._nodeId);
                return this._classList;
            },
            get style() {
                if (!this._style) this._style = _createStyleObject(this._nodeId);
                return this._style;
            },
            // ── Form control surface (Phase 1.3 / Section 5.3) ──
            get value() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') return this.textContent;
                if (tag === 'progress') {
                    var v = _getAttribute(this._nodeId, "value");
                    if (v === null) return -1;
                    var num = parseFloat(v);
                    return isNaN(num) ? 0 : Math.max(0, num);
                }
                if (tag === 'meter') {
                    var v = _getAttribute(this._nodeId, "value");
                    var num = parseFloat(v);
                    var min = this.min;
                    var max = this.max;
                    if (isNaN(num)) return min;
                    return Math.max(min, Math.min(max, num));
                }
                if (tag === 'select') {
                    var opts = this.options || [];
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].selected) {
                            return opts[i].value;
                        }
                    }
                    if (!this.multiple && opts.length > 0) {
                        return opts[0].value;
                    }
                    return "";
                }
                if (tag === 'option') {
                    var a = _getAttribute(this._nodeId, "value");
                    return a !== null ? a : (this.textContent || '').trim();
                }
                var v = _getAttribute(this._nodeId, "data-mango-value");
                if (v !== null) return v;
                var attr = _getAttribute(this._nodeId, "value");
                return attr === null ? "" : attr;
            },
            set value(val) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') {
                    this.textContent = (val === null || val === undefined) ? "" : String(val);
                    return;
                }
                if (tag === 'select') {
                    var targetVal = (val === null || val === undefined) ? "" : String(val);
                    var opts = this.options || [];
                    var matched = false;
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].value === targetVal) {
                            opts[i].selected = true;
                            matched = true;
                        } else if (!this.multiple) {
                            opts[i].selected = false;
                        }
                    }
                    if (!matched && !this.multiple) {
                        this.selectedIndex = -1;
                    }
                    return;
                }
                if (tag === 'option') {
                    this.setAttribute("value", (val === null || val === undefined) ? "" : String(val));
                    return;
                }
                if (_getAttribute(this._nodeId, "data-mango-default-value") === null) {
                    var initVal = _getAttribute(this._nodeId, "value");
                    _setAttribute(this._nodeId, "data-mango-default-value", initVal === null ? "" : initVal);
                }
                var str = (val === null || val === undefined) ? "" : String(val);
                _setAttribute(this._nodeId, "data-mango-value", str);
                _setAttribute(this._nodeId, "value", str);
            },
            get defaultValue() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') {
                    return this._defaultValue !== undefined ? this._defaultValue : this.textContent;
                }
                if (tag === 'textarea') {
                    var def = _getAttribute(this._nodeId, "data-mango-default-value");
                    return def !== null ? def : this.textContent;
                }
                var def = _getAttribute(this._nodeId, "data-mango-default-value");
                if (def !== null) return def;
                var a = _getAttribute(this._nodeId, "value");
                return a === null ? "" : a;
            },
            set defaultValue(val) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') {
                    this._defaultValue = String(val);
                    return;
                }
                _setAttribute(this._nodeId, "data-mango-default-value", String(val));
                _setAttribute(this._nodeId, "value", String(val));
            },
            get defaultChecked() {
                var def = this.getAttribute("data-mango-default-checked");
                if (def !== null) return def === "true";
                return this.hasAttribute("checked");
            },
            set defaultChecked(val) {
                this.setAttribute("data-mango-default-checked", val ? "true" : "false");
                if (val) this.setAttribute("checked", "");
                else this.removeAttribute("checked");
            },
            get checked() {
                var dyn = this.getAttribute("data-mango-checked");
                if (dyn !== null) return dyn === "true";
                return this.hasAttribute("checked");
            },
            set checked(val) {
                if (!this.hasAttribute("data-mango-default-checked")) {
                    this.setAttribute("data-mango-default-checked", this.hasAttribute("checked") ? "true" : "false");
                }
                this.setAttribute("data-mango-checked", val ? "true" : "false");
                if (val) this.setAttribute("checked", "");
                else this.removeAttribute("checked");
                this.dispatchEvent({ type: 'change' });
            },
            get indeterminate() { return this.getAttribute("data-mango-indeterminate") === "true"; },
            set indeterminate(val) { this.setAttribute("data-mango-indeterminate", val ? "true" : "false"); },
            get disabled() { return this.hasAttribute("disabled"); },
            set disabled(val) { if (val) this.setAttribute("disabled", ""); else this.removeAttribute("disabled"); },
            get required() { return this.hasAttribute("required"); },
            set required(val) { if (val) this.setAttribute("required", ""); else this.removeAttribute("required"); },
            get readOnly() { return this.hasAttribute("readonly"); },
            set readOnly(val) { if (val) this.setAttribute("readonly", ""); else this.removeAttribute("readonly"); },
            get placeholder() { var a = _getAttribute(this._nodeId, "placeholder"); return a === null ? "" : a; },
            set placeholder(val) { _setAttribute(this._nodeId, "placeholder", String(val)); },
            get name() { var a = _getAttribute(this._nodeId, "name"); return a === null ? "" : a; },
            set name(val) { _setAttribute(this._nodeId, "name", String(val)); },
            get pattern() { var p = this.getAttribute('pattern'); return p === null ? '' : p; },
            set pattern(val) { this.setAttribute('pattern', String(val)); },
            get type() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') return 'output';
                if (tag === 'fieldset') return 'fieldset';
                if (tag === 'select') return this.multiple ? 'select-multiple' : 'select-one';
                var t = _getAttribute(this._nodeId, "type");
                if (t !== null && t !== '') return String(t).toLowerCase();
                return tag === 'textarea' ? 'textarea' : 'text';
            },
            set type(val) { _setAttribute(this._nodeId, "type", String(val)); },
            get min() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'meter') {
                    var m = _getAttribute(this._nodeId, "min");
                    var num = parseFloat(m);
                    return isNaN(num) ? 0.0 : num;
                }
                var a = _getAttribute(this._nodeId, "min"); return a === null ? "" : a;
            },
            set min(val) { _setAttribute(this._nodeId, "min", String(val)); },
            get max() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'progress') {
                    var m = _getAttribute(this._nodeId, "max");
                    if (m === null) return 1.0;
                    var num = parseFloat(m);
                    return isNaN(num) || num <= 0 ? 1.0 : num;
                }
                if (tag === 'meter') {
                    var m = _getAttribute(this._nodeId, "max");
                    var num = parseFloat(m);
                    var min = this.min;
                    return isNaN(num) ? Math.max(min, 1.0) : Math.max(min, num);
                }
                var a = _getAttribute(this._nodeId, "max"); return a === null ? "" : a;
            },
            set max(val) { _setAttribute(this._nodeId, "max", String(val)); },
            get step() { var a = _getAttribute(this._nodeId, "step"); return a === null ? "" : a; },
            get low() {
                var l = _getAttribute(this._nodeId, "low");
                var num = parseFloat(l);
                var min = this.min;
                var max = this.max;
                if (isNaN(num)) return min;
                return Math.max(min, Math.min(max, num));
            },
            set low(val) { _setAttribute(this._nodeId, "low", String(val)); },
            get high() {
                var h = _getAttribute(this._nodeId, "high");
                var num = parseFloat(h);
                var min = this.min;
                var max = this.max;
                var low = this.low;
                if (isNaN(num)) return max;
                return Math.max(low, Math.min(max, num));
            },
            set high(val) { _setAttribute(this._nodeId, "high", String(val)); },
            get optimum() {
                var o = _getAttribute(this._nodeId, "optimum");
                var num = parseFloat(o);
                var min = this.min;
                var max = this.max;
                if (isNaN(num)) return min + (max - min) / 2.0;
                return Math.max(min, Math.min(max, num));
            },
            set optimum(val) { _setAttribute(this._nodeId, "optimum", String(val)); },
            get position() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'progress') {
                    var v = _getAttribute(this._nodeId, "value");
                    if (v === null) return -1;
                    var val = this.value;
                    var max = this.max;
                    return max > 0 ? Math.max(0, Math.min(1, val / max)) : -1;
                }
                return undefined;
            },
            get multiple() {
                var tag = (this.tagName || '').toLowerCase();
                return (tag === 'select' || tag === 'input') ? this.hasAttribute('multiple') : false;
            },
            set multiple(val) {
                if (val) this.setAttribute('multiple', '');
                else this.removeAttribute('multiple');
            },
            get size() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select' || tag === 'input') {
                    var s = this.getAttribute('size');
                    if (s === null) return tag === 'select' ? (this.multiple ? 4 : 1) : 20;
                    var n = parseInt(s, 10);
                    return isNaN(n) || n <= 0 ? (tag === 'select' ? 1 : 20) : n;
                }
                return undefined;
            },
            set size(val) {
                var n = parseInt(val, 10);
                this.setAttribute('size', String(isNaN(n) || n <= 0 ? 1 : n));
            },
            get selectedIndex() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select') {
                    var opts = this.options || [];
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].selected) return i;
                    }
                    if (!this.multiple && opts.length > 0) return 0;
                    return -1;
                }
                return -1;
            },
            set selectedIndex(idx) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select') {
                    var target = parseInt(idx, 10);
                    var opts = this.options || [];
                    for (var i = 0; i < opts.length; i++) {
                        opts[i].selected = (i === target);
                    }
                }
            },
            get selectedOptions() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select') {
                    var opts = this.options || [];
                    var sel = [];
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].selected) sel.push(opts[i]);
                    }
                    if (sel.length === 0 && !this.multiple && opts.length > 0) {
                        sel.push(opts[0]);
                    }
                    sel.item = function(i) { return sel[i] || null; };
                    return sel;
                }
                return undefined;
            },
            get text() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') return (this.textContent || '').trim();
                return this.textContent;
            },
            set text(val) {
                this.textContent = String(val);
            },
            get selected() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') {
                    var dyn = this.getAttribute('data-mango-selected');
                    if (dyn !== null) return dyn === 'true';
                    return this.hasAttribute('selected');
                }
                return false;
            },
            set selected(val) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') {
                    if (!this.hasAttribute("data-mango-default-selected")) {
                        this.setAttribute("data-mango-default-selected", this.hasAttribute("selected") ? "true" : "false");
                    }
                    var boolVal = !!val;
                    if (boolVal) {
                        this.setAttribute('data-mango-selected', 'true');
                        var p = this.parentElement;
                        while (p && (p.tagName || '').toLowerCase() !== 'select') {
                            p = p.parentElement;
                        }
                        if (p && !p.multiple) {
                            var all = p.options || [];
                            for (var i = 0; i < all.length; i++) {
                                if (all[i] !== this) {
                                    all[i].removeAttribute('selected');
                                    all[i].setAttribute('data-mango-selected', 'false');
                                }
                            }
                        }
                    } else {
                        this.removeAttribute('selected');
                        this.setAttribute('data-mango-selected', 'false');
                    }
                }
            },
            get defaultSelected() {
                var def = this.getAttribute("data-mango-default-selected");
                if (def !== null) return def === "true";
                return this.hasAttribute('selected');
            },
            set defaultSelected(val) {
                this.setAttribute("data-mango-default-selected", val ? "true" : "false");
                if (val) this.setAttribute('selected', '');
                else this.removeAttribute('selected');
            },
            get index() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') {
                    var p = this.parentElement;
                    while (p && (p.tagName || '').toLowerCase() !== 'select') {
                        p = p.parentElement;
                    }
                    if (p) {
                        var opts = p.options || [];
                        for (var i = 0; i < opts.length; i++) {
                            if (opts[i] === this) return i;
                        }
                    }
                }
                return 0;
            },
            get label() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'optgroup' || tag === 'option' || tag === 'track') {
                    var l = this.getAttribute('label');
                    return l === null ? '' : l;
                }
                return undefined;
            },
            set label(val) {
                this.setAttribute('label', String(val));
            },
            get files() {
                if (this._files) return this._files;
                var multiple = this.hasAttribute('multiple');
                this._files = { length: 0, item: function() { return null; }, 0: null };
                if (!multiple) { this._files.item = function() { return null; }; }
                return this._files;
            },
            get form() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'legend') {
                    var p = this.parentElement;
                    while (p) {
                        var pTag = (p.tagName || '').toLowerCase();
                        if (pTag === 'fieldset') return p.form;
                        if (pTag === 'form') return p;
                        p = p.parentElement;
                    }
                    return null;
                }
                var formAttr = this.getAttribute('form');
                if (formAttr && typeof document !== 'undefined' && document.getElementById) {
                    var f = document.getElementById(formAttr);
                    if (f && (f.tagName || '').toLowerCase() === 'form') return f;
                }
                var p = this._nodeId;
                while (p !== null && p !== undefined) {
                    var parentId = _getParent(p);
                    if (parentId === null || parentId === undefined) return null;
                    var el = _wrapElement(parentId);
                    if (el && el.tagName && el.tagName.toLowerCase() === 'form') return el;
                    p = parentId;
                }
                return null;
            },
            get elements() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form' || tag === 'fieldset') {
                    var list = this.querySelectorAll('input, button, select, textarea, output, fieldset');
                    var res = [];
                    for (var i = 0; i < list.length; i++) {
                        res[i] = list[i];
                        var name = list[i].getAttribute('name');
                        if (name && !(name in res)) res[name] = list[i];
                        var id = list[i].getAttribute('id');
                        if (id && !(id in res)) res[id] = list[i];
                    }
                    res.item = function(index) { return res[index] || null; };
                    res.namedItem = function(name) { return res[name] || null; };
                    return res;
                }
                return undefined;
            },
            get length() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    return (this.elements || []).length;
                }
                if (tag === 'select') {
                    return (this.options || []).length;
                }
                var l = this.getAttribute('length');
                return l === null ? 0 : parseInt(l, 10) || 0;
            },
            stepUp: function(n) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'input') return;
                var t = this.type;
                if (t !== 'number' && t !== 'range' && t !== 'date' && t !== 'time') return;
                var stepAttr = this.getAttribute('step');
                if (stepAttr && stepAttr.toLowerCase() === 'any') return;
                var step = parseFloat(stepAttr);
                if (isNaN(step) || step <= 0) step = 1.0;
                var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
                var cur = parseFloat(this.value);
                if (isNaN(cur)) {
                    var minVal = parseFloat(this.min);
                    cur = isNaN(minVal) ? 0.0 : minVal;
                }
                var next = cur + count * step;
                var min = parseFloat(this.min);
                if (!isNaN(min) && next < min) next = min;
                var max = parseFloat(this.max);
                if (!isNaN(max) && next > max) next = max;
                var decimals = 0;
                var stepStr = String(step);
                var dotIdx = stepStr.indexOf('.');
                if (dotIdx >= 0) decimals = stepStr.length - dotIdx - 1;
                this.value = decimals > 0 ? next.toFixed(decimals) : String(Math.round(next));
            },
            stepDown: function(n) {
                var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
                this.stepUp(-count);
            },
            submit: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    this.dispatchEvent({ type: 'submit', target: this });
                }
            },
            reset: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    var elems = this.elements || [];
                    for (var i = 0; i < elems.length; i++) {
                        var el = elems[i];
                        var elTag = (el.tagName || '').toLowerCase();
                        if (elTag === 'input') {
                            var t = el.type;
                            if (t === 'checkbox' || t === 'radio') {
                                el.checked = el.defaultChecked;
                            } else {
                                el.value = el.defaultValue;
                            }
                        } else if (elTag === 'textarea') {
                            el.value = el.defaultValue;
                        } else if (elTag === 'select') {
                            var opts = el.options || [];
                            for (var j = 0; j < opts.length; j++) {
                                opts[j].selected = opts[j].defaultSelected;
                            }
                        }
                    }
                    this.dispatchEvent({ type: 'reset', target: this });
                }
            },
            get validity() { return this._computeValidity(); },
            get validationMessage() {
                var v = this._computeValidity();
                if (v.valid) return "";
                var custom = this.getAttribute('data-mango-custom-validity');
                if (custom) return custom;
                if (v.valueMissing) return "Please fill out this field.";
                if (v.typeMismatch) return "Please match the requested type.";
                if (v.patternMismatch) return "Please match the requested format.";
                var minLen = this.getAttribute('minlength');
                if (v.tooShort && minLen !== null) return "Please lengthen this text to " + minLen + " characters or more.";
                var maxLen = this.getAttribute('maxlength');
                if (v.tooLong && maxLen !== null) return "Please shorten this text to " + maxLen + " characters or less.";
                if (v.rangeUnderflow) return "Value must be greater than or equal to " + this.min + ".";
                if (v.rangeOverflow) return "Value must be less than or equal to " + this.max + ".";
                if (v.stepMismatch) return "Please select a valid value.";
                if (v.badInput) return "Please enter a number.";
                return "Invalid value.";
            },
            get willValidate() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'input' && tag !== 'select' && tag !== 'textarea' && tag !== 'button') return false;
                var t = this.type;
                return !(t === 'hidden' || t === 'button' || t === 'reset' || this.disabled || this.readOnly);
            },
            checkValidity: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    var elems = this.elements || [];
                    var allValid = true;
                    var firstInvalid = null;
                    for (var i = 0; i < elems.length; i++) {
                        var el = elems[i];
                        if (el.willValidate && typeof el.checkValidity === 'function' && !el.checkValidity()) {
                            if (!firstInvalid) firstInvalid = el;
                            allValid = false;
                        }
                    }
                    return allValid;
                }
                var v = this._computeValidity();
                if (!v.valid) {
                    this.dispatchEvent({ type: 'invalid', target: this });
                }
                return v.valid;
            },
            reportValidity: function() { return this.checkValidity(); },
            setCustomValidity: function(message) {
                if (message) this.setAttribute('data-mango-custom-validity', String(message));
                else this.removeAttribute('data-mango-custom-validity');
            },
            _computeValidity: function() {
                var custom = this.getAttribute('data-mango-custom-validity');
                var result = {
                    valueMissing: false,
                    typeMismatch: false,
                    patternMismatch: false,
                    tooLong: false,
                    tooShort: false,
                    rangeUnderflow: false,
                    rangeOverflow: false,
                    stepMismatch: false,
                    badInput: false,
                    customError: !!custom,
                    valid: true
                };
                if (custom) { result.valid = false; return result; }
                if (!this.willValidate) return result;
                var val = this.value;
                var t = this.type;
                var tag = (this.tagName || '').toLowerCase();

                if (this.required) {
                    if (t === 'checkbox' || t === 'radio') {
                        if (!this.checked) { result.valueMissing = true; result.valid = false; }
                    } else if (tag === 'select') {
                        if (!val || !String(val).trim()) {
                            result.valueMissing = true; result.valid = false;
                        }
                    } else if (!val || !String(val).trim()) {
                        result.valueMissing = true; result.valid = false;
                    }
                }

                if (val && String(val).length > 0) {
                    var strVal = String(val);
                    if (t === 'email') {
                        var parts = this.hasAttribute('multiple') ? strVal.split(',') : [strVal];
                        for (var i = 0; i < parts.length; i++) {
                            var s = parts[i].trim();
                            var at = s.indexOf('@');
                            var domain = at === -1 ? '' : s.substring(at + 1);
                            if (at < 1 || domain.indexOf('.') < 1 || /\s/.test(domain)) {
                                result.typeMismatch = true; result.valid = false; break;
                            }
                        }
                    } else if (t === 'url') {
                        if (!/^([a-z][a-z0-9+.-]*:|\/\/)/i.test(strVal)) {
                            result.typeMismatch = true; result.valid = false;
                        }
                    } else if (t === 'number') {
                        var num = parseFloat(strVal);
                        if (isNaN(num)) {
                            result.badInput = true; result.valid = false;
                        }
                    }

                    // pattern check
                    var pattern = this.getAttribute('pattern');
                    if (pattern && (t === 'text' || t === 'search' || t === 'url' || t === 'tel' || t === 'email' || t === 'password')) {
                        try {
                            var regex = new RegExp("^(?:" + pattern + ")$");
                            if (!regex.test(strVal)) {
                                result.patternMismatch = true; result.valid = false;
                            }
                        } catch (e) {}
                    }

                    // range check (number, range, date, time)
                    if (t === 'number' || t === 'range') {
                        var num = parseFloat(strVal);
                        if (!isNaN(num)) {
                            var min = parseFloat(this.min);
                            if (!isNaN(min) && num < min) {
                                result.rangeUnderflow = true; result.valid = false;
                            }
                            var max = parseFloat(this.max);
                            if (!isNaN(max) && num > max) {
                                result.rangeOverflow = true; result.valid = false;
                            }
                            var stepAttr = this.getAttribute('step');
                            if (stepAttr && stepAttr.toLowerCase() !== 'any') {
                                var step = parseFloat(stepAttr);
                                if (!isNaN(step) && step > 0) {
                                    var base = !isNaN(min) ? min : 0;
                                    var diff = Math.abs(num - base);
                                    var rem = diff % step;
                                    if (rem > 0.0001 && Math.abs(rem - step) > 0.0001) {
                                        result.stepMismatch = true; result.valid = false;
                                    }
                                }
                            }
                        }
                    } else if (t === 'date' || t === 'time') {
                        var min = this.getAttribute('min');
                        if (min && strVal < min) {
                            result.rangeUnderflow = true; result.valid = false;
                        }
                        var max = this.getAttribute('max');
                        if (max && strVal > max) {
                            result.rangeOverflow = true; result.valid = false;
                        }
                    }
                }

                var minLen = this.getAttribute('minlength');
                if (minLen !== null && String(val).length < parseInt(minLen, 10)) { result.tooShort = true; result.valid = false; }
                var maxLen = this.getAttribute('maxlength');
                if (maxLen !== null && String(val).length > parseInt(maxLen, 10)) { result.tooLong = true; result.valid = false; }

                if (result.valueMissing || result.typeMismatch || result.patternMismatch ||
                    result.tooLong || result.tooShort || result.rangeUnderflow ||
                    result.rangeOverflow || result.stepMismatch || result.badInput || result.customError) {
                    result.valid = false;
                }
                return result;
            },
            get selectionStart() { var s = this.getAttribute('data-mango-selection-start'); return s === null ? 0 : parseInt(s, 10); },
            set selectionStart(val) { this.setAttribute('data-mango-selection-start', String(val)); },
            get selectionEnd() { var s = this.getAttribute('data-mango-selection-end'); return s === null ? 0 : parseInt(s, 10); },
            set selectionEnd(val) { this.setAttribute('data-mango-selection-end', String(val)); },
            get selectionDirection() { return ['forward', 'backward', 'none'].indexOf(this.getAttribute('data-mango-selection-direction')) >= 0 ? this.getAttribute('data-mango-selection-direction') : 'none'; },
            select: function() {
                var len = String(this.value === undefined ? '' : this.value).length;
                this.setSelectionRange(0, len);
            },
            setSelectionRange: function(start, end, direction) {
                start = start === undefined ? 0 : parseInt(start, 10) || 0;
                end = end === undefined ? start : parseInt(end, 10) || 0;
                this.setAttribute('data-mango-selection-start', String(start));
                this.setAttribute('data-mango-selection-end', String(end));
                this.setAttribute('data-mango-selection-direction', direction || 'none');
                this.dispatchEvent({ type: 'select', target: this });
            },
            get src() { return this.getAttribute("src") || ""; },
            set src(val) { this.setAttribute("src", val); },
            get currentSrc() { return this.src; },
            get poster() { return this.getAttribute("poster") || ""; },
            set poster(val) { this.setAttribute("poster", val); },
            get controls() { return this.hasAttribute("controls"); },
            set controls(val) { if (val) this.setAttribute("controls", ""); else this.removeAttribute("controls"); },
            get autoplay() { return this.hasAttribute("autoplay"); },
            set autoplay(val) { if (val) this.setAttribute("autoplay", ""); else this.removeAttribute("autoplay"); },
            get loop() { return this.hasAttribute("loop"); },
            set loop(val) { if (val) this.setAttribute("loop", ""); else this.removeAttribute("loop"); },
            get muted() { return this.hasAttribute("muted") || this.getAttribute("data-mango-muted") === "true"; },
            set muted(val) {
                if (val) { this.setAttribute("muted", ""); this.setAttribute("data-mango-muted", "true"); }
                else { this.removeAttribute("muted"); this.setAttribute("data-mango-muted", "false"); }
                this.dispatchEvent({ type: 'volumechange' });
            },
            get paused() {
                var p = this.getAttribute("data-mango-playing");
                if (p === "true") return false;
                if (p === "false") return true;
                return !this.autoplay;
            },
            get currentTime() {
                var t = parseFloat(this.getAttribute("data-mango-time") || "0");
                return isNaN(t) ? 0 : t;
            },
            set currentTime(val) {
                var num = Math.max(0, parseFloat(val) || 0);
                this.setAttribute("data-mango-time", String(num));
                this.dispatchEvent({ type: 'timeupdate' });
            },
            get duration() {
                var d = parseFloat(this.getAttribute("data-mango-duration") || "100");
                return isNaN(d) ? 100 : d;
            },
            get volume() {
                return this._volume !== undefined ? this._volume : 1.0;
            },
            set volume(val) {
                this._volume = Math.max(0, Math.min(1, parseFloat(val) || 0));
                this.dispatchEvent({ type: 'volumechange' });
            },
            get playbackRate() {
                return this._playbackRate !== undefined ? this._playbackRate : 1.0;
            },
            set playbackRate(val) {
                this._playbackRate = parseFloat(val) || 1.0;
                this.dispatchEvent({ type: 'ratechange' });
            },
            get readyState() { return 4; },
            get networkState() { return 1; },
            get ended() { return false; },
            get videoWidth() {
                var w = parseInt(this.getAttribute("width"), 10);
                return !isNaN(w) && w > 0 ? w : 640;
            },
            get videoHeight() {
                var h = parseInt(this.getAttribute("height"), 10);
                return !isNaN(h) && h > 0 ? h : 360;
            },
            play: function() {
                this.setAttribute("data-mango-playing", "true");
                var self = this;
                setTimeout(function() {
                    self.dispatchEvent({ type: 'play' });
                    self.dispatchEvent({ type: 'playing' });
                }, 0);
                return (typeof Promise !== 'undefined') ? Promise.resolve() : null;
            },
            pause: function() {
                this.setAttribute("data-mango-playing", "false");
                var self = this;
                setTimeout(function() {
                    self.dispatchEvent({ type: 'pause' });
                }, 0);
            },
            load: function() {
                var self = this;
                setTimeout(function() {
                    self.dispatchEvent({ type: 'loadstart' });
                    self.dispatchEvent({ type: 'loadedmetadata' });
                    self.dispatchEvent({ type: 'canplay' });
                }, 0);
            },
            canPlayType: function(type) {
                if (!type) return "";
                var t = String(type).toLowerCase();
                if (t.indexOf("mp4") !== -1 || t.indexOf("webm") !== -1 || t.indexOf("ogg") !== -1 ||
                    t.indexOf("audio/mpeg") !== -1 || t.indexOf("audio/mp3") !== -1 ||
                    t.indexOf("audio/wav") !== -1 || t.indexOf("audio/ogg") !== -1 || t.indexOf("audio/aac") !== -1) {
                    return "probably";
                }
                return "maybe";
            },
            get srcdoc() { return this.getAttribute("srcdoc") || ""; },
            set srcdoc(val) { this.setAttribute("srcdoc", val); },
            get width() {
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    var w = parseInt(this.getAttribute("width"), 10);
                    return (!isNaN(w) && w >= 0) ? w : 300;
                }
                return this.getAttribute("width") || "";
            },
            set width(val) {
                var num = parseInt(val, 10);
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    if (isNaN(num) || num < 0) num = 300;
                    this.setAttribute("width", String(num));
                    var h = parseInt(this.getAttribute("height"), 10);
                    if (isNaN(h) || h < 0) h = 150;
                    _canvasResize(this._nodeId, num, h);
                } else {
                    this.setAttribute("width", val);
                }
            },
            get height() {
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    var h = parseInt(this.getAttribute("height"), 10);
                    return (!isNaN(h) && h >= 0) ? h : 150;
                }
                return this.getAttribute("height") || "";
            },
            set height(val) {
                var num = parseInt(val, 10);
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    if (isNaN(num) || num < 0) num = 150;
                    this.setAttribute("height", String(num));
                    var w = parseInt(this.getAttribute("width"), 10);
                    if (isNaN(w) || w < 0) w = 300;
                    _canvasResize(this._nodeId, w, num);
                } else {
                    this.setAttribute("height", val);
                }
            },
            get sandbox() {
                var self = this;
                return {
                    get value() { return self.getAttribute("sandbox") || ""; },
                    set value(v) { self.setAttribute("sandbox", v); },
                    contains: function(token) {
                        var tokens = (self.getAttribute("sandbox") || "").split(/\s+/);
                        return tokens.indexOf(token) !== -1;
                    },
                    add: function(token) {
                        var tokens = (self.getAttribute("sandbox") || "").split(/\s+/).filter(Boolean);
                        if (tokens.indexOf(token) === -1) {
                            tokens.push(token);
                            self.setAttribute("sandbox", tokens.join(" "));
                        }
                    },
                    remove: function(token) {
                        var tokens = (self.getAttribute("sandbox") || "").split(/\s+/).filter(function(t) { return t !== token && t.length > 0; });
                        self.setAttribute("sandbox", tokens.join(" "));
                    },
                    toString: function() { return self.getAttribute("sandbox") || ""; }
                };
            },
            set sandbox(val) { this.setAttribute("sandbox", val); },
            // <dialog> surface
            get open() { return this.hasAttribute('open'); },
            set open(val) {
                if (val) this.setAttribute('open', '');
                else this.removeAttribute('open');
            },
            get returnValue() { return this._returnValue || ''; },
            set returnValue(val) { this._returnValue = String(val); },
            show: function() {
                this.setAttribute('open', '');
                this.removeAttribute('data-mango-modal');
            },
            showModal: function() {
                if (this.hasAttribute('open')) {
                    throw new Error("InvalidStateError: Dialog is already open");
                }
                this.setAttribute('data-mango-modal', 'true');
                this.setAttribute('open', '');
                this.focus();
            },
            close: function(retVal) {
                if (!this.hasAttribute('open')) return;
                if (retVal !== undefined) this.returnValue = String(retVal);
                this.removeAttribute('open');
                this.removeAttribute('data-mango-modal');
                this.dispatchEvent({ type: 'close', target: this });
            },
            // <datalist> & <select>
            get options() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'datalist') {
                    return this.getElementsByTagName('option');
                }
                if (tag === 'select') {
                    var list = this.getElementsByTagName('option');
                    var self = this;
                    list.add = function(opt, before) { self.add(opt, before); };
                    list.remove = function(index) { self.remove(index); };
                    return list;
                }
                return undefined;
            },
            add: function(opt, before) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'select') return;
                if (before === undefined || before === null) {
                    this.appendChild(opt);
                } else if (typeof before === 'number') {
                    var ref = (this.options || [])[before];
                    if (ref) this.insertBefore(opt, ref);
                    else this.appendChild(opt);
                } else {
                    this.insertBefore(opt, before);
                }
            },
            get list() {
                var listId = this.getAttribute('list');
                if (!listId || typeof document === 'undefined') return null;
                var el = document.getElementById(listId);
                return (el && el.tagName && el.tagName.toLowerCase() === 'datalist') ? el : null;
            },
            // <map> & <area>
            get areas() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'map') {
                    return this.getElementsByTagName('area');
                }
                return undefined;
            },
            get shape() { return this.getAttribute('shape') || 'rect'; },
            set shape(val) { this.setAttribute('shape', String(val)); },
            get coords() { return this.getAttribute('coords') || ''; },
            set coords(val) { this.setAttribute('coords', String(val)); },
            get alt() { return this.getAttribute('alt') || ''; },
            set alt(val) { this.setAttribute('alt', String(val)); },
            get target() { return this.getAttribute('target') || ''; },
            set target(val) { this.setAttribute('target', String(val)); },
            // <object> & <embed>
            get data() { return this.getAttribute('data') || ''; },
            set data(val) { this.setAttribute('data', String(val)); },
            get htmlFor() { return this.getAttribute('for') || ''; },
            set htmlFor(val) { this.setAttribute('for', String(val)); },
            get contentDocument() {
                if (this.tagName && this.tagName.toLowerCase() === 'iframe') {
                    if (!this._contentDocument) {
                        this._contentDocument = {
                            readyState: 'complete',
                            body: _wrapElement(_origCE('body')),
                            head: _wrapElement(_origCE('head')),
                            createElement: function(t) { return _wrapElement(_origCE(t)); },
                            createTextNode: function(t) { return _wrapElement(_origCTN(t)); },
                            getElementById: function(id) { return null; },
                            querySelector: function(s) { return null; },
                            querySelectorAll: function(s) { return []; }
                        };
                    }
                    return this._contentDocument;
                }
                return null;
            },
            get contentWindow() {
                if (this.tagName && this.tagName.toLowerCase() === 'iframe') {
                    if (!this._contentWindow) {
                        var doc = this.contentDocument;
                        this._contentWindow = {
                            document: doc,
                            frameElement: this,
                            postMessage: function(msg, targetOrigin) {},
                            addEventListener: function() {},
                            removeEventListener: function() {}
                        };
                    }
                    return this._contentWindow;
                }
                return null;
            },
            appendChild: function(child) {
                var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
                _appendChild(this._nodeId, childId);
                var childObj = _wrapElement(childId);
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [childObj],
                        removedNodes: []
                    });
                }
                return child;
            },
            removeChild: function(child) {
                var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
                var childObj = _wrapElement(childId);
                _removeChild(this._nodeId, childId);
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [],
                        removedNodes: [childObj]
                    });
                }
                return child;
            },
            insertBefore: function(newChild, refChild) {
                var childId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
                var refId = (typeof refChild === 'object' && refChild !== null && refChild._nodeId !== undefined) ? refChild._nodeId : refChild;
                _insertBefore(this._nodeId, childId, (refId === undefined || refId === null) ? 0 : refId);
                var newObj = _wrapElement(childId);
                var refObj = (refId && refId !== 0) ? _wrapElement(refId) : null;
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [newObj],
                        removedNodes: [],
                        nextSibling: refObj
                    });
                }
                return newChild;
            },
            replaceChild: function(newChild, oldChild) {
                var newId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
                var oldId = (typeof oldChild === 'object' && oldChild !== null && oldChild._nodeId !== undefined) ? oldChild._nodeId : oldChild;
                var oldObj = _wrapElement(oldId);
                var newObj = _wrapElement(newId);
                _replaceChild(this._nodeId, newId, oldId);
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [newObj],
                        removedNodes: [oldObj]
                    });
                }
                return oldChild;
            },
            insertAdjacentElement: function(position, element) {
                var pos = String(position).toLowerCase();
                var parentId = _getParent(this._nodeId);
                var parentObj = parentId !== null ? _wrapElement(parentId) : null;
                if (pos === 'beforeend') {
                    this.appendChild(element);
                } else if (pos === 'afterbegin') {
                    this.insertBefore(element, this.firstChild);
                } else if (pos === 'beforebegin') {
                    if (!parentObj) return null;
                    parentObj.insertBefore(element, this);
                } else if (pos === 'afterend') {
                    if (!parentObj) return null;
                    parentObj.insertBefore(element, this.nextSibling);
                } else {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.", "SyntaxError") : new SyntaxError("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
                    err.name = "SyntaxError";
                    throw err;
                }
                return element;
            },
            insertAdjacentHTML: function(position, html) {
                var pos = String(position).toLowerCase();
                if (pos !== 'beforebegin' && pos !== 'afterbegin' && pos !== 'beforeend' && pos !== 'afterend') {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.", "SyntaxError") : new SyntaxError("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
                    err.name = "SyntaxError";
                    throw err;
                }
                var container = _wrapElement(_createFragment());
                container.innerHTML = String(html);
                var parentId = _getParent(this._nodeId);
                var parentObj = parentId !== null ? _wrapElement(parentId) : null;
                if (pos === 'beforeend') {
                    this.appendChild(container);
                } else if (pos === 'afterbegin') {
                    this.insertBefore(container, this.firstChild);
                } else if (pos === 'beforebegin') {
                    if (!parentObj) return;
                    parentObj.insertBefore(container, this);
                } else if (pos === 'afterend') {
                    if (!parentObj) return;
                    parentObj.insertBefore(container, this.nextSibling);
                }
            },
            insertAdjacentText: function(position, text) {
                var node = _wrapElement(_origCTN(String(text)));
                return this.insertAdjacentElement(position, node);
            },
            setAttribute: function(name, value) {
                var oldVal = _getAttribute(this._nodeId, name);
                _setAttribute(this._nodeId, name, String(value));
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    var lower = String(name).toLowerCase();
                    if (lower === 'width' || lower === 'height') {
                        _canvasResize(this._nodeId, this.width, this.height);
                    }
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'attributes',
                        target: this,
                        attributeName: String(name),
                        oldValue: oldVal
                    });
                }
            },
            getAttribute: function(name) { return _getAttribute(this._nodeId, name); },
            hasAttribute: function(name) { return _getAttribute(this._nodeId, name) !== null; },
            removeAttribute: function(name) {
                var oldVal = _getAttribute(this._nodeId, name);
                _removeAttribute(this._nodeId, name);
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'attributes',
                        target: this,
                        attributeName: String(name),
                        oldValue: oldVal
                    });
                }
            },
            getAttributeNS: function(ns, name) { return this.getAttribute(name); },
            setAttributeNS: function(ns, name, value) { this.setAttribute(name, value); },
            removeAttributeNS: function(ns, name) { this.removeAttribute(name); },
            hasAttributeNS: function(ns, name) { return this.hasAttribute(name); },
            attachShadow: function(init) {
                var srId = _attachShadow(this._nodeId);
                var sr = (srId !== null && srId !== undefined) ? _wrapElement(srId) : _wrapElement(_createFragment());
                sr.mode = (init && init.mode) || 'open';
                sr.host = this;
                this.shadowRoot = (sr.mode === 'open') ? sr : null;
                this._shadowRoot = sr;
                return sr;
            },
            assignedNodes: function(opts) {
                if (this.tagName && this.tagName.toLowerCase() === 'slot') {
                    var p = this.parentElement;
                    while (p && !p.host) p = p.parentElement;
                    var src = (p && p.host) ? p.host.childNodes : this.childNodes;
                    var arr = [];
                    if (src) {
                        for (var i = 0; i < src.length; i++) arr.push(src[i]);
                    }
                    return arr;
                }
                return [];
            },
            assignedElements: function(opts) {
                return (this.assignedNodes(opts) || []).filter(function(n) { return n.nodeType === 1; });
            },
            toDataURL: function(type, quality) {
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    return _canvasToDataUrl(this._nodeId, type || 'image/png');
                }
                return "";
            },
            getContext: function(type) {
                if (type === '2d') {
                    if (!this._context2d) {
                        this._context2d = _createCanvasRenderingContext2D(this);
                    }
                    return this._context2d;
                }
                return null;
            },
            animate: function(keyframes, options) {
                return {
                    play: function() {},
                    pause: function() {},
                    cancel: function() {},
                    finish: function() {},
                    reverse: function() {},
                    currentTime: 0,
                    startTime: 0,
                    playbackRate: 1,
                    playState: 'running',
                    onfinish: null,
                    oncancel: null,
                    finished: (typeof Promise !== 'undefined') ? Promise.resolve(this) : null
                };
            },
            addEventListener: function(type, listener, options) {
                _addDOMEventListener(this, type, listener, options);
            },
            removeEventListener: function(type, listener, options) {
                _removeDOMEventListener(this, type, listener, options);
            },
            getBoundingClientRect: function() {
                var r = _getElementRect(this._nodeId) || [0, 0, 0, 0];
                var x = r[0];
                var y = r[1];
                var w = r[2];
                var h = r[3];
                if (typeof globalThis !== 'undefined' && globalThis.DOMRect) {
                    try { return new globalThis.DOMRect(x, y, w, h); } catch(e) {}
                }
                var rect = {
                    x: x, y: y, top: y, left: x,
                    right: x + w, bottom: y + h,
                    width: w, height: h
                };
                rect.toJSON = function() {
                    return { x: this.x, y: this.y, top: this.top, left: this.left, right: this.right, bottom: this.bottom, width: this.width, height: this.height };
                };
                return rect;
            },
            getClientRects: function() {
                var r = this.getBoundingClientRect();
                if (typeof globalThis !== 'undefined' && globalThis.DOMRectList) {
                    return new globalThis.DOMRectList([r]);
                }
                return [r];
            },
            contains: function(other) {
                if (other === null || other === undefined) return false;
                var otherId = (typeof other === 'object') ? other._nodeId : other;
                if (otherId === undefined || otherId === null) return false;
                return _elementContains(this._nodeId, otherId);
            },
            hasChildNodes: function() { return (_getChildren(this._nodeId) || []).length > 0 || (_getChildNodeIds(this._nodeId) || []).length > 0; },
            normalize: function() {},
            getRootNode: function() { return document; },
            querySelector: function(sel) {
                var id = _querySelector(this._nodeId, String(sel));
                return id !== null ? _wrapElement(id) : null;
            },
            querySelectorAll: function(sel) {
                var ids = _querySelectorAll(this._nodeId, String(sel)) || [];
                var list = ids.map(_wrapElement);
                return _createNodeList(function() { return list; });
            },
            getElementsByTagName: function(tag) {
                var self = this;
                return _createHTMLCollection(function() {
                    var ids = _querySelectorAll(self._nodeId, String(tag)) || [];
                    return ids.map(_wrapElement);
                });
            },
            getElementsByClassName: function(cls) {
                var self = this;
                return _createHTMLCollection(function() {
                    var ids = _querySelectorAll(self._nodeId, "." + String(cls)) || [];
                    return ids.map(_wrapElement);
                });
            },
            focus: function(options) {
                if (this.disabled) return;
                var prev = document.activeElement;
                if (prev === this) return;
                document.activeElement = this;
                _setFocusState(this._nodeId, true);
                if (prev && prev !== this) {
                    if (prev._nodeId) _setFocusState(prev._nodeId, false);
                    if (prev.dispatchEvent) {
                        var bEvt = (typeof globalThis.FocusEvent === 'function')
                            ? new globalThis.FocusEvent('blur', { bubbles: false, relatedTarget: this })
                            : { type: 'blur', target: prev, relatedTarget: this, bubbles: false };
                        prev.dispatchEvent(bEvt);
                        var foEvt = (typeof globalThis.FocusEvent === 'function')
                            ? new globalThis.FocusEvent('focusout', { bubbles: true, relatedTarget: this })
                            : { type: 'focusout', target: prev, relatedTarget: this, bubbles: true };
                        prev.dispatchEvent(foEvt);
                    }
                }
                var fEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('focus', { bubbles: false, relatedTarget: prev })
                    : { type: 'focus', target: this, relatedTarget: prev, bubbles: false };
                this.dispatchEvent(fEvt);
                var fiEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('focusin', { bubbles: true, relatedTarget: prev })
                    : { type: 'focusin', target: this, relatedTarget: prev, bubbles: true };
                this.dispatchEvent(fiEvt);
            },
            blur: function() {
                _setFocusState(this._nodeId, false);
                var wasActive = (document.activeElement === this);
                if (wasActive) {
                    document.activeElement = document.body || null;
                }
                var next = document.activeElement;
                var bEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('blur', { bubbles: false, relatedTarget: next })
                    : { type: 'blur', target: this, relatedTarget: next, bubbles: false };
                this.dispatchEvent(bEvt);
                var foEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('focusout', { bubbles: true, relatedTarget: next })
                    : { type: 'focusout', target: this, relatedTarget: next, bubbles: true };
                this.dispatchEvent(foEvt);
            },
            click: function() {
                if (this.disabled) return;
                var evt;
                if (typeof globalThis.MouseEvent === 'function') {
                    try {
                        evt = new globalThis.MouseEvent('click', {
                            bubbles: true,
                            cancelable: true,
                            view: (typeof window !== 'undefined' ? window : null),
                            detail: 1
                        });
                    } catch(e) {
                        evt = { type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false };
                    }
                } else {
                    evt = { type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false };
                }
                var notPrevented = this.dispatchEvent(evt);
                if (notPrevented !== false && !evt.defaultPrevented) {
                    var tag = (this.tagName || '').toLowerCase();
                    if (tag === 'input') {
                        var itype = (this.type || this.getAttribute('type') || 'text').toLowerCase();
                        if (itype === 'checkbox') {
                            this.checked = !this.checked;
                        } else if (itype === 'radio') {
                            this.checked = true;
                        } else if (itype === 'submit') {
                            var f = this.form || this.closest('form');
                            if (f && f.submit) f.submit();
                        } else if (itype === 'reset') {
                            var f = this.form || this.closest('form');
                            if (f && f.reset) f.reset();
                        }
                    } else if (tag === 'button') {
                        var btype = (this.type || this.getAttribute('type') || 'submit').toLowerCase();
                        if (btype === 'submit') {
                            var f = this.form || this.closest('form');
                            if (f && f.submit) f.submit();
                        } else if (btype === 'reset') {
                            var f = this.form || this.closest('form');
                            if (f && f.reset) f.reset();
                        }
                    }
                }
            },
            getAttributeNames: function() { return _getAttributeNames(this._nodeId) || []; },
            getElementsByName: function(name) {
                var self = this;
                return _createNodeList(function() {
                    var ids = _querySelectorAll(self._nodeId, '[name="' + String(name) + '"]') || [];
                    return ids.map(_wrapElement);
                });
            },
            getAttributeNode: function(name) {
                var v = _getAttribute(this._nodeId, name);
                return v === null ? null : { name: name, value: v, specified: true };
            },
            toggleAttribute: function(name, force) {
                var has = this.hasAttribute(name);
                var shouldAdd = (force === undefined) ? !has : !!force;
                if (shouldAdd) this.setAttribute(name, '');
                else this.removeAttribute(name);
                return shouldAdd;
            },
            setPointerCapture: function() {},
            releasePointerCapture: function() {},
            get parentNode() {
                var p = _getParent(this._nodeId);
                return p !== null ? _wrapElement(p) : document;
            },
            get parentElement() {
                var p = _getParent(this._nodeId);
                return p !== null ? _wrapElement(p) : null;
            },
            get children() {
                var self = this;
                return _createHTMLCollection(function() {
                    var arr = _getChildren(self._nodeId) || [];
                    return arr.map(_wrapElement);
                });
            },
            get childNodes() {
                var self = this;
                return _createNodeList(function() {
                    var arr = _getChildNodeIds(self._nodeId) || [];
                    return arr.map(_wrapElement);
                });
            },
            get firstChild() {
                var id = _getFirstChild(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get lastChild() {
                var id = _getLastChild(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get nextSibling() {
                var id = _getNextSibling(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get previousSibling() {
                var id = _getPreviousSibling(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get childElementCount() {
                return (_getChildren(this._nodeId) || []).length;
            },
            get firstElementChild() {
                var c = _getFirstElementChild(this._nodeId);
                return c !== null ? _wrapElement(c) : null;
            },
            get lastElementChild() {
                var c = _getLastElementChild(this._nodeId);
                return c !== null ? _wrapElement(c) : null;
            },
            get nextElementSibling() {
                var s = _getNextElementSibling(this._nodeId);
                return s !== null ? _wrapElement(s) : null;
            },
            get previousElementSibling() {
                var s = _getPreviousElementSibling(this._nodeId);
                return s !== null ? _wrapElement(s) : null;
            },
            get offsetWidth() { return Math.round(this.getBoundingClientRect().width); },
            get offsetHeight() { return Math.round(this.getBoundingClientRect().height); },
            get offsetParent() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var cs = window.getComputedStyle(this);
                    if (cs && cs.position === 'fixed') return null;
                }
                if (this.tagName === 'BODY' || this.tagName === 'HTML') return null;
                var cur = this.parentElement;
                while (cur && cur.tagName !== 'BODY' && cur.tagName !== 'HTML') {
                    if (typeof window !== 'undefined' && window.getComputedStyle) {
                        var s = window.getComputedStyle(cur);
                        if (s && s.position && s.position !== 'static') return cur;
                    }
                    cur = cur.parentElement;
                }
                if (cur && cur.tagName === 'BODY') return cur;
                return null;
            },
            get offsetLeft() {
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().left - p.getBoundingClientRect().left);
                return Math.round(this.getBoundingClientRect().left);
            },
            get offsetTop() {
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().top - p.getBoundingClientRect().top);
                return Math.round(this.getBoundingClientRect().top);
            },
            get clientWidth() {
                var bl = 0, br = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    bl = parseFloat(s.borderLeftWidth) || 0;
                    br = parseFloat(s.borderRightWidth) || 0;
                }
                return Math.max(0, Math.round(this.getBoundingClientRect().width - bl - br));
            },
            get clientHeight() {
                var bt = 0, bb = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    bt = parseFloat(s.borderTopWidth) || 0;
                    bb = parseFloat(s.borderBottomWidth) || 0;
                }
                return Math.max(0, Math.round(this.getBoundingClientRect().height - bt - bb));
            },
            get clientTop() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderTopWidth) || 0);
                }
                return 0;
            },
            get clientLeft() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderLeftWidth) || 0);
                }
                return 0;
            },
            get scrollWidth() { return Math.max(this.clientWidth, Math.round(this.getBoundingClientRect().width)); },
            get scrollHeight() { return Math.max(this.clientHeight, Math.round(this.getBoundingClientRect().height)); },
            get scrollTop() { return this._scrollTop || 0; },
            set scrollTop(v) {
                this._scrollTop = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {
                    if (typeof window !== 'undefined') window.scrollY = this._scrollTop;
                }
            },
            get scrollLeft() { return this._scrollLeft || 0; },
            set scrollLeft(v) {
                this._scrollLeft = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {
                    if (typeof window !== 'undefined') window.scrollX = this._scrollLeft;
                }
            },
            scrollTo: function(x, y) {
                if (typeof x === 'object' && x !== null) {
                    if (x.left !== undefined) this.scrollLeft = x.left;
                    if (x.top !== undefined) this.scrollTop = x.top;
                } else {
                    if (x !== undefined) this.scrollLeft = x;
                    if (y !== undefined) this.scrollTop = y;
                }
            },
            scrollBy: function(x, y) {
                if (typeof x === 'object' && x !== null) {
                    if (x.left !== undefined) this.scrollLeft += x.left;
                    if (x.top !== undefined) this.scrollTop += x.top;
                } else {
                    if (x !== undefined) this.scrollLeft += x;
                    if (y !== undefined) this.scrollTop += y;
                }
            },
            scroll: function(x, y) {
                this.scrollTo(x, y);
            },
            scrollIntoView: function(options) {
                var rect = this.getBoundingClientRect();
                var alignToTop = (options === undefined || options === true || (typeof options === 'object' && options.block !== 'end'));
                var top = (window.scrollY || 0) + (alignToTop ? rect.top : (rect.bottom - ((typeof window !== 'undefined' && window.innerHeight) || 600)));
                var left = (window.scrollX || 0) + rect.left;
                var behavior = (typeof options === 'object' && options.behavior) || 'auto';
                if (typeof window.scrollTo === 'function') {
                    window.scrollTo({ top: top, left: left, behavior: behavior });
                }
            },
            cloneNode: function(deep) { return _wrapElement(_cloneNode(this._nodeId, !!deep)); },
            get isConnected() { return _getParent(this._nodeId) !== null; },
            matches: function(sel) { return _elementMatches(this._nodeId, String(sel)); },
            closest: function(sel) {
                var matchId = _elementClosest(this._nodeId, String(sel));
                return matchId !== null ? _wrapElement(matchId) : null;
            },
            // DOM insertion conveniences. `append`/`prepend` accept Node or string.
            _toNode: function(item) {
                if (item !== null && typeof item === 'object' && (item._nodeId !== undefined || item.nodeType !== undefined)) {
                    return item;
                }
                return _wrapElement(_origCTN(String(item)));
            },
            append: function() {
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                this.appendChild(frag);
            },
            prepend: function() {
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                var first = this.firstChild;
                if (first) {
                    this.insertBefore(frag, first);
                } else {
                    this.appendChild(frag);
                }
            },
            before: function() {
                var parent = this.parentElement || this.parentNode;
                if (!parent || parent === document) {
                    var pId = _getParent(this._nodeId);
                    if (pId === null) return;
                    parent = _wrapElement(pId);
                }
                if (!parent) return;
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                parent.insertBefore(frag, this);
            },
            after: function() {
                var parent = this.parentElement || this.parentNode;
                if (!parent || parent === document) {
                    var pId = _getParent(this._nodeId);
                    if (pId === null) return;
                    parent = _wrapElement(pId);
                }
                if (!parent) return;
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                parent.insertBefore(frag, this.nextSibling);
            },
            replaceWith: function() {
                var parent = this.parentElement || this.parentNode;
                if (!parent || parent === document) {
                    var pId = _getParent(this._nodeId);
                    if (pId === null) return;
                    parent = _wrapElement(pId);
                }
                if (!parent) return;
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                parent.insertBefore(frag, this);
                parent.removeChild(this);
            },
            replaceChildren: function() {
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                while (this.firstChild) {
                    this.removeChild(this.firstChild);
                }
                this.appendChild(frag);
            },
            remove: function(index) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select' && arguments.length > 0 && typeof index === 'number') {
                    var opts = this.options || [];
                    var opt = opts[index];
                    if (opt) {
                        var pId = _getParent(opt._nodeId);
                        if (pId !== null) {
                            var pObj = _wrapElement(pId);
                            if (pObj && pObj.removeChild) pObj.removeChild(opt);
                            else _removeChild(pId, opt._nodeId);
                        }
                    }
                    return;
                }
                var parentId = _getParent(this._nodeId);
                if (parentId !== null) {
                    var parentObj = _wrapElement(parentId);
                    if (parentObj && parentObj.removeChild) {
                        parentObj.removeChild(this);
                    } else {
                        _removeChild(parentId, this._nodeId);
                    }
                }
            },
            dispatchEvent: function(evt) {
                return _dispatchDOMEvent(this, evt);
            }
        };

        var tag = el.tagName ? el.tagName.toLowerCase() : '';
        var nt = el.nodeType;
        if (typeof globalThis !== 'undefined') {
            if (nt === 11 && globalThis.DocumentFragment) {
                Object.setPrototypeOf(el, globalThis.DocumentFragment.prototype);
            } else if (nt === 9 && globalThis.Document) {
                Object.setPrototypeOf(el, globalThis.Document.prototype);
            } else if (nt === 3 && globalThis.Text) {
                Object.setPrototypeOf(el, globalThis.Text.prototype);
            } else if (nt === 8 && globalThis.Comment) {
                Object.setPrototypeOf(el, globalThis.Comment.prototype);
            } else {
                var customCtor = (globalThis.customElements && globalThis.customElements.get) ? globalThis.customElements.get(tag) : null;
                if (customCtor && customCtor.prototype) {
                    Object.setPrototypeOf(el, customCtor.prototype);
                } else if (tag === 'template' && globalThis.HTMLTemplateElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLTemplateElement.prototype);
                } else if (tag === 'slot' && globalThis.HTMLSlotElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLSlotElement.prototype);
                } else if (tag === 'dialog' && globalThis.HTMLDialogElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLDialogElement.prototype);
                } else if (tag === 'progress' && globalThis.HTMLProgressElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLProgressElement.prototype);
                } else if (tag === 'meter' && globalThis.HTMLMeterElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLMeterElement.prototype);
                } else if (tag === 'output' && globalThis.HTMLOutputElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLOutputElement.prototype);
                } else if (tag === 'datalist' && globalThis.HTMLDataListElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLDataListElement.prototype);
                } else if (tag === 'map' && globalThis.HTMLMapElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLMapElement.prototype);
                } else if (tag === 'area' && globalThis.HTMLAreaElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLAreaElement.prototype);
                } else if (tag === 'object' && globalThis.HTMLObjectElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLObjectElement.prototype);
                } else if (tag === 'embed' && globalThis.HTMLEmbedElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLEmbedElement.prototype);
                } else if (tag === 'ruby' && globalThis.HTMLRubyElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLRubyElement.prototype);
                } else if (tag === 'img' && globalThis.HTMLImageElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLImageElement.prototype);
                } else if (tag === 'iframe' && globalThis.HTMLIFrameElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLIFrameElement.prototype);
                } else if (tag === 'canvas' && globalThis.HTMLCanvasElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLCanvasElement.prototype);
                } else if (tag === 'video' && globalThis.HTMLVideoElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLVideoElement.prototype);
                } else if (tag === 'audio' && globalThis.HTMLAudioElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLAudioElement.prototype);
                } else if (tag === 'input' && globalThis.HTMLInputElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLInputElement.prototype);
                } else if (tag === 'select' && globalThis.HTMLSelectElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLSelectElement.prototype);
                } else if (tag === 'option' && globalThis.HTMLOptionElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLOptionElement.prototype);
                } else if (tag === 'optgroup' && globalThis.HTMLOptGroupElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLOptGroupElement.prototype);
                } else if (tag === 'form' && globalThis.HTMLFormElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLFormElement.prototype);
                } else if (tag === 'fieldset' && globalThis.HTMLFieldSetElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLFieldSetElement.prototype);
                } else if (tag === 'legend' && globalThis.HTMLLegendElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLLegendElement.prototype);
                } else if (tag === 'textarea' && globalThis.HTMLTextAreaElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLTextAreaElement.prototype);
                } else if (tag === 'button' && globalThis.HTMLButtonElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLButtonElement.prototype);
                } else if (tag === 'a' && globalThis.HTMLAnchorElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLAnchorElement.prototype);
                } else if (tag === 'div' && globalThis.HTMLDivElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLDivElement.prototype);
                } else if (tag === 'span' && globalThis.HTMLSpanElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLSpanElement.prototype);
                } else if (globalThis.HTMLElement) {
                    Object.setPrototypeOf(el, globalThis.HTMLElement.prototype);
                }
            }
        }
        _wrapperCache[nodeId] = el;
        return el;
    }

    var _origGetById = document.getElementById;
    document.getElementById = function(id) { return _wrapElement(_origGetById(id)); };
    // Chrome-internal handle used by Rust-triggered scripts to reach the cached
    // wrapper (and therefore its listeners) for a specific node id.
    try { globalThis._mangoWrap = _wrapElement; } catch (e) {}
    document.querySelector = function(sel) {
        var id = _querySelector(0, String(sel));
        return id !== null ? _wrapElement(id) : null;
    };
    document.querySelectorAll = function(sel) {
        var ids = _querySelectorAll(0, String(sel)) || [];
        var list = ids.map(_wrapElement);
        return _createNodeList(function() { return list; });
    };
    document.getElementsByTagName = function(tag) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, String(tag)) || [];
            return ids.map(_wrapElement);
        });
    };
    document.getElementsByClassName = function(cls) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, "." + String(cls)) || [];
            return ids.map(_wrapElement);
        });
    };
    var _origCE = document.createElement;
    document.createElement = function(tag) { return _wrapElement(_origCE(tag)); };
    document.createElementNS = function(ns, tag) { return _wrapElement(_origCE(tag)); };
    var _origCTN = document.createTextNode;
    document.createTextNode = function(text) { return _wrapElement(_origCTN(text)); };

    document.addEventListener = function(type, listener, options) {
        _addDOMEventListener(document, type, listener, options);
        if (type === 'DOMContentLoaded' || type === 'load') {
            setTimeout(function() {
                try {
                    if (typeof listener === 'function') listener.call(document, { type: type, target: document, bubbles: false });
                    else if (listener && typeof listener.handleEvent === 'function') listener.handleEvent({ type: type, target: document, bubbles: false });
                } catch(e) {}
            }, 0);
        }
    };
    document.removeEventListener = function(type, listener, options) {
        _removeDOMEventListener(document, type, listener, options);
    };
    document.dispatchEvent = function(evt) {
        return _dispatchDOMEvent(document, evt);
    };
    // `document.cookie` — a live view over the shared, persisted cookie jar.
    // HttpOnly cookies never appear here; assignments are applied to the jar.
    Object.defineProperty(document, 'cookie', {
        get: function() { return _getDocumentCookie(); },
        set: function(v) { _setDocumentCookie(String(v)); },
        configurable: true
    });
    Object.defineProperty(document, 'compatMode', {
        get: function() { return _getCompatMode(); }
    });
    Object.defineProperty(document, 'characterSet', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'charset', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'inputEncoding', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'contentType', {
        get: function() { return _getContentType(); },
        configurable: true
    });
    document.readyState = "complete";
    try {
        document.location = globalThis.location;
    } catch(e) {}
    document.createDocumentFragment = function() {
        return _wrapElement(_createFragment());
    };
    Object.defineProperty(document, 'forms', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('forms') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'images', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('images') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'links', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('links') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'anchors', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('anchors') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'scripts', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('scripts') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'embeds', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('embeds') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'children', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getChildren(0) || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'childNodes', {
        get: function() {
            return _createNodeList(function() {
                return (_getChildNodeIds(0) || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'firstChild', {
        get: function() {
            var id = _getFirstChild(0);
            return id !== null ? _wrapElement(id) : null;
        }
    });
    Object.defineProperty(document, 'lastChild', {
        get: function() {
            var id = _getLastChild(0);
            return id !== null ? _wrapElement(id) : null;
        }
    });
    Object.defineProperty(document, 'childElementCount', {
        get: function() {
            return (_getChildren(0) || []).length;
        }
    });
    document.getElementsByName = function(name) {
        return _createNodeList(function() {
            var ids = _querySelectorAll(0, '[name="' + String(name) + '"]') || [];
            return ids.map(_wrapElement);
        });
    };
    document.appendChild = function(child) {
        var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
        _appendChild(0, childId);
        var childObj = _wrapElement(childId);
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [childObj],
                removedNodes: []
            });
        }
        return child;
    };
    document.removeChild = function(child) {
        var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
        var childObj = _wrapElement(childId);
        _removeChild(0, childId);
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [],
                removedNodes: [childObj]
            });
        }
        return child;
    };
    document.insertBefore = function(newChild, refChild) {
        var childId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
        var refId = (typeof refChild === 'object' && refChild !== null && refChild._nodeId !== undefined) ? refChild._nodeId : refChild;
        _insertBefore(0, childId, (refId === undefined || refId === null) ? 0 : refId);
        var newObj = _wrapElement(childId);
        var refObj = (refId && refId !== 0) ? _wrapElement(refId) : null;
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [newObj],
                removedNodes: [],
                nextSibling: refObj
            });
        }
        return newChild;
    };
    document.replaceChild = function(newChild, oldChild) {
        var newId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
        var oldId = (typeof oldChild === 'object' && oldChild !== null && oldChild._nodeId !== undefined) ? oldChild._nodeId : oldChild;
        var oldObj = _wrapElement(oldId);
        var newObj = _wrapElement(newId);
        _replaceChild(0, newId, oldId);
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [newObj],
                removedNodes: [oldObj]
            });
        }
        return oldChild;
    };
    document.cloneNode = function(deep) {
        return _wrapElement(_cloneNode(0, !!deep));
    };
    document.elementFromPoint = function(x, y) {
        var id = _elementFromPoint(Number(x), Number(y));
        return id === null ? null : _wrapElement(id);
    };
    document.elementsFromPoint = function(x, y) {
        if (typeof _elementsFromPoint === 'function') {
            var ids = _elementsFromPoint(Number(x), Number(y));
            if (ids && ids.length > 0) {
                return ids.map(_wrapElement);
            }
        }
        var el = document.elementFromPoint(x, y);
        return el === null ? [] : [el];
    };
    var _docActiveElement = null;
    Object.defineProperty(document, 'activeElement', {
        get: function() { return _docActiveElement || document.body || null; },
        set: function(el) { _docActiveElement = el; },
        configurable: true
    });
    document.hasChildNodes = function() { return (_getChildNodeIds(0) || []).length > 0; };
    document.contains = function(other) {
        if (!other) return false;
        var otherId = (other && other._nodeId !== undefined) ? other._nodeId : other;
        return _elementContains(0, otherId);
    };

    document.append = function() {
        var frag = _wrapElement(_createFragment());
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined))
                ? arg : _wrapElement(_origCTN(String(arg)));
            frag.appendChild(node);
        }
        document.appendChild(frag);
    };
    document.prepend = function() {
        var frag = _wrapElement(_createFragment());
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined))
                ? arg : _wrapElement(_origCTN(String(arg)));
            frag.appendChild(node);
        }
        var first = document.firstChild;
        if (first) document.insertBefore(frag, first);
        else document.appendChild(frag);
    };
    document.replaceChildren = function() {
        var frag = _wrapElement(_createFragment());
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined))
                ? arg : _wrapElement(_origCTN(String(arg)));
            frag.appendChild(node);
        }
        while (document.firstChild) {
            document.removeChild(document.firstChild);
        }
        document.appendChild(frag);
    };

    function _getNodeIndex(node) {
        if (!node || !node.parentNode) return 0;
        var siblings = node.parentNode.childNodes || [];
        for (var i = 0; i < siblings.length; i++) {
            if (siblings[i] === node || (siblings[i]._nodeId !== undefined && siblings[i]._nodeId === node._nodeId)) {
                return i;
            }
        }
        return 0;
    }

    function _getNodeLength(node) {
        if (!node) return 0;
        if (node.nodeType === 3 || node.nodeType === 8) {
            return (node.nodeValue !== null && node.nodeValue !== undefined) ? String(node.nodeValue).length : (node.textContent || '').length;
        }
        return (node.childNodes || []).length;
    }

    function _getCommonAncestor(a, b) {
        if (!a || !b) return a || b || document;
        if (a === b || (a._nodeId !== undefined && a._nodeId === b._nodeId)) return a;
        var aAncestors = [];
        var cur = a;
        while (cur) {
            aAncestors.push(cur);
            cur = cur.parentNode;
        }
        cur = b;
        while (cur) {
            for (var i = 0; i < aAncestors.length; i++) {
                if (aAncestors[i] === cur || (aAncestors[i]._nodeId !== undefined && aAncestors[i]._nodeId === cur._nodeId)) {
                    return cur;
                }
            }
            cur = cur.parentNode;
        }
        return (typeof document !== 'undefined') ? document : a;
    }

    function _isDescendantOf(node, ancestor) {
        var cur = node ? node.parentNode : null;
        while (cur) {
            if (cur === ancestor || (cur._nodeId !== undefined && cur._nodeId === ancestor._nodeId)) return true;
            cur = cur.parentNode;
        }
        return false;
    }

    function _comparePoints(nodeA, offsetA, nodeB, offsetB) {
        if (nodeA === nodeB || (nodeA && nodeB && nodeA._nodeId !== undefined && nodeA._nodeId === nodeB._nodeId)) {
            return offsetA < offsetB ? -1 : (offsetA > offsetB ? 1 : 0);
        }
        if (_isDescendantOf(nodeB, nodeA)) {
            var child = nodeB;
            while (child.parentNode && child.parentNode !== nodeA && !(child.parentNode._nodeId !== undefined && child.parentNode._nodeId === nodeA._nodeId)) {
                child = child.parentNode;
            }
            var idx = _getNodeIndex(child);
            return idx < offsetA ? 1 : -1;
        }
        if (_isDescendantOf(nodeA, nodeB)) {
            var child = nodeA;
            while (child.parentNode && child.parentNode !== nodeB && !(child.parentNode._nodeId !== undefined && child.parentNode._nodeId === nodeB._nodeId)) {
                child = child.parentNode;
            }
            var idx = _getNodeIndex(child);
            return offsetB <= idx ? 1 : -1;
        }
        var common = _getCommonAncestor(nodeA, nodeB);
        if (!common) return 0;
        var childA = nodeA;
        while (childA.parentNode && childA.parentNode !== common && !(childA.parentNode._nodeId !== undefined && childA.parentNode._nodeId === common._nodeId)) {
            childA = childA.parentNode;
        }
        var childB = nodeB;
        while (childB.parentNode && childB.parentNode !== common && !(childB.parentNode._nodeId !== undefined && childB.parentNode._nodeId === common._nodeId)) {
            childB = childB.parentNode;
        }
        var idxA = _getNodeIndex(childA);
        var idxB = _getNodeIndex(childB);
        return idxA < idxB ? -1 : 1;
    }

    function Range() {
        this.startContainer = (typeof document !== 'undefined') ? (document.body || document) : null;
        this.startOffset = 0;
        this.endContainer = this.startContainer;
        this.endOffset = 0;
        this.collapsed = true;
        this.commonAncestorContainer = this.startContainer;
    }

    Range.START_TO_START = 0;
    Range.START_TO_END = 1;
    Range.END_TO_END = 2;
    Range.END_TO_START = 3;

    Range.prototype.START_TO_START = 0;
    Range.prototype.START_TO_END = 1;
    Range.prototype.END_TO_END = 2;
    Range.prototype.END_TO_START = 3;

    Range.prototype._updateState = function() {
        this.collapsed = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId)) && (this.startOffset === this.endOffset);
        this.commonAncestorContainer = _getCommonAncestor(this.startContainer, this.endContainer);
    };

    Range.prototype.setStart = function(node, offset) {
        if (!node) throw new Error("TypeError: Node must not be null");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        this.startContainer = node;
        this.startOffset = offset;
        if (!this.endContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.endContainer = node;
            this.endOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setEnd = function(node, offset) {
        if (!node) throw new Error("TypeError: Node must not be null");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        this.endContainer = node;
        this.endOffset = offset;
        if (!this.startContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.startContainer = node;
            this.startOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setStartBefore = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setStart(node.parentNode, _getNodeIndex(node));
    };

    Range.prototype.setStartAfter = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setStart(node.parentNode, _getNodeIndex(node) + 1);
    };

    Range.prototype.setEndBefore = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setEnd(node.parentNode, _getNodeIndex(node));
    };

    Range.prototype.setEndAfter = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setEnd(node.parentNode, _getNodeIndex(node) + 1);
    };

    Range.prototype.collapse = function(toStart) {
        if (toStart) {
            this.endContainer = this.startContainer;
            this.endOffset = this.startOffset;
        } else {
            this.startContainer = this.endContainer;
            this.startOffset = this.endOffset;
        }
        this._updateState();
    };

    Range.prototype.selectNode = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        var idx = _getNodeIndex(node);
        this.setStart(node.parentNode, idx);
        this.setEnd(node.parentNode, idx + 1);
    };

    Range.prototype.selectNodeContents = function(node) {
        if (!node) throw new Error("TypeError: Node must not be null");
        this.setStart(node, 0);
        this.setEnd(node, _getNodeLength(node));
    };

    Range.prototype.compareBoundaryPoints = function(how, sourceRange) {
        if (!sourceRange) throw new Error("TypeError: sourceRange must not be null");
        if (how === 0) {
            return _comparePoints(this.startContainer, this.startOffset, sourceRange.startContainer, sourceRange.startOffset);
        } else if (how === 1) {
            return _comparePoints(this.endContainer, this.endOffset, sourceRange.startContainer, sourceRange.startOffset);
        } else if (how === 2) {
            return _comparePoints(this.endContainer, this.endOffset, sourceRange.endContainer, sourceRange.endOffset);
        } else if (how === 3) {
            return _comparePoints(this.startContainer, this.startOffset, sourceRange.endContainer, sourceRange.endOffset);
        }
        throw new Error("NotSupportedError: Invalid comparison method");
    };

    Range.prototype.cloneRange = function() {
        var r = new Range();
        r.setStart(this.startContainer, this.startOffset);
        r.setEnd(this.endContainer, this.endOffset);
        return r;
    };

    Range.prototype.deleteContents = function() {
        if (this.collapsed) return;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = s.substring(0, this.startOffset) + s.substring(this.endOffset);
            } else if (this.startContainer.childNodes) {
                var kids = Array.prototype.slice.call(this.startContainer.childNodes);
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    this.startContainer.removeChild(kids[i]);
                }
            }
        } else {
            if (this.startContainer.nodeType === 3) {
                var s1 = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = s1.substring(0, this.startOffset);
            }
            if (this.endContainer.nodeType === 3) {
                var s2 = this.endContainer.nodeValue || '';
                this.endContainer.nodeValue = s2.substring(this.endOffset);
            }
        }
        this.collapse(true);
    };

    Range.prototype.cloneContents = function() {
        var frag = _wrapElement(_createFragment());
        if (this.collapsed) return frag;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s)));
            } else if (this.startContainer.childNodes) {
                var kids = this.startContainer.childNodes;
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    if (kids[i] && kids[i].cloneNode) frag.appendChild(kids[i].cloneNode(true));
                }
            }
        } else {
            if (this.startContainer.nodeType === 3) {
                var s1 = (this.startContainer.nodeValue || '').substring(this.startOffset);
                frag.appendChild(_wrapElement(_origCTN(s1)));
            }
            if (this.endContainer.nodeType === 3) {
                var s2 = (this.endContainer.nodeValue || '').substring(0, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s2)));
            }
        }
        return frag;
    };

    Range.prototype.extractContents = function() {
        var frag = _wrapElement(_createFragment());
        if (this.collapsed) return frag;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s)));
                var full = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = full.substring(0, this.startOffset) + full.substring(this.endOffset);
            } else if (this.startContainer.childNodes) {
                var kids = Array.prototype.slice.call(this.startContainer.childNodes);
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    if (kids[i]) frag.appendChild(kids[i]);
                }
            }
        } else {
            var fragClone = this.cloneContents();
            this.deleteContents();
            return fragClone;
        }
        this.collapse(true);
        return frag;
    };

    Range.prototype.insertNode = function(node) {
        if (!node) throw new Error("TypeError: Node must not be null");
        if (this.startContainer.nodeType === 3) {
            var full = this.startContainer.nodeValue || '';
            var before = full.substring(0, this.startOffset);
            var after = full.substring(this.startOffset);
            this.startContainer.nodeValue = before;
            var afterNode = _wrapElement(_origCTN(after));
            var p = this.startContainer.parentNode;
            if (p) {
                p.insertBefore(afterNode, this.startContainer.nextSibling);
                p.insertBefore(node, afterNode);
            }
        } else {
            var ref = (this.startContainer.childNodes && this.startContainer.childNodes[this.startOffset]) || null;
            this.startContainer.insertBefore(node, ref);
        }
    };

    Range.prototype.surroundContents = function(newParent) {
        if (!newParent) throw new Error("TypeError: newParent must not be null");
        var frag = this.extractContents();
        while (newParent.firstChild) newParent.removeChild(newParent.firstChild);
        this.insertNode(newParent);
        newParent.appendChild(frag);
        this.selectNode(newParent);
    };

    Range.prototype.createContextualFragment = function(html) {
        var frag = _wrapElement(_createFragment());
        frag.innerHTML = String(html);
        return frag;
    };

    Range.prototype.toString = function() {
        if (this.collapsed) return '';
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                return (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
            }
            return (this.startContainer.textContent || '');
        }
        if (this.startContainer.nodeType === 3 && this.endContainer.nodeType === 3) {
            var s1 = (this.startContainer.nodeValue || '').substring(this.startOffset);
            var s2 = (this.endContainer.nodeValue || '').substring(0, this.endOffset);
            return s1 + s2;
        }
        return (this.commonAncestorContainer && this.commonAncestorContainer.textContent) ? this.commonAncestorContainer.textContent : '';
    };

    Range.prototype.isPointInRange = function(node, offset) {
        return _comparePoints(node, offset, this.startContainer, this.startOffset) >= 0 && _comparePoints(node, offset, this.endContainer, this.endOffset) <= 0;
    };

    Range.prototype.comparePoint = function(node, offset) {
        if (_comparePoints(node, offset, this.startContainer, this.startOffset) < 0) return -1;
        if (_comparePoints(node, offset, this.endContainer, this.endOffset) > 0) return 1;
        return 0;
    };

    Range.prototype.intersectsNode = function(node) {
        if (!node) return false;
        var p = node.parentNode;
        if (!p) return false;
        var idx = _getNodeIndex(node);
        return _comparePoints(p, idx, this.endContainer, this.endOffset) < 0 && _comparePoints(p, idx + 1, this.startContainer, this.startOffset) > 0;
    };

    Range.prototype.getBoundingClientRect = function() {
        return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, toJSON: function() { return this; } };
    };

    Range.prototype.getClientRects = function() {
        return [this.getBoundingClientRect()];
    };

    Range.prototype.detach = function() {};

    function Selection() {
        this._range = null;
        this.anchorNode = null;
        this.anchorOffset = 0;
        this.focusNode = null;
        this.focusOffset = 0;
        this.isCollapsed = true;
        this.rangeCount = 0;
        this.type = 'None';
    }

    Selection.prototype._sync = function() {
        if (this._range) {
            this.anchorNode = this._range.startContainer;
            this.anchorOffset = this._range.startOffset;
            this.focusNode = this._range.endContainer;
            this.focusOffset = this._range.endOffset;
            this.isCollapsed = this._range.collapsed;
            this.rangeCount = 1;
            this.type = this._range.collapsed ? 'Caret' : 'Range';
        } else {
            this.anchorNode = null;
            this.anchorOffset = 0;
            this.focusNode = null;
            this.focusOffset = 0;
            this.isCollapsed = true;
            this.rangeCount = 0;
            this.type = 'None';
        }
    };

    Selection.prototype.getRangeAt = function(index) {
        if (Number(index) !== 0 || !this._range) {
            throw new Error("IndexSizeError: Selection does not contain a range at index " + index);
        }
        return this._range;
    };

    Selection.prototype.addRange = function(range) {
        if (!range) return;
        this._range = range;
        this._sync();
    };

    Selection.prototype.removeRange = function(range) {
        if (this._range === range) {
            this.removeAllRanges();
        }
    };

    Selection.prototype.removeAllRanges = function() {
        this._range = null;
        this._sync();
    };

    Selection.prototype.empty = function() {
        this.removeAllRanges();
    };

    Selection.prototype.collapse = function(node, offset) {
        var r = new Range();
        r.setStart(node, offset || 0);
        r.collapse(true);
        this.addRange(r);
    };

    Selection.prototype.collapseToStart = function() {
        if (this._range) {
            this._range.collapse(true);
            this._sync();
        }
    };

    Selection.prototype.collapseToEnd = function() {
        if (this._range) {
            this._range.collapse(false);
            this._sync();
        }
    };

    Selection.prototype.extend = function(node, offset) {
        if (this._range) {
            this._range.setEnd(node, offset || 0);
            this._sync();
        }
    };

    Selection.prototype.setBaseAndExtent = function(anchorNode, anchorOffset, focusNode, focusOffset) {
        var r = new Range();
        r.setStart(anchorNode, anchorOffset || 0);
        r.setEnd(focusNode, focusOffset || 0);
        this.addRange(r);
    };

    Selection.prototype.selectAllChildren = function(node) {
        var r = new Range();
        r.selectNodeContents(node);
        this.addRange(r);
    };

    Selection.prototype.deleteFromDocument = function() {
        if (this._range) {
            this._range.deleteContents();
            this._sync();
        }
    };

    Selection.prototype.containsNode = function(node, partlyContained) {
        if (!this._range) return false;
        return this._range.intersectsNode(node);
    };

    Selection.prototype.toString = function() {
        return this._range ? this._range.toString() : '';
    };

    globalThis.Range = Range;
    globalThis.Selection = Selection;

    document.createRange = function() {
        return new Range();
    };
    document.getSelection = function() {
        if (!document._selection) {
            document._selection = new Selection();
        }
        return document._selection;
    };
    if (typeof window !== 'undefined') {
        window.getSelection = function() {
            return document.getSelection();
        };
    }
    document.createComment = function(text) {
        return _wrapElement(_createComment(String(text === undefined ? '' : text)));
    };
    document.createEvent = function(type) {
        var evt = (typeof globalThis.Event !== 'undefined') ? new globalThis.Event(type || "") : { type: type || "" };
        evt.initEvent = function(t, b, c) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
        };
        evt.initCustomEvent = function(t, b, c, detail) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
            this.detail = detail;
        };
        evt.initMouseEvent = function(t, b, c) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
        };
        return evt;
    };
    document.createTreeWalker = function(root, whatToShow, filter) {
        return {
            root: root,
            currentNode: root,
            whatToShow: whatToShow || -1,
            filter: filter,
            nextNode: function() { return null; },
            previousNode: function() { return null; },
            parentNode: function() { return null; },
            firstChild: function() { return null; },
            lastChild: function() { return null; },
            nextSibling: function() { return null; },
            previousSibling: function() { return null; }
        };
    };
    document.createNodeIterator = function(root, whatToShow, filter) {
        return {
            root: root,
            whatToShow: whatToShow || -1,
            filter: filter,
            nextNode: function() { return null; },
            previousNode: function() { return null; },
            detach: function() {}
        };
    };
    document.importNode = function(node, deep) {
        return (node && node.cloneNode) ? node.cloneNode(deep) : node;
    };
    document.adoptNode = function(node) { return node; };
    document.defaultView = globalThis;
    document.hasFocus = function() { return true; };
    document.implementation = {
        createHTMLDocument: function(title) {
            return {
                title: title || '',
                createElement: function(t) { return _wrapElement(_origCE(t)); },
                createElementNS: function(ns, t) { return _wrapElement(_origCE(t)); },
                createTextNode: function(t) { return _wrapElement(_origCTN(t)); },
                createDocumentFragment: function() { return _wrapElement(_createFragment()); },
                querySelector: function() { return null; },
                querySelectorAll: function() { return _createNodeList(function() { return []; }); },
                getElementById: function() { return null; },
                getElementsByTagName: function() { return _createHTMLCollection(function() { return []; }); },
                body: _wrapElement(_origCE('body')),
                head: _wrapElement(_origCE('head'))
            };
        },
        hasFeature: function() { return true; },
        createDocument: function() { return document; }
    };

    Object.defineProperty(document, 'body', {
        get: function() { return document.querySelector('body'); }
    });
    Object.defineProperty(document, 'head', {
        get: function() { return document.querySelector('head'); }
    });
    Object.defineProperty(document, 'documentElement', {
        get: function() { return document.querySelector('html') || document.body; }
    });
    Object.defineProperty(document, 'title', {
        get: function() {
            var t = document.querySelector('title');
            return t ? t.textContent : "";
        },
        set: function(val) {
            var t = document.querySelector('title');
            if (t) t.textContent = val;
        }
    });
    Object.defineProperty(document, 'currentScript', {
        get: function() {
            var s = document.getElementsByTagName('script');
            return s && s.length > 0 ? s[s.length - 1] : null;
        },
        configurable: true
    });
    document.fonts = {
        ready: Promise.resolve(),
        check: function() { return true; },
        load: function() { return Promise.resolve([]); },
        addEventListener: function() {},
        removeEventListener: function() {},
        dispatchEvent: function() { return true; }
    };

    document._nodeId = 0;
    document.nodeType = 9;
    document.nodeName = '#document';
    if (typeof globalThis !== 'undefined' && globalThis.Document) {
        try { Object.setPrototypeOf(document, globalThis.Document.prototype); } catch(e) {}
    }
    globalThis._addDOMEventListener = _addDOMEventListener;
    globalThis._removeDOMEventListener = _removeDOMEventListener;
    globalThis._dispatchDOMEvent = _dispatchDOMEvent;
    globalThis._dispatchInternalEvent = function(nodeId, type, dict) {
        var target;
        if (nodeId === -1 || nodeId === 'window') {
            target = (typeof globalThis !== 'undefined' && globalThis.window) ? globalThis.window : document;
        } else if (nodeId === null || nodeId === undefined || nodeId === 0 || nodeId === 'document') {
            target = document;
        } else {
            target = _wrapElement(nodeId);
        }
        if (!target) target = document;
        dict = dict || {};
        var ctor = (typeof globalThis.Event === 'function') ? globalThis.Event : function(t, d) {
            this.type = t;
            this.bubbles = !!(d && d.bubbles);
            this.cancelable = !!(d && d.cancelable);
        };
        if (dict.eventType === 'mouse' && typeof globalThis.MouseEvent === 'function') ctor = globalThis.MouseEvent;
        else if (dict.eventType === 'keyboard' && typeof globalThis.KeyboardEvent === 'function') ctor = globalThis.KeyboardEvent;
        else if (dict.eventType === 'focus' && typeof globalThis.FocusEvent === 'function') ctor = globalThis.FocusEvent;
        else if (dict.eventType === 'input' && typeof globalThis.InputEvent === 'function') ctor = globalThis.InputEvent;
        else if (dict.eventType === 'custom' && typeof globalThis.CustomEvent === 'function') ctor = globalThis.CustomEvent;
        else if (dict.eventType === 'wheel' && typeof globalThis.WheelEvent === 'function') ctor = globalThis.WheelEvent;
        else if (dict.eventType === 'touch' && typeof globalThis.TouchEvent === 'function') ctor = globalThis.TouchEvent;
        else if (dict.eventType === 'ui' && typeof globalThis.UIEvent === 'function') ctor = globalThis.UIEvent;
        else if (dict.eventType === 'submit' && typeof globalThis.SubmitEvent === 'function') ctor = globalThis.SubmitEvent;
        var evt = new ctor(type, dict);
        return _dispatchDOMEvent(target, evt);
    };
    "#;

    if let Err(e) = context.eval(boa_engine::Source::from_bytes(shim)) {
        log::error!("Failed to install DOM shim: {}", e);
        #[cfg(test)]
        panic!("Failed to install DOM shim: {}", e);
    }
}

/// Deep- or shallow-clones a node (and its attributes) into the same document.
fn clone_subtree(doc: &mut Document, node_id: NodeId, deep: bool) -> Option<NodeId> {
    let data = doc.get(node_id)?.data.clone();
    let new_id = match data {
        NodeData::Element(el) => doc.create_element(&el.tag_name, el.attributes.clone()),
        NodeData::Text(t) => doc.create_text(&t),
        NodeData::Comment(c) => doc.create_comment(&c),
        NodeData::DocumentFragment => doc.create_fragment(),
        NodeData::DocumentType { name, public_id, system_id } => {
            doc.create_doctype(&name, &public_id, &system_id)
        }
        NodeData::Document => return None,
    };
    if deep {
        let children: Vec<NodeId> = doc.children(node_id).map(|n| n.id).collect();
        for child in children {
            if let Some(cloned) = clone_subtree(doc, child, true) {
                doc.append_child(new_id, cloned);
            }
        }
        if let Some(NodeData::Element(orig_el)) = doc.get(node_id).map(|n| &n.data) {
            if let Some(orig_frag) = orig_el.template_contents {
                if let Some(new_frag) = doc.template_contents(new_id) {
                    let frag_children: Vec<NodeId> = doc.children(orig_frag).map(|n| n.id).collect();
                    for fchild in frag_children {
                        if let Some(cloned) = clone_subtree(doc, fchild, true) {
                            doc.append_child(new_frag, cloned);
                        }
                    }
                }
            }
        }
    }
    Some(new_id)
}

/// Copies all element/text/comment children of `source` into `target` under `parent`.
///
/// Used by `innerHTML`/`outerHTML` setters: the fragment is parsed into a temporary
/// `Document` and then adopted into the live tree.
fn adopt_children(source: &Document, source_id: NodeId, target: &mut Document, parent: NodeId) {
    for child in source.children(source_id) {
        match &child.data {
            NodeData::Element(el) => {
                let new_id = target.create_element(&el.tag_name, el.attributes.clone());
                target.append_child(parent, new_id);
                adopt_children(source, child.id, target, new_id);
            }
            NodeData::Text(t) => {
                let new_id = target.create_text(t);
                target.append_child(parent, new_id);
            }
            NodeData::Comment(c) => {
                let new_id = target.create_comment(c);
                target.append_child(parent, new_id);
            }
            _ => {}
        }
    }
}

/// Copies all element/text/comment children of `source` into `target` under `parent`,
/// inserting before `before_node`.
fn adopt_children_before(
    source: &Document,
    source_id: NodeId,
    target: &mut Document,
    parent: NodeId,
    before_node: NodeId,
) {
    for child in source.children(source_id) {
        match &child.data {
            NodeData::Element(el) => {
                let new_id = target.create_element(&el.tag_name, el.attributes.clone());
                target.insert_before(parent, new_id, before_node);
                adopt_children(source, child.id, target, new_id);
            }
            NodeData::Text(t) => {
                let new_id = target.create_text(t);
                target.insert_before(parent, new_id, before_node);
            }
            NodeData::Comment(c) => {
                let new_id = target.create_comment(c);
                target.insert_before(parent, new_id, before_node);
            }
            _ => {}
        }
    }
}

/// Converts a `dataset` property name to its `data-*` attribute name
/// (`myValue` → `data-my-value`, per the HTML spec).
fn dataset_prop_to_attribute(prop: &str) -> String {
    let mut out = String::from("data-");
    for ch in prop.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Converts a `data-*` suffix back to a camelCase `dataset` key (`my-value` → `myValue`).
fn camel_case_key(suffix: &str) -> String {
    let mut out = String::with_capacity(suffix.len());
    let mut upper_next = false;
    for ch in suffix.chars() {
        if ch == '-' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Sets (or replaces) an attribute on an element payload.
fn set_element_attribute(el: &mut mango_html::dom::ElementData, name: &str, value: &str) {
    if let Some(entry) = el
        .attributes
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
    {
        entry.1 = value.to_string();
    } else {
        el.attributes.push((name.to_string(), value.to_string()));
    }
}

/// Collects document-level collections (`document.forms`, `document.images`, …).
fn collect_elements_by_kind(doc: &Document, node_id: NodeId, kind: &str, out: &mut Vec<NodeId>) {
    for child in doc.children(node_id) {
        if let NodeData::Element(el) = &child.data {
            let tag = el.tag_name.to_ascii_lowercase();
            let matches = match kind {
                "forms" => tag == "form",
                "images" => tag == "img",
                "links" => (tag == "a" || tag == "area") && el.get_attribute("href").is_some(),
                "anchors" => tag == "a" && el.get_attribute("name").is_some(),
                "scripts" => tag == "script",
                "embeds" => tag == "embed" || tag == "object",
                "all" => true,
                _ => false,
            };
            if matches {
                out.push(child.id);
            }
        }
        collect_elements_by_kind(doc, child.id, kind, out);
    }
}

/// Recursively finds an element by its `id` attribute.
fn find_element_by_id_recursive(doc: &Document, node_id: NodeId, target_id: &str) -> Option<NodeId> {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
            && elem.id() == Some(target_id) {
                return Some(node_id);
            }
    for child in doc.children(node_id) {
        if let Some(found) = find_element_by_id_recursive(doc, child.id, target_id) {
            return Some(found);
        }
    }
    None
}

/// Simple CSS selector matching: supports `#id`, `.class`, and `tagname`.
fn query_selector_simple(doc: &Document, node_id: NodeId, selector: &str) -> Option<NodeId> {
    let selector = selector.trim();
    if let Some(id) = selector.strip_prefix('#') {
        return find_element_by_id_recursive(doc, node_id, id);
    }
    if let Some(class) = selector.strip_prefix('.') {
        return find_element_by_class_recursive(doc, node_id, class);
    }
    doc.find_element_by_tag(node_id, selector)
}

fn find_element_by_class_recursive(doc: &Document, node_id: NodeId, target_class: &str) -> Option<NodeId> {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
            && elem.has_class(target_class) {
                return Some(node_id);
            }
    for child in doc.children(node_id) {
        if let Some(found) = find_element_by_class_recursive(doc, child.id, target_class) {
            return Some(found);
        }
    }
    None
}

fn find_first_matching_element(
    doc: &Document,
    current: NodeId,
    selectors: &mango_css::SelectorList,
) -> Option<NodeId> {
    for child in doc.children(current) {
        if matches!(child.data, NodeData::Element(_)) {
            if selectors.matches(child.id, doc) {
                return Some(child.id);
            }
            if let Some(found) = find_first_matching_element(doc, child.id, selectors) {
                return Some(found);
            }
        }
    }
    None
}

fn collect_matching_elements(
    doc: &Document,
    current: NodeId,
    selectors: &mango_css::SelectorList,
    out: &mut Vec<NodeId>,
) {
    for child in doc.children(current) {
        if matches!(child.data, NodeData::Element(_)) {
            if selectors.matches(child.id, doc) {
                out.push(child.id);
            }
            collect_matching_elements(doc, child.id, selectors, out);
        }
    }
}

