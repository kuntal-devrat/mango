//! Rule index structure and rule addition methods.

use std::collections::HashMap;

use mango_html::dom::{Document, ElementData, NodeId};

use crate::cascade::media::{MediaEnvironment, current_media_environment, matches_media_query_env};
use crate::cascade::types::{
    IndexedContainerRule, IndexedRule, KeySelector, MatchedDeclaration, Origin, PseudoRuleBucket,
};
use crate::parser::{Rule, StyleRule, Stylesheet};

use super::keys::extract_subject_keys_from_is_where;

/// An index of CSS rules grouped by the key selector (ID, class, tag, attribute, universal, pseudo)
/// to reduce cascade resolution complexity from O(Rules) to O(1) candidate lookup per element.
#[derive(Debug, Clone)]
pub struct RuleIndex<'a> {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub layer_count: u32,
    pub layer_map: HashMap<String, u32>,
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
                    let layer_id = if let Some(ref name) = layer_rule.name {
                        if let Some(&existing_id) = self.layer_map.get(name) {
                            existing_id
                        } else {
                            self.layer_count += 1;
                            self.layer_map.insert(name.clone(), self.layer_count);
                            self.layer_count
                        }
                    } else {
                        self.layer_count += 1;
                        self.layer_count
                    };
                    self.add_layer_rules(&layer_rule.rules, layer_id, origin, order);
                }
                Rule::Scope(scope_rule) => {
                    for style_rule in &scope_rule.rules {
                        self.add_style_rule_with_layer_and_scope(
                            style_rule,
                            origin,
                            order,
                            None,
                            scope_rule.root.as_ref(),
                            scope_rule.limit.as_ref(),
                        );
                    }
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

    fn add_layer_rules(
        &mut self,
        rules: &'a [Rule],
        layer_id: u32,
        origin: Origin,
        order: &mut usize,
    ) {
        for rule in rules {
            match rule {
                Rule::Style(style_rule) => {
                    self.add_style_rule_with_layer_and_scope(
                        style_rule,
                        origin,
                        order,
                        Some(layer_id),
                        None,
                        None,
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
                            Some(layer_id),
                            None,
                            None,
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
                Rule::Layer(nested_layer_rule) => {
                    let nested_layer_id = if let Some(ref name) = nested_layer_rule.name {
                        if let Some(&existing_id) = self.layer_map.get(name) {
                            existing_id
                        } else {
                            self.layer_count += 1;
                            self.layer_map.insert(name.clone(), self.layer_count);
                            self.layer_count
                        }
                    } else {
                        self.layer_count += 1;
                        self.layer_count
                    };
                    self.add_layer_rules(&nested_layer_rule.rules, nested_layer_id, origin, order);
                }
                Rule::Scope(scope_rule) => {
                    for style_rule in &scope_rule.rules {
                        self.add_style_rule_with_layer_and_scope(
                            style_rule,
                            origin,
                            order,
                            Some(layer_id),
                            scope_rule.root.as_ref(),
                            scope_rule.limit.as_ref(),
                        );
                    }
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
        self.add_style_rule_with_layer_and_scope(style_rule, origin, order, None, None, None);
    }

    #[allow(dead_code)]
    fn add_style_rule_with_layer(
        &mut self,
        style_rule: &'a StyleRule,
        origin: Origin,
        order: &mut usize,
        layer_order: Option<u32>,
    ) {
        self.add_style_rule_with_layer_and_scope(
            style_rule,
            origin,
            order,
            layer_order,
            None,
            None,
        );
    }

    fn add_style_rule_with_layer_and_scope(
        &mut self,
        style_rule: &'a StyleRule,
        origin: Origin,
        order: &mut usize,
        layer_order: Option<u32>,
        scope_root: Option<&'a crate::selectors::SelectorList>,
        scope_limit: Option<&'a crate::selectors::SelectorList>,
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
                layer_order,
                scope_root,
                scope_limit,
            };

            // 1. Check if subject targets a pseudo-element
            let mut pseudo_target = None;
            for s in &subject.simple_selectors {
                if let crate::selectors::SimpleSelector::PseudoElement(p) = s {
                    let plower = p.to_ascii_lowercase();
                    if !plower.starts_with("slotted") {
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
                let mut key_id = None;
                let mut key_class = None;
                let mut key_tag = None;
                let mut key_attr = None;
                let mut is_root = false;
                let mut is_where_keys = None;

                for s in &subject.simple_selectors {
                    match s {
                        crate::selectors::SimpleSelector::Id(id) if key_id.is_none() => {
                            key_id = Some(id.clone());
                        }
                        crate::selectors::SimpleSelector::Class(cls) if key_class.is_none() => {
                            key_class = Some(cls.clone());
                        }
                        crate::selectors::SimpleSelector::Type(tag) if key_tag.is_none() => {
                            key_tag = Some(tag.to_ascii_lowercase());
                        }
                        crate::selectors::SimpleSelector::Attribute { name, .. }
                            if key_attr.is_none() =>
                        {
                            key_attr = Some(name.to_ascii_lowercase());
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
                    bucket.id_rules.entry(id).or_default().push(indexed);
                } else if let Some(cls) = key_class {
                    bucket.class_rules.entry(cls).or_default().push(indexed);
                } else if let Some(tag) = key_tag {
                    bucket.tag_rules.entry(tag).or_default().push(indexed);
                } else if let Some(attr) = key_attr {
                    bucket.attr_rules.entry(attr).or_default().push(indexed);
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

            // 2. Classify key selector: ID > Class > Tag > Attr > Root > :is/:where > Universal
            let mut key_id = None;
            let mut key_class = None;
            let mut key_tag = None;
            let mut key_attr = None;
            let mut is_root = false;
            let mut is_where_keys = None;

            for s in &subject.simple_selectors {
                match s {
                    crate::selectors::SimpleSelector::Id(id) if key_id.is_none() => {
                        key_id = Some(id.clone());
                    }
                    crate::selectors::SimpleSelector::Class(cls) if key_class.is_none() => {
                        key_class = Some(cls.clone());
                    }
                    crate::selectors::SimpleSelector::Type(tag) if key_tag.is_none() => {
                        key_tag = Some(tag.to_ascii_lowercase());
                    }
                    crate::selectors::SimpleSelector::Attribute { name, .. }
                        if key_attr.is_none() =>
                    {
                        key_attr = Some(name.to_ascii_lowercase());
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
                self.id_rules.entry(id).or_default().push(indexed);
            } else if let Some(cls) = key_class {
                self.class_rules.entry(cls).or_default().push(indexed);
            } else if let Some(tag) = key_tag {
                self.tag_rules.entry(tag).or_default().push(indexed);
            } else if let Some(attr) = key_attr {
                self.attr_rules.entry(attr).or_default().push(indexed);
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
                layer_order,
                scope_root,
                scope_limit,
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
