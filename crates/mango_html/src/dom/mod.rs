//! DOM tree data structures and arena-allocated node graph conforming to WHATWG DOM.
//!
//! Every node is owned by an [`mango_core::Arena`] inside the [`Document`]. Nodes
//! reference their parent, siblings, and children via lightweight [`NodeId`] handles.

pub mod document;
pub mod mutation;
pub mod node;
pub mod traversal;

#[cfg(test)]
mod tests;

pub use document::Document;
pub use node::{DomError, ElementData, Node, NodeData, NodeId, QuirksMode};
pub use traversal::ChildrenIter;
