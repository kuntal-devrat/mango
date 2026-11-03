//! Stylesheet rule ingestion and bucketing for RuleIndex.

use std::sync::Arc;

use crate::cascade::media::matches_media_query_env;
use crate::cascade::types::{IndexedContainerRule, IndexedRule, KeySelector, Origin, ScopeDef};
use crate::parser::{Rule, StyleRule, Stylesheet};

use super::index::RuleIndex;
use super::keys::extract_subject_keys_from_is_where;

impl<'a> RuleIndex<'a> {
    /// Ingests all style rules, media queries, layers, and @property definitions from a parsed stylesheet.
    pub fn add_stylesheet(&mut self, sheet: &'a Stylesheet, origin: Origin, order: &mut usize) {
        let scoping_element = sheet.scoping_element;
        for rule in &sheet.rules {
            match rule {
                Rule::Style(style_rule) => {
                    if let Some(scop_id) = scoping_element {
                        let scope_def = ScopeDef {
                            root: None,
                            limit: None,
                            scoping_element: Some(scop_id),
                        };
                        let scope_chain: Arc<[ScopeDef<'a>]> = Arc::from([scope_def]);
                        self.add_style_rule_with_layer_and_scope(
                            style_rule,
                            origin,
                            order,
                            Arc::from([]),
                            scope_chain,
                        );
                    } else {
                        self.add_style_rule(style_rule, origin, order);
                    }
                }
                Rule::Media(media_rule) => {
                    if matches_media_query_env(
                        &media_rule.query,
                        self.viewport_width,
                        self.viewport_height,
                        &self.env,
                    ) {
                        for style_rule in &media_rule.rules {
                            if let Some(scop_id) = scoping_element {
                                let scope_def = ScopeDef {
                                    root: None,
                                    limit: None,
                                    scoping_element: Some(scop_id),
                                };
                                let scope_chain: Arc<[ScopeDef<'a>]> = Arc::from([scope_def]);
                                self.add_style_rule_with_layer_and_scope(
                                    style_rule,
                                    origin,
                                    order,
                                    Arc::from([]),
                                    scope_chain,
                                );
                            } else {
                                self.add_style_rule(style_rule, origin, order);
                            }
                        }
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
                    self.add_property_rule(prop_rule, Arc::from([]), *order);
                }
                Rule::Layer(layer_rule) => {
                    let (layer_path, names) =
                        self.resolve_layer_path(layer_rule.name.as_deref(), &[]);
                    let initial_scope: Arc<[ScopeDef<'a>]> = if let Some(scop_id) = scoping_element
                    {
                        Arc::from([ScopeDef {
                            root: None,
                            limit: None,
                            scoping_element: Some(scop_id),
                        }])
                    } else {
                        Arc::from([])
                    };
                    self.add_layer_rules(
                        &layer_rule.rules,
                        layer_path,
                        &names,
                        origin,
                        order,
                        initial_scope,
                    );
                }
                Rule::Scope(scope_rule) => {
                    self.add_scope_rules(
                        scope_rule,
                        origin,
                        order,
                        Arc::from([]),
                        &[],
                        scoping_element,
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

    pub(crate) fn resolve_layer_path(
        &mut self,
        declared_name: Option<&str>,
        parent_names: &[String],
    ) -> (Arc<[u32]>, Vec<String>) {
        let (name_owned, is_anon) = match declared_name {
            Some(n) => (n.to_string(), false),
            None => {
                self.anon_layer_counter += 1;
                (format!("__anon_{}", self.anon_layer_counter), true)
            }
        };

        let mut parts = parent_names.to_vec();
        if !is_anon {
            for subpart in name_owned.split('.') {
                let trimmed = subpart.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
        } else {
            parts.push(name_owned);
        }

        let str_refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        let path = self.layer_scope.resolve_path(&str_refs);
        (Arc::from(path.as_slice()), parts)
    }

    pub(crate) fn add_layer_rules(
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

    pub(crate) fn add_style_rule(
        &mut self,
        style_rule: &'a StyleRule,
        origin: Origin,
        order: &mut usize,
    ) {
        self.add_style_rule_with_layer_and_scope(
            style_rule,
            origin,
            order,
            Arc::from([]),
            Arc::from([]),
        );
    }

    pub(crate) fn add_scope_rules(
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

    pub(crate) fn add_style_rule_with_layer_and_scope(
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
}
