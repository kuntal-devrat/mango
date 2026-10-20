//! Implementation of `:nth-child()`, `:nth-last-child()`, and An+B index matching.

use mango_html::dom::{Document, NodeData, NodeId};

use super::has::get_or_parse_complex_selector;

/// Matches a 1-based position against an `an+b`, `odd`, `even`, or integer formula (CSS 2.1 §17 / Selectors 3).
pub fn matches_an_plus_b(formula: &str, pos: usize) -> bool {
    let s = formula.trim().replace(' ', "");
    if s.eq_ignore_ascii_case("odd") {
        return pos % 2 == 1;
    }
    if s.eq_ignore_ascii_case("even") {
        return pos.is_multiple_of(2);
    }
    if let Ok(b) = s.parse::<isize>() {
        return pos as isize == b;
    }

    // Parse an+b formula: [+/-][a]n[+/-b]
    let lower = s.to_ascii_lowercase();
    if let Some(n_idx) = lower.find('n') {
        let a_str = &lower[..n_idx];
        let b_str = &lower[n_idx + 1..];

        let a: isize = if a_str.is_empty() || a_str == "+" {
            1
        } else if a_str == "-" {
            -1
        } else {
            a_str.parse::<isize>().unwrap_or(0)
        };

        let b: isize = if b_str.is_empty() {
            0
        } else if let Some(stripped) = b_str.strip_prefix('+') {
            stripped.parse::<isize>().unwrap_or(0)
        } else {
            b_str.parse::<isize>().unwrap_or(0)
        };

        let p = pos as isize;
        if a == 0 {
            p == b
        } else {
            let diff = p - b;
            if a > 0 {
                diff >= 0 && diff % a == 0
            } else {
                diff <= 0 && diff % a == 0
            }
        }
    } else {
        false
    }
}

/// Parses and matches `:nth-child(An+B [of S])` and `:nth-last-child(An+B [of S])` (Selectors Level 4).
pub(crate) fn parse_and_match_nth_child(
    arg: &str,
    node_id: NodeId,
    doc: &Document,
    from_last: bool,
) -> bool {
    let Some(node) = doc.get(node_id) else {
        return false;
    };
    let node_id = node.id;
    let Some(parent_id) = node.parent else {
        return false;
    };

    let (formula, sel_filter) = if let Some(idx) = arg.to_ascii_lowercase().find(" of ") {
        let f = arg[..idx].trim();
        let s = arg[idx + 4..].trim();
        (f, Some(s))
    } else {
        (arg.trim(), None)
    };

    let selector_list = sel_filter.and_then(get_or_parse_complex_selector);

    if let Some(list) = &selector_list
        && !list.matches(node_id, doc)
    {
        return false;
    }

    if from_last {
        let mut matching_elements = Vec::new();
        for child in doc.children(parent_id) {
            if matches!(child.data, NodeData::Element(_)) {
                if let Some(list) = &selector_list {
                    if list.matches(child.id, doc) {
                        matching_elements.push(child.id);
                    }
                } else {
                    matching_elements.push(child.id);
                }
            }
        }
        if let Some(idx_from_start) = matching_elements.iter().position(|&id| id == node_id) {
            let pos_from_end = matching_elements.len() - idx_from_start;
            matches_an_plus_b(formula, pos_from_end)
        } else {
            false
        }
    } else {
        let mut sibling_idx = 0;
        let mut current_pos = 0;
        for child in doc.children(parent_id) {
            if matches!(child.data, NodeData::Element(_)) {
                let matches_filter = if let Some(list) = &selector_list {
                    list.matches(child.id, doc)
                } else {
                    true
                };
                if matches_filter {
                    sibling_idx += 1;
                    if child.id == node_id {
                        current_pos = sibling_idx;
                        break;
                    }
                }
            }
        }
        if current_pos == 0 {
            return false;
        }
        matches_an_plus_b(formula, current_pos)
    }
}
