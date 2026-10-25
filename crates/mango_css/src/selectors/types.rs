//! Core AST types and evaluation methods for CSS selectors.

use mango_html::dom::{Document, NodeData, NodeId};

use super::has::{get_or_parse_complex_selector, matches_simple_pattern};
use super::nav::{parent_element, prev_element_sibling};
use super::pseudo::matches_pseudo_class;

thread_local! {
    static CURRENT_SCOPE_ROOT: std::cell::Cell<Option<NodeId>> = const { std::cell::Cell::new(None) };
    static CURRENT_HOST_ELEMENT: std::cell::Cell<Option<NodeId>> = const { std::cell::Cell::new(None) };
}

/// Sets the active scoping root NodeId for evaluating `:scope` pseudo-class.
pub fn set_current_scope_root(root: Option<NodeId>) {
    CURRENT_SCOPE_ROOT.with(|c| c.set(root));
}

/// Retrieves the active scoping root NodeId if one is currently set.
pub fn current_scope_root() -> Option<NodeId> {
    CURRENT_SCOPE_ROOT.with(|c| c.get())
}

/// Sets the active shadow host NodeId for evaluating `:host` and `:host-context` pseudo-classes.
pub fn set_current_host_element(host: Option<NodeId>) {
    CURRENT_HOST_ELEMENT.with(|c| c.set(host));
}

/// Retrieves the active shadow host NodeId if one is currently set.
pub fn current_host_element() -> Option<NodeId> {
    CURRENT_HOST_ELEMENT.with(|c| c.get())
}

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
    /// Nesting selector `&` (CSS Nesting Module Level 1)
    Nesting,
}

impl SimpleSelector {
    /// Tests if this simple selector matches the given DOM element.
    pub fn matches(&self, node_id: NodeId, doc: &Document) -> bool {
        let Some(node) = doc.get(node_id) else {
            return false;
        };
        let node_id = node.id;
        let NodeData::Element(el) = &node.data else {
            return false;
        };

        match self {
            SimpleSelector::Universal => true,
            SimpleSelector::Nesting => match current_scope_root() {
                Some(root_id) => node_id.raw() == root_id.raw(),
                None => true,
            },
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
                                || v.strip_prefix(val_ref)
                                    .is_some_and(|rem| rem.starts_with('-'))
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
            SimpleSelector::PseudoElement(p) => {
                let lower = p.to_ascii_lowercase();
                if lower == "slotted" || lower.starts_with("slotted(") {
                    let is_slotted = if let Some(parent_id) = node.parent {
                        if let Some(host_id) = current_host_element()
                            && host_id == parent_id
                        {
                            true
                        } else if let Some(parent_node) = doc.get(parent_id)
                            && let NodeData::Element(parent_el) = &parent_node.data
                        {
                            parent_el.tag_name.eq_ignore_ascii_case("slot")
                                || parent_el.shadow_root.is_some()
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if is_slotted {
                        if lower.starts_with("slotted(") && lower.ends_with(')') {
                            let inner = p[8..p.len() - 1].trim();
                            if inner.is_empty() {
                                true
                            } else if let Some(list) = get_or_parse_complex_selector(inner) {
                                list.matches(node_id, doc)
                            } else {
                                matches_simple_pattern(inner, el, node_id, doc)
                            }
                        } else {
                            true
                        }
                    } else {
                        false
                    }
                } else {
                    // Pseudo-elements (::before, ::after, ::placeholder) never match standard DOM host elements
                    false
                }
            }
            SimpleSelector::PseudoClass(pseudo) => {
                matches_pseudo_class(pseudo, el, node_id, node, doc)
            }
        }
    }
}

/// A sequence of simple selectors that all apply to the same element (e.g. `div.card#hero`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompoundSelector {
    pub simple_selectors: Vec<SimpleSelector>,
}

/// Finds the shadow host that matches the given host-filtering predicate for an element's part.
/// Supports `exportparts` forwarding across nested shadow roots per CSS Shadow Parts § 3.
pub fn find_matching_part_host<F>(
    node_id: NodeId,
    part_name: &str,
    doc: &Document,
    matches_host: F,
) -> Option<NodeId>
where
    F: Fn(NodeId) -> bool,
{
    let node = doc.get(node_id)?;
    let NodeData::Element(elem) = &node.data else {
        return None;
    };
    let part_attr = elem.get_attribute("part")?;
    let mut current_parts: Vec<String> = part_attr
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if current_parts.is_empty() {
        return None;
    }

    let cur_sr = doc.get_enclosing_shadow_root(node_id)?;
    let mut cur_host = doc.get_host_for_shadow_root(cur_sr)?;

    loop {
        if current_parts
            .iter()
            .any(|p| p.eq_ignore_ascii_case(part_name))
        {
            if matches_host(cur_host) {
                return Some(cur_host);
            }
        }

        // Check if cur_host is inside an enclosing shadow root and exports parts
        let next_sr = match doc.get_enclosing_shadow_root(cur_host) {
            Some(sr) => sr,
            None => break,
        };
        let next_host = match doc.get_host_for_shadow_root(next_sr) {
            Some(h) => h,
            None => break,
        };
        let host_node = match doc.get(cur_host) {
            Some(n) => n,
            None => break,
        };
        let NodeData::Element(host_elem) = &host_node.data else {
            break;
        };
        let exportparts_attr = match host_elem.get_attribute("exportparts") {
            Some(a) => a,
            None => break,
        };

        let mut next_parts = Vec::new();
        for item in exportparts_attr.split(',') {
            let item = item.trim();
            if let Some((inner, outer)) = item.split_once(':') {
                let inner = inner.trim();
                let outer = outer.trim();
                if current_parts.iter().any(|p| p.eq_ignore_ascii_case(inner)) {
                    next_parts.push(outer.to_string());
                }
            } else if current_parts.iter().any(|p| p.eq_ignore_ascii_case(item)) {
                next_parts.push(item.to_string());
            }
        }

        if next_parts.is_empty() {
            break;
        }

        current_parts = next_parts;
        cur_host = next_host;
    }

    None
}

impl CompoundSelector {
    pub fn new(simple_selectors: Vec<SimpleSelector>) -> Self {
        Self { simple_selectors }
    }

    pub fn has_part_pseudo(&self) -> bool {
        self.simple_selectors.iter().any(|s| match s {
            SimpleSelector::PseudoElement(p) => {
                let lower = p.to_ascii_lowercase();
                lower.starts_with("part(") && lower.ends_with(')')
            }
            _ => false,
        })
    }

    /// Tests if all simple selectors in this compound selector match the node.
    pub fn matches(&self, node_id: NodeId, doc: &Document) -> bool {
        self.matches_internal(node_id, doc, true)
    }

    /// Tests if this compound selector matches in a non-subject position.
    /// Per CSS Selectors 4 § 4.1, :visited cannot match on non-subject elements.
    pub fn matches_non_subject(&self, node_id: NodeId, doc: &Document) -> bool {
        self.matches_internal(node_id, doc, false)
    }

    fn matches_internal(&self, node_id: NodeId, doc: &Document, is_subject: bool) -> bool {
        if self.simple_selectors.is_empty() {
            return false;
        }

        if !is_subject
            && self.simple_selectors.iter().any(|s| match s {
                SimpleSelector::PseudoClass(p) => p.eq_ignore_ascii_case("visited"),
                _ => false,
            })
        {
            return false;
        }

        if let Some(part_sel) = self.simple_selectors.iter().find(|s| match s {
            SimpleSelector::PseudoElement(p) => {
                let lower = p.to_ascii_lowercase();
                lower.starts_with("part(") && lower.ends_with(')')
            }
            _ => false,
        }) {
            let part_name = match part_sel {
                SimpleSelector::PseudoElement(p) => p[5..p.len() - 1].trim(),
                _ => unreachable!(),
            };

            let prefix_matches = |host_id: NodeId| {
                self.simple_selectors
                    .iter()
                    .filter(|s| match s {
                        SimpleSelector::PseudoElement(p) => {
                            let lower = p.to_ascii_lowercase();
                            !(lower.starts_with("part(") && lower.ends_with(')'))
                        }
                        _ => true,
                    })
                    .all(|s| s.matches(host_id, doc))
            };

            return find_matching_part_host(node_id, part_name, doc, prefix_matches).is_some();
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

pub(crate) fn canonical_pseudo(p: &str) -> &str {
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

        let start_node = if let Some(part_sel) =
            last_compound.simple_selectors.iter().find(|s| match s {
                SimpleSelector::PseudoElement(p) => {
                    let lower = p.to_ascii_lowercase();
                    lower.starts_with("part(") && lower.ends_with(')')
                }
                _ => false,
            }) {
            let part_name = match part_sel {
                SimpleSelector::PseudoElement(p) => p[5..p.len() - 1].trim(),
                _ => unreachable!(),
            };
            let prefix_matches = |host_id: NodeId| {
                last_compound
                    .simple_selectors
                    .iter()
                    .filter(|s| match s {
                        SimpleSelector::PseudoElement(p) => {
                            let lower = p.to_ascii_lowercase();
                            !(lower.starts_with("part(") && lower.ends_with(')'))
                        }
                        _ => true,
                    })
                    .all(|s| s.matches(host_id, doc))
            };
            if let Some(host_id) = find_matching_part_host(node_id, part_name, doc, prefix_matches)
            {
                host_id
            } else {
                return false;
            }
        } else {
            node_id
        };

        self.match_tail(
            start_node,
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
                    let is_anchor = prev_compound.simple_selectors == [SimpleSelector::Universal]
                        || prev_compound.simple_selectors == [SimpleSelector::Nesting];
                    if !is_anchor && !prev_compound.matches_non_subject(parent, doc) {
                        return false;
                    }
                    return true;
                }
                if !prev_compound.matches_non_subject(parent, doc) {
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
                        let is_anchor = prev_compound.simple_selectors
                            == [SimpleSelector::Universal]
                            || prev_compound.simple_selectors == [SimpleSelector::Nesting];
                        if Some(parent) == expected_origin
                            && (is_anchor || prev_compound.matches_non_subject(parent, doc))
                        {
                            return true;
                        }
                    } else if prev_compound.matches_non_subject(parent, doc) {
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
                    let is_anchor = prev_compound.simple_selectors == [SimpleSelector::Universal]
                        || prev_compound.simple_selectors == [SimpleSelector::Nesting];
                    if !is_anchor && !prev_compound.matches_non_subject(sibling, doc) {
                        return false;
                    }
                    return true;
                }
                if !prev_compound.matches_non_subject(sibling, doc) {
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
                        let is_anchor = prev_compound.simple_selectors
                            == [SimpleSelector::Universal]
                            || prev_compound.simple_selectors == [SimpleSelector::Nesting];
                        if Some(sibling) == expected_origin
                            && (is_anchor || prev_compound.matches_non_subject(sibling, doc))
                        {
                            return true;
                        }
                    } else if prev_compound.matches_non_subject(sibling, doc) {
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
