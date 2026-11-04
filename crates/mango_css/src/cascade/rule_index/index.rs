//! Rule index structure and rule addition methods.

use std::collections::HashMap;
use std::sync::Arc;

use mango_html::dom::{Document, ElementData, NodeId};

use crate::cascade::media::{MediaEnvironment, current_media_environment};
use crate::cascade::types::{
    IndexedContainerRule, IndexedRule, MatchedDeclaration, Origin, PseudoRuleBucket,
    RegisteredKeyframes, RegisteredProperty, compare_layer_paths,
};
use crate::parser::{KeyframesRule, PropertyRule, Stylesheet};

/// A hierarchical scope for CSS cascade layer ordering (CSS Cascade Level 5 § 6.4).
#[derive(Debug, Clone, Default)]
pub struct LayerScope {
    pub count: u32,
    pub sublayers: HashMap<String, LayerScopeEntry>,
}

#[derive(Debug, Clone)]
pub struct LayerScopeEntry {
    pub order: u32,
    pub children: LayerScope,
}

impl LayerScope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves or declares a hierarchical layer path like `["A", "B"]` or `["A"]`.
    pub fn resolve_path(&mut self, parts: &[&str]) -> Vec<u32> {
        let mut path = Vec::with_capacity(parts.len());
        let mut current_scope = self;
        for &part in parts {
            let next_order = if let Some(entry) = current_scope.sublayers.get(part) {
                entry.order
            } else {
                current_scope.count += 1;
                let order = current_scope.count;
                current_scope.sublayers.insert(
                    part.to_string(),
                    LayerScopeEntry {
                        order,
                        children: LayerScope::default(),
                    },
                );
                order
            };
            path.push(next_order);
            if let Some(entry) = current_scope.sublayers.get_mut(part) {
                current_scope = &mut entry.children;
            } else {
                break;
            }
        }
        path
    }
}

/// An index of CSS rules grouped by the key selector (ID, class, tag, attribute, universal, pseudo)
/// to reduce cascade resolution complexity from O(Rules) to O(1) candidate lookup per element.
#[derive(Debug, Clone)]
pub struct RuleIndex<'a> {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub layer_count: u32,
    pub layer_map: HashMap<String, u32>,
    pub layer_scope: LayerScope,
    pub anon_layer_counter: usize,
    pub property_rules: HashMap<String, RegisteredProperty>,
    pub keyframe_rules: HashMap<String, RegisteredKeyframes>,
    pub(crate) id_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub(crate) class_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub(crate) tag_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub(crate) attr_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub(crate) root_rules: Vec<IndexedRule<'a>>,
    pub(crate) universal_rules: Vec<IndexedRule<'a>>,
    pub(crate) pseudo_rules: HashMap<String, PseudoRuleBucket<'a>>,
    pub container_rules: Vec<IndexedContainerRule<'a>>,
    pub container_def_rules: Vec<IndexedRule<'a>>,
    pub env: MediaEnvironment,
    pub position_try_rules: HashMap<String, Vec<crate::properties::Declaration>>,
    pub view_transition: Option<crate::parser::ViewTransitionRule>,
}

impl<'a> Default for RuleIndex<'a> {
    fn default() -> Self {
        Self::new_with_size(1280.0, 900.0)
    }
}

impl<'a> RuleIndex<'a> {
    /// Number of indexed candidate entries, including bucket aliases.
    /// Useful for profiling stylesheet fan-out without exposing internals.
    pub fn indexed_entry_count(&self) -> usize {
        self.id_rules.values().map(Vec::len).sum::<usize>()
            + self.class_rules.values().map(Vec::len).sum::<usize>()
            + self.tag_rules.values().map(Vec::len).sum::<usize>()
            + self.attr_rules.values().map(Vec::len).sum::<usize>()
            + self.root_rules.len()
            + self.universal_rules.len()
            + self
                .pseudo_rules
                .values()
                .map(|b| b.total_rules())
                .sum::<usize>()
            + self.container_def_rules.len()
    }

    pub fn universal_rule_count(&self) -> usize {
        self.universal_rules.len()
    }

    pub fn has_id_rule(&self, id: &str) -> bool {
        self.id_rules.contains_key(id)
    }

    pub fn has_attr_rule(&self, attr: &str) -> bool {
        let lower = attr.to_ascii_lowercase();
        self.attr_rules.contains_key(&lower)
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_size(viewport_width: f32, viewport_height: f32) -> Self {
        let mut env = current_media_environment();
        env.viewport_width = viewport_width;
        env.viewport_height = viewport_height;
        Self::new_with_env(env)
    }

    pub fn new_with_env(env: MediaEnvironment) -> Self {
        Self {
            viewport_width: env.viewport_width,
            viewport_height: env.viewport_height,
            layer_count: 0,
            layer_map: HashMap::new(),
            layer_scope: LayerScope::default(),
            anon_layer_counter: 0,
            property_rules: HashMap::new(),
            keyframe_rules: HashMap::new(),
            id_rules: HashMap::new(),
            class_rules: HashMap::new(),
            tag_rules: HashMap::new(),
            attr_rules: HashMap::new(),
            root_rules: Vec::new(),
            universal_rules: Vec::new(),
            pseudo_rules: HashMap::new(),
            container_rules: Vec::new(),
            container_def_rules: Vec::new(),
            env,
            position_try_rules: HashMap::new(),
            view_transition: None,
        }
    }

    pub fn from_stylesheets(
        stylesheets: &[&'a Stylesheet],
        origin: Origin,
        order: &mut usize,
    ) -> Self {
        Self::from_stylesheets_with_size(stylesheets, origin, order, 1280.0, 900.0)
    }

    pub fn from_stylesheets_with_size(
        stylesheets: &[&'a Stylesheet],
        origin: Origin,
        order: &mut usize,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Self {
        let mut index = Self::new_with_size(viewport_width, viewport_height);
        for sheet in stylesheets {
            index.add_stylesheet(sheet, origin, order);
        }
        index
    }

    pub fn from_stylesheets_with_env(
        stylesheets: &[&'a Stylesheet],
        origin: Origin,
        order: &mut usize,
        env: MediaEnvironment,
    ) -> Self {
        let mut index = Self::new_with_env(env);
        for sheet in stylesheets {
            index.add_stylesheet(sheet, origin, order);
        }
        index
    }

    pub fn add_property_rule(
        &mut self,
        rule: &'a PropertyRule,
        layer_path: Arc<[u32]>,
        order: usize,
    ) {
        if let Some(existing) = self.property_rules.get(&rule.name) {
            let layer_cmp = compare_layer_paths(&layer_path, &existing.layer_path);
            if layer_cmp == std::cmp::Ordering::Greater
                || (layer_cmp == std::cmp::Ordering::Equal && order >= existing.source_order)
            {
                self.property_rules.insert(
                    rule.name.clone(),
                    RegisteredProperty {
                        name: rule.name.clone(),
                        syntax: rule.syntax.clone(),
                        inherits: rule.inherits,
                        initial_value: rule.initial_value.clone(),
                        layer_path,
                        source_order: order,
                    },
                );
            }
        } else {
            self.property_rules.insert(
                rule.name.clone(),
                RegisteredProperty {
                    name: rule.name.clone(),
                    syntax: rule.syntax.clone(),
                    inherits: rule.inherits,
                    initial_value: rule.initial_value.clone(),
                    layer_path,
                    source_order: order,
                },
            );
        }
    }

    pub fn add_keyframes_rule(
        &mut self,
        rule: &'a KeyframesRule,
        layer_path: Arc<[u32]>,
        order: usize,
    ) {
        if let Some(existing) = self.keyframe_rules.get(&rule.name) {
            let layer_cmp = compare_layer_paths(&layer_path, &existing.layer_path);
            if layer_cmp == std::cmp::Ordering::Greater
                || (layer_cmp == std::cmp::Ordering::Equal && order >= existing.source_order)
            {
                self.keyframe_rules.insert(
                    rule.name.clone(),
                    RegisteredKeyframes {
                        name: rule.name.clone(),
                        keyframes: rule.clone(),
                        layer_path,
                        source_order: order,
                    },
                );
            }
        } else {
            self.keyframe_rules.insert(
                rule.name.clone(),
                RegisteredKeyframes {
                    name: rule.name.clone(),
                    keyframes: rule.clone(),
                    layer_path,
                    source_order: order,
                },
            );
        }
    }

    /// Collects candidate style rules that could match the given element node.
    pub fn collect_matches_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        elem: &ElementData,
        source_order: &mut usize,
        out: &mut Vec<MatchedDeclaration>,
    ) {
        super::matching::collect_matches_for_node(self, node_id, doc, elem, source_order, out);
    }

    /// Collects candidate ::part() rules from an outer document stylesheet that match the target node.
    pub fn collect_part_matches_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        source_order: &mut usize,
        out: &mut Vec<MatchedDeclaration>,
    ) {
        super::matching::collect_part_matches_for_node(self, node_id, doc, source_order, out);
    }

    /// Collects candidate rules for a pseudo-element on the target node using indexed buckets.
    pub fn collect_pseudo_matches_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        pseudo: &str,
        source_order: &mut usize,
        out: &mut Vec<MatchedDeclaration>,
    ) {
        super::matching::collect_pseudo_matches_for_node(
            self,
            node_id,
            doc,
            pseudo,
            source_order,
            out,
        );
    }

    /// Returns true if this index contains any candidate rules for the given pseudo-element.
    pub fn has_pseudo_rules(&self, pseudo: &str) -> bool {
        let key = pseudo.to_ascii_lowercase();
        self.pseudo_rules
            .get(&key)
            .is_some_and(|b| b.total_rules() > 0)
    }

    /// Locates the appropriate ancestor container element for `node_id`, matching `name_filter` if given,
    /// and returns its resolved (width, height) in pixels.
    pub fn find_container_size_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        name_filter: Option<&str>,
    ) -> Option<(f32, f32)> {
        super::matching::find_container_size_for_node(self, node_id, doc, name_filter)
    }
}
