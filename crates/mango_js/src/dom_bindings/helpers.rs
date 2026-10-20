use mango_html::dom::{Document, NodeData, NodeId};

/// Deep- or shallow-clones a node (and its attributes) into the same document.
pub(crate) fn clone_subtree(doc: &mut Document, node_id: NodeId, deep: bool) -> Option<NodeId> {
    let data = doc.get(node_id)?.data.clone();
    let new_id = match data {
        NodeData::Element(el) => doc.create_element(&el.tag_name, el.attributes.clone()),
        NodeData::Text(t) => doc.create_text(&t),
        NodeData::Comment(c) => doc.create_comment(&c),
        NodeData::DocumentFragment => doc.create_fragment(),
        NodeData::DocumentType {
            name,
            public_id,
            system_id,
        } => doc.create_doctype(&name, &public_id, &system_id),
        NodeData::Document => return None,
    };
    if deep {
        let children: Vec<NodeId> = doc.children(node_id).map(|n| n.id).collect();
        for child in children {
            if let Some(cloned) = clone_subtree(doc, child, true) {
                doc.append_child(new_id, cloned);
            }
        }
        if let Some(NodeData::Element(orig_el)) = doc.get(node_id).map(|n| &n.data)
            && let Some(orig_frag) = orig_el.template_contents
            && let Some(new_frag) = doc.template_contents(new_id)
        {
            let frag_children: Vec<NodeId> = doc.children(orig_frag).map(|n| n.id).collect();
            for fchild in frag_children {
                if let Some(cloned) = clone_subtree(doc, fchild, true) {
                    doc.append_child(new_frag, cloned);
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
pub(crate) fn adopt_children(
    source: &Document,
    source_id: NodeId,
    target: &mut Document,
    parent: NodeId,
) {
    for child in source.children(source_id) {
        match &child.data {
            NodeData::Element(el) => {
                let new_id = target.create_element(&el.tag_name, el.attributes.clone());
                target.append_child(parent, new_id);
                if el.tag_name.eq_ignore_ascii_case("template") {
                    if let Some(src_frag) = source.template_contents(child.id) {
                        let dst_frag = target.get_or_create_template_contents(new_id);
                        adopt_children(source, src_frag, target, dst_frag);
                    }
                }
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
pub(crate) fn adopt_children_before(
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
                let _ = target.insert_before(parent, new_id, before_node);
                if el.tag_name.eq_ignore_ascii_case("template") {
                    if let Some(src_frag) = source.template_contents(child.id) {
                        let dst_frag = target.get_or_create_template_contents(new_id);
                        adopt_children(source, src_frag, target, dst_frag);
                    }
                }
                adopt_children(source, child.id, target, new_id);
            }
            NodeData::Text(t) => {
                let new_id = target.create_text(t);
                let _ = target.insert_before(parent, new_id, before_node);
            }
            NodeData::Comment(c) => {
                let new_id = target.create_comment(c);
                let _ = target.insert_before(parent, new_id, before_node);
            }
            _ => {}
        }
    }
}

/// Collects document-level collections (`document.forms`, `document.images`, …).
pub(crate) fn collect_elements_by_kind(
    doc: &Document,
    node_id: NodeId,
    kind: &str,
    out: &mut Vec<NodeId>,
) {
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
pub(crate) fn find_element_by_id_recursive(
    doc: &Document,
    node_id: NodeId,
    target_id: &str,
) -> Option<NodeId> {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
        && elem.id() == Some(target_id)
    {
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
pub(crate) fn query_selector_simple(
    doc: &Document,
    node_id: NodeId,
    selector: &str,
) -> Option<NodeId> {
    let selector = selector.trim();
    if let Some(id) = selector.strip_prefix('#') {
        return find_element_by_id_recursive(doc, node_id, id);
    }
    if let Some(class) = selector.strip_prefix('.') {
        return find_element_by_class_recursive(doc, node_id, class);
    }
    doc.find_element_by_tag(node_id, selector)
}

pub(crate) fn find_element_by_class_recursive(
    doc: &Document,
    node_id: NodeId,
    target_class: &str,
) -> Option<NodeId> {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
        && elem.has_class(target_class)
    {
        return Some(node_id);
    }
    for child in doc.children(node_id) {
        if let Some(found) = find_element_by_class_recursive(doc, child.id, target_class) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn find_first_matching_element(
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

pub(crate) fn collect_matching_elements(
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
