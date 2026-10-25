//! Pseudo-class matching logic for CSS Selectors Level 3 and Level 4.

use mango_html::dom::{Document, ElementData, Node, NodeData, NodeId};

use super::has::{get_or_parse_complex_selector, matches_has, matches_simple_pattern};
use super::nth::{matches_an_plus_b, parse_and_match_nth_child};
use super::types::{current_host_element, current_scope_root};
use super::validation::{
    element_is_read_write, element_is_valid, form_is_valid, is_in_disabled_fieldset,
    is_submittable_candidate,
};

/// Evaluates whether an element matches a given CSS pseudo-class string.
pub(crate) fn matches_pseudo_class(
    pseudo: &str,
    el: &ElementData,
    node_id: NodeId,
    node: &Node,
    doc: &Document,
) -> bool {
    let lower = pseudo.to_ascii_lowercase();
    match lower.as_str() {
        "first-child" => {
            if let Some(parent_id) = node.parent {
                let mut first_elem = None;
                for child in doc.children(parent_id) {
                    if matches!(child.data, NodeData::Element(_)) {
                        first_elem = Some(child.id);
                        break;
                    }
                }
                first_elem == Some(node_id)
            } else {
                false
            }
        }
        "last-child" => {
            if let Some(parent_id) = node.parent {
                let mut last_elem = None;
                for child in doc.children(parent_id) {
                    if matches!(child.data, NodeData::Element(_)) {
                        last_elem = Some(child.id);
                    }
                }
                last_elem == Some(node_id)
            } else {
                false
            }
        }
        "only-child" => {
            if let Some(parent_id) = node.parent {
                let mut elem_count = 0;
                for child in doc.children(parent_id) {
                    if matches!(child.data, NodeData::Element(_)) {
                        elem_count += 1;
                    }
                }
                elem_count == 1
            } else {
                false
            }
        }
        "first-of-type" => {
            if let Some(parent_id) = node.parent {
                let tag = &el.tag_name;
                for child in doc.children(parent_id) {
                    if let NodeData::Element(child_el) = &child.data
                        && child_el.tag_name.eq_ignore_ascii_case(tag)
                    {
                        return child.id == node_id;
                    }
                }
                false
            } else {
                false
            }
        }
        "last-of-type" => {
            if let Some(parent_id) = node.parent {
                let tag = &el.tag_name;
                let mut last_matching = None;
                for child in doc.children(parent_id) {
                    if let NodeData::Element(child_el) = &child.data
                        && child_el.tag_name.eq_ignore_ascii_case(tag)
                    {
                        last_matching = Some(child.id);
                    }
                }
                last_matching == Some(node_id)
            } else {
                false
            }
        }
        "only-of-type" => {
            if let Some(parent_id) = node.parent {
                let tag = &el.tag_name;
                let mut count = 0;
                for child in doc.children(parent_id) {
                    if let NodeData::Element(child_el) = &child.data
                        && child_el.tag_name.eq_ignore_ascii_case(tag)
                    {
                        count += 1;
                    }
                }
                count == 1
            } else {
                false
            }
        }
        "link" | "any-link" => {
            el.tag_name.eq_ignore_ascii_case("a") && el.get_attribute("href").is_some()
        }
        "visited" => false,
        "root" => el.tag_name.eq_ignore_ascii_case("html"),
        "scope" => {
            if let Some(scope_id) = current_scope_root() {
                node_id.raw() == scope_id.raw()
            } else {
                el.tag_name.eq_ignore_ascii_case("html")
            }
        }
        "empty" => !doc.children(node_id).any(|child| match &child.data {
            NodeData::Element(_) => true,
            NodeData::Text(t) => !t.trim().is_empty(),
            _ => false,
        }),
        "target" => {
            if el.get_attribute("data-mango-target") == Some("true") {
                return true;
            }
            if let (Some(target_id), Some(id)) = (&doc.target_id, el.id())
                && !target_id.is_empty()
                && id == target_id
            {
                return true;
            }
            false
        }
        "checked" => {
            let tag = el.tag_name.to_ascii_lowercase();
            if tag == "option" {
                el.get_attribute("selected").is_some()
                    || el.get_attribute("data-mango-selected") == Some("true")
            } else if tag == "input" {
                el.get_attribute("checked").is_some()
                    || el.get_attribute("data-mango-checked") == Some("true")
            } else {
                el.get_attribute("checked").is_some()
            }
        }
        "indeterminate" => {
            if el.get_attribute("data-mango-indeterminate") == Some("true") {
                return true;
            }
            let tag = el.tag_name.to_ascii_lowercase();
            if tag == "progress" {
                // A progress element with no value attribute is in indeterminate state (HTML5 §4.10.13)
                el.get_attribute("value").is_none()
            } else if tag == "input" {
                let input_type = el
                    .get_attribute("type")
                    .unwrap_or("text")
                    .to_ascii_lowercase();
                if input_type == "checkbox" {
                    el.get_attribute("indeterminate").is_some()
                        || el.get_attribute("data-mango-indeterminate") == Some("true")
                } else {
                    false
                }
            } else {
                false
            }
        }
        "disabled" => {
            let tag = el.tag_name.to_ascii_lowercase();
            if matches!(
                tag.as_str(),
                "button" | "input" | "select" | "textarea" | "optgroup" | "option" | "fieldset"
            ) {
                el.get_attribute("disabled").is_some() || is_in_disabled_fieldset(node_id, doc)
            } else {
                false
            }
        }
        "enabled" => {
            let tag = el.tag_name.to_ascii_lowercase();
            if matches!(
                tag.as_str(),
                "button" | "input" | "select" | "textarea" | "optgroup" | "option" | "fieldset"
            ) {
                el.get_attribute("disabled").is_none() && !is_in_disabled_fieldset(node_id, doc)
            } else {
                false
            }
        }
        // Form validity pseudo-classes (W3C HTML5 Form Validation & Selectors 4)
        "required" => {
            let tag = el.tag_name.to_ascii_lowercase();
            matches!(tag.as_str(), "input" | "select" | "textarea")
                && el.get_attribute("required").is_some()
        }
        "optional" => {
            let tag = el.tag_name.to_ascii_lowercase();
            matches!(tag.as_str(), "input" | "select" | "textarea")
                && el.get_attribute("required").is_none()
        }
        "valid" => {
            let tag = el.tag_name.to_ascii_lowercase();
            if tag == "form" {
                form_is_valid(node_id, doc)
            } else if matches!(tag.as_str(), "input" | "select" | "textarea") {
                element_is_valid(node_id, el)
            } else {
                false
            }
        }
        "invalid" => {
            let tag = el.tag_name.to_ascii_lowercase();
            if tag == "form" {
                !form_is_valid(node_id, doc)
            } else if matches!(tag.as_str(), "input" | "select" | "textarea") {
                is_submittable_candidate(el)
                    && !element_is_valid(node_id, el)
                    && el.get_attribute("disabled").is_none()
            } else {
                false
            }
        }
        "placeholder-shown" => {
            let is_input = el.tag_name.eq_ignore_ascii_case("input");
            let is_textarea = el.tag_name.eq_ignore_ascii_case("textarea");
            if is_input || is_textarea {
                if is_input {
                    let input_type = el
                        .get_attribute("type")
                        .unwrap_or("text")
                        .to_ascii_lowercase();
                    if !matches!(
                        input_type.as_str(),
                        "text" | "search" | "url" | "tel" | "email" | "password" | "number" | ""
                    ) {
                        return false;
                    }
                }
                let has_placeholder = el
                    .get_attribute("placeholder")
                    .is_some_and(|p| !p.is_empty());
                if !has_placeholder {
                    return false;
                }
                let val = el
                    .get_attribute("data-mango-value")
                    .or_else(|| el.get_attribute("value"));
                if let Some(v) = val
                    && !v.is_empty()
                {
                    return false;
                }
                if is_textarea {
                    let mut text = String::new();
                    for child in doc.children(node_id) {
                        if let NodeData::Text(t) = &child.data {
                            text.push_str(t);
                        }
                    }
                    if !text.is_empty() {
                        return false;
                    }
                }
                true
            } else {
                false
            }
        }
        "read-write" => element_is_read_write(el),
        "read-only" => !element_is_read_write(el),
        "focus-within" => {
            if el.get_attribute("data-mango-focused") == Some("true")
                || el.get_attribute("data-mango-focus-visible") == Some("true")
            {
                return true;
            }
            let mut stack: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            while let Some(id) = stack.pop() {
                if let Some(n) = doc.get(id)
                    && let NodeData::Element(child) = &n.data
                {
                    if child.get_attribute("data-mango-focused") == Some("true")
                        || child.get_attribute("data-mango-focus-visible") == Some("true")
                    {
                        return true;
                    }
                    stack.extend(doc.children(id).map(|c| c.id));
                }
            }
            false
        }
        "focus" => el.get_attribute("data-mango-focused") == Some("true"),
        "focus-visible" => {
            el.get_attribute("data-mango-focus-visible") == Some("true")
                || el.get_attribute("data-mango-focused") == Some("true")
        }
        "hover" => el.is_hovered(),
        "active" => el.is_active(),
        "popover-open" => {
            el.get_attribute("popover").is_some()
                && (el.get_attribute("data-mango-popover-open") == Some("true")
                    || el.get_attribute(":popover-open") == Some("true"))
        }
        s if s.starts_with("nth-child(") && s.ends_with(')') => {
            let arg = pseudo[10..pseudo.len() - 1].trim();
            parse_and_match_nth_child(arg, node_id, doc, false)
        }
        s if s.starts_with("nth-last-child(") && s.ends_with(')') => {
            let arg = pseudo[15..pseudo.len() - 1].trim();
            parse_and_match_nth_child(arg, node_id, doc, true)
        }
        s if s.starts_with("nth-of-type(") && s.ends_with(')') => {
            let arg = pseudo[12..pseudo.len() - 1].trim();
            if let Some(parent_id) = node.parent {
                let tag = &el.tag_name;
                let mut type_pos = 0;
                for child in doc.children(parent_id) {
                    if let NodeData::Element(child_el) = &child.data
                        && child_el.tag_name.eq_ignore_ascii_case(tag)
                    {
                        type_pos += 1;
                        if child.id == node_id {
                            return matches_an_plus_b(arg, type_pos);
                        }
                    }
                }
                false
            } else {
                false
            }
        }
        s if s.starts_with("nth-last-of-type(") && s.ends_with(')') => {
            let arg = pseudo[17..pseudo.len() - 1].trim();
            if let Some(parent_id) = node.parent {
                let tag = &el.tag_name;
                let mut same_type = Vec::new();
                for child in doc.children(parent_id) {
                    if let NodeData::Element(child_el) = &child.data
                        && child_el.tag_name.eq_ignore_ascii_case(tag)
                    {
                        same_type.push(child.id);
                    }
                }
                if let Some(idx) = same_type.iter().position(|&id| id == node_id) {
                    let pos_from_end = same_type.len() - idx;
                    matches_an_plus_b(arg, pos_from_end)
                } else {
                    false
                }
            } else {
                false
            }
        }
        s if (s.starts_with("is(") && s.ends_with(')'))
            || (s.starts_with("where(") && s.ends_with(')')) =>
        {
            let prefix_len = if s.starts_with("is(") { 3 } else { 6 };
            let inner = pseudo[prefix_len..pseudo.len() - 1].trim();
            if inner.is_empty() {
                false
            } else if let Some(list) = get_or_parse_complex_selector(inner) {
                list.matches(node_id, doc)
            } else {
                inner
                    .split(',')
                    .any(|part| matches_simple_pattern(part, el, node_id, doc))
            }
        }
        s if s.starts_with("not(") && s.ends_with(')') => {
            let inner = pseudo[4..pseudo.len() - 1].trim();
            if inner.is_empty() {
                false
            } else if let Some(list) = get_or_parse_complex_selector(inner) {
                !list.matches(node_id, doc)
            } else {
                !inner
                    .split(',')
                    .any(|part| matches_simple_pattern(part, el, node_id, doc))
            }
        }
        s if s.starts_with("has(") && s.ends_with(')') => {
            let inner = pseudo[4..pseudo.len() - 1].trim();
            if inner.is_empty() {
                false
            } else if let Some(list) = get_or_parse_complex_selector(inner) {
                matches_has(node_id, &list.selectors, doc)
            } else {
                false
            }
        }
        "host" => {
            if let Some(host_id) = current_host_element() {
                node_id.raw() == host_id.raw()
            } else {
                el.shadow_root.is_some()
            }
        }
        s if s.starts_with("host(") && s.ends_with(')') => {
            let host_matches = if let Some(host_id) = current_host_element() {
                node_id.raw() == host_id.raw()
            } else {
                el.shadow_root.is_some()
            };
            if host_matches {
                let inner = pseudo[5..pseudo.len() - 1].trim();
                if inner.is_empty() {
                    true
                } else if let Some(list) = get_or_parse_complex_selector(inner) {
                    list.matches(node_id, doc)
                } else {
                    matches_simple_pattern(inner, el, node_id, doc)
                }
            } else {
                false
            }
        }
        s if s.starts_with("host-context(") && s.ends_with(')') => {
            let host_matches = if let Some(host_id) = current_host_element() {
                node_id.raw() == host_id.raw()
            } else {
                el.shadow_root.is_some()
            };
            if host_matches {
                let inner = pseudo[13..pseudo.len() - 1].trim();
                if inner.is_empty() {
                    true
                } else if let Some(list) = get_or_parse_complex_selector(inner) {
                    let mut curr = Some(node_id);
                    let mut matched = false;
                    while let Some(curr_id) = curr {
                        if list.matches(curr_id, doc) {
                            matched = true;
                            break;
                        }
                        curr = doc.get(curr_id).and_then(|n| n.parent);
                    }
                    matched
                } else {
                    false
                }
            } else {
                false
            }
        }
        _ => false,
    }
}
