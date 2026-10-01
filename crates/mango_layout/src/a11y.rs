//! # Accessibility (A11y) Tree & ARIA Engine (GAP-020)
//!
//! Converts DOM elements and layout boxes into an accessible tree conforming to
//! W3C WAI-ARIA 1.2 and the HTML Accessibility API Mappings (AAM).
//!
//! Provides screen-reader semantics, accessible name computation (AccName 1.2),
//! role resolution (implicit HTML + explicit ARIA), state extraction, and
//! spatial bounding boxes for assistive technologies.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;

use crate::box_tree::LayoutBox;
use mango_core::Rect;
use mango_html::dom::{Document, NodeData, NodeId};

// ─── Roles ───────────────────────────────────────────────────────────────────

/// ARIA and HTML standard accessibility roles.
///
/// Covers the WAI-ARIA 1.2 taxonomy and HTML-AAM implicit role mappings.
/// Missing exotic roles (e.g. `application`, `document`, `toolbar`) can be
/// added as needed without breaking consumers — the `Generic` fallback covers
/// any unmapped role string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum A11yRole {
    // Document structure
    RootWebArea,
    Generic,
    /// `role="presentation"` or `role="none"` — semantics removed, children kept.
    Presentation,

    // Landmark roles
    Header,
    Footer,
    Navigation,
    Main,
    Complementary,
    Form,
    Search,
    Region,

    // Sectioning / Grouping
    Article,
    Section,
    Heading { level: u8 },
    Group,
    Blockquote,
    Figure,
    FigCaption,

    // Interactive widgets
    Button,
    Link,
    Checkbox,
    RadioButton,
    TextInput,
    Slider,
    SpinButton,
    Switch,
    ComboBox,
    ListBox,
    Option,

    // Images
    Image,

    // Lists
    List,
    ListItem,

    // Tables
    Table,
    Row,
    Cell,
    ColumnHeader,
    RowHeader,

    // Dialogs
    Dialog,
    Alert,
    AlertDialog,

    // Tabs
    Tab,
    TabList,
    TabPanel,

    // Menus
    Menu,
    MenuBar,
    MenuItem,
    MenuItemCheckBox,
    MenuItemRadio,

    // Trees
    Tree,
    TreeItem,

    // Live regions
    Status,
    Log,
    Marquee,
    Timer,

    // Misc
    ProgressBar,
    Meter,
    Separator,
    Tooltip,
    Feed,
    Math,
    Note,
    Definition,
    Term,
    Code,
    Emphasis,
    Strong,
    Subscript,
    Superscript,

    // Text leaf
    StaticText,
}

// ─── States / Properties ─────────────────────────────────────────────────────

/// Accessible states and properties mapped from both ARIA attributes and native
/// HTML attributes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11yState {
    pub disabled: bool,
    pub hidden: bool,
    pub expanded: Option<bool>,
    pub checked: Option<CheckedState>,
    pub pressed: Option<PressedState>,
    pub selected: Option<bool>,
    pub readonly: bool,
    pub required: bool,
    pub focusable: bool,
    pub invalid: Option<String>,
    pub busy: bool,
    pub modal: bool,
    pub has_popup: Option<String>,
    pub autocomplete: Option<String>,
    pub current: Option<String>,
    pub orientation: Option<String>,
    pub multiselectable: bool,
    pub multiline: bool,

    // Value properties
    pub value: Option<String>,
    pub value_text: Option<String>,
    pub value_min: Option<f32>,
    pub value_max: Option<f32>,
    pub value_now: Option<f32>,

    // Live region properties
    pub live: Option<String>,
    pub atomic: Option<bool>,
    pub relevant: Option<String>,

    // Relationship properties (stored as raw ID strings)
    pub controls: Vec<String>,
    pub owns: Vec<String>,
    pub flowto: Vec<String>,
    pub active_descendant: Option<String>,
    pub error_message: Option<String>,

    // Set/position properties
    pub pos_in_set: Option<u32>,
    pub set_size: Option<u32>,
    pub level: Option<u32>,

    // Table properties
    pub col_count: Option<u32>,
    pub col_index: Option<u32>,
    pub col_span: Option<u32>,
    pub row_count: Option<u32>,
    pub row_index: Option<u32>,
    pub row_span: Option<u32>,

    // Custom role description
    pub role_description: Option<String>,
    pub key_shortcuts: Option<String>,
}

/// Tri-state checked (for checkboxes).
#[derive(Debug, Clone, PartialEq)]
pub enum CheckedState {
    True,
    False,
    Mixed,
}

/// Tri-state pressed (for toggle buttons).
#[derive(Debug, Clone, PartialEq)]
pub enum PressedState {
    True,
    False,
    Mixed,
}

// ─── A11y Node & Tree ────────────────────────────────────────────────────────

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
    ///
    /// Pre-builds a bounds cache from the layout tree so that per-node lookups
    /// are O(1) instead of O(n).
    pub fn build(doc: &Document, layout_root: Option<&LayoutBox>) -> Self {
        let mut next_id = 1;

        // O3: Pre-build bounds cache for O(1) lookups
        let mut bounds_cache = HashMap::new();
        if let Some(root) = layout_root {
            build_bounds_cache(root, &mut bounds_cache);
        }

        let root = build_node(doc, doc.root(), layout_root, &bounds_cache, &mut next_id);
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

    /// Finds a node by its DOM node ID.
    pub fn find_by_node_id(&self, target: NodeId) -> Option<&A11yNode> {
        self.root.as_ref().and_then(|r| find_by_node_id_rec(r, target))
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

fn find_by_node_id_rec(node: &A11yNode, target: NodeId) -> Option<&A11yNode> {
    if node.node_id == Some(target) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_by_node_id_rec(child, target) {
            return Some(found);
        }
    }
    None
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

// O4: Use fmt::Write for zero-alloc dump
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
        state_parts.push("disabled".to_string());
    }
    if node.state.busy {
        state_parts.push("busy".to_string());
    }
    if node.state.modal {
        state_parts.push("modal".to_string());
    }
    if let Some(ref c) = node.state.checked {
        state_parts.push(match c {
            CheckedState::True => "checked".to_string(),
            CheckedState::False => "unchecked".to_string(),
            CheckedState::Mixed => "mixed".to_string(),
        });
    }
    if let Some(ref p) = node.state.pressed {
        state_parts.push(match p {
            PressedState::True => "pressed".to_string(),
            PressedState::False => "not pressed".to_string(),
            PressedState::Mixed => "partially pressed".to_string(),
        });
    }
    if let Some(e) = node.state.expanded {
        state_parts.push(if e { "expanded".to_string() } else { "collapsed".to_string() });
    }
    if let Some(ref v) = node.state.value {
        state_parts.push(format!("value={v}"));
    }
    if let Some(ref live) = node.state.live {
        state_parts.push(format!("live={live}"));
    }
    let state_str = if state_parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", state_parts.join(", "))
    };

    let _ = writeln!(out, "{indent}[{role_str}]{name_part}{state_str}");
    for child in &node.children {
        dump_node(child, depth + 1, out);
    }
}

// ─── Bounds Cache ────────────────────────────────────────────────────────────

/// Pre-builds a HashMap<NodeId, Rect> from the layout tree for O(1) bounds lookups.
fn build_bounds_cache(layout_box: &LayoutBox, cache: &mut HashMap<NodeId, Rect>) {
    if let Some(node_id) = layout_box.node_id {
        cache.insert(node_id, layout_box.dimensions.border_box());
    }
    for child in &layout_box.children {
        build_bounds_cache(child, cache);
    }
}

// ─── Tree Construction ───────────────────────────────────────────────────────

/// Recursively builds an `A11yNode` for a DOM node.
fn build_node(
    doc: &Document,
    node_id: NodeId,
    layout_root: Option<&LayoutBox>,
    bounds_cache: &HashMap<NodeId, Rect>,
    next_id: &mut usize,
) -> Option<A11yNode> {
    let dom_node = doc.get(node_id)?;

    match &dom_node.data {
        NodeData::Document => {
            let mut children = Vec::new();
            for child in doc.children(node_id) {
                if let Some(child_a11y) =
                    build_node(doc, child.id, layout_root, bounds_cache, next_id)
                {
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
            let bounds = bounds_cache.get(&node_id).copied().unwrap_or(Rect::ZERO);
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
                "head" | "script" | "style" | "meta" | "link" | "template" | "noscript"
            ) {
                return None;
            }

            // B7: <input type="hidden"> is excluded from the a11y tree
            if tag == "input"
                && elem
                    .get_attribute("type")
                    .map(|t| t.eq_ignore_ascii_case("hidden"))
                    .unwrap_or(false)
            {
                return None;
            }

            // G3: If a layout tree is available and this element has no box,
            // it's display:none — exclude it from the a11y tree.
            if layout_root.is_some() && !bounds_cache.contains_key(&node_id) {
                // Exception: elements with aria-hidden="false" should not be
                // excluded by display:none (spec edge case). Currently we've
                // already returned None for aria-hidden="true" above, and
                // aria-hidden="false" is meant to override ancestor hiding, but
                // a display:none element without aria-hidden should be excluded.
                return None;
            }

            let role = resolve_role(&tag, elem, doc, node_id);
            let state = resolve_state(elem, &tag);
            let name = compute_accessible_name(doc, node_id, elem, &tag, &mut HashSet::new());
            let bounds = bounds_cache
                .get(&node_id)
                .copied()
                .unwrap_or(Rect::ZERO);

            // G4: role="presentation"/"none" — flatten children into parent
            if role == A11yRole::Presentation {
                let mut children = Vec::new();
                for child in doc.children(node_id) {
                    if let Some(child_a11y) =
                        build_node(doc, child.id, layout_root, bounds_cache, next_id)
                    {
                        children.push(child_a11y);
                    }
                }
                // Return children directly by wrapping them — caller will
                // flatten. For now we emit a Generic wrapper since our tree is
                // recursive and we can't return multiple nodes. The Presentation
                // role signals to consumers to ignore this node's semantics.
                if children.is_empty() {
                    return None;
                }
                let id = *next_id;
                *next_id += 1;
                return Some(A11yNode {
                    id,
                    node_id: Some(node_id),
                    role: A11yRole::Presentation,
                    name: String::new(),
                    description: None,
                    state: A11yState::default(),
                    bounds,
                    children,
                });
            }

            let mut children = Vec::new();
            for child in doc.children(node_id) {
                if let Some(child_a11y) =
                    build_node(doc, child.id, layout_root, bounds_cache, next_id)
                {
                    children.push(child_a11y);
                }
            }

            let id = *next_id;
            *next_id += 1;

            // G10: aria-describedby
            let description = compute_description(doc, elem);

            Some(A11yNode {
                id,
                node_id: Some(node_id),
                role,
                name,
                description,
                state,
                bounds,
                children,
            })
        }
        _ => None,
    }
}

// ─── Role Resolution ─────────────────────────────────────────────────────────

/// Resolves the accessibility role from explicit ARIA or implicit HTML tag semantics.
fn resolve_role(
    tag: &str,
    elem: &mango_html::dom::ElementData,
    doc: &Document,
    node_id: NodeId,
) -> A11yRole {
    // 1. Explicit ARIA role takes precedence
    if let Some(explicit_role) = elem.get_attribute("role") {
        // ARIA spec: take the first valid token from the space-separated list
        let r = explicit_role.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        if let Some(role) = match_aria_role(&r, elem) {
            return role;
        }
        // Unknown role string falls through to implicit mapping
    }

    // 2. Implicit HTML role
    match tag {
        "header" => {
            // B3: Only landmark when not inside article/aside/main/nav/section
            if is_inside_sectioning(doc, node_id) {
                A11yRole::Generic
            } else {
                A11yRole::Header
            }
        }
        "footer" => {
            // B3: Only landmark when not inside article/aside/main/nav/section
            if is_inside_sectioning(doc, node_id) {
                A11yRole::Generic
            } else {
                A11yRole::Footer
            }
        }
        "nav" => A11yRole::Navigation,
        "main" => A11yRole::Main,
        "aside" => A11yRole::Complementary,
        "article" => A11yRole::Article,
        "section" => {
            // HTML-AAM: <section> with accessible name → region, else generic
            if elem.get_attribute("aria-label").is_some()
                || elem.get_attribute("aria-labelledby").is_some()
            {
                A11yRole::Region
            } else {
                A11yRole::Section
            }
        }
        "form" => {
            // HTML-AAM: <form> with accessible name → form landmark, else generic
            if elem.get_attribute("aria-label").is_some()
                || elem.get_attribute("aria-labelledby").is_some()
            {
                A11yRole::Form
            } else {
                A11yRole::Generic
            }
        }
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
        "img" => {
            // HTML-AAM: <img alt=""> with empty alt → presentation
            if elem.get_attribute("alt") == Some("") {
                A11yRole::Presentation
            } else {
                A11yRole::Image
            }
        }
        "ul" | "ol" => A11yRole::List,
        "li" => A11yRole::ListItem,
        "table" => A11yRole::Table,
        "tr" => A11yRole::Row,
        "td" => A11yRole::Cell,
        "th" => {
            // B2: Check scope attribute for row/column header
            match elem
                .get_attribute("scope")
                .map(|s| s.to_ascii_lowercase())
                .as_deref()
            {
                Some("row") | Some("rowgroup") => A11yRole::RowHeader,
                _ => A11yRole::ColumnHeader,
            }
        }
        "dialog" => A11yRole::Dialog,
        "input" => {
            let input_type = elem
                .get_attribute("type")
                .unwrap_or("text")
                .to_ascii_lowercase();
            match input_type.as_str() {
                "button" | "submit" | "reset" | "image" => A11yRole::Button, // B6: input[type=image]
                "checkbox" => A11yRole::Checkbox,
                "radio" => A11yRole::RadioButton,
                "range" => A11yRole::Slider,
                "number" => A11yRole::SpinButton,
                "search" => A11yRole::TextInput, // could be SearchBox in future
                "email" | "tel" | "url" | "text" | "password" => A11yRole::TextInput,
                _ => A11yRole::TextInput,
            }
        }
        "textarea" => A11yRole::TextInput,
        "select" => {
            // B5: <select multiple> or <select size="N"> where N>1 → listbox
            let is_multiple = elem.get_attribute("multiple").is_some();
            let size_gt_1 = elem
                .get_attribute("size")
                .and_then(|s| s.parse::<u32>().ok())
                .map(|s| s > 1)
                .unwrap_or(false);
            if is_multiple || size_gt_1 {
                A11yRole::ListBox
            } else {
                A11yRole::ComboBox
            }
        }
        "option" => A11yRole::Option,
        "optgroup" => A11yRole::Group,
        "fieldset" => A11yRole::Group,
        "output" => A11yRole::Status, // G12
        "progress" => A11yRole::ProgressBar,
        "meter" => A11yRole::Meter,
        "hr" => A11yRole::Separator,
        "details" => A11yRole::Group, // G13
        "summary" => A11yRole::Button, // G13: disclosure trigger
        "figure" => A11yRole::Figure,
        "figcaption" => A11yRole::FigCaption,
        "blockquote" => A11yRole::Blockquote,
        "dfn" => A11yRole::Term,
        "dt" => A11yRole::Term,
        "dd" => A11yRole::Definition,
        "code" => A11yRole::Code,
        "em" => A11yRole::Emphasis,
        "strong" => A11yRole::Strong,
        "sub" => A11yRole::Subscript,
        "sup" => A11yRole::Superscript,
        "abbr" => A11yRole::Generic, // no dedicated role, but keeps title as description
        "label" => A11yRole::Generic, // labels are associators, not landmarks
        _ => A11yRole::Generic,
    }
}

/// Matches an explicit ARIA role string to our enum.
fn match_aria_role(r: &str, elem: &mango_html::dom::ElementData) -> Option<A11yRole> {
    Some(match r {
        "banner" | "header" => A11yRole::Header,
        "contentinfo" | "footer" => A11yRole::Footer,
        "navigation" => A11yRole::Navigation,
        "main" => A11yRole::Main,
        "complementary" => A11yRole::Complementary,
        "form" => A11yRole::Form,
        "search" => A11yRole::Search,
        "region" => A11yRole::Region,
        "article" => A11yRole::Article,
        "section" => A11yRole::Section,
        "group" => A11yRole::Group,
        "button" => A11yRole::Button,
        "link" => A11yRole::Link,
        "checkbox" => A11yRole::Checkbox,
        "radio" => A11yRole::RadioButton,
        "textbox" => A11yRole::TextInput,
        "slider" => A11yRole::Slider,
        "spinbutton" => A11yRole::SpinButton,
        "switch" => A11yRole::Switch,
        "img" => A11yRole::Image,
        "list" => A11yRole::List,
        "listitem" => A11yRole::ListItem,
        "listbox" => A11yRole::ListBox,
        "option" => A11yRole::Option,
        "table" => A11yRole::Table,
        "row" => A11yRole::Row,
        "cell" => A11yRole::Cell,
        "columnheader" => A11yRole::ColumnHeader,
        "rowheader" => A11yRole::RowHeader,
        "grid" => A11yRole::Table, // grid maps to table-like
        "gridcell" => A11yRole::Cell,
        "dialog" => A11yRole::Dialog,
        "alertdialog" => A11yRole::AlertDialog,
        "alert" => A11yRole::Alert,
        "tab" => A11yRole::Tab,
        "tablist" => A11yRole::TabList,
        "tabpanel" => A11yRole::TabPanel,
        "combobox" => A11yRole::ComboBox,
        "menu" => A11yRole::Menu,
        "menubar" => A11yRole::MenuBar,
        "menuitem" => A11yRole::MenuItem,
        "menuitemcheckbox" => A11yRole::MenuItemCheckBox,
        "menuitemradio" => A11yRole::MenuItemRadio,
        "tree" => A11yRole::Tree,
        "treeitem" => A11yRole::TreeItem,
        "status" => A11yRole::Status,
        "log" => A11yRole::Log,
        "marquee" => A11yRole::Marquee,
        "timer" => A11yRole::Timer,
        "progressbar" => A11yRole::ProgressBar,
        "meter" => A11yRole::Meter,
        "separator" => A11yRole::Separator,
        "tooltip" => A11yRole::Tooltip,
        "feed" => A11yRole::Feed,
        "math" => A11yRole::Math,
        "note" => A11yRole::Note,
        "definition" => A11yRole::Definition,
        "term" => A11yRole::Term,
        "figure" => A11yRole::Figure,
        "blockquote" => A11yRole::Blockquote,
        "code" => A11yRole::Code,
        "emphasis" => A11yRole::Emphasis,
        "strong" => A11yRole::Strong,
        "subscript" => A11yRole::Subscript,
        "superscript" => A11yRole::Superscript,
        "presentation" | "none" => A11yRole::Presentation, // G4
        "heading" => {
            let level = elem
                .get_attribute("aria-level")
                .and_then(|l| l.parse().ok())
                .unwrap_or(2);
            A11yRole::Heading { level }
        }
        _ => return None,
    })
}

/// B3: Checks whether a node is nested inside a sectioning content element
/// (`<article>`, `<aside>`, `<main>`, `<nav>`, `<section>`).
fn is_inside_sectioning(doc: &Document, node_id: NodeId) -> bool {
    let mut current = node_id;
    loop {
        let Some(node) = doc.get(current) else {
            return false;
        };
        let Some(parent_id) = node.parent else {
            return false;
        };
        let Some(parent_node) = doc.get(parent_id) else {
            return false;
        };
        if let NodeData::Element(pelem) = &parent_node.data {
            let ptag = pelem.tag_name.to_ascii_lowercase();
            if matches!(
                ptag.as_str(),
                "article" | "aside" | "main" | "nav" | "section"
            ) {
                return true;
            }
        }
        if matches!(parent_node.data, NodeData::Document) {
            return false;
        }
        current = parent_id;
    }
}

// ─── State Resolution ────────────────────────────────────────────────────────

/// Resolves accessible state and properties from attributes.
fn resolve_state(elem: &mango_html::dom::ElementData, tag: &str) -> A11yState {
    let mut state = A11yState::default();

    // Disabled
    state.disabled = elem.get_attribute("disabled").is_some()
        || elem.get_attribute("aria-disabled") == Some("true");

    // Read-only
    state.readonly = elem.get_attribute("readonly").is_some()
        || elem.get_attribute("aria-readonly") == Some("true");

    // Required
    state.required = elem.get_attribute("required").is_some()
        || elem.get_attribute("aria-required") == Some("true");

    // Busy
    state.busy = elem.get_attribute("aria-busy") == Some("true");

    // Modal
    state.modal = elem.get_attribute("aria-modal") == Some("true");

    // Multiselectable
    state.multiselectable = elem.get_attribute("aria-multiselectable") == Some("true")
        || (tag == "select" && elem.get_attribute("multiple").is_some());

    // Multiline
    state.multiline = elem.get_attribute("aria-multiline") == Some("true") || tag == "textarea";

    // Expanded (also handles <details open>)
    if let Some(v) = elem.get_attribute("aria-expanded") {
        state.expanded = Some(v == "true");
    } else if tag == "details" {
        state.expanded = Some(elem.get_attribute("open").is_some());
    }

    // Checked (tri-state: true/false/mixed)
    if let Some(v) = elem.get_attribute("aria-checked") {
        state.checked = Some(match v {
            "true" => CheckedState::True,
            "mixed" => CheckedState::Mixed,
            _ => CheckedState::False,
        });
    } else if elem.get_attribute("checked").is_some() {
        state.checked = Some(CheckedState::True);
    }

    // Pressed (tri-state for toggle buttons)
    if let Some(v) = elem.get_attribute("aria-pressed") {
        state.pressed = Some(match v {
            "true" => PressedState::True,
            "mixed" => PressedState::Mixed,
            _ => PressedState::False,
        });
    }

    // Selected
    if let Some(v) = elem.get_attribute("aria-selected") {
        state.selected = Some(v == "true");
    } else if tag == "option" && elem.get_attribute("selected").is_some() {
        state.selected = Some(true);
    }

    // Invalid
    if let Some(v) = elem.get_attribute("aria-invalid") {
        if v != "false" {
            state.invalid = Some(v.to_string());
        }
    }

    // Has popup
    if let Some(v) = elem.get_attribute("aria-haspopup") {
        if v != "false" {
            state.has_popup = Some(v.to_string());
        }
    }

    // Autocomplete
    if let Some(v) = elem
        .get_attribute("aria-autocomplete")
        .or_else(|| elem.get_attribute("autocomplete"))
    {
        state.autocomplete = Some(v.to_string());
    }

    // Current
    if let Some(v) = elem.get_attribute("aria-current") {
        if v != "false" {
            state.current = Some(v.to_string());
        }
    }

    // Orientation
    state.orientation = elem
        .get_attribute("aria-orientation")
        .map(|s| s.to_string());

    // Value (native + ARIA)
    if let Some(v) = elem.get_attribute("value") {
        state.value = Some(v.to_string());
    }
    state.value_text = elem
        .get_attribute("aria-valuetext")
        .map(|s| s.to_string());
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

    // Live region properties
    state.live = elem.get_attribute("aria-live").map(|s| s.to_string());
    if let Some(v) = elem.get_attribute("aria-atomic") {
        state.atomic = Some(v == "true");
    }
    state.relevant = elem
        .get_attribute("aria-relevant")
        .map(|s| s.to_string());

    // Relationship IDs
    state.controls = split_ids(elem.get_attribute("aria-controls"));
    state.owns = split_ids(elem.get_attribute("aria-owns"));
    state.flowto = split_ids(elem.get_attribute("aria-flowto"));
    state.active_descendant = elem
        .get_attribute("aria-activedescendant")
        .map(|s| s.to_string());
    state.error_message = elem
        .get_attribute("aria-errormessage")
        .map(|s| s.to_string());

    // Set/position
    state.pos_in_set = elem
        .get_attribute("aria-posinset")
        .and_then(|v| v.parse().ok());
    state.set_size = elem
        .get_attribute("aria-setsize")
        .and_then(|v| v.parse().ok());
    state.level = elem
        .get_attribute("aria-level")
        .and_then(|v| v.parse().ok());

    // Table properties
    state.col_count = elem
        .get_attribute("aria-colcount")
        .and_then(|v| v.parse().ok());
    state.col_index = elem
        .get_attribute("aria-colindex")
        .and_then(|v| v.parse().ok());
    state.col_span = elem
        .get_attribute("aria-colspan")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("colspan").and_then(|v| v.parse().ok()));
    state.row_count = elem
        .get_attribute("aria-rowcount")
        .and_then(|v| v.parse().ok());
    state.row_index = elem
        .get_attribute("aria-rowindex")
        .and_then(|v| v.parse().ok());
    state.row_span = elem
        .get_attribute("aria-rowspan")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("rowspan").and_then(|v| v.parse().ok()));

    // Role description & key shortcuts
    state.role_description = elem
        .get_attribute("aria-roledescription")
        .map(|s| s.to_string());
    state.key_shortcuts = elem
        .get_attribute("aria-keyshortcuts")
        .map(|s| s.to_string());

    // Focusable heuristic
    state.focusable = is_natively_focusable(tag, elem)
        || elem.get_attribute("tabindex").is_some();

    state
}

/// Determines if an element is natively focusable (without tabindex).
fn is_natively_focusable(tag: &str, elem: &mango_html::dom::ElementData) -> bool {
    match tag {
        "a" | "area" => elem.get_attribute("href").is_some(),
        "button" | "textarea" | "select" | "details" | "summary" => true,
        "input" => {
            let t = elem.get_attribute("type").unwrap_or("text");
            !t.eq_ignore_ascii_case("hidden")
        }
        _ => false,
    }
}

/// Splits a space-separated ID reference list into a Vec of Strings.
fn split_ids(attr: Option<&str>) -> Vec<String> {
    attr.map(|s| {
        s.split_whitespace()
            .map(|id| id.to_string())
            .collect()
    })
    .unwrap_or_default()
}

// ─── Accessible Name Computation (AccName 1.2) ──────────────────────────────

/// Computes the accessible name according to W3C AccName 1.2 computation rules.
///
/// B1 fix: `aria-labelledby` (step 2A) is checked BEFORE `aria-label` (step 2D).
/// B4 fix: Uses a `visited` set to prevent infinite recursion cycles.
fn compute_accessible_name(
    doc: &Document,
    node_id: NodeId,
    elem: &mango_html::dom::ElementData,
    tag: &str,
    visited: &mut HashSet<NodeId>,
) -> String {
    // Prevent infinite recursion (B4)
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
                    // Don't recurse into already-visited nodes
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
        "textarea" | "select" => {
            // G8, G9: These elements also need <label for> and placeholder support
            compute_labelable_name(doc, node_id, elem, tag)
        }
        "button" | "a" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "summary" | "output"
        | "th" | "td" | "li" | "option" | "legend" | "caption" | "dt" | "dd" | "figcaption" => {
            let text = doc.text_content(node_id);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
            String::new()
        }
        "fieldset" => {
            // G6: <legend> provides accessible name for <fieldset>
            if let Some(legend_text) = find_child_element_text(doc, node_id, "legend") {
                return legend_text;
            }
            String::new()
        }
        "table" => {
            // G7: <caption> provides accessible name for <table>
            if let Some(caption_text) = find_child_element_text(doc, node_id, "caption") {
                return caption_text;
            }
            String::new()
        }
        "figure" => {
            // <figcaption> provides accessible name for <figure>
            if let Some(figcap_text) = find_child_element_text(doc, node_id, "figcaption") {
                return figcap_text;
            }
            String::new()
        }
        "details" => {
            // <summary> provides accessible name for <details>
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
fn compute_input_name(
    doc: &Document,
    node_id: NodeId,
    elem: &mango_html::dom::ElementData,
) -> String {
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
    if let Some(id) = elem.id() {
        if let Some(label_text) = find_label_for_id(doc, doc.root(), id) {
            return label_text;
        }
    }

    // G5: Try implicit label (wrapping <label>)
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

/// G8/G9: Computes accessible name for labelable elements (<textarea>, <select>).
fn compute_labelable_name(
    doc: &Document,
    node_id: NodeId,
    elem: &mango_html::dom::ElementData,
    tag: &str,
) -> String {
    // Try explicit <label for="...">
    if let Some(id) = elem.id() {
        if let Some(label_text) = find_label_for_id(doc, doc.root(), id) {
            return label_text;
        }
    }

    // G5: Try implicit label (wrapping <label>)
    if let Some(label_text) = find_wrapping_label(doc, node_id) {
        return label_text;
    }

    // Placeholder fallback (textarea supports placeholder)
    if tag == "textarea" {
        if let Some(placeholder) = elem.get_attribute("placeholder") {
            let t = placeholder.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
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

/// G5: Finds an implicit wrapping `<label>` ancestor and returns its text.
fn find_wrapping_label(doc: &Document, node_id: NodeId) -> Option<String> {
    let mut current = node_id;
    loop {
        let node = doc.get(current)?;
        let parent_id = node.parent?;
        let parent_node = doc.get(parent_id)?;
        if let NodeData::Element(pelem) = &parent_node.data {
            if pelem.tag_name.eq_ignore_ascii_case("label") {
                // This is a wrapping label — get its text content
                // (excluding the input element's own text contribution,
                // but text_content already handles that since inputs have no text children)
                let text = doc.text_content(parent_id);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        if matches!(parent_node.data, NodeData::Document) {
            return None;
        }
        current = parent_id;
    }
}

/// Finds the text of the first direct child element with the given tag name.
fn find_child_element_text(doc: &Document, parent_id: NodeId, child_tag: &str) -> Option<String> {
    for child in doc.children(parent_id) {
        if let NodeData::Element(elem) = &child.data {
            if elem.tag_name.eq_ignore_ascii_case(child_tag) {
                let text = doc.text_content(child.id);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

// ─── Description Computation ─────────────────────────────────────────────────

/// G10: Computes the accessible description from `aria-describedby`, `aria-description`, or `title`.
fn compute_description(
    doc: &Document,
    elem: &mango_html::dom::ElementData,
) -> Option<String> {
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

// ─── Tests ───────────────────────────────────────────────────────────────────

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
        assert_eq!(checkboxes[0].state.checked, Some(CheckedState::True));

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

    #[test]
    fn test_aria_labelledby_precedence_over_aria_label() {
        // B1: aria-labelledby should take precedence over aria-label
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <span id="lbl">From Labelledby</span>
                <button aria-label="From Label" aria-labelledby="lbl">X</button>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let buttons = tree.find_by_role(&A11yRole::Button);
        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].name, "From Labelledby");
    }

    #[test]
    fn test_th_scope_row_becomes_row_header() {
        // B2: <th scope="row"> → RowHeader
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <table>
                    <tr>
                        <th scope="row">Row Header</th>
                        <th scope="col">Column Header</th>
                        <th>Default Header</th>
                    </tr>
                </table>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let row_headers = tree.find_by_role(&A11yRole::RowHeader);
        assert_eq!(row_headers.len(), 1);
        assert_eq!(row_headers[0].name, "Row Header");

        let col_headers = tree.find_by_role(&A11yRole::ColumnHeader);
        assert_eq!(col_headers.len(), 2); // scope=col and default
    }

    #[test]
    fn test_header_inside_article_is_generic() {
        // B3: <header> inside <article> is NOT a landmark
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <header>Top Banner</header>
                <article>
                    <header>Article Header</header>
                </article>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let headers = tree.find_by_role(&A11yRole::Header);
        // Only the top-level header should be a landmark
        assert_eq!(headers.len(), 1);
    }

    #[test]
    fn test_input_type_hidden_excluded() {
        // B7: <input type="hidden"> should not appear in tree
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <input type="hidden" name="csrf" value="abc123" />
                <input type="text" placeholder="Visible" />
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let inputs = tree.find_by_role(&A11yRole::TextInput);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].name, "Visible");
    }

    #[test]
    fn test_select_multiple_is_listbox() {
        // B5: <select multiple> → ListBox
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <select multiple>
                    <option>A</option>
                    <option>B</option>
                </select>
                <select>
                    <option>C</option>
                </select>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let listboxes = tree.find_by_role(&A11yRole::ListBox);
        assert_eq!(listboxes.len(), 1);
        let comboboxes = tree.find_by_role(&A11yRole::ComboBox);
        assert_eq!(comboboxes.len(), 1);
    }

    #[test]
    fn test_presentation_role_strips_semantics() {
        // G4: role="presentation" should produce Presentation role
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <table role="presentation">
                    <tr><td>Cell</td></tr>
                </table>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        // The table should NOT appear as a Table role
        let tables = tree.find_by_role(&A11yRole::Table);
        assert!(tables.is_empty());
        // It should be Presentation
        let presentations = tree.find_by_role(&A11yRole::Presentation);
        assert!(!presentations.is_empty());
    }

    #[test]
    fn test_fieldset_legend_name() {
        // G6: <legend> provides accessible name for <fieldset>
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <fieldset>
                    <legend>Personal Info</legend>
                    <input type="text" placeholder="Name" />
                </fieldset>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let groups = tree.find_by_role(&A11yRole::Group);
        assert!(!groups.is_empty());
        assert_eq!(groups[0].name, "Personal Info");
    }

    #[test]
    fn test_progress_bar_role() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <progress value="70" max="100">70%</progress>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let bars = tree.find_by_role(&A11yRole::ProgressBar);
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].state.value_now, Some(70.0));
        assert_eq!(bars[0].state.value_max, Some(100.0));
    }

    #[test]
    fn test_details_summary_expanded() {
        // G13: <details>/<summary> support
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <details open>
                    <summary>More Info</summary>
                    <p>Details content here.</p>
                </details>
                <details>
                    <summary>Closed Section</summary>
                    <p>Hidden content.</p>
                </details>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);

        let groups = tree.find_by_role(&A11yRole::Group);
        assert_eq!(groups.len(), 2);
        // First <details open> should be expanded
        assert_eq!(groups[0].state.expanded, Some(true));
        assert_eq!(groups[0].name, "More Info");
        // Second <details> (no open) should be collapsed
        assert_eq!(groups[1].state.expanded, Some(false));
    }

    #[test]
    fn test_live_region_properties() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <div role="status" aria-live="polite" aria-atomic="true">Loading...</div>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let statuses = tree.find_by_role(&A11yRole::Status);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].state.live, Some("polite".to_string()));
        assert_eq!(statuses[0].state.atomic, Some(true));
    }

    #[test]
    fn test_aria_describedby() {
        // G10: aria-describedby
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <span id="desc">Password must be 8+ chars</span>
                <input type="password" aria-label="Password" aria-describedby="desc" />
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let inputs = tree.find_by_role(&A11yRole::TextInput);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].name, "Password");
        assert_eq!(
            inputs[0].description,
            Some("Password must be 8+ chars".to_string())
        );
    }

    #[test]
    fn test_img_empty_alt_is_presentation() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <img src="decorative.png" alt="" />
                <img src="meaningful.png" alt="Chart" />
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let images = tree.find_by_role(&A11yRole::Image);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].name, "Chart");
    }

    #[test]
    fn test_focusable_state() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <button>Click Me</button>
                <div tabindex="0">Focusable Div</div>
                <div>Not Focusable</div>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);

        // Button should be natively focusable
        let buttons = tree.find_by_role(&A11yRole::Button);
        assert_eq!(buttons.len(), 1);
        assert!(buttons[0].state.focusable);

        // Collect all Generic nodes (divs)
        let generics = tree.find_by_role(&A11yRole::Generic);
        // Find the ones that are focusable vs not
        let focusable_generics: Vec<_> = generics.iter().filter(|n| n.state.focusable).collect();
        let non_focusable_generics: Vec<_> =
            generics.iter().filter(|n| !n.state.focusable).collect();

        assert!(
            !focusable_generics.is_empty(),
            "div with tabindex=0 should be focusable"
        );
        assert!(
            !non_focusable_generics.is_empty(),
            "plain div should not be focusable"
        );
    }

    #[test]
    fn test_table_caption_name() {
        // G7: <caption> provides accessible name for <table>
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <table>
                    <caption>Monthly Sales</caption>
                    <tr><td>100</td></tr>
                </table>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let tables = tree.find_by_role(&A11yRole::Table);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "Monthly Sales");
    }

    #[test]
    fn test_separator_role() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <hr />
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let seps = tree.find_by_role(&A11yRole::Separator);
        assert_eq!(seps.len(), 1);
    }

    #[test]
    fn test_form_with_label_is_landmark() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <form aria-label="Search form">
                    <input type="search" />
                </form>
                <form>
                    <input type="text" />
                </form>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let forms = tree.find_by_role(&A11yRole::Form);
        // Only the form with aria-label should be a Form landmark
        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].name, "Search form");
    }

    #[test]
    fn test_aside_is_complementary() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <aside>Sidebar content</aside>
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let complementary = tree.find_by_role(&A11yRole::Complementary);
        assert_eq!(complementary.len(), 1);
    }

    #[test]
    fn test_input_type_image_is_button() {
        // B6: <input type="image"> → Button
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <input type="image" alt="Submit form" src="btn.png" />
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let buttons = tree.find_by_role(&A11yRole::Button);
        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].name, "Submit form");
    }

    #[test]
    fn test_checked_mixed_state() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <input type="checkbox" aria-checked="mixed" aria-label="Select all" />
            </body>
            </html>
        "#;
        let doc = mango_html::parse_html(html);
        let tree = A11yTree::build(&doc, None);
        let checkboxes = tree.find_by_role(&A11yRole::Checkbox);
        assert_eq!(checkboxes.len(), 1);
        assert_eq!(checkboxes[0].state.checked, Some(CheckedState::Mixed));
    }
}
