use std::collections::HashSet;
use std::sync::Arc;

use mango_css::computed::{ComputedStyle, compute_style_with_index};
use mango_css::parser::Stylesheet;
use mango_css::values::Display;
use mango_css::{Origin, RuleIndex};
use mango_html::dom::{Document, NodeData, NodeId};

use super::extract::extract_style_elements;
use super::node::StyledNode;

/// Incremental style invalidator tracking dirty DOM nodes.
///
/// Instead of recalculating style for all $N$ nodes in the document on every change,
/// `StyleInvalidator` tracks which DOM nodes mutated (via attribute changes or class toggles)
/// and recalculates only those subtrees in $O(\text{changed\_nodes})$.
#[derive(Debug, Clone, Default)]
pub struct StyleInvalidator {
    dirty_nodes: HashSet<NodeId>,
}

impl StyleInvalidator {
    /// Creates a new empty `StyleInvalidator`.
    pub fn new() -> Self {
        Self {
            dirty_nodes: HashSet::new(),
        }
    }

    /// Marks a specific DOM node as dirty.
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        self.dirty_nodes.insert(node_id);
    }

    /// Marks a collection of DOM nodes as dirty.
    pub fn mark_all_dirty(&mut self, nodes: impl IntoIterator<Item = NodeId>) {
        self.dirty_nodes.extend(nodes);
    }

    /// Checks if a node is marked dirty.
    pub fn is_dirty(&self, node_id: NodeId) -> bool {
        self.dirty_nodes.contains(&node_id)
    }

    /// Clears all dirty flags.
    pub fn clear(&mut self) {
        self.dirty_nodes.clear();
    }

    /// Returns the number of currently tracked dirty nodes.
    pub fn len(&self) -> usize {
        self.dirty_nodes.len()
    }

    /// Returns `true` if there are no dirty nodes.
    pub fn is_empty(&self) -> bool {
        self.dirty_nodes.is_empty()
    }

    /// Updates only the dirty nodes in an existing style tree in-place.
    ///
    /// Returns the number of nodes whose styles were recalculated.
    pub fn invalidate_and_update(
        &mut self,
        root: &mut StyledNode,
        doc: &Document,
        author_styles: &[&Stylesheet],
    ) -> usize {
        if self.dirty_nodes.is_empty() {
            return 0;
        }

        let embedded_sheets = extract_style_elements(doc);
        let mut all_author_sheets: Vec<&Stylesheet> = Vec::new();
        all_author_sheets.extend(author_styles);
        for sheet in &embedded_sheets {
            all_author_sheets.push(sheet);
        }

        let mut order = 1000;
        let author_index =
            RuleIndex::from_stylesheets(&all_author_sheets, Origin::Author, &mut order);

        let recalculated =
            update_styled_subtree(root, doc, &self.dirty_nodes, &author_index, None, false);

        self.clear();
        recalculated
    }
}

pub fn has_inherited_property_changed(old: &ComputedStyle, new: &ComputedStyle) -> bool {
    old.color != new.color
        || old.font_size != new.font_size
        || old.font_family != new.font_family
        || old.font_weight != new.font_weight
        || old.font_style != new.font_style
        || old.line_height != new.line_height
        || old.text_align != new.text_align
        || old.text_transform != new.text_transform
        || old.letter_spacing != new.letter_spacing
        || old.word_spacing != new.word_spacing
        || old.text_indent != new.text_indent
        || old.white_space != new.white_space
        || old.visibility != new.visibility
        || old.direction != new.direction
        || old.writing_mode != new.writing_mode
        || old.cursor != new.cursor
        || old.word_break != new.word_break
        || old.overflow_wrap != new.overflow_wrap
        || old.hyphens != new.hyphens
        || old.list_style_type != new.list_style_type
        || old.list_style_position != new.list_style_position
        || old.border_collapse != new.border_collapse
        || old.border_spacing != new.border_spacing
}

pub fn update_styled_subtree(
    node: &mut StyledNode,
    doc: &Document,
    dirty_nodes: &HashSet<NodeId>,
    author_index: &RuleIndex,
    parent_style: Option<&ComputedStyle>,
    parent_inherited_changed: bool,
) -> usize {
    let mut recalculated_count = 0;
    let mut this_node_changed_inherited = false;

    let is_directly_dirty = node.node_id.is_some_and(|nid| dirty_nodes.contains(&nid));
    let needs_recalc = is_directly_dirty || parent_inherited_changed;

    if needs_recalc
        && let Some(nid) = node.node_id
        && let Some(dom_node) = doc.get(nid)
    {
        match &dom_node.data {
            NodeData::Element(elem) => {
                let old_style = node.style.clone();
                node.style = Arc::new(compute_style_with_index(
                    nid,
                    doc,
                    author_index,
                    parent_style,
                ));
                node.attributes = elem
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.to_lowercase(), v.clone()))
                    .collect();

                // Check if any inherited property changed
                if has_inherited_property_changed(&old_style, &node.style) {
                    this_node_changed_inherited = true;
                }
                recalculated_count += 1;
            }
            NodeData::Text(_) => {
                if let Some(ps) = parent_style {
                    let mut style = ps.clone();
                    style.display = Display::Inline;
                    style.background_image = None;
                    style.background_color = mango_core::Color::TRANSPARENT;
                    style.box_shadow = None;
                    style.border_top_width = 0.0;
                    style.border_right_width = 0.0;
                    style.border_bottom_width = 0.0;
                    style.border_left_width = 0.0;
                    node.style = Arc::new(style);
                    recalculated_count += 1;
                }
            }
            _ => {}
        }
    }

    let propagate_to_children = this_node_changed_inherited;
    let current_style = node.style.clone();

    for child in &mut node.children {
        recalculated_count += update_styled_subtree(
            child,
            doc,
            dirty_nodes,
            author_index,
            Some(&current_style),
            propagate_to_children,
        );
    }

    recalculated_count
}
