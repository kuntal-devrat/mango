//! CSS Nesting Module Level 1 selector expansion algorithms.

use super::types::{Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector};

/// Expands a child selector list nested within a parent selector list according to
/// the CSS Nesting Module Level 1 specification.
pub fn expand_nested_selectors(parent: &SelectorList, child: &SelectorList) -> SelectorList {
    let mut expanded = Vec::new();
    for p in &parent.selectors {
        for c in &child.selectors {
            expanded.push(expand_single_complex_selector(p, c));
        }
    }
    SelectorList::new(expanded)
}

fn contains_nesting_simple(simple: &SimpleSelector) -> bool {
    matches!(simple, SimpleSelector::Nesting)
}

fn contains_nesting_compound(compound: &CompoundSelector) -> bool {
    compound
        .simple_selectors
        .iter()
        .any(contains_nesting_simple)
}

fn contains_nesting_complex(complex: &ComplexSelector) -> bool {
    contains_nesting_compound(&complex.head)
        || complex
            .tail
            .iter()
            .any(|(_, comp)| contains_nesting_compound(comp))
}

fn expand_single_complex_selector(
    parent: &ComplexSelector,
    child: &ComplexSelector,
) -> ComplexSelector {
    if !contains_nesting_complex(child) {
        // No & present: per spec, implicitly treated as `& <child>` or `& <comb> <child>`
        if child.head.simple_selectors == vec![SimpleSelector::Universal] && !child.tail.is_empty()
        {
            let mut tail = parent.tail.clone();
            tail.extend(child.tail.clone());
            return ComplexSelector::new(parent.head.clone(), tail);
        }

        let mut tail = parent.tail.clone();
        tail.push((Combinator::Descendant, child.head.clone()));
        tail.extend(child.tail.clone());
        return ComplexSelector::new(parent.head.clone(), tail);
    }

    // & is present in child: substitute each occurrence of `&`
    // Step 1: Head expansion
    let (new_head, initial_tail) = if contains_nesting_compound(&child.head) {
        let non_nesting: Vec<SimpleSelector> = child
            .head
            .simple_selectors
            .iter()
            .filter(|s| !matches!(s, SimpleSelector::Nesting))
            .cloned()
            .collect();

        if non_nesting.is_empty() {
            // & was alone in child.head (e.g. `& .foo`)
            (parent.head.clone(), parent.tail.clone())
        } else {
            // & was attached to other simple selectors (e.g. `&:hover`, `&.active`)
            // Merge with the last compound selector of parent!
            if parent.tail.is_empty() {
                let mut merged = parent.head.simple_selectors.clone();
                merged.extend(non_nesting);
                (CompoundSelector::new(merged), Vec::new())
            } else {
                let mut tail = parent.tail[..parent.tail.len() - 1].to_vec();
                let (last_comb, last_comp) = &parent.tail[parent.tail.len() - 1];
                let mut merged = last_comp.simple_selectors.clone();
                merged.extend(non_nesting);
                tail.push((*last_comb, CompoundSelector::new(merged)));
                (parent.head.clone(), tail)
            }
        }
    } else {
        (child.head.clone(), Vec::new())
    };

    // Step 2: Tail expansion
    let mut final_tail = initial_tail;
    for (comb, comp) in &child.tail {
        if contains_nesting_compound(comp) {
            let non_nesting: Vec<SimpleSelector> = comp
                .simple_selectors
                .iter()
                .filter(|s| !matches!(s, SimpleSelector::Nesting))
                .cloned()
                .collect();

            if non_nesting.is_empty() {
                final_tail.push((*comb, parent.head.clone()));
                final_tail.extend(parent.tail.clone());
            } else if parent.tail.is_empty() {
                let mut merged = parent.head.simple_selectors.clone();
                merged.extend(non_nesting);
                final_tail.push((*comb, CompoundSelector::new(merged)));
            } else {
                final_tail.push((*comb, parent.head.clone()));
                let len = parent.tail.len();
                for item in &parent.tail[..len - 1] {
                    final_tail.push(item.clone());
                }
                let (last_comb, last_comp) = &parent.tail[len - 1];
                let mut merged = last_comp.simple_selectors.clone();
                merged.extend(non_nesting);
                final_tail.push((*last_comb, CompoundSelector::new(merged)));
            }
        } else {
            final_tail.push((*comb, comp.clone()));
        }
    }

    ComplexSelector::new(new_head, final_tail)
}
