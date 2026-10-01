//! # Accessibility (A11y) Tree & ARIA Engine (GAP-020)
//!
//! Converts DOM elements and layout boxes into an accessible tree conforming to
//! W3C WAI-ARIA 1.2 and the HTML Accessibility API Mappings (AAM).
//!
//! Provides screen-reader semantics, accessible name computation (AccName 1.1),
//! role resolution (implicit HTML + explicit ARIA), state extraction, and
//! spatial bounding boxes for assistive technologies.

use crate::box_tree::LayoutBox;
use mango_core::Rect;
use mango_html::dom::{Document, NodeData, NodeId};

/// ARIA and HTML standard accessibility roles.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum A11yRole {
    RootWebArea,
    Header,
    Footer,
    Navigation,
    Main,
    Article,
    Section,
    Heading { level: u8 },
    Button,
    Link,
    Checkbox,
    RadioButton,
    TextInput,
    Slider,
    Image,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    RowHeader,
    Dialog,
    Alert,
    Tab,
    TabList,
    TabPanel,
    ComboBox,
    StaticText,
    Generic,
}

/// Accessible states and properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11yState {
    pub disabled: bool,
    pub hidden: bool,
    pub expanded: Option<bool>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub readonly: bool,
    pub required: bool,
    pub value: Option<String>,
    pub value_min: Option<f32>,
    pub value_max: Option<f32>,
    pub value_now: Option<f32>,
}

/// An individual node in the accessibility tree.
#[derive(Debug, Clone)]
pub struct A11yNode {
    pub id: usize,
    pub node_id: Option<NodeId>,
    pub role: A11yRole,
    pub name: String,
    pub description: Option<String>,
    pub state: A11yState,
    pub bounds: Rect,
    pub children: Vec<A11yNode>,
}

/// Complete accessibility tree for a document.
#[derive(Debug, Clone)]
pub struct A11yTree {
    pub root: Option<A11yNode>,
}

impl A11yTree {
    /// Builds an accessibility tree from the DOM document and the computed layout tree.
    pub fn build(doc: &Document, layout_root: Option<&LayoutBox>) -> Self {
        let mut next_id = 1;
        let root = build_node(doc, doc.root(), layout_root, &mut next_id);
        A11yTree { root }
    }

    /// Finds all nodes matching a specific role.
    pub fn find_by_role(&self, role: &A11yRole) -> Vec<&A11yNode> {
        let mut results = Vec::new();
        if let Some(ref r) = self.root {
            collect_by_role(r, role, &mut results);
        }
        results
    }

    /// Finds all nodes whose accessible name contains the given substring (case-insensitive).
    pub fn find_by_name(&self, name_substr: &str) -> Vec<&A11yNode> {
        let mut results = Vec::new();
        let lower = name_substr.to_ascii_lowercase();
        if let Some(ref r) = self.root {
            collect_by_name(r, &lower, &mut results);
        }
        results
    }

    /// Produces a formatted text dump of the accessibility tree for debugging and testing.
    pub fn dump_tree(&self) -> String {
        let mut out = String::new();
        if let Some(ref r) = self.root {
            dump_node(r, 0, &mut out);
        }
        out
    }
}

fn collect_by_role<'a>(node: &'a A11yNode, target_role: &A11yRole, acc: &mut Vec<&'a A11yNode>) {
    if &node.role == target_role {
        acc.push(node);
    }
    for child in &node.children {
        collect_by_role(child, target_role, acc);
    }
}

fn collect_by_name<'a>(node: &'a A11yNode, target_lower: &str, acc: &mut Vec<&'a A11yNode>) {
    if node.name.to_ascii_lowercase().contains(target_lower) {
        acc.push(node);
    }
    for child in &node.children {
        collect_by_name(child, target_lower, acc);
    }
}

fn dump_node(node: &A11yNode, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let role_str = match &node.role {
        A11yRole::Heading { level } => format!("Heading (level {level})"),
        other => format!("{other:?}"),
    };

    let name_part = if node.name.is_empty() {
        String::new()
    } else {
        format!(" \"{}\"", node.name)
    };

    let mut state_parts = Vec::new();
    if node.state.disabled {
        state_parts.push("disabled");
    }
    if let Some(c) = node.state.checked {
        state_parts.push(if c { "checked" } else { "unchecked" });
    }
    if let Some(e) = node.state.expanded {
        state_parts.push(if e { "expanded" } else { "collapsed" });
    }
    if let Some(ref v) = node.state.value {
        state_parts.push(v.as_str());
    }
    let state_str = if state_parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", state_parts.join(", "))
    };

    out.push_str(&format!("{indent}[{role_str}]{name_part}{state_str}\n"));
    for child in &node.children {
        dump_node(child, depth + 1, out);
    }
}

/// Recursively builds an `A11yNode` for a DOM node.
fn build_node(
    doc: &Document,
    node_id: NodeId,
    layout_root: Option<&LayoutBox>,
    next_id: &mut usize,
) -> Option<A11yNode> {
    let dom_node = doc.get(node_id)?;

    match &dom_node.data {
        NodeData::Document => {
            let mut children = Vec::new();
            for child in doc.children(node_id) {
                if let Some(child_a11y) = build_node(doc, child.id, layout_root, next_id) {
                    children.push(child_a11y);
                }
            }
            let id = *next_id;
            *next_id += 1;
            let title = doc
                .find_element_by_tag(doc.root(), "title")
                .map(|t| doc.text_content(t).trim().to_string())
                .unwrap_or_default();

            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role: A11yRole::RootWebArea,
                name: title,
                description: None,
                state: A11yState::default(),
                bounds: layout_root
                    .map(|b| b.dimensions.border_box())
                    .unwrap_or(Rect::ZERO),
                children,
            })
        }
        NodeData::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            let id = *next_id;
            *next_id += 1;
            let bounds = find_bounds_for_node(node_id, layout_root);
            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role: A11yRole::StaticText,
                name: trimmed.to_string(),
                description: None,
                state: A11yState::default(),
                bounds,
                children: Vec::new(),
            })
        }
        NodeData::Element(elem) => {
            // Check aria-hidden="true" or hidden attribute
            if elem.get_attribute("aria-hidden") == Some("true")
                || elem.get_attribute("hidden").is_some()
            {
                return None;
            }

            // Suppress non-rendered tags
            let tag = elem.tag_name.to_ascii_lowercase();
            if matches!(
                tag.as_str(),
                "head" | "script" | "style" | "meta" | "link" | "template"
            ) {
                return None;
            }

            let role = resolve_role(&tag, elem);
            let state = resolve_state(elem);
            let name = compute_accessible_name(doc, node_id, elem, &tag);
            let bounds = find_bounds_for_node(node_id, layout_root);

            let mut children = Vec::new();
            for child in doc.children(node_id) {
                if let Some(child_a11y) = build_node(doc, child.id, layout_root, next_id) {
                    children.push(child_a11y);
                }
            }

            let id = *next_id;
            *next_id += 1;

            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role,
                name,
                description: elem
                    .get_attribute("aria-description")
                    .or_else(|| elem.get_attribute("title"))
                    .map(|s| s.to_string()),
                state,
                bounds,
                children,
            })
        }
        _ => None,
    }
}

/// Resolves the accessibility role from explicit ARIA or implicit HTML tag semantics.
fn resolve_role(tag: &str, elem: &mango_html::dom::ElementData) -> A11yRole {
    // 1. Explicit ARIA role takes precedence
    if let Some(explicit_role) = elem.get_attribute("role") {
        let r = explicit_role.trim().to_ascii_lowercase();
        match r.as_str() {
            "banner" | "header" => return A11yRole::Header,
            "contentinfo" | "footer" => return A11yRole::Footer,
            "navigation" => return A11yRole::Navigation,
            "main" => return A11yRole::Main,
            "article" => return A11yRole::Article,
            "section" => return A11yRole::Section,
            "button" => return A11yRole::Button,
            "link" => return A11yRole::Link,
            "checkbox" => return A11yRole::Checkbox,
            "radio" => return A11yRole::RadioButton,
            "textbox" => return A11yRole::TextInput,
            "slider" => return A11yRole::Slider,
            "img" => return A11yRole::Image,
            "list" => return A11yRole::List,
            "listitem" => return A11yRole::ListItem,
            "table" => return A11yRole::Table,
            "row" => return A11yRole::Row,
            "cell" => return A11yRole::Cell,
            "columnheader" => return A11yRole::ColumnHeader,
            "rowheader" => return A11yRole::RowHeader,
            "dialog" => return A11yRole::Dialog,
            "alert" => return A11yRole::Alert,
            "tab" => return A11yRole::Tab,
            "tablist" => return A11yRole::TabList,
            "tabpanel" => return A11yRole::TabPanel,
            "combobox" => return A11yRole::ComboBox,
            "heading" => {
                let level = elem
                    .get_attribute("aria-level")
                    .and_then(|l| l.parse().ok())
                    .unwrap_or(2);
                return A11yRole::Heading { level };
            }
            _ => {}
        }
    }

    // 2. Implicit HTML role
    match tag {
        "header" => A11yRole::Header,
        "footer" => A11yRole::Footer,
        "nav" => A11yRole::Navigation,
        "main" => A11yRole::Main,
        "article" => A11yRole::Article,
        "section" => A11yRole::Section,
        "h1" => A11yRole::Heading { level: 1 },
        "h2" => A11yRole::Heading { level: 2 },
        "h3" => A11yRole::Heading { level: 3 },
        "h4" => A11yRole::Heading { level: 4 },
        "h5" => A11yRole::Heading { level: 5 },
        "h6" => A11yRole::Heading { level: 6 },
        "button" => A11yRole::Button,
        "a" => {
            if elem.get_attribute("href").is_some() {
                A11yRole::Link
            } else {
                A11yRole::Generic
            }
        }
        "img" => A11yRole::Image,
        "ul" | "ol" => A11yRole::List,
        "li" => A11yRole::ListItem,
        "table" => A11yRole::Table,
        "tr" => A11yRole::Row,
        "td" => A11yRole::Cell,
        "th" => A11yRole::ColumnHeader,
        "dialog" => A11yRole::Dialog,
        "input" => {
            let input_type = elem
                .get_attribute("type")
                .unwrap_or("text")
                .to_ascii_lowercase();
            match input_type.as_str() {
                "button" | "submit" | "reset" => A11yRole::Button,
                "checkbox" => A11yRole::Checkbox,
                "radio" => A11yRole::RadioButton,
                "range" => A11yRole::Slider,
                _ => A11yRole::TextInput,
            }
        }
        "textarea" => A11yRole::TextInput,
        "select" => A11yRole::ComboBox,
        _ => A11yRole::Generic,
    }
}

/// Resolves accessible state and properties from attributes.
fn resolve_state(elem: &mango_html::dom::ElementData) -> A11yState {
    let mut state = A11yState::default();

    state.disabled = elem.get_attribute("disabled").is_some()
        || elem.get_attribute("aria-disabled") == Some("true");
    state.readonly = elem.get_attribute("readonly").is_some()
        || elem.get_attribute("aria-readonly") == Some("true");
    state.required = elem.get_attribute("required").is_some()
        || elem.get_attribute("aria-required") == Some("true");

    if let Some(v) = elem.get_attribute("aria-expanded") {
        state.expanded = Some(v == "true");
    }

    if let Some(v) = elem.get_attribute("aria-checked") {
        state.checked = Some(v == "true");
    } else if elem.get_attribute("checked").is_some() {
        state.checked = Some(true);
    }

    if let Some(v) = elem.get_attribute("aria-selected") {
        state.selected = Some(v == "true");
    }

    if let Some(v) = elem.get_attribute("value") {
        state.value = Some(v.to_string());
    }

    state.value_min = elem
        .get_attribute("aria-valuemin")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("min").and_then(|v| v.parse().ok()));
    state.value_max = elem
        .get_attribute("aria-valuemax")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("max").and_then(|v| v.parse().ok()));
    state.value_now = elem
        .get_attribute("aria-valuenow")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("value").and_then(|v| v.parse().ok()));

    state
}

/// Computes the accessible name according to W3C AccName computation rules.
fn compute_accessible_name(
    doc: &Document,
    node_id: NodeId,
    elem: &mango_html::dom::ElementData,
    tag: &str,
) -> String {
    // 1. aria-label has highest precedence
    if let Some(label) = elem.get_attribute("aria-label") {
        let t = label.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }

    // 2. aria-labelledby references other element IDs
    if let Some(labelled_by) = elem.get_attribute("aria-labelledby") {
        let mut names = Vec::new();
        for id in labelled_by.split_whitespace() {
            if let Some(referenced_node_id) = doc.find_element_by_id(doc.root(), id) {
                let text = doc.text_content(referenced_node_id);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    names.push(trimmed.to_string());
                }
            }
        }
        if !names.is_empty() {
            return names.join(" ");
        }
    }

    // 3. Native element-specific name rules
    match tag {
        "img" => {
            if let Some(alt) = elem.get_attribute("alt") {
                return alt.trim().to_string();
            }
        }
        "input" => {
            let input_type = elem
                .get_attribute("type")
                .unwrap_or("text")
                .to_ascii_lowercase();
            if matches!(input_type.as_str(), "button" | "submit" | "reset")
                && let Some(val) = elem.get_attribute("value")
            {
                return val.trim().to_string();
            }
            if let Some(placeholder) = elem.get_attribute("placeholder") {
                return placeholder.trim().to_string();
            }
            // Check associated <label for="...">
            if let Some(id) = elem.id()
                && let Some(label_text) = find_label_for_id(doc, doc.root(), id)
            {
                return label_text;
            }
        }
        "button" | "a" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let text = doc.text_content(node_id);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        _ => {}
    }

    // 4. Fallback to title attribute
    if let Some(title) = elem.get_attribute("title") {
        return title.trim().to_string();
    }

    String::new()
}

/// Finds text of a `<label for="...">` matching an input ID.
fn find_label_for_id(doc: &Document, root: NodeId, target_id: &str) -> Option<String> {
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

/// Finds the layout bounding box for a given DOM node ID.
fn find_bounds_for_node(node_id: NodeId, layout_root: Option<&LayoutBox>) -> Rect {
    if let Some(root) = layout_root
        && let Some(layout_box) = root.find_box_for_node(node_id)
    {
        return layout_box.dimensions.border_box();
    }
    Rect::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_tree_roles_and_accname() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>A11y Test</title></head>
            <body>
                <header>
                    <nav aria-label="Main Navigation">
                        <a href="/home">Home</a>
                        <a href="/about">About Us</a>
                    </nav>
                </header>
                <main>
                    <h1>Page Heading</h1>
                    <button aria-label="Close dialog">X</button>
                    <img src="pic.jpg" alt="A lovely sunrise" />
                    <input type="checkbox" id="tos" checked />
                    <label for="tos">Agree to Terms</label>
                    <div role="slider" aria-valuenow="50" aria-valuemin="0" aria-valuemax="100"></div>
                    <div aria-hidden="true">Hidden text</div>
                </main>
            </body>
            </html>
        "#;

        let doc = mango_html::parse_html(html);
        let a11y_tree = A11yTree::build(&doc, None);

        // Find by roles
        let navs = a11y_tree.find_by_role(&A11yRole::Navigation);
        assert_eq!(navs.len(), 1);
        assert_eq!(navs[0].name, "Main Navigation");

        let links = a11y_tree.find_by_role(&A11yRole::Link);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].name, "Home");
        assert_eq!(links[1].name, "About Us");

        let headings = a11y_tree.find_by_role(&A11yRole::Heading { level: 1 });
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].name, "Page Heading");

        let buttons = a11y_tree.find_by_role(&A11yRole::Button);
        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].name, "Close dialog");

        let images = a11y_tree.find_by_role(&A11yRole::Image);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].name, "A lovely sunrise");

        let checkboxes = a11y_tree.find_by_role(&A11yRole::Checkbox);
        assert_eq!(checkboxes.len(), 1);
        assert_eq!(checkboxes[0].name, "Agree to Terms");
        assert_eq!(checkboxes[0].state.checked, Some(true));

        let sliders = a11y_tree.find_by_role(&A11yRole::Slider);
        assert_eq!(sliders.len(), 1);
        assert_eq!(sliders[0].state.value_now, Some(50.0));
        assert_eq!(sliders[0].state.value_min, Some(0.0));
        assert_eq!(sliders[0].state.value_max, Some(100.0));

        // Hidden content should not be present
        let hidden = a11y_tree.find_by_name("Hidden text");
        assert!(
            hidden.is_empty(),
            "aria-hidden elements must not appear in the accessibility tree"
        );

        // Dump tree check
        let dump = a11y_tree.dump_tree();
        assert!(dump.contains("[Navigation] \"Main Navigation\""));
        assert!(dump.contains("[Heading (level 1)] \"Page Heading\""));
        assert!(dump.contains("[Checkbox] \"Agree to Terms\" [checked]"));
    }
}
