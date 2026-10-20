use std::sync::Arc;

use mango_css::computed::ComputedStyle;
use mango_html::dom::NodeId;

/// A node in the style tree, holding computed style, optional DOM ID, and optional text payload.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledNode {
    pub node_id: Option<NodeId>,
    pub style: Arc<ComputedStyle>,
    pub text: Option<String>,
    /// The element's tag name (e.g. "div", "img"), or None for text/document nodes.
    pub tag_name: Option<String>,
    /// Element attributes (e.g. "src", "width", "height").
    pub attributes: Vec<(String, String)>,
    pub children: Vec<StyledNode>,
}

impl StyledNode {
    pub fn new(node_id: Option<NodeId>, style: impl Into<Arc<ComputedStyle>>) -> Self {
        Self {
            node_id,
            style: style.into(),
            text: None,
            tag_name: None,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_text(
        node_id: Option<NodeId>,
        style: impl Into<Arc<ComputedStyle>>,
        text: String,
    ) -> Self {
        Self {
            node_id,
            style: style.into(),
            text: Some(text),
            tag_name: None,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Returns `true` if this styled node represents a text fragment.
    pub fn is_text(&self) -> bool {
        self.text.is_some()
    }
}
