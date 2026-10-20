use std::collections::HashSet;

use mango_html::dom::{Document, ElementData, NodeData, NodeId};

/// Computes the accessible name according to W3C AccName 1.2 computation rules.
///
/// `aria-labelledby` (step 2A) is checked BEFORE `aria-label` (step 2D).
/// Uses a `visited` set to prevent infinite recursion cycles.
pub fn compute_accessible_name(
    doc: &Document,
    node_id: NodeId,
    elem: &ElementData,
    tag: &str,
    visited: &mut HashSet<NodeId>,
) -> String {
    // Prevent infinite recursion
    if !visited.insert(node_id) {
        return String::new();
    }

    // Step 2A: aria-labelledby references other element IDs (highest precedence)
    if let Some(labelled_by) = elem.get_attribute("aria-labelledby") {
        let ids: Vec<&str> = labelled_by.split_whitespace().collect();
        if !ids.is_empty() {
            let mut names = Vec::new();
            for id in &ids {
                if let Some(referenced_node_id) = doc.find_element_by_id(doc.root(), id) {
                    if !visited.contains(&referenced_node_id) {
                        let text = doc.text_content(referenced_node_id);
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            names.push(trimmed.to_string());
                        }
                    }
                }
            }
            if !names.is_empty() {
                visited.remove(&node_id);
                return names.join(" ");
            }
        }
    }

    // Step 2D: aria-label
    if let Some(label) = elem.get_attribute("aria-label") {
        let t = label.trim();
        if !t.is_empty() {
            visited.remove(&node_id);
            return t.to_string();
        }
    }

    // Step 2E: Native element-specific name rules (HTML-AAM)
    let result = match tag {
        "img" => {
            if let Some(alt) = elem.get_attribute("alt") {
                let t = alt.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
            String::new()
        }
        "input" => compute_input_name(doc, node_id, elem),
        "textarea" | "select" => compute_labelable_name(doc, node_id, elem, tag),
        "button" | "a" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "summary" | "output" | "th"
        | "td" | "li" | "option" | "legend" | "caption" | "dt" | "dd" | "figcaption" => {
            let text = doc.text_content(node_id);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
            String::new()
        }
        "fieldset" => {
            if let Some(legend_text) = find_child_element_text(doc, node_id, "legend") {
                return legend_text;
            }
            String::new()
        }
        "table" => {
            if let Some(caption_text) = find_child_element_text(doc, node_id, "caption") {
                return caption_text;
            }
            String::new()
        }
        "figure" => {
            if let Some(figcap_text) = find_child_element_text(doc, node_id, "figcaption") {
                return figcap_text;
            }
            String::new()
        }
        "details" => {
            if let Some(summary_text) = find_child_element_text(doc, node_id, "summary") {
                return summary_text;
            }
            String::new()
        }
        _ => String::new(),
    };

    if !result.is_empty() {
        visited.remove(&node_id);
        return result;
    }

    // Step 2I: Fallback to title attribute
    if let Some(title) = elem.get_attribute("title") {
        let t = title.trim();
        if !t.is_empty() {
            visited.remove(&node_id);
            return t.to_string();
        }
    }

    visited.remove(&node_id);
    String::new()
}

/// Computes the accessible name for `<input>` elements.
pub fn compute_input_name(doc: &Document, node_id: NodeId, elem: &ElementData) -> String {
    let input_type = elem
        .get_attribute("type")
        .unwrap_or("text")
        .to_ascii_lowercase();

    // Button-like inputs: use value attribute
    if matches!(input_type.as_str(), "button" | "submit" | "reset" | "image") {
        if let Some(val) = elem.get_attribute("value") {
            let t = val.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        // For submit/reset, use default text if no value
        match input_type.as_str() {
            "submit" => return "Submit".to_string(),
            "reset" => return "Reset".to_string(),
            "image" => {
                if let Some(alt) = elem.get_attribute("alt") {
                    let t = alt.trim();
                    if !t.is_empty() {
                        return t.to_string();
                    }
                }
                return "Submit".to_string();
            }
            _ => {}
        }
    }

    // Try explicit <label for="...">
    if let Some(id) = elem.id()
        && let Some(label_text) = find_label_for_id(doc, doc.root(), id)
    {
        return label_text;
    }

    // Try implicit label (wrapping <label>)
    if let Some(label_text) = find_wrapping_label(doc, node_id) {
        return label_text;
    }

    // Placeholder fallback
    if let Some(placeholder) = elem.get_attribute("placeholder") {
        let t = placeholder.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }

    String::new()
}

/// Computes accessible name for labelable elements (<textarea>, <select>).
pub fn compute_labelable_name(
    doc: &Document,
    node_id: NodeId,
    elem: &ElementData,
    tag: &str,
) -> String {
    // Try explicit <label for="...">
    if let Some(id) = elem.id()
        && let Some(label_text) = find_label_for_id(doc, doc.root(), id)
    {
        return label_text;
    }

    // Try implicit label (wrapping <label>)
    if let Some(label_text) = find_wrapping_label(doc, node_id) {
        return label_text;
    }

    // Placeholder fallback (textarea supports placeholder)
    if tag == "textarea"
        && let Some(placeholder) = elem.get_attribute("placeholder")
    {
        let t = placeholder.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }

    String::new()
}

/// Finds text of a `<label for="...">` matching an input ID.
pub fn find_label_for_id(doc: &Document, root: NodeId, target_id: &str) -> Option<String> {
    for child in doc.children(root) {
        if let NodeData::Element(elem) = &child.data
            && elem.tag_name.eq_ignore_ascii_case("label")
            && elem.get_attribute("for") == Some(target_id)
        {
            let text = doc.text_content(child.id);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Some(found) = find_label_for_id(doc, child.id, target_id) {
            return Some(found);
        }
    }
    None
}

/// Finds an implicit wrapping `<label>` ancestor and returns its text.
pub fn find_wrapping_label(doc: &Document, node_id: NodeId) -> Option<String> {
    let mut current = node_id;
    loop {
        let node = doc.get(current)?;
        let parent_id = node.parent?;
        let parent_node = doc.get(parent_id)?;
        if let NodeData::Element(pelem) = &parent_node.data
            && pelem.tag_name.eq_ignore_ascii_case("label")
        {
            let text = doc.text_content(parent_id);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if matches!(parent_node.data, NodeData::Document) {
            return None;
        }
        current = parent_id;
    }
}

/// Finds the text of the first direct child element with the given tag name.
pub fn find_child_element_text(
    doc: &Document,
    parent_id: NodeId,
    child_tag: &str,
) -> Option<String> {
    for child in doc.children(parent_id) {
        if let NodeData::Element(elem) = &child.data
            && elem.tag_name.eq_ignore_ascii_case(child_tag)
        {
            let text = doc.text_content(child.id);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Computes the accessible description from `aria-describedby`, `aria-description`, or `title`.
pub fn compute_description(doc: &Document, elem: &ElementData) -> Option<String> {
    // aria-describedby takes precedence
    if let Some(described_by) = elem.get_attribute("aria-describedby") {
        let mut descriptions = Vec::new();
        for id in described_by.split_whitespace() {
            if let Some(referenced_node_id) = doc.find_element_by_id(doc.root(), id) {
                let text = doc.text_content(referenced_node_id);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    descriptions.push(trimmed.to_string());
                }
            }
        }
        if !descriptions.is_empty() {
            return Some(descriptions.join(" "));
        }
    }

    // aria-description
    if let Some(desc) = elem.get_attribute("aria-description") {
        let t = desc.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }

    // title attribute as description fallback (only if not used as name)
    if let Some(title) = elem.get_attribute("title") {
        let t = title.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }

    None
}
