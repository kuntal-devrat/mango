//! WHATWG HTML5 active formatting elements and Adoption Agency Algorithm (§13.2.4.3 / §13.2.6.4.7).

use super::types::{FormattingEntry, TreeBuilder};
use crate::dom::{NodeData, NodeId};
use crate::elements::is_special_element;

impl<'a> TreeBuilder<'a> {
    pub(crate) fn reconstruct_active_formatting_elements(&mut self) {
        if self.active_formatting_elements.is_empty() {
            return;
        }

        // 1. If the last entry is a marker or already in open_elements, return.
        match self.active_formatting_elements.last().unwrap() {
            FormattingEntry::Marker => return,
            FormattingEntry::Element { node_id, .. } => {
                if self.open_elements.contains(node_id) {
                    return;
                }
            }
        }

        // 2. Find the last entry before the end that is either a marker or in open_elements.
        let mut start_idx = self.active_formatting_elements.len() - 1;
        while start_idx > 0 {
            let prev = &self.active_formatting_elements[start_idx - 1];
            match prev {
                FormattingEntry::Marker => break,
                FormattingEntry::Element { node_id, .. } => {
                    if self.open_elements.contains(node_id) {
                        break;
                    }
                }
            }
            start_idx -= 1;
        }

        // 3. Reconstruct forwards from start_idx
        for i in start_idx..self.active_formatting_elements.len() {
            let (tag_name, attributes) = match &self.active_formatting_elements[i] {
                FormattingEntry::Marker => continue,
                FormattingEntry::Element {
                    tag_name,
                    attributes,
                    ..
                } => (tag_name.clone(), attributes.clone()),
            };

            let parent = self.current_node();
            let new_child = self.doc.create_element(&tag_name, attributes.clone());
            self.doc.append_child(parent, new_child);
            self.open_elements.push(new_child);

            self.active_formatting_elements[i] = FormattingEntry::Element {
                tag_name,
                attributes,
                node_id: new_child,
            };
        }
    }

    /// Pushes an element to the active formatting elements list, enforcing WHATWG "Noah's Ark" (rule of three).
    pub(crate) fn push_active_formatting_element(
        &mut self,
        tag_name: &str,
        attributes: &[(String, String)],
        node_id: NodeId,
    ) {
        let mut count = 0;
        let mut earliest_idx = None;
        for (i, entry) in self.active_formatting_elements.iter().enumerate().rev() {
            match entry {
                FormattingEntry::Marker => break,
                FormattingEntry::Element {
                    tag_name: t,
                    attributes: a,
                    ..
                } => {
                    if t.eq_ignore_ascii_case(tag_name) && a == attributes {
                        count += 1;
                        earliest_idx = Some(i);
                    }
                }
            }
        }

        if count >= 3
            && let Some(idx) = earliest_idx
        {
            self.active_formatting_elements.remove(idx);
        }

        self.active_formatting_elements
            .push(FormattingEntry::Element {
                tag_name: tag_name.to_string(),
                attributes: attributes.to_vec(),
                node_id,
            });
    }

    /// Clears active formatting elements up to the last marker (or until empty if no marker).
    pub(crate) fn clear_active_formatting_to_marker(&mut self) {
        while let Some(entry) = self.active_formatting_elements.pop() {
            if matches!(entry, FormattingEntry::Marker) {
                break;
            }
        }
    }

    /// Executes the WHATWG Adoption Agency Algorithm (AAA) for formatting end tags.
    pub(crate) fn handle_formatting_end_tag(&mut self, subject: &str) {
        // If the current node is the target tag and not in active formatting elements, pop and return.
        if let Some(curr_tag) = self.current_tag_name()
            && curr_tag.eq_ignore_ascii_case(subject)
        {
            let curr_id = self.current_node();
            let in_active = self.active_formatting_elements.iter().any(|entry| {
                    matches!(entry, FormattingEntry::Element { node_id, .. } if *node_id == curr_id)
                });
            if !in_active {
                self.open_elements.pop();
                return;
            }
        }

        // Outer loop: up to 8 iterations per WHATWG HTML5 §13.2.6.4.7
        for _ in 0..8 {
            // Step 1: Find formatting_element: the last element in active_formatting_elements
            // between the end and the last marker that has tag name == subject.
            let fmt_pos = self
                .active_formatting_elements
                .iter()
                .rposition(|entry| match entry {
                    FormattingEntry::Marker => false,
                    FormattingEntry::Element { tag_name, .. } => {
                        tag_name.eq_ignore_ascii_case(subject)
                    }
                });

            let fmt_idx = match fmt_pos {
                Some(idx) => {
                    let has_marker_after = self.active_formatting_elements[idx..]
                        .iter()
                        .any(|e| matches!(e, FormattingEntry::Marker));
                    if has_marker_after {
                        return;
                    }
                    idx
                }
                None => {
                    if self.is_in_scope(subject) {
                        self.pop_until(subject);
                    }
                    return;
                }
            };

            let (fmt_tag, fmt_attrs, fmt_node_id) = match &self.active_formatting_elements[fmt_idx]
            {
                FormattingEntry::Element {
                    tag_name,
                    attributes,
                    node_id,
                } => (tag_name.clone(), attributes.clone(), *node_id),
                FormattingEntry::Marker => return,
            };

            // Step 2: Check if formatting_element is in open_elements
            let stack_pos = self.open_elements.iter().rposition(|&id| id == fmt_node_id);
            let stack_idx = match stack_pos {
                Some(idx) => idx,
                None => {
                    self.active_formatting_elements.remove(fmt_idx);
                    return;
                }
            };

            // Step 3: Check if in scope
            if !self.is_in_scope(&fmt_tag) {
                return;
            }

            // Step 4: Find furthest block among open elements after fmt_node_id
            // Per spec this is the LAST special element, not the first.
            let mut furthest_block = None;
            for &id in &self.open_elements[stack_idx + 1..] {
                if let Some(node) = self.doc.get(id)
                    && let NodeData::Element(el) = &node.data
                    && is_special_element(&el.tag_name)
                {
                    furthest_block = Some(id);
                }
            }

            // Step 5: If no furthest block, pop up to fmt_node_id and return
            if furthest_block.is_none() {
                while let Some(top) = self.open_elements.pop() {
                    if top == fmt_node_id {
                        break;
                    }
                }
                if fmt_idx < self.active_formatting_elements.len() {
                    self.active_formatting_elements.remove(fmt_idx);
                }
                return;
            }

            // Step 6: There IS a furthest block
            let fb_id = furthest_block.unwrap();
            let common_ancestor = if stack_idx > 0 {
                self.open_elements[stack_idx - 1]
            } else {
                self.doc.root()
            };

            let mut node;
            let mut last_node = fb_id;

            let fb_stack_idx = self
                .open_elements
                .iter()
                .rposition(|&id| id == fb_id)
                .unwrap();
            let mut curr_stack_idx = fb_stack_idx;

            for _ in 0..3 {
                if curr_stack_idx == 0 {
                    break;
                }
                curr_stack_idx -= 1;
                node = self.open_elements[curr_stack_idx];

                if node == fmt_node_id {
                    break;
                }

                let active_idx = self.active_formatting_elements.iter().position(
                    |e| matches!(e, FormattingEntry::Element { node_id, .. } if *node_id == node),
                );

                let active_idx = match active_idx {
                    Some(idx) => idx,
                    None => {
                        self.open_elements.retain(|&id| id != node);
                        continue;
                    }
                };

                let (node_tag, node_attrs) = match &self.active_formatting_elements[active_idx] {
                    FormattingEntry::Element {
                        tag_name,
                        attributes,
                        ..
                    } => (tag_name.clone(), attributes.clone()),
                    FormattingEntry::Marker => continue,
                };

                let clone_id = self.doc.create_element(&node_tag, node_attrs.clone());
                self.active_formatting_elements[active_idx] = FormattingEntry::Element {
                    tag_name: node_tag,
                    attributes: node_attrs,
                    node_id: clone_id,
                };
                self.open_elements[curr_stack_idx] = clone_id;

                self.doc.append_child(clone_id, last_node);
                last_node = clone_id;
            }

            // Insert last_node into common_ancestor after fmt_node_id
            let _ = self
                .doc
                .insert_after(common_ancestor, last_node, fmt_node_id);

            // Create new element for formatting_element
            let new_fmt_elem = self.doc.create_element(&fmt_tag, fmt_attrs);

            // Move all existing children of furthest_block into new_fmt_elem
            let fb_children: Vec<NodeId> = self.doc.children(fb_id).map(|c| c.id).collect();
            for child_id in fb_children {
                self.doc.append_child(new_fmt_elem, child_id);
            }
            self.doc.append_child(fb_id, new_fmt_elem);

            // Remove old formatting_element from active_formatting_elements and open_elements
            self.active_formatting_elements.retain(|e| {
                !matches!(e, FormattingEntry::Element { node_id, .. } if *node_id == fmt_node_id)
            });
            self.open_elements.retain(|&id| id != fmt_node_id);
        }
    }
}
