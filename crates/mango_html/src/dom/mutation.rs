//! DOM tree mutation algorithms conforming to WHATWG DOM: insertion, removal, replacement, and recycling.

use super::document::Document;
use super::node::{DomError, NodeData, NodeId};

impl Document {
    /// Returns `true` if `ancestor` is an ancestor of `descendant` (or if they are identical).
    pub fn is_ancestor(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        if ancestor.matches(descendant) {
            return true;
        }
        let mut cur = self.arena.get(descendant).and_then(|n| n.parent);
        while let Some(p) = cur {
            if p.matches(ancestor) {
                return true;
            }
            cur = self.arena.get(p).and_then(|n| n.parent);
        }
        false
    }

    pub const DOCUMENT_POSITION_DISCONNECTED: u16 = 0x01;
    pub const DOCUMENT_POSITION_PRECEDING: u16 = 0x02;
    pub const DOCUMENT_POSITION_FOLLOWING: u16 = 0x04;
    pub const DOCUMENT_POSITION_CONTAINS: u16 = 0x08;
    pub const DOCUMENT_POSITION_CONTAINED_BY: u16 = 0x10;
    pub const DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: u16 = 0x20;

    /// Compares the document position of two nodes in tree order per W3C DOM spec.
    pub fn compare_document_position(&self, a: NodeId, b: NodeId) -> u16 {
        if a.matches(b) {
            return 0;
        }

        if self.arena.get(a).is_none() || self.arena.get(b).is_none() {
            let order = if a.raw() < b.raw() {
                Self::DOCUMENT_POSITION_FOLLOWING
            } else {
                Self::DOCUMENT_POSITION_PRECEDING
            };
            return Self::DOCUMENT_POSITION_DISCONNECTED
                | Self::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC
                | order;
        }

        // Check if b is descendant of a (a contains b)
        if self.is_ancestor(a, b) {
            return Self::DOCUMENT_POSITION_CONTAINED_BY | Self::DOCUMENT_POSITION_FOLLOWING;
        }

        // Check if a is descendant of b (b contains a)
        if self.is_ancestor(b, a) {
            return Self::DOCUMENT_POSITION_CONTAINS | Self::DOCUMENT_POSITION_PRECEDING;
        }

        // Build ancestor chains from root to a and from root to b
        let mut path_a = Vec::new();
        let mut cur = Some(a);
        while let Some(n) = cur {
            path_a.push(n);
            cur = self.arena.get(n).and_then(|node| node.parent);
        }
        path_a.reverse();

        let mut path_b = Vec::new();
        let mut cur = Some(b);
        while let Some(n) = cur {
            path_b.push(n);
            cur = self.arena.get(n).and_then(|node| node.parent);
        }
        path_b.reverse();

        // Disconnected subtrees
        if path_a.is_empty() || path_b.is_empty() || !path_a[0].matches(path_b[0]) {
            let order = if a.raw() < b.raw() {
                Self::DOCUMENT_POSITION_FOLLOWING
            } else {
                Self::DOCUMENT_POSITION_PRECEDING
            };
            return Self::DOCUMENT_POSITION_DISCONNECTED
                | Self::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC
                | order;
        }

        // Find lowest common ancestor
        let min_len = path_a.len().min(path_b.len());
        let mut diff_idx = min_len;
        for i in 0..min_len {
            if !path_a[i].matches(path_b[i]) {
                diff_idx = i;
                break;
            }
        }

        if diff_idx == 0 {
            let order = if a.raw() < b.raw() {
                Self::DOCUMENT_POSITION_FOLLOWING
            } else {
                Self::DOCUMENT_POSITION_PRECEDING
            };
            return Self::DOCUMENT_POSITION_DISCONNECTED
                | Self::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC
                | order;
        }

        let lca = path_a[diff_idx - 1];
        let branch_a = path_a[diff_idx];
        let branch_b = path_b[diff_idx];

        // Traverse children of lca to see whether branch_a or branch_b appears first
        for child in self.children(lca) {
            if child.id.matches(branch_b) {
                // b comes before a
                return Self::DOCUMENT_POSITION_PRECEDING;
            }
            if child.id.matches(branch_a) {
                // a comes before b, so b follows a
                return Self::DOCUMENT_POSITION_FOLLOWING;
            }
        }

        Self::DOCUMENT_POSITION_FOLLOWING
    }

    /// Appends a child node to a parent node.
    ///
    /// Per the DOM spec, appending a `DocumentFragment` moves all of its children
    /// rather than inserting the fragment itself.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        let parent = match self.arena.get(parent) {
            Some(n) => n.id,
            None => return,
        };
        let child = match self.arena.get(child) {
            Some(n) => n.id,
            None => return,
        };
        if parent == child || self.is_ancestor(child, parent) {
            return;
        }

        // Validate that parent can have children and child can be appended
        if let Some(p_node) = self.arena.get(parent) {
            if matches!(
                p_node.data,
                NodeData::Text(_) | NodeData::Comment(_) | NodeData::DocumentType { .. }
            ) {
                return;
            }
        } else {
            return;
        }

        if let Some(c_node) = self.arena.get(child) {
            if matches!(c_node.data, NodeData::Document) {
                return;
            }
            if matches!(c_node.data, NodeData::DocumentType { .. })
                && !matches!(self.arena.get(parent).map(|p| &p.data), Some(NodeData::Document))
            {
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
            let Some(p) = self.arena.get_mut(parent) else {
                return;
            };
            let old_last = p.last_child;
            p.last_child = Some(child);
            if p.first_child.is_none() {
                p.first_child = Some(child);
            }
            old_last
        };

        if let Some(old_last) = prev_last
            && let Some(prev) = self.arena.get_mut(old_last)
        {
            prev.next_sibling = Some(child);
        }

        if let Some(c) = self.arena.get_mut(child) {
            c.parent = Some(parent);
            c.prev_sibling = prev_last;
            c.next_sibling = None;
        }

        self.dirty_nodes.insert(parent);
        self.version = self.version.wrapping_add(1);
    }

    /// Inserts a child node immediately before an existing reference child.
    ///
    /// Per the DOM spec, inserting a `DocumentFragment` moves all of its children
    /// rather than inserting the fragment itself.
    ///
    /// Returns `Err(NotFoundError)` when `before` is unknown or not a child of
    /// `parent`; `Err(HierarchyRequestError)` for ancestor cycles or invalid
    /// parent/child node types. Never falls back to append.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        before: NodeId,
    ) -> Result<(), DomError> {
        let parent = match self.arena.get(parent) {
            None => return Err(DomError::NotFoundError),
            Some(n) => n.id,
        };
        let child = match self.arena.get(child) {
            None => return Err(DomError::NotFoundError),
            Some(n) => n.id,
        };
        let before = match self.arena.get(before) {
            None => return Err(DomError::NotFoundError),
            Some(n) => n.id,
        };
        if child == before || parent == child || self.is_ancestor(child, parent) {
            return Err(DomError::HierarchyRequestError);
        }

        match self.arena.get(parent) {
            None => return Err(DomError::NotFoundError),
            Some(p_node) => {
                if matches!(
                    p_node.data,
                    NodeData::Text(_) | NodeData::Comment(_) | NodeData::DocumentType { .. }
                ) {
                    return Err(DomError::HierarchyRequestError);
                }
            }
        }

        match self.arena.get(child) {
            None => return Err(DomError::NotFoundError),
            Some(c_node) => {
                if matches!(c_node.data, NodeData::Document) {
                    return Err(DomError::HierarchyRequestError);
                }
                if matches!(c_node.data, NodeData::DocumentType { .. })
                    && !matches!(self.arena.get(parent).map(|p| &p.data), Some(NodeData::Document))
                {
                    return Err(DomError::HierarchyRequestError);
                }
            }
        }

        if self.is_document_fragment(child) {
            let children: Vec<NodeId> = self.children(child).map(|n| n.id).collect();
            for grandchild in children {
                self.insert_before(parent, grandchild, before)?;
            }
            return Ok(());
        }

        let b_parent = match self.arena.get(before) {
            None => return Err(DomError::NotFoundError),
            Some(b_node) => b_node.parent,
        };
        if b_parent != Some(parent) {
            return Err(DomError::NotFoundError);
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
        self.version = self.version.wrapping_add(1);
        Ok(())
    }

    /// Replaces an existing child node with a new child node in `parent`.
    ///
    /// Per the DOM spec, if `new_child` is a `DocumentFragment`, its children are inserted
    /// in place of `old_child`. Returns `old_child`.
    pub fn replace_child(
        &mut self,
        parent: NodeId,
        new_child: NodeId,
        old_child: NodeId,
    ) -> Result<NodeId, DomError> {
        let parent = match self.arena.get(parent) {
            Some(n) => n.id,
            None => return Err(DomError::NotFoundError),
        };
        let new_child = match self.arena.get(new_child) {
            Some(n) => n.id,
            None => return Err(DomError::NotFoundError),
        };
        let old_child = match self.arena.get(old_child) {
            Some(n) => n.id,
            None => return Err(DomError::NotFoundError),
        };
        if new_child == old_child {
            return Ok(old_child);
        }
        if let Some(old) = self.arena.get(old_child) {
            if old.parent != Some(parent) {
                return Err(DomError::NotFoundError);
            }
        } else {
            return Err(DomError::NotFoundError);
        }

        self.insert_before(parent, new_child, old_child)?;
        self.detach(old_child);
        Ok(old_child)
    }

    /// Inserts a child node immediately after an existing reference child.
    ///
    /// Returns `Err(NotFoundError)` when `after` is unknown or not a child of
    /// `parent`; `Err(HierarchyRequestError)` for ancestor cycles or invalid
    /// node types. Never falls back to append on error.
    pub fn insert_after(
        &mut self,
        parent: NodeId,
        child: NodeId,
        after: NodeId,
    ) -> Result<(), DomError> {
        let parent = match self.arena.get(parent) {
            None => return Err(DomError::NotFoundError),
            Some(n) => n.id,
        };
        let child = match self.arena.get(child) {
            None => return Err(DomError::NotFoundError),
            Some(n) => n.id,
        };
        let after = match self.arena.get(after) {
            None => return Err(DomError::NotFoundError),
            Some(n) => n.id,
        };
        if child == after {
            return Ok(());
        }
        // Validate parent/child.
        if parent == child || self.is_ancestor(child, parent) {
            return Err(DomError::HierarchyRequestError);
        }
        match self.arena.get(parent) {
            None => return Err(DomError::NotFoundError),
            Some(p) => {
                if matches!(
                    p.data,
                    NodeData::Text(_) | NodeData::Comment(_) | NodeData::DocumentType { .. }
                ) {
                    return Err(DomError::HierarchyRequestError);
                }
            }
        }
        match self.arena.get(child) {
            None => return Err(DomError::NotFoundError),
            Some(c) => {
                if matches!(c.data, NodeData::Document | NodeData::DocumentType { .. }) {
                    return Err(DomError::HierarchyRequestError);
                }
            }
        }
        let (after_parent, next_sibling) = match self.arena.get(after) {
            None => return Err(DomError::NotFoundError),
            Some(a) => (a.parent, a.next_sibling),
        };
        if after_parent != Some(parent) {
            return Err(DomError::NotFoundError);
        }
        if let Some(next) = next_sibling {
            self.insert_before(parent, child, next)
        } else {
            self.append_child(parent, child);
            Ok(())
        }
    }

    /// Detaches a node from its current parent and siblings.
    pub fn detach(&mut self, node: NodeId) {
        let node = match self.arena.get(node) {
            Some(n) => n.id,
            None => return,
        };
        let (parent, prev, next) = match self.arena.get(node) {
            Some(n) => (n.parent, n.prev_sibling, n.next_sibling),
            None => return,
        };

        if let Some(p) = parent
            && let Some(p_node) = self.arena.get_mut(p)
        {
            if p_node.first_child == Some(node) {
                p_node.first_child = next;
            }
            if p_node.last_child == Some(node) {
                p_node.last_child = prev;
            }
        }

        if let Some(prev_id) = prev
            && let Some(prev_node) = self.arena.get_mut(prev_id)
        {
            prev_node.next_sibling = next;
        }

        if let Some(next_id) = next
            && let Some(next_node) = self.arena.get_mut(next_id)
        {
            next_node.prev_sibling = prev;
        }

        if let Some(n) = self.arena.get_mut(node) {
            n.parent = None;
            n.prev_sibling = None;
            n.next_sibling = None;
        }

        if let Some(p) = parent {
            self.dirty_nodes.insert(p);
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Detaches a node from its parent and recycles the arena slots for this node
    /// and all its descendants back into the arena free-list.
    pub fn remove_child_and_recycle(&mut self, node: NodeId) {
        self.detach(node);
        self.recycle_subtree(node);
    }

    /// Recursively frees a node and all of its descendants, recycling their arena slots.
    pub fn recycle_subtree(&mut self, root: NodeId) {
        let mut to_free = Vec::new();
        let mut stack = vec![root];
        while let Some(curr) = stack.pop() {
            to_free.push(curr);
            let mut next_child = self.arena.get(curr).and_then(|n| n.first_child);
            while let Some(c) = next_child {
                stack.push(c);
                next_child = self.arena.get(c).and_then(|n| n.next_sibling);
            }
        }
        for nid in to_free {
            self.dirty_nodes.remove(&nid);
            self.shadow_hosts.remove(&nid);
            self.shadow_hosts.retain(|_, &mut host| host != nid);
            self.arena.free_raw(nid.raw());
        }
    }
}
