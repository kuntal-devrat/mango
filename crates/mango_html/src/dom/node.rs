//! DOM nodes, element data, and DOM error types conforming to WHATWG DOM.

use mango_core::{Id, InternedString};

/// DOM errors per WHATWG DOM spec (NotFoundError / HierarchyRequestError).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomError {
    NotFoundError,
    HierarchyRequestError,
}

impl std::fmt::Display for DomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomError::NotFoundError => write!(f, "NotFoundError"),
            DomError::HierarchyRequestError => write!(f, "HierarchyRequestError"),
        }
    }
}

impl std::error::Error for DomError {}

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
    pub shadow_root_mode: Option<String>,
    pub shadow_root_clonable: bool,
    pub shadow_root_delegates_focus: bool,
    pub template_contents: Option<NodeId>,
    pub is_hovered: bool,
    pub is_active: bool,
}

impl ElementData {
    pub fn new(tag_name: impl Into<String>, attributes: Vec<(String, String)>) -> Self {
        Self {
            tag_name: tag_name.into(),
            tag_atom: None,
            attributes,
            shadow_root: None,
            shadow_root_mode: None,
            shadow_root_clonable: false,
            shadow_root_delegates_focus: false,
            template_contents: None,
            is_hovered: false,
            is_active: false,
        }
    }

    /// Creates a new `ElementData` with an interned tag atom.
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
            shadow_root_mode: None,
            shadow_root_clonable: false,
            shadow_root_delegates_focus: false,
            template_contents: None,
            is_hovered: false,
            is_active: false,
        }
    }

    /// Returns whether the element is hovered.
    pub fn is_hovered(&self) -> bool {
        self.is_hovered || self.get_attribute("data-mango-hover") == Some("true")
    }

    /// Returns whether the element is active.
    pub fn is_active(&self) -> bool {
        self.is_active || self.get_attribute("data-mango-active") == Some("true")
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
        if let Some((_, v)) = self
            .attributes
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
        {
            *v = value.to_string();
        } else {
            self.attributes.push((name.to_string(), value.to_string()));
        }
    }

    /// Removes an attribute by name (case-insensitive for HTML attributes).
    pub fn remove_attribute(&mut self, name: &str) {
        self.attributes
            .retain(|(k, _)| !k.eq_ignore_ascii_case(name));
    }

    /// Returns `true` if this element is `<details>`.
    pub fn is_details(&self) -> bool {
        self.tag_name.eq_ignore_ascii_case("details")
    }

    /// Returns `true` if this element is `<summary>`.
    pub fn is_summary(&self) -> bool {
        self.tag_name.eq_ignore_ascii_case("summary")
    }

    /// Returns `true` if this element has the `open` attribute.
    pub fn is_open(&self) -> bool {
        self.get_attribute("open").is_some()
    }

    /// Toggles the `open` attribute on this element, returning the new open state.
    pub fn toggle_open(&mut self) -> bool {
        if self.is_open() {
            self.remove_attribute("open");
            false
        } else {
            self.set_attribute("open", "");
            true
        }
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
