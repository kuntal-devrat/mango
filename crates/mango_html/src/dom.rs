//! DOM tree data structures and arena-allocated node graph.
//!
//! Every node is owned by an [`Arena<Node>`] inside the [`Document`]. Nodes
//! reference their parent, siblings, and children via lightweight [`NodeId`] handles.

use std::collections::HashSet;
use mango_core::{Arena, Id, InternedString, StringInterner};

/// Handle to a DOM node inside a [`Document`]'s arena.
pub type NodeId = Id<Node>;

/// A DOM node in the tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub data: NodeData,
}

/// The specific payload of a DOM node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeData {
    Document,
    DocumentType {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element(ElementData),
    Text(String),
    Comment(String),
    /// A `DocumentFragment` created by `document.createDocumentFragment()`.
    ///
    /// Fragments are never rendered: inserting one into the tree moves its children
    /// instead (see `Document::append_child` / `insert_before`).
    DocumentFragment,
}

/// Payload for an element node (tag name, attributes).
#[derive(Debug, Clone, PartialEq)]
pub struct ElementData {
    pub tag_name: String,
    pub tag_atom: Option<InternedString>,
    pub attributes: Vec<(String, String)>,
    pub shadow_root: Option<NodeId>,
    pub template_contents: Option<NodeId>,
}

impl ElementData {
    pub fn new(tag_name: impl Into<String>, attributes: Vec<(String, String)>) -> Self {
        Self {
            tag_name: tag_name.into(),
            tag_atom: None,
            attributes,
            shadow_root: None,
            template_contents: None,
        }
    }

    /// Creates a new `ElementData` with an interned tag atom (OPT-002).
    pub fn with_atom(
        tag_name: impl Into<String>,
        tag_atom: InternedString,
        attributes: Vec<(String, String)>,
    ) -> Self {
        Self {
            tag_name: tag_name.into(),
            tag_atom: Some(tag_atom),
            attributes,
            shadow_root: None,
            template_contents: None,
        }
    }

    /// Looks up an attribute value by name (case-insensitive for HTML attributes).
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Returns the element's `id` attribute, if present.
    pub fn id(&self) -> Option<&str> {
        self.get_attribute("id")
    }

    /// Returns an iterator over CSS classes from the `class` attribute.
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.get_attribute("class")
            .unwrap_or("")
            .split_ascii_whitespace()
    }

    /// Returns `true` if the element has the specified class.
    pub fn has_class(&self, class_name: &str) -> bool {
        self.classes().any(|c| c == class_name)
    }

    /// Sets an attribute value by name (case-insensitive for HTML attributes).
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some((_, v)) = self.attributes.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
            *v = value.to_string();
        } else {
            self.attributes.push((name.to_string(), value.to_string()));
        }
    }

    /// Removes an attribute by name (case-insensitive for HTML attributes).
    pub fn remove_attribute(&mut self, name: &str) {
        self.attributes.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
    }
}

/// Rendering mode determined by DOCTYPE parsing per WHATWG HTML §13.2.6.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuirksMode {
    #[default]
    NoQuirks,
    LimitedQuirks,
    Quirks,
}

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

/// A complete DOM document containing all allocated nodes.
#[derive(Debug, Clone)]
pub struct Document {
    arena: Arena<Node>,
    root: NodeId,
    pub quirks_mode: QuirksMode,
    /// String interner for deduplicating tag names and attribute names (OPT-002).
    pub interner: StringInterner,
    /// Set of DOM nodes marked dirty due to attribute/class/DOM mutations (ARCH-002).
    pub dirty_nodes: HashSet<NodeId>,
    /// The document's character encoding (canonical name e.g. "UTF-8", "windows-1252") (PRD 5.4).
    pub character_set: String,
    /// The document's MIME type (e.g. "text/html").
    pub content_type: String,
    /// The document's active target fragment identifier for the :target selector (PRD 6.1).
    pub target_id: Option<String>,
}

impl Document {
    /// Creates a new empty document with a root `Document` node and pre-populated string interner.
    pub fn new() -> Self {
        let mut arena = Arena::new();
        // Allocate root placeholder
        let root = arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Document,
        });

        Self {
            arena,
            root,
            quirks_mode: QuirksMode::NoQuirks,
            interner: StringInterner::with_common_strings(),
            dirty_nodes: HashSet::new(),
            character_set: "UTF-8".to_string(),
            content_type: "text/html".to_string(),
            target_id: None,
        }
    }

    /// Sets or clears the active target fragment identifier for the `:target` CSS pseudo-class.
    pub fn set_target_id(&mut self, target: Option<String>) {
        self.target_id = target;
    }

    /// Marks a DOM node as dirty for incremental style recalculation (ARCH-002).
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        self.dirty_nodes.insert(node_id);
    }

    /// Returns a reference to the set of dirty nodes.
    pub fn dirty_nodes(&self) -> &HashSet<NodeId> {
        &self.dirty_nodes
    }

    /// Returns `true` if any nodes are currently marked dirty.
    pub fn has_dirty_nodes(&self) -> bool {
        !self.dirty_nodes.is_empty()
    }

    /// Takes the set of dirty nodes, clearing the internal tracker.
    pub fn take_dirty_nodes(&mut self) -> HashSet<NodeId> {
        std::mem::take(&mut self.dirty_nodes)
    }

    /// Clears all dirty flags.
    pub fn clear_dirty(&mut self) {
        self.dirty_nodes.clear();
    }

    /// Sets an attribute on an element, automatically marking the node dirty (ARCH-002).
    pub fn set_attribute(&mut self, node_id: NodeId, name: &str, value: &str) {
        if let Some(node) = self.arena.get_mut(node_id) {
            if let NodeData::Element(elem) = &mut node.data {
                let lower = name.to_ascii_lowercase();
                if let Some(pos) = elem.attributes.iter().position(|(k, _)| k.eq_ignore_ascii_case(&lower)) {
                    elem.attributes[pos].1 = value.to_string();
                } else {
                    elem.attributes.push((lower, value.to_string()));
                }
                self.dirty_nodes.insert(node_id);
            }
        }
    }

    /// Removes an attribute from an element, marking the node dirty (ARCH-002).
    pub fn remove_attribute(&mut self, node_id: NodeId, name: &str) {
        if let Some(node) = self.arena.get_mut(node_id) {
            if let NodeData::Element(elem) = &mut node.data {
                elem.attributes.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
                self.dirty_nodes.insert(node_id);
            }
        }
    }

    /// Adds a CSS class to an element if not already present, marking the node dirty (ARCH-002).
    pub fn add_class(&mut self, node_id: NodeId, class_name: &str) {
        if let Some(node) = self.arena.get_mut(node_id) {
            if let NodeData::Element(elem) = &mut node.data {
                let existing = elem.get_attribute("class").unwrap_or("").to_string();
                let classes: Vec<&str> = existing.split_ascii_whitespace().collect();
                if !classes.contains(&class_name) {
                    let new_class = if existing.is_empty() {
                        class_name.to_string()
                    } else {
                        format!("{existing} {class_name}")
                    };
                    if let Some(pos) = elem.attributes.iter().position(|(k, _)| k.eq_ignore_ascii_case("class")) {
                        elem.attributes[pos].1 = new_class;
                    } else {
                        elem.attributes.push(("class".to_string(), new_class));
                    }
                    self.dirty_nodes.insert(node_id);
                }
            }
        }
    }

    /// Removes a CSS class from an element, marking the node dirty (ARCH-002).
    pub fn remove_class(&mut self, node_id: NodeId, class_name: &str) {
        if let Some(node) = self.arena.get_mut(node_id) {
            if let NodeData::Element(elem) = &mut node.data {
                if let Some(class_val) = elem.get_attribute("class") {
                    let updated: Vec<&str> = class_val
                        .split_ascii_whitespace()
                        .filter(|&c| c != class_name)
                        .collect();
                    let new_class = updated.join(" ");
                    if let Some(pos) = elem.attributes.iter().position(|(k, _)| k.eq_ignore_ascii_case("class")) {
                        elem.attributes[pos].1 = new_class;
                    }
                    self.dirty_nodes.insert(node_id);
                }
            }
        }
    }

    /// Toggles a CSS class on an element, returning `true` if added, `false` if removed (ARCH-002).
    pub fn toggle_class(&mut self, node_id: NodeId, class_name: &str) -> bool {
        let has = if let Some(node) = self.get(node_id) {
            if let NodeData::Element(elem) = &node.data {
                elem.has_class(class_name)
            } else {
                false
            }
        } else {
            false
        };

        if has {
            self.remove_class(node_id, class_name);
            false
        } else {
            self.add_class(node_id, class_name);
            true
        }
    }

    /// Interns a string into the document's string interner (OPT-002).
    pub fn intern(&mut self, s: &str) -> InternedString {
        self.interner.intern(s)
    }

    /// Resolves an interned handle back to its string slice (OPT-002).
    pub fn resolve_atom(&self, atom: InternedString) -> &str {
        self.interner.resolve(atom)
    }

    /// Returns the root document node ID.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Gets a reference to a node by its ID.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.arena.get(id)
    }

    /// Gets a mutable reference to a node by its ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.arena.get_mut(id)
    }

    /// Creates an Element node in the document arena, automatically interning
    /// its tag name and attribute keys into the document's interner (OPT-002).
    pub fn create_element(&mut self, tag_name: &str, attributes: Vec<(String, String)>) -> NodeId {
        let tag_atom = self.interner.intern(tag_name);
        for (k, _) in &attributes {
            self.interner.intern(k);
        }
        let template_contents = if tag_name.eq_ignore_ascii_case("template") {
            Some(self.create_fragment())
        } else {
            None
        };
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Element(ElementData {
                tag_name: tag_name.to_string(),
                tag_atom: Some(tag_atom),
                attributes,
                shadow_root: None,
                template_contents,
            }),
        })
    }

    /// Creates a `DocumentFragment` node in the document arena.
    pub fn create_fragment(&mut self) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::DocumentFragment,
        })
    }

    /// Returns the template content `DocumentFragment` for a `<template>` node, if any.
    pub fn template_contents(&self, node_id: NodeId) -> Option<NodeId> {
        if let Some(node) = self.get(node_id) {
            if let NodeData::Element(el) = &node.data {
                return el.template_contents;
            }
        }
        None
    }

    /// Gets or creates a template content `DocumentFragment` for a `<template>` element.
    pub fn get_or_create_template_contents(&mut self, node_id: NodeId) -> NodeId {
        if let Some(contents) = self.template_contents(node_id) {
            return contents;
        }
        let frag = self.create_fragment();
        if let Some(node) = self.get_mut(node_id) {
            if let NodeData::Element(el) = &mut node.data {
                el.template_contents = Some(frag);
            }
        }
        frag
    }

    /// Sets the template content `DocumentFragment` for a `<template>` element.
    pub fn set_template_contents(&mut self, node_id: NodeId, contents: NodeId) {
        if let Some(node) = self.get_mut(node_id) {
            if let NodeData::Element(el) = &mut node.data {
                el.template_contents = Some(contents);
            }
        }
    }

    /// Returns `true` when the node is a `DocumentFragment`.
    pub fn is_document_fragment(&self, node_id: NodeId) -> bool {
        matches!(self.get(node_id).map(|n| &n.data), Some(NodeData::DocumentFragment))
    }

    /// Attaches a shadow root (as a DocumentFragment) to an element node.
    pub fn attach_shadow(&mut self, host_id: NodeId) -> Option<NodeId> {
        let shadow_fragment = self.create_fragment();
        if let Some(host_node) = self.get_mut(host_id) {
            if let NodeData::Element(el) = &mut host_node.data {
                el.shadow_root = Some(shadow_fragment);
                return Some(shadow_fragment);
            }
        }
        None
    }

    /// Returns the shadow root attached to an element node, if any.
    pub fn get_shadow_root(&self, host_id: NodeId) -> Option<NodeId> {
        if let Some(node) = self.get(host_id) {
            if let NodeData::Element(el) = &node.data {
                return el.shadow_root;
            }
        }
        None
    }

    /// Creates a Text node in the document arena.
    pub fn create_text(&mut self, text: &str) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Text(text.to_string()),
        })
    }

    /// Creates a Comment node in the document arena.
    pub fn create_comment(&mut self, comment: &str) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Comment(comment.to_string()),
        })
    }

    /// Creates a DocumentType node in the document arena.
    pub fn create_doctype(&mut self, name: &str, public_id: &str, system_id: &str) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::DocumentType {
                name: name.to_string(),
                public_id: public_id.to_string(),
                system_id: system_id.to_string(),
            },
        })
    }

    /// Returns `true` if `ancestor` is an ancestor of `descendant` (or if they are identical).
    pub fn is_ancestor(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        if ancestor == descendant {
            return true;
        }
        let mut cur = self.arena.get(descendant).and_then(|n| n.parent);
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.arena.get(p).and_then(|n| n.parent);
        }
        false
    }

    /// Appends a child node to a parent node.
    ///
    /// Per the DOM spec, appending a `DocumentFragment` moves all of its children
    /// rather than inserting the fragment itself.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        if parent == child || self.is_ancestor(child, parent) {
            return;
        }

        // Validate that parent can have children and child can be appended
        if let Some(p_node) = self.arena.get(parent) {
            if matches!(p_node.data, NodeData::Text(_) | NodeData::Comment(_) | NodeData::DocumentType { .. }) {
                return;
            }
        } else {
            return;
        }

        if let Some(c_node) = self.arena.get(child) {
            if matches!(c_node.data, NodeData::Document | NodeData::DocumentType { .. }) {
                return;
            }
        } else {
            return;
        }

        if self.is_document_fragment(child) {
            let children: Vec<NodeId> = self.children(child).map(|n| n.id).collect();
            for grandchild in children {
                self.append_child(parent, grandchild);
            }
            return;
        }

        // Disconnect child from any previous parent/siblings first
        self.detach(child);

        let prev_last = {
            let Some(p) = self.arena.get_mut(parent) else { return; };
            let old_last = p.last_child;
            p.last_child = Some(child);
            if p.first_child.is_none() {
                p.first_child = Some(child);
            }
            old_last
        };

        if let Some(old_last) = prev_last
            && let Some(prev) = self.arena.get_mut(old_last) {
                prev.next_sibling = Some(child);
            }

        if let Some(c) = self.arena.get_mut(child) {
            c.parent = Some(parent);
            c.prev_sibling = prev_last;
            c.next_sibling = None;
        }

        self.dirty_nodes.insert(parent);
    }

    /// Inserts a child node immediately before an existing reference child.
    ///
    /// Per the DOM spec, inserting a `DocumentFragment` moves all of its children
    /// rather than inserting the fragment itself.
    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, before: NodeId) {
        if child == before || parent == child || self.is_ancestor(child, parent) {
            return;
        }

        if let Some(p_node) = self.arena.get(parent) {
            if matches!(p_node.data, NodeData::Text(_) | NodeData::Comment(_) | NodeData::DocumentType { .. }) {
                return;
            }
        } else {
            return;
        }

        if let Some(c_node) = self.arena.get(child) {
            if matches!(c_node.data, NodeData::Document | NodeData::DocumentType { .. }) {
                return;
            }
        } else {
            return;
        }

        if self.is_document_fragment(child) {
            let children: Vec<NodeId> = self.children(child).map(|n| n.id).collect();
            for grandchild in children {
                self.insert_before(parent, grandchild, before);
            }
            return;
        }

        let Some(b_node) = self.arena.get(before) else {
            self.append_child(parent, child);
            return;
        };
        if b_node.parent != Some(parent) {
            self.append_child(parent, child);
            return;
        }

        self.detach(child);

        let prev_sibling = self.arena.get(before).and_then(|b| b.prev_sibling);

        if let Some(prev) = prev_sibling {
            if let Some(p_node) = self.arena.get_mut(prev) {
                p_node.next_sibling = Some(child);
            }
        } else {
            // Inserting before the first child
            if let Some(p) = self.arena.get_mut(parent) {
                p.first_child = Some(child);
            }
        }

        if let Some(b_node) = self.arena.get_mut(before) {
            b_node.prev_sibling = Some(child);
        }

        if let Some(c) = self.arena.get_mut(child) {
            c.parent = Some(parent);
            c.prev_sibling = prev_sibling;
            c.next_sibling = Some(before);
        }

        self.dirty_nodes.insert(parent);
    }

    /// Replaces an existing child node with a new child node in `parent`.
    ///
    /// Per the DOM spec, if `new_child` is a `DocumentFragment`, its children are inserted
    /// in place of `old_child`. Returns `old_child`.
    pub fn replace_child(&mut self, parent: NodeId, new_child: NodeId, old_child: NodeId) -> NodeId {
        if new_child == old_child {
            return old_child;
        }
        if let Some(old) = self.arena.get(old_child) {
            if old.parent != Some(parent) {
                return old_child;
            }
        } else {
            return old_child;
        }

        self.insert_before(parent, new_child, old_child);
        self.detach(old_child);
        old_child
    }

    /// Inserts a child node immediately after an existing reference child.
    pub fn insert_after(&mut self, parent: NodeId, child: NodeId, after: NodeId) {
        if child == after {
            return;
        }
        let next_sibling = self.arena.get(after).and_then(|a| a.next_sibling);
        if let Some(next) = next_sibling {
            self.insert_before(parent, child, next);
        } else {
            self.append_child(parent, child);
        }
    }

    /// Detaches a node from its current parent and siblings.
    pub fn detach(&mut self, node: NodeId) {
        let (parent, prev, next) = match self.arena.get(node) {
            Some(n) => (n.parent, n.prev_sibling, n.next_sibling),
            None => return,
        };

        if let Some(p) = parent
            && let Some(p_node) = self.arena.get_mut(p) {
                if p_node.first_child == Some(node) {
                    p_node.first_child = next;
                }
                if p_node.last_child == Some(node) {
                    p_node.last_child = prev;
                }
            }

        if let Some(prev_id) = prev
            && let Some(prev_node) = self.arena.get_mut(prev_id) {
                prev_node.next_sibling = next;
            }

        if let Some(next_id) = next
            && let Some(next_node) = self.arena.get_mut(next_id) {
                next_node.prev_sibling = prev;
            }

        if let Some(n) = self.arena.get_mut(node) {
            n.parent = None;
            n.prev_sibling = None;
            n.next_sibling = None;
        }

        if let Some(p) = parent {
            self.dirty_nodes.insert(p);
        }
    }

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
        self.serialize_node(node_id, &mut out);
        out
    }

    /// Serializes only the children of `node_id` (used by `innerHTML`).
    pub fn serialize_children(&self, node_id: NodeId) -> String {
        let mut out = String::new();
        if let Some(node) = self.arena.get(node_id) {
            if let NodeData::Element(elem) = &node.data {
                if elem.tag_name.eq_ignore_ascii_case("template") {
                    if let Some(frag_id) = elem.template_contents {
                        for child in self.children(frag_id) {
                            self.serialize_node(child.id, &mut out);
                        }
                        return out;
                    }
                }
            }
        }
        for child in self.children(node_id) {
            self.serialize_node(child.id, &mut out);
        }
        out
    }

    fn serialize_node(&self, node_id: NodeId, out: &mut String) {
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
                    if elem.tag_name.eq_ignore_ascii_case("template") && elem.template_contents.is_some() {
                        if let Some(frag_id) = elem.template_contents {
                            for child in self.children(frag_id) {
                                self.serialize_node(child.id, out);
                            }
                        }
                    } else {
                        for child in self.children(node_id) {
                            self.serialize_node(child.id, out);
                        }
                    }
                    out.push_str("</");
                    out.push_str(&elem.tag_name);
                    out.push('>');
                }
            }
            NodeData::Text(text) => {
                let is_raw_text = node.parent
                    .and_then(|p| self.arena.get(p))
                    .map(|pn| match &pn.data {
                        NodeData::Element(el) => matches!(
                            el.tag_name.to_ascii_lowercase().as_str(),
                            "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext"
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
            NodeData::DocumentType { name, public_id, system_id } => {
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
                    self.serialize_node(child.id, out);
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

    fn collect_text(&self, node_id: NodeId, out: &mut String) {
        if let Some(node) = self.arena.get(node_id) {
            match &node.data {
                NodeData::Text(t) => out.push_str(t),
                NodeData::Element(el) if el.tag_name.eq_ignore_ascii_case("template") => {
                    // WHATWG HTML5 §4.12.3: textContent of template is empty
                }
                _ => {
                    for child in self.children(node_id) {
                        self.collect_text(child.id, out);
                    }
                }
            }
        }
    }

    /// Finds the first descendant element matching the tag name (case-insensitive).
    pub fn find_element_by_tag(&self, root: NodeId, tag: &str) -> Option<NodeId> {
        for child in self.children(root) {
            if let NodeData::Element(elem) = &child.data
                && elem.tag_name.eq_ignore_ascii_case(tag) {
                    return Some(child.id);
                }
            if let Some(found) = self.find_element_by_tag(child.id, tag) {
                return Some(found);
            }
        }
        None
    }

    /// Finds the first descendant element matching the `id` attribute.
    pub fn find_element_by_id(&self, root: NodeId, id: &str) -> Option<NodeId> {
        for child in self.children(root) {
            if let NodeData::Element(elem) = &child.data
                && elem.id() == Some(id) {
                    return Some(child.id);
                }
            if let Some(found) = self.find_element_by_id(child.id, id) {
                return Some(found);
            }
        }
        None
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
            NodeData::DocumentType { name, .. } => out.push_str(&format!("{pad}<!DOCTYPE {name}>\n")),
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

impl Default for Document {
    fn default() -> Self {
        Self::new()
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
        self.current = node.next_sibling;
        Some(node)
    }
}

#[cfg(test)]
mod tests {
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
        doc.insert_before(root, b, c);

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
        doc.insert_before(root, frag, d);

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
        let old = doc.replace_child(root, new_node, b);
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

        doc.replace_child(root, frag, new_node);
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
    fn test_serialize_template_inner_html() {
        let mut doc = Document::new();
        let t = doc.create_element("template", Vec::new());
        let frag = doc.create_fragment();
        let span = doc.create_element("span", Vec::new());
        let text = doc.create_text("inside template");
        doc.append_child(span, text);
        doc.append_child(frag, span);
        if let Some(node) = doc.get_mut(t) {
            if let NodeData::Element(elem) = &mut node.data {
                elem.template_contents = Some(frag);
            }
        }
        let inner = doc.serialize_children(t);
        assert_eq!(inner, "<span>inside template</span>");
    }

    #[test]
    fn test_serialize_doctype_identifiers() {
        let mut doc = Document::new();
        let dt = doc.create_doctype("html", "-//W3C//DTD HTML 4.01//EN", "http://www.w3.org/TR/html4/strict.dtd");
        let html = doc.serialize_html(dt);
        assert_eq!(html, "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">");
    }
}
