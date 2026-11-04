//! WHATWG HTML5 compliant TreeSink implementation for html5ever.
//!
//! Connects `html5ever`'s browser-grade parser state machine directly to
//! Mango's arena-allocated [`Document`] and [`Node`] tree.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use html5ever::tendril::StrTendril;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, QualName};
use mango_core::Atom;

use crate::dom::{Document, NodeData, NodeId};
use crate::tree_builder::charset::{canonical_charset, extract_charset_from_content};

/// A `TreeSink` adapter that constructs an arena-allocated [`Document`].
pub struct MangoTreeSink {
    pub(crate) doc: RefCell<Document>,
    qual_names: RefCell<HashMap<NodeId, QualName>>,
    template_contents: RefCell<HashMap<NodeId, NodeId>>,
    declarative_templates: RefCell<std::collections::HashSet<NodeId>>,
}

impl MangoTreeSink {
    /// Creates a new `MangoTreeSink` with an empty document.
    pub fn new() -> Self {
        Self::with_document(Document::new())
    }

    /// Creates a new `MangoTreeSink` wrapping an existing document.
    pub fn with_document(doc: Document) -> Self {
        Self {
            doc: RefCell::new(doc),
            qual_names: RefCell::new(HashMap::new()),
            template_contents: RefCell::new(HashMap::new()),
            declarative_templates: RefCell::new(std::collections::HashSet::new()),
        }
    }
}

impl Default for MangoTreeSink {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSink for MangoTreeSink {
    type Handle = NodeId;
    type Output = Document;
    type ElemName<'a> = std::cell::Ref<'a, QualName>;

    fn finish(self) -> Self::Output {
        self.doc.into_inner()
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        log::trace!("HTML5 parse error: {msg}");
    }

    fn get_document(&self) -> Self::Handle {
        self.doc.borrow().root
    }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> Self::ElemName<'a> {
        std::cell::Ref::map(self.qual_names.borrow(), |map| {
            map.get(target).expect("element QualName missing")
        })
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Self::Handle {
        let mut doc = self.doc.borrow_mut();
        let tag_str = &name.local;
        let mut mango_attrs = Vec::with_capacity(attrs.len());
        for attr in &attrs {
            let attr_name = if let Some(ref prefix) = attr.name.prefix {
                format!("{}:{}", prefix, attr.name.local)
            } else {
                attr.name.local.to_string()
            };
            mango_attrs.push((Atom::from(attr_name), attr.value.to_string()));
        }

        // Process meta charset / http-equiv
        if *tag_str == html5ever::local_name!("meta") {
            for (k, v) in &mango_attrs {
                if k.eq_ignore_ascii_case("charset")
                    && let Some(canon) = canonical_charset(v)
                {
                    doc.character_set = canon.to_string();
                    break;
                }
            }
            let is_http_equiv_ct = mango_attrs.iter().any(|(k, v)| {
                k.eq_ignore_ascii_case("http-equiv")
                    && v.trim().eq_ignore_ascii_case("content-type")
            });
            if is_http_equiv_ct
                && let Some((_, content)) = mango_attrs
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content"))
                && let Some(cs) = extract_charset_from_content(content)
                && let Some(canon) = canonical_charset(&cs)
            {
                doc.character_set = canon.to_string();
            }
        }

        let element_id = doc.create_element(tag_str, mango_attrs);

        if flags.template || name.local == html5ever::local_name!("template") {
            let frag = doc.create_fragment();
            self.template_contents.borrow_mut().insert(element_id, frag);
            if let Some(node) = doc.arena.get_mut(element_id) {
                if let NodeData::Element(ref mut el) = node.data {
                    el.template_contents = Some(frag);
                }
            }
        }

        self.qual_names.borrow_mut().insert(element_id, name);
        element_id
    }

    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        self.doc.borrow_mut().create_comment(&text)
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
        self.doc
            .borrow_mut()
            .create_comment(&format!("?{target} {data}?"))
    }

    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut doc = self.doc.borrow_mut();
        match child {
            NodeOrText::AppendNode(node_id) => {
                doc.append_child(*parent, node_id);
            }
            NodeOrText::AppendText(tendril) => {
                let merged = doc
                    .arena
                    .get(*parent)
                    .and_then(|p| p.last_child)
                    .and_then(|last_id| doc.arena.get_mut(last_id))
                    .and_then(|last_node| match &mut last_node.data {
                        NodeData::Text(s) => {
                            s.push_str(&tendril);
                            Some(())
                        }
                        _ => None,
                    });
                if merged.is_none() {
                    let text_id = doc.create_text(&tendril);
                    doc.append_child(*parent, text_id);
                }
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let has_parent = self
            .doc
            .borrow()
            .arena
            .get(*element)
            .and_then(|n| n.parent)
            .is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let mut doc = self.doc.borrow_mut();
        let root = doc.root;
        let doctype = doc.create_doctype(&name, &public_id, &system_id);
        doc.append_child(root, doctype);
    }

    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        *self
            .template_contents
            .borrow()
            .get(target)
            .expect("template contents missing")
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x.matches(*y)
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        let qm = match mode {
            QuirksMode::Quirks => crate::dom::QuirksMode::Quirks,
            QuirksMode::LimitedQuirks => crate::dom::QuirksMode::LimitedQuirks,
            QuirksMode::NoQuirks => crate::dom::QuirksMode::NoQuirks,
        };
        self.doc.borrow_mut().quirks_mode = qm;
    }

    fn append_before_sibling(&self, sibling: &Self::Handle, new_node: NodeOrText<Self::Handle>) {
        let mut doc = self.doc.borrow_mut();
        let parent = doc
            .arena
            .get(*sibling)
            .and_then(|n| n.parent)
            .expect("sibling must have parent");
        let prev_sibling = doc.arena.get(*sibling).and_then(|n| n.prev_sibling);
        match new_node {
            NodeOrText::AppendText(tendril) => {
                let merged = prev_sibling
                    .and_then(|prev_id| doc.arena.get_mut(prev_id))
                    .and_then(|prev_node| match &mut prev_node.data {
                        NodeData::Text(s) => {
                            s.push_str(&tendril);
                            Some(())
                        }
                        _ => None,
                    });
                if merged.is_none() {
                    let text_id = doc.create_text(&tendril);
                    let _ = doc.insert_before(parent, text_id, *sibling);
                }
            }
            NodeOrText::AppendNode(node_id) => {
                let _ = doc.insert_before(parent, node_id, *sibling);
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let mut doc = self.doc.borrow_mut();
        if let Some(node) = doc.arena.get_mut(*target) {
            if let NodeData::Element(ref mut elem) = node.data {
                for attr in attrs {
                    let attr_name = if let Some(ref prefix) = attr.name.prefix {
                        format!("{}:{}", prefix, attr.name.local)
                    } else {
                        attr.name.local.to_string()
                    };
                    let atom = Atom::from(attr_name);
                    if !elem.attributes.iter().any(|(k, _)| k == &atom) {
                        elem.attributes.push((atom, attr.value.to_string()));
                    }
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        self.doc.borrow_mut().detach(*target);
    }

    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
        let mut doc = self.doc.borrow_mut();
        let mut child = doc.arena.get(*node).and_then(|n| n.first_child);
        let mut children_to_move = Vec::new();
        while let Some(c) = child {
            children_to_move.push(c);
            child = doc.arena.get(c).and_then(|n| n.next_sibling);
        }
        for c in children_to_move {
            doc.append_child(*new_parent, c);
        }
    }

    fn pop(&self, node: &Self::Handle) {
        if self.declarative_templates.borrow().contains(node) {
            self.doc.borrow_mut().detach(*node);
        }
    }

    fn allow_declarative_shadow_roots(&self, _intended_parent: &Self::Handle) -> bool {
        true
    }

    fn attach_declarative_shadow(
        &self,
        element: &Self::Handle,
        shadow_root: &Self::Handle,
        attrs: &[Attribute],
    ) -> bool {
        let mut mode = "open";
        let mut clonable = false;
        let mut delegates_focus = false;
        for attr in attrs {
            let key = &*attr.name.local;
            if key.eq_ignore_ascii_case("shadowrootmode") || key.eq_ignore_ascii_case("shadowroot")
            {
                mode = if attr.value.eq_ignore_ascii_case("closed") {
                    "closed"
                } else {
                    "open"
                };
            } else if key.eq_ignore_ascii_case("shadowrootclonable") {
                clonable = true;
            } else if key.eq_ignore_ascii_case("shadowrootdelegatesfocus") {
                delegates_focus = true;
            }
        }
        self.declarative_templates.borrow_mut().insert(*shadow_root);
        let shadow_frag = self
            .template_contents
            .borrow()
            .get(shadow_root)
            .copied()
            .unwrap_or(*shadow_root);

        let mut doc = self.doc.borrow_mut();
        if let Some(node) = doc.arena.get_mut(*element) {
            if let NodeData::Element(ref mut el) = node.data {
                el.shadow_root = Some(shadow_frag);
                el.shadow_root_mode = Some(mode.to_string());
                el.shadow_root_clonable = clonable;
                el.shadow_root_delegates_focus = delegates_focus;
                doc.shadow_hosts.insert(shadow_frag, *element);
                return true;
            }
        }
        false
    }
}

/// Parses raw HTML string into a [`Document`] using `html5ever`.
pub fn parse_html5ever(html: &str) -> Document {
    let sink = MangoTreeSink::new();
    let mut opts = html5ever::ParseOpts::default();
    opts.tree_builder.scripting_enabled = false;
    let parser = html5ever::parse_document(sink, opts);
    let mut bytes = html.as_bytes();
    let mut doc = parser
        .from_utf8()
        .read_from(&mut bytes)
        .expect("HTML5ever parsing should not fail on in-memory buffers");
    doc.dirty_nodes.clear();
    doc
}
