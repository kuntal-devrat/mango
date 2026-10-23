//! DOM tree data structures and arena-allocated node graph conforming to WHATWG DOM.
//!
//! Every node is owned by an [`mango_core::Arena`] inside the [`Document`]. Nodes
//! reference their parent, siblings, and children via lightweight [`NodeId`] handles.

pub mod document;
pub mod js_id;
pub mod mutation;
pub mod node;
pub mod traversal;

#[cfg(test)]
mod tests;

pub use document::Document;
pub use js_id::{
    JS_ID_GENERATION_MASK, JS_ID_MAX_GENERATION, JS_ID_MAX_INDEX, node_id_from_js, node_id_to_js,
};
pub use mutation::MAX_DOM_DEPTH;
pub use node::{DomError, ElementData, Node, NodeData, NodeId, QuirksMode};
pub use traversal::ChildrenIter;
