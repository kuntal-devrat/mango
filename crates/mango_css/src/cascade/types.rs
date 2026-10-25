//! Core cascade types: origins, matched declarations, and indexed rule representations.

use std::collections::HashMap;
use std::sync::Arc;

use crate::parser::StyleRule;
use crate::properties::Declaration;
use crate::specificity::Specificity;

/// Style origin for cascade ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    UserAgent = 0,
    PresentationalHint = 1,
    Shadow = 2,
    Author = 3,
    Inline = 4,
}

/// Compares two hierarchical layer paths according to CSS Cascade Level 5 § 6.4.
///
/// An empty path `&[]` represents an unlayered style.
/// For normal declarations, unlayered styles override layered styles at each level,
/// and later declared layers override earlier declared layers.
pub fn compare_layer_paths(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    let min_len = a.len().min(b.len());
    for i in 0..min_len {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    // All common prefix components are equal.
    // If lengths differ, the shorter path is unlayered at this depth.
    // Per CSS Cascade 5 § 6.4, unlayered styles take precedence over
    // layered styles at the same nesting level (for normal declarations).
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Greater,
        std::cmp::Ordering::Greater => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => std::cmp::Ordering::Equal,
    }
}

/// A property declaration that matched a target DOM node during cascade evaluation.
#[derive(Debug, Clone)]
pub struct MatchedDeclaration {
    pub declaration: Declaration,
    pub specificity: Specificity,
    pub origin: Origin,
    pub source_order: usize,
    pub layer_path: Arc<[u32]>,
    pub rule_id: usize,
    pub scope_proximity: Option<u32>,
}

impl MatchedDeclaration {
    #[inline]
    pub fn precedence(&self) -> u8 {
        match (self.origin, self.declaration.important) {
            (Origin::UserAgent, true) => 9,
            (Origin::Inline, true) => 8,
            (Origin::Shadow, true) => 7,
            (Origin::Author, true) => 6,
            (Origin::Inline, false) => 5,
            (Origin::Author, false) => 4,
            (Origin::Shadow, false) => 3,
            (Origin::PresentationalHint, true) => 3,
            (Origin::PresentationalHint, false) => 2,
            (Origin::UserAgent, false) => 1,
        }
    }

    /// Full comparison according to CSS Cascade Level 5/6 precedence:
    /// 1. Origin + Importance
    /// 2. Layer order (hierarchical, inverted for !important)
    /// 3. Specificity (higher wins)
    /// 4. Scope proximity (CSS Cascading Level 6 § 3.3, smaller distance wins)
    /// 5. Source order (later wins)
    pub fn compare_cascade(&self, other: &Self) -> std::cmp::Ordering {
        let prec = self.precedence().cmp(&other.precedence());
        if prec != std::cmp::Ordering::Equal {
            return prec;
        }

        if (self.origin == Origin::Author || self.origin == Origin::Shadow)
            && self.origin == other.origin
        {
            let layer_cmp = if self.declaration.important {
                compare_layer_paths(&other.layer_path, &self.layer_path)
            } else {
                compare_layer_paths(&self.layer_path, &other.layer_path)
            };
            if layer_cmp != std::cmp::Ordering::Equal {
                return layer_cmp;
            }
        }

        let spec = self.specificity.cmp(&other.specificity);
        if spec != std::cmp::Ordering::Equal {
            return spec;
        }

        // CSS Cascading 6 § 3.3: Scope proximity (smaller distance to scoping root wins)
        match (self.scope_proximity, other.scope_proximity) {
            (Some(p1), Some(p2)) => {
                let prox_cmp = p2.cmp(&p1);
                if prox_cmp != std::cmp::Ordering::Equal {
                    return prox_cmp;
                }
            }
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (None, None) => {}
        }

        self.source_order.cmp(&other.source_order)
    }

    /// Computes a legacy sort key according to CSS Cascade Level 5 precedence.
    pub fn cascade_weight(&self) -> (u8, u32, Specificity, usize) {
        let precedence = self.precedence();
        let layer_rank = if self.origin == Origin::Author || self.origin == Origin::Shadow {
            if self.layer_path.is_empty() {
                if self.declaration.important {
                    0
                } else {
                    u32::MAX
                }
            } else {
                let first = self.layer_path[0];
                if self.declaration.important {
                    u32::MAX.saturating_sub(first)
                } else {
                    first
                }
            }
        } else {
            0
        };
        (precedence, layer_rank, self.specificity, self.source_order)
    }
}

/// A scoping definition representing an `@scope` boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeDef<'a> {
    pub root: Option<&'a crate::selectors::SelectorList>,
    pub limit: Option<&'a crate::selectors::SelectorList>,
    pub scoping_element: Option<mango_html::dom::NodeId>,
}

/// An indexed style rule for accelerated lookup.
#[derive(Debug, Clone)]
pub struct IndexedRule<'a> {
    pub style_rule: &'a StyleRule,
    pub origin: Origin,
    pub source_order: usize,
    pub layer_path: Arc<[u32]>,
    pub scope_chain: Arc<[ScopeDef<'a>]>,
}

/// A registered CSS property declaration (from `@property`).
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredProperty {
    pub name: String,
    pub syntax: Option<String>,
    pub inherits: bool,
    pub initial_value: Option<crate::values::Value>,
    pub layer_path: Arc<[u32]>,
    pub source_order: usize,
}

/// A registered `@keyframes` rule with layer ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredKeyframes {
    pub name: String,
    pub keyframes: crate::parser::KeyframesRule,
    pub layer_path: Arc<[u32]>,
    pub source_order: usize,
}

/// An indexed container query rule for accelerated container matching.
#[derive(Debug, Clone)]
pub struct IndexedContainerRule<'a> {
    pub name: Option<String>,
    pub query: String,
    pub style_rule: &'a StyleRule,
    pub origin: Origin,
    pub source_order: usize,
}

/// A candidate key selector extracted from compound or :is()/:where() selectors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeySelector {
    Id(String),
    Class(String),
    Tag(String),
    Attr(String),
    Root,
}

/// An indexed bucket of pseudo-element rules (e.g. ::before, ::after) grouped by subject key selector.
#[derive(Debug, Clone, Default)]
pub struct PseudoRuleBucket<'a> {
    pub id_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub class_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub tag_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub attr_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub root_rules: Vec<IndexedRule<'a>>,
    pub universal_rules: Vec<IndexedRule<'a>>,
}

impl<'a> PseudoRuleBucket<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_rules(&self) -> usize {
        self.universal_rules.len()
            + self.root_rules.len()
            + self.id_rules.values().map(|v| v.len()).sum::<usize>()
            + self.class_rules.values().map(|v| v.len()).sum::<usize>()
            + self.tag_rules.values().map(|v| v.len()).sum::<usize>()
            + self.attr_rules.values().map(|v| v.len()).sum::<usize>()
    }
}
