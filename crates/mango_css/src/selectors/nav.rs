//! DOM navigation helpers for CSS selector matching.

use mango_html::dom::{Document, NodeData, NodeId};

/// Checks whether `node_id` is a descendant of `ancestor_id` in `doc`.
pub fn is_descendant_of(mut node_id: NodeId, ancestor_id: NodeId, doc: &Document) -> bool {
    while let Some(parent) = parent_element(node_id, doc) {
        if parent == ancestor_id {
            return true;
        }
        node_id = parent;
    }
    false
}

/// Returns the parent element of `node_id` if it is an Element node.
pub fn parent_element(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let node = doc.get(node_id)?;
    let parent_id = node.parent?;
    let parent = doc.get(parent_id)?;
    if matches!(parent.data, NodeData::Element(_)) {
        Some(parent_id)
    } else {
        None
    }
}

/// Returns the previous sibling element of `node_id`, skipping non-element nodes.
pub fn prev_element_sibling(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let mut curr_id = doc.get(node_id)?.prev_sibling;
    while let Some(id) = curr_id {
        let node = doc.get(id)?;
        if matches!(node.data, NodeData::Element(_)) {
            return Some(id);
        }
        curr_id = node.prev_sibling;
    }
    None
}

/// Returns the next sibling element of `node_id`, skipping non-element nodes.
pub fn next_element_sibling(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let mut curr_id = doc.get(node_id)?.next_sibling;
    while let Some(id) = curr_id {
        let node = doc.get(id)?;
        if matches!(node.data, NodeData::Element(_)) {
            return Some(id);
        }
        curr_id = node.next_sibling;
    }
    None
}

/// Collects all subsequent sibling elements of `node_id`.
pub fn subsequent_element_siblings(node_id: NodeId, doc: &Document) -> Vec<NodeId> {
    let mut siblings = Vec::new();
    let mut curr_id = doc.get(node_id).and_then(|n| n.next_sibling);
    while let Some(id) = curr_id {
        if let Some(node) = doc.get(id) {
            if matches!(node.data, NodeData::Element(_)) {
                siblings.push(id);
            }
            curr_id = node.next_sibling;
        } else {
            break;
        }
    }
    siblings
}
