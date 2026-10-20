//! HTML5 constraint-validation and form state pseudo-class helpers.

use mango_html::dom::{Document, ElementData, NodeData, NodeId};

use super::nav::{is_descendant_of, parent_element};

static REGEX_CACHE: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, Option<regex::Regex>>>,
> = std::sync::OnceLock::new();

pub(crate) fn match_cached_pattern(pattern: &str, value: &str) -> bool {
    let cache_lock =
        REGEX_CACHE.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()));
    {
        let cache = cache_lock.read().unwrap_or_else(|e| e.into_inner());
        if let Some(opt_re) = cache.get(pattern) {
            return opt_re.as_ref().map(|re| re.is_match(value)).unwrap_or(true);
        }
    }
    let mut cache = cache_lock.write().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= 512 {
        cache.clear();
    }
    let anchored = format!("^(?:{})$", pattern);
    let opt_re = regex::Regex::new(&anchored).ok();
    let matches = opt_re.as_ref().map(|re| re.is_match(value)).unwrap_or(true);
    cache.insert(pattern.to_string(), opt_re);
    matches
}

/// Checks whether a node is inside a disabled <fieldset> (and not in the fieldset's first <legend>).
pub(crate) fn is_in_disabled_fieldset(node_id: NodeId, doc: &Document) -> bool {
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
pub(crate) fn element_is_read_write(el: &ElementData) -> bool {
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
pub(crate) fn is_submittable_candidate(el: &ElementData) -> bool {
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
pub(crate) fn form_is_valid(form_id: NodeId, doc: &Document) -> bool {
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

/// Minimal HTML constraint-validation check backing `:valid` / `:invalid`
/// and `element.checkValidity()`.
///
/// Returns `true` when the control satisfies its `required`, `type`, `minlength`,
/// `maxlength`, and `pattern` constraints.
pub fn element_is_valid(node_id: NodeId, el: &ElementData) -> bool {
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
        if let Some(pat) = el.get_attribute("pattern")
            && !match_cached_pattern(pat, &value)
        {
            return false;
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
