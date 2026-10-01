//! CSS selector specificity calculation.
//!
//! Specificity is a weight tuple `(a, b, c)` assigned to a CSS selector according
//! to the W3C Selectors specification:
//! - `a`: Count of ID selectors (`#id`)
//! - `b`: Count of class selectors (`.class`), attribute selectors (`[attr]`), and pseudo-classes (`:hover`)
//! - `c`: Count of type selectors (`div`) and pseudo-elements
//!
//! Universal selector (`*`) and combinators (` `, `>`, `+`, `~`) contribute `(0, 0, 0)`.

use std::cmp::Ordering;
use crate::selectors::{ComplexSelector, SimpleSelector};

/// Specificity tuple `(a, b, c)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl Specificity {
    pub const ZERO: Specificity = Specificity(0, 0, 0);

    /// Calculates the specificity of a [`ComplexSelector`].
    pub fn of(selector: &ComplexSelector) -> Self {
        let mut a = 0;
        let mut b = 0;
        let mut c = 0;

        let mut tally_simple = |simple: &SimpleSelector| match simple {
            SimpleSelector::Id(_) => a += 1,
            SimpleSelector::Class(_) | SimpleSelector::Attribute { .. } => b += 1,
            SimpleSelector::PseudoClass(pseudo) => {
                let lower = pseudo.to_ascii_lowercase();
                if lower.starts_with("where(") && lower.ends_with(')') {
                    // W3C Selectors 4: :where() always contributes zero specificity (0, 0, 0)
                } else if (lower.starts_with("is(") || lower.starts_with("not(") || lower.starts_with("has("))
                    && lower.ends_with(')')
                {
                    let prefix_len = if lower.starts_with("is(") { 3 } else { 4 };
                    let inner = &pseudo[prefix_len..pseudo.len() - 1];
                    if let Some(list) = crate::parser::parse_selectors(inner) {
                        let max_spec = list
                            .selectors
                            .iter()
                            .map(Specificity::of)
                            .max()
                            .unwrap_or(Specificity::ZERO);
                        a += max_spec.0;
                        b += max_spec.1;
                        c += max_spec.2;
                    } else {
                        b += 1;
                    }
                } else if (lower.starts_with("nth-child(") || lower.starts_with("nth-last-child("))
                    && lower.ends_with(')')
                {
                    b += 1;
                    let inner = &pseudo[..pseudo.len() - 1];
                    if let Some(idx) = inner.to_ascii_lowercase().find(" of ") {
                        let sel_str = inner[idx + 4..].trim();
                        if let Some(list) = crate::parser::parse_selectors(sel_str) {
                            let max_spec = list
                                .selectors
                                .iter()
                                .map(Specificity::of)
                                .max()
                                .unwrap_or(Specificity::ZERO);
                            a += max_spec.0;
                            b += max_spec.1;
                            c += max_spec.2;
                        }
                    }
                } else {
                    b += 1;
                }
            }
            SimpleSelector::Type(_) | SimpleSelector::PseudoElement(_) => c += 1,
            SimpleSelector::Universal => {}
        };

        for simple in &selector.head.simple_selectors {
            tally_simple(simple);
        }

        for (_, compound) in &selector.tail {
            for simple in &compound.simple_selectors {
                tally_simple(simple);
            }
        }

        Specificity(a, b, c)
    }
}

impl PartialOrd for Specificity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Specificity {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.cmp(&other.0) {
            Ordering::Equal => match self.1.cmp(&other.1) {
                Ordering::Equal => self.2.cmp(&other.2),
                other_ord => other_ord,
            },
            other_ord => other_ord,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selectors::{Combinator, CompoundSelector};

    #[test]
    fn test_specificity_ordering() {
        // #id (1, 0, 0) vs .class.active (0, 2, 0)
        let s_id = Specificity(1, 0, 0);
        let s_class = Specificity(0, 2, 0);
        let s_elem = Specificity(0, 0, 3);

        assert!(s_id > s_class);
        assert!(s_class > s_elem);
        assert!(Specificity(0, 1, 2) < Specificity(0, 2, 0));
        assert!(Specificity(1, 0, 0) == Specificity(1, 0, 0));
    }

    #[test]
    fn test_specificity_calculation() {
        // div.card > #hero.primary
        let sel = ComplexSelector {
            head: CompoundSelector {
                simple_selectors: vec![
                    SimpleSelector::Type("div".to_string()),
                    SimpleSelector::Class("card".to_string()),
                ],
            },
            tail: vec![(
                Combinator::Child,
                CompoundSelector {
                    simple_selectors: vec![
                        SimpleSelector::Id("hero".to_string()),
                        SimpleSelector::Class("primary".to_string()),
                    ],
                },
            )],
        };

        // IDs: 1 (#hero), Classes/Attrs: 2 (.card, .primary), Types: 1 (div) -> (1, 2, 1)
        assert_eq!(Specificity::of(&sel), Specificity(1, 2, 1));
    }

    #[test]
    fn test_functional_pseudo_class_specificity() {
        use crate::parser::parse_selectors;

        // :where(#header, .item) always has (0, 0, 0)
        let where_sel = &parse_selectors(":where(#header, .item)").unwrap().selectors[0];
        assert_eq!(Specificity::of(where_sel), Specificity(0, 0, 0));

        // section:where(#header) has (0, 0, 1) due to section type selector
        let section_where = &parse_selectors("section:where(#header)").unwrap().selectors[0];
        assert_eq!(Specificity::of(section_where), Specificity(0, 0, 1));

        // :is(h1, #header) takes max argument specificity -> (1, 0, 0)
        let is_sel = &parse_selectors(":is(h1, #header)").unwrap().selectors[0];
        assert_eq!(Specificity::of(is_sel), Specificity(1, 0, 0));

        // :not(.first, div#main) takes max argument specificity -> (1, 0, 1)
        let not_sel = &parse_selectors(":not(.first, div#main)").unwrap().selectors[0];
        assert_eq!(Specificity::of(not_sel), Specificity(1, 0, 1));

        // a:has(> img) -> a (0, 0, 1) + img (0, 0, 1) = (0, 0, 2)
        let has_sel = &parse_selectors("a:has(> img)").unwrap().selectors[0];
        assert_eq!(Specificity::of(has_sel), Specificity(0, 0, 2));

        // div:has(.badge, #special) -> div (0, 0, 1) + max(.badge, #special) (1, 0, 0) = (1, 0, 1)
        let has_max = &parse_selectors("div:has(.badge, #special)").unwrap().selectors[0];
        assert_eq!(Specificity::of(has_max), Specificity(1, 0, 1));

        // li:nth-child(2 of .highlight, #main) -> li (0,0,1) + nth-child (0,1,0) + max(.highlight, #main) (1,0,0) = (1, 1, 1)
        let nth_of = &parse_selectors("li:nth-child(2 of .highlight, #main)").unwrap().selectors[0];
        assert_eq!(Specificity::of(nth_of), Specificity(1, 1, 1));
    }
}
