//! Core cascade types: origins, matched declarations, and indexed rule representations.

use std::collections::HashMap;

use crate::parser::StyleRule;
use crate::properties::Declaration;
use crate::specificity::Specificity;

/// Style origin for cascade ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    UserAgent = 0,
    PresentationalHint = 1,
    Author = 2,
    Inline = 3,
}

/// A property declaration that matched a target DOM node during cascade evaluation.
#[derive(Debug, Clone)]
pub struct MatchedDeclaration {
    pub declaration: Declaration,
    pub specificity: Specificity,
    pub origin: Origin,
    pub source_order: usize,
    pub layer_order: Option<u32>,
}

impl MatchedDeclaration {
    /// Computes a sort key according to CSS Cascade Level 5 precedence:
    ///
    /// 1. Origin + Importance
    /// 2. Layer order (CSS Cascade 5 §4.4)
    /// 3. Specificity (higher wins)
    /// 4. Source order (later wins)
    pub fn cascade_weight(&self) -> (u8, u32, Specificity, usize) {
        let precedence = match (self.origin, self.declaration.important) {
            (Origin::UserAgent, true) => 7,
            (Origin::Inline, true) => 6,
            (Origin::Author, true) => 5,
            (Origin::Inline, false) => 4,
            (Origin::Author, false) => 3,
            (Origin::PresentationalHint, true) => 3,
            (Origin::PresentationalHint, false) => 2,
            (Origin::UserAgent, false) => 1,
        };

        let layer_rank = if self.origin == Origin::Author {
            match (self.declaration.important, self.layer_order) {
                // Normal author declarations: unlayered styles win over layered styles
                (false, None) => u32::MAX,
                // Within layered styles, later declared layers win over earlier layers
                (false, Some(id)) => id,
                // Important author declarations: earlier layers win over later, and layered wins over unlayered
                (true, None) => 0,
                (true, Some(id)) => u32::MAX.saturating_sub(id),
            }
        } else {
            0
        };

        (precedence, layer_rank, self.specificity, self.source_order)
    }
}

/// An indexed style rule for accelerated lookup.
#[derive(Debug, Clone)]
pub struct IndexedRule<'a> {
    pub style_rule: &'a StyleRule,
    pub origin: Origin,
    pub source_order: usize,
    pub layer_order: Option<u32>,
    pub scope_root: Option<&'a crate::selectors::SelectorList>,
    pub scope_limit: Option<&'a crate::selectors::SelectorList>,
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
