use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;

use crate::box_tree::LayoutBox;
use mango_core::Rect;
use mango_html::dom::{Document, NodeData, NodeId};

use super::name::{compute_accessible_name, compute_description};
use super::role::{A11yRole, resolve_role};
use super::state::{A11yState, CheckedState, PressedState, resolve_state};

/// An individual node in the accessibility tree.
#[derive(Debug, Clone)]
pub struct A11yNode {
    pub id: usize,
    pub node_id: Option<NodeId>,
    pub role: A11yRole,
    pub name: String,
    pub description: Option<String>,
    pub state: A11yState,
    pub bounds: Rect,
    pub children: Vec<A11yNode>,
}

/// Complete accessibility tree for a document.
#[derive(Debug, Clone)]
pub struct A11yTree {
    pub root: Option<A11yNode>,
}

impl A11yTree {
    /// Builds an accessibility tree from the DOM document and the computed layout tree.
    ///
    /// Pre-builds a bounds cache from the layout tree so that per-node lookups
    /// are O(1) instead of O(n).
    pub fn build(doc: &Document, layout_root: Option<&LayoutBox>) -> Self {
        let mut next_id = 1;

        let mut bounds_cache = HashMap::new();
        if let Some(root) = layout_root {
            build_bounds_cache(root, &mut bounds_cache);
        }

        let root = build_node(doc, doc.root(), layout_root, &bounds_cache, &mut next_id);
        A11yTree { root }
    }

    /// Finds all nodes matching a specific role.
    pub fn find_by_role(&self, role: &A11yRole) -> Vec<&A11yNode> {
        let mut results = Vec::new();
        if let Some(ref r) = self.root {
            collect_by_role(r, role, &mut results);
        }
        results
    }

    /// Finds all nodes whose accessible name contains the given substring (case-insensitive).
    pub fn find_by_name(&self, name_substr: &str) -> Vec<&A11yNode> {
        let mut results = Vec::new();
        let lower = name_substr.to_ascii_lowercase();
        if let Some(ref r) = self.root {
            collect_by_name(r, &lower, &mut results);
        }
        results
    }

    /// Finds a node by its DOM node ID.
    pub fn find_by_node_id(&self, target: NodeId) -> Option<&A11yNode> {
        self.root
            .as_ref()
            .and_then(|r| find_by_node_id_rec(r, target))
    }

    /// Produces a formatted text dump of the accessibility tree for debugging and testing.
    pub fn dump_tree(&self) -> String {
        let mut out = String::new();
        if let Some(ref r) = self.root {
            dump_node(r, 0, &mut out);
        }
        out
    }
}

pub fn find_by_node_id_rec(node: &A11yNode, target: NodeId) -> Option<&A11yNode> {
    if node.node_id == Some(target) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_by_node_id_rec(child, target) {
            return Some(found);
        }
    }
    None
}

pub fn collect_by_role<'a>(
    node: &'a A11yNode,
    target_role: &A11yRole,
    acc: &mut Vec<&'a A11yNode>,
) {
    if &node.role == target_role {
        acc.push(node);
    }
    for child in &node.children {
        collect_by_role(child, target_role, acc);
    }
}

pub fn collect_by_name<'a>(node: &'a A11yNode, target_lower: &str, acc: &mut Vec<&'a A11yNode>) {
    if node.name.to_ascii_lowercase().contains(target_lower) {
        acc.push(node);
    }
    for child in &node.children {
        collect_by_name(child, target_lower, acc);
    }
}

pub fn dump_node(node: &A11yNode, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let role_str = match &node.role {
        A11yRole::Heading { level } => format!("Heading (level {level})"),
        other => format!("{other:?}"),
    };

    let name_part = if node.name.is_empty() {
        String::new()
    } else {
        format!(" \"{}\"", node.name)
    };

    let mut state_parts = Vec::new();
    if node.state.disabled {
        state_parts.push("disabled".to_string());
    }
    if node.state.busy {
        state_parts.push("busy".to_string());
    }
    if node.state.modal {
        state_parts.push("modal".to_string());
    }
    if let Some(ref c) = node.state.checked {
        state_parts.push(match c {
            CheckedState::True => "checked".to_string(),
            CheckedState::False => "unchecked".to_string(),
            CheckedState::Mixed => "mixed".to_string(),
        });
    }
    if let Some(ref p) = node.state.pressed {
        state_parts.push(match p {
            PressedState::True => "pressed".to_string(),
            PressedState::False => "not pressed".to_string(),
            PressedState::Mixed => "partially pressed".to_string(),
        });
    }
    if let Some(e) = node.state.expanded {
        state_parts.push(if e {
            "expanded".to_string()
        } else {
            "collapsed".to_string()
        });
    }
    if let Some(ref v) = node.state.value {
        state_parts.push(format!("value={v}"));
    }
    if let Some(ref live) = node.state.live {
        state_parts.push(format!("live={live}"));
    }
    let state_str = if state_parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", state_parts.join(", "))
    };

    let _ = writeln!(out, "{indent}[{role_str}]{name_part}{state_str}");
    for child in &node.children {
        dump_node(child, depth + 1, out);
    }
}

/// Pre-builds a HashMap<NodeId, Rect> from the layout tree for O(1) bounds lookups.
pub fn build_bounds_cache(layout_box: &LayoutBox, cache: &mut HashMap<NodeId, Rect>) {
    if let Some(node_id) = layout_box.node_id {
        cache.insert(node_id, layout_box.dimensions.border_box());
    }
    for child in &layout_box.children {
        build_bounds_cache(child, cache);
    }
}

/// Recursively builds an `A11yNode` for a DOM node.
pub fn build_node(
    doc: &Document,
    node_id: NodeId,
    layout_root: Option<&LayoutBox>,
    bounds_cache: &HashMap<NodeId, Rect>,
    next_id: &mut usize,
) -> Option<A11yNode> {
    let dom_node = doc.get(node_id)?;

    match &dom_node.data {
        NodeData::Document => {
            let mut children = Vec::new();
            for child in doc.children(node_id) {
                if let Some(child_a11y) =
                    build_node(doc, child.id, layout_root, bounds_cache, next_id)
                {
                    children.push(child_a11y);
                }
            }
            let id = *next_id;
            *next_id += 1;
            let title = doc
                .find_element_by_tag(doc.root(), "title")
                .map(|t| doc.text_content(t).trim().to_string())
                .unwrap_or_default();

            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role: A11yRole::RootWebArea,
                name: title,
                description: None,
                state: A11yState::default(),
                bounds: layout_root
                    .map(|b| b.dimensions.border_box())
                    .unwrap_or(Rect::ZERO),
                children,
            })
        }
        NodeData::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            let id = *next_id;
            *next_id += 1;
            let bounds = bounds_cache.get(&node_id).copied().unwrap_or(Rect::ZERO);
            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role: A11yRole::StaticText,
                name: trimmed.to_string(),
                description: None,
                state: A11yState::default(),
                bounds,
                children: Vec::new(),
            })
        }
        NodeData::Element(elem) => {
            // Check aria-hidden="true" or hidden attribute
            if elem.get_attribute("aria-hidden") == Some("true")
                || elem.get_attribute("hidden").is_some()
            {
                return None;
            }

            // Suppress non-rendered tags
            let tag = elem.tag_name.to_ascii_lowercase();
            if matches!(
                tag.as_str(),
                "head" | "script" | "style" | "meta" | "link" | "template" | "noscript"
            ) {
                return None;
            }

            // <input type="hidden"> is excluded from the a11y tree
            if tag == "input"
                && elem
                    .get_attribute("type")
                    .map(|t| t.eq_ignore_ascii_case("hidden"))
                    .unwrap_or(false)
            {
                return None;
            }

            // If a layout tree is available and this element has no box,
            // it's display:none — exclude it from the a11y tree.
            if layout_root.is_some() && !bounds_cache.contains_key(&node_id) {
                return None;
            }

            let role = resolve_role(&tag, elem, doc, node_id);
            let state = resolve_state(elem, &tag);
            let name = compute_accessible_name(doc, node_id, elem, &tag, &mut HashSet::new());
            let bounds = bounds_cache.get(&node_id).copied().unwrap_or(Rect::ZERO);

            // role="presentation"/"none" — flatten children into parent
            if role == A11yRole::Presentation {
                let mut children = Vec::new();
                for child in doc.children(node_id) {
                    if let Some(child_a11y) =
                        build_node(doc, child.id, layout_root, bounds_cache, next_id)
                    {
                        children.push(child_a11y);
                    }
                }
                if children.is_empty() {
                    return None;
                }
                let id = *next_id;
                *next_id += 1;
                return Some(A11yNode {
                    id,
                    node_id: Some(node_id),
                    role: A11yRole::Presentation,
                    name: String::new(),
                    description: None,
                    state: A11yState::default(),
                    bounds,
                    children,
                });
            }

            let mut children = Vec::new();
            for child in doc.children(node_id) {
                if let Some(child_a11y) =
                    build_node(doc, child.id, layout_root, bounds_cache, next_id)
                {
                    children.push(child_a11y);
                }
            }

            let id = *next_id;
            *next_id += 1;

            let description = compute_description(doc, elem);

            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role,
                name,
                description,
                state,
                bounds,
                children,
            })
        }
        _ => None,
    }
}
