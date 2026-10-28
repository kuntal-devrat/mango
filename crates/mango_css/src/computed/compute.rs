//! High-level style computation orchestrators.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mango_html::dom::{Document, NodeId};

use super::apply::apply_cascaded_properties;
use super::custom_props::{resolve_value_vars, resolve_value_vars_with_local, value_has_vars};
use super::style::ComputedStyle;
use crate::parser::Stylesheet;
use crate::values::{Display, Value};

/// Computes the final style for a DOM node using a pre-indexed author stylesheet rule index and optional shadow index.
pub fn compute_style_with_shadow_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    shadow_index: Option<&crate::cascade::RuleIndex>,
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    compute_style_with_shadow_and_part_index(
        node_id,
        doc,
        author_index,
        shadow_index,
        None,
        parent_style,
    )
}

pub fn compute_style_with_shadow_and_part_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    shadow_index: Option<&crate::cascade::RuleIndex>,
    part_index: Option<&crate::cascade::RuleIndex>,
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    let part_indexes = match part_index {
        Some(pi) => vec![pi],
        None => Vec::new(),
    };
    compute_style_with_shadow_and_part_indexes(
        node_id,
        doc,
        author_index,
        shadow_index,
        &part_indexes,
        parent_style,
    )
}

pub fn compute_style_with_shadow_and_part_indexes(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    shadow_index: Option<&crate::cascade::RuleIndex>,
    part_indexes: &[&crate::cascade::RuleIndex],
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    let (cascaded, important_props) =
        crate::cascade::resolve_cascade_with_shadow_and_part_indexes_and_parent_details(
            node_id,
            doc,
            author_index,
            shadow_index,
            part_indexes,
            parent_style.map(|p| p.writing_mode),
            parent_style.map(|p| p.direction),
        );
    let mut style = ComputedStyle::default();

    // ── 1. Inherited properties: start with parent values if available ──
    if let Some(parent) = parent_style {
        style.root_font_size = parent.root_font_size;
        // Custom properties are inherited by reference. Cloning this map for
        // every descendant turns a few thousand root variables into gigabytes
        // on real pages (GitHub ships ~1,900 variables in one stylesheet).
        // Copy-on-write below preserves CSS semantics when a node overrides a
        // variable, while the common case remains one shared immutable map.
        style.custom_properties = Arc::clone(&parent.custom_properties);
        // Inherit any inheritable custom properties from parent.local_custom_properties
        if !parent.local_custom_properties.is_empty() {
            for (prop, val) in &parent.local_custom_properties {
                let non_inherited = author_index
                    .property_rules
                    .get(prop)
                    .or_else(|| shadow_index.and_then(|shadow| shadow.property_rules.get(prop)))
                    .is_some_and(|reg| !reg.inherits);
                if !non_inherited {
                    let cp = Arc::make_mut(&mut style.custom_properties);
                    cp.insert(prop.clone(), val.clone());
                }
            }
        }
        style.color = parent.color;
        style.font_size = parent.font_size;
        style.font_weight = parent.font_weight;
        style.font_style = parent.font_style;
        style.font_family = parent.font_family.clone();
        style.line_height = parent.line_height;
        style.text_align = parent.text_align;
        style.text_transform = parent.text_transform;
        style.letter_spacing = parent.letter_spacing;
        style.word_spacing = parent.word_spacing;
        style.text_indent = parent.text_indent;
        style.direction = parent.direction;
        style.visibility = parent.visibility;
        style.white_space = parent.white_space;
        style.list_style_type = parent.list_style_type;
        style.list_style_position = parent.list_style_position;
        style.cursor = parent.cursor;
        style.border_collapse = parent.border_collapse;
        style.border_spacing = parent.border_spacing;
        style.caption_side = parent.caption_side;
        style.writing_mode = parent.writing_mode;
        style.scrollbar_width = parent.scrollbar_width;
        style.scrollbar_color = parent.scrollbar_color;
        style.word_break = parent.word_break;
        style.overflow_wrap = parent.overflow_wrap;
        style.hyphens = parent.hyphens;
        style.text_emphasis_style = parent.text_emphasis_style.clone();
        style.text_emphasis_color = parent.text_emphasis_color;
        style.font_feature_settings = parent.font_feature_settings.clone();
        style.font_variation_settings = parent.font_variation_settings.clone();
        style.accent_color = parent.accent_color;
        style.quotes = parent.quotes.clone();
        style.text_wrap = parent.text_wrap;
    }

    // Collect custom properties defined directly on this element
    let mut custom_props = Vec::new();
    for (prop, val) in &cascaded {
        if prop.starts_with("--") {
            let non_inherited = author_index
                .property_rules
                .get(prop)
                .or_else(|| shadow_index.and_then(|shadow| shadow.property_rules.get(prop)))
                .is_some_and(|reg| !reg.inherits);
            if non_inherited {
                style
                    .local_custom_properties
                    .insert(prop.clone(), val.clone());
            } else {
                custom_props.push((prop, val));
            }
        }
    }
    if !custom_props.is_empty() {
        let cp = Arc::make_mut(&mut style.custom_properties);
        for (prop, val) in custom_props {
            cp.insert(prop.clone(), val.clone());
        }
    }

    // Apply registered @property initial-values for custom properties not explicitly set
    for (name, reg) in &author_index.property_rules {
        if !reg.inherits {
            if !style.local_custom_properties.contains_key(name)
                && !style.custom_properties.contains_key(name)
            {
                if let Some(ref init_val) = reg.initial_value {
                    style
                        .local_custom_properties
                        .insert(name.clone(), init_val.clone());
                }
            }
        } else if !style.custom_properties.contains_key(name) {
            if let Some(ref init_val) = reg.initial_value {
                let cp = Arc::make_mut(&mut style.custom_properties);
                cp.insert(name.clone(), init_val.clone());
            }
        }
    }
    if let Some(shadow) = shadow_index {
        for (name, reg) in &shadow.property_rules {
            if !reg.inherits {
                if !style.local_custom_properties.contains_key(name)
                    && !style.custom_properties.contains_key(name)
                {
                    if let Some(ref init_val) = reg.initial_value {
                        style
                            .local_custom_properties
                            .insert(name.clone(), init_val.clone());
                    }
                }
            } else if !style.custom_properties.contains_key(name) {
                if let Some(ref init_val) = reg.initial_value {
                    let cp = Arc::make_mut(&mut style.custom_properties);
                    cp.insert(name.clone(), init_val.clone());
                }
            }
        }
    }

    // Pre-resolve custom property var(--*) references in cascaded declarations.
    let mut resolved_cascaded: HashMap<String, Value> = HashMap::with_capacity(cascaded.len());
    let mut visited = HashSet::new();
    for (prop, val) in &cascaded {
        if prop.starts_with("--") {
            resolved_cascaded.insert(prop.clone(), val.clone());
            continue;
        }
        if !value_has_vars(val) {
            resolved_cascaded.insert(prop.clone(), val.clone());
        } else {
            visited.clear();
            let resolved = if style.local_custom_properties.is_empty() {
                resolve_value_vars(val, &style.custom_properties, &mut visited)
            } else {
                resolve_value_vars_with_local(
                    val,
                    &style.local_custom_properties,
                    &style.custom_properties,
                    &mut visited,
                )
            };
            if let Some(resolved_val) = resolved {
                resolved_cascaded.insert(prop.clone(), resolved_val);
            } else {
                // CSS Custom Properties Level 1 § 3.2: If a custom property cannot be resolved
                // (e.g. references an unset variable without a fallback, or has a cycle),
                // the declaration is invalid at computed-value time, which resolves as 'unset'.
                resolved_cascaded.insert(prop.clone(), Value::Keyword("unset".to_string()));
            }
        }
    }

    expand_deferred_shorthands(&mut resolved_cascaded);

    apply_cascaded_properties(&mut style, &resolved_cascaded, parent_style);

    // Apply keyframe animation styles
    // Per CSS Cascade 4 § 3, animation declarations are overridden by !important author declarations.
    let animations = style.animations.clone();
    let mut animated_custom_props = false;
    for anim in &animations {
        let anim_name = &anim.name;
        if anim_name.is_empty() {
            continue;
        }
        let progress = if anim.duration_ms > 0.0 && anim.delay_ms < 0.0 {
            (-anim.delay_ms / anim.duration_ms).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let decls_opt = if let Some(kf_reg) = author_index.keyframe_rules.get(anim_name) {
            let kf = crate::animation::Keyframes::from_rule(&kf_reg.keyframes);
            Some(kf.sample_with_underlying(progress, Some(&resolved_cascaded)))
        } else if let Some(shadow) = shadow_index {
            shadow.keyframe_rules.get(anim_name).map(|kf_reg| {
                let kf = crate::animation::Keyframes::from_rule(&kf_reg.keyframes);
                kf.sample_with_underlying(progress, Some(&resolved_cascaded))
            })
        } else {
            None
        };

        if let Some(mut decls) = decls_opt {
            decls.retain(|prop, _| !important_props.contains(prop));
            for (k, v) in &decls {
                if k.starts_with("--") {
                    animated_custom_props = true;
                    style.local_custom_properties.insert(k.clone(), v.clone());
                    let cp = std::sync::Arc::make_mut(&mut style.custom_properties);
                    cp.insert(k.clone(), v.clone());
                }
            }
            apply_cascaded_properties(&mut style, &decls, parent_style);
        }
    }

    if animated_custom_props {
        let mut re_resolved = HashMap::new();
        for (prop, val) in &cascaded {
            if !prop.starts_with("--") && value_has_vars(val) {
                visited.clear();
                let resolved = if style.local_custom_properties.is_empty() {
                    resolve_value_vars(val, &style.custom_properties, &mut visited)
                } else {
                    resolve_value_vars_with_local(
                        val,
                        &style.local_custom_properties,
                        &style.custom_properties,
                        &mut visited,
                    )
                };
                if let Some(res) = resolved {
                    re_resolved.insert(prop.clone(), res);
                }
            }
        }
        if !re_resolved.is_empty() {
            expand_deferred_shorthands(&mut re_resolved);
            apply_cascaded_properties(&mut style, &re_resolved, parent_style);
        }
    }

    // CSS Overflow 3 §3.3:
    // If one of overflow-x or overflow-y is visible and the other is not visible,
    // then visible becomes auto.
    if style.overflow_x == crate::values::Overflow::Visible
        && style.overflow_y != crate::values::Overflow::Visible
    {
        style.overflow_x = crate::values::Overflow::Auto;
    } else if style.overflow_y == crate::values::Overflow::Visible
        && style.overflow_x != crate::values::Overflow::Visible
    {
        style.overflow_y = crate::values::Overflow::Auto;
    }

    if parent_style.is_none() {
        style.root_font_size = style.font_size;
    }
    style
}

/// Computes the final style for a DOM node using a pre-indexed author stylesheet rule index.
pub fn compute_style_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    compute_style_with_shadow_index(node_id, doc, author_index, None, parent_style)
}

/// Computes the style for a pseudo-element (`before` or `after`) of an element using a pre-indexed author stylesheet rule index and optional shadow index.
pub fn compute_pseudo_style_with_shadow_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    shadow_index: Option<&crate::cascade::RuleIndex>,
    host_style: &ComputedStyle,
    pseudo: &str,
) -> Option<ComputedStyle> {
    if !author_index.has_pseudo_rules(pseudo)
        && shadow_index.is_none_or(|si| !si.has_pseudo_rules(pseudo))
    {
        return None;
    }

    let cascaded = crate::cascade::resolve_pseudo_element_cascade_with_shadow_index(
        node_id,
        doc,
        author_index,
        shadow_index,
        pseudo,
    );
    if cascaded.is_empty() {
        return None;
    }

    let mut style = ComputedStyle {
        color: host_style.color,
        font_size: host_style.font_size,
        root_font_size: host_style.root_font_size,
        font_weight: host_style.font_weight,
        font_style: host_style.font_style,
        font_family: host_style.font_family.clone(),
        line_height: host_style.line_height,
        text_align: host_style.text_align,
        text_transform: host_style.text_transform,
        letter_spacing: host_style.letter_spacing,
        word_spacing: host_style.word_spacing,
        text_indent: host_style.text_indent,
        visibility: host_style.visibility,
        white_space: host_style.white_space,
        cursor: host_style.cursor,
        word_break: host_style.word_break,
        overflow_wrap: host_style.overflow_wrap,
        hyphens: host_style.hyphens,
        text_emphasis_style: host_style.text_emphasis_style.clone(),
        text_emphasis_color: host_style.text_emphasis_color,
        font_feature_settings: host_style.font_feature_settings.clone(),
        font_variation_settings: host_style.font_variation_settings.clone(),
        accent_color: host_style.accent_color,
        quotes: host_style.quotes.clone(),
        custom_properties: Arc::clone(&host_style.custom_properties),
        display: Display::Inline,
        ..Default::default()
    };

    let mut resolved_cascaded: HashMap<String, Value> = HashMap::with_capacity(cascaded.len());
    let mut visited = HashSet::new();
    for (prop, val) in &cascaded {
        if prop.starts_with("--") {
            resolved_cascaded.insert(prop.clone(), val.clone());
            continue;
        }
        if !value_has_vars(val) {
            resolved_cascaded.insert(prop.clone(), val.clone());
        } else {
            visited.clear();
            let resolved_val = resolve_value_vars(val, &style.custom_properties, &mut visited)
                .unwrap_or_else(|| val.clone());
            resolved_cascaded.insert(prop.clone(), resolved_val);
        }
    }

    expand_deferred_shorthands(&mut resolved_cascaded);

    apply_cascaded_properties(&mut style, &resolved_cascaded, Some(host_style));
    Some(style)
}

/// Computes the style for a pseudo-element (`before` or `after`) of an element using a pre-indexed author stylesheet rule index.
pub fn compute_pseudo_style_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &crate::cascade::RuleIndex,
    host_style: &ComputedStyle,
    pseudo: &str,
) -> Option<ComputedStyle> {
    compute_pseudo_style_with_shadow_index(node_id, doc, author_index, None, host_style, pseudo)
}

/// Computes the final style for a DOM node given the document and author stylesheets.
pub fn compute_style(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
    parent_style: Option<&ComputedStyle>,
) -> ComputedStyle {
    let mut order = 1000;
    let author_index = crate::cascade::RuleIndex::from_stylesheets(
        author_stylesheets,
        crate::cascade::Origin::Author,
        &mut order,
    );
    let computed_parent;
    let resolved_parent = if parent_style.is_none() {
        if let Some(parent_id) = doc.get(node_id).and_then(|n| n.parent) {
            if doc
                .get(parent_id)
                .map(|n| matches!(n.data, mango_html::dom::NodeData::Element(_)))
                .unwrap_or(false)
            {
                computed_parent = Some(compute_style(parent_id, doc, author_stylesheets, None));
                computed_parent.as_ref()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        parent_style
    };
    compute_style_with_index(node_id, doc, &author_index, resolved_parent)
}

/// Computes the style for a pseudo-element (`before` or `after`) of an element.
pub fn compute_pseudo_style(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
    host_style: &ComputedStyle,
    pseudo: &str,
) -> Option<ComputedStyle> {
    let mut order = 1000;
    let author_index = crate::cascade::RuleIndex::from_stylesheets(
        author_stylesheets,
        crate::cascade::Origin::Author,
        &mut order,
    );
    compute_pseudo_style_with_index(node_id, doc, &author_index, host_style, pseudo)
}

fn expand_deferred_shorthands(resolved_cascaded: &mut HashMap<String, Value>) {
    let shorthand_props: Vec<(String, Value)> = resolved_cascaded
        .iter()
        .filter(|(prop, _)| crate::properties::Declaration::is_shorthand_name(prop))
        .map(|(prop, val)| (prop.clone(), val.clone()))
        .collect();
    for (prop, val) in shorthand_props {
        let decl = crate::properties::Declaration::new(prop, val, false);
        for expanded in decl.expand_shorthand() {
            resolved_cascaded.insert(expanded.name, expanded.value);
        }
    }
}
