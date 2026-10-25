//! Rule index structure and rule addition methods.

use std::collections::HashMap;
use std::sync::Arc;

use mango_html::dom::{Document, ElementData, NodeId};

use crate::cascade::media::{MediaEnvironment, current_media_environment, matches_media_query_env};
use crate::cascade::types::{
    IndexedContainerRule, IndexedRule, KeySelector, MatchedDeclaration, Origin, PseudoRuleBucket,
    RegisteredKeyframes, RegisteredProperty, ScopeDef, compare_layer_paths,
};
use crate::parser::{KeyframesRule, PropertyRule, Rule, StyleRule, Stylesheet};

use super::keys::extract_subject_keys_from_is_where;

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

    pub fn add_stylesheet(&mut self, sheet: &'a Stylesheet, origin: Origin, order: &mut usize) {
        for rule in &sheet.rules {
            match rule {
                Rule::Style(style_rule) => {
                    self.add_style_rule(style_rule, origin, order);
                }
                Rule::Media(media_rule)
                    if matches_media_query_env(
                        &media_rule.query,
                        self.viewport_width,
                        self.viewport_height,
                        &self.env,
                    ) =>
                {
                    for style_rule in &media_rule.rules {
                        self.add_style_rule(style_rule, origin, order);
                    }
                }
                Rule::Container(container_rule) => {
                    for style_rule in &container_rule.rules {
                        *order += 1;
                        self.container_rules.push(IndexedContainerRule {
                            name: container_rule.name.clone(),
                            query: container_rule.query.clone(),
                            style_rule,
                            origin,
                            source_order: *order,
                        });
                    }
                }
                Rule::Layer(layer_rule) => {
                    let (layer_path, names) =
                        self.resolve_layer_path(layer_rule.name.as_deref(), &[]);
                    self.add_layer_rules(
                        &layer_rule.rules,
                        layer_path,
                        &names,
                        origin,
                        order,
                        Arc::from([]),
                    );
                }
                Rule::Property(prop_rule) => {
                    *order += 1;
                    self.add_property_rule(prop_rule, Arc::from([]), *order);
                }
                Rule::Scope(scope_rule) => {
                    self.add_scope_rules(
                        scope_rule,
                        origin,
                        order,
                        Arc::from([]),
                        &[],
                        sheet.scoping_element,
                    );
                }
                Rule::Keyframes(kf) => {
                    *order += 1;
                    self.add_keyframes_rule(kf, Arc::from([]), *order);
                }
                Rule::PositionTry(pos_try) => {
                    self.position_try_rules
                        .insert(pos_try.name.clone(), pos_try.declarations.clone());
                }
                Rule::ViewTransition(vt) => {
                    self.view_transition = Some(vt.clone());
                }
                _ => {}
            }
        }
    }

    fn resolve_layer_path(
        &mut self,
        name: Option<&str>,
        parent_names: &[String],
    ) -> (Arc<[u32]>, Vec<String>) {
        let mut full_names: Vec<String> = parent_names.to_vec();
        if let Some(n) = name {
            for part in n.split('.') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    full_names.push(trimmed.to_string());
                }
            }
        } else {
            self.anon_layer_counter += 1;
            full_names.push(format!("$anon_{}", self.anon_layer_counter));
        }

        let parts: Vec<&str> = full_names.iter().map(|s| s.as_str()).collect();
        let path = self.layer_scope.resolve_path(&parts);
        (Arc::from(path), full_names)
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

    fn add_layer_rules(
        &mut self,
        rules: &'a [Rule],
        layer_path: Arc<[u32]>,
        parent_names: &[String],
        origin: Origin,
        order: &mut usize,
        scope_chain: Arc<[ScopeDef<'a>]>,
    ) {
        for rule in rules {
            match rule {
                Rule::Style(style_rule) => {
                    self.add_style_rule_with_layer_and_scope(
                        style_rule,
                        origin,
                        order,
                        layer_path.clone(),
                        scope_chain.clone(),
                    );
                }
                Rule::Media(media_rule)
                    if matches_media_query_env(
                        &media_rule.query,
                        self.viewport_width,
                        self.viewport_height,
                        &self.env,
                    ) =>
                {
                    for style_rule in &media_rule.rules {
                        self.add_style_rule_with_layer_and_scope(
                            style_rule,
                            origin,
                            order,
                            layer_path.clone(),
                            scope_chain.clone(),
                        );
                    }
                }
                Rule::Container(container_rule) => {
                    for style_rule in &container_rule.rules {
                        *order += 1;
                        self.container_rules.push(IndexedContainerRule {
                            name: container_rule.name.clone(),
                            query: container_rule.query.clone(),
                            style_rule,
                            origin,
                            source_order: *order,
                        });
                    }
                }
                Rule::Property(prop_rule) => {
                    *order += 1;
                    self.add_property_rule(prop_rule, layer_path.clone(), *order);
                }
                Rule::Layer(nested_layer_rule) => {
                    let (nested_path, nested_names) =
                        self.resolve_layer_path(nested_layer_rule.name.as_deref(), parent_names);
                    self.add_layer_rules(
                        &nested_layer_rule.rules,
                        nested_path,
                        &nested_names,
                        origin,
                        order,
                        scope_chain.clone(),
                    );
                }
                Rule::Scope(scope_rule) => {
                    let parent_scopes: Vec<ScopeDef<'a>> = scope_chain.iter().cloned().collect();
                    let parent_scoping = parent_scopes.last().and_then(|s| s.scoping_element);
                    self.add_scope_rules(
                        scope_rule,
                        origin,
                        order,
                        layer_path.clone(),
                        &parent_scopes,
                        parent_scoping,
                    );
                }
                Rule::Keyframes(kf) => {
                    *order += 1;
                    self.add_keyframes_rule(kf, layer_path.clone(), *order);
                }
                Rule::PositionTry(pos_try) => {
                    self.position_try_rules
                        .insert(pos_try.name.clone(), pos_try.declarations.clone());
                }
                Rule::ViewTransition(vt) => {
                    self.view_transition = Some(vt.clone());
                }
                _ => {}
            }
        }
    }

    fn add_style_rule(&mut self, style_rule: &'a StyleRule, origin: Origin, order: &mut usize) {
        self.add_style_rule_with_layer_and_scope(
            style_rule,
            origin,
            order,
            Arc::from([]),
            Arc::from([]),
        );
    }

    fn add_scope_rules(
        &mut self,
        scope_rule: &'a crate::parser::ScopeRule,
        origin: Origin,
        order: &mut usize,
        layer_path: Arc<[u32]>,
        parent_scopes: &[ScopeDef<'a>],
        scoping_element: Option<mango_html::dom::NodeId>,
    ) {
        let mut current_scopes = parent_scopes.to_vec();
        current_scopes.push(ScopeDef {
            root: scope_rule.root.as_ref(),
            limit: scope_rule.limit.as_ref(),
            scoping_element,
        });
        let scope_chain: Arc<[ScopeDef<'a>]> = Arc::from(current_scopes.as_slice());

        for rule in &scope_rule.rules {
            match rule {
                Rule::Style(style_rule) => {
                    self.add_style_rule_with_layer_and_scope(
                        style_rule,
                        origin,
                        order,
                        layer_path.clone(),
                        scope_chain.clone(),
                    );
                }
                Rule::Layer(layer_rule) => {
                    let (new_layer_path, names) =
                        self.resolve_layer_path(layer_rule.name.as_deref(), &[]);
                    self.add_layer_rules(
                        &layer_rule.rules,
                        new_layer_path,
                        &names,
                        origin,
                        order,
                        scope_chain.clone(),
                    );
                }
                Rule::Media(media_rule)
                    if matches_media_query_env(
                        &media_rule.query,
                        self.viewport_width,
                        self.viewport_height,
                        &self.env,
                    ) =>
                {
                    for style_rule in &media_rule.rules {
                        self.add_style_rule_with_layer_and_scope(
                            style_rule,
                            origin,
                            order,
                            layer_path.clone(),
                            scope_chain.clone(),
                        );
                    }
                }
                Rule::Scope(nested_scope) => {
                    self.add_scope_rules(
                        nested_scope,
                        origin,
                        order,
                        layer_path.clone(),
                        &current_scopes,
                        scoping_element,
                    );
                }
                _ => {}
            }
        }
    }

    #[allow(dead_code)]
    fn add_style_rule_with_layer(
        &mut self,
        style_rule: &'a StyleRule,
        origin: Origin,
        order: &mut usize,
        layer_path: Arc<[u32]>,
    ) {
        self.add_style_rule_with_layer_and_scope(
            style_rule,
            origin,
            order,
            layer_path,
            Arc::from([]),
        );
    }

    fn add_style_rule_with_layer_and_scope(
        &mut self,
        style_rule: &'a StyleRule,
        origin: Origin,
        order: &mut usize,
        layer_path: Arc<[u32]>,
        scope_chain: Arc<[ScopeDef<'a>]>,
    ) {
        *order += 1;
        let rule_order = *order;

        for complex_sel in &style_rule.selectors.selectors {
            let subject = if let Some((_, compound)) = complex_sel.tail.last() {
                compound
            } else {
                &complex_sel.head
            };

            let indexed = IndexedRule {
                style_rule,
                origin,
                source_order: rule_order,
                layer_path: layer_path.clone(),
                scope_chain: scope_chain.clone(),
            };

            // 1. Check if subject targets a pseudo-element
            let mut pseudo_target = None;
            for s in &subject.simple_selectors {
                if let crate::selectors::SimpleSelector::PseudoElement(p) = s {
                    let plower = p.to_ascii_lowercase();
                    if !plower.starts_with("slotted") && !plower.starts_with("part") {
                        pseudo_target = Some(plower);
                        break;
                    }
                }
            }

            if let Some(mut pseudo) = pseudo_target {
                if pseudo == "-webkit-input-placeholder"
                    || pseudo == "-moz-placeholder"
                    || pseudo == "-ms-input-placeholder"
                {
                    pseudo = "placeholder".to_string();
                } else if pseudo == "-moz-selection" {
                    pseudo = "selection".to_string();
                }
                let bucket = self.pseudo_rules.entry(pseudo).or_default();
                let mut key_id: Option<&str> = None;
                let mut key_class: Option<&str> = None;
                let mut key_tag: Option<&str> = None;
                let mut key_attr: Option<&str> = None;
                let mut is_root = false;
                let mut is_where_keys = None;

                for s in &subject.simple_selectors {
                    match s {
                        crate::selectors::SimpleSelector::Id(id) if key_id.is_none() => {
                            key_id = Some(id.as_str());
                        }
                        crate::selectors::SimpleSelector::Class(cls) if key_class.is_none() => {
                            key_class = Some(cls.as_str());
                        }
                        crate::selectors::SimpleSelector::Type(tag) if key_tag.is_none() => {
                            key_tag = Some(tag.as_str());
                        }
                        crate::selectors::SimpleSelector::Attribute { name, .. }
                            if key_attr.is_none() =>
                        {
                            key_attr = Some(name.as_str());
                        }
                        crate::selectors::SimpleSelector::PseudoClass(p) => {
                            if p.eq_ignore_ascii_case("root") {
                                is_root = true;
                            } else if (p.starts_with("is(") || p.starts_with("where("))
                                && is_where_keys.is_none()
                            {
                                is_where_keys = extract_subject_keys_from_is_where(p);
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(id) = key_id {
                    bucket
                        .id_rules
                        .entry(id.to_string())
                        .or_default()
                        .push(indexed);
                } else if let Some(cls) = key_class {
                    bucket
                        .class_rules
                        .entry(cls.to_string())
                        .or_default()
                        .push(indexed);
                } else if let Some(tag) = key_tag {
                    bucket
                        .tag_rules
                        .entry(tag.to_ascii_lowercase())
                        .or_default()
                        .push(indexed);
                } else if let Some(attr) = key_attr {
                    bucket
                        .attr_rules
                        .entry(attr.to_ascii_lowercase())
                        .or_default()
                        .push(indexed);
                } else if is_root {
                    bucket.root_rules.push(indexed);
                } else if let Some(keys) = is_where_keys {
                    for k in keys {
                        match k {
                            KeySelector::Id(id) => {
                                bucket.id_rules.entry(id).or_default().push(indexed.clone());
                            }
                            KeySelector::Class(cls) => {
                                bucket
                                    .class_rules
                                    .entry(cls)
                                    .or_default()
                                    .push(indexed.clone());
                            }
                            KeySelector::Tag(tag) => {
                                bucket
                                    .tag_rules
                                    .entry(tag)
                                    .or_default()
                                    .push(indexed.clone());
                            }
                            KeySelector::Attr(attr) => {
                                bucket
                                    .attr_rules
                                    .entry(attr)
                                    .or_default()
                                    .push(indexed.clone());
                            }
                            KeySelector::Root => bucket.root_rules.push(indexed.clone()),
                        }
                    }
                } else {
                    bucket.universal_rules.push(indexed);
                }
                continue;
            }

            let has_part = subject.simple_selectors.iter().any(|s| match s {
                crate::selectors::SimpleSelector::PseudoElement(p) => {
                    p.to_ascii_lowercase().starts_with("part(")
                }
                _ => false,
            });

            if has_part {
                self.universal_rules.push(indexed);
                continue;
            }

            // 2. Classify key selector: ID > Class > Tag > Attr > Root > :is/:where > Universal
            let mut key_id: Option<&str> = None;
            let mut key_class: Option<&str> = None;
            let mut key_tag: Option<&str> = None;
            let mut key_attr: Option<&str> = None;
            let mut is_root = false;
            let mut is_where_keys = None;

            for s in &subject.simple_selectors {
                match s {
                    crate::selectors::SimpleSelector::Id(id) if key_id.is_none() => {
                        key_id = Some(id.as_str());
                    }
                    crate::selectors::SimpleSelector::Class(cls) if key_class.is_none() => {
                        key_class = Some(cls.as_str());
                    }
                    crate::selectors::SimpleSelector::Type(tag) if key_tag.is_none() => {
                        key_tag = Some(tag.as_str());
                    }
                    crate::selectors::SimpleSelector::Attribute { name, .. }
                        if key_attr.is_none() =>
                    {
                        key_attr = Some(name.as_str());
                    }
                    crate::selectors::SimpleSelector::PseudoClass(p) => {
                        if p.eq_ignore_ascii_case("root") {
                            is_root = true;
                        } else if (p.starts_with("is(") || p.starts_with("where("))
                            && is_where_keys.is_none()
                        {
                            is_where_keys = extract_subject_keys_from_is_where(p);
                        }
                    }
                    _ => {}
                }
            }

            if let Some(id) = key_id {
                self.id_rules
                    .entry(id.to_string())
                    .or_default()
                    .push(indexed);
            } else if let Some(cls) = key_class {
                self.class_rules
                    .entry(cls.to_string())
                    .or_default()
                    .push(indexed);
            } else if let Some(tag) = key_tag {
                self.tag_rules
                    .entry(tag.to_ascii_lowercase())
                    .or_default()
                    .push(indexed);
            } else if let Some(attr) = key_attr {
                self.attr_rules
                    .entry(attr.to_ascii_lowercase())
                    .or_default()
                    .push(indexed);
            } else if is_root {
                self.root_rules.push(indexed);
            } else if let Some(keys) = is_where_keys {
                for k in keys {
                    match k {
                        KeySelector::Id(id) => {
                            self.id_rules.entry(id).or_default().push(indexed.clone());
                        }
                        KeySelector::Class(cls) => {
                            self.class_rules
                                .entry(cls)
                                .or_default()
                                .push(indexed.clone());
                        }
                        KeySelector::Tag(tag) => {
                            self.tag_rules.entry(tag).or_default().push(indexed.clone());
                        }
                        KeySelector::Attr(attr) => {
                            self.attr_rules
                                .entry(attr)
                                .or_default()
                                .push(indexed.clone());
                        }
                        KeySelector::Root => self.root_rules.push(indexed.clone()),
                    }
                }
            } else {
                self.universal_rules.push(indexed);
            }
        }

        // Track rules that define container properties for fast O(1) container lookups
        let declares_container = style_rule.declarations.iter().any(|d| {
            let n = d.name.to_ascii_lowercase();
            n == "container-type" || n == "container" || n == "contain" || n == "container-name"
        });
        if declares_container {
            self.container_def_rules.push(IndexedRule {
                style_rule,
                origin,
                source_order: rule_order,
                layer_path: layer_path.clone(),
                scope_chain: scope_chain.clone(),
            });
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
