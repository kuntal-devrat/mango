//! DOM tree traversal, iteration, querying, and HTML serialization.

use super::document::Document;
use super::node::{Node, NodeData, NodeId};

/// Escapes text for safe insertion into serialized HTML.
fn escape_html_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escapes an attribute value for safe insertion into serialized HTML.
fn escape_html_attribute(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

impl Document {
    /// Returns an iterator over the immediate children of `parent`.
    pub fn children(&self, parent: NodeId) -> ChildrenIter<'_> {
        let first = self.arena.get(parent).and_then(|n| n.first_child);
        ChildrenIter {
            doc: self,
            current: first,
        }
    }

    /// Serializes a node's subtree back to HTML (used by `innerHTML`/`outerHTML`).
    pub fn serialize_html(&self, node_id: NodeId) -> String {
        let mut out = String::new();
        self.serialize_node(node_id, &mut out, 0);
        out
    }

    /// Serializes only the children of `node_id` (used by `innerHTML`).
    pub fn serialize_children(&self, node_id: NodeId) -> String {
        let mut out = String::new();
        if let Some(node) = self.arena.get(node_id)
            && let NodeData::Element(elem) = &node.data
            && elem.tag_name.eq_ignore_ascii_case("template")
            && let Some(frag_id) = elem.template_contents
        {
            for child in self.children(frag_id) {
                self.serialize_node(child.id, &mut out, 0);
            }
            return out;
        }
        for child in self.children(node_id) {
            self.serialize_node(child.id, &mut out, 0);
        }
        out
    }

    fn serialize_node(&self, node_id: NodeId, out: &mut String, depth: usize) {
        if depth > 512 {
            return;
        }
        let Some(node) = self.arena.get(node_id) else {
            return;
        };
        match &node.data {
            NodeData::Element(elem) => {
                out.push('<');
                out.push_str(&elem.tag_name);
                for (name, value) in &elem.attributes {
                    out.push(' ');
                    out.push_str(name);
                    out.push_str("=\"");
                    out.push_str(&escape_html_attribute(value));
                    out.push('"');
                }
                out.push('>');
                if !crate::elements::is_void_element(&elem.tag_name) {
                    if elem.tag_name.eq_ignore_ascii_case("template")
                        && elem.template_contents.is_some()
                    {
                        if let Some(frag_id) = elem.template_contents {
                            for child in self.children(frag_id) {
                                self.serialize_node(child.id, out, depth + 1);
                            }
                        }
                    } else {
                        for child in self.children(node_id) {
                            self.serialize_node(child.id, out, depth + 1);
                        }
                    }
                    out.push_str("</");
                    out.push_str(&elem.tag_name);
                    out.push('>');
                }
            }
            NodeData::Text(text) => {
                let is_raw_text = node
                    .parent
                    .and_then(|p| self.arena.get(p))
                    .map(|pn| match &pn.data {
                        NodeData::Element(el) => matches!(
                            el.tag_name.to_ascii_lowercase().as_str(),
                            "script"
                                | "style"
                                | "xmp"
                                | "iframe"
                                | "noembed"
                                | "noframes"
                                | "plaintext"
                        ),
                        _ => false,
                    })
                    .unwrap_or(false);
                if is_raw_text {
                    out.push_str(text);
                } else {
                    out.push_str(&escape_html_text(text));
                }
            }
            NodeData::Comment(comment) => {
                out.push_str("<!--");
                out.push_str(comment);
                out.push_str("-->");
            }
            NodeData::DocumentType {
                name,
                public_id,
                system_id,
            } => {
                out.push_str("<!DOCTYPE ");
                out.push_str(name);
                if !public_id.is_empty() {
                    out.push_str(" PUBLIC \"");
                    out.push_str(public_id);
                    out.push('"');
                    if !system_id.is_empty() {
                        out.push_str(" \"");
                        out.push_str(system_id);
                        out.push('"');
                    }
                } else if !system_id.is_empty() {
                    out.push_str(" SYSTEM \"");
                    out.push_str(system_id);
                    out.push('"');
                }
                out.push('>');
            }
            NodeData::Document | NodeData::DocumentFragment => {
                for child in self.children(node_id) {
                    self.serialize_node(child.id, out, depth + 1);
                }
            }
        }
    }

    /// Recursively gathers all text content under a given node.
    pub fn text_content(&self, node_id: NodeId) -> String {
        let mut text = String::new();
        self.collect_text(node_id, &mut text);
        text
    }

    fn collect_text(&self, root_id: NodeId, out: &mut String) {
        let mut stack = vec![root_id];
        while let Some(node_id) = stack.pop() {
            if let Some(node) = self.arena.get(node_id) {
                match &node.data {
                    NodeData::Text(t) => out.push_str(t),
                    NodeData::Element(el) if el.tag_name.eq_ignore_ascii_case("template") => {
                        // WHATWG HTML5 §4.12.3: textContent of template is empty
                    }
                    _ => {
                        let mut kids: Vec<NodeId> = self.children(node_id).map(|c| c.id).collect();
                        kids.reverse();
                        stack.extend(kids);
                    }
                }
            }
        }
    }

    /// Finds the first descendant element matching the tag name (case-insensitive).
    pub fn find_element_by_tag(&self, root: NodeId, tag: &str) -> Option<NodeId> {
        let mut stack = vec![root];
        while let Some(curr) = stack.pop() {
            let mut kids: Vec<NodeId> = Vec::new();
            for child in self.children(curr) {
                if let NodeData::Element(elem) = &child.data
                    && elem.tag_name.eq_ignore_ascii_case(tag)
                {
                    return Some(child.id);
                }
                kids.push(child.id);
            }
            kids.reverse();
            stack.extend(kids);
        }
        None
    }

    /// Finds the first descendant element matching the `id` attribute.
    pub fn find_element_by_id(&self, root: NodeId, id: &str) -> Option<NodeId> {
        let mut stack = vec![root];
        while let Some(curr) = stack.pop() {
            let mut kids: Vec<NodeId> = Vec::new();
            for child in self.children(curr) {
                if let NodeData::Element(elem) = &child.data
                    && elem.id() == Some(id)
                {
                    return Some(child.id);
                }
                kids.push(child.id);
            }
            kids.reverse();
            stack.extend(kids);
        }
        None
    }

    /// Finds the closest ancestor of `start` that satisfies the given predicate.
    pub fn find_ancestor<F>(&self, start: NodeId, predicate: F) -> Option<NodeId>
    where
        F: Fn(&Node) -> bool,
    {
        let mut curr = self.get(start).and_then(|n| n.parent);
        while let Some(nid) = curr {
            if let Some(node) = self.get(nid) {
                if predicate(node) {
                    return Some(nid);
                }
                curr = node.parent;
            } else {
                break;
            }
        }
        None
    }

    /// Returns the `<details>` ancestor element for a given `<summary>` node, if any.
    pub fn find_details_for_summary(&self, summary_id: NodeId) -> Option<NodeId> {
        self.find_ancestor(summary_id, |node| {
            if let NodeData::Element(el) = &node.data {
                el.is_details()
            } else {
                false
            }
        })
    }

    /// Generates a visual debug tree representation of the DOM.
    pub fn dump_tree(&self, root: NodeId) -> String {
        let mut out = String::new();
        self.dump_node(root, 0, &mut out);
        out
    }

    fn dump_node(&self, node_id: NodeId, indent: usize, out: &mut String) {
        let Some(node) = self.arena.get(node_id) else {
            return;
        };
        let pad = "  ".repeat(indent);
        match &node.data {
            NodeData::Document => out.push_str(&format!("{pad}#document\n")),
            NodeData::DocumentFragment => out.push_str(&format!("{pad}#document-fragment\n")),
            NodeData::DocumentType { name, .. } => {
                out.push_str(&format!("{pad}<!DOCTYPE {name}>\n"))
            }
            NodeData::Element(elem) => {
                let attrs = if elem.attributes.is_empty() {
                    String::new()
                } else {
                    let formatted = elem
                        .attributes
                        .iter()
                        .map(|(k, v)| format!("{k}=\"{v}\""))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!(" {formatted}")
                };
                out.push_str(&format!("{pad}<{}{attrs}>\n", elem.tag_name));
            }
            NodeData::Text(t) => {
                let clean = t.trim();
                if !clean.is_empty() {
                    out.push_str(&format!("{pad}\"{clean}\"\n"));
                }
            }
            NodeData::Comment(c) => out.push_str(&format!("{pad}<!-- {c} -->\n")),
        }

        for child in self.children(node_id) {
            self.dump_node(child.id, indent + 1, out);
        }
    }
}

/// Iterator over child nodes of a given parent.
pub struct ChildrenIter<'a> {
    doc: &'a Document,
    current: Option<NodeId>,
}

impl<'a> Iterator for ChildrenIter<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        let current_id = self.current?;
        let node = self.doc.get(current_id)?;
        // Snapshot next_sibling before yielding so mutation of the current
        // node (e.g. detach clearing its links) cannot corrupt iteration.
        let next = node.next_sibling;
        self.current = next;
        Some(node)
    }
}
