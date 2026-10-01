//! CSS selectors: simple, compound, complex selectors, and DOM matching engine.

use mango_html::dom::{Document, NodeData, NodeId};

/// Attribute matching operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeOperator {
    /// `[attr]` exists.
    Exists,
    /// `[attr="val"]` exact match.
    Exact,
    /// `[attr~="val"]` whitespace-separated word list includes `val`.
    Includes,
    /// `[attr^="val"]` starts with prefix.
    Prefix,
    /// `[attr$="val"]` ends with suffix.
    Suffix,
    /// `[attr*="val"]` contains substring.
    Substring,
    /// `[attr|="val"]` exact match or starts with `val-`.
    DashMatch,
}

/// A basic atomic selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleSelector {
    /// Universal selector `*`
    Universal,
    /// Tag name selector (e.g. `div`, `h1`)
    Type(String),
    /// Class name selector (e.g. `.card`)
    Class(String),
    /// ID selector (e.g. `#main`)
    Id(String),
    /// Attribute selector (e.g. `[type="text"]`, `[attr="val" i]`)
    Attribute {
        name: String,
        op: AttributeOperator,
        value: String,
        case_insensitive: bool,
    },
    /// Pseudo-class selector (e.g. `:first-child`, `:hover`, `:checked`)
    PseudoClass(String),
    /// Pseudo-element selector (e.g. `::before`, `::after`, `::placeholder`)
    PseudoElement(String),
}

impl SimpleSelector {
    /// Tests if this simple selector matches the given DOM element.
    pub fn matches(&self, node_id: NodeId, doc: &Document) -> bool {
        let Some(node) = doc.get(node_id) else {
            return false;
        };
        let NodeData::Element(el) = &node.data else {
            return false;
        };

        match self {
            SimpleSelector::Universal => true,
            SimpleSelector::Type(tag) => el.tag_name.eq_ignore_ascii_case(tag),
            SimpleSelector::Class(class_name) => el.has_class(class_name),
            SimpleSelector::Id(id_name) => el.id() == Some(id_name),
            SimpleSelector::Attribute {
                name,
                op,
                value,
                case_insensitive,
            } => {
                let raw_val = el.get_attribute(name);
                let (attr_val, target_val) = if *case_insensitive {
                    (
                        raw_val.map(|s| s.to_ascii_lowercase()),
                        value.to_ascii_lowercase(),
                    )
                } else {
                    (raw_val.map(|s| s.to_string()), value.clone())
                };
                let val_ref = target_val.as_str();
                match op {
                    AttributeOperator::Exists => raw_val.is_some(),
                    AttributeOperator::Exact => attr_val.as_deref() == Some(val_ref),
                    AttributeOperator::Includes => attr_val
                        .as_deref()
                        .unwrap_or("")
                        .split_ascii_whitespace()
                        .any(|word| word == val_ref),
                    AttributeOperator::DashMatch => {
                        if let Some(v) = attr_val.as_deref() {
                            v == val_ref
                                || (v.starts_with(val_ref) && v[val_ref.len()..].starts_with('-'))
                        } else {
                            false
                        }
                    }
                    AttributeOperator::Prefix => {
                        if val_ref.is_empty() {
                            false
                        } else {
                            attr_val
                                .as_deref()
                                .map(|v| v.starts_with(val_ref))
                                .unwrap_or(false)
                        }
                    }
                    AttributeOperator::Suffix => {
                        if val_ref.is_empty() {
                            false
                        } else {
                            attr_val
                                .as_deref()
                                .map(|v| v.ends_with(val_ref))
                                .unwrap_or(false)
                        }
                    }
                    AttributeOperator::Substring => {
                        if val_ref.is_empty() {
                            false
                        } else {
                            attr_val
                                .as_deref()
                                .map(|v| v.contains(val_ref))
                                .unwrap_or(false)
                        }
                    }
                }
            }
            SimpleSelector::PseudoElement(_) => {
                // Pseudo-elements (::before, ::after, ::placeholder) never match standard DOM host elements
                false
            }
            SimpleSelector::PseudoClass(pseudo) => {
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
                            "button"
                                | "input"
                                | "select"
                                | "textarea"
                                | "optgroup"
                                | "option"
                                | "fieldset"
                        ) {
                            el.get_attribute("disabled").is_some()
                                || is_in_disabled_fieldset(node_id, doc)
                        } else {
                            false
                        }
                    }
                    "enabled" => {
                        let tag = el.tag_name.to_ascii_lowercase();
                        if matches!(
                            tag.as_str(),
                            "button"
                                | "input"
                                | "select"
                                | "textarea"
                                | "optgroup"
                                | "option"
                                | "fieldset"
                        ) {
                            el.get_attribute("disabled").is_none()
                                && !is_in_disabled_fieldset(node_id, doc)
                        } else {
                            false
                        }
                    }
                    // Form validity pseudo-classes (Phase 1.3 form validation API & Selectors 4)
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
                                    "text"
                                        | "search"
                                        | "url"
                                        | "tel"
                                        | "email"
                                        | "password"
                                        | "number"
                                        | ""
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
                                    || child.get_attribute("data-mango-focus-visible")
                                        == Some("true")
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
                    "hover" => el.get_attribute("data-mango-hover") == Some("true"),
                    "active" => el.get_attribute("data-mango-active") == Some("true"),
                    s if s.starts_with("nth-child(") && s.ends_with(')') => {
                        let arg = s[10..s.len() - 1].trim();
                        parse_and_match_nth_child(arg, node_id, doc, false)
                    }
                    s if s.starts_with("nth-last-child(") && s.ends_with(')') => {
                        let arg = s[15..s.len() - 1].trim();
                        parse_and_match_nth_child(arg, node_id, doc, true)
                    }
                    s if s.starts_with("nth-of-type(") && s.ends_with(')') => {
                        let arg = s[12..s.len() - 1].trim();
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
                        let arg = s[17..s.len() - 1].trim();
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
                        let inner = &s[prefix_len..s.len() - 1];
                        if let Some(list) = get_or_parse_complex_selector(inner) {
                            list.matches(node_id, doc)
                        } else {
                            inner
                                .split(',')
                                .any(|part| matches_simple_pattern(part, el, node_id, doc))
                        }
                    }
                    s if s.starts_with("not(") && s.ends_with(')') => {
                        let inner = &s[4..s.len() - 1];
                        if let Some(list) = get_or_parse_complex_selector(inner) {
                            !list.matches(node_id, doc)
                        } else {
                            !inner
                                .split(',')
                                .any(|part| matches_simple_pattern(part, el, node_id, doc))
                        }
                    }
                    s if s.starts_with("has(") && s.ends_with(')') => {
                        let inner = &s[4..s.len() - 1];
                        if let Some(list) = get_or_parse_complex_selector(inner) {
                            matches_has(node_id, &list.selectors, doc)
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
        }
    }
}

fn get_or_parse_complex_selector(inner: &str) -> Option<SelectorList> {
    use std::sync::{OnceLock, RwLock};
    static CACHE: OnceLock<RwLock<std::collections::HashMap<String, Option<SelectorList>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()));

    // Fast path: read lock for cache hits.
    if let Ok(guard) = cache.read()
        && let Some(res) = guard.get(inner)
    {
        return res.clone();
    }

    let parsed = crate::parser::parse_selectors(inner);
    if let Ok(mut guard) = cache.write() {
        // Evict half the cache to avoid thundering-herd stampede that a full
        // clear() would cause — all threads would re-parse simultaneously.
        if guard.len() >= 2048 {
            let keys_to_remove: Vec<String> = guard.keys().take(guard.len() / 2).cloned().collect();
            for key in keys_to_remove {
                guard.remove(&key);
            }
        }
        guard.insert(inner.to_string(), parsed.clone());
    }
    parsed
}

/// Checks whether a node is inside a disabled <fieldset> (and not in the fieldset's first <legend>).
fn is_in_disabled_fieldset(node_id: NodeId, doc: &Document) -> bool {
    let mut curr = node_id;
    while let Some(parent_id) = parent_element(curr, doc) {
        if let Some(parent_node) = doc.get(parent_id)
            && let NodeData::Element(parent_el) = &parent_node.data
            && parent_el.tag_name.eq_ignore_ascii_case("fieldset")
            && parent_el.get_attribute("disabled").is_some()
        {
            // Check if node is inside the fieldset's first <legend> child
            let mut first_legend = None;
            for child in doc.children(parent_id) {
                if let NodeData::Element(ch_el) = &child.data
                    && ch_el.tag_name.eq_ignore_ascii_case("legend")
                {
                    first_legend = Some(child.id);
                    break;
                }
            }
            if let Some(legend_id) = first_legend
                && (curr == legend_id || is_descendant_of(node_id, legend_id, doc))
            {
                return false;
            }
            return true;
        }
        curr = parent_id;
    }
    false
}

/// Returns true when an element is in an editable (:read-write) state per HTML5 / Selectors 4 §4.1.5.
fn element_is_read_write(el: &mango_html::dom::ElementData) -> bool {
    if let Some(ce) = el.get_attribute("contenteditable")
        && !ce.eq_ignore_ascii_case("false")
    {
        return true;
    }
    if el.get_attribute("readonly").is_some() || el.get_attribute("disabled").is_some() {
        return false;
    }
    let tag = el.tag_name.to_ascii_lowercase();
    if tag == "textarea" {
        return true;
    }
    if tag == "input" {
        let t = el
            .get_attribute("type")
            .unwrap_or("text")
            .to_ascii_lowercase();
        return matches!(
            t.as_str(),
            "text"
                | "search"
                | "url"
                | "tel"
                | "email"
                | "password"
                | "number"
                | "date"
                | "time"
                | ""
        );
    }
    false
}

/// Returns true if an input is a candidate for constraint validation.
fn is_submittable_candidate(el: &mango_html::dom::ElementData) -> bool {
    let input_type = el
        .get_attribute("type")
        .unwrap_or("text")
        .to_ascii_lowercase();
    !matches!(
        input_type.as_str(),
        "submit" | "button" | "reset" | "hidden" | "image"
    )
}

/// Tests whether all validatable submittable controls in a <form> satisfy their constraints.
fn form_is_valid(form_id: NodeId, doc: &Document) -> bool {
    let mut stack: Vec<NodeId> = doc.children(form_id).map(|c| c.id).collect();
    while let Some(id) = stack.pop() {
        if let Some(node) = doc.get(id)
            && let NodeData::Element(el) = &node.data
        {
            let tag = el.tag_name.to_ascii_lowercase();
            if matches!(tag.as_str(), "input" | "select" | "textarea")
                && is_submittable_candidate(el)
                && el.get_attribute("disabled").is_none()
                && !element_is_valid(id, el)
            {
                return false;
            }
            stack.extend(doc.children(id).map(|c| c.id));
        }
    }
    true
}

/// Parses and matches `:nth-child(An+B [of S])` and `:nth-last-child(An+B [of S])` (Selectors Level 4).
fn parse_and_match_nth_child(arg: &str, node_id: NodeId, doc: &Document, from_last: bool) -> bool {
    let Some(node) = doc.get(node_id) else {
        return false;
    };
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

    let selector_list = sel_filter.and_then(crate::parser::parse_selectors);

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

/// Minimal HTML constraint-validation check backing `:valid` / `:invalid`
/// and `element.checkValidity()`.
///
/// Returns `true` when the control satisfies its `required`, `type`, `minlength`,
/// `maxlength`, and `pattern` constraints.
pub fn element_is_valid(node_id: NodeId, el: &mango_html::dom::ElementData) -> bool {
    let tag = el.tag_name.to_ascii_lowercase();
    if !matches!(tag.as_str(), "input" | "textarea" | "select") {
        return true;
    }
    let input_type = el
        .get_attribute("type")
        .unwrap_or("text")
        .to_ascii_lowercase();
    // Buttons and hidden fields are not validated.
    if matches!(
        input_type.as_str(),
        "submit" | "button" | "reset" | "hidden" | "image"
    ) {
        return true;
    }
    let _ = node_id;

    // Check custom validity set via JS setCustomValidity()
    if el.get_attribute("data-mango-custom-validity").is_some() {
        return false;
    }

    let value = el
        .get_attribute("data-mango-value")
        .or_else(|| el.get_attribute("value"))
        .unwrap_or("")
        .to_string();

    if el.get_attribute("required").is_some() {
        if matches!(input_type.as_str(), "checkbox" | "radio") {
            if el.get_attribute("checked").is_none() {
                return false;
            }
        } else if value.trim().is_empty() {
            return false;
        }
    }

    if !value.is_empty() {
        match input_type.as_str() {
            "email" => {
                // `multiple` allows a comma-separated list of addresses.
                let parts: Vec<&str> = if el.get_attribute("multiple").is_some() {
                    value.split(',').collect()
                } else {
                    vec![value.as_str()]
                };
                let email_ok = |s: &str| {
                    let s = s.trim();
                    let (local, domain) = match s.split_once('@') {
                        Some(parts) => parts,
                        None => return false,
                    };
                    !local.is_empty()
                        && domain.contains('.')
                        && !domain.starts_with('.')
                        && !domain.ends_with('.')
                        && !domain.contains(' ')
                };
                if !parts.iter().all(|p| email_ok(p)) {
                    return false;
                }
            }
            "url" => {
                if !(value.contains("://") || value.starts_with("//")) {
                    return false;
                }
            }
            "number" | "range" => {
                let Ok(num) = value.trim().parse::<f64>() else {
                    return false;
                };
                if let Some(min) = el.get_attribute("min").and_then(|v| v.parse::<f64>().ok())
                    && num < min
                {
                    return false;
                }
                if let Some(max) = el.get_attribute("max").and_then(|v| v.parse::<f64>().ok())
                    && num > max
                {
                    return false;
                }
                if let Some(step) = el.get_attribute("step").and_then(|v| v.parse::<f64>().ok())
                    && step > 0.0
                {
                    let min_val = el
                        .get_attribute("min")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let diff = (num - min_val).abs();
                    let rem = diff % step;
                    if rem > 0.0001 && (step - rem) > 0.0001 {
                        return false;
                    }
                }
            }
            "date" => {
                if let Some(min) = el.get_attribute("min")
                    && value.as_str() < min
                {
                    return false;
                }
                if let Some(max) = el.get_attribute("max")
                    && value.as_str() > max
                {
                    return false;
                }
            }
            "time" => {
                if let Some(min) = el.get_attribute("min")
                    && value.as_str() < min
                {
                    return false;
                }
                if let Some(max) = el.get_attribute("max")
                    && value.as_str() > max
                {
                    return false;
                }
            }
            _ => {}
        }

        // HTML5 pattern regex constraint
        if let Some(pat) = el.get_attribute("pattern") {
            let anchored = format!("^(?:{})$", pat);
            if let Ok(re) = regex::Regex::new(&anchored)
                && !re.is_match(&value)
            {
                return false;
            }
        }
    }

    // Length constraints
    let char_count = value.chars().count();
    if let Some(min) = el
        .get_attribute("minlength")
        .and_then(|v| v.parse::<usize>().ok())
        && char_count < min
    {
        return false;
    }
    if let Some(max) = el
        .get_attribute("maxlength")
        .and_then(|v| v.parse::<usize>().ok())
        && char_count > max
    {
        return false;
    }

    true
}

fn matches_has(node_id: NodeId, selectors: &[ComplexSelector], doc: &Document) -> bool {
    for sel in selectors {
        // Check if selector starts with a relative combinator (e.g. > img, + p, ~ p)
        let is_rel =
            sel.head.simple_selectors == vec![SimpleSelector::Universal] && !sel.tail.is_empty();

        if is_rel {
            let first_comb = sel.tail[0].0;
            match first_comb {
                Combinator::NextSibling => {
                    if let Some(next) = next_element_sibling(node_id, doc) {
                        if sel.tail.len() == 1 {
                            if sel.tail[0].1.matches(next, doc) {
                                return true;
                            }
                        } else if sel.matches_relative(next, Some(node_id), doc) {
                            return true;
                        }
                    }
                }
                Combinator::SubsequentSibling => {
                    for sib in subsequent_element_siblings(node_id, doc) {
                        if sel.tail.len() == 1 {
                            if sel.tail[0].1.matches(sib, doc) {
                                return true;
                            }
                        } else if sel.matches_relative(sib, Some(node_id), doc) {
                            return true;
                        }
                    }
                }
                Combinator::Child => {
                    for child in doc.children(node_id) {
                        if matches!(child.data, NodeData::Element(_)) {
                            if sel.tail.len() == 1 {
                                if sel.tail[0].1.matches(child.id, doc) {
                                    return true;
                                }
                            } else if sel.matches_relative(child.id, Some(node_id), doc) {
                                return true;
                            }
                        }
                    }
                }
                Combinator::Descendant => {
                    let mut stack: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
                    while let Some(curr) = stack.pop() {
                        if matches!(doc.get(curr).map(|n| &n.data), Some(NodeData::Element(_))) {
                            if sel.matches_relative(curr, Some(node_id), doc) {
                                return true;
                            }
                            stack.extend(doc.children(curr).map(|c| c.id));
                        }
                    }
                }
            }
        } else {
            // Implicit descendant combinator: any descendant in node_id's subtree
            let mut stack: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
            while let Some(curr) = stack.pop() {
                if matches!(doc.get(curr).map(|n| &n.data), Some(NodeData::Element(_))) {
                    if sel.tail.is_empty() {
                        if sel.head.matches(curr, doc) {
                            return true;
                        }
                    } else if sel.matches(curr, doc)
                        && leftmost_origin_within(curr, sel, node_id, doc)
                    {
                        return true;
                    }
                    stack.extend(doc.children(curr).map(|c| c.id));
                }
            }
        }
    }
    false
}

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

fn matches_simple_pattern(
    pattern: &str,
    el: &mango_html::dom::ElementData,
    node_id: NodeId,
    doc: &Document,
) -> bool {
    let p = pattern.trim();
    if p.is_empty() || p == "*" {
        return true;
    }
    if let Some(class) = p.strip_prefix('.') {
        el.has_class(class)
    } else if let Some(id) = p.strip_prefix('#') {
        el.id() == Some(id)
    } else if let Some(pseudo) = p.strip_prefix(':') {
        SimpleSelector::PseudoClass(pseudo.to_string()).matches(node_id, doc)
    } else if p.starts_with('[') && p.ends_with(']') {
        let attr_inner = &p[1..p.len() - 1];
        if let Some((k, v)) = attr_inner.split_once('=') {
            let key = k.trim();
            let val = v.trim().trim_matches('"').trim_matches('\'');
            el.get_attribute(key) == Some(val)
        } else {
            el.get_attribute(attr_inner.trim()).is_some()
        }
    } else {
        el.tag_name.eq_ignore_ascii_case(p)
    }
}

/// A sequence of simple selectors that all apply to the same element (e.g. `div.card#hero`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompoundSelector {
    pub simple_selectors: Vec<SimpleSelector>,
}

impl CompoundSelector {
    pub fn new(simple_selectors: Vec<SimpleSelector>) -> Self {
        Self { simple_selectors }
    }

    /// Tests if all simple selectors in this compound selector match the node.
    pub fn matches(&self, node_id: NodeId, doc: &Document) -> bool {
        if self.simple_selectors.is_empty() {
            return false;
        }
        self.simple_selectors
            .iter()
            .all(|s| s.matches(node_id, doc))
    }
}

/// Combinator connecting compound selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// Descendant combinator (space ` `)
    Descendant,
    /// Child combinator (`>`)
    Child,
    /// Next-sibling combinator (`+`)
    NextSibling,
    /// Subsequent-sibling combinator (`~`)
    SubsequentSibling,
}

fn canonical_pseudo(p: &str) -> &str {
    if p.eq_ignore_ascii_case("-webkit-input-placeholder")
        || p.eq_ignore_ascii_case("-moz-placeholder")
        || p.eq_ignore_ascii_case("-ms-input-placeholder")
        || p.eq_ignore_ascii_case("placeholder")
    {
        "placeholder"
    } else if p.eq_ignore_ascii_case("-moz-selection") || p.eq_ignore_ascii_case("selection") {
        "selection"
    } else if p.eq_ignore_ascii_case("marker") {
        "marker"
    } else if p.eq_ignore_ascii_case("before") {
        "before"
    } else if p.eq_ignore_ascii_case("after") {
        "after"
    } else if p.eq_ignore_ascii_case("first-line") {
        "first-line"
    } else if p.eq_ignore_ascii_case("first-letter") {
        "first-letter"
    } else {
        p
    }
}

/// A sequence of compound selectors separated by combinators (e.g. `div.main > p + span`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexSelector {
    pub head: CompoundSelector,
    pub tail: Vec<(Combinator, CompoundSelector)>,
}

impl ComplexSelector {
    pub fn new(head: CompoundSelector, tail: Vec<(Combinator, CompoundSelector)>) -> Self {
        Self { head, tail }
    }

    /// Matches this complex selector against a DOM element using right-to-left evaluation.
    pub fn matches(&self, node_id: NodeId, doc: &Document) -> bool {
        self.matches_relative(node_id, None, doc)
    }

    /// Matches this complex selector with an optional expected origin constraint on the leftmost compound.
    pub fn matches_relative(
        &self,
        node_id: NodeId,
        expected_origin: Option<NodeId>,
        doc: &Document,
    ) -> bool {
        if self.tail.is_empty() {
            return self.head.matches(node_id, doc);
        }

        // Right-to-left matching
        let (last_comb, last_compound) = self.tail.last().unwrap();
        if !last_compound.matches(node_id, doc) {
            return false;
        }

        self.match_tail(
            node_id,
            self.tail.len() - 1,
            *last_comb,
            expected_origin,
            doc,
        )
    }

    /// Tests whether this selector targets the specified pseudo-element of `node_id`.
    pub fn matches_pseudo_element(&self, node_id: NodeId, doc: &Document, pseudo: &str) -> bool {
        let last_compound = if self.tail.is_empty() {
            &self.head
        } else {
            &self.tail.last().unwrap().1
        };

        let pseudo_canon = canonical_pseudo(pseudo);

        // Check that the last compound has this pseudo-element
        let has_pseudo = last_compound.simple_selectors.iter().any(|s| {
            if let SimpleSelector::PseudoElement(p) = s {
                canonical_pseudo(p).eq_ignore_ascii_case(pseudo_canon)
            } else {
                false
            }
        });
        if !has_pseudo {
            return false;
        }

        // For ::marker, only list items or elements with display: list-item can have a marker
        if pseudo_canon == "marker" {
            let Some(node) = doc.get(node_id) else {
                return false;
            };
            if let NodeData::Element(el) = &node.data {
                let is_list = el.tag_name.eq_ignore_ascii_case("li")
                    || el.get_attribute("data-mango-display") == Some("list-item");
                if !is_list {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Match all non-pseudo simple selectors in the subject against node_id
        let host_matches = last_compound.simple_selectors.iter().all(|s| {
            if matches!(s, SimpleSelector::PseudoElement(_)) {
                true
            } else {
                s.matches(node_id, doc)
            }
        });
        if !host_matches {
            return false;
        }

        if self.tail.is_empty() {
            return true;
        }

        let (last_comb, _) = self.tail.last().unwrap();
        self.match_tail(node_id, self.tail.len() - 1, *last_comb, None, doc)
    }

    fn match_tail(
        &self,
        current_node: NodeId,
        tail_idx: usize,
        combinator: Combinator,
        expected_origin: Option<NodeId>,
        doc: &Document,
    ) -> bool {
        let prev_compound = if tail_idx == 0 {
            &self.head
        } else {
            &self.tail[tail_idx - 1].1
        };

        match combinator {
            Combinator::Child => {
                let Some(parent) = parent_element(current_node, doc) else {
                    return false;
                };
                if tail_idx == 0 && expected_origin.is_some() {
                    if Some(parent) != expected_origin {
                        return false;
                    }
                    if !prev_compound.matches(parent, doc) {
                        return false;
                    }
                    return true;
                }
                if !prev_compound.matches(parent, doc) {
                    return false;
                }
                if tail_idx == 0 {
                    true
                } else {
                    let next_comb = self.tail[tail_idx - 1].0;
                    self.match_tail(parent, tail_idx - 1, next_comb, expected_origin, doc)
                }
            }

            Combinator::Descendant => {
                let mut curr = current_node;
                while let Some(parent) = parent_element(curr, doc) {
                    if tail_idx == 0 && expected_origin.is_some() {
                        if Some(parent) == expected_origin && prev_compound.matches(parent, doc) {
                            return true;
                        }
                    } else if prev_compound.matches(parent, doc) {
                        if tail_idx == 0 {
                            return true;
                        }
                        let next_comb = self.tail[tail_idx - 1].0;
                        if self.match_tail(parent, tail_idx - 1, next_comb, expected_origin, doc) {
                            return true;
                        }
                    }
                    curr = parent;
                }
                false
            }

            Combinator::NextSibling => {
                let Some(sibling) = prev_element_sibling(current_node, doc) else {
                    return false;
                };
                if tail_idx == 0 && expected_origin.is_some() {
                    if Some(sibling) != expected_origin {
                        return false;
                    }
                    if !prev_compound.matches(sibling, doc) {
                        return false;
                    }
                    return true;
                }
                if !prev_compound.matches(sibling, doc) {
                    return false;
                }
                if tail_idx == 0 {
                    true
                } else {
                    let next_comb = self.tail[tail_idx - 1].0;
                    self.match_tail(sibling, tail_idx - 1, next_comb, expected_origin, doc)
                }
            }

            Combinator::SubsequentSibling => {
                let mut curr = current_node;
                while let Some(sibling) = prev_element_sibling(curr, doc) {
                    if tail_idx == 0 && expected_origin.is_some() {
                        if Some(sibling) == expected_origin && prev_compound.matches(sibling, doc) {
                            return true;
                        }
                    } else if prev_compound.matches(sibling, doc) {
                        if tail_idx == 0 {
                            return true;
                        }
                        let next_comb = self.tail[tail_idx - 1].0;
                        if self.match_tail(sibling, tail_idx - 1, next_comb, expected_origin, doc) {
                            return true;
                        }
                    }
                    curr = sibling;
                }
                false
            }
        }
    }
}

/// A comma-separated list of complex selectors (e.g. `h1, h2, h3`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectorList {
    pub selectors: Vec<ComplexSelector>,
}

impl SelectorList {
    pub fn new(selectors: Vec<ComplexSelector>) -> Self {
        Self { selectors }
    }

    /// Tests if any complex selector in this list matches the DOM element.
    pub fn matches(&self, node_id: NodeId, doc: &Document) -> bool {
        self.selectors.iter().any(|s| s.matches(node_id, doc))
    }

    /// Tests if any complex selector in this list targets the specified pseudo-element of `node_id`.
    pub fn matches_pseudo_element(&self, node_id: NodeId, doc: &Document, pseudo: &str) -> bool {
        self.selectors
            .iter()
            .any(|s| s.matches_pseudo_element(node_id, doc, pseudo))
    }
}

fn leftmost_origin_within(
    mut curr: NodeId,
    sel: &ComplexSelector,
    scope_id: NodeId,
    doc: &Document,
) -> bool {
    for (comb, _) in sel.tail.iter().rev() {
        match comb {
            Combinator::Child => {
                let Some(parent) = parent_element(curr, doc) else {
                    return false;
                };
                curr = parent;
            }
            Combinator::Descendant => {
                let mut found = None;
                let mut p = curr;
                while let Some(parent) = parent_element(p, doc) {
                    if parent == scope_id || is_descendant_of(parent, scope_id, doc) {
                        found = Some(parent);
                        break;
                    }
                    p = parent;
                }
                let Some(ancestor) = found else {
                    return false;
                };
                curr = ancestor;
            }
            Combinator::NextSibling => {
                let Some(sib) = prev_element_sibling(curr, doc) else {
                    return false;
                };
                curr = sib;
            }
            Combinator::SubsequentSibling => {
                let Some(sib) = prev_element_sibling(curr, doc) else {
                    return false;
                };
                curr = sib;
            }
        }
    }
    curr == scope_id || is_descendant_of(curr, scope_id, doc)
}

pub fn is_descendant_of(mut node_id: NodeId, ancestor_id: NodeId, doc: &Document) -> bool {
    while let Some(parent) = parent_element(node_id, doc) {
        if parent == ancestor_id {
            return true;
        }
        node_id = parent;
    }
    false
}

pub fn parent_element(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let node = doc.get(node_id)?;
    let parent_id = node.parent?;
    let parent = doc.get(parent_id)?;
    if matches!(parent.data, NodeData::Element(_)) {
        Some(parent_id)
    } else {
        None
    }
}

pub fn prev_element_sibling(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let mut curr_id = doc.get(node_id)?.prev_sibling;
    while let Some(id) = curr_id {
        let node = doc.get(id)?;
        if matches!(node.data, NodeData::Element(_)) {
            return Some(id);
        }
        curr_id = node.prev_sibling;
    }
    None
}

pub fn next_element_sibling(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let mut curr_id = doc.get(node_id)?.next_sibling;
    while let Some(id) = curr_id {
        let node = doc.get(id)?;
        if matches!(node.data, NodeData::Element(_)) {
            return Some(id);
        }
        curr_id = node.next_sibling;
    }
    None
}

pub fn subsequent_element_siblings(node_id: NodeId, doc: &Document) -> Vec<NodeId> {
    let mut siblings = Vec::new();
    let mut curr_id = doc.get(node_id).and_then(|n| n.next_sibling);
    while let Some(id) = curr_id {
        if let Some(node) = doc.get(id) {
            if matches!(node.data, NodeData::Element(_)) {
                siblings.push(id);
            }
            curr_id = node.next_sibling;
        } else {
            break;
        }
    }
    siblings
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_html::parse_html;

    #[test]
    fn test_selector_matching_basic() {
        let html = r#"<div class="card" id="main"><p class="text">Hello</p></div>"#;
        let doc = parse_html(html);
        let root = doc.root();

        let div_id = doc.find_element_by_tag(root, "div").unwrap();
        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        // Type selector
        assert!(SimpleSelector::Type("div".to_string()).matches(div_id, &doc));
        assert!(!SimpleSelector::Type("span".to_string()).matches(div_id, &doc));

        // Class selector
        assert!(SimpleSelector::Class("card".to_string()).matches(div_id, &doc));
        assert!(SimpleSelector::Class("text".to_string()).matches(p_id, &doc));

        // ID selector
        assert!(SimpleSelector::Id("main".to_string()).matches(div_id, &doc));

        // Compound selector: div.card#main
        let compound = CompoundSelector::new(vec![
            SimpleSelector::Type("div".to_string()),
            SimpleSelector::Class("card".to_string()),
            SimpleSelector::Id("main".to_string()),
        ]);
        assert!(compound.matches(div_id, &doc));
    }

    #[test]
    fn test_complex_selector_child_and_descendant() {
        let html = r#"<div class="wrapper"><div class="inner"><p>Text</p></div></div>"#;
        let doc = parse_html(html);
        let root = doc.root();

        let p_id = doc.find_element_by_tag(root, "p").unwrap();

        // div.inner > p
        let child_sel = ComplexSelector::new(
            CompoundSelector::new(vec![
                SimpleSelector::Type("div".to_string()),
                SimpleSelector::Class("inner".to_string()),
            ]),
            vec![(
                Combinator::Child,
                CompoundSelector::new(vec![SimpleSelector::Type("p".to_string())]),
            )],
        );
        assert!(child_sel.matches(p_id, &doc));

        // div.wrapper p (descendant)
        let desc_sel = ComplexSelector::new(
            CompoundSelector::new(vec![
                SimpleSelector::Type("div".to_string()),
                SimpleSelector::Class("wrapper".to_string()),
            ]),
            vec![(
                Combinator::Descendant,
                CompoundSelector::new(vec![SimpleSelector::Type("p".to_string())]),
            )],
        );
        assert!(desc_sel.matches(p_id, &doc));
    }

    #[test]
    fn test_pseudo_class_link_and_visited() {
        let html = r#"<p><a href="https://example.com">Link</a><a>Not a link</a></p>"#;
        let doc = parse_html(html);
        let root = doc.root();

        // Find the p element
        let p_id = doc.find_element_by_tag(root, "p").unwrap();
        let p_children = doc.children(p_id);

        let mut link_with_href = None;
        let mut link_without_href = None;
        for child in p_children {
            if let NodeData::Element(el) = &child.data
                && el.tag_name == "a"
            {
                if el.get_attribute("href").is_some() {
                    link_with_href = Some(child.id);
                } else {
                    link_without_href = Some(child.id);
                }
            }
        }

        let link_sel = SimpleSelector::PseudoClass("link".to_string());
        assert!(link_sel.matches(link_with_href.unwrap(), &doc));
        assert!(!link_sel.matches(link_without_href.unwrap(), &doc));
    }

    #[test]
    fn test_pseudo_element_selector_isolation() {
        let html = r#"<div class="clearfix">Content</div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let pseudo_sel = ComplexSelector::new(
            CompoundSelector::new(vec![
                SimpleSelector::Class("clearfix".to_string()),
                SimpleSelector::PseudoElement("after".to_string()),
            ]),
            vec![],
        );

        // Crucial test: .clearfix::after MUST NOT match the div itself!
        assert!(!pseudo_sel.matches(div_id, &doc));

        // But it MUST match when querying the pseudo-element!
        assert!(pseudo_sel.matches_pseudo_element(div_id, &doc, "after"));
        assert!(!pseudo_sel.matches_pseudo_element(div_id, &doc, "before"));
    }

    #[test]
    fn test_form_validity_pseudo_classes() {
        let html = r#"
            <form>
                <input id="req-empty" type="text" required />
                <input id="req-filled" type="text" required value="hello" />
                <input id="opt" type="text" />
                <input id="bad-email" type="email" value="not-an-email" />
                <input id="good-email" type="email" value="user@example.com" />
                <input id="num" type="number" value="12" />
                <input id="bad-num" type="number" value="abc" />
                <input id="ro" type="text" readonly />
                <input id="ph" type="text" placeholder="Type here" />
                <input id="hidden-field" type="hidden" required />
            </form>
        "#;
        let doc = parse_html(html);
        let root = doc.root();
        let by_id = |id: &str| doc.find_element_by_id(root, id).unwrap();

        let required = SimpleSelector::PseudoClass("required".to_string());
        let optional = SimpleSelector::PseudoClass("optional".to_string());
        let valid = SimpleSelector::PseudoClass("valid".to_string());
        let invalid = SimpleSelector::PseudoClass("invalid".to_string());
        let read_only = SimpleSelector::PseudoClass("read-only".to_string());
        let ph_shown = SimpleSelector::PseudoClass("placeholder-shown".to_string());

        assert!(required.matches(by_id("req-empty"), &doc));
        assert!(optional.matches(by_id("opt"), &doc));
        assert!(!required.matches(by_id("opt"), &doc));

        assert!(
            !valid.matches(by_id("req-empty"), &doc),
            "empty required field is invalid"
        );
        assert!(invalid.matches(by_id("req-empty"), &doc));
        assert!(valid.matches(by_id("req-filled"), &doc));
        assert!(!invalid.matches(by_id("req-filled"), &doc));

        assert!(!valid.matches(by_id("bad-email"), &doc));
        assert!(valid.matches(by_id("good-email"), &doc));
        assert!(valid.matches(by_id("num"), &doc));
        assert!(!valid.matches(by_id("bad-num"), &doc));

        // Hidden inputs never participate in constraint validation
        assert!(valid.matches(by_id("hidden-field"), &doc));

        assert!(read_only.matches(by_id("ro"), &doc));
        assert!(!read_only.matches(by_id("req-filled"), &doc));

        assert!(
            ph_shown.matches(by_id("ph"), &doc),
            "placeholder with no value"
        );
        assert!(
            !ph_shown.matches(by_id("req-filled"), &doc),
            "placeholder is hidden once a value is set"
        );
    }

    #[test]
    fn test_focus_within_ancestor_matching() {
        let html = r#"<div id="wrap"><p id="mid"><input id="inner"/></p></div>"#;
        let mut doc = parse_html(html);
        let root = doc.root();
        let wrap = doc.find_element_by_id(root, "wrap").unwrap();
        let inner = doc.find_element_by_id(root, "inner").unwrap();

        let focus_within = SimpleSelector::PseudoClass("focus-within".to_string());
        assert!(!focus_within.matches(wrap, &doc));

        if let Some(node) = doc.get_mut(inner)
            && let NodeData::Element(el) = &mut node.data
        {
            el.attributes
                .push(("data-mango-focused".to_string(), "true".to_string()));
        }
        assert!(
            focus_within.matches(wrap, &doc),
            "ancestors match :focus-within"
        );
    }

    #[test]
    fn test_an_plus_b_matching() {
        assert!(matches_an_plus_b("odd", 1));
        assert!(!matches_an_plus_b("odd", 2));
        assert!(matches_an_plus_b("even", 2));
        assert!(!matches_an_plus_b("even", 3));
        assert!(matches_an_plus_b("2n+1", 1));
        assert!(!matches_an_plus_b("2n+1", 2));
        assert!(matches_an_plus_b("2n+1", 3));
        assert!(matches_an_plus_b("2n", 2));
        assert!(!matches_an_plus_b("2n", 3));
        assert!(matches_an_plus_b("2n", 4));
        assert!(matches_an_plus_b("-n+3", 1));
        assert!(matches_an_plus_b("-n+3", 2));
        assert!(matches_an_plus_b("-n+3", 3));
        assert!(!matches_an_plus_b("-n+3", 4));
        assert!(matches_an_plus_b("3", 3));
        assert!(!matches_an_plus_b("3", 2));
    }

    #[test]
    fn test_type_and_functional_pseudo_classes() {
        let html = r#"
            <div>
                <h1>Title</h1>
                <p class="first">First para</p>
                <p class="second">Second para</p>
                <p class="third">Third para</p>
                <span>Footer</span>
            </div>
        "#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let mut p_ids = Vec::new();
        let mut span_id = None;
        for child in doc.children(div_id) {
            if let NodeData::Element(el) = &child.data {
                if el.tag_name == "p" {
                    p_ids.push(child.id);
                } else if el.tag_name == "span" {
                    span_id = Some(child.id);
                }
            }
        }

        // first-of-type
        let first_of_type = SimpleSelector::PseudoClass("first-of-type".to_string());
        assert!(first_of_type.matches(p_ids[0], &doc));
        assert!(!first_of_type.matches(p_ids[1], &doc));

        // last-of-type
        let last_of_type = SimpleSelector::PseudoClass("last-of-type".to_string());
        assert!(last_of_type.matches(p_ids[2], &doc));
        assert!(!last_of_type.matches(p_ids[1], &doc));

        // only-of-type
        let only_of_type = SimpleSelector::PseudoClass("only-of-type".to_string());
        assert!(only_of_type.matches(span_id.unwrap(), &doc));
        assert!(!only_of_type.matches(p_ids[0], &doc));

        // nth-of-type(2)
        let nth_of_type_2 = SimpleSelector::PseudoClass("nth-of-type(2)".to_string());
        assert!(nth_of_type_2.matches(p_ids[1], &doc));
        assert!(!nth_of_type_2.matches(p_ids[0], &doc));

        // is(span, h2)
        let is_sel = SimpleSelector::PseudoClass("is(span, h2)".to_string());
        assert!(is_sel.matches(span_id.unwrap(), &doc));
        assert!(!is_sel.matches(p_ids[0], &doc));

        // where(.second, span)
        let where_sel = SimpleSelector::PseudoClass("where(.second, span)".to_string());
        assert!(where_sel.matches(p_ids[1], &doc));
        assert!(where_sel.matches(span_id.unwrap(), &doc));
        assert!(!where_sel.matches(p_ids[0], &doc));

        // not(.first)
        let not_sel = SimpleSelector::PseudoClass("not(.first)".to_string());
        assert!(!not_sel.matches(p_ids[0], &doc));
        assert!(not_sel.matches(p_ids[1], &doc));
    }

    #[test]
    fn test_has_pseudo_class_matching() {
        fn find_by_class(node_id: NodeId, class_name: &str, doc: &Document) -> Option<NodeId> {
            for child in doc.children(node_id) {
                if let NodeData::Element(el) = &child.data
                    && el.has_class(class_name)
                {
                    return Some(child.id);
                }
                if let Some(found) = find_by_class(child.id, class_name, doc) {
                    return Some(found);
                }
            }
            None
        }

        let html = r#"
            <div id="container">
                <article class="with-img">
                    <h2>Card 1</h2>
                    <img src="photo.jpg" alt="test" />
                </article>
                <article class="no-img">
                    <h2>Card 2</h2>
                    <p>Just text</p>
                </article>
                <section class="parent">
                    <div class="direct-child">
                        <span class="nested">Deep</span>
                    </div>
                </section>
            </div>
        "#;
        let doc = parse_html(html);
        let root = doc.root();
        let with_img_id = find_by_class(root, "with-img", &doc).unwrap();
        let no_img_id = find_by_class(root, "no-img", &doc).unwrap();
        let section_id = find_by_class(root, "parent", &doc).unwrap();

        // article:has(img)
        let has_img = SimpleSelector::PseudoClass("has(img)".to_string());
        assert!(has_img.matches(with_img_id, &doc));
        assert!(!has_img.matches(no_img_id, &doc));

        // section:has(> .direct-child) - direct child combinator
        let has_direct = SimpleSelector::PseudoClass("has(> .direct-child)".to_string());
        assert!(has_direct.matches(section_id, &doc));

        // section:has(> .nested) - should be false because .nested is not a direct child
        let has_not_direct = SimpleSelector::PseudoClass("has(> .nested)".to_string());
        assert!(!has_not_direct.matches(section_id, &doc));

        // section:has(.nested) - descendant combinator matches
        let has_descendant = SimpleSelector::PseudoClass("has(.nested)".to_string());
        assert!(has_descendant.matches(section_id, &doc));
    }

    #[test]
    fn test_form_element_validation() {
        let html = r#"
            <form id="f">
                <input id="num-ok" type="number" min="10" max="50" step="5" value="25" />
                <input id="num-bad-min" type="number" min="10" max="50" value="5" />
                <input id="num-bad-step" type="number" min="10" max="50" step="5" value="23" />
                <input id="pat-ok" type="text" pattern="[A-Z]{3}-[0-9]{3}" value="ABC-123" />
                <input id="pat-bad" type="text" pattern="[A-Z]{3}-[0-9]{3}" value="abc-123" />
                <input id="date-ok" type="date" min="2026-01-01" max="2026-12-31" value="2026-06-15" />
                <input id="date-bad" type="date" min="2026-01-01" max="2026-12-31" value="2027-01-01" />
            </form>
        "#;
        let doc = parse_html(html);

        fn is_valid_by_id(id: &str, doc: &Document) -> bool {
            let root = doc.root();
            let mut stack = vec![root];
            while let Some(nid) = stack.pop() {
                if let Some(node) = doc.get(nid) {
                    if let NodeData::Element(el) = &node.data
                        && el.get_attribute("id") == Some(id)
                    {
                        return element_is_valid(nid, el);
                    }
                    for c in doc.children(nid) {
                        stack.push(c.id);
                    }
                }
            }
            panic!("Element #{} not found", id);
        }

        assert!(is_valid_by_id("num-ok", &doc));
        assert!(!is_valid_by_id("num-bad-min", &doc));
        assert!(!is_valid_by_id("num-bad-step", &doc));
        assert!(is_valid_by_id("pat-ok", &doc));
        assert!(!is_valid_by_id("pat-bad", &doc));
        assert!(is_valid_by_id("date-ok", &doc));
        assert!(!is_valid_by_id("date-bad", &doc));
    }

    #[test]
    fn test_selectors_level_4() {
        let html = r#"
            <div id="wrapper">
                <!-- Target test element -->
                <section id="section-target" data-mango-target="true">
                    <h1 id="heading">Title</h1>
                    <p id="first-p" class="intro">Intro paragraph</p>
                    <span id="sibling-span">Span text</span>
                </section>

                <!-- List for nth-child(An+B of S), nth-of-type, etc. -->
                <ul id="list">
                    <li id="li-1" class="item active">Item 1</li>
                    <li id="li-2" class="item">Item 2</li>
                    <li id="li-3" class="separator">Separator</li>
                    <li id="li-4" class="item active">Item 4</li>
                    <li id="li-5" class="item">Item 5</li>
                </ul>

                <!-- Only child and type tests -->
                <div id="single-parent">
                    <p id="only-p">Only child and only of type</p>
                </div>
                <div id="multi-type-parent">
                    <span id="span-type-1">Span 1</span>
                    <span id="span-type-2">Span 2</span>
                    <h2 id="only-h2">Only H2</h2>
                </div>

                <!-- Empty test elements -->
                <div id="truly-empty"></div>
                <div id="comment-empty"><!-- a comment should be ignored --></div>
                <div id="whitespace-empty">
                    
                </div>
                <div id="not-empty">Text inside</div>

                <!-- Form controls for validity, enabled/disabled, checked, indeterminate, placeholder-shown, read-only/write -->
                <form id="test-form">
                    <input id="inp-enabled" type="text" placeholder="Enter name" value="" />
                    <input id="inp-disabled" type="text" disabled value="disabled" />
                    <input id="inp-required" type="text" required value="" />
                    <input id="inp-optional" type="text" value="optional text" />
                    <input id="chk-checked" type="checkbox" checked />
                    <input id="chk-unchecked" type="checkbox" />
                    <input id="chk-indet" type="checkbox" data-mango-indeterminate="true" />
                    <progress id="prog-indet"></progress>
                    <progress id="prog-determinate" value="50" max="100"></progress>
                    <textarea id="ta-editable" placeholder="Type here"></textarea>
                    <textarea id="ta-readonly" readonly>Read-only text</textarea>
                </form>

                <div id="editable-div" contenteditable="true">Editable content</div>
                <p id="readonly-p">Plain paragraph</p>

                <!-- Focus elements -->
                <div id="focus-container">
                    <input id="inp-focus-visible" type="text" data-mango-focus-visible="true" />
                </div>
            </div>
        "#;

        let mut doc = parse_html(html);
        let root = doc.root();
        fn collect_ids(
            doc: &Document,
            node_id: NodeId,
            map: &mut std::collections::HashMap<String, NodeId>,
        ) {
            for child in doc.children(node_id) {
                if let mango_html::NodeData::Element(elem) = &child.data
                    && let Some(id) = elem.id()
                {
                    map.insert(id.to_string(), child.id);
                }
                collect_ids(doc, child.id, map);
            }
        }
        let mut el_map = std::collections::HashMap::new();
        collect_ids(&doc, root, &mut el_map);
        let el = |id: &str| *el_map.get(id).unwrap();

        // 1. :is() and :where()
        let is_sel = crate::parser::parse_selectors(":is(h1, h2, #nonexistent)").unwrap();
        assert!(is_sel.matches(el("heading"), &doc));
        assert!(is_sel.matches(el("only-h2"), &doc));
        assert!(!is_sel.matches(el("first-p"), &doc));

        let where_sel =
            crate::parser::parse_selectors(":where(p.intro, span#sibling-span)").unwrap();
        assert!(where_sel.matches(el("first-p"), &doc));
        assert!(where_sel.matches(el("sibling-span"), &doc));
        assert!(!where_sel.matches(el("heading"), &doc));

        // 2. :not() with complex selectors
        let not_complex =
            crate::parser::parse_selectors(":not(#section-target > p.intro)").unwrap();
        assert!(!not_complex.matches(el("first-p"), &doc));
        assert!(not_complex.matches(el("heading"), &doc));

        let not_multi = crate::parser::parse_selectors(":not(.active, .separator)").unwrap();
        assert!(!not_multi.matches(el("li-1"), &doc));
        assert!(!not_multi.matches(el("li-3"), &doc));
        assert!(not_multi.matches(el("li-2"), &doc));

        // 3. :has() parent and sibling selector
        // Relative next-sibling: h1:has(+ p)
        let has_next_sib = crate::parser::parse_selectors("h1:has(+ p)").unwrap();
        assert!(has_next_sib.matches(el("heading"), &doc));

        // Relative subsequent-sibling: h1:has(~ span)
        let has_sub_sib = crate::parser::parse_selectors("h1:has(~ span)").unwrap();
        assert!(has_sub_sib.matches(el("heading"), &doc));

        // Sibling selector that doesn't match: p:has(+ h1)
        let has_no_match = crate::parser::parse_selectors("p:has(+ h1)").unwrap();
        assert!(!has_no_match.matches(el("first-p"), &doc));

        // Relative child selector: section:has(> h1#heading)
        let has_child = crate::parser::parse_selectors("section:has(> h1#heading)").unwrap();
        assert!(has_child.matches(el("section-target"), &doc));

        // Descendant selector: section:has(span#sibling-span)
        let has_descendant =
            crate::parser::parse_selectors("section:has(span#sibling-span)").unwrap();
        assert!(has_descendant.matches(el("section-target"), &doc));

        // 4. :nth-child(An+B [of S]) and :nth-last-child(An+B [of S])
        // Filtered: 2 of .active (among items with .active, index 2 is li-4)
        let nth_of_active = crate::parser::parse_selectors("li:nth-child(2 of .active)").unwrap();
        assert!(!nth_of_active.matches(el("li-1"), &doc)); // index 1 of .active
        assert!(!nth_of_active.matches(el("li-2"), &doc)); // not .active
        assert!(nth_of_active.matches(el("li-4"), &doc)); // index 2 of .active

        // nth-last-child(1 of .active) should be li-4
        let nth_last_of =
            crate::parser::parse_selectors("li:nth-last-child(1 of .active)").unwrap();
        assert!(nth_last_of.matches(el("li-4"), &doc));
        assert!(!nth_last_of.matches(el("li-1"), &doc));

        // Standard nth-of-type / nth-last-of-type
        let nth_of_type = crate::parser::parse_selectors("span:nth-of-type(2)").unwrap();
        assert!(nth_of_type.matches(el("span-type-2"), &doc));
        assert!(!nth_of_type.matches(el("span-type-1"), &doc));

        let nth_last_of_type = crate::parser::parse_selectors("span:nth-last-of-type(1)").unwrap();
        assert!(nth_last_of_type.matches(el("span-type-2"), &doc));

        // 5. :first-of-type, :last-of-type, :only-of-type, :only-child
        let first_of_type = SimpleSelector::PseudoClass("first-of-type".to_string());
        assert!(first_of_type.matches(el("span-type-1"), &doc));
        assert!(!first_of_type.matches(el("span-type-2"), &doc));

        let last_of_type = SimpleSelector::PseudoClass("last-of-type".to_string());
        assert!(last_of_type.matches(el("span-type-2"), &doc));

        let only_of_type = SimpleSelector::PseudoClass("only-of-type".to_string());
        assert!(only_of_type.matches(el("only-h2"), &doc));
        assert!(!only_of_type.matches(el("span-type-1"), &doc));

        let only_child = SimpleSelector::PseudoClass("only-child".to_string());
        assert!(only_child.matches(el("only-p"), &doc));
        assert!(!only_child.matches(el("only-h2"), &doc)); // sibling of spans

        // 6. :empty pseudo-class (comments & whitespace ignored)
        let empty_sel = SimpleSelector::PseudoClass("empty".to_string());
        assert!(empty_sel.matches(el("truly-empty"), &doc));
        assert!(
            empty_sel.matches(el("comment-empty"), &doc),
            "comments must be ignored by :empty"
        );
        assert!(
            empty_sel.matches(el("whitespace-empty"), &doc),
            "whitespace must be ignored by :empty in Selectors 4"
        );
        assert!(!empty_sel.matches(el("not-empty"), &doc));

        // 7. :target pseudo-class
        let target_sel = SimpleSelector::PseudoClass("target".to_string());
        assert!(target_sel.matches(el("section-target"), &doc)); // via data-mango-target
        doc.set_target_id(Some("heading".to_string()));
        assert!(
            target_sel.matches(el("heading"), &doc),
            ":target matches document.target_id"
        );
        assert!(!target_sel.matches(el("first-p"), &doc));

        // 8. :focus-visible and :focus-within
        let focus_vis = SimpleSelector::PseudoClass("focus-visible".to_string());
        let focus_within = SimpleSelector::PseudoClass("focus-within".to_string());
        assert!(focus_vis.matches(el("inp-focus-visible"), &doc));
        assert!(
            focus_within.matches(el("focus-container"), &doc),
            "ancestor matches :focus-within"
        );
        assert!(!focus_within.matches(el("section-target"), &doc));

        // 9. :placeholder-shown
        let ph_shown = SimpleSelector::PseudoClass("placeholder-shown".to_string());
        assert!(ph_shown.matches(el("inp-enabled"), &doc));
        assert!(ph_shown.matches(el("ta-editable"), &doc));
        assert!(!ph_shown.matches(el("inp-optional"), &doc)); // has value "optional text"

        // 10. :enabled, :disabled, :checked, :indeterminate
        let enabled_sel = SimpleSelector::PseudoClass("enabled".to_string());
        let disabled_sel = SimpleSelector::PseudoClass("disabled".to_string());
        let checked_sel = SimpleSelector::PseudoClass("checked".to_string());
        let indet_sel = SimpleSelector::PseudoClass("indeterminate".to_string());

        assert!(enabled_sel.matches(el("inp-enabled"), &doc));
        assert!(!enabled_sel.matches(el("inp-disabled"), &doc));
        assert!(
            !enabled_sel.matches(el("wrapper"), &doc),
            "div cannot match :enabled"
        );

        assert!(disabled_sel.matches(el("inp-disabled"), &doc));
        assert!(!disabled_sel.matches(el("inp-enabled"), &doc));

        assert!(checked_sel.matches(el("chk-checked"), &doc));
        assert!(!checked_sel.matches(el("chk-unchecked"), &doc));

        assert!(indet_sel.matches(el("chk-indet"), &doc));
        assert!(
            indet_sel.matches(el("prog-indet"), &doc),
            "<progress> with no value matches :indeterminate"
        );
        assert!(!indet_sel.matches(el("prog-determinate"), &doc));

        // 11. :required, :optional, :valid, :invalid
        let req_sel = SimpleSelector::PseudoClass("required".to_string());
        let opt_sel = SimpleSelector::PseudoClass("optional".to_string());
        let valid_sel = SimpleSelector::PseudoClass("valid".to_string());
        let invalid_sel = SimpleSelector::PseudoClass("invalid".to_string());

        assert!(req_sel.matches(el("inp-required"), &doc));
        assert!(!req_sel.matches(el("inp-optional"), &doc));
        assert!(
            !req_sel.matches(el("wrapper"), &doc),
            "div cannot match :required"
        );

        assert!(opt_sel.matches(el("inp-optional"), &doc));
        assert!(!opt_sel.matches(el("inp-required"), &doc));
        assert!(
            !opt_sel.matches(el("wrapper"), &doc),
            "div cannot match :optional"
        );

        assert!(
            invalid_sel.matches(el("inp-required"), &doc),
            "empty required field is :invalid"
        );
        assert!(!valid_sel.matches(el("inp-required"), &doc));
        assert!(valid_sel.matches(el("inp-optional"), &doc));

        // 12. :read-only, :read-write
        let rw_sel = SimpleSelector::PseudoClass("read-write".to_string());
        let ro_sel = SimpleSelector::PseudoClass("read-only".to_string());

        assert!(rw_sel.matches(el("inp-enabled"), &doc));
        assert!(rw_sel.matches(el("ta-editable"), &doc));
        assert!(
            rw_sel.matches(el("editable-div"), &doc),
            "contenteditable elements match :read-write"
        );

        assert!(!rw_sel.matches(el("inp-disabled"), &doc));
        assert!(!rw_sel.matches(el("ta-readonly"), &doc));
        assert!(
            !rw_sel.matches(el("readonly-p"), &doc),
            "normal paragraphs match :read-only"
        );

        assert!(ro_sel.matches(el("readonly-p"), &doc));
        assert!(ro_sel.matches(el("inp-disabled"), &doc));
        assert!(ro_sel.matches(el("ta-readonly"), &doc));
        assert!(!ro_sel.matches(el("inp-enabled"), &doc));

        // 13. ::placeholder pseudo-element
        let ph_pseudo = crate::parser::parse_selectors("input::placeholder")
            .unwrap()
            .selectors[0]
            .clone();
        assert!(ph_pseudo.matches_pseudo_element(el("inp-enabled"), &doc, "placeholder"));
        assert!(ph_pseudo.matches_pseudo_element(
            el("inp-enabled"),
            &doc,
            "-webkit-input-placeholder"
        ));

        // 14. ::selection pseudo-element
        let sel_pseudo = crate::parser::parse_selectors("p::selection")
            .unwrap()
            .selectors[0]
            .clone();
        assert!(sel_pseudo.matches_pseudo_element(el("first-p"), &doc, "selection"));
        assert!(sel_pseudo.matches_pseudo_element(el("first-p"), &doc, "-moz-selection"));
        assert!(!sel_pseudo.matches_pseudo_element(el("heading"), &doc, "selection")); // h1 is not p

        // 15. ::marker pseudo-element
        let marker_pseudo = crate::parser::parse_selectors("li::marker")
            .unwrap()
            .selectors[0]
            .clone();
        assert!(marker_pseudo.matches_pseudo_element(el("li-1"), &doc, "marker"));
        assert!(
            !marker_pseudo.matches_pseudo_element(el("first-p"), &doc, "marker"),
            "p does not have ::marker"
        );
    }

    #[test]
    fn test_attribute_operators_dashmatch_empty_and_case_insensitive() {
        let html = r#"
            <div id="en" lang="en"></div>
            <div id="en-us" lang="en-US"></div>
            <div id="english" lang="english"></div>
            <input id="chk" type="checkbox">
        "#;
        let doc = mango_html::tree_builder::parse_html(html);
        let root = doc.root();
        let el = |id: &str| doc.find_element_by_id(root, id).unwrap();

        // DashMatch: [lang|="en"]
        let dash_sel = crate::parser::parse_selectors("[lang|=\"en\"]").unwrap();
        assert!(
            dash_sel.matches(el("en"), &doc),
            "[lang|='en'] matches 'en'"
        );
        assert!(
            dash_sel.matches(el("en-us"), &doc),
            "[lang|='en'] matches 'en-US'"
        );
        assert!(
            !dash_sel.matches(el("english"), &doc),
            "[lang|='en'] does not match 'english'"
        );

        // Empty string matches should return false
        let empty_prefix = crate::parser::parse_selectors("[lang^=\"\"]").unwrap();
        assert!(
            !empty_prefix.matches(el("en"), &doc),
            "[attr^=''] must never match"
        );
        let empty_suffix = crate::parser::parse_selectors("[lang$=\"\"]").unwrap();
        assert!(
            !empty_suffix.matches(el("en"), &doc),
            "[attr$=''] must never match"
        );
        let empty_sub = crate::parser::parse_selectors("[lang*=\"\"]").unwrap();
        assert!(
            !empty_sub.matches(el("en"), &doc),
            "[attr*=''] must never match"
        );

        // Case-insensitive attribute match: [type="CHECKBOX" i]
        let case_sel = crate::parser::parse_selectors("[type=\"CHECKBOX\" i]").unwrap();
        assert!(
            case_sel.matches(el("chk"), &doc),
            "[type='CHECKBOX' i] matches 'checkbox'"
        );
    }
}
