use mango_html::dom::{Document, ElementData, NodeData, NodeId};

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
    Heading {
        level: u8,
    },
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

/// Resolves the accessibility role from explicit ARIA or implicit HTML tag semantics.
pub fn resolve_role(tag: &str, elem: &ElementData, doc: &Document, node_id: NodeId) -> A11yRole {
    // 1. Explicit ARIA role takes precedence
    if let Some(explicit_role) = elem.get_attribute("role") {
        // ARIA spec: take the first valid token from the space-separated list
        let r = explicit_role
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if let Some(role) = match_aria_role(&r, elem) {
            return role;
        }
        // Unknown role string falls through to implicit mapping
    }

    // 2. Implicit HTML role
    match tag {
        "header" => {
            if is_inside_sectioning(doc, node_id) {
                A11yRole::Generic
            } else {
                A11yRole::Header
            }
        }
        "footer" => {
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
                "button" | "submit" | "reset" | "image" => A11yRole::Button,
                "checkbox" => A11yRole::Checkbox,
                "radio" => A11yRole::RadioButton,
                "range" => A11yRole::Slider,
                "number" => A11yRole::SpinButton,
                "search" => A11yRole::TextInput,
                "email" | "tel" | "url" | "text" | "password" => A11yRole::TextInput,
                _ => A11yRole::TextInput,
            }
        }
        "textarea" => A11yRole::TextInput,
        "select" => {
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
        "output" => A11yRole::Status,
        "progress" => A11yRole::ProgressBar,
        "meter" => A11yRole::Meter,
        "hr" => A11yRole::Separator,
        "details" => A11yRole::Group,
        "summary" => A11yRole::Button,
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
        "abbr" => A11yRole::Generic,
        "label" => A11yRole::Generic,
        _ => A11yRole::Generic,
    }
}

/// Matches an explicit ARIA role string to our enum.
pub fn match_aria_role(r: &str, elem: &ElementData) -> Option<A11yRole> {
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
        "grid" => A11yRole::Table,
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
        "presentation" | "none" => A11yRole::Presentation,
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

/// Checks whether a node is nested inside a sectioning content element
/// (`<article>`, `<aside>`, `<main>`, `<nav>`, `<section>`).
pub fn is_inside_sectioning(doc: &Document, node_id: NodeId) -> bool {
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
