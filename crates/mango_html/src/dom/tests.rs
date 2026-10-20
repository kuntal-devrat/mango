use super::*;

#[test]
fn test_dom_tree_construction() {
    let mut doc = Document::new();
    let root = doc.root();

    let html = doc.create_element("html", vec![]);
    doc.append_child(root, html);

    let body = doc.create_element("body", vec![("class".into(), "main-page".into())]);
    doc.append_child(html, body);

    let h1 = doc.create_element("h1", vec![("id".into(), "title".into())]);
    let text = doc.create_text("Welcome to Mango Browser!");
    doc.append_child(h1, text);
    doc.append_child(body, h1);

    assert_eq!(doc.text_content(body), "Welcome to Mango Browser!");

    let found_h1 = doc.find_element_by_tag(root, "h1");
    assert!(found_h1.is_some());
    let h1_node = doc.get(found_h1.unwrap()).unwrap();
    if let NodeData::Element(el) = &h1_node.data {
        assert_eq!(el.id(), Some("title"));
    } else {
        panic!("expected element");
    }
}

#[test]
fn test_insert_before_and_detach() {
    let mut doc = Document::new();
    let root = doc.root();

    let a = doc.create_text("A");
    let c = doc.create_text("C");
    doc.append_child(root, a);
    doc.append_child(root, c);

    let b = doc.create_text("B");
    let _ = doc.insert_before(root, b, c);

    let texts: Vec<String> = doc
        .children(root)
        .filter_map(|n| match &n.data {
            NodeData::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["A", "B", "C"]);

    doc.detach(b);
    let texts_after: Vec<String> = doc
        .children(root)
        .filter_map(|n| match &n.data {
            NodeData::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts_after, vec!["A", "C"]);
}

#[test]
fn test_dom_element_string_interning() {
    let mut doc = Document::new();
    let e1 = doc.create_element("div", vec![("class".to_string(), "foo".to_string())]);
    let e2 = doc.create_element("div", vec![("class".to_string(), "bar".to_string())]);
    let e3 = doc.create_element("span", vec![("id".to_string(), "baz".to_string())]);

    let node1 = doc.get(e1).unwrap();
    let node2 = doc.get(e2).unwrap();
    let node3 = doc.get(e3).unwrap();

    let atom1 = match &node1.data {
        NodeData::Element(el) => el.tag_atom.unwrap(),
        _ => unreachable!(),
    };
    let atom2 = match &node2.data {
        NodeData::Element(el) => el.tag_atom.unwrap(),
        _ => unreachable!(),
    };
    let atom3 = match &node3.data {
        NodeData::Element(el) => el.tag_atom.unwrap(),
        _ => unreachable!(),
    };

    // Identical tags share the exact same interned handle
    assert_eq!(atom1, atom2);
    assert_ne!(atom1, atom3);
    assert_eq!(doc.resolve_atom(atom1), "div");
    assert_eq!(doc.resolve_atom(atom3), "span");
}

#[test]
fn test_insert_before_document_fragment() {
    let mut doc = Document::new();
    let root = doc.root();
    let a = doc.create_text("A");
    let d = doc.create_text("D");
    doc.append_child(root, a);
    doc.append_child(root, d);

    let frag = doc.create_fragment();
    let b = doc.create_text("B");
    let c = doc.create_text("C");
    doc.append_child(frag, b);
    doc.append_child(frag, c);

    // Inserting fragment before D moves B and C in front of D
    let _ = doc.insert_before(root, frag, d);

    let texts: Vec<String> = doc
        .children(root)
        .filter_map(|n| match &n.data {
            NodeData::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["A", "B", "C", "D"]);
    assert_eq!(doc.children(frag).count(), 0);
}

#[test]
fn test_replace_child() {
    let mut doc = Document::new();
    let root = doc.root();
    let a = doc.create_text("A");
    let b = doc.create_text("B");
    let c = doc.create_text("C");
    doc.append_child(root, a);
    doc.append_child(root, b);
    doc.append_child(root, c);

    let new_node = doc.create_text("REPLACED");
    let old = doc
        .replace_child(root, new_node, b)
        .expect("replace_child should succeed");
    assert_eq!(old, b);

    let texts: Vec<String> = doc
        .children(root)
        .filter_map(|n| match &n.data {
            NodeData::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["A", "REPLACED", "C"]);

    // Replacing with fragment
    let frag = doc.create_fragment();
    let f1 = doc.create_text("F1");
    let f2 = doc.create_text("F2");
    doc.append_child(frag, f1);
    doc.append_child(frag, f2);

    doc.replace_child(root, frag, new_node)
        .expect("replace with fragment should succeed");
    let texts_frag: Vec<String> = doc
        .children(root)
        .filter_map(|n| match &n.data {
            NodeData::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts_frag, vec!["A", "F1", "F2", "C"]);
}

#[test]
fn test_replace_child_does_not_detach_old_on_hierarchy_error() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("div", Vec::new());
    doc.append_child(root, parent);
    let child = doc.create_text("target_child");
    doc.append_child(parent, child);

    // Attempt to replace `child` with `parent` itself (cycle / invalid hierarchy)
    let res = doc.replace_child(parent, parent, child);
    assert!(
        res.is_err(),
        "Replacing child with its own parent should fail with hierarchy error"
    );

    // Verify `child` is NOT detached and still resides under `parent`
    let children: Vec<NodeId> = doc.children(parent).map(|n| n.id).collect();
    assert_eq!(
        children,
        vec![child],
        "Child must remain intact under parent on hierarchy error"
    );
}

#[test]
fn test_serialize_template_inner_html() {
    let mut doc = Document::new();
    let t = doc.create_element("template", Vec::new());
    let frag = doc.create_fragment();
    let span = doc.create_element("span", Vec::new());
    let text = doc.create_text("inside template");
    doc.append_child(span, text);
    doc.append_child(frag, span);
    if let Some(node) = doc.get_mut(t)
        && let NodeData::Element(elem) = &mut node.data
    {
        elem.template_contents = Some(frag);
    }
    let inner = doc.serialize_children(t);
    assert_eq!(inner, "<span>inside template</span>");
}

#[test]
fn test_serialize_doctype_identifiers() {
    let mut doc = Document::new();
    let dt = doc.create_doctype(
        "html",
        "-//W3C//DTD HTML 4.01//EN",
        "http://www.w3.org/TR/html4/strict.dtd",
    );
    let html = doc.serialize_html(dt);
    assert_eq!(
        html,
        "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">"
    );
}

#[test]
fn test_remove_child_and_recycle_subtree() {
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div", Vec::new());
    let p = doc.create_element("p", Vec::new());
    let text = doc.create_text("child text");
    doc.append_child(p, text);
    doc.append_child(div, p);
    doc.append_child(root, div);

    let initial_total = doc.arena.total_slots();
    assert_eq!(doc.arena.free_slots(), 0);

    // Remove div and its subtree (div, p, text = 3 nodes)
    doc.remove_child_and_recycle(div);
    assert_eq!(doc.arena.free_slots(), 3);

    // Next 3 allocations must reuse the 3 recycled slots without expanding arena total_slots
    let n1 = doc.create_element("span", Vec::new());
    let n2 = doc.create_element("span", Vec::new());
    let n3 = doc.create_element("span", Vec::new());
    doc.append_child(root, n1);
    doc.append_child(root, n2);
    doc.append_child(root, n3);

    assert_eq!(doc.arena.free_slots(), 0);
    assert_eq!(doc.arena.total_slots(), initial_total);
}
